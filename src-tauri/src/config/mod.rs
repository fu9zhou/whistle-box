use serde::{Deserialize, Serialize};
use std::fs;
use std::net::IpAddr;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WhistleConnection {
    pub mode: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub socks_port: u16,
    #[serde(default)]
    pub timeout: u32,
    #[serde(default)]
    pub storage_path: String,
    #[serde(default)]
    pub upstream_proxy: String,
    #[serde(default = "default_true")]
    pub intercept_https: bool,
}

impl Default for WhistleConnection {
    fn default() -> Self {
        Self {
            mode: "embedded".to_string(),
            host: "127.0.0.1".to_string(),
            port: 18899,
            username: String::new(),
            password: String::new(),
            socks_port: 0,
            timeout: 60,
            storage_path: String::new(),
            upstream_proxy: String::new(),
            intercept_https: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppSettings {
    #[serde(default = "default_true")]
    pub minimize_to_tray: bool,
    #[serde(default)]
    pub start_on_boot: bool,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default)]
    pub proxy_bypass: String,
    #[serde(default = "default_false")]
    pub local_auth_bypass: bool,
    #[serde(default = "default_tray_click")]
    pub tray_click_action: String,
    #[serde(default)]
    pub last_proxy_mode: String,
    #[serde(default = "default_external_host")]
    pub external_host: String,
    #[serde(default = "default_external_port")]
    pub external_port: u16,
    #[serde(default)]
    pub external_username: String,
    #[serde(default)]
    pub external_password: String,
}

fn default_true() -> bool {
    true
}
fn default_false() -> bool {
    false
}
fn default_theme() -> String {
    "dark".to_string()
}
fn default_tray_click() -> String {
    "show_window".to_string()
}
fn default_external_host() -> String {
    "127.0.0.1".to_string()
}
fn default_external_port() -> u16 {
    8899
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProxyRule {
    pub id: String,
    pub pattern: String,
    pub enabled: bool,
    pub comment: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub rules: Vec<ProxyRule>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppConfig {
    pub whistle: WhistleConnection,
    pub proxy_mode: String,
    pub active_profile_id: String,
    pub profiles: Vec<Profile>,
    pub auto_start_whistle: bool,
    pub auto_start_proxy: bool,
    pub pac_server_port: u16,
    pub auth_proxy_port: u16,
    #[serde(default)]
    pub app_settings: AppSettings,
    #[serde(default)]
    pub setup_completed: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        let default_profile = Profile {
            id: "default".to_string(),
            name: "默认配置".to_string(),
            rules: vec![],
        };

        Self {
            whistle: WhistleConnection::default(),
            proxy_mode: "direct".to_string(),
            active_profile_id: "default".to_string(),
            profiles: vec![default_profile],
            auto_start_whistle: true,
            auto_start_proxy: false,
            pac_server_port: 18901,
            auth_proxy_port: 18900,
            app_settings: AppSettings::default(),
            setup_completed: false,
        }
    }
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            minimize_to_tray: true,
            start_on_boot: false,
            theme: "dark".to_string(),
            proxy_bypass: "localhost;127.*;10.*;172.16.*;192.168.*;<local>".to_string(),
            local_auth_bypass: false,
            tray_click_action: "show_window".to_string(),
            last_proxy_mode: "direct".to_string(),
            external_host: "127.0.0.1".to_string(),
            external_port: 8899,
            external_username: String::new(),
            external_password: String::new(),
        }
    }
}

impl AppConfig {
    pub fn config_path() -> PathBuf {
        let dir = crate::utils::app_data_dir();
        let _ = fs::create_dir_all(&dir);
        dir.join("config.json")
    }

    pub fn load() -> Result<Self, String> {
        let path = Self::config_path();
        if !path.exists() {
            let config = Self::default();
            config.save()?;
            return Ok(config);
        }
        let data = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let parsed = serde_json::from_str::<Self>(&data)
            .map_err(|e| e.to_string())
            .and_then(|config| {
                validate_config(&config)?;
                Ok(config)
            });
        if parsed.is_err() {
            let id = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            fs::copy(&path, path.with_extension(format!("invalid.{id}.json")))
                .map_err(|e| format!("Cannot preserve invalid config: {e}"))?;
        }
        parsed
    }

