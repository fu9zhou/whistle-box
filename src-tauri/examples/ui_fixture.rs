use std::io::{BufRead, Write};
use whistle_box_lib::{auth, config::WhistleConnection, whistle::process};
#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let port: u16 = args[1].parse().unwrap();
    let auth_port: u16 = args[2].parse().unwrap();
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let conn = WhistleConnection {
        port,
        username: "fixture".into(),
        password: "fixture-secret".into(),
        storage_path: args[3].clone(),
        ..Default::default()
    };
    let node = root.join("binaries/node-x86_64-pc-windows-msvc.exe");
    let launcher = root.join("resources/whistle/launcher.cjs");
    let pid = process::spawn(node, launcher, &conn).await.unwrap();
    auth::start_auth_proxy_internal(
        auth_port,
        "127.0.0.1".into(),
        port,
        conn.username,
        conn.password,
        false,
    )
    .await
    .unwrap();
    println!(
        "{}",
        serde_json::json!({"pid":pid,"url":format!("http://127.0.0.1:{auth_port}/?_token={}",auth::get_auth_token())})
    );
    std::io::stdout().flush().unwrap();
    tokio::task::spawn_blocking(|| {
        let _ = std::io::stdin().lock().lines().next();
    })
    .await
    .unwrap();
    auth::stop_auth_proxy().await;
    process::stop().await.unwrap();
}
