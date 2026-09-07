#[cfg(windows)]
pub mod ownership;
pub mod pac;
use crate::AppState;
use std::sync::atomic::Ordering;
#[derive(serde::Serialize)]
pub struct ProxyStatus {
    pub enabled: bool,
    pub mode: String,
    pub host: String,
    pub port: u16,
    pub pac_url: Option<String>,
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
        InternetSetOptionW(
            std::ptr::null_mut(),
            INTERNET_OPTION_SETTINGS_CHANGED,
            std::ptr::null_mut(),
            0,
        );
        InternetSetOptionW(
            std::ptr::null_mut(),
            INTERNET_OPTION_REFRESH,
            std::ptr::null_mut(),
            0,
        );
    }
    log::info!("Notified system of proxy settings change");
}

pub async fn clear_system_proxy() -> Result<(), String> {
    #[cfg(windows)]
    {
        ownership::restore()
    }
    #[cfg(not(windows))]
    {
        Ok(())
    }
}
pub async fn set_proxy_mode_internal(state: &AppState, mode: &str) -> Result<(), String> {
    if !["direct", "global", "rule"].contains(&mode) {
        return Err("未知代理模式".into());
    }
    let config = state.config.lock().await.clone();
    let (host, port) = config.active_endpoint();
    let (user, pass) = config.active_credentials();
    if mode == "direct" {
        clear_system_proxy().await?;
    } else {
        if !crate::utils::is_private_or_loopback(&host) {
            return Err("代理目标必须是本机或私有网络".into());
        }
        if !crate::whistle::check_whistle_alive(&host, port, &user, &pass).await {
            return Err("Whistle 尚未就绪或认证失败，未修改系统代理".into());
        }
        let pac = if mode == "rule" {
            pac::start_for_config(&config, state).await?;
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();
            Some(format!(
                "http://127.0.0.1:{}/proxy.pac?t={stamp}",
                config.pac_server_port
            ))
        } else {
            None
        };
        #[cfg(windows)]
        ownership::apply(&host, port, &config.app_settings.proxy_bypass, pac)?;
        #[cfg(not(windows))]
        {
            let _ = pac;
            return Err("此平台暂不支持安全恢复系统代理，请手动配置代理".into());
        }
    }
    *state.proxy_mode.lock().await = mode.into();
    Ok(())
}
#[tauri::command]
pub async fn cmd_set_proxy_mode(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    mode: String,
) -> Result<(), String> {
    state.user_action.store(true, Ordering::SeqCst);
    let _operation = state.operations.lock().await;
    let previous = state.config.lock().await.clone();
    let mut next = previous.clone();
    next.proxy_mode = mode.clone();
    next.app_settings.last_proxy_mode = mode.clone();
    next.save()?;
    *state.config.lock().await = next;
    if let Err(error) = set_proxy_mode_internal(&state, &mode).await {
        *state.config.lock().await = previous.clone();
        previous
            .save()
            .map_err(|rollback| format!("{error}; 配置恢复失败: {rollback}"))?;
        return Err(error);
    }
    crate::tray::update_tray(&app, &mode, *state.whistle_running.lock().await);
    Ok(())
}
#[tauri::command]
pub async fn cmd_get_proxy_status(
    state: tauri::State<'_, AppState>,
) -> Result<ProxyStatus, String> {
    let _operation = state.operations.lock().await;
    #[cfg(windows)]
    if !ownership::is_owned()? {
        *state.proxy_mode.lock().await = "direct".into();
    }
    let (host, port) = state.config.lock().await.active_endpoint();
    let mode = state.proxy_mode.lock().await.clone();
    let pac_url = if mode == "rule" {
        Some(format!(
            "http://127.0.0.1:{}/proxy.pac",
            *state.pac_server_port.lock().await
        ))
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
pub async fn cmd_clear_proxy(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    cmd_set_proxy_mode(app, state, "direct".into()).await
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
    let result = cmd_clear_proxy(app, state).await;
    let mut steps = vec![RepairStep {
        name: "恢复 WhistleBox 接管前的代理设置".into(),
        success: result.is_ok(),
        message: result
            .err()
            .unwrap_or_else(|| "已释放本应用的代理设置；保留其他软件后续修改".into()),
    }];
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let ok = std::process::Command::new("ipconfig")
            .arg("/flushdns")
            .creation_flags(0x08000000)
            .output()
            .is_ok_and(|o| o.status.success());
        steps.push(RepairStep {
            name: "刷新 DNS 缓存".into(),
            success: ok,
            message: if ok {
                "DNS 缓存已刷新"
            } else {
                "DNS 刷新失败"
            }
            .into(),
        });
    }
    Ok(RepairResult {
        success: steps.iter().all(|s| s.success),
        steps,
    })
}
