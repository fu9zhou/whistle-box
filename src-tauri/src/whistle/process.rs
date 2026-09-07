use crate::{config::WhistleConnection, utils};
use std::io::{BufRead, Write};
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use tauri::Manager;
use tokio::sync::Mutex;

pub struct ManagedWhistle {
    pub child: Child,
    pub connection: WhistleConnection,
    #[cfg(windows)]
    _job: ProcessJob,
}
impl Drop for ManagedWhistle {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
    }
}
pub static OWNED: Mutex<Option<ManagedWhistle>> = Mutex::const_new(None);

pub async fn stop() -> Result<(), String> {
    if let Some(mut process) = OWNED.lock().await.take() {
        if process
            .child
            .try_wait()
            .map_err(|e| e.to_string())?
            .is_none()
        {
            process
                .child
                .kill()
                .map_err(|e| format!("Cannot stop owned Whistle: {e}"))?;
        }
        process.child.wait().map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn resource(handle: &tauri::AppHandle, relative: &str) -> Result<PathBuf, String> {
    let mut candidates = Vec::new();
    if cfg!(debug_assertions) {
        candidates.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative));
    }
    if let Ok(dir) = handle.path().resource_dir() {
        candidates.push(dir.join(relative));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join(relative));
        }
    }
    candidates
        .into_iter()
        .find(|p| p.is_file())
        .ok_or_else(|| format!("Required embedded resource missing: {relative}"))
}

pub async fn start(handle: &tauri::AppHandle, conn: &WhistleConnection) -> Result<u32, String> {
    stop().await?;
    let node_name = if cfg!(windows) { "node.exe" } else { "node" };
    let arch = std::env::consts::ARCH;
    let triple = if cfg!(windows) {
        format!("{arch}-pc-windows-msvc")
    } else if cfg!(target_os = "macos") {
        format!("{arch}-apple-darwin")
    } else {
        format!("{arch}-unknown-linux-gnu")
    };
    let dev_name = format!(
        "binaries/node-{triple}{}",
        if cfg!(windows) { ".exe" } else { "" }
    );
    let node = resource(handle, node_name).or_else(|_| resource(handle, &dev_name))?;
    let launcher = resource(handle, "resources/whistle/launcher.cjs")?;
    spawn(node, launcher, conn).await
}

