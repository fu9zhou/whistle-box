use crate::AppState;
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

const TRAY_ID: &str = "whistlebox-tray";

fn generate_icon_rgba(r: u8, g: u8, b: u8) -> Vec<u8> {
    let size: usize = 32;
    let center = size as f64 / 2.0;
    let outer_r = 14.0;
    let inner_r = 11.0;
    let mut pixels = vec![0u8; size * size * 4];

    for y in 0..size {
        for x in 0..size {
            let dx = x as f64 - center + 0.5;
            let dy = y as f64 - center + 0.5;
            let dist = (dx * dx + dy * dy).sqrt();
            let idx = (y * size + x) * 4;

            if dist <= inner_r {
                pixels[idx] = r;
                pixels[idx + 1] = g;
                pixels[idx + 2] = b;
                pixels[idx + 3] = 255;
            } else if dist <= outer_r {
                let alpha = ((outer_r - dist) / (outer_r - inner_r) * 255.0) as u8;
                pixels[idx] = r;
                pixels[idx + 1] = g;
                pixels[idx + 2] = b;
                pixels[idx + 3] = alpha;
            }
        }
    }

    let letter_w: [(usize, usize); 30] = [
        (11,10),(12,11),(13,12),(14,13),(15,14),(16,13),(17,12),(18,11),(19,10),
        (11,11),(13,13),(14,14),(15,15),(16,14),(17,13),(19,11),
        (12,12),(18,12),
        (14,15),(16,15),
        (15,16),
        (11,12),(19,12),
        (12,13),(18,13),
        (13,14),(17,14),
        (14,16),(16,16),
        (15,17),
    ];

    for &(px, py) in &letter_w {
        if px < size && py < size {
            let idx = (py * size + px) * 4;
            pixels[idx] = 255;
            pixels[idx + 1] = 255;
            pixels[idx + 2] = 255;
            pixels[idx + 3] = 255;
        }
    }

    pixels
}

fn get_icon_for_mode(mode: &str) -> Image<'static> {
    let rgba = match mode {
        "global" => generate_icon_rgba(16, 185, 129),
        "rule" => generate_icon_rgba(59, 130, 246),
        _ => generate_icon_rgba(128, 128, 128),
    };
    Image::new_owned(rgba, 32, 32)
}

fn get_tooltip(mode: &str, whistle_running: bool) -> String {
    let mode_label = match mode {
        "global" => "全局代理",
        "rule" => "规则代理",
        _ => "直连模式",
    };
    if whistle_running {
        format!("WhistleBox - {}", mode_label)
    } else {
        format!("WhistleBox - {} (Whistle 未启动)", mode_label)
    }
}

fn build_menu(app: &AppHandle, current_mode: &str) -> tauri::Result<Menu<tauri::Wry>> {
    let menu = Menu::new(app)?;

    let show_item = MenuItem::with_id(app, "show_window", "显示主窗口", true, None::<&str>)?;
    menu.append(&show_item)?;

    menu.append(&PredefinedMenuItem::separator(app)?)?;

    let direct_item = CheckMenuItem::with_id(
        app, "mode_direct", "直连模式", true, current_mode == "direct", None::<&str>,
    )?;
    let rule_item = CheckMenuItem::with_id(
        app, "mode_rule", "规则代理", true, current_mode == "rule", None::<&str>,
    )?;
    let global_item = CheckMenuItem::with_id(
        app, "mode_global", "全局代理", true, current_mode == "global", None::<&str>,
    )?;
    menu.append(&direct_item)?;
    menu.append(&rule_item)?;
    menu.append(&global_item)?;

    menu.append(&PredefinedMenuItem::separator(app)?)?;

    let whistle_item = MenuItem::with_id(app, "open_whistle", "打开 Whistle 界面", true, None::<&str>)?;
    menu.append(&whistle_item)?;

    let settings_item = MenuItem::with_id(app, "open_settings", "设置", true, None::<&str>)?;
    menu.append(&settings_item)?;

    menu.append(&PredefinedMenuItem::separator(app)?)?;

    let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    menu.append(&quit_item)?;

    Ok(menu)
}

pub fn init_tray(app: &tauri::App) -> tauri::Result<()> {
    let handle = app.handle().clone();
    let icon = get_icon_for_mode("direct");
    let tooltip = get_tooltip("direct", false);
    let menu = build_menu(app.handle(), "direct")?;

    let menu_handle = handle.clone();
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .tooltip(tooltip)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app_handle, event| {
            handle_menu_event(app_handle, event.id().as_ref());
        })
        .on_tray_icon_event(move |tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let handle = menu_handle.clone();
                tauri::async_runtime::spawn(async move {
                    handle_left_click(&handle).await;
                });
                let _ = tray.app_handle();
            }
        })
        .build(app)?;

    Ok(())
}

