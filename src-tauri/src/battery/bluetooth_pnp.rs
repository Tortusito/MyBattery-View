use super::{clean_device_name, infer_category, BatteryDevice};
use std::collections::HashSet;
use windows::core::GUID;
use windows::Win32::Devices::DeviceAndDriverInstallation::{
    SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInfo, SetupDiGetClassDevsW,
    SetupDiGetDevicePropertyW, DIGCF_ALLCLASSES, DIGCF_PRESENT, SP_DEVINFO_DATA,
};
use windows::Win32::Devices::Properties::{DEVPROPKEY, DEVPROPTYPE};

const DEVPKEY_BLUETOOTH_BATTERY: DEVPROPKEY = DEVPROPKEY {
    fmtid: GUID::from_u128(0x104EA319_6EE2_4701_BD47_8DDBF425BBE5),
    pid: 2,
};

const DEVPKEY_DEVICE_IS_CONNECTED: DEVPROPKEY = DEVPROPKEY {
    fmtid: GUID::from_u128(0x83DA6326_97A6_4088_9453_A1923F573B29),
    pid: 15,
};

const DEVPKEY_DEVICE_FRIENDLY_NAME: DEVPROPKEY = DEVPROPKEY {
    fmtid: GUID::from_u128(0xA45C254E_DF1C_4EFD_8020_67D146A850E0),
    pid: 14,
};

const DEVPKEY_NAME: DEVPROPKEY = DEVPROPKEY {
    fmtid: GUID::from_u128(0xB725F130_47EF_101A_A5F1_02608C9EEBAC),
    pid: 10,
};

const DEVPKEY_DEVICE_CONTAINER_ID: DEVPROPKEY = DEVPROPKEY {
    fmtid: GUID::from_u128(0x8C7ED206_3F8A_4827_B3AB_AE9E1FAEFC6C),
    pid: 2,
};

pub fn read_pnp_batteries() -> Vec<BatteryDevice> {
    let mut list = Vec::new();
    let mut seen_containers: HashSet<[u8; 16]> = HashSet::new();

    unsafe {
        let Ok(dev_info) = SetupDiGetClassDevsW(None, None, None, DIGCF_ALLCLASSES | DIGCF_PRESENT) else {
            return list;
        };

        let mut dev_data = SP_DEVINFO_DATA {
            cbSize: std::mem::size_of::<SP_DEVINFO_DATA>() as u32,
            ..Default::default()
        };

        let mut index = 0u32;
        while SetupDiEnumDeviceInfo(dev_info, index, &mut dev_data).is_ok() {
            index += 1;

            let Some(battery) = get_byte_prop(dev_info, &dev_data, &DEVPKEY_BLUETOOTH_BATTERY) else {
                continue;
            };
            if battery > 100 {
                continue;
            }

            if let Some(connected) = get_byte_prop(dev_info, &dev_data, &DEVPKEY_DEVICE_IS_CONNECTED) {
                if connected == 0 {
                    continue;
                }
            }

            let container_bytes = get_guid_bytes(dev_info, &dev_data, &DEVPKEY_DEVICE_CONTAINER_ID);
            if let Some(cb) = container_bytes {
                if cb != [0u8; 16] && !seen_containers.insert(cb) {
                    continue;
                }
            }

            let raw_name = get_string_prop(dev_info, &dev_data, &DEVPKEY_DEVICE_FRIENDLY_NAME)
                .or_else(|| get_string_prop(dev_info, &dev_data, &DEVPKEY_NAME))
                .unwrap_or_else(|| "Dispositivo Bluetooth".to_string());

            let name = clean_device_name(&raw_name);
            let id = container_bytes
                .map(|b| format!("pnp-{}", b.iter().map(|x| format!("{:02x}", x)).collect::<String>()))
                .unwrap_or_else(|| format!("pnp-{}", name.to_lowercase()));

            list.push(BatteryDevice {
                id,
                category: infer_category(&name),
                name,
                percent: battery,
                connection: "BT".to_string(),
                charging: false,
            });
        }

        let _ = SetupDiDestroyDeviceInfoList(dev_info);
    }

    list
}

unsafe fn get_byte_prop(
    dev_info: windows::Win32::Devices::DeviceAndDriverInstallation::HDEVINFO,
    dev_data: &SP_DEVINFO_DATA,
    key: &DEVPROPKEY,
) -> Option<u8> {
    let mut prop_type = DEVPROPTYPE::default();
    let mut buf = [0u8; 1];
    let mut req_size = 0u32;
    if SetupDiGetDevicePropertyW(
        dev_info,
        dev_data,
        key,
        &mut prop_type,
        Some(&mut buf),
        Some(&mut req_size),
        0,
    )
    .is_ok()
    {
        Some(buf[0])
    } else {
        None
    }
}

unsafe fn get_guid_bytes(
    dev_info: windows::Win32::Devices::DeviceAndDriverInstallation::HDEVINFO,
    dev_data: &SP_DEVINFO_DATA,
    key: &DEVPROPKEY,
) -> Option<[u8; 16]> {
    let mut prop_type = DEVPROPTYPE::default();
    let mut buf = [0u8; 16];
    let mut req_size = 0u32;
    if SetupDiGetDevicePropertyW(
        dev_info,
        dev_data,
        key,
        &mut prop_type,
        Some(&mut buf),
        Some(&mut req_size),
        0,
    )
    .is_ok()
    {
        Some(buf)
    } else {
        None
    }
}

unsafe fn get_string_prop(
    dev_info: windows::Win32::Devices::DeviceAndDriverInstallation::HDEVINFO,
    dev_data: &SP_DEVINFO_DATA,
    key: &DEVPROPKEY,
) -> Option<String> {
    let mut prop_type = DEVPROPTYPE::default();
    let mut req_size = 0u32;
    let _ = SetupDiGetDevicePropertyW(dev_info, dev_data, key, &mut prop_type, None, Some(&mut req_size), 0);
    if req_size <= 2 {
        return None;
    }

    let mut buf = vec![0u8; req_size as usize];
    if SetupDiGetDevicePropertyW(
        dev_info,
        dev_data,
        key,
        &mut prop_type,
        Some(&mut buf),
        Some(&mut req_size),
        0,
    )
    .is_err()
    {
        return None;
    }

    let u16s: Vec<u16> = buf
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .take_while(|&c| c != 0)
        .collect();
    let s = String::from_utf16_lossy(&u16s).trim().to_string();
    if s.is_empty() { None } else { Some(s) }
}