pub async fn spawn(
    node: PathBuf,
    launcher: PathBuf,
    conn: &WhistleConnection,
) -> Result<u32, String> {
    // A failed bind is a conflict, never permission to terminate its owner.
    for port in [conn.port, conn.socks_port].into_iter().filter(|p| *p != 0) {
        let _probe = std::net::TcpListener::bind((conn.host.as_str(), port)).map_err(|_| {
            format!("端口 {port} 已被占用，请修改端口或使用外部模式；未停止占用进程")
        })?;
    }
    let data = if !conn.storage_path.is_empty() {
        PathBuf::from(&conn.storage_path)
    } else if std::env::var_os("WHISTLEBOX_DATA_DIR").is_some() {
        utils::app_data_dir().join("whistle")
    } else {
        dirs::home_dir()
            .ok_or("Cannot locate user home")?
            .join(".WhistleBoxData")
    };
    std::fs::create_dir_all(&data)
        .map_err(|e| format!("Cannot create embedded data directory: {e}"))?;
    let mut options = serde_json::to_value(conn).map_err(|e| e.to_string())?;
    options["baseDir"] = serde_json::Value::String(data.to_string_lossy().into_owned());
    let mut command = Command::new(node);
    command.env_remove("NODE_OPTIONS").env_remove("NODE_PATH");
    command
        .arg(node_script_path(&launcher))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    command.creation_flags(utils::CREATE_NO_WINDOW);
    let mut child = command
        .spawn()
        .map_err(|e| format!("Cannot launch embedded Whistle: {e}"))?;
    #[cfg(windows)]
    let job = match ProcessJob::assign(&child) {
        Ok(job) => job,
        Err(e) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(e);
        }
    };
    for stream in [
        child
            .stdout
            .take()
            .map(|v| Box::new(v) as Box<dyn std::io::Read + Send>),
        child
            .stderr
            .take()
            .map(|v| Box::new(v) as Box<dyn std::io::Read + Send>),
    ]
    .into_iter()
    .flatten()
    {
        let secrets = [
            conn.username.clone(),
            conn.password.clone(),
            conn.upstream_proxy.clone(),
        ];
        std::thread::spawn(move || {
            for line in std::io::BufReader::new(stream)
                .lines()
                .map_while(Result::ok)
            {
                let mut text = line;
                for secret in &secrets {
                    if !secret.is_empty() {
                        text = text.replace(secret, "[redacted]");
                    }
                }
                log::info!(
                    "Embedded Whistle: {}",
                    text.chars().take(2048).collect::<String>()
                );
            }
        });
    }
    let pid = child.id();
    let mut managed = ManagedWhistle {
        child,
        connection: conn.clone(),
        #[cfg(windows)]
        _job: job,
    };
    let input = managed
        .child
        .stdin
        .as_mut()
        .ok_or("Missing child input pipe")?;
    writeln!(input, "{options}")
        .and_then(|_| input.flush())
        .map_err(|e| e.to_string())?;
    for _ in 0..60 {
        if let Some(exit) = managed.child.try_wait().map_err(|e| e.to_string())? {
            return Err(format!("内置 Whistle 启动退出 ({exit})，请查看应用日志"));
        }
        if let Some(info) =
            super::server_info(&conn.host, conn.port, &conn.username, &conn.password).await
        {
            if info["server"]["pid"].as_u64() != Some(pid as u64) {
                return Err("启动端口由其他实例接管；未停止外部实例".into());
            }
            *OWNED.lock().await = Some(managed);
            return Ok(pid);
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
    Err("内置 Whistle 在限定时间内未就绪；请检查日志和端口配置".into())
}

#[cfg(windows)]
struct ProcessJob(windows_sys::Win32::Foundation::HANDLE);
#[cfg(windows)]
unsafe impl Send for ProcessJob {}
#[cfg(windows)]
impl ProcessJob {
    fn assign(child: &Child) -> Result<Self, String> {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::{Foundation::CloseHandle, System::JobObjects::*};
        unsafe {
            let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if handle.is_null() {
                return Err(std::io::Error::last_os_error().to_string());
            }
            let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let set = SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                &limits as *const _ as *const _,
                std::mem::size_of_val(&limits) as u32,
            );
            if set == 0 || AssignProcessToJobObject(handle, child.as_raw_handle()) == 0 {
                let error = std::io::Error::last_os_error();
                CloseHandle(handle);
                return Err(error.to_string());
            }
            Ok(Self(handle))
        }
    }
}
#[cfg(windows)]
impl Drop for ProcessJob {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

// Tauri resource paths can contain a Windows verbatim prefix plus '/' joins.
// Node 22 treats that mixed representation as a drive-relative path.
fn node_script_path(path: &std::path::Path) -> String {
    let raw = path.to_string_lossy();
    #[cfg(windows)]
    {
        let raw = raw.replace('/', "\\");
        if let Some(rest) = raw.strip_prefix("\\\\?\\UNC\\") {
            return format!("\\\\{rest}");
        }
        raw.strip_prefix("\\\\?\\").unwrap_or(&raw).to_string()
    }
    #[cfg(not(windows))]
    {
        raw.into_owned()
    }
}
#[cfg(all(test, windows))]
mod path_tests {
    #[test]
    fn normalizes_tauri_verbatim_resource_path_for_node() {
        let path = std::path::Path::new(r"\\?\D:\apps\WhistleBox\resources/whistle/launcher.cjs");
        assert_eq!(
            super::node_script_path(path),
            r"D:\apps\WhistleBox\resources\whistle\launcher.cjs"
        );
    }
}
