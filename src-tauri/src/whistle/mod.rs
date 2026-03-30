use crate::AppState;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU16, AtomicU32, Ordering};
use tauri::AppHandle;
use tauri::Manager;
use tokio::time::{sleep, Duration};

use crate::utils::{kill_process, kill_process_by_port, kill_whistle_by_session, is_port_free};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
use crate::utils::CREATE_NO_WINDOW;

static WHISTLE_PID: AtomicU32 = AtomicU32::new(0);
static WHISTLE_PORT: AtomicU16 = AtomicU16::new(0);
static WHISTLE_LAST_START: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[derive(serde::Serialize)]
pub struct WhistleStatus {
    pub running: bool,
    pub mode: String,
    pub host: String,
    pub port: u16,
    pub pid: u32,
    pub uptime_check: bool,
}

fn health_check_client() -> &'static reqwest::Client {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(3))
            .no_proxy()
            .pool_max_idle_per_host(2)
            .build()
            .unwrap_or_default()
    })
}

pub async fn check_whistle_alive(host: &str, port: u16, username: &str, password: &str) -> bool {
    let url = format!("http://{}:{}/cgi-bin/server-info", host, port);
    let client = health_check_client();

    let mut req = client.get(&url);
    if !username.is_empty() {
        req = req.basic_auth(username, Some(password));
    }

    req.send().await.map(|r| r.status().is_success()).unwrap_or(false)
}

pub async fn start_health_monitor(handle: AppHandle) {
    let mut consecutive_failures: u32 = 0;

    loop {
        sleep(Duration::from_secs(8)).await;

        let state = handle.state::<AppState>();
        let running = *state.whistle_running.lock().await;

        if !running {
            consecutive_failures = 0;
            continue;
        }

        // Don't check health too soon after a start/restart (give it 30s to settle)
        let last_start = WHISTLE_LAST_START.load(Ordering::Relaxed);
        if last_start > 0 && now_secs().saturating_sub(last_start) < 30 {
            continue;
        }

        let config = state.config.lock().await;
        let is_embedded = config.whistle.mode == "embedded";
        let (host, port) = config.active_endpoint();
        let (username, password) = config.active_credentials();
        drop(config);

        let alive = check_whistle_alive(&host, port, &username, &password).await;

        if alive {
            consecutive_failures = 0;
            continue;
        }

        consecutive_failures += 1;

        if is_embedded {
            // Require 3 consecutive failures before restarting to avoid spurious restarts
            if consecutive_failures < 3 {
                log::warn!(
                    "Whistle health check failed ({}/3 before restart), {}:{}",
                    consecutive_failures, host, port
                );
                continue;
            }

            log::warn!("Embedded whistle failed 3 consecutive health checks, attempting restart...");
            consecutive_failures = 0;

            let pid = WHISTLE_PID.load(Ordering::Relaxed);
            if pid > 0 {
                kill_process(pid);
            }

            let whistle_conn = {
                let config = state.config.lock().await;
                config.whistle.clone()
            };
            let intercept_https = whistle_conn.intercept_https;
            let w_host = whistle_conn.host.clone();
            let w_port = whistle_conn.port;
            let w_user = whistle_conn.username.clone();
            let w_pass = whistle_conn.password.clone();
            match start_whistle_internal(&handle, &whistle_conn).await {
                Ok(_) => {
                    // Verify the restart actually worked
                    let post_alive = check_whistle_alive(&w_host, w_port, &w_user, &w_pass).await;
                    if post_alive {
                        sync_https_interception(&w_host, w_port, &w_user, &w_pass, intercept_https).await;
                        let mode = state.proxy_mode.lock().await.clone();
                        crate::tray::update_tray(&handle, &mode, true);
                        log::info!("Whistle restarted successfully via keep-alive");
                    } else {
                        log::error!("Whistle restart completed but health check still failing");
                        let mut running = state.whistle_running.lock().await;
                        *running = false;
                        drop(running);
                        let mode = state.proxy_mode.lock().await.clone();
                        crate::tray::update_tray(&handle, &mode, false);
                    }
                }
                Err(e) => {
                    let mut running = state.whistle_running.lock().await;
                    *running = false;
                    drop(running);
                    let mode = state.proxy_mode.lock().await.clone();
                    crate::tray::update_tray(&handle, &mode, false);
                    log::error!("Failed to restart whistle: {}", e);
                }
            }
        } else {
            if consecutive_failures >= 3 {
                log::warn!("Whistle health check failed 3 times for {}:{}", host, port);
                let mut running = state.whistle_running.lock().await;
                *running = false;
                drop(running);
                let mode = state.proxy_mode.lock().await.clone();
                crate::tray::update_tray(&handle, &mode, false);
                consecutive_failures = 0;
            }
        }
    }
}


