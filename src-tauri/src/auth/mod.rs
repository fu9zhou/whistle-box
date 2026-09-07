use crate::{utils, AppState};
use base64::Engine;
use futures_util::StreamExt;
use http_body_util::{combinators::UnsyncBoxBody, BodyExt, Full, StreamBody};
use hyper::body::{Bytes, Frame};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use std::{
    convert::Infallible,
    sync::{Arc, OnceLock},
    time::Duration,
};
use tokio::{
    net::TcpListener,
    sync::{Mutex, RwLock, Semaphore},
    task::JoinHandle,
};
type BoxError = Box<dyn std::error::Error + Send + Sync>;
type ProxyBody = UnsyncBoxBody<Bytes, BoxError>;
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
struct Listener {
    port: u16,
    config: Arc<RwLock<AuthProxyConfig>>,
    task: JoinHandle<()>,
}
static LISTENER: Mutex<Option<Listener>> = Mutex::const_new(None);
static AUTH_TOKEN: OnceLock<String> = OnceLock::new();
pub fn get_auth_token() -> &'static str {
    AUTH_TOKEN.get_or_init(|| {
        let mut bytes = [0; 32];
        getrandom::getrandom(&mut bytes).expect("Secure randomness unavailable");
        bytes.iter().map(|v| format!("{v:02x}")).collect()
    })
}
fn full(body: impl Into<Bytes>) -> ProxyBody {
    Full::new(body.into())
        .map_err(|never| match never {})
        .boxed_unsync()
}
fn error(status: u16, message: &str) -> Response<ProxyBody> {
    Response::builder()
        .status(status)
        .header("Content-Type", "text/plain; charset=utf-8")
        .header("Cache-Control", "no-store")
        .body(full(message.to_string()))
        .unwrap()
}
fn check_host_port(raw: &str, port: u16) -> bool {
    reqwest::Url::parse(raw).is_ok_and(|url| {
        url.scheme() == "http"
            && url.port_or_known_default() == Some(port)
            && matches!(url.host_str(), Some("127.0.0.1" | "localhost"))
    })
}
fn query_token(query: Option<&str>, token: &str) -> bool {
    query.is_some_and(|q| {
        q.split('&')
            .any(|p| p.strip_prefix("_token=") == Some(token))
    })
}
fn has_valid_token<B>(req: &Request<B>, config: &AuthProxyConfig) -> bool {
    query_token(req.uri().query(), &config.secret_token)
}
fn cookie_name(port: u16) -> String {
    format!("whistlebox_session_{port}")
}
fn is_authorized_request<B>(req: &Request<B>, config: &AuthProxyConfig) -> bool {
    if has_valid_token(req, config) {
        return true;
    }
    let origin = req
        .headers()
        .get("origin")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if !origin.is_empty() && !check_host_port(origin, config.listen_port) {
        return false;
    }
    let referer = req
        .headers()
        .get("referer")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if check_host_port(referer, config.listen_port)
        && reqwest::Url::parse(referer).is_ok_and(|u| query_token(u.query(), &config.secret_token))
    {
        return true;
    }
    let expected = format!(
        "{}={}",
        cookie_name(config.listen_port),
        config.secret_token
    );
    req.headers()
        .get("cookie")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.split(';').any(|v| v.trim() == expected))
}
fn hop_header(key: &str) -> bool {
    matches!(
        key,
        "host"
            | "connection"
            | "keep-alive"
            | "proxy-connection"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
            | "content-length"
    )
}
fn clean_path(uri: &hyper::Uri) -> String {
    let mut path = uri.path().to_string();
    let query = uri
        .query()
        .unwrap_or("")
        .split('&')
        .filter(|p| !p.is_empty() && !p.starts_with("_token=") && !p.starts_with("_theme="))
        .collect::<Vec<_>>()
        .join("&");
    if !query.is_empty() {
        path.push('?');
        path.push_str(&query);
    }
    path
}
fn html_injection(theme: &str) -> String {
    let theme = if theme == "light" { "light" } else { "dark" };
    format!(
        r#"<script>(function(){{var t="{theme}";document.documentElement.setAttribute('data-theme',t);new MutationObserver(function(){{if(document.documentElement.getAttribute('data-theme')!==t)document.documentElement.setAttribute('data-theme',t)}}).observe(document.documentElement,{{attributes:true,attributeFilter:['data-theme']}});var done=false;function ready(){{var el=document.getElementById('container');if(!done&&el&&el.children.length){{done=true;parent.postMessage({{type:'whistlebox-ui-ready'}},'*');}}}}new MutationObserver(ready).observe(document,{{childList:true,subtree:true}});window.addEventListener('load',ready);window.addEventListener('error',function(){{parent.postMessage({{type:'whistlebox-ui-error'}},'*')}},true);}})();</script>"#
    )
}
async fn proxy_request(
    req: Request<hyper::body::Incoming>,
    config: Arc<RwLock<AuthProxyConfig>>,
) -> Result<Response<ProxyBody>, Infallible> {
    let cfg = config.read().await.clone();
    if !cfg.local_auth_bypass && !is_authorized_request(&req, &cfg) {
        return Ok(error(403, "请通过 WhistleBox 打开此界面"));
    }
    if cfg.local_auth_bypass {
        if let Some(origin) = req.headers().get("origin").and_then(|v| v.to_str().ok()) {
            if !check_host_port(origin, cfg.listen_port) {
                return Ok(error(403, "不允许跨站请求"));
            }
        }
    }
    if req.uri().path() == "/__health" {
        let mut info = cfg.http_client.get(utils::http_url(
            &cfg.target_host,
            cfg.target_port,
            "/cgi-bin/server-info",
        ));
        if let Some(auth) = &cfg.auth_header {
            info = info.header("Authorization", auth);
        }
        let healthy = match info.send().await {
            Ok(r) if r.status().is_success() => r
                .json::<serde_json::Value>()
                .await
                .ok()
                .is_some_and(|v| crate::whistle::valid_server_info(&v)),
            _ => false,
        };
        if !healthy {
            return Ok(error(503, "Whistle 实例未就绪"));
        }
        let mut page = cfg
            .http_client
            .get(utils::http_url(&cfg.target_host, cfg.target_port, "/"));
        if let Some(auth) = &cfg.auth_header {
            page = page.header("Authorization", auth);
        }
        return Ok(match page.send().await {
            Ok(r) if r.status().is_success() => error(200, "ok"),
            _ => error(503, "Whistle 页面不可用或认证失败"),
        });
    }
    if req
        .headers()
        .get("upgrade")
        .is_some_and(|v| v.as_bytes().eq_ignore_ascii_case(b"websocket"))
    {
        return Ok(proxy_websocket(req, &cfg).await);
    }
    if req.headers().contains_key("upgrade") {
        return Ok(error(400, "Unsupported upgrade protocol"));
    }
    let bootstrap = has_valid_token(&req, &cfg);
    let theme = req
        .uri()
        .query()
        .unwrap_or("")
        .split('&')
        .find_map(|p| p.strip_prefix("_theme="))
        .unwrap_or("dark")
        .to_string();
    let path = clean_path(req.uri());
    let target = utils::http_url(&cfg.target_host, cfg.target_port, &path);
    let mut builder = cfg.http_client.request(req.method().clone(), &target);
    let extra_hops = req
        .headers()
        .get("connection")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .split(',')
        .map(|s| s.trim().to_lowercase())
        .collect::<Vec<_>>();
    for (key, value) in req.headers() {
        let k = key.as_str();
        if hop_header(k)
            || extra_hops.iter().any(|h| h == k)
            || matches!(
                k,
                "authorization" | "accept-encoding" | "referer" | "origin" | "cookie"
            )
        {
            continue;
        }
        builder = builder.header(key, value);
    }
    if let Some(cookie) = req.headers().get("cookie").and_then(|v| v.to_str().ok()) {
        let cookie = cookie
            .split(';')
            .filter(|v| !v.trim().starts_with("whistlebox_session_"))
            .collect::<Vec<_>>()
            .join(";");
        if !cookie.is_empty() {
            builder = builder.header("cookie", cookie);
        }
    }
    builder = builder.header("Accept-Encoding", "identity");
    if let Some(auth) = &cfg.auth_header {
        builder = builder.header("Authorization", auth);
    }
    let body =
        match tokio::time::timeout(Duration::from_secs(30), read_upload(req.into_body())).await {
            Ok(Ok(body)) => body,
            Ok(Err(status)) => return Ok(error(status, "请求超过大小限制或上传中断")),
            Err(_) => return Ok(error(408, "上传超时")),
        };
    let mut upstream = match builder.body(body).send().await {
        Ok(r) => r,
        Err(_) => return Ok(error(502, "无法连接 Whistle，请检查地址、端口及运行状态")),
    };
    if upstream.status() == 401 {
        return Ok(error(502, "Whistle 认证失败，请检查用户名和密码"));
    }
    let html = upstream
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.contains("text/html"));
    let mut response = Response::builder().status(upstream.status());
    for (key, value) in upstream.headers() {
        let k = key.as_str();
        if hop_header(k)
            || matches!(
                k,
                "www-authenticate"
                    | "x-frame-options"
                    | "content-security-policy"
                    | "content-security-policy-report-only"
                    | "access-control-allow-origin"
            )
        {
            continue;
        }
        if html && matches!(k, "etag" | "last-modified" | "cache-control") {
            continue;
        }
        response = response.header(key, value);
    }
    if bootstrap {
        response = response.header(
            "Set-Cookie",
            format!(
                "{}={}; Path=/; HttpOnly; Secure; SameSite=None; Partitioned",
                cookie_name(cfg.listen_port),
                cfg.secret_token
            ),
        );
    }
    let body = if html {
        let mut bytes = Vec::new();
        loop {
            match upstream.chunk().await {
                Ok(Some(chunk)) => {
                    if bytes.len() + chunk.len() > 4 * 1024 * 1024 {
                        return Ok(error(502, "上游 HTML 超过 4 MiB 限制"));
                    }
                    bytes.extend_from_slice(&chunk);
                }
                Ok(None) => break,
                Err(_) => return Ok(error(502, "读取 Whistle 页面失败")),
            }
        }
        let mut text = String::from_utf8_lossy(&bytes).into_owned();
        if text.contains("</head>") {
            text = text.replacen("</head>", &format!("{}</head>", html_injection(&theme)), 1);
        }
        response = response
            .header("Cache-Control", "no-store")
            .header("Referrer-Policy", "same-origin");
        full(text)
    } else {
        StreamBody::new(
            upstream
                .bytes_stream()
                .map(|item| item.map(Frame::data).map_err(|e| Box::new(e) as BoxError)),
        )
        .boxed_unsync()
    };
    Ok(response.body(body).unwrap())
}

