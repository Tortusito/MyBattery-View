use super::BatteryDevice;
use windows::Win32::UI::Input::XboxController::{
    XInputGetBatteryInformation, BATTERY_DEVTYPE_GAMEPAD, BATTERY_LEVEL_EMPTY, BATTERY_LEVEL_LOW,
    BATTERY_LEVEL_MEDIUM, BATTERY_TYPE_DISCONNECTED, BATTERY_TYPE_WIRED, XINPUT_BATTERY_INFORMATION,
};

pub fn read_xbox_controllers() -> Vec<BatteryDevice> {
    let mut list = Vec::new();

    unsafe {
        for idx in 0..4u32 {
            let mut info = XINPUT_BATTERY_INFORMATION::default();
            if XInputGetBatteryInformation(idx, BATTERY_DEVTYPE_GAMEPAD, &mut info) != 0 {
                continue;
            }
            if info.BatteryType == BATTERY_TYPE_DISCONNECTED || info.BatteryType == BATTERY_TYPE_WIRED {
                continue;
            }

            let percent = match info.BatteryLevel {
                x if x == BATTERY_LEVEL_EMPTY => 10,
                x if x == BATTERY_LEVEL_LOW => 35,
                x if x == BATTERY_LEVEL_MEDIUM => 70,
                _ => 100,
            };

            list.push(BatteryDevice {
                id: format!("xinput-{}", idx),
                name: format!("Mando Xbox #{}", idx + 1),
                percent,
                category: "gamepad".to_string(),
                connection: "Xbox".to_string(),
                charging: false,
            });
        }
    }

    list
}
