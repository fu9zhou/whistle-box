#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

use std::net::IpAddr;

#[cfg(target_os = "windows")]
pub const CREATE_NO_WINDOW: u32 = 0x08000000;

pub fn is_private_or_loopback(host: &str) -> bool {
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        return match ip {
            IpAddr::V4(v4) => v4.is_loopback() || v4.is_private(),
            IpAddr::V6(v6) => v6.is_loopback(),
        };
    }
    false
}

pub fn kill_process(pid: u32) {
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("taskkill")
            .args(["/F", "/PID", &pid.to_string()])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = std::process::Command::new("kill")
            .args(["-9", &pid.to_string()])
            .output();
    }
}

fn is_whistle_or_app_process(pid: u32) -> bool {
    #[cfg(target_os = "windows")]
    {
        if let Ok(output) = std::process::Command::new("wmic")
            .args(["process", "where", &format!("ProcessId={}", pid), "get", "Name,ExecutablePath", "/format:list"])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
        {
            let text = String::from_utf8_lossy(&output.stdout).to_lowercase();
            return text.contains("node") || text.contains("whistle") || text.contains("whistlebox");
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(output) = std::process::Command::new("ps")
            .args(["-p", &pid.to_string(), "-o", "comm=", "-o", "args="])
            .output()
        {
            let text = String::from_utf8_lossy(&output.stdout).to_lowercase();
            return text.contains("node") || text.contains("whistle") || text.contains("whistlebox");
        }
    }
    false
}

pub fn kill_whistle_by_session() {
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("wmic")
            .args(["process", "where", "commandline like '%whistlebox_embedded%' and name='node.exe'", "call", "terminate"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
    }
}

pub fn is_port_free(port: u16) -> bool {
    std::net::TcpListener::bind(("127.0.0.1", port)).is_ok()
}

pub fn kill_process_by_port(port: u16) {
    #[cfg(target_os = "windows")]
    {
        let output = std::process::Command::new("netstat")
            .args(["-ano"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
        if let Ok(output) = output {
            let text = String::from_utf8_lossy(&output.stdout);
            let target_port = port.to_string();
            for line in text.lines() {
                if !line.contains("LISTENING") {
                    continue;
                }
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() < 5 {
                    continue;
                }
                let addr = parts[1].trim_start_matches('[');
                let matches_port = addr
                    .rsplit_once(':')
                    .map(|(_, p)| p.trim_end_matches(']') == target_port.as_str())
                    .unwrap_or(false);
                if matches_port {
                    if let Ok(pid) = parts[parts.len() - 1].parse::<u32>() {
                        if pid > 0 {
                            if is_whistle_or_app_process(pid) {
                                log::info!("Killing process on port {} with PID {}", port, pid);
                                kill_process(pid);
                            } else {
                                log::warn!("Skipping non-WhistleBox process on port {} (PID {})", port, pid);
                            }
                        }
                    }
                }
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let output = std::process::Command::new("lsof")
            .args(["-ti", &format!(":{}", port)])
            .output();
        if let Ok(output) = output {
            let text = String::from_utf8_lossy(&output.stdout);
            for pid_str in text.trim().lines() {
                if let Ok(pid) = pid_str.trim().parse::<u32>() {
                    if pid > 0 {
                        if is_whistle_or_app_process(pid) {
                            log::info!("Killing process on port {} with PID {}", port, pid);
                            kill_process(pid);
                        } else {
                            log::warn!("Skipping non-WhistleBox process on port {} (PID {})", port, pid);
                        }
                    }
                }
            }
        }
    }
}