pub async fn stop_auth_proxy() {
    if let Some(listener) = LISTENER.lock().await.take() {
        listener.task.abort();
        let _ = listener.task.await;
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
    if !utils::is_private_or_loopback(&target_host) {
        return Err("认证代理仅支持本机或私有网络目标".into());
    }
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(45))
        .pool_max_idle_per_host(10)
        .build()
        .map_err(|e| e.to_string())?;
    let auth = if username.is_empty() {
        None
    } else {
        Some(format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode(format!("{username}:{password}"))
        ))
    };
    let cfg = AuthProxyConfig {
        target_host,
        target_port,
        auth_header: auth,
        secret_token: get_auth_token().into(),
        local_auth_bypass,
        listen_port: port,
        http_client: client,
    };
    let mut active = LISTENER.lock().await;
    if let Some(listener) = active.as_ref() {
        if listener.port == port && !listener.task.is_finished() {
            *listener.config.write().await = cfg;
            return Ok(());
        }
    }
    // Bind before replacing the active listener. Never terminate the port owner.
    let socket = TcpListener::bind(("127.0.0.1", port))
        .await
        .map_err(|e| format!("认证代理端口 {port} 无法绑定: {e}"))?;
    let config = Arc::new(RwLock::new(cfg));
    let shared = config.clone();
    let task = tokio::spawn(async move {
        let limit = Arc::new(Semaphore::new(32));
        loop {
            let (stream, _) = match socket.accept().await {
                Ok(s) => s,
                Err(_) => break,
            };
            let Ok(permit) = limit.clone().try_acquire_owned() else {
                continue;
            };
            let cfg = shared.clone();
            tokio::spawn(async move {
                let _permit = permit;
                let service = service_fn(move |req| proxy_request(req, cfg.clone()));
                let _ = tokio::time::timeout(
                    Duration::from_secs(120),
                    http1::Builder::new()
                        .serve_connection(TokioIo::new(stream), service)
                        .with_upgrades(),
                )
                .await;
            });
        }
    });
    if let Some(old) = active.replace(Listener { port, config, task }) {
        old.task.abort();
        let _ = old.task.await;
    }
    Ok(())
}
#[tauri::command]
pub async fn cmd_start_auth_proxy(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let _operation = state.operations.lock().await;
    let config = state.config.lock().await.clone();
    let (host, port) = config.active_endpoint();
    let (user, pass) = config.active_credentials();
    start_auth_proxy_internal(
        config.auth_proxy_port,
        host,
        port,
        user,
        pass,
        config.app_settings.local_auth_bypass,
    )
    .await?;
    *state.auth_proxy_port.lock().await = config.auth_proxy_port;
    Ok(format!(
        "http://127.0.0.1:{}?_token={}",
        config.auth_proxy_port,
        get_auth_token()
    ))
}
#[tauri::command]
pub async fn cmd_get_auth_proxy_url(state: tauri::State<'_, AppState>) -> Result<String, String> {
    Ok(format!(
        "http://127.0.0.1:{}?_token={}",
        *state.auth_proxy_port.lock().await,
        get_auth_token()
    ))
}
#[tauri::command]
pub async fn cmd_probe_auth_proxy(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let port = *state.auth_proxy_port.lock().await;
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| e.to_string())?;
    Ok(client
        .get(format!(
            "http://127.0.0.1:{port}/__health?_token={}",
            get_auth_token()
        ))
        .send()
        .await
        .is_ok_and(|r| r.status().is_success()))
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn fixture() -> (String, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let cfg = Arc::new(RwLock::new(AuthProxyConfig {
            target_host: "127.0.0.1".into(),
            target_port: 1,
            auth_header: None,
            secret_token: "correct-fixture-token".into(),
            local_auth_bypass: false,
            listen_port: port,
            http_client: reqwest::Client::builder()
                .no_proxy()
                .timeout(std::time::Duration::from_secs(1))
                .build()
                .unwrap(),
        }));
        let task = tokio::spawn(async move {
            loop {
                let (socket, _) = listener.accept().await.unwrap();
                let cfg = cfg.clone();
                tokio::spawn(async move {
                    let service = service_fn(move |req| proxy_request(req, cfg.clone()));
                    let _ = http1::Builder::new()
                        .serve_connection(TokioIo::new(socket), service)
                        .await;
                });
            }
        });
        (format!("http://127.0.0.1:{port}"), task)
    }

    #[tokio::test]
    async fn forged_referer_cannot_authorize() {
        let (url, task) = fixture().await;
        let resp = reqwest::Client::builder()
            .no_proxy()
            .build()
            .unwrap()
            .get(format!("{url}/"))
            .header("Referer", format!("{url}/?_token=wrong"))
            .send()
            .await
            .unwrap();
        task.abort();
        assert_eq!(resp.status(), 403);
    }

    #[tokio::test]
    async fn tauri_origin_alone_is_not_a_credential() {
        let (url, task) = fixture().await;
        let resp = reqwest::Client::builder()
            .no_proxy()
            .build()
            .unwrap()
            .get(format!("{url}/"))
            .header("Origin", "tauri://localhost")
            .send()
            .await
            .unwrap();
        task.abort();
        assert_eq!(resp.status(), 403);
    }

    #[tokio::test]
    async fn readiness_fails_when_upstream_is_unavailable() {
        let (url, task) = fixture().await;
        let resp = reqwest::Client::builder()
            .no_proxy()
            .build()
            .unwrap()
            .get(format!("{url}/__health?_token=correct-fixture-token"))
            .send()
            .await
            .unwrap();
        task.abort();
        assert!(!resp.status().is_success());
    }
}

