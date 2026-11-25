use serde::Serialize;
use std::collections::HashMap;

#[cfg(windows)]
mod ble;
#[cfg(windows)]
mod bluetooth_pnp;
#[cfg(windows)]
mod hid;
#[cfg(windows)]
mod xinput;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatteryDevice {
    pub id: String,
    pub name: String,
    pub percent: u8,
    pub category: String,
    pub connection: String,
    pub charging: bool,
}

pub fn clean_device_name(name: &str) -> String {
    let cleaned = name
        .replace(" Hands-Free AG Audio", "")
        .replace(" Hands-Free", "")
        .replace(" Stereo", "")
        .trim()
        .to_string();
    if cleaned.is_empty() {
        "Dispositivo inalámbrico".to_string()
    } else {
        cleaned
    }
}

pub fn infer_category(name: &str) -> String {
    let lower = name.to_lowercase();
    if lower.contains("airpod")
        || lower.contains("bud")
        || lower.contains("head")
        || lower.contains("pod")
        || lower.contains("wh-")
        || lower.contains("wf-")
        || lower.contains("bose")
        || lower.contains("jabra")
        || lower.contains("beats")
        || lower.contains("audio")
        || lower.contains("ear")
    {
        return "headphones".to_string();
    }
    if lower.contains("mouse")
        || lower.contains("ratón")
        || lower.contains("raton")
        || lower.contains("mx master")
        || lower.contains("mx anywhere")
        || lower.contains("viper")
        || lower.contains("deathadder")
        || lower.contains("g pro")
    {
        return "mouse".to_string();
    }
    if lower.contains("keyboard")
        || lower.contains("teclado")
        || lower.contains("keychron")
        || lower.contains("mx keys")
        || lower.contains("k380")
        || lower.contains("anne pro")
    {
        return "keyboard".to_string();
    }
    if lower.contains("xbox")
        || lower.contains("dualsense")
        || lower.contains("dualshock")
        || lower.contains("controller")
        || lower.contains("gamepad")
        || lower.contains("mando")
        || lower.contains("8bitdo")
        || lower.contains("joy-con")
    {
        return "gamepad".to_string();
    }
    "generic".to_string()
}

pub fn collect_all_devices() -> Vec<BatteryDevice> {
    #[cfg(windows)]
    {
        let mut map: HashMap<String, BatteryDevice> = HashMap::new();

        for dev in bluetooth_pnp::read_pnp_batteries() {
            map.insert(dev.name.to_lowercase(), dev);
        }

        for dev in ble::read_ble_batteries() {
            map.entry(dev.name.to_lowercase()).or_insert(dev);
        }

        for dev in hid::read_hid_batteries() {
            map.entry(dev.name.to_lowercase()).or_insert(dev);
        }

        for dev in xinput::read_xbox_controllers() {
            let has_xbox = map.values().any(|d| {
                d.category == "gamepad" && d.name.to_lowercase().contains("xbox")
            });
            if !has_xbox {
                map.insert(dev.id.clone(), dev);
            }
        }

        let mut list: Vec<BatteryDevice> = map.into_values().collect();
        list.sort_by(|a, b| a.percent.cmp(&b.percent).then_with(|| a.name.cmp(&b.name)));
        list
    }

    #[cfg(not(windows))]
    {
        Vec::new()
    }
}
