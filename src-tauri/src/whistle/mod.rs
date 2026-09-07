pub mod process;
use crate::{utils, AppState};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Manager};
use tokio::time::{sleep, Duration};
static WANTED: AtomicBool = AtomicBool::new(false);
#[derive(Clone, serde::Serialize)]
pub struct WhistleStatus {
    pub running: bool,
    pub mode: String,
    pub host: String,
    pub port: u16,
    pub pid: u32,
    pub uptime_check: bool,
}
fn client() -> &'static reqwest::Client {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(3))
            .build()
            .expect("HTTP client initialization failed")
    })
}
pub fn valid_server_info(info: &serde_json::Value) -> bool {
    info["ec"].as_u64() == Some(0)
        && info["server"]["pid"].as_u64().is_some_and(|p| p > 0)
        && info["server"]["version"]
            .as_str()
            .is_some_and(|s| !s.is_empty())
        && info["server"]["port"].as_u64().is_some_and(|p| p > 0)
}
pub async fn server_info(
    host: &str,
    port: u16,
    user: &str,
    pass: &str,
) -> Option<serde_json::Value> {
    let mut req = client().get(utils::http_url(host, port, "/cgi-bin/server-info"));
    if !user.is_empty() {
        req = req.basic_auth(user, Some(pass));
    }
    let info = req
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .json::<serde_json::Value>()
        .await
        .ok()?;
    if !valid_server_info(&info) {
        return None;
    }
    // server-info is public in Whistle 2.10.2. Check a protected endpoint as well.
    let mut req = client().get(utils::http_url(host, port, "/"));
    if !user.is_empty() {
        req = req.basic_auth(user, Some(pass));
    }
    if !req.send().await.ok()?.status().is_success() {
        return None;
    }
    Some(info)
}
pub async fn check_whistle_alive(host: &str, port: u16, user: &str, pass: &str) -> bool {
    server_info(host, port, user, pass).await.is_some()
}
pub async fn sync_https_interception(
    host: &str,
    port: u16,
    user: &str,
    pass: &str,
    enable: bool,
) -> Result<(), String> {
    let mut req = client()
        .post(utils::http_url(
            host,
            port,
            "/cgi-bin/intercept-https-connects",
        ))
        .form(&[("interceptHttpsConnects", if enable { "1" } else { "0" })]);
    if !user.is_empty() {
        req = req.basic_auth(user, Some(pass));
    }
    let response = req
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;
    let result = response
        .json::<serde_json::Value>()
        .await
        .map_err(|e| e.to_string())?;
    if result["ec"].as_i64() != Some(0) {
        return Err("Whistle 拒绝 HTTPS 抓包设置".into());
    }
    Ok(())
}
pub async fn start_internal(handle: &AppHandle, state: &AppState) -> Result<WhistleStatus, String> {
    let config = state.config.lock().await.clone();
    let conn = &config.whistle;
    let (host, port) = config.active_endpoint();
    let (user, pass) = config.active_credentials();
    let pid = if conn.mode == "embedded" {
        let same = {
            let mut owned = process::OWNED.lock().await;
            owned.as_mut().is_some_and(|p| {
                p.connection == *conn && p.child.try_wait().ok().flatten().is_none()
            })
        };
        if same && check_whistle_alive(&host, port, &user, &pass).await {
            process::OWNED
                .lock()
                .await
                .as_ref()
                .map(|p| p.child.id())
                .unwrap_or(0)
        } else {
            *state.whistle_running.lock().await = false;
            tokio::time::timeout(Duration::from_secs(30), process::start(handle, conn))
                .await
                .map_err(|_| "内置 Whistle 启动超时")??
        }
    } else {
        if !check_whistle_alive(&host, port, &user, &pass).await {
            return Err("无法连接外部 Whistle，或认证失败".into());
        }
        0
    };
    if conn.mode == "embedded" {
        WANTED.store(true, Ordering::SeqCst);
        if let Err(e) =
            sync_https_interception(&host, port, &user, &pass, conn.intercept_https).await
        {
            log::warn!("HTTPS interception setup failed: {e}");
        }
    }
    *state.whistle_running.lock().await = true;
    Ok(WhistleStatus {
        running: true,
        mode: conn.mode.clone(),
        host,
        port,
        pid,
        uptime_check: true,
    })
}
pub async fn stop_internal(state: &AppState) -> Result<(), String> {
    WANTED.store(false, Ordering::SeqCst);
    crate::proxy::clear_system_proxy().await?;
    *state.proxy_mode.lock().await = "direct".into();
    process::stop().await?;
    *state.whistle_running.lock().await = false;
    Ok(())
}
#[tauri::command]
pub async fn cmd_start_whistle(
    handle: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<WhistleStatus, String> {
    state.user_action.store(true, Ordering::SeqCst);
    let _operation = state.operations.lock().await;
    let result = start_internal(&handle, &state).await;
    if result.is_err() && crate::proxy::clear_system_proxy().await.is_ok() {
        *state.proxy_mode.lock().await = "direct".into();
    }
    crate::tray::update_tray(
        &handle,
        &state.proxy_mode.lock().await.clone(),
        result.is_ok(),
    );
    result
}
#[tauri::command]
pub async fn cmd_stop_whistle(state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.user_action.store(true, Ordering::SeqCst);
    let _operation = state.operations.lock().await;
    stop_internal(&state).await
}
#[tauri::command]
pub async fn cmd_get_whistle_status(
    state: tauri::State<'_, AppState>,
) -> Result<WhistleStatus, String> {
    let _operation = state.operations.lock().await;
    let config = state.config.lock().await.clone();
    let (host, port) = config.active_endpoint();
    let (user, pass) = config.active_credentials();
    let mut pid = 0;
    if config.whistle.mode == "embedded" {
        let mut owned = process::OWNED.lock().await;
        if let Some(process) = owned.as_mut() {
            if process
                .child
                .try_wait()
                .map_err(|e| e.to_string())?
                .is_none()
            {
                pid = process.child.id();
            }
        }
    }
    let alive = (config.whistle.mode == "external" || pid > 0)
        && check_whistle_alive(&host, port, &user, &pass).await;
    *state.whistle_running.lock().await = alive;
    Ok(WhistleStatus {
        running: if config.whistle.mode == "embedded" {
            pid > 0
        } else {
            alive
        },
        mode: config.whistle.mode,
        host,
        port,
        pid,
        uptime_check: alive,
    })
}
#[tauri::command]
pub async fn cmd_check_external_whistle(
    host: String,
    port: u16,
    username: String,
    password: String,
) -> Result<bool, String> {
    if !utils::is_private_or_loopback(&host) || port == 0 {
        return Err("仅支持本机或私有网络地址及有效端口".into());
    }
    Ok(check_whistle_alive(&host, port, &username, &password).await)
}
#[tauri::command]
pub async fn cmd_sync_https_interception(
    state: tauri::State<'_, AppState>,
    enable: bool,
) -> Result<(), String> {
    let _operation = state.operations.lock().await;
    let config = state.config.lock().await.clone();
    let (host, port) = config.active_endpoint();
    let (user, pass) = config.active_credentials();
    sync_https_interception(&host, port, &user, &pass, enable).await
}
pub async fn start_health_monitor(handle: AppHandle) {
    let mut failures = 0;
    loop {
        sleep(Duration::from_secs(8)).await;
        let state = handle.state::<AppState>();
        let _operation = state.operations.lock().await;
        if !WANTED.load(Ordering::SeqCst) && *state.proxy_mode.lock().await == "direct" {
            failures = 0;
            continue;
        }
        let config = state.config.lock().await.clone();

        let (host, port) = config.active_endpoint();
        let (user, pass) = config.active_credentials();
        if check_whistle_alive(&host, port, &user, &pass).await {
            failures = 0;
            continue;
        }
        failures += 1;
        if failures < 3 {
            continue;
        }
        failures = 0;
        let recovery = if config.whistle.mode == "embedded" && WANTED.load(Ordering::SeqCst) {
            start_internal(&handle, &state).await.map(|_| ())
        } else {
            Err("外部 Whistle 已断开".into())
        };
        if let Err(e) = recovery {
            log::error!("Embedded recovery failed: {e}");
            *state.whistle_running.lock().await = false;
            if crate::proxy::clear_system_proxy().await.is_ok() {
                *state.proxy_mode.lock().await = "direct".into();
            }
        }
        crate::tray::update_tray(
            &handle,
            &state.proxy_mode.lock().await.clone(),
            *state.whistle_running.lock().await,
        );
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn rejects_generic_success_responses() {
        assert!(!super::valid_server_info(&serde_json::json!({"ok":true})));
        assert!(super::valid_server_info(
            &serde_json::json!({"ec":0,"server":{"pid":42,"port":18899,"version":"2.10.2"}})
        ));
    }
}
