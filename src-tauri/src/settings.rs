use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettings {
    pub theme: String,
    pub accent_color: String,
    pub icon_style: String,
    pub poll_interval_sec: u64,
    pub notify_low: bool,
    pub notify_threshold: u8,
    pub launch_at_login: bool,
    pub custom_icons: HashMap<String, String>,
    pub custom_names: HashMap<String, String>,
    pub hidden_devices: Vec<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: "system".to_string(),
            accent_color: "wyforce".to_string(),
            icon_style: "line".to_string(),
            poll_interval_sec: 30,
            notify_low: true,
            notify_threshold: 20,
            launch_at_login: true,
            custom_icons: HashMap::new(),
            custom_names: HashMap::new(),
            hidden_devices: Vec::new(),
        }
    }
}

fn settings_path() -> PathBuf {
    #[cfg(windows)]
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    #[cfg(not(windows))]
    let base = std::env::var_os("HOME")
        .map(|h| PathBuf::from(h).join(".config"))
        .unwrap_or_else(|| PathBuf::from("."));

    base.join("MyBatteryView").join("settings.json")
}

pub fn load_settings() -> AppSettings {
    let path = settings_path();
    let Ok(raw) = std::fs::read_to_string(&path) else {
        // First run: enable autostart by default and persist initial settings
        let defaults = AppSettings::default();
        save_settings(&defaults);
        return defaults;
    };
    let mut s: AppSettings = serde_json::from_str(&raw).unwrap_or_default();
    s.launch_at_login = is_startup_enabled();
    s
}

pub fn save_settings(settings: &AppSettings) {
    let path = settings_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(settings) {
        let _ = std::fs::write(path, json);
    }
    set_startup_enabled(settings.launch_at_login);
}

#[cfg(windows)]
pub fn is_startup_enabled() -> bool {
    use windows::core::w;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ,
    };

    unsafe {
        let mut hkey = HKEY::default();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run"),
            0,
            KEY_READ,
            &mut hkey,
        )
        .is_err()
        {
            return false;
        }
        let exists = RegQueryValueExW(hkey, w!("MyBatteryView"), None, None, None, None).is_ok();
        let _ = RegCloseKey(hkey);
        exists
    }
}

#[cfg(not(windows))]
pub fn is_startup_enabled() -> bool {
    false
}

#[cfg(windows)]
pub fn set_startup_enabled(enable: bool) {
    use windows::core::w;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
        KEY_WRITE, REG_SZ,
    };

    unsafe {
        let mut hkey = HKEY::default();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run"),
            0,
            KEY_WRITE,
            &mut hkey,
        )
        .is_err()
        {
            return;
        }

        if enable {
            if let Ok(exe) = std::env::current_exe() {
                let val = format!("\"{}\" --minimized", exe.display());
                let mut wide: Vec<u16> = val.encode_utf16().collect();
                wide.push(0);
                let bytes = std::slice::from_raw_parts(wide.as_ptr() as *const u8, wide.len() * 2);
                let _ = RegSetValueExW(hkey, w!("MyBatteryView"), 0, REG_SZ, Some(bytes));
            }
        } else {
            let _ = RegDeleteValueW(hkey, w!("MyBatteryView"));
        }

        let _ = RegCloseKey(hkey);
    }
}

#[cfg(not(windows))]
pub fn set_startup_enabled(_enable: bool) {}