fn find_node_binary(handle: &AppHandle) -> Result<PathBuf, String> {
    let triple = std::env::var("TAURI_ENV_TARGET_TRIPLE")
        .unwrap_or_else(|_| "x86_64-pc-windows-msvc".to_string());

    #[cfg(target_os = "windows")]
    let candidates = vec![
        format!("node-{}.exe", triple),
        "node.exe".to_string(),
    ];
    #[cfg(not(target_os = "windows"))]
    let candidates = vec![
        format!("node-{}", triple),
        "node".to_string(),
    ];

    for node_name in &candidates {
        // Dev mode: look in src-tauri/binaries/
        let dev_path = std::env::current_dir()
            .unwrap_or_default()
            .join("binaries")
            .join(node_name);
        if dev_path.exists() {
            log::info!("Found node binary (dev): {}", dev_path.display());
            return Ok(dev_path);
        }

        // Prod: relative to the exe
        if let Ok(exe_dir) = std::env::current_exe() {
            let exe_parent = exe_dir.parent().unwrap_or(exe_dir.as_path());
            let prod_path = exe_parent.join(node_name);
            if prod_path.exists() {
                log::info!("Found node binary (prod): {}", prod_path.display());
                return Ok(prod_path);
            }
        }

        // Try resource_dir
        if let Ok(res_dir) = handle.path().resource_dir() {
            let res_path = res_dir.join(node_name);
            if res_path.exists() {
                log::info!("Found node binary (resource): {}", res_path.display());
                return Ok(res_path);
            }
        }
    }

    Err(format!("Node binary not found in any expected location (tried: {:?})", candidates))
}

fn find_whistle_script(handle: &AppHandle) -> Result<PathBuf, String> {
    let whistle_relative = PathBuf::from("resources")
        .join("whistle")
        .join("node_modules")
        .join("whistle")
        .join("bin")
        .join("whistle.js");

    // Dev mode: look in src-tauri/resources/
    let dev_path = std::env::current_dir()
        .unwrap_or_default()
        .join(&whistle_relative);
    if dev_path.exists() {
        log::info!("Found whistle script (dev): {}", dev_path.display());
        return Ok(dev_path);
    }

    // Prod: relative to exe
    if let Ok(exe_dir) = std::env::current_exe() {
        let exe_parent = exe_dir.parent().unwrap_or(exe_dir.as_path());
        let prod_path = exe_parent.join(&whistle_relative);
        if prod_path.exists() {
            log::info!("Found whistle script (prod): {}", prod_path.display());
            return Ok(prod_path);
        }
    }

    // Try resource_dir
    if let Ok(res_dir) = handle.path().resource_dir() {
        let res_path = res_dir.join(&whistle_relative);
        if res_path.exists() {
            log::info!("Found whistle script (resource): {}", res_path.display());
            return Ok(res_path);
        }
    }

    Err("Whistle script not found in any expected location".to_string())
}

use crate::config::WhistleConnection;

