pub mod auth;
mod autostart;
mod certificates;
pub mod config;
mod proxy;
mod runtime;
mod tray;
pub mod utils;
pub mod whistle;

use config::AppConfig;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::Manager;
use tokio::sync::Mutex;

pub struct AppState {
    pub operations: Mutex<()>,
    pub user_action: AtomicBool,
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
            log::info!(
                "System uptime {}s < 120s, treating as boot start",
                uptime_secs
            );
            return true;
        }
    }
    false
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    runtime::init_logging();
    if std::env::args().any(|a| a == "--cleanup-only") {
        #[cfg(windows)]
        if let Err(e) = proxy::ownership::restore() {
            log::error!("Proxy recovery failed: {e}");
            std::process::exit(1);
        }
        return;
    }

    let config = AppConfig::load().unwrap_or_else(|e| {
        log::warn!(
            "Failed to load config, using defaults (setup_completed will be false): {}",
            e
        );
        AppConfig::default()
    });

    let minimize_to_tray_init = config.app_settings.minimize_to_tray;
    let state = AppState {
        operations: Mutex::new(()),
        user_action: AtomicBool::new(false),
        auth_proxy_port: Arc::new(Mutex::new(config.auth_proxy_port)),
        pac_server_port: Arc::new(Mutex::new(config.pac_server_port)),
        config: Arc::new(Mutex::new(config)),
        whistle_running: Arc::new(Mutex::new(false)),
        proxy_mode: Arc::new(Mutex::new("direct".to_string())),
        minimize_to_tray: Arc::new(AtomicBool::new(minimize_to_tray_init)),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            if argv.iter().any(|a| a == "--shutdown") {
                request_shutdown(app.clone(), argv.iter().any(|a| a == "--remove-autostart"));
                return;
            }
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
            certificates::cmd_install_cert,
            certificates::cmd_uninstall_cert,
            certificates::cmd_check_cert_installed,
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
            if std::env::args().any(|a| a == "--shutdown") {
                request_shutdown(
                    app.handle().clone(),
                    std::env::args().any(|a| a == "--remove-autostart"),
                );
                return Ok(());
            }
            tray::init_tray(app)?;

            let handle = app.handle().clone();

            let auto_handle = handle.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(if detect_boot_start() {
                    8
                } else {
                    2
                }))
                .await;
                let state = auto_handle.state::<AppState>();
                let _operation = state.operations.lock().await;
                if state.user_action.load(Ordering::SeqCst) {
                    return;
                }
                if let Err(e) = proxy::clear_system_proxy().await {
                    log::error!("Proxy recovery failed: {e}");
                    return;
                }
                let config = state.config.lock().await.clone();
                if config.setup_completed
                    && config.auto_start_whistle
                    && config.whistle.mode == "embedded"
                {
                    if let Err(e) = whistle::start_internal(&auto_handle, &state).await {
                        log::error!("Auto-start failed: {e}");
                    }
                }
                let (host, port) = config.active_endpoint();
                let (user, pass) = config.active_credentials();
                match auth::start_auth_proxy_internal(
                    config.auth_proxy_port,
                    host,
                    port,
                    user,
                    pass,
                    config.app_settings.local_auth_bypass,
                )
                .await
                {
                    Ok(()) => *state.auth_proxy_port.lock().await = config.auth_proxy_port,
                    Err(e) => log::error!("Auth proxy startup failed: {e}"),
                }
                if config.setup_completed
                    && config.auto_start_proxy
                    && config.app_settings.last_proxy_mode != "direct"
                {
                    if let Err(e) =
                        proxy::set_proxy_mode_internal(&state, &config.app_settings.last_proxy_mode)
                            .await
                    {
                        log::error!("Proxy restore failed: {e}");
                    }
                }
                tray::update_tray(
                    &auto_handle,
                    &state.proxy_mode.lock().await.clone(),
                    *state.whistle_running.lock().await,
                );
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

fn request_shutdown(handle: tauri::AppHandle, remove_autostart: bool) {
    tauri::async_runtime::spawn(async move {
        let state = handle.state::<AppState>();
        if let Err(e) = whistle::cmd_stop_whistle(state).await {
            log::error!("Shutdown cleanup failed: {e}");
            return;
        }
        if remove_autostart {
            if let Err(e) = autostart::cmd_set_autostart(handle.clone(), false).await {
                log::error!("Autostart cleanup failed: {e}");
                return;
            }
        }
        handle.exit(0);
    });
}
