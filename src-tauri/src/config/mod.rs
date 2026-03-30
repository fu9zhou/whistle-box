use serde::{Deserialize, Serialize};
use std::fs;
use std::net::IpAddr;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

fn default_true() -> bool { true }
fn default_false() -> bool { false }
fn default_theme() -> String { "dark".to_string() }
fn default_tray_click() -> String { "show_window".to_string() }
fn default_external_host() -> String { "127.0.0.1".to_string() }
fn default_external_port() -> u16 { 8899 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyRule {
    pub id: String,
    pub pattern: String,
    pub enabled: bool,
    pub comment: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub rules: Vec<ProxyRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    fn config_path() -> PathBuf {
        let base = dirs::config_dir()
            .or_else(|| std::env::var("APPDATA").ok().map(PathBuf::from))
            .unwrap_or_else(|| {
                log::warn!("config_dir() unavailable, falling back to exe parent");
                std::env::current_exe()
                    .ok()
                    .and_then(|p| p.parent().map(|p| p.to_path_buf()))
                    .unwrap_or_else(|| PathBuf::from("."))
            });
        let dir = base.join("WhistleBox");
        if let Err(e) = fs::create_dir_all(&dir) {
            log::error!("Failed to create config dir {:?}: {}", dir, e);
        }
        dir.join("config.json")
    }

    pub fn load() -> Result<Self, String> {
        let path = Self::config_path();
        if !path.exists() {
            log::info!("Config file not found, creating default: {:?}", path);
            let default = Self::default();
            default.save()?;
            return Ok(default);
        }
        let data = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        match serde_json::from_str::<Self>(&data) {
            Ok(config) => {
                validate_config(&config)?;
                log::info!("Config loaded: setup_completed={}", config.setup_completed);
                Ok(config)
            }
            Err(e) => {
                log::error!("Config deserialization failed: {}. Attempting partial recovery.", e);
                let backup_path = path.with_extension(format!(
                    "corrupt.{}.json",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs()
                ));
                if let Err(be) = fs::copy(&path, &backup_path) {
                    log::warn!("Failed to backup corrupt config: {}", be);
                } else {
                    log::info!("Backed up corrupt config to {:?}", backup_path);
                }
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&data) {
                    let setup_completed = val.get("setup_completed")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    if setup_completed {
                        log::info!("Recovered setup_completed=true from corrupted config");
                        let mut default = Self::default();
                        default.setup_completed = true;
                        default.save()?;
                        return Ok(default);
                    }
                }
                Err(e.to_string())
            }
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::config_path();
        let data = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        let tmp_path = path.with_extension("json.tmp");
        fs::write(&tmp_path, &data).map_err(|e| format!("Failed to write temp config: {}", e))?;
        fs::rename(&tmp_path, &path).map_err(|e| format!("Failed to rename config: {}", e))
    }

    pub fn get_active_profile(&self) -> Option<&Profile> {
        self.profiles.iter().find(|p| p.id == self.active_profile_id)
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
            (self.app_settings.external_host.clone(), self.app_settings.external_port)
        }
    }

    pub fn active_credentials(&self) -> (String, String) {
        if self.whistle.mode == "embedded" {
            (self.whistle.username.clone(), self.whistle.password.clone())
        } else {
            (self.app_settings.external_username.clone(), self.app_settings.external_password.clone())
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
        return Err("Embedded mode only allows loopback host (127.0.0.1/::1/localhost)".to_string());
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

#[tauri::command]
pub async fn cmd_save_config(
    state: tauri::State<'_, AppState>,
    config: AppConfig,
) -> Result<(), String> {
    validate_config(&config)?;
    config.save()?;
    state
        .minimize_to_tray
        .store(config.app_settings.minimize_to_tray, std::sync::atomic::Ordering::Relaxed);
    let mut current = state.config.lock().await;
    *current = config;
    Ok(())
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
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<AppConfig, String> {
    validate_user_path(&path, &[".json"])?;
    let data = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let config: AppConfig = serde_json::from_str(&data).map_err(|e| e.to_string())?;
    validate_config(&config)?;
    config.save()?;
    let mut current = state.config.lock().await;
    *current = config.clone();
    Ok(config)
}

#[tauri::command]
pub async fn cmd_get_profiles(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<Profile>, String> {
    let config = state.config.lock().await;
    Ok(config.profiles.clone())
}

#[tauri::command]
pub async fn cmd_switch_profile(
    state: tauri::State<'_, AppState>,
    profile_id: String,
) -> Result<(), String> {
    let mut config = state.config.lock().await;
    if config.profiles.iter().any(|p| p.id == profile_id) {
        config.active_profile_id = profile_id;
        config.save()?;
        Ok(())
    } else {
        Err("Profile not found".to_string())
    }
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

    let whistle_url = format!("http://{}:{}/cgi-bin/rules/export", host, port);
    let client = reqwest::Client::builder().no_proxy().build()
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

    let whistle_url = format!("http://{}:{}/cgi-bin/rules/import", host, port);
    let client = reqwest::Client::builder().no_proxy().build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

    let is_json = data.trim_start().starts_with('{');
    let mut req = if is_json {
        client.post(&whistle_url)
            .header("Content-Type", "application/json")
            .body(data)
    } else {
        client.post(&whistle_url)
            .header("Content-Type", "text/plain")
            .body(data)
    };
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
            let em = json.get("em").and_then(|v| v.as_str()).unwrap_or("Unknown error");
            return Err(format!("Whistle import failed: {}", em));
        }
    }

    log::info!("Whistle rules imported successfully");
    Ok(())
}
