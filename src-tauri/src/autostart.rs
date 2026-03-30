#[cfg(target_os = "windows")]
#[tauri::command]
pub async fn cmd_set_autostart(_app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    use winreg::enums::*;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (run_key, _) = hkcu
        .create_subkey(r"Software\Microsoft\Windows\CurrentVersion\Run")
        .map_err(|e| format!("Failed to open registry: {}", e))?;

    if enabled {
        let exe_path = std::env::current_exe()
            .map_err(|e| format!("Failed to get exe path: {}", e))?;
        let exe_str = format!("\"{}\" --autostart", exe_path.display());
        log::info!("Setting autostart registry: WhistleBox = {}", exe_str);
        run_key
            .set_value("WhistleBox", &exe_str)
            .map_err(|e| format!("Failed to set autostart: {}", e))?;
    } else {
        log::info!("Removing autostart registry: WhistleBox");
        let _ = run_key.delete_value("WhistleBox");
    }
    Ok(())
}

#[cfg(target_os = "windows")]
#[tauri::command]
pub async fn cmd_get_autostart(_app: tauri::AppHandle) -> Result<bool, String> {
    use winreg::enums::*;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let run_key = match hkcu.open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Run") {
        Ok(k) => k,
        Err(_) => return Ok(false),
    };
    let result = run_key.get_value::<String, _>("WhistleBox");
    match &result {
        Ok(_) => log::debug!("Autostart: enabled"),
        Err(_) => log::debug!("Autostart: not configured"),
    }
    Ok(result.is_ok())
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
pub async fn cmd_set_autostart(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let manager = app.autolaunch();
    if enabled {
        manager.enable().map_err(|e| e.to_string())?;
    } else {
        manager.disable().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
pub async fn cmd_get_autostart(app: tauri::AppHandle) -> Result<bool, String> {
    use tauri_plugin_autostart::ManagerExt;
    let manager = app.autolaunch();
    manager.is_enabled().map_err(|e| e.to_string())
}
