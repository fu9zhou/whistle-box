use crate::{utils, AppState};
use base64::Engine;
use std::collections::BTreeMap;
use tokio::sync::Mutex;
static CERT_LOCK: Mutex<()> = Mutex::const_new(());

async fn current_certificate(state: &AppState) -> Result<Vec<u8>, String> {
    let config = state.config.lock().await.clone();
    let (host, port) = config.active_endpoint();
    let (user, pass) = config.active_credentials();
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;
    let mut req = client.get(utils::http_url(&host, port, "/cgi-bin/rootca"));
    if !user.is_empty() {
        req = req.basic_auth(user, Some(pass));
    }
    let mut resp = req
        .send()
        .await
        .map_err(|_| "无法获取当前实例证书，请确认 Whistle 已启动")?
        .error_for_status()
        .map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    while let Some(chunk) = resp.chunk().await.map_err(|e| e.to_string())? {
        if bytes.len() + chunk.len() > 1024 * 1024 {
            return Err("证书响应过大".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    if bytes.is_empty() {
        return Err("证书响应为空".into());
    }
    Ok(bytes)
}

#[cfg(windows)]
fn certificate_action(action: &str, cert: &[u8], owned: &str) -> Result<serde_json::Value, String> {
    use std::io::Write;
    use std::os::windows::process::CommandExt;
    let script = include_str!("../resources/certificates.ps1");
    let encoded: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let mut child = std::process::Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-EncodedCommand",
            &base64::engine::general_purpose::STANDARD.encode(encoded),
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .creation_flags(utils::CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| e.to_string())?;
    let input = serde_json::json!({"action":action,"certificate":base64::engine::general_purpose::STANDARD.encode(cert),"ownedThumbprint":owned});
    child
        .stdin
        .take()
        .ok_or("Missing certificate input pipe")?
        .write_all(input.to_string().as_bytes())
        .map_err(|e| e.to_string())?;
    let output = child.wait_with_output().map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err("证书操作失败，请检查证书格式和当前用户证书存储权限".into());
    }
    serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())
}
#[cfg(not(windows))]
fn certificate_action(
    _action: &str,
    _cert: &[u8],
    _owned: &str,
) -> Result<serde_json::Value, String> {
    Err("此平台请手动管理当前实例的 CA 证书".into())
}
fn ownership_path() -> std::path::PathBuf {
    utils::app_data_dir().join("owned-certificates.json")
}
fn owners() -> Result<BTreeMap<String, String>, String> {
    match std::fs::read(ownership_path()) {
        Ok(data) => serde_json::from_slice(&data).map_err(|e| e.to_string()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
        Err(e) => Err(e.to_string()),
    }
}
fn save_owners(owners: &BTreeMap<String, String>) -> Result<(), String> {
    std::fs::create_dir_all(utils::app_data_dir()).map_err(|e| e.to_string())?;
    let tmp = ownership_path().with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_vec(owners).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    std::fs::rename(tmp, ownership_path()).map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn cmd_check_cert_installed(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let cert = current_certificate(&state).await?;
    let value = tokio::task::spawn_blocking(move || certificate_action("status", &cert, ""))
        .await
        .map_err(|e| e.to_string())??;
    Ok(value["exists"].as_bool().unwrap_or(false))
}
#[tauri::command]
pub async fn cmd_install_cert(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let _lock = CERT_LOCK.lock().await;
    let cert = current_certificate(&state).await?;
    tokio::task::spawn_blocking(move || {
        let status = certificate_action("status", &cert, "")?;
        if status["exists"] == true {
            return Ok("当前实例证书已受信任，保留原有归属".into());
        }
        let thumbprint = status["thumbprint"]
            .as_str()
            .ok_or("Missing certificate fingerprint")?;
        let mut owned = owners()?;
        owned.insert(
            thumbprint.into(),
            base64::engine::general_purpose::STANDARD.encode(&cert),
        );
        let installed = certificate_action("install", &cert, "")?;
        if installed["added"] != true {
            return Ok("当前实例证书已受信任，保留原有归属".into());
        }
        save_owners(&owned).map_err(|e| {
            format!("证书已安装，但归属记录保存失败；已保留证书，请检查数据目录权限: {e}")
        })?;
        Ok("已安装当前实例证书，并记录指纹归属".into())
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn cmd_uninstall_cert(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let _lock = CERT_LOCK.lock().await;
    let cert = current_certificate(&state).await?;
    tokio::task::spawn_blocking(move || {
        let status = certificate_action("status", &cert, "")?;
        let thumbprint = status["thumbprint"]
            .as_str()
            .ok_or("Missing certificate fingerprint")?;
        let mut owned = owners()?;
        if !owned.contains_key(thumbprint) {
            return Err(
                "该证书不是由 WhistleBox 安装，已保留；如需删除请在系统证书管理器中操作".into(),
            );
        }
        certificate_action("remove", &cert, thumbprint)?;
        owned.remove(thumbprint);
        save_owners(&owned)?;
        Ok("已移除当前实例由 WhistleBox 安装的证书；其他证书未变".into())
    })
    .await
    .map_err(|e| e.to_string())?
}
