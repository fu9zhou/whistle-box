use crate::AppState;
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
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

pub fn is_valid_domain_pattern(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 253
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '*' || c == '?')
        && !s.contains("..")
}

fn sanitize_pac_host(host: &str) -> String {
    host.chars()
        .filter(|c| {
            c.is_ascii_alphanumeric()
                || *c == '.'
                || *c == '-'
                || *c == ':'
                || *c == '['
                || *c == ']'
        })
        .collect()
}

fn generate_pac(rules: &[(String, bool)], proxy_host: &str, proxy_port: u16) -> String {
    let host = sanitize_pac_host(proxy_host);
    let safe_host = if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host
    };
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
}"#
        .to_string();
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
    log::info!(
        "PAC content updated ({} rules, {} enabled)",
        rules.len(),
        rules.iter().filter(|(_, e)| *e).count()
    );
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
    let mut active = PAC_LISTENER.lock().await;
    if active
        .as_ref()
        .is_some_and(|(current, task)| *current == port && !task.is_finished())
    {
        update_pac_content(&rules, &proxy_host, proxy_port).await;
        return Ok(());
    }
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = TcpListener::bind(addr)
        .await
        .map_err(|e| format!("Failed to bind PAC server on port {}: {}", port, e))?;

    log::info!("PAC server started on http://127.0.0.1:{}", port);

    update_pac_content(&rules, &proxy_host, proxy_port).await;
    let conn_limiter = pac_conn_limiter();
    let task = tokio::spawn(async move {
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
                    log::warn!(
                        "PAC server reached max concurrent connections, dropping connection"
                    );
                    continue;
                }
            };

            tokio::spawn(async move {
                let _permit = permit;
                let service = service_fn(handle_pac_request);

                if let Err(e) = tokio::time::timeout(
                    std::time::Duration::from_secs(15),
                    http1::Builder::new().serve_connection(io, service),
                )
                .await
                {
                    log::error!("PAC connection error: {}", e);
                }
            });
        }
    });
    if let Some((_, old)) = active.replace((port, task)) {
        old.abort();
        let _ = old.await;
    }
    Ok(())
}

static PAC_LISTENER: tokio::sync::Mutex<Option<(u16, tokio::task::JoinHandle<()>)>> =
    tokio::sync::Mutex::const_new(None);
pub async fn stop_pac_server() {
    if let Some((_, task)) = PAC_LISTENER.lock().await.take() {
        task.abort();
        let _ = task.await;
    }
}

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

pub async fn start_for_config(
    config: &crate::config::AppConfig,
    state: &AppState,
) -> Result<u16, String> {
    let (rules, host, port) = collect_rules(config);
    start_pac_server_internal(config.pac_server_port, rules, host, port).await?;
    *state.pac_server_port.lock().await = config.pac_server_port;
    Ok(config.pac_server_port)
}
#[tauri::command]
pub async fn cmd_start_pac_server(state: tauri::State<'_, AppState>) -> Result<u16, String> {
    let _operation = state.operations.lock().await;
    let config = state.config.lock().await.clone();
    start_for_config(&config, &state).await
}

#[tauri::command]
pub async fn cmd_refresh_pac(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let _operation = state.operations.lock().await;
    let config = state.config.lock().await;
    let (rules, proxy_host, proxy_port) = collect_rules(&config);
    drop(config);

    update_pac_content(&rules, &proxy_host, proxy_port).await;

    let mode = state.proxy_mode.lock().await.clone();
    if mode == "rule" {
        #[cfg(target_os = "windows")]
        {
            crate::proxy::set_proxy_mode_internal(&state, "rule").await?;
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn cmd_get_pac_url(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let port = state.pac_server_port.lock().await;
    Ok(format!("http://127.0.0.1:{}/proxy.pac", port))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn free_port() -> u16 {
        std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port()
    }
    #[tokio::test]
    async fn port_reconfiguration_preserves_old_service_on_conflict() {
        let first = free_port();
        let next = free_port();
        let occupied = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let blocked = occupied.local_addr().unwrap().port();
        start_pac_server_internal(
            first,
            vec![("example.com".into(), true)],
            "127.0.0.1".into(),
            18899,
        )
        .await
        .unwrap();
        assert!(
            start_pac_server_internal(blocked, vec![], "127.0.0.1".into(), 18899)
                .await
                .is_err()
        );
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let body = client
            .get(format!("http://127.0.0.1:{first}/proxy.pac"))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        assert!(body.contains("example.com"));
        start_pac_server_internal(next, vec![("new.example".into(), true)], "::1".into(), 9000)
            .await
            .unwrap();
        let body = client
            .get(format!("http://127.0.0.1:{next}/proxy.pac"))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        assert!(body.contains("new.example") && body.contains("[::1]:9000"));
        assert!(tokio::net::TcpStream::connect(("127.0.0.1", first))
            .await
            .is_err());
        stop_pac_server().await;
        assert!(tokio::net::TcpStream::connect(("127.0.0.1", blocked))
            .await
            .is_ok());
    }
}