    pub fn save(&self) -> Result<(), String> {
        static SAVE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _save = SAVE_LOCK.lock().map_err(|e| e.to_string())?;
        let path = Self::config_path();
        let data = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        let tmp_path = path.with_extension("json.tmp");
        fs::write(&tmp_path, &data).map_err(|e| format!("Failed to write temp config: {}", e))?;
        fs::rename(&tmp_path, &path).map_err(|e| format!("Failed to rename config: {}", e))
    }

    pub fn get_active_profile(&self) -> Option<&Profile> {
        self.profiles
            .iter()
            .find(|p| p.id == self.active_profile_id)
    }

    pub fn get_active_rules(&self) -> Vec<&ProxyRule> {
        self.get_active_profile()
            .map(|p| p.rules.iter().filter(|r| r.enabled).collect())
            .unwrap_or_default()
    }

    pub fn active_endpoint(&self) -> (String, u16) {
        if self.whistle.mode == "embedded" {
            (self.whistle.host.clone(), self.whistle.port)
        } else {
            (
                self.app_settings.external_host.clone(),
                self.app_settings.external_port,
            )
        }
    }

    pub fn active_credentials(&self) -> (String, String) {
        if self.whistle.mode == "embedded" {
            (self.whistle.username.clone(), self.whistle.password.clone())
        } else {
            (
                self.app_settings.external_username.clone(),
                self.app_settings.external_password.clone(),
            )
        }
    }
}

fn validate_user_path(path: &str, allowed_extensions: &[&str]) -> Result<(), String> {
    let p = std::path::Path::new(path);
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
    let ext_lower = format!(".{}", ext.to_lowercase());
    if !allowed_extensions.iter().any(|&a| a == ext_lower) {
        return Err(format!("Unsupported file extension: {}", ext_lower));
    }
    if let Some(parent) = p.parent() {
        if !parent.exists() {
            return Err("Parent directory does not exist".to_string());
        }
        if let Ok(canonical) = parent.canonicalize() {
            let canonical_str = canonical.to_string_lossy();
            if canonical_str.contains("..") {
                return Err("Path traversal detected".to_string());
            }
        }
    }
    if path.contains("..") {
        return Err("Path traversal not allowed".to_string());
    }
    Ok(())
}

fn is_valid_loopback_host(host: &str) -> bool {
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        return ip.is_loopback();
    }
    false
}

fn is_private_or_loopback_host(host: &str) -> bool {
    crate::utils::is_private_or_loopback(host)
}

fn validate_upstream_proxy(proxy: &str) -> Result<(), String> {
    if proxy.is_empty() {
        return Ok(());
    }
    if proxy.len() > 200 {
        return Err("Upstream proxy value too long".to_string());
    }
    if proxy.contains(' ') || proxy.contains(';') || proxy.contains('|') || proxy.contains('&') {
        return Err("Upstream proxy contains invalid characters".to_string());
    }
    let normalized = if proxy.contains("://") {
        proxy.to_string()
    } else {
        format!("http://{}", proxy)
    };
    let parsed = reqwest::Url::parse(&normalized)
        .map_err(|_| "Upstream proxy must be a valid URL or host:port".to_string())?;
    match parsed.scheme() {
        "http" | "https" | "socks5" => Ok(()),
        _ => Err("Upstream proxy only supports http/https/socks5".to_string()),
    }
}