fn handle_menu_event(app: &AppHandle, id: &str) {
    match id {
        "show_window" => {
            show_main_window(app);
        }
        "mode_direct" | "mode_rule" | "mode_global" => {
            let mode = match id {
                "mode_direct" => "direct",
                "mode_rule" => "rule",
                "mode_global" => "global",
                _ => return,
            };
            let handle = app.clone();
            tauri::async_runtime::spawn(async move {
                switch_proxy_mode(&handle, mode).await;
            });
        }
        "open_settings" => {
            show_main_window(app);
            let _ = app.emit("navigate", "settings");
        }
        "open_whistle" => {
            let handle = app.clone();
            tauri::async_runtime::spawn(async move {
                let state = handle.state::<AppState>();
                let (local_auth_bypass, whistle_mode, w_host, w_port, ext_host, ext_port) = {
                    let config = state.config.lock().await;
                    (
                        config.app_settings.local_auth_bypass,
                        config.whistle.mode.clone(),
                        config.whistle.host.clone(),
                        config.whistle.port,
                        config.app_settings.external_host.clone(),
                        config.app_settings.external_port,
                    )
                };
                let auth_port = *state.auth_proxy_port.lock().await;
                let (url, target_host) = if local_auth_bypass {
                    (format!("http://127.0.0.1:{}", auth_port), "127.0.0.1".to_string())
                } else if whistle_mode == "embedded" {
                    (format!("http://{}:{}", w_host, w_port), w_host)
                } else {
                    (format!("http://{}:{}", ext_host, ext_port), ext_host)
                };

                if !crate::utils::is_private_or_loopback(&target_host) {
                    log::warn!("Refusing to open non-local URL from tray: {}", url);
                    return;
                }

                #[cfg(target_os = "windows")]
                { let _ = std::process::Command::new("cmd").args(["/c", "start", "", &url]).creation_flags(0x08000000).spawn(); }
                #[cfg(target_os = "macos")]
                { let _ = std::process::Command::new("open").arg(&url).spawn(); }
                #[cfg(target_os = "linux")]
                { let _ = std::process::Command::new("xdg-open").arg(&url).spawn(); }
            });
        }
        "quit" => {
            let handle = app.clone();
            tauri::async_runtime::spawn(async move {
                let state = handle.state::<AppState>();
                let _ = crate::proxy::clear_system_proxy().await;
                let _ = crate::whistle::cmd_stop_whistle(state.clone()).await;
                handle.exit(0);
            });
        }
        _ => {}
    }
}

async fn handle_left_click(app: &AppHandle) {
    let state = app.state::<AppState>();
    let action = {
        let config = state.config.lock().await;
        config.app_settings.tray_click_action.clone()
    };
    let current_mode = state.proxy_mode.lock().await.clone();

    match action.as_str() {
        "toggle_direct_rule" => {
            let next = if current_mode == "rule" { "direct" } else { "rule" };
            switch_proxy_mode(app, next).await;
        }
        "toggle_direct_global" => {
            let next = if current_mode == "global" { "direct" } else { "global" };
            switch_proxy_mode(app, next).await;
        }
        "cycle" => {
            let next = match current_mode.as_str() {
                "direct" => "rule",
                "rule" => "global",
                _ => "direct",
            };
            switch_proxy_mode(app, next).await;
        }
        _ => {
            toggle_main_window(app);
        }
    }
}

fn toggle_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
        } else {
            let _ = window.show();
            let _ = window.unminimize();
            let _ = window.set_focus();
        }
    }
}

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

async fn switch_proxy_mode(app: &AppHandle, mode: &str) {
    let state = app.state::<AppState>();

    if mode == "rule" {
        let _ = crate::proxy::pac::cmd_start_pac_server(app.state::<AppState>()).await;
    }

    match crate::proxy::set_proxy_mode_internal(state.inner(), mode).await {
        Ok(()) => {
            log::info!("Tray: switched proxy mode to {}", mode);
        }
        Err(e) => {
            log::error!("Tray: failed to switch proxy mode: {}", e);
            return;
        }
    }

    {
        let mut config = state.config.lock().await;
        config.proxy_mode = mode.to_string();
        config.app_settings.last_proxy_mode = mode.to_string();
        if let Err(e) = config.save() {
            log::warn!("Failed to persist proxy mode to disk: {}", e);
        }
    }

    let whistle_running = *state.whistle_running.lock().await;
    update_tray(app, mode, whistle_running);
}

pub fn update_tray(app: &AppHandle, mode: &str, whistle_running: bool) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let icon = get_icon_for_mode(mode);
        let tooltip = get_tooltip(mode, whistle_running);
        let _ = tray.set_icon(Some(icon));
        let _ = tray.set_tooltip(Some(&tooltip));

        if let Ok(menu) = build_menu(app, mode) {
            let _ = tray.set_menu(Some(menu));
        }
    }
}
