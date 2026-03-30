use crate::AppState;
use base64::Engine;
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use http_body_util::{BodyExt, Full};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use tokio::net::TcpListener;
use tokio::sync::{RwLock, Semaphore};

#[derive(Clone)]
struct AuthProxyConfig {
    target_host: String,
    target_port: u16,
    auth_header: Option<String>,
    secret_token: String,
    local_auth_bypass: bool,
    listen_port: u16,
    http_client: reqwest::Client,
}

static SHARED_CONFIG: OnceLock<Arc<RwLock<AuthProxyConfig>>> = OnceLock::new();
static AUTH_PROXY_CONN_LIMITER: OnceLock<Arc<Semaphore>> = OnceLock::new();
const AUTH_PROXY_MAX_CONNECTIONS: usize = 128;

fn auth_conn_limiter() -> Arc<Semaphore> {
    AUTH_PROXY_CONN_LIMITER
        .get_or_init(|| Arc::new(Semaphore::new(AUTH_PROXY_MAX_CONNECTIONS)))
        .clone()
}

fn check_host_port(url: &str, port: u16) -> bool {
    let url_lower = url.to_lowercase();
    let stripped = url_lower
        .strip_prefix("http://")
        .or_else(|| url_lower.strip_prefix("https://"))
        .unwrap_or(&url_lower);
    let host_port = stripped.split('/').next().unwrap_or("");
    let expected_a = format!("127.0.0.1:{}", port);
    let expected_b = format!("localhost:{}", port);
    host_port == expected_a || host_port == expected_b
}

fn is_private_or_loopback_host(host: &str) -> bool {
    crate::utils::is_private_or_loopback(host)
}

fn has_valid_token(req: &Request<hyper::body::Incoming>, config: &AuthProxyConfig) -> bool {
    if let Some(query) = req.uri().query() {
        let expected = format!("_token={}", config.secret_token);
        for param in query.split('&') {
            if param == expected {
                return true;
            }
        }
    }
    false
}

fn is_authorized_request(req: &Request<hyper::body::Incoming>, config: &AuthProxyConfig) -> bool {
    if has_valid_token(req, config) {
        return true;
    }

    let origin = req.headers().get("origin").and_then(|v| v.to_str().ok()).unwrap_or("");
    let referer = req.headers().get("referer").and_then(|v| v.to_str().ok()).unwrap_or("");

    let from_tauri = origin.starts_with("tauri://") || referer.starts_with("tauri://");
    let from_self = check_host_port(origin, config.listen_port)
        || check_host_port(referer, config.listen_port);

    #[cfg(debug_assertions)]
    let from_dev = origin.starts_with("http://localhost:1420")
        || referer.starts_with("http://localhost:1420");
    #[cfg(not(debug_assertions))]
    let from_dev = false;

    if from_tauri || from_self || from_dev {
        let has_token_in_referer = referer.contains("_token=");
        if has_token_in_referer || from_tauri {
            return true;
        }
    }

    false
}

fn extract_theme_param(uri: &hyper::Uri) -> Option<String> {
    uri.query().and_then(|q| {
        q.split('&').find_map(|pair| {
            let mut kv = pair.splitn(2, '=');
            match (kv.next(), kv.next()) {
                (Some("_theme"), Some(v)) if v == "dark" || v == "light" => Some(v.to_string()),
                _ => None,
            }
        })
    })
}

fn inject_theme_into_html(body: &[u8], theme: &str) -> Option<Vec<u8>> {
    let text = std::str::from_utf8(body).ok()?;
    if !text.contains("<head") && !text.contains("<HEAD") {
        return None;
    }
    let theme_script = format!(
        r#"<script>(function(){{var t="{}";document.documentElement.setAttribute("data-theme",t);new MutationObserver(function(){{document.documentElement.getAttribute("data-theme")!==t&&document.documentElement.setAttribute("data-theme",t)}}).observe(document.documentElement,{{attributes:true,attributeFilter:["data-theme"]}})}})();</script>"#,
        theme
    );
    let injected = if let Some(pos) = text.find("</head>") {
        format!("{}{}{}", &text[..pos], theme_script, &text[pos..])
    } else if let Some(pos) = text.find("</HEAD>") {
        format!("{}{}{}", &text[..pos], theme_script, &text[pos..])
    } else {
        return None;
    };
    Some(injected.into_bytes())
}

