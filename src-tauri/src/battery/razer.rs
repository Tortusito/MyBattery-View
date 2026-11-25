use super::{clean_device_name, infer_category, BatteryDevice};
use std::collections::HashSet;
use windows::core::PCWSTR;
use windows::Win32::Devices::DeviceAndDriverInstallation::{
    SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInterfaces, SetupDiGetClassDevsW,
    SetupDiGetDeviceInterfaceDetailW, DIGCF_DEVICEINTERFACE, DIGCF_PRESENT,
    SP_DEVICE_INTERFACE_DATA, SP_DEVICE_INTERFACE_DETAIL_DATA_W,
};
use windows::Win32::Devices::HumanInterfaceDevice::{
    HidD_FreePreparsedData, HidD_GetAttributes, HidD_GetFeature, HidD_GetHidGuid,
    HidD_GetPreparsedData, HidD_GetProductString, HidD_SetFeature, HidP_GetCaps,
    HIDD_ATTRIBUTES, HIDP_CAPS, HIDP_STATUS_SUCCESS, PHIDP_PREPARSED_DATA,
};
use windows::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};

const RAZER_VID: u16 = 0x1532;

pub fn read_razer_batteries() -> Vec<BatteryDevice> {
    let mut list = Vec::new();
    let mut seen = HashSet::new();

    unsafe {
        let hid_guid = HidD_GetHidGuid();
        let Ok(dev_info) = SetupDiGetClassDevsW(
            Some(&hid_guid),
            None,
            None,
            DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
        ) else {
            return list;
        };

        let mut iface_data = SP_DEVICE_INTERFACE_DATA {
            cbSize: std::mem::size_of::<SP_DEVICE_INTERFACE_DATA>() as u32,
            ..Default::default()
        };

        let mut index = 0u32;
        while SetupDiEnumDeviceInterfaces(dev_info, None, &hid_guid, index, &mut iface_data).is_ok() {
            index += 1;
            let Some(path_u16) = get_interface_path(dev_info, &iface_data) else {
                continue;
            };
            if !seen.insert(path_u16.clone()) {
                continue;
            }
            if let Some(dev) = try_query_razer(&path_u16) {
                list.push(dev);
            }
        }

        let _ = SetupDiDestroyDeviceInfoList(dev_info);
    }

    list
}

unsafe fn get_interface_path(
    dev_info: windows::Win32::Devices::DeviceAndDriverInstallation::HDEVINFO,
    iface_data: &SP_DEVICE_INTERFACE_DATA,
) -> Option<Vec<u16>> {
    let mut req_size = 0u32;
    let _ = SetupDiGetDeviceInterfaceDetailW(dev_info, iface_data, None, 0, Some(&mut req_size), None);
    if req_size <= 4 {
        return None;
    }
    let mut buf = vec![0u8; req_size as usize];
    let detail_ptr = buf.as_mut_ptr() as *mut SP_DEVICE_INTERFACE_DETAIL_DATA_W;
    (*detail_ptr).cbSize = std::mem::size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32;
    if SetupDiGetDeviceInterfaceDetailW(dev_info, iface_data, Some(detail_ptr), req_size, None, None).is_err() {
        return None;
    }
    let path_ptr = std::ptr::addr_of!((*detail_ptr).DevicePath) as *const u16;
    let mut len = 0usize;
    while *path_ptr.add(len) != 0 {
        len += 1;
    }
    let mut path = std::slice::from_raw_parts(path_ptr, len).to_vec();
    path.push(0);
    Some(path)
}

unsafe fn try_query_razer(path_u16: &[u16]) -> Option<BatteryDevice> {
    let handle: HANDLE = CreateFileW(
        PCWSTR(path_u16.as_ptr()),
        0,
        FILE_SHARE_READ | FILE_SHARE_WRITE,
        None,
        OPEN_EXISTING,
        FILE_FLAGS_AND_ATTRIBUTES(0),
        None,
    )
    .ok()?;

    if handle == INVALID_HANDLE_VALUE {
        return None;
    }

    struct HandleGuard(HANDLE);
    impl Drop for HandleGuard {
        fn drop(&mut self) {
            unsafe { let _ = CloseHandle(self.0); }
        }
    }
    let _guard = HandleGuard(handle);

    let mut attrs = HIDD_ATTRIBUTES {
        Size: std::mem::size_of::<HIDD_ATTRIBUTES>() as u32,
        ..Default::default()
    };
    if !HidD_GetAttributes(handle, &mut attrs).as_bool() || attrs.VendorID != RAZER_VID {
        return None;
    }

    let mut preparsed = PHIDP_PREPARSED_DATA::default();
    if !HidD_GetPreparsedData(handle, &mut preparsed).as_bool() || preparsed.0 == 0 {
        return None;
    }
    let mut caps = HIDP_CAPS::default();
    let ok_caps = HidP_GetCaps(preparsed, &mut caps) == HIDP_STATUS_SUCCESS;
    let _ = HidD_FreePreparsedData(preparsed);

    if !ok_caps || caps.FeatureReportByteLength < 90 {
        return None;
    }

    // Razer HID feature packet: 91 bytes (Report ID 0x00 + 90-byte Razer payload)
    let mut report = vec![0u8; caps.FeatureReportByteLength as usize];
    report[0] = 0x00; // Report ID
    report[2] = 0x1F; // Transaction ID
    report[6] = 0x02; // Data size
    report[7] = 0x07; // Command class: Power
    report[8] = 0x80; // Command ID: Get battery level

    let mut crc = 0u8;
    for b in &report[3..89] {
        crc ^= *b;
    }
    report[89] = crc;

    if !HidD_SetFeature(handle, report.as_ptr() as *const _, report.len() as u32).as_bool() {
        return None;
    }
    std::thread::sleep(std::time::Duration::from_millis(25));

    let mut resp = vec![0u8; caps.FeatureReportByteLength as usize];
    resp[0] = 0x00;
    if !HidD_GetFeature(handle, resp.as_mut_ptr() as *mut _, resp.len() as u32).as_bool() {
        return None;
    }

    // Status byte at resp[1] == 0x02 (success)
    if resp[1] != 0x02 {
        return None;
    }

    let raw_level = resp[10];
    let percent = ((raw_level as f64 / 255.0) * 100.0).round() as u8;
    if percent == 0 {
        return None;
    }

    let mut name_buf = [0u8; 256];
    let raw_name = if HidD_GetProductString(handle, name_buf.as_mut_ptr() as *mut _, name_buf.len() as u32).as_bool() {
        let u16s: Vec<u16> = name_buf
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .take_while(|&c| c != 0)
            .collect();
        String::from_utf16_lossy(&u16s)
    } else {
        "Razer Wireless".to_string()
    };

    let name = clean_device_name(&raw_name);
    let id = format!("razer-{:04x}-{:04x}", attrs.VendorID, attrs.ProductID);

    Some(BatteryDevice {
        id,
        category: infer_category(&name),
        name,
        percent: percent.min(100),
        connection: "Razer 2.4G".to_string(),
        charging: false,
    })
}