async fn proxy_websocket(
    mut req: Request<hyper::body::Incoming>,
    cfg: &AuthProxyConfig,
) -> Response<ProxyBody> {
    let mut upstream = cfg
        .http_client
        .get(utils::http_url(
            &cfg.target_host,
            cfg.target_port,
            &clean_path(req.uri()),
        ))
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket");
    for name in [
        "sec-websocket-key",
        "sec-websocket-version",
        "sec-websocket-protocol",
        "sec-websocket-extensions",
    ] {
        if let Some(value) = req.headers().get(name) {
            upstream = upstream.header(name, value);
        }
    }
    if let Some(auth) = &cfg.auth_header {
        upstream = upstream.header("Authorization", auth);
    }
    let response = match upstream.send().await {
        Ok(r) if r.status() == 101 => r,
        _ => return error(502, "WebSocket 上游握手失败"),
    };
    let mut builder = Response::builder()
        .status(101)
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket");
    for name in [
        "sec-websocket-accept",
        "sec-websocket-protocol",
        "sec-websocket-extensions",
    ] {
        if let Some(value) = response.headers().get(name) {
            builder = builder.header(name, value);
        }
    }
    let downstream = hyper::upgrade::on(&mut req);
    tokio::spawn(async move {
        if let (Ok(down), Ok(mut up)) = tokio::join!(downstream, response.upgrade()) {
            let mut down = TokioIo::new(down);
            let _ = tokio::time::timeout(
                Duration::from_secs(3600),
                tokio::io::copy_bidirectional(&mut down, &mut up),
            )
            .await;
        }
    });
    builder.body(full("")).unwrap()
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    #[tokio::test]
    async fn session_streaming_upgrade_and_rebinding() {
        let upstream = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let target = upstream.local_addr().unwrap().port();
        let task = tokio::spawn(async move {
            loop {
                let (socket, _) = upstream.accept().await.unwrap();
                tokio::spawn(async move {
                    let service =
                        service_fn(|mut req: Request<hyper::body::Incoming>| async move {
                            assert_eq!(
                                req.headers().get("authorization").unwrap(),
                                "Basic dXNlcjpwYXNz"
                            );
                            if req.uri().path() == "/ws" {
                                let on = hyper::upgrade::on(&mut req);
                                tokio::spawn(async move {
                                    let mut socket = TokioIo::new(on.await.unwrap());
                                    let mut bytes = [0; 3];
                                    socket.read_exact(&mut bytes).await.unwrap();
                                    socket.write_all(&bytes).await.unwrap();
                                });
                                return Ok::<_, Infallible>(
                                    Response::builder()
                                        .status(101)
                                        .header("connection", "upgrade")
                                        .header("upgrade", "websocket")
                                        .body(full(""))
                                        .unwrap(),
                                );
                            }
                            if req.uri().path() == "/large" {
                                return Ok(Response::new(full(vec![7; 10 * 1024 * 1024])));
                            }
                            Ok(Response::new(full("fixture-ok")))
                        });
                    let _ = http1::Builder::new()
                        .serve_connection(TokioIo::new(socket), service)
                        .with_upgrades()
                        .await;
                });
            }
        });
        let free = || {
            std::net::TcpListener::bind("127.0.0.1:0")
                .unwrap()
                .local_addr()
                .unwrap()
                .port()
        };
        let port = free();
        let next = free();
        let blocker = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let blocked = blocker.local_addr().unwrap().port();
        start_auth_proxy_internal(
            port,
            "127.0.0.1".into(),
            target,
            "user".into(),
            "pass".into(),
            false,
        )
        .await
        .unwrap();
        let client = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap();
        let root = format!("http://127.0.0.1:{port}");
        let cookie = format!("{}={}", cookie_name(port), get_auth_token());
        assert_eq!(
            client
                .get(&root)
                .header("cookie", &cookie)
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap(),
            "fixture-ok"
        );
        assert_eq!(
            client
                .get(&root)
                .header("cookie", &cookie)
                .header("origin", "https://untrusted.invalid")
                .send()
                .await
                .unwrap()
                .status(),
            403
        );
        let response = client
            .get(format!("{root}/large"))
            .header("cookie", &cookie)
            .send()
            .await
            .unwrap();
        assert_eq!(response.bytes().await.unwrap().len(), 10 * 1024 * 1024);
        let response = client
            .post(&root)
            .header("cookie", &cookie)
            .body(vec![0; 9 * 1024 * 1024])
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 413);
        let response = client
            .get(format!("{root}/ws"))
            .header("cookie", &cookie)
            .header("connection", "upgrade")
            .header("upgrade", "websocket")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 101);
        let mut socket = response.upgrade().await.unwrap();
        socket.write_all(b"abc").await.unwrap();
        let mut buf = [0; 3];
        socket.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"abc");
        assert!(start_auth_proxy_internal(
            blocked,
            "127.0.0.1".into(),
            target,
            "user".into(),
            "pass".into(),
            false
        )
        .await
        .is_err());
        assert_eq!(
            client
                .get(&root)
                .header("cookie", &cookie)
                .send()
                .await
                .unwrap()
                .status(),
            200
        );
        start_auth_proxy_internal(
            next,
            "127.0.0.1".into(),
            target,
            "user".into(),
            "pass".into(),
            false,
        )
        .await
        .unwrap();
        assert!(tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .is_err());
        assert_eq!(
            client
                .get(format!(
                    "http://127.0.0.1:{next}/?_token={}",
                    get_auth_token()
                ))
                .send()
                .await
                .unwrap()
                .status(),
            200
        );
        stop_auth_proxy().await;
        task.abort();
    }
}

async fn read_upload(mut incoming: hyper::body::Incoming) -> Result<Bytes, u16> {
    const LIMIT: usize = 8 * 1024 * 1024;
    let mut result = Vec::new();
    let mut received = 0usize;
    while let Some(frame) = incoming.frame().await {
        let frame = frame.map_err(|_| 400u16)?;
        if let Ok(data) = frame.into_data() {
            received = received.saturating_add(data.len());
            if received <= LIMIT {
                result.extend_from_slice(&data);
            }
            // Drain a bounded excess so ordinary oversized uploads receive the
            // 413 response instead of a TCP reset while they are still writing.
            if received > 4 * LIMIT {
                return Err(413);
            }
        }
    }
    if received > LIMIT {
        Err(413)
    } else {
        Ok(Bytes::from(result))
    }
}