fn validate_config(config: &AppConfig) -> Result<(), String> {
    if !["embedded", "external"].contains(&config.whistle.mode.as_str()) {
        return Err(format!("Invalid whistle mode: {}", config.whistle.mode));
    }
    if !["direct", "global", "rule"].contains(&config.proxy_mode.as_str()) {
        return Err(format!("Invalid proxy mode: {}", config.proxy_mode));
    }
    if config.whistle.port == 0 {
        return Err("Whistle port cannot be 0".to_string());
    }
    if config.pac_server_port == 0 || config.auth_proxy_port == 0 {
        return Err("PAC/Auth proxy port cannot be 0".to_string());
    }
    if config.app_settings.external_port == 0 {
        return Err("External whistle port cannot be 0".to_string());
    }
    let mut ports = std::collections::BTreeSet::new();
    let mut local_ports = vec![config.auth_proxy_port, config.pac_server_port];
    if config.whistle.mode == "embedded" {
        local_ports.push(config.whistle.port);
        if config.whistle.socks_port > 0 {
            local_ports.push(config.whistle.socks_port);
        }
    } else if is_valid_loopback_host(&config.app_settings.external_host) {
        local_ports.push(config.app_settings.external_port);
    }
    if local_ports.into_iter().any(|port| !ports.insert(port)) {
        return Err("Whistle、SOCKS、认证代理和 PAC 的本地监听端口不得重复".into());
    }
    let mut profile_ids = std::collections::BTreeSet::new();
    for profile in &config.profiles {
        if !profile_ids.insert(&profile.id) {
            return Err("Profile id must be unique".into());
        }
        for rule in &profile.rules {
            if !crate::proxy::pac::is_valid_domain_pattern(&rule.pattern) {
                return Err(format!("无效域名规则: {}", rule.pattern));
            }
        }
    }
    if config.profiles.is_empty() {
        return Err("At least one profile is required".to_string());
    }
    if !config
        .profiles
        .iter()
        .any(|p| p.id == config.active_profile_id)
    {
        return Err("Active profile id is not found in profiles".to_string());
    }

    if config.whistle.mode == "embedded" && !is_valid_loopback_host(&config.whistle.host) {
        return Err(
            "Embedded mode only allows loopback host (127.0.0.1/::1/localhost)".to_string(),
        );
    }
    if !is_private_or_loopback_host(&config.app_settings.external_host) {
        return Err("External whistle host must be localhost or private network IP".to_string());
    }
    if !config.whistle.storage_path.is_empty() {
        if config.whistle.storage_path.contains("..") {
            return Err("Storage path must not contain '..'".to_string());
        }
        if config.whistle.storage_path.len() > 260 {
            return Err("Storage path too long".to_string());
        }
    }
    validate_upstream_proxy(&config.whistle.upstream_proxy)?;

    Ok(())
}

use crate::AppState;

#[tauri::command]
pub async fn cmd_get_config(state: tauri::State<'_, AppState>) -> Result<AppConfig, String> {
    let config = state.config.lock().await;
    Ok(config.clone())
}

// Apply only fields changed relative to the caller's snapshot. Conflicting edits
// must be retried, never silently overwrite a newer tray/background update.
fn merge_changes(
    current: &mut serde_json::Value,
    base: &serde_json::Value,
    next: &serde_json::Value,
) -> Result<(), String> {
    if next == base {
        return Ok(());
    }
    if let (Some(current), Some(base), Some(next)) =
        (current.as_object_mut(), base.as_object(), next.as_object())
    {
        for (key, value) in next {
            if let (Some(target), Some(previous)) = (current.get_mut(key), base.get(key)) {
                merge_changes(target, previous, value)?;
            } else {
                current.insert(key.clone(), value.clone());
            }
        }
    } else {
        if current != base && current != next {
            return Err("配置已被其他操作更新，请重新加载后重试".into());
        }
        *current = next.clone();
    }
    Ok(())
}

async fn apply_services(
    handle: &tauri::AppHandle,
    state: &AppState,
    old: &AppConfig,
    new: &AppConfig,
    was_running: bool,
    proxy_mode: &str,
) -> Result<(), String> {
    let target_changed = old.whistle != new.whistle
        || old.active_endpoint() != new.active_endpoint()
        || old.active_credentials() != new.active_credentials();
    if target_changed {
        crate::proxy::clear_system_proxy().await?;
        *state.proxy_mode.lock().await = "direct".into();
        if old.whistle != new.whistle {
            crate::whistle::stop_internal(state).await?;
        }
    }
    if was_running && new.whistle.mode == "embedded" {
        crate::whistle::start_internal(handle, state).await?;
    }
    let (host, port) = new.active_endpoint();
    let (user, pass) = new.active_credentials();
    crate::auth::start_auth_proxy_internal(
        new.auth_proxy_port,
        host,
        port,
        user,
        pass,
        new.app_settings.local_auth_bypass,
    )
    .await?;
    *state.auth_proxy_port.lock().await = new.auth_proxy_port;
    if proxy_mode == "rule" {
        crate::proxy::pac::start_for_config(new, state).await?;
    } else {
        if old.pac_server_port != new.pac_server_port {
            crate::proxy::pac::stop_pac_server().await;
        }
        let rules = new
            .get_active_rules()
            .into_iter()
            .map(|r| (r.pattern.clone(), r.enabled))
            .collect::<Vec<_>>();
        let (host, port) = new.active_endpoint();
        crate::proxy::pac::update_pac_content(&rules, &host, port).await;
    }
    if proxy_mode != "direct" {
        crate::proxy::set_proxy_mode_internal(state, proxy_mode).await?;
    }
    *state.pac_server_port.lock().await = new.pac_server_port;
    state.minimize_to_tray.store(
        new.app_settings.minimize_to_tray,
        std::sync::atomic::Ordering::Relaxed,
    );
    Ok(())
}