async fn proxy_request(
    req: Request<hyper::body::Incoming>,
    config: Arc<RwLock<AuthProxyConfig>>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    if req.uri().path() == "/__health" {
        return Ok(Response::builder()
            .status(200)
            .header("Access-Control-Allow-Origin", "*")
            .header("Cache-Control", "no-store")
            .body(Full::new(Bytes::from("ok")))
            .unwrap());
    }

    let cfg = config.read().await;

    if !cfg.local_auth_bypass && !is_authorized_request(&req, &cfg) {
        return Ok(Response::builder()
            .status(403)
            .body(Full::new(Bytes::from("Forbidden: This proxy is only accessible from WhistleBox app.")))
            .unwrap());
    }

    let theme_param = extract_theme_param(req.uri());

    let target_url = format!(
        "http://{}:{}{}",
        cfg.target_host,
        cfg.target_port,
        req.uri().path_and_query().map(|pq| pq.as_str()).unwrap_or("/")
    );

    let auth_header = cfg.auth_header.clone();
    let client = cfg.http_client.clone();
    drop(cfg);

    let method = match req.method().as_str() {
        "GET" => reqwest::Method::GET,
        "POST" => reqwest::Method::POST,
        "PUT" => reqwest::Method::PUT,
        "DELETE" => reqwest::Method::DELETE,
        "PATCH" => reqwest::Method::PATCH,
        "HEAD" => reqwest::Method::HEAD,
        "OPTIONS" => reqwest::Method::OPTIONS,
        _ => reqwest::Method::GET,
    };

    let mut builder = client.request(method, &target_url);

    for (key, value) in req.headers() {
        let k = key.as_str();
        if k != "host" && k != "authorization" && k != "accept-encoding" {
            if let Ok(v) = value.to_str() {
                builder = builder.header(k, v);
            }
        }
    }

    if let Some(ref auth) = auth_header {
        builder = builder.header("Authorization", auth);
        log::debug!("Auth proxy: injecting auth header (len={})", auth.len());
    } else {
        log::debug!("Auth proxy: no auth header to inject");
    }

    let body_bytes = req.collect().await.map(|b| b.to_bytes()).unwrap_or_default();
    if !body_bytes.is_empty() {
        builder = builder.body(body_bytes.to_vec());
    }

    match builder.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();

            if status == 401 {
                log::warn!("Auth proxy: Whistle returned 401 - credentials mismatch, may need restart");
                return Ok(Response::builder()
                    .status(502)
                    .header("Content-Type", "text/html; charset=utf-8")
                    .body(Full::new(Bytes::from(
                        "<html><body style='font-family:sans-serif;text-align:center;padding:40px'>\
                         <h3>认证失败</h3>\
                         <p>Whistle 拒绝了当前凭据，请在设置中重启 Whistle 后刷新页面。</p>\
                         </body></html>"
                    )))
                    .unwrap());
            }

            let mut response_builder = Response::builder().status(status);

            for (key, value) in resp.headers() {
                let k = key.as_str();
                if matches!(
                    k,
                    "transfer-encoding"
                        | "content-length"
                        | "www-authenticate"
                        | "x-frame-options"
                        | "content-security-policy"
                        | "content-security-policy-report-only"
                ) {
                    continue;
                }
                response_builder = response_builder.header(k, value);
            }

            response_builder = response_builder
                .header("Access-Control-Allow-Origin", "*")
                .header("Access-Control-Allow-Methods", "GET, POST, PUT, DELETE, OPTIONS, HEAD")
                .header("Access-Control-Allow-Headers", "*");

            let body = resp.bytes().await.unwrap_or_default();
            let final_body = if let Some(ref theme) = theme_param {
                let content_type = response_builder
                    .headers_ref()
                    .and_then(|h| h.get("content-type"))
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("")
                    .to_string();
                if content_type.contains("text/html") {
                    inject_theme_into_html(&body, theme)
                        .map(Bytes::from)
                        .unwrap_or(body)
                } else {
                    body
                }
            } else {
                body
            };
            Ok(response_builder
                .body(Full::new(Bytes::from(final_body.to_vec())))
                .unwrap())
        }
        Err(e) => {
            log::error!("Auth proxy upstream error: {}", e);
            Ok(Response::builder()
                .status(502)
                .header("Content-Type", "text/html; charset=utf-8")
                .body(Full::new(Bytes::from(
                    "<html><body style='font-family:sans-serif;text-align:center;padding:40px'>\
                     <h3>代理连接失败</h3>\
                     <p>无法连接到 Whistle，请确认 Whistle 已启动并检查网络设置。</p>\
                     </body></html>"
                )))
                .unwrap())
        }
    }
}

