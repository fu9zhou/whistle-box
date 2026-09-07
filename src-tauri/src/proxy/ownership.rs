use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use winreg::{enums::*, RegKey};
const KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Internet Settings";
static LOCK: Mutex<()> = Mutex::new(());

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
struct Snapshot {
    enable: Option<u32>,
    server: Option<String>,
    bypass: Option<String>,
    pac: Option<String>,
}
#[derive(Serialize, Deserialize)]
struct Ownership {
    before: Snapshot,
    applied: Snapshot,
}
fn journal() -> std::path::PathBuf {
    crate::utils::app_data_dir().join("proxy-owner.json")
}
fn read() -> Result<Snapshot, String> {
    let key = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(KEY)
        .map_err(|e| e.to_string())?;
    Ok(Snapshot {
        enable: key.get_value("ProxyEnable").ok(),
        server: key.get_value("ProxyServer").ok(),
        bypass: key.get_value("ProxyOverride").ok(),
        pac: key.get_value("AutoConfigURL").ok(),
    })
}
fn write(value: &Snapshot) -> Result<(), String> {
    let key = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(KEY, KEY_SET_VALUE)
        .map_err(|e| e.to_string())?;
    for (name, value) in [
        ("ProxyServer", &value.server),
        ("ProxyOverride", &value.bypass),
        ("AutoConfigURL", &value.pac),
    ] {
        if let Some(value) = value {
            key.set_value(name, value).map_err(|e| e.to_string())?;
        } else {
            match key.delete_value(name) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.to_string()),
            }
        }
    }
    if let Some(value) = value.enable {
        key.set_value("ProxyEnable", &value)
            .map_err(|e| e.to_string())?;
    } else {
        let _ = key.delete_value("ProxyEnable");
    }
    super::notify_proxy_change();
    Ok(())
}
fn load() -> Result<Option<Ownership>, String> {
    match std::fs::read(journal()) {
        Ok(data) => serde_json::from_slice(&data)
            .map(Some)
            .map_err(|e| format!("Cannot read proxy recovery journal: {e}")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}
fn save(value: &Ownership) -> Result<(), String> {
    std::fs::create_dir_all(crate::utils::app_data_dir()).map_err(|e| e.to_string())?;
    let tmp = journal().with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_vec(value).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    std::fs::rename(tmp, journal()).map_err(|e| e.to_string())
}
fn restore_target(current: &Snapshot, ownership: &Ownership) -> Option<Snapshot> {
    (current == &ownership.applied).then(|| ownership.before.clone())
}
pub fn restore() -> Result<(), String> {
    let _guard = LOCK.lock().map_err(|e| e.to_string())?;
    if let Some(ownership) = load()? {
        if let Some(before) = restore_target(&read()?, &ownership) {
            write(&before)?;
        }
        std::fs::remove_file(journal()).map_err(|e| e.to_string())?;
    }
    Ok(())
}
pub fn apply(host: &str, port: u16, bypass: &str, pac: Option<String>) -> Result<(), String> {
    let _guard = LOCK.lock().map_err(|e| e.to_string())?;
    let current = read()?;
    let previous = load()?;
    let before = match &previous {
        Some(previous) if previous.applied == current => previous.before.clone(),
        _ => current.clone(),
    };
    let applied = if let Some(pac) = pac {
        Snapshot {
            enable: Some(0),
            pac: Some(pac),
            ..current.clone()
        }
    } else {
        Snapshot {
            enable: Some(1),
            server: Some(if host.contains(':') {
                format!("[{host}]:{port}")
            } else {
                format!("{host}:{port}")
            }),
            bypass: Some(bypass.into()),
            pac: None,
        }
    };
    save(&Ownership {
        before,
        applied: applied.clone(),
    })?;
    if let Err(e) = write(&applied) {
        if write(&current).is_ok() {
            if let Some(previous) = previous {
                save(&previous)?;
            } else {
                std::fs::remove_file(journal()).map_err(|e| e.to_string())?;
            }
        }
        return Err(e);
    }
    Ok(())
}
pub fn is_owned() -> Result<bool, String> {
    let _guard = LOCK.lock().map_err(|e| e.to_string())?;
    Ok(load()?.is_some_and(|owner| read().is_ok_and(|current| current == owner.applied)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn restores_only_unchanged_owned_proxy() {
        let before = Snapshot {
            pac: Some("http://company/proxy.pac".into()),
            ..Default::default()
        };
        let applied = Snapshot {
            enable: Some(1),
            server: Some("127.0.0.1:18899".into()),
            ..Default::default()
        };
        let ownership = Ownership {
            before: before.clone(),
            applied: applied.clone(),
        };
        assert_eq!(restore_target(&applied, &ownership), Some(before.clone()));
        assert_eq!(restore_target(&before, &ownership), None);
        let other = Snapshot {
            server: Some("127.0.0.1:8899".into()),
            ..applied
        };
        assert_eq!(restore_target(&other, &ownership), None);
    }
}