pub async fn sync_https_interception(host: &str, port: u16, username: &str, password: &str, enable: bool) {
    let client = health_check_client();
    let url = format!("http://{}:{}/cgi-bin/intercept-https-connects", host, port);
    let value = if enable { "1" } else { "0" };
    let mut req = client
        .post(&url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(format!("interceptHttpsConnects={}", value));
    if !username.is_empty() {
        req = req.basic_auth(username, Some(password));
    }

    match req.send().await {
        Ok(resp) => {
            log::info!("HTTPS interception sync (enable={}) response: {}", enable, resp.status());
        }
        Err(e) => {
            log::warn!("Failed to sync HTTPS interception state: {}", e);
        }
    }
}

async fn start_whistle_internal(
    handle: &AppHandle,
    conn: &WhistleConnection,
) -> Result<u32, String> {
    let old_pid = WHISTLE_PID.load(Ordering::Relaxed);
    if old_pid > 0 {
        log::info!("Killing previously tracked whistle process (PID {})", old_pid);
        kill_process(old_pid);
        WHISTLE_PID.store(0, Ordering::Relaxed);
        sleep(Duration::from_millis(500)).await;
    }

    let old_port = WHISTLE_PORT.load(Ordering::Relaxed);
    if old_port > 0 && old_port != conn.port {
        log::info!("Port changed ({} -> {}), killing old port process", old_port, conn.port);
        kill_process_by_port(old_port);
    }

    kill_whistle_by_session();
    kill_process_by_port(conn.port);
    sleep(Duration::from_millis(800)).await;

    for _ in 0..10 {
        if is_port_free(conn.port) {
            break;
        }
        log::info!("Port {} still occupied, waiting...", conn.port);
        kill_process_by_port(conn.port);
        sleep(Duration::from_millis(500)).await;
    }
    if !is_port_free(conn.port) {
        return Err(format!("Port {} is still occupied after cleanup, cannot start Whistle", conn.port));
    }

    let node_bin = find_node_binary(handle)?;
    let whistle_script = find_whistle_script(handle)?;

    let mut cmd = std::process::Command::new(&node_bin);
    cmd.arg(whistle_script.to_string_lossy().to_string());
    cmd.arg("start");
    cmd.arg("--host");
    cmd.arg(&conn.host);
    cmd.arg("-p");
    cmd.arg(conn.port.to_string());

    if !conn.username.is_empty() {
        cmd.arg("-n");
        cmd.arg(&conn.username);
    }
    if !conn.password.is_empty() {
        cmd.arg("-w");
        cmd.arg(&conn.password);
    }
    if conn.socks_port > 0 {
        cmd.arg("--socksPort");
        cmd.arg(conn.socks_port.to_string());
    }
    if conn.timeout > 0 && conn.timeout != 60 {
        cmd.arg("-t");
        cmd.arg(conn.timeout.to_string());
    }
    if !conn.storage_path.is_empty() {
        cmd.arg("-D");
        cmd.arg(&conn.storage_path);
    }
    if conn.mode == "embedded" {
        if conn.storage_path.is_empty() {
            let home = dirs::home_dir().or_else(|| {
                std::env::var("USERPROFILE").ok().map(PathBuf::from)
            }).unwrap_or_else(|| {
                log::warn!("home_dir() unavailable, falling back to exe parent");
                std::env::current_exe()
                    .ok()
                    .and_then(|p| p.parent().map(|p| p.to_path_buf()))
                    .unwrap_or_else(|| PathBuf::from("C:\\"))
            });
            let embedded_data_dir = home.join(".WhistleBoxData");
            if let Err(e) = std::fs::create_dir_all(&embedded_data_dir) {
                log::error!("Failed to create whistle data dir {:?}: {}", embedded_data_dir, e);
            }
            cmd.arg("-D");
            cmd.arg(embedded_data_dir.to_string_lossy().to_string());
        }
        cmd.arg("-S");
        cmd.arg("whistlebox_embedded");
    }
    let mut modes: Vec<&str> = Vec::new();
    if conn.intercept_https {
        modes.push("capture");
    }
    if !modes.is_empty() {
        cmd.arg("-M");
        cmd.arg(modes.join(","));
    }

    if !conn.upstream_proxy.is_empty() {
        let proxy_url = if conn.upstream_proxy.contains("://") {
            conn.upstream_proxy.clone()
        } else {
            format!("http://{}", conn.upstream_proxy)
        };
        cmd.env("HTTP_PROXY", &proxy_url);
        cmd.env("HTTPS_PROXY", &proxy_url);
    }

    if let Ok(exe_dir) = std::env::current_exe() {
        if let Some(exe_parent) = exe_dir.parent() {
            cmd.current_dir(exe_parent);
        }
    }

    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::null());

    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    log::info!(
        "Starting whistle: {} {} start --host {} -p {} (auth={})",
        node_bin.display(),
        whistle_script.display(),
        conn.host,
        conn.port,
        !conn.username.is_empty()
    );

    let child = cmd.spawn().map_err(|e| format!("Failed to spawn whistle: {}", e))?;
    let pid = child.id();
    WHISTLE_PID.store(pid, Ordering::Relaxed);
    WHISTLE_PORT.store(conn.port, Ordering::Relaxed);
    WHISTLE_LAST_START.store(now_secs(), Ordering::Relaxed);

    // Wait for whistle to start (up to ~30s with increasing intervals)
    for i in 0..12 {
        sleep(Duration::from_millis(500 * (i.min(6) + 1))).await;
        if check_whistle_alive(&conn.host, conn.port, &conn.username, &conn.password).await {
            log::info!("Whistle started and healthy, PID {}", pid);
            return Ok(pid);
        }
    }

    // whistle's "start" command forks a daemon and exits, so the initial PID may not be the daemon
    // Try to find the actual whistle process by checking the port
    #[cfg(target_os = "windows")]
    {
        if let Ok(output) = std::process::Command::new("netstat")
            .args(["-ano"])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
        {
            let text = String::from_utf8_lossy(&output.stdout);
            let target_port = conn.port.to_string();
            for line in text.lines() {
                if !line.contains("LISTENING") {
                    continue;
                }
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() < 5 {
                    continue;
                }
                let matches_port = parts[1]
                    .rsplit_once(':')
                    .map(|(_, p)| p == target_port.as_str())
                    .unwrap_or(false);
                if matches_port {
                    if let Ok(real_pid) = parts[parts.len() - 1].parse::<u32>() {
                        WHISTLE_PID.store(real_pid, Ordering::Relaxed);
                        log::info!("Found whistle daemon PID: {}", real_pid);
                        return Ok(real_pid);
                    }
                }
            }
        }
    }

    log::warn!("Whistle started (PID {}) but health check not passing yet", pid);
    Ok(pid)
}