static AUTH_PROXY_STARTING: AtomicBool = AtomicBool::new(false);
static AUTH_PROXY_LISTENING: AtomicBool = AtomicBool::new(false);
static AUTH_TOKEN: OnceLock<String> = OnceLock::new();

fn generate_token() -> String {
    let mut buf = [0u8; 32];
    if let Err(e) = getrandom::getrandom(&mut buf) {
        log::error!("getrandom failed: {}", e);
        panic!("Cannot generate secure auth token: getrandom unavailable ({}). WhistleBox requires a working CSPRNG.", e);
    }
    buf.iter().map(|b| format!("{:02x}", b)).collect()
}

pub fn get_auth_token() -> &'static str {
    AUTH_TOKEN.get_or_init(generate_token)
}

fn build_auth_header(username: &str, password: &str) -> Option<String> {
    if !username.is_empty() {
        let credentials = format!("{}:{}", username, password);
        let encoded = base64::engine::general_purpose::STANDARD.encode(credentials);
        Some(format!("Basic {}", encoded))
    } else {
        None
    }
}

pub async fn start_auth_proxy_internal(
    port: u16,
    target_host: String,
    target_port: u16,
    username: String,
    password: String,
    local_auth_bypass: bool,
) -> Result<(), String> {
    if !is_private_or_loopback_host(&target_host) {
        return Err("Auth proxy target host must be localhost or private network IP".to_string());
    }
    let token = get_auth_token().to_string();
    let auth_header = build_auth_header(&username, &password);
    log::info!(
        "Auth proxy init: target={}:{}, username='{}', auth_header={}, bypass={}",
        target_host, target_port, username,
        if auth_header.is_some() { "present" } else { "NONE" },
        local_auth_bypass
    );

    let http_client = reqwest::Client::builder()
        .no_proxy()
        .pool_max_idle_per_host(10)
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

    let new_cfg = AuthProxyConfig {
        target_host: target_host.clone(),
        target_port,
        auth_header,
        secret_token: token.clone(),
        local_auth_bypass,
        listen_port: port,
        http_client,
    };

    if let Some(shared) = SHARED_CONFIG.get() {
        let mut cfg = shared.write().await;
        log::info!(
            "Updating auth proxy config: target={}:{} (bypass={})",
            target_host, target_port, local_auth_bypass
        );
        *cfg = new_cfg;

        if AUTH_PROXY_LISTENING.load(Ordering::SeqCst) {
            return Ok(());
        }
        log::warn!("Auth proxy config exists but listener is not active, will attempt restart");
    } else {
        let config = Arc::new(RwLock::new(new_cfg.clone()));
        if SHARED_CONFIG.set(config).is_err() {
            if let Some(shared) = SHARED_CONFIG.get() {
                let mut cfg = shared.write().await;
                *cfg = new_cfg;
                drop(cfg);
            }
        }
    }

    if AUTH_PROXY_STARTING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        log::info!("Auth proxy bind already in progress, waiting for it to complete...");
        for _ in 0..100 {
            if AUTH_PROXY_LISTENING.load(Ordering::SeqCst) {
                log::info!("Auth proxy listener is now ready");
                return Ok(());
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        if AUTH_PROXY_LISTENING.load(Ordering::SeqCst) {
            log::info!("Auth proxy listener became ready during final check");
            return Ok(());
        }
        log::error!("Auth proxy bind wait timed out after 10s, listener not ready");
        return Err("Auth proxy listener not ready after 10s wait".to_string());
    }

    if local_auth_bypass {
        log::info!("Local auth bypass enabled - local browser access will not require authentication");
    }

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = {
        let mut last_err = None;
        let mut bound = None;
        for attempt in 0..5 {
            match TcpListener::bind(addr).await {
                Ok(l) => { bound = Some(l); break; }
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::AddrInUse {
                        if attempt == 0 {
                            log::warn!("Auth proxy port {} in use, killing stale process", port);
                            crate::utils::kill_process_by_port(port);
                        }
                        if attempt < 4 {
                            log::warn!("Auth proxy port {} in use, retrying in 2s (attempt {}/5)", port, attempt + 1);
                            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                        }
                    }
                    last_err = Some(e);
                }
            }
        }
        match bound {
            Some(l) => l,
            None => {
                AUTH_PROXY_STARTING.store(false, Ordering::SeqCst);
                let e = last_err.unwrap_or_else(|| std::io::Error::other("bind failed"));
                return Err(format!("Failed to bind auth proxy on port {}: {}", port, e));
            }
        }
    };

    AUTH_PROXY_LISTENING.store(true, Ordering::SeqCst);
    AUTH_PROXY_STARTING.store(false, Ordering::SeqCst);
    log::info!("Auth proxy started on http://127.0.0.1:{}", port);

    let config = SHARED_CONFIG.get().unwrap().clone();
    let conn_limiter = auth_conn_limiter();
    tokio::spawn(async move {
        loop {
            let (stream, _) = match listener.accept().await {
                Ok(conn) => conn,
                Err(e) => {
                    log::error!("Auth proxy accept error: {}", e);
                    continue;
                }
            };
            let io = TokioIo::new(stream);
            let config = config.clone();
            let permit = match conn_limiter.clone().try_acquire_owned() {
                Ok(permit) => permit,
                Err(_) => {
                    log::warn!(
                        "Auth proxy reached max concurrent connections ({}), dropping connection",
                        AUTH_PROXY_MAX_CONNECTIONS
                    );
                    continue;
                }
            };

            tokio::spawn(async move {
                let _permit = permit;
                let service = service_fn(move |req| {
                    let config = config.clone();
                    proxy_request(req, config)
                });

                if let Err(e) = http1::Builder::new().serve_connection(io, service).await {
                    if !e.to_string().contains("connection closed") {
                        log::error!("Auth proxy connection error: {}", e);
                    }
                }
            });
        }
    });

    Ok(())
}