pub async fn apply_config(
    handle: &tauri::AppHandle,
    state: &AppState,
    next: AppConfig,
    base: Option<AppConfig>,
) -> Result<AppConfig, String> {
    state
        .user_action
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let _operation = state.operations.lock().await;
    let old = state.config.lock().await.clone();
    let next = if let Some(base) = base {
        let mut merged = serde_json::to_value(&old).map_err(|e| e.to_string())?;
        merge_changes(
            &mut merged,
            &serde_json::to_value(base).map_err(|e| e.to_string())?,
            &serde_json::to_value(next).map_err(|e| e.to_string())?,
        )?;
        serde_json::from_value(merged).map_err(|e| e.to_string())?
    } else {
        next
    };
    validate_config(&next)?;
    let was_running = crate::whistle::process::OWNED.lock().await.is_some();
    #[cfg(windows)]
    if !crate::proxy::ownership::is_owned()? {
        *state.proxy_mode.lock().await = "direct".into();
    }
    let mode = state.proxy_mode.lock().await.clone();
    next.save()?;
    *state.config.lock().await = next.clone();
    if let Err(error) = apply_services(handle, state, &old, &next, was_running, &mode).await {
        *state.config.lock().await = old.clone();
        let rollback_file = old.save();
        let rollback_services =
            apply_services(handle, state, &next, &old, was_running, &mode).await;
        return Err(format!(
            "应用配置失败: {error}; 配置恢复: {}; 服务恢复: {}",
            rollback_file.err().unwrap_or_else(|| "成功".into()),
            rollback_services.err().unwrap_or_else(|| "成功".into())
        ));
    }
    crate::tray::update_tray(
        handle,
        &state.proxy_mode.lock().await.clone(),
        *state.whistle_running.lock().await,
    );
    Ok(next)
}

#[tauri::command]
pub async fn cmd_save_config(
    handle: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    config: AppConfig,
    base_config: Option<AppConfig>,
) -> Result<AppConfig, String> {
    apply_config(&handle, &state, config, base_config).await
}

