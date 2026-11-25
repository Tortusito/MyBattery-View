use super::{clean_device_name, infer_category, BatteryDevice};
use windows::Devices::Bluetooth::GenericAttributeProfile::{
    GattCharacteristicUuids, GattCommunicationStatus, GattDeviceService, GattServiceUuids,
};
use windows::Devices::Bluetooth::{BluetoothCacheMode, BluetoothConnectionStatus, BluetoothLEDevice};
use windows::Devices::Enumeration::DeviceInformation;
use windows::Storage::Streams::DataReader;

pub fn read_ble_batteries() -> Vec<BatteryDevice> {
    read_ble_inner().unwrap_or_default()
}

fn read_ble_inner() -> windows::core::Result<Vec<BatteryDevice>> {
    let mut list = Vec::new();
    let battery_uuid = GattServiceUuids::Battery()?;
    let selector = GattDeviceService::GetDeviceSelectorFromUuid(battery_uuid)?;
    let infos = DeviceInformation::FindAllAsyncAqsFilter(&selector)?.get()?;
    let level_uuid = GattCharacteristicUuids::BatteryLevel()?;

    let count = infos.Size()?;
    for i in 0..count {
        let Ok(info) = infos.GetAt(i) else { continue };
        let Ok(info_id) = info.Id() else { continue };

        let Ok(service_op) = GattDeviceService::FromIdAsync(&info_id) else { continue };
        let Ok(service) = service_op.get() else { continue };

        let mut dev_name = info.Name().map(|s| s.to_string_lossy()).unwrap_or_default();
        let mut dev_id = info_id.to_string_lossy();

        if let Ok(session) = service.Session() {
            if let Ok(bt_id) = session.DeviceId() {
                if let Ok(id_hstr) = bt_id.Id() {
                    if let Ok(ble_op) = BluetoothLEDevice::FromIdAsync(&id_hstr) {
                        if let Ok(ble_dev) = ble_op.get() {
                            if let Ok(status) = ble_dev.ConnectionStatus() {
                                if status != BluetoothConnectionStatus::Connected {
                                    continue;
                                }
                            }
                            if let Ok(n) = ble_dev.Name() {
                                let ns = n.to_string_lossy();
                                if !ns.trim().is_empty() {
                                    dev_name = ns;
                                }
                            }
                            if let Ok(id) = ble_dev.DeviceId() {
                                dev_id = id.to_string_lossy();
                            }
                        }
                    }
                }
            }
        }

        let Ok(chars_op) = service.GetCharacteristicsForUuidWithCacheModeAsync(level_uuid, BluetoothCacheMode::Uncached) else {
            continue;
        };
        let Ok(chars_res) = chars_op.get() else { continue };
        if chars_res.Status()? != GattCommunicationStatus::Success {
            continue;
        }
        let Ok(chars) = chars_res.Characteristics() else { continue };
        if chars.Size()? == 0 {
            continue;
        }
        let Ok(ch) = chars.GetAt(0) else { continue };
        let Ok(read_op) = ch.ReadValueWithCacheModeAsync(BluetoothCacheMode::Uncached) else { continue };
        let Ok(read_res) = read_op.get() else { continue };
        if read_res.Status()? != GattCommunicationStatus::Success {
            continue;
        }
        let Ok(buf) = read_res.Value() else { continue };
        if buf.Length()? == 0 {
            continue;
        }

        let reader = DataReader::FromBuffer(&buf)?;
        let percent = reader.ReadByte()?;
        if percent > 100 {
            continue;
        }

        let name = clean_device_name(if dev_name.trim().is_empty() { "Dispositivo BLE" } else { &dev_name });
        list.push(BatteryDevice {
            id: dev_id,
            category: infer_category(&name),
            name,
            percent,
            connection: "BLE".to_string(),
            charging: false,
        });
    }

    Ok(list)
}