#[tauri::command]
pub async fn cmd_start_auth_proxy(
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let config = state.config.lock().await;
    let port = config.auth_proxy_port;
    let (target_host, target_port) = config.active_endpoint();
    let (username, password) = config.active_credentials();
    let local_auth_bypass = config.app_settings.local_auth_bypass;
    drop(config);

    start_auth_proxy_internal(port, target_host, target_port, username, password, local_auth_bypass).await?;

    let mut auth_port = state.auth_proxy_port.lock().await;
    *auth_port = port;

    let token = get_auth_token();
    Ok(format!("http://127.0.0.1:{}?_token={}", port, token))
}

#[tauri::command]
pub async fn cmd_get_auth_proxy_url(
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let port = state.auth_proxy_port.lock().await;
    Ok(format!("http://127.0.0.1:{}", port))
}

#[tauri::command]
pub async fn cmd_probe_auth_proxy(
    state: tauri::State<'_, AppState>,
) -> Result<bool, String> {
    if !AUTH_PROXY_LISTENING.load(Ordering::SeqCst) {
        return Ok(false);
    }
    let port = *state.auth_proxy_port.lock().await;
    let url = format!("http://127.0.0.1:{}/__health", port);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .no_proxy()
        .build()
        .map_err(|e| e.to_string())?;
    match client.get(&url).send().await {
        Ok(resp) => Ok(resp.status().is_success()),
        Err(_) => Ok(false),
    }
}