#[tauri::command]
pub async fn cmd_export_config(
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<(), String> {
    validate_user_path(&path, &[".json"])?;
    let config = state.config.lock().await;
    let data = serde_json::to_string_pretty(&*config).map_err(|e| e.to_string())?;
    fs::write(&path, data).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn cmd_import_config(
    handle: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<AppConfig, String> {
    validate_user_path(&path, &[".json"])?;
    let data = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let config: AppConfig = serde_json::from_str(&data).map_err(|e| e.to_string())?;
    apply_config(&handle, &state, config, None).await
}

#[tauri::command]
pub async fn cmd_get_profiles(state: tauri::State<'_, AppState>) -> Result<Vec<Profile>, String> {
    let config = state.config.lock().await;
    Ok(config.profiles.clone())
}

#[tauri::command]
pub async fn cmd_switch_profile(
    handle: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    profile_id: String,
) -> Result<(), String> {
    let base = state.config.lock().await.clone();
    let mut next = base.clone();
    next.active_profile_id = profile_id;
    apply_config(&handle, &state, next, Some(base)).await?;
    Ok(())
}

#[tauri::command]
pub async fn cmd_export_whistle_rules(
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<(), String> {
    validate_user_path(&path, &[".txt", ".json"])?;
    let config = state.config.lock().await;
    let (host, port) = config.active_endpoint();
    let (username, password) = config.active_credentials();
    drop(config);

    let whistle_url = crate::utils::http_url(&host, port, "/cgi-bin/rules/export");
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;
    let mut req = client.get(&whistle_url);
    if !username.is_empty() {
        req = req.basic_auth(&username, Some(&password));
    }

    let resp = req.send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("Whistle returned status {}", resp.status()));
    }
    let body = resp.text().await.map_err(|e| e.to_string())?;
    fs::write(&path, body).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn cmd_import_whistle_rules(
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<(), String> {
    validate_user_path(&path, &[".txt", ".json"])?;
    let data = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let config = state.config.lock().await;
    let (host, port) = config.active_endpoint();
    let (username, password) = config.active_credentials();
    drop(config);

    let whistle_url = crate::utils::http_url(&host, port, "/cgi-bin/rules/import");
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

    let rules: serde_json::Value = if data.trim_start().starts_with('{') {
        serde_json::from_str(&data).map_err(|e| format!("无效 JSON 规则文件: {e}"))?
    } else {
        serde_json::json!({"Default": data})
    };
    if !rules.is_object() {
        return Err("规则文件必须是 JSON 对象或纯文本".into());
    }
    let part = reqwest::multipart::Part::text(rules.to_string())
        .file_name("rules.json")
        .mime_str("application/json")
        .map_err(|e| e.to_string())?;
    let mut req = client
        .post(&whistle_url)
        .multipart(reqwest::multipart::Form::new().part("rules", part));
    if !username.is_empty() {
        req = req.basic_auth(&username, Some(&password));
    }

    let resp = req.send().await.map_err(|e| e.to_string())?;
    let status = resp.status();
    let resp_body = resp.text().await.unwrap_or_default();

    if !status.is_success() {
        return Err(format!("Whistle returned status {}: {}", status, resp_body));
    }

    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&resp_body) {
        if json.get("ec").and_then(|v| v.as_i64()).unwrap_or(0) != 0 {
            let em = json
                .get("em")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown error");
            return Err(format!("Whistle import failed: {}", em));
        }
    }

    log::info!("Whistle rules imported successfully");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_configuration_is_preserved_with_backup() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../.tooling/config-backup-test");
        std::fs::create_dir_all(&root).unwrap();
        let previous = std::env::var_os("WHISTLEBOX_DATA_DIR");
        std::env::set_var("WHISTLEBOX_DATA_DIR", &root);
        let config = AppConfig {
            active_profile_id: "missing-profile".into(),
            ..Default::default()
        };
        let data = serde_json::to_string(&config).unwrap();
        std::fs::write(root.join("config.json"), &data).unwrap();
        let result = AppConfig::load();
        if let Some(previous) = previous {
            std::env::set_var("WHISTLEBOX_DATA_DIR", previous);
        } else {
            std::env::remove_var("WHISTLEBOX_DATA_DIR");
        }
        assert!(result.is_err());
        assert_eq!(
            std::fs::read_to_string(root.join("config.json")).unwrap(),
            data
        );
        assert!(std::fs::read_dir(root).unwrap().any(|p| p
            .unwrap()
            .file_name()
            .to_string_lossy()
            .contains("invalid.")));
    }
    #[test]
    fn concurrent_config_edits_merge_without_losing_newer_fields() {
        let base = serde_json::json!({"proxy":"direct","port":8899});
        let mut current = serde_json::json!({"proxy":"global","port":8899});
        merge_changes(
            &mut current,
            &base,
            &serde_json::json!({"proxy":"direct","port":9000}),
        )
        .unwrap();
        assert_eq!(current, serde_json::json!({"proxy":"global","port":9000}));
        assert!(merge_changes(
            &mut current,
            &base,
            &serde_json::json!({"proxy":"rule","port":8899})
        )
        .is_err());
    }
    #[test]
    fn refuses_overlapping_local_service_ports() {
        let mut config = AppConfig::default();
        config.auth_proxy_port = config.whistle.port;
        assert!(validate_config(&config).is_err());
        let mut config = AppConfig::default();
        config.whistle.socks_port = config.pac_server_port;
        assert!(validate_config(&config).is_err());
    }

    #[test]
    fn defaults_and_remote_same_number_ports_are_valid() {
        assert!(validate_config(&AppConfig::default()).is_ok());
        let mut config = AppConfig::default();
        config.whistle.mode = "external".into();
        config.app_settings.external_host = "192.168.1.2".into();
        config.app_settings.external_port = config.auth_proxy_port;
        assert!(validate_config(&config).is_ok());
    }
}
