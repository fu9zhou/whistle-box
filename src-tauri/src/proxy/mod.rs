pub mod pac;

use crate::AppState;
use sysproxy::Sysproxy;

#[derive(serde::Serialize)]
pub struct ProxyStatus {
    pub enabled: bool,
    pub mode: String,
    pub host: String,
    pub port: u16,
    pub pac_url: Option<String>,
}

#[cfg(target_os = "windows")]
const INTERNET_SETTINGS_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Internet Settings";

#[cfg(target_os = "windows")]
fn win_delete_registry_value(name: &str) {
    use winreg::enums::*;
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(key) = hkcu.open_subkey_with_flags(INTERNET_SETTINGS_KEY, KEY_SET_VALUE) {
        let _ = key.delete_value(name);
    }
}

#[cfg(target_os = "windows")]
fn win_set_registry_string(name: &str, value: &str) -> Result<(), String> {
    use winreg::enums::*;
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu
        .open_subkey_with_flags(INTERNET_SETTINGS_KEY, KEY_SET_VALUE)
        .map_err(|e| format!("Failed to open registry key: {}", e))?;
    key.set_value(name, &value)
        .map_err(|e| format!("Failed to set registry value {}: {}", name, e))?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn win_set_registry_dword(name: &str, value: u32) -> Result<(), String> {
    use winreg::enums::*;
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu
        .open_subkey_with_flags(INTERNET_SETTINGS_KEY, KEY_SET_VALUE)
        .map_err(|e| format!("Failed to open registry key: {}", e))?;
    key.set_value(name, &value)
        .map_err(|e| format!("Failed to set registry value {}: {}", name, e))?;
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn notify_proxy_change() {
    #[link(name = "wininet")]
    extern "system" {
        fn InternetSetOptionW(
            h_internet: *mut std::ffi::c_void,
            dw_option: u32,
            lp_buffer: *mut std::ffi::c_void,
            dw_buffer_length: u32,
        ) -> i32;
    }
    const INTERNET_OPTION_SETTINGS_CHANGED: u32 = 39;
    const INTERNET_OPTION_REFRESH: u32 = 37;
    unsafe {
        InternetSetOptionW(std::ptr::null_mut(), INTERNET_OPTION_SETTINGS_CHANGED, std::ptr::null_mut(), 0);
        InternetSetOptionW(std::ptr::null_mut(), INTERNET_OPTION_REFRESH, std::ptr::null_mut(), 0);
    }
    log::info!("Notified system of proxy settings change");
}

#[cfg(target_os = "windows")]
pub fn refresh_pac_system_proxy(pac_port: u16) -> Result<(), String> {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let pac_url = format!("http://127.0.0.1:{}/proxy.pac?t={}", pac_port, timestamp);
    win_set_registry_string("AutoConfigURL", &pac_url)?;
    notify_proxy_change();
    Ok(())
}

pub async fn clear_system_proxy() -> Result<(), String> {
    let proxy = Sysproxy::get_system_proxy().unwrap_or(Sysproxy {
        enable: false,
        host: String::new(),
        port: 0,
        bypass: String::new(),
    });
    let proxy = Sysproxy { enable: false, ..proxy };
    proxy.set_system_proxy().map_err(|e| format!("Failed to clear system proxy: {}", e))?;

    #[cfg(target_os = "windows")]
    {
        win_delete_registry_value("AutoConfigURL");
        let _ = win_set_registry_string("ProxyServer", "");
    }

    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        for service in &["Wi-Fi", "Ethernet"] {
            let _ = Command::new("networksetup")
                .args(["-setautoproxystate", service, "off"])
                .output();
        }
    }

    Ok(())
}

fn set_global_proxy(host: &str, port: u16, bypass: &str) -> Result<(), String> {
    let proxy = Sysproxy {
        enable: true,
        host: host.to_string(),
        port,
        bypass: bypass.to_string(),
    };
    proxy.set_system_proxy().map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn set_proxy_mode_internal(
    state: &AppState,
    mode: &str,
) -> Result<(), String> {
    let (proxy_host, proxy_port, bypass) = {
        let config = state.config.lock().await;
        let (h, p) = config.active_endpoint();
        if mode != "direct" {
            let is_safe = if let Ok(ip) = h.parse::<std::net::IpAddr>() {
                match ip {
                    std::net::IpAddr::V4(v4) => v4.is_loopback() || v4.is_private(),
                    std::net::IpAddr::V6(v6) => v6.is_loopback(),
                }
            } else {
                h == "localhost"
            };
            if !is_safe {
                return Err(format!("Refusing to set system proxy to non-local host: {}", h));
            }
        }
        let b = if config.app_settings.proxy_bypass.is_empty() {
            "localhost;127.*;10.*;172.16.*;192.168.*;<local>".to_string()
        } else {
            config.app_settings.proxy_bypass.clone()
        };
        (h, p, b)
    };

    match mode {
        "global" => {
            set_global_proxy(&proxy_host, proxy_port, &bypass)?;

            #[cfg(target_os = "windows")]
            {
                win_delete_registry_value("AutoConfigURL");
            }

            #[cfg(target_os = "macos")]
            {
                use std::process::Command;
                let services = ["Wi-Fi", "Ethernet"];
                for service in &services {
                    let _ = Command::new("networksetup")
                        .args(["-setautoproxystate", service, "off"])
                        .output();
                }
            }
        }
        "rule" => {
            let pac_port = *state.pac_server_port.lock().await;
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();
            let pac_url = format!("http://127.0.0.1:{}/proxy.pac?t={}", pac_port, timestamp);

            #[cfg(target_os = "windows")]
            {
                win_set_registry_string("AutoConfigURL", &pac_url)?;
                win_set_registry_dword("ProxyEnable", 0)?;
                notify_proxy_change();
            }

            #[cfg(target_os = "macos")]
            {
                use std::process::Command;
                let services = ["Wi-Fi", "Ethernet"];
                for service in &services {
                    let _ = Command::new("networksetup")
                        .args(["-setautoproxyurl", service, &pac_url])
                        .output();
                    let _ = Command::new("networksetup")
                        .args(["-setautoproxystate", service, "on"])
                        .output();
                }
            }

            #[cfg(target_os = "linux")]
            {
                use std::process::Command;
                let _ = Command::new("gsettings")
                    .args(["set", "org.gnome.system.proxy", "mode", "auto"])
                    .output();
                let _ = Command::new("gsettings")
                    .args([
                        "set",
                        "org.gnome.system.proxy",
                        "autoconfig-url",
                        &pac_url,
                    ])
                    .output();
            }
        }
        "direct" => {
            clear_system_proxy().await?;

            #[cfg(target_os = "windows")]
            {
                win_delete_registry_value("AutoConfigURL");
            }

            #[cfg(target_os = "macos")]
            {
                use std::process::Command;
                let services = ["Wi-Fi", "Ethernet"];
                for service in &services {
                    let _ = Command::new("networksetup")
                        .args(["-setautoproxystate", service, "off"])
                        .output();
                }
            }

            #[cfg(target_os = "linux")]
            {
                use std::process::Command;
                let _ = Command::new("gsettings")
                    .args(["set", "org.gnome.system.proxy", "mode", "none"])
                    .output();
            }
        }
        _ => return Err(format!("Unknown proxy mode: {}", mode)),
    }

    let mut proxy_mode = state.proxy_mode.lock().await;
    *proxy_mode = mode.to_string();
    Ok(())
}

#[tauri::command]
pub async fn cmd_set_proxy_mode(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    mode: String,
) -> Result<(), String> {
    set_proxy_mode_internal(&state, &mode).await?;

    {
        let mut config = state.config.lock().await;
        config.proxy_mode = mode.clone();
        config.app_settings.last_proxy_mode = mode.clone();
        if let Err(e) = config.save() {
            log::warn!("Failed to persist proxy mode to disk: {}", e);
        }
    }

    let whistle_running = *state.whistle_running.lock().await;
    crate::tray::update_tray(&app, &mode, whistle_running);

    Ok(())
}

#[tauri::command]
pub async fn cmd_get_proxy_status(
    state: tauri::State<'_, AppState>,
) -> Result<ProxyStatus, String> {
    let (host, port) = {
        let config = state.config.lock().await;
        config.active_endpoint()
    };
    let mode = state.proxy_mode.lock().await.clone();

    let pac_url = if mode == "rule" {
        let pac_port = *state.pac_server_port.lock().await;
        Some(format!("http://127.0.0.1:{}/proxy.pac", pac_port))
    } else {
        None
    };

    Ok(ProxyStatus {
        enabled: mode != "direct",
        mode,
        host,
        port,
        pac_url,
    })
}

#[tauri::command]
pub async fn cmd_clear_proxy(app: tauri::AppHandle, state: tauri::State<'_, AppState>) -> Result<(), String> {
    cmd_set_proxy_mode(app, state, "direct".to_string()).await
}

#[derive(serde::Serialize)]
pub struct RepairResult {
    pub success: bool,
    pub steps: Vec<RepairStep>,
}

#[derive(serde::Serialize)]
pub struct RepairStep {
    pub name: String,
    pub success: bool,
    pub message: String,
}

#[tauri::command]
pub async fn cmd_repair_network(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<RepairResult, String> {
    let mut steps = Vec::new();

    // Step 1: Set proxy mode to direct
    match cmd_set_proxy_mode(app, state, "direct".to_string()).await {
        Ok(()) => steps.push(RepairStep {
            name: "切换到直连模式".into(),
            success: true,
            message: "已切换到直连模式".into(),
        }),
        Err(e) => steps.push(RepairStep {
            name: "切换到直连模式".into(),
            success: false,
            message: format!("切换失败: {}", e),
        }),
    }

    // Step 2: Flush DNS
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        use std::os::windows::process::CommandExt;
        let dns_result = Command::new("ipconfig")
            .args(["/flushdns"])
            .creation_flags(0x08000000)
            .output();
        steps.push(RepairStep {
            name: "刷新 DNS 缓存".into(),
            success: dns_result.map(|o| o.status.success()).unwrap_or(false),
            message: "DNS 缓存已刷新".into(),
        });
    }

    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        let dns_result = Command::new("dscacheutil")
            .args(["-flushcache"])
            .output();
        let _ = Command::new("sudo")
            .args(["killall", "-HUP", "mDNSResponder"])
            .output();
        steps.push(RepairStep {
            name: "刷新 DNS 缓存".into(),
            success: dns_result.is_ok(),
            message: "DNS 缓存已刷新".into(),
        });
    }

    #[cfg(target_os = "linux")]
    {
        use std::process::Command;
        let dns_result = Command::new("systemd-resolve")
            .args(["--flush-caches"])
            .output();
        let dns_ok = dns_result.map(|o| o.status.success()).unwrap_or(false);
        steps.push(RepairStep {
            name: "刷新 DNS 缓存".into(),
            success: dns_ok,
            message: if dns_ok { "DNS 缓存已刷新".into() } else { "DNS 刷新失败（systemd-resolve 不可用）".into() },
        });
    }

    // Step 3: Clear PAC auto-config URL and proxy settings
    #[cfg(target_os = "windows")]
    {
        win_delete_registry_value("AutoConfigURL");
        let proxy_enable_result = win_set_registry_dword("ProxyEnable", 0);
        let proxy_server_result = win_set_registry_string("ProxyServer", "");
        let all_ok = proxy_enable_result.is_ok() && proxy_server_result.is_ok();
        steps.push(RepairStep {
            name: "清除 PAC 自动配置".into(),
            success: all_ok,
            message: if all_ok { "PAC 配置和代理设置已清除".into() } else { "部分清除失败".into() },
        });
    }

    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        let services = ["Wi-Fi", "Ethernet"];
        let mut mac_ok = true;
        for service in &services {
            if Command::new("networksetup")
                .args(["-setautoproxystate", service, "off"])
                .output()
                .is_err()
            {
                mac_ok = false;
            }
            let _ = Command::new("networksetup")
                .args(["-setwebproxystate", service, "off"])
                .output();
            let _ = Command::new("networksetup")
                .args(["-setsecurewebproxystate", service, "off"])
                .output();
            let _ = Command::new("networksetup")
                .args(["-setsocksfirewallproxystate", service, "off"])
                .output();
        }
        steps.push(RepairStep {
            name: "清除 macOS 网络代理".into(),
            success: mac_ok,
            message: if mac_ok { "代理已清除" } else { "部分清除失败" }.into(),
        });
    }

    #[cfg(target_os = "linux")]
    {
        use std::process::Command;
        let gsettings_result = Command::new("gsettings")
            .args(["set", "org.gnome.system.proxy", "mode", "none"])
            .output();
        steps.push(RepairStep {
            name: "清除 Linux 系统代理".into(),
            success: gsettings_result.is_ok(),
            message: if gsettings_result.is_ok() { "代理已清除" } else { "清除失败" }.into(),
        });
    }

    // Step 4: Clear system proxy via sysproxy
    match clear_system_proxy().await {
        Ok(()) => steps.push(RepairStep {
            name: "清除系统 HTTP 代理".into(),
            success: true,
            message: "系统代理已清除".into(),
        }),
        Err(e) => steps.push(RepairStep {
            name: "清除系统 HTTP 代理".into(),
            success: false,
            message: format!("清除失败: {}", e),
        }),
    }

    let all_ok = steps.iter().all(|s| s.success);
    Ok(RepairResult {
        success: all_ok,
        steps,
    })
}