#[tauri::command]
pub async fn cmd_start_whistle(
    handle: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<WhistleStatus, String> {
    let config = state.config.lock().await;
    let conn = config.whistle.clone();
    drop(config);

    if conn.mode == "embedded" {
        let pid = start_whistle_internal(&handle, &conn).await?;
        let mut running = state.whistle_running.lock().await;
        *running = true;

        let alive = check_whistle_alive(&conn.host, conn.port, &conn.username, &conn.password).await;

        if alive {
            sync_https_interception(
                &conn.host, conn.port, &conn.username, &conn.password, conn.intercept_https,
            ).await;
        }

        Ok(WhistleStatus {
            running: true,
            mode: "embedded".to_string(),
            host: conn.host,
            port: conn.port,
            pid,
            uptime_check: alive,
        })
    } else {
        let config = state.config.lock().await;
        let (ext_host, ext_port) = config.active_endpoint();
        let (ext_user, ext_pass) = config.active_credentials();
        drop(config);
        let alive = check_whistle_alive(&ext_host, ext_port, &ext_user, &ext_pass).await;

        let mut running = state.whistle_running.lock().await;
        *running = alive;

        Ok(WhistleStatus {
            running: alive,
            mode: "external".to_string(),
            host: ext_host,
            port: ext_port,
            pid: 0,
            uptime_check: alive,
        })
    }
}

#[tauri::command]
pub async fn cmd_stop_whistle(
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let config = state.config.lock().await;
    let (config_host, config_port) = config.active_endpoint();
    let (stop_user, stop_pass) = config.active_credentials();
    drop(config);

    let actual_port = WHISTLE_PORT.load(Ordering::Relaxed);

    let stop_port = if actual_port > 0 { actual_port } else { config_port };

    let mut ports_to_stop = vec![stop_port];
    if config_port != stop_port {
        ports_to_stop.push(config_port);
    }

    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .no_proxy()
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            log::error!("Failed to build HTTP client for stop: {}", e);
            reqwest::Client::new()
        }
    };

    for &port in &ports_to_stop {
        let stop_url = format!("http://{}:{}/cgi-bin/stop", config_host, port);
        let mut req = client.get(&stop_url);
        if !stop_user.is_empty() {
            req = req.basic_auth(&stop_user, Some(&stop_pass));
        }
        match req.send().await {
            Ok(resp) => log::info!("Whistle stop API (port {}) responded: {}", port, resp.status()),
            Err(e) => log::warn!("Whistle stop API (port {}) failed: {}", port, e),
        }
    }

    let pid = WHISTLE_PID.load(Ordering::Relaxed);
    if pid > 0 {
        kill_process(pid);
        WHISTLE_PID.store(0, Ordering::Relaxed);
        log::info!("Killed whistle by stored PID: {}", pid);
    }

    kill_whistle_by_session();

    let is_local = config_host == "127.0.0.1" || config_host == "localhost" || config_host == "0.0.0.0";
    if is_local {
        for &port in &ports_to_stop {
            kill_process_by_port(port);
        }
    }

    WHISTLE_PORT.store(0, Ordering::Relaxed);

    sleep(Duration::from_millis(1000)).await;

    let mut running = state.whistle_running.lock().await;
    *running = false;
    Ok(())
}

