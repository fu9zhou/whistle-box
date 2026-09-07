use std::io::Write;
use std::path::PathBuf;

pub fn app_data_dir() -> PathBuf {
    std::env::var_os("WHISTLEBOX_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            dirs::config_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("WhistleBox")
        })
}

pub fn http_url(host: &str, port: u16, path: &str) -> String {
    let host = host.trim_matches(['[', ']']);
    let host = if host.contains(':') {
        format!("[{host}]")
    } else {
        host.to_string()
    };
    format!("http://{host}:{port}{path}")
}

struct BoundedLog {
    path: PathBuf,
}
impl Write for BoundedLog {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if self.path.metadata().map(|m| m.len()).unwrap_or(0) > 2 * 1024 * 1024 {
            let backup = self.path.with_extension("previous.log");
            if backup.exists() {
                std::fs::remove_file(&backup)?;
            }
            std::fs::rename(&self.path, backup)?;
        }
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?
            .write_all(buf)?;
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
pub fn init_logging() {
    let dir = app_data_dir();
    if std::fs::create_dir_all(&dir).is_ok() {
        let _ = env_logger::Builder::from_env(
            env_logger::Env::default().filter_or("WHISTLEBOX_LOG", "info"),
        )
        .target(env_logger::Target::Pipe(Box::new(BoundedLog {
            path: dir.join("whistlebox.log"),
        })))
        .try_init();
    } else {
        let _ = env_logger::try_init();
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn ipv6_urls_are_well_formed() {
        assert_eq!(super::http_url("::1", 8899, "/"), "http://[::1]:8899/");
        assert_eq!(
            super::http_url("127.0.0.1", 8899, "/"),
            "http://127.0.0.1:8899/"
        );
    }
}
