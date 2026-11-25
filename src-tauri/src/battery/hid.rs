use super::{clean_device_name, infer_category, BatteryDevice};
use std::collections::HashSet;
use windows::core::PCWSTR;
use windows::Win32::Devices::DeviceAndDriverInstallation::{
    SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInterfaces, SetupDiGetClassDevsW,
    SetupDiGetDeviceInterfaceDetailW, DIGCF_DEVICEINTERFACE, DIGCF_PRESENT,
    SP_DEVICE_INTERFACE_DATA, SP_DEVICE_INTERFACE_DETAIL_DATA_W,
};
use windows::Win32::Devices::HumanInterfaceDevice::{
    HidD_FreePreparsedData, HidD_GetFeature, HidD_GetHidGuid, HidD_GetInputReport,
    HidD_GetPreparsedData, HidD_GetProductString, HidP_Feature, HidP_GetCaps,
    HidP_GetUsageValue, HidP_GetValueCaps, HidP_Input, HIDP_CAPS, HIDP_REPORT_TYPE,
    HIDP_STATUS_SUCCESS, HIDP_VALUE_CAPS, PHIDP_PREPARSED_DATA,
};
use windows::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};

const USAGE_PAGE_GENERIC_DEVICE: u16 = 0x06;
const USAGE_BATTERY_STRENGTH: u16 = 0x20;
const USAGE_PAGE_BATTERY_SYSTEM: u16 = 0x85;
const USAGE_ABSOLUTE_STATE_OF_CHARGE: u16 = 0x65;
const USAGE_REMAINING_CAPACITY: u16 = 0x66;

pub fn read_hid_batteries() -> Vec<BatteryDevice> {
    let mut list = Vec::new();
    let mut seen_paths = HashSet::new();

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
            if let Some(path_u16) = get_interface_path(dev_info, &iface_data) {
                if !seen_paths.insert(path_u16.clone()) {
                    continue;
                }
                if let Some(dev) = try_read_device(&path_u16) {
                    list.push(dev);
                }
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

unsafe fn try_read_device(path_u16: &[u16]) -> Option<BatteryDevice> {
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

    let mut preparsed = PHIDP_PREPARSED_DATA::default();
    if !HidD_GetPreparsedData(handle, &mut preparsed).as_bool() || preparsed.0 == 0 {
        return None;
    }

    struct PreparsedGuard(PHIDP_PREPARSED_DATA);
    impl Drop for PreparsedGuard {
        fn drop(&mut self) {
            unsafe { let _ = HidD_FreePreparsedData(self.0); }
        }
    }
    let _pguard = PreparsedGuard(preparsed);

    let mut caps = HIDP_CAPS::default();
    if HidP_GetCaps(preparsed, &mut caps) != HIDP_STATUS_SUCCESS {
        return None;
    }

    let percent = extract_battery(handle, preparsed, HidP_Feature, caps.NumberFeatureValueCaps, caps.FeatureReportByteLength)
        .or_else(|| extract_battery(handle, preparsed, HidP_Input, caps.NumberInputValueCaps, caps.InputReportByteLength))?;

    let mut name_buf = [0u8; 256];
    let raw_name = if HidD_GetProductString(handle, name_buf.as_mut_ptr() as *mut _, name_buf.len() as u32).as_bool() {
        let u16s: Vec<u16> = name_buf
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .take_while(|&c| c != 0)
            .collect();
        String::from_utf16_lossy(&u16s)
    } else {
        "Periférico inalámbrico".to_string()
    };

    let name = clean_device_name(&raw_name);
    let id = String::from_utf16_lossy(&path_u16[..path_u16.len().saturating_sub(1)]);

    Some(BatteryDevice {
        id,
        category: infer_category(&name),
        name,
        percent,
        connection: "Inalámbrico".to_string(),
        charging: false,
    })
}

unsafe fn extract_battery(
    handle: HANDLE,
    preparsed: PHIDP_PREPARSED_DATA,
    report_type: HIDP_REPORT_TYPE,
    value_caps_count: u16,
    report_byte_len: u16,
) -> Option<u8> {
    if value_caps_count == 0 || report_byte_len == 0 {
        return None;
    }

    let mut value_caps = vec![HIDP_VALUE_CAPS::default(); value_caps_count as usize];
    let mut len = value_caps_count;
    if HidP_GetValueCaps(report_type, value_caps.as_mut_ptr(), &mut len, preparsed) != HIDP_STATUS_SUCCESS {
        return None;
    }

    for cap in value_caps.iter().take(len as usize) {
        if cap.IsRange.as_bool() {
            continue;
        }
        let usage = cap.Anonymous.NotRange.Usage;
        let is_battery = (cap.UsagePage == USAGE_PAGE_BATTERY_SYSTEM
            && (usage == USAGE_ABSOLUTE_STATE_OF_CHARGE || usage == USAGE_REMAINING_CAPACITY))
            || (cap.UsagePage == USAGE_PAGE_GENERIC_DEVICE && usage == USAGE_BATTERY_STRENGTH);

        if !is_battery {
            continue;
        }

        let mut report = vec![0u8; report_byte_len as usize];
        report[0] = cap.ReportID;

        let ok = if report_type == HidP_Feature {
            HidD_GetFeature(handle, report.as_mut_ptr() as *mut _, report.len() as u32).as_bool()
        } else {
            HidD_GetInputReport(handle, report.as_mut_ptr() as *mut _, report.len() as u32).as_bool()
        };

        if !ok {
            continue;
        }

        // Ignore empty/all-zero payloads (common on sleeping/idle 2.4 GHz dongles)
        if report.len() > 1 && report[1..].iter().all(|&b| b == 0) {
            continue;
        }

        let mut raw_val = 0u32;
        if HidP_GetUsageValue(
            report_type,
            cap.UsagePage,
            cap.LinkCollection,
            usage,
            &mut raw_val,
            preparsed,
            &report,
        ) == HIDP_STATUS_SUCCESS
        {
            let max = if cap.LogicalMax > cap.LogicalMin { cap.LogicalMax } else { 100 };
            let min = cap.LogicalMin;
            if max <= min || (raw_val as i32) < min || (raw_val as i32) > max {
                continue;
            }
            let pct = (((raw_val as i64 - min as i64) as f64 / (max - min) as f64) * 100.0).round() as i64;
            if report_type == HidP_Input && pct == 0 {
                continue;
            }
            return Some(pct.clamp(0, 100) as u8);
        }
    }

    None
}
