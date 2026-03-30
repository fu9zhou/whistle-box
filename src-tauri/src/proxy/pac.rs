use crate::AppState;
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use http_body_util::Full;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::{Arc, OnceLock};
use tokio::net::TcpListener;
use tokio::sync::{RwLock, Semaphore};

fn pac_content_store() -> &'static Arc<RwLock<String>> {
    static STORE: OnceLock<Arc<RwLock<String>>> = OnceLock::new();
    STORE.get_or_init(|| Arc::new(RwLock::new(String::new())))
}

fn pac_conn_limiter() -> Arc<Semaphore> {
    static LIMITER: OnceLock<Arc<Semaphore>> = OnceLock::new();
    const MAX_PAC_CONNECTIONS: usize = 64;
    LIMITER
        .get_or_init(|| Arc::new(Semaphore::new(MAX_PAC_CONNECTIONS)))
        .clone()
}

fn is_valid_domain_pattern(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 253
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '*' || c == '?')
        && !s.contains("..")
}

fn sanitize_pac_host(host: &str) -> String {
    host.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '.' || *c == '-' || *c == ':' || *c == '[' || *c == ']')
        .collect()
}

fn generate_pac(rules: &[(String, bool)], proxy_host: &str, proxy_port: u16) -> String {
    let safe_host = sanitize_pac_host(proxy_host);
    let mut conditions = Vec::new();
    for (pattern, enabled) in rules {
        if !enabled {
            continue;
        }
        let lower = pattern.to_lowercase();
        if !is_valid_domain_pattern(&lower) {
            log::warn!("Skipping invalid PAC rule pattern: {}", pattern);
            continue;
        }
        if lower.contains('*') || lower.contains('?') {
            conditions.push(format!(r#"    shExpMatch(h, "{}")"#, lower));
        } else {
            conditions.push(format!(r#"    h === "{}""#, lower));
        }
    }

    if conditions.is_empty() {
        return r#"function FindProxyForURL(url, host) {
  return "DIRECT";
}"#.to_string();
    }

    let condition_str = conditions.join(" ||\n");
    format!(
        r#"function FindProxyForURL(url, host) {{
  var h = host.toLowerCase();
  if (
{}
  ) {{
    return "PROXY {}:{}";
  }}
  return "DIRECT";
}}"#,
        condition_str, safe_host, proxy_port
    )
}

pub async fn update_pac_content(rules: &[(String, bool)], proxy_host: &str, proxy_port: u16) {
    let content = generate_pac(rules, proxy_host, proxy_port);
    log::info!("PAC content updated ({} rules, {} enabled)", rules.len(), rules.iter().filter(|(_, e)| *e).count());
    let mut store = pac_content_store().write().await;
    *store = content;
}

async fn handle_pac_request(
    _req: Request<hyper::body::Incoming>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let content = pac_content_store().read().await.clone();
    let response = Response::builder()
        .header("Content-Type", "application/x-ns-proxy-autoconfig")
        .header("Cache-Control", "no-cache, no-store, must-revalidate")
        .body(Full::new(Bytes::from(content)))
        .unwrap();
    Ok(response)
}

pub async fn start_pac_server_internal(
    port: u16,
    rules: Vec<(String, bool)>,
    proxy_host: String,
    proxy_port: u16,
) -> Result<(), String> {
    update_pac_content(&rules, &proxy_host, proxy_port).await;

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = TcpListener::bind(addr)
        .await
        .map_err(|e| format!("Failed to bind PAC server on port {}: {}", port, e))?;

    log::info!("PAC server started on http://127.0.0.1:{}", port);

    let conn_limiter = pac_conn_limiter();
    tokio::spawn(async move {
        loop {
            let (stream, _) = match listener.accept().await {
                Ok(conn) => conn,
                Err(e) => {
                    log::error!("PAC server accept error: {}", e);
                    continue;
                }
            };
            let io = TokioIo::new(stream);
            let permit = match conn_limiter.clone().try_acquire_owned() {
                Ok(permit) => permit,
                Err(_) => {
                    log::warn!("PAC server reached max concurrent connections, dropping connection");
                    continue;
                }
            };

            tokio::spawn(async move {
                let _permit = permit;
                let service = service_fn(|req| handle_pac_request(req));

                if let Err(e) = http1::Builder::new().serve_connection(io, service).await {
                    log::error!("PAC connection error: {}", e);
                }
            });
        }
    });

    Ok(())
}

use std::sync::atomic::{AtomicBool, Ordering};
static PAC_SERVER_RUNNING: AtomicBool = AtomicBool::new(false);

fn collect_rules(config: &crate::config::AppConfig) -> (Vec<(String, bool)>, String, u16) {
    let (proxy_host, proxy_port) = config.active_endpoint();
    let rules: Vec<(String, bool)> = config
        .get_active_profile()
        .map(|p| {
            p.rules
                .iter()
                .map(|r| (r.pattern.clone(), r.enabled))
                .collect()
        })
        .unwrap_or_default();
    (rules, proxy_host, proxy_port)
}

#[tauri::command]
pub async fn cmd_start_pac_server(state: tauri::State<'_, AppState>) -> Result<u16, String> {
    let config = state.config.lock().await;
    let port = config.pac_server_port;
    let (rules, proxy_host, proxy_port) = collect_rules(&config);
    drop(config);

    update_pac_content(&rules, &proxy_host, proxy_port).await;

    if PAC_SERVER_RUNNING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        log::info!("PAC server already running on port {}, rules refreshed", port);
        return Ok(port);
    }

    if let Err(e) = start_pac_server_internal(port, rules, proxy_host, proxy_port).await {
        PAC_SERVER_RUNNING.store(false, Ordering::SeqCst);
        return Err(e);
    }

    let mut pac_port = state.pac_server_port.lock().await;
    *pac_port = port;

    Ok(port)
}

#[tauri::command]
pub async fn cmd_refresh_pac(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let config = state.config.lock().await;
    let (rules, proxy_host, proxy_port) = collect_rules(&config);
    drop(config);

    update_pac_content(&rules, &proxy_host, proxy_port).await;

    let mode = state.proxy_mode.lock().await.clone();
    if mode == "rule" {
        #[cfg(target_os = "windows")]
        {
            let pac_port = *state.pac_server_port.lock().await;
            crate::proxy::refresh_pac_system_proxy(pac_port)?;
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn cmd_get_pac_url(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let port = state.pac_server_port.lock().await;
    Ok(format!("http://127.0.0.1:{}/proxy.pac", port))
}
