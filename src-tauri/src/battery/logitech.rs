use super::{clean_device_name, infer_category, BatteryDevice};
use std::collections::HashSet;
use windows::core::PCWSTR;
use windows::Win32::Devices::DeviceAndDriverInstallation::{
    SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInterfaces, SetupDiGetClassDevsW,
    SetupDiGetDeviceInterfaceDetailW, DIGCF_DEVICEINTERFACE, DIGCF_PRESENT,
    SP_DEVICE_INTERFACE_DATA, SP_DEVICE_INTERFACE_DETAIL_DATA_W,
};
use windows::Win32::Devices::HumanInterfaceDevice::{
    HidD_FreePreparsedData, HidD_GetAttributes, HidD_GetHidGuid, HidD_GetPreparsedData,
    HidD_GetProductString, HidP_GetCaps, HIDD_ATTRIBUTES, HIDP_CAPS, HIDP_STATUS_SUCCESS,
    PHIDP_PREPARSED_DATA,
};
use windows::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_IO_PENDING, HANDLE, INVALID_HANDLE_VALUE, WAIT_OBJECT_0,
};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, ReadFile, WriteFile, FILE_FLAGS_AND_ATTRIBUTES, FILE_FLAG_OVERLAPPED,
    FILE_GENERIC_READ, FILE_GENERIC_WRITE, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject};
use windows::Win32::System::IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED};

const LOGITECH_VID: u16 = 0x046D;

pub fn read_logitech_batteries() -> Vec<BatteryDevice> {
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
            if let Some(dev) = try_query_logitech(&path_u16) {
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

unsafe fn try_query_logitech(path_u16: &[u16]) -> Option<BatteryDevice> {
    let handle: HANDLE = CreateFileW(
        PCWSTR(path_u16.as_ptr()),
        FILE_GENERIC_READ.0 | FILE_GENERIC_WRITE.0,
        FILE_SHARE_READ | FILE_SHARE_WRITE,
        None,
        OPEN_EXISTING,
        FILE_FLAGS_AND_ATTRIBUTES(FILE_FLAG_OVERLAPPED.0),
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
    if !HidD_GetAttributes(handle, &mut attrs).as_bool() || attrs.VendorID != LOGITECH_VID {
        return None;
    }

    let mut preparsed = PHIDP_PREPARSED_DATA::default();
    if !HidD_GetPreparsedData(handle, &mut preparsed).as_bool() || preparsed.0 == 0 {
        return None;
    }

    let mut caps = HIDP_CAPS::default();
    let caps_ok = HidP_GetCaps(preparsed, &mut caps) == HIDP_STATUS_SUCCESS;
    let _ = HidD_FreePreparsedData(preparsed);

    if !caps_ok || caps.UsagePage != 0xFF00 || caps.OutputReportByteLength < 20 || caps.InputReportByteLength < 20 {
        return None;
    }

    // Try direct device index (0xFF) and receiver slot 1 (0x01)
    let mut pct_found: Option<u8> = None;
    for dev_idx in [0xFFu8, 0x01u8] {
        if let Some(pct) = query_hidpp_battery(handle, dev_idx, caps.OutputReportByteLength as usize, caps.InputReportByteLength as usize) {
            pct_found = Some(pct);
            break;
        }
    }

    let percent = pct_found?;

    let mut name_buf = [0u8; 256];
    let raw_name = if HidD_GetProductString(handle, name_buf.as_mut_ptr() as *mut _, name_buf.len() as u32).as_bool() {
        let u16s: Vec<u16> = name_buf
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .take_while(|&c| c != 0)
            .collect();
        String::from_utf16_lossy(&u16s)
    } else {
        "Logitech Wireless".to_string()
    };

    let name = clean_device_name(&raw_name);
    let id = format!("logi-{:04x}-{:04x}", attrs.VendorID, attrs.ProductID);

    Some(BatteryDevice {
        id,
        category: infer_category(&name),
        name,
        percent,
        connection: "Logitech 2.4G".to_string(),
        charging: false,
    })
}

unsafe fn query_hidpp_battery(handle: HANDLE, dev_idx: u8, out_len: usize, in_len: usize) -> Option<u8> {
    // Try Feature 0x1004 (Unified Battery) then 0x1000 (Battery Level Status)
    for (feat_hi, feat_lo, fn_idx) in [(0x10u8, 0x04u8, 0x10u8), (0x10u8, 0x00u8, 0x00u8)] {
        let mut req = vec![0u8; out_len];
        req[0] = 0x11; // Long HID++ report
        req[1] = dev_idx;
        req[2] = 0x00; // IRoot
        req[3] = 0x0A; // GetFeature + sw_id 0x0A
        req[4] = feat_hi;
        req[5] = feat_lo;

        let resp = send_recv_hid(handle, &req, in_len)?;
        if resp.len() < 7 || resp[2] == 0x8F || resp[4] == 0 {
            continue;
        }

        let feature_index = resp[4];
        let mut bat_req = vec![0u8; out_len];
        bat_req[0] = 0x11;
        bat_req[1] = dev_idx;
        bat_req[2] = feature_index;
        bat_req[3] = fn_idx | 0x0A;

        let bat_resp = send_recv_hid(handle, &bat_req, in_len)?;
        if bat_resp.len() >= 7 && bat_resp[2] == feature_index {
            let pct = bat_resp[4];
            if pct <= 100 {
                return Some(pct);
            }
        }
    }
    None
}

unsafe fn send_recv_hid(handle: HANDLE, out_buf: &[u8], in_len: usize) -> Option<Vec<u8>> {
    let Ok(event_w) = CreateEventW(None, true, false, None) else { return None; };
    let mut ov_w = OVERLAPPED { hEvent: event_w, ..Default::default() };
    let mut written = 0u32;
    if WriteFile(handle, Some(out_buf), Some(&mut written), Some(&mut ov_w)).is_err()
        && GetLastError() != ERROR_IO_PENDING
    {
        let _ = CloseHandle(event_w);
        return None;
    }
    if WaitForSingleObject(event_w, 80) != WAIT_OBJECT_0 {
        let _ = CancelIoEx(handle, Some(&ov_w));
        let mut tmp = 0u32;
        let _ = GetOverlappedResult(handle, &ov_w, &mut tmp, true);
        let _ = CloseHandle(event_w);
        return None;
    }
    let _ = CloseHandle(event_w);

    for _ in 0..3 {
        let Ok(event_r) = CreateEventW(None, true, false, None) else { return None; };
        let mut ov_r = OVERLAPPED { hEvent: event_r, ..Default::default() };
        let mut in_buf = vec![0u8; in_len];
        let mut read = 0u32;
        if ReadFile(handle, Some(&mut in_buf), Some(&mut read), Some(&mut ov_r)).is_err()
            && GetLastError() != ERROR_IO_PENDING
        {
            let _ = CloseHandle(event_r);
            return None;
        }
        if WaitForSingleObject(event_r, 80) != WAIT_OBJECT_0 {
            let _ = CancelIoEx(handle, Some(&ov_r));
            let mut tmp = 0u32;
            let _ = GetOverlappedResult(handle, &ov_r, &mut tmp, true);
            let _ = CloseHandle(event_r);
            return None;
        }
        let mut transferred = 0u32;
        let ok = GetOverlappedResult(handle, &ov_r, &mut transferred, false).is_ok();
        let _ = CloseHandle(event_r);
        if ok && transferred >= 7 && (in_buf[0] == 0x11 || in_buf[0] == 0x10) {
            return Some(in_buf);
        }
    }
    None
}