#[tauri::command]
pub async fn cmd_get_whistle_status(
    state: tauri::State<'_, AppState>,
) -> Result<WhistleStatus, String> {
    let config = state.config.lock().await;
    let whistle_conn = config.whistle.clone();
    let (check_host, check_port) = config.active_endpoint();
    let (check_user, check_pass) = config.active_credentials();
    drop(config);

    let embedded_running = *state.whistle_running.lock().await;
    let pid = WHISTLE_PID.load(Ordering::Relaxed);
    let is_external = whistle_conn.mode != "embedded";

    let should_check = if is_external {
        true
    } else {
        embedded_running
    };

    let alive = if should_check {
        check_whistle_alive(
            &check_host,
            check_port,
            &check_user,
            &check_pass,
        )
        .await
    } else {
        false
    };

    Ok(WhistleStatus {
        running: if is_external { alive } else { embedded_running },
        mode: whistle_conn.mode,
        host: check_host,
        port: check_port,
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
    let is_private = if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        match ip {
            std::net::IpAddr::V4(v4) => v4.is_loopback() || v4.is_private(),
            std::net::IpAddr::V6(v6) => v6.is_loopback(),
        }
    } else {
        host == "localhost"
    };
    if !is_private {
        return Err("Only local/private network addresses are allowed".to_string());
    }
    Ok(check_whistle_alive(&host, port, &username, &password).await)
}

#[tauri::command]
pub async fn cmd_install_cert(
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let (host, port, username, password) = {
        let config = state.config.lock().await;
        let (h, p) = config.active_endpoint();
        let (u, pw) = config.active_credentials();
        (h, p, u, pw)
    };

    let url = format!("http://{}:{}/cgi-bin/rootca", host, port);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .no_proxy()
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

    let mut req = client.get(&url);
    if !username.is_empty() {
        req = req.basic_auth(&username, Some(&password));
    }

    let resp = req.send().await.map_err(|e| format!("下载证书失败: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("下载证书失败，Whistle 返回状态码: {}", resp.status()));
    }

    let cert_bytes = resp.bytes().await.map_err(|e| format!("读取证书数据失败: {}", e))?;
    if cert_bytes.is_empty() {
        return Err("Whistle 返回了空的证书数据".to_string());
    }

    let temp_dir = std::env::temp_dir();
    let cert_name = format!("whistlebox_rootca_{}.crt", std::process::id());
    let cert_path = temp_dir.join(cert_name);
    std::fs::write(&cert_path, &cert_bytes)
        .map_err(|e| format!("保存证书到临时文件失败: {}", e))?;

    install_cert_to_system(&cert_path)?;

    let _ = std::fs::remove_file(&cert_path);
    Ok("证书已成功安装到系统受信任的根证书存储区".to_string())
}

#[tauri::command]
pub async fn cmd_uninstall_cert() -> Result<String, String> {
    uninstall_cert_from_system()
}

#[tauri::command]
pub async fn cmd_check_cert_installed() -> Result<bool, String> {
    check_cert_installed()
}

#[tauri::command]
pub async fn cmd_sync_https_interception(
    state: tauri::State<'_, crate::AppState>,
    enable: bool,
) -> Result<(), String> {
    let config = state.config.lock().await;
    let (host, port) = config.active_endpoint();
    let (user, pass) = config.active_credentials();
    drop(config);
    sync_https_interception(&host, port, &user, &pass, enable).await;
    Ok(())
}

#[cfg(target_os = "windows")]
fn check_cert_installed() -> Result<bool, String> {
    let output = std::process::Command::new("certutil")
        .args(["-store", "-user", "Root"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("读取证书存储失败: {}", e))?;

    let store_text = decode_windows_output(&output.stdout);
    let lower = store_text.to_lowercase();
    Ok(lower.contains("whistle") && (lower.contains("rootca") || lower.contains("root ca") || lower.contains("cn=whistle")))
}

#[cfg(target_os = "macos")]
fn check_cert_installed() -> Result<bool, String> {
    let output = std::process::Command::new("security")
        .args(["find-certificate", "-a", "-c", "whistle", "-Z", "/Library/Keychains/System.keychain"])
        .output()
        .map_err(|e| format!("搜索证书失败: {}", e))?;

    let text = String::from_utf8_lossy(&output.stdout);
    Ok(text.contains("SHA-1 hash:") || text.contains("SHA-256 hash:"))
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn check_cert_installed() -> Result<bool, String> {
    Ok(false)
}

#[cfg(target_os = "windows")]
fn uninstall_cert_from_system() -> Result<String, String> {
    let output = std::process::Command::new("certutil")
        .args(["-store", "-user", "Root"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("读取证书存储失败: {}", e))?;

    let store_text = decode_windows_output(&output.stdout);
    let mut removed = 0u32;
    let mut found = 0u32;
    let mut last_error = String::new();

    let blocks: Vec<&str> = store_text.split("================").collect();
    for block in &blocks {
        let block_lower = block.to_lowercase();
        if !block_lower.contains("whistle") {
            continue;
        }
        if let Some(hash) = extract_sha1_hash(block) {
            found += 1;
            let del = std::process::Command::new("certutil")
                .args(["-delstore", "-user", "Root", &hash])
                .creation_flags(CREATE_NO_WINDOW)
                .output();
            match del {
                Ok(del_out) if del_out.status.success() => {
                    removed += 1;
                    log::info!("Removed whistle root cert with hash {}", hash);
                }
                Ok(del_out) => {
                    let stderr = decode_windows_output(&del_out.stderr);
                    let stdout = decode_windows_output(&del_out.stdout);
                    last_error = format!("{}{}", stdout, stderr);
                    log::warn!("Failed to remove cert {}: {}", hash, last_error);
                }
                Err(e) => {
                    last_error = e.to_string();
                }
            }
        }
    }

    if removed > 0 {
        Ok(format!("已成功移除 {} 个 Whistle 根证书", removed))
    } else if found > 0 {
        if last_error.contains("拒绝") || last_error.contains("denied") || last_error.contains("Access") {
            Err("找到 Whistle 根证书但权限不足，请以管理员身份运行 WhistleBox".to_string())
        } else {
            Err(format!("找到 {} 个 Whistle 根证书但移除失败: {}", found, last_error))
        }
    } else {
        Err("未找到已安装的 Whistle 根证书".to_string())
    }
}

#[cfg(target_os = "windows")]
fn extract_sha1_hash(block: &str) -> Option<String> {
    for line in block.lines() {
        if line.contains("(sha1)") && line.contains(":") {
            let after_colon = line.split(':').skip(1).collect::<Vec<&str>>().join(":");
            let hash: String = after_colon.chars().filter(|c| c.is_ascii_hexdigit()).collect();
            if hash.len() >= 40 {
                return Some(hash[..40].to_string());
            }
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn uninstall_cert_from_system() -> Result<String, String> {
    let output = std::process::Command::new("security")
        .args(["find-certificate", "-a", "-c", "whistle", "-Z", "/Library/Keychains/System.keychain"])
        .output()
        .map_err(|e| format!("搜索证书失败: {}", e))?;

    let text = String::from_utf8_lossy(&output.stdout);
    let mut removed = 0u32;

    for line in text.lines() {
        if line.starts_with("SHA-1 hash:") || line.starts_with("SHA-256 hash:") {
            if let Some(hash) = line.split(':').nth(1) {
                let hash = hash.trim();
                let del = std::process::Command::new("security")
                    .args(["delete-certificate", "-Z", hash, "/Library/Keychains/System.keychain"])
                    .output();
                if let Ok(del_out) = del {
                    if del_out.status.success() {
                        removed += 1;
                    }
                }
            }
        }
    }

    if removed > 0 {
        Ok(format!("已成功移除 {} 个 Whistle 根证书", removed))
    } else {
        Err("未找到已安装的 Whistle 根证书".to_string())
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn uninstall_cert_from_system() -> Result<String, String> {
    Err("当前系统不支持自动移除证书，请手动操作".to_string())
}

#[cfg(target_os = "windows")]
fn decode_windows_output(bytes: &[u8]) -> String {
    let (decoded, _, _) = encoding_rs::GBK.decode(bytes);
    decoded.into_owned()
}

#[cfg(target_os = "windows")]
fn install_cert_to_system(cert_path: &std::path::Path) -> Result<(), String> {
    let output = std::process::Command::new("certutil")
        .args(["-addstore", "-user", "-f", "Root", &cert_path.to_string_lossy()])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("执行 certutil 失败: {}", e))?;

    if output.status.success() {
        log::info!("Certificate installed to user Root store via certutil");
        Ok(())
    } else {
        let stderr = decode_windows_output(&output.stderr);
        let stdout = decode_windows_output(&output.stdout);
        let exit_code = output.status.code().unwrap_or(-1);
        log::warn!("certutil install failed (exit {}): stdout={}, stderr={}", exit_code, stdout, stderr);
        if stderr.contains("拒绝") || stderr.contains("denied") || stdout.contains("拒绝") || stdout.contains("denied") {
            Err("需要管理员权限才能安装根证书，请以管理员身份运行 WhistleBox".to_string())
        } else {
            Err(format!("安装证书失败: {}{}", stdout, stderr))
        }
    }
}

#[cfg(target_os = "macos")]
fn install_cert_to_system(cert_path: &std::path::Path) -> Result<(), String> {
    let output = std::process::Command::new("security")
        .args([
            "add-trusted-cert",
            "-d",
            "-r", "trustRoot",
            "-k", "/Library/Keychains/System.keychain",
            &cert_path.to_string_lossy(),
        ])
        .output()
        .map_err(|e| format!("执行 security 命令失败: {}", e))?;

    if output.status.success() {
        log::info!("Certificate installed successfully via security command");
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("安装证书失败: {}", stderr))
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn install_cert_to_system(cert_path: &std::path::Path) -> Result<(), String> {
    Err(format!(
        "当前系统不支持自动安装证书，请手动安装: {}",
        cert_path.display()
    ))
}
