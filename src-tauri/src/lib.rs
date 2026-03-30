mod auth;
mod autostart;
mod config;
mod proxy;
mod tray;
pub mod utils;
mod whistle;

use config::AppConfig;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::Manager;
use tokio::sync::Mutex;

pub struct AppState {
    pub config: Arc<Mutex<AppConfig>>,
    pub whistle_running: Arc<Mutex<bool>>,
    pub proxy_mode: Arc<Mutex<String>>,
    pub auth_proxy_port: Arc<Mutex<u16>>,
    pub pac_server_port: Arc<Mutex<u16>>,
    pub minimize_to_tray: Arc<AtomicBool>,
}

fn detect_boot_start() -> bool {
    if std::env::args().any(|a| a == "--autostart") {
        return true;
    }
    #[cfg(target_os = "windows")]
    {
        extern "system" {
            fn GetTickCount64() -> u64;
        }
        let uptime_secs = unsafe { GetTickCount64() / 1000 };
        if uptime_secs < 120 {
            log::info!("System uptime {}s < 120s, treating as boot start", uptime_secs);
            return true;
        }
    }
    false
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::init();

    let config = AppConfig::load().unwrap_or_else(|e| {
        log::warn!("Failed to load config, using defaults (setup_completed will be false): {}", e);
        let default = AppConfig::default();
        if let Err(save_err) = default.save() {
            log::error!("Failed to save default config: {}", save_err);
        }
        default
    });

    let minimize_to_tray_init = config.app_settings.minimize_to_tray;
    let state = AppState {
        config: Arc::new(Mutex::new(config)),
        whistle_running: Arc::new(Mutex::new(false)),
        proxy_mode: Arc::new(Mutex::new("direct".to_string())),
        auth_proxy_port: Arc::new(Mutex::new(18900)),
        pac_server_port: Arc::new(Mutex::new(18901)),
        minimize_to_tray: Arc::new(AtomicBool::new(minimize_to_tray_init)),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            proxy::cmd_set_proxy_mode,
            proxy::cmd_get_proxy_status,
            proxy::cmd_clear_proxy,
            proxy::cmd_repair_network,
            whistle::cmd_start_whistle,
            whistle::cmd_stop_whistle,
            whistle::cmd_get_whistle_status,
            whistle::cmd_check_external_whistle,
            whistle::cmd_install_cert,
            whistle::cmd_uninstall_cert,
            whistle::cmd_check_cert_installed,
            whistle::cmd_sync_https_interception,
            auth::cmd_start_auth_proxy,
            auth::cmd_get_auth_proxy_url,
            auth::cmd_probe_auth_proxy,
            config::cmd_get_config,
            config::cmd_save_config,
            config::cmd_export_config,
            config::cmd_import_config,
            config::cmd_get_profiles,
            config::cmd_switch_profile,
            config::cmd_export_whistle_rules,
            config::cmd_import_whistle_rules,
            proxy::pac::cmd_start_pac_server,
            proxy::pac::cmd_refresh_pac,
            proxy::pac::cmd_get_pac_url,
            autostart::cmd_set_autostart,
            autostart::cmd_get_autostart,
        ])
        .setup(|app| {
            tray::init_tray(app)?;

            let handle = app.handle().clone();

            let auto_handle = handle.clone();
            tauri::async_runtime::spawn(async move {
                let state = auto_handle.state::<AppState>();

                // Always clear residual system proxy on startup to prevent proxy loops
                if let Err(e) = proxy::clear_system_proxy().await {
                    log::warn!("Failed to clear residual system proxy on startup: {}", e);
                } else {
                    log::info!("Cleared residual system proxy on startup");
                }

                let config = state.config.lock().await;
                let auto_start = config.auto_start_whistle;
                let auto_start_proxy = config.auto_start_proxy;
                let last_proxy_mode = config.app_settings.last_proxy_mode.clone();
                let conn = config.whistle.clone();
                let auth_port = config.auth_proxy_port;
                let local_auth_bypass = config.app_settings.local_auth_bypass;
                drop(config);

                if auto_start && conn.mode == "embedded" {
                    let is_boot_start = detect_boot_start();
                    let boot_delay = if is_boot_start { 8u64 } else { 2u64 };
                    tokio::time::sleep(std::time::Duration::from_secs(boot_delay)).await;
                    log::info!("Auto-starting embedded whistle (boot={}, delay={}s)...", is_boot_start, boot_delay);

                    let max_attempts = if is_boot_start { 3u32 } else { 1u32 };
                    let mut success = false;

                    for attempt in 1..=max_attempts {
                        match whistle::cmd_start_whistle(auto_handle.clone(), auto_handle.state::<AppState>()).await {
                            Ok(status) if status.uptime_check => {
                                log::info!("Whistle auto-started (attempt {}/{}): running={}, pid={}", attempt, max_attempts, status.running, status.pid);
                                let mode = if last_proxy_mode.is_empty() { "direct" } else { &last_proxy_mode };
                                tray::update_tray(&auto_handle, mode, true);
                                success = true;
                                break;
                            }
                            Ok(status) => {
                                log::warn!("Whistle spawned but health check failed (attempt {}/{}): pid={}", attempt, max_attempts, status.pid);
                                if attempt < max_attempts {
                                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                                }
                            }
                            Err(e) => {
                                log::warn!("Whistle auto-start attempt {}/{} failed: {}", attempt, max_attempts, e);
                                if attempt < max_attempts {
                                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                                }
                            }
                        }
                    }

                    if !success {
                        log::error!("Failed to auto-start whistle after {} attempts", max_attempts);
                    }
                }

                log::info!("Auto-starting auth proxy on port {}...", auth_port);
                match auth::start_auth_proxy_internal(
                    auth_port,
                    conn.host.clone(),
                    conn.port,
                    conn.username.clone(),
                    conn.password.clone(),
                    local_auth_bypass,
                ).await {
                    Ok(()) => log::info!("Auth proxy auto-started"),
                    Err(e) => log::error!("Failed to auto-start auth proxy: {}", e),
                }

                if auto_start_proxy && last_proxy_mode != "direct" && !last_proxy_mode.is_empty() {
                    log::info!("Restoring last proxy mode: {}", last_proxy_mode);
                    if last_proxy_mode == "rule" {
                        if let Err(e) = proxy::pac::cmd_start_pac_server(auto_handle.state::<AppState>()).await {
                            log::error!("Failed to start PAC server during proxy restore: {}", e);
                        }
                    }
                    match proxy::set_proxy_mode_internal(state.inner(), &last_proxy_mode).await {
                        Ok(()) => {
                            let whistle_running = *state.whistle_running.lock().await;
                            tray::update_tray(&auto_handle, &last_proxy_mode, whistle_running);
                            log::info!("Proxy mode restored to {}", last_proxy_mode);
                        }
                        Err(e) => log::error!("Failed to restore proxy mode: {}", e),
                    }
                }
            });

            tauri::async_runtime::spawn(async move {
                whistle::start_health_monitor(handle).await;
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let handle = window.app_handle().clone();
                let state = handle.state::<AppState>();
                let minimize_to_tray = state.minimize_to_tray.load(Ordering::Relaxed);

                if minimize_to_tray {
                    api.prevent_close();
                    let _ = window.hide();
                    log::info!("Window hidden to tray");
                } else {
                    api.prevent_close();
                    let window = window.clone();
                    tauri::async_runtime::spawn(async move {
                        let state = handle.state::<AppState>();
                        if let Err(e) = proxy::clear_system_proxy().await {
                            log::error!("Failed to clear proxy on exit: {}", e);
                        }
                        if let Err(e) = whistle::cmd_stop_whistle(state.clone()).await {
                            log::error!("Failed to stop whistle on exit: {}", e);
                        }
                        log::info!("Cleaned up proxy and whistle on exit");
                        let _ = window.destroy();
                    });
                }
            }
        })
        .run(tauri::generate_context!())
        .unwrap_or_else(|e| {
            log::error!("Failed to run WhistleBox: {}", e);
            eprintln!("Fatal: {}", e);
            std::process::exit(1);
        });
}
