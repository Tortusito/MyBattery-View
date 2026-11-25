using System.Runtime.InteropServices;
using System.Text;
using MyBatteryView.Models;
using Windows.Devices.Bluetooth;
using Windows.Devices.Bluetooth.GenericAttributeProfile;
using Windows.Devices.Enumeration;
using Windows.Storage.Streams;

namespace MyBatteryView.Services;

public static class BluetoothBatteryService
{
    public static async Task<IReadOnlyList<BluetoothBatteryDevice>> GetConnectedDevicesAsync(CancellationToken ct = default)
    {
        var results = new Dictionary<string, BluetoothBatteryDevice>(StringComparer.OrdinalIgnoreCase);

        // 1. Classic Bluetooth & PnP battery properties (Windows native)
        foreach (var dev in ReadPnPBluetoothBatteries())
        {
            results[NormalizeKey(dev.Name)] = dev;
        }

        // 2. BLE Battery Service (0x180F / 0x2A19)
        try
        {
            var bleDevices = await ReadBleBatteriesAsync(ct).ConfigureAwait(false);
            foreach (var dev in bleDevices)
            {
                var key = NormalizeKey(dev.Name);
                if (!results.ContainsKey(key))
                {
                    results[key] = dev;
                }
            }
        }
        catch
        {
            // BLE stack may be unavailable if Bluetooth radio is off
        }

        // 3. Standard Wireless / USB HID Battery (2.4 GHz dongles & HID devices)
        try
        {
            foreach (var dev in HidBatteryService.ReadHidBatteries(InferCategory))
            {
                var key = NormalizeKey(dev.Name);
                if (!results.ContainsKey(key))
                {
                    results[key] = dev;
                }
            }
        }
        catch
        {
            // Ignore HID enumeration errors
        }

        // 4. Xbox Wireless / Bluetooth Controllers (XInput)
        foreach (var dev in XInputBatteryService.ReadXboxControllers())
        {
            // Avoid duplicate entry if already reported by Bluetooth PnP as an Xbox controller
            bool alreadyHasXbox = results.Values.Any(d =>
                d.Category == DeviceCategory.Gamepad &&
                d.Name.Contains("xbox", StringComparison.OrdinalIgnoreCase));

            if (!alreadyHasXbox)
            {
                results[dev.Id] = dev;
            }
        }

        return results.Values
            .OrderBy(d => d.BatteryPercent)
            .ThenBy(d => d.Name, StringComparer.OrdinalIgnoreCase)
            .ToList();
    }

    private static string NormalizeKey(string name) => name.Trim().ToLowerInvariant();

    private static List<BluetoothBatteryDevice> ReadPnPBluetoothBatteries()
    {
        var list = new List<BluetoothBatteryDevice>();
        var seenContainers = new HashSet<Guid>();

        IntPtr devInfoSet = SetupDiGetClassDevsW(IntPtr.Zero, IntPtr.Zero, IntPtr.Zero, DIGCF_ALLCLASSES | DIGCF_PRESENT);
        if (devInfoSet == IntPtr.Zero || devInfoSet == new IntPtr(-1))
        {
            return list;
        }

        try
        {
            var devInfoData = new SP_DEVINFO_DATA
            {
                cbSize = (uint)Marshal.SizeOf<SP_DEVINFO_DATA>()
            };

            uint index = 0;
            while (SetupDiEnumDeviceInfo(devInfoSet, index++, ref devInfoData))
            {
                if (!TryGetByteProperty(devInfoSet, ref devInfoData, DeVpKeyBluetoothBattery, out byte battery))
                {
                    continue;
                }

                if (battery > 100) continue;

                // Skip disconnected cached PnP nodes if IsConnected is explicitly false
                if (TryGetBoolProperty(devInfoSet, ref devInfoData, DeVpKeyDeviceIsConnected, out bool isConnected) && !isConnected)
                {
                    continue;
                }

                if (TryGetGuidProperty(devInfoSet, ref devInfoData, DeVpKeyDeviceContainerId, out Guid containerId) &&
                    containerId != Guid.Empty &&
                    !seenContainers.Add(containerId))
                {
                    continue;
                }

                string? name = GetStringProperty(devInfoSet, ref devInfoData, DeVpKeyDeviceFriendlyName)
                            ?? GetStringProperty(devInfoSet, ref devInfoData, DeVpKeyName);

                if (string.IsNullOrWhiteSpace(name))
                {
                    name = "Dispositivo Bluetooth";
                }

                // Clean typical Windows hands-free / stereo suffixes if present
                name = CleanDeviceName(name);

                list.Add(new BluetoothBatteryDevice(
                    Id: containerId != Guid.Empty ? containerId.ToString() : name,
                    Name: name,
                    BatteryPercent: battery,
                    Category: InferCategory(name),
                    IsBle: false
                ));
            }
        }
        finally
        {
            SetupDiDestroyDeviceInfoList(devInfoSet);
        }

        return list;
    }

    private static async Task<List<BluetoothBatteryDevice>> ReadBleBatteriesAsync(CancellationToken ct)
    {
        var list = new List<BluetoothBatteryDevice>();
        string selector = GattDeviceService.GetDeviceSelectorFromUuid(GattServiceUuids.Battery);
        DeviceInformationCollection serviceInfos = await DeviceInformation.FindAllAsync(selector).AsTask(ct).ConfigureAwait(false);

        foreach (DeviceInformation info in serviceInfos)
        {
            ct.ThrowIfCancellationRequested();

            try
            {
                using GattDeviceService? service = await GattDeviceService.FromIdAsync(info.Id).AsTask(ct).ConfigureAwait(false);
                if (service is null) continue;

                BluetoothLEDevice? bleDevice = service.Session?.DeviceId is not null
                    ? await BluetoothLEDevice.FromIdAsync(service.Session.DeviceId.Id).AsTask(ct).ConfigureAwait(false)
                    : null;

                using (bleDevice)
                {
                    if (bleDevice is not null && bleDevice.ConnectionStatus != BluetoothConnectionStatus.Connected)
                    {
                        continue;
                    }

                    GattCharacteristicsResult charResult = await service
                        .GetCharacteristicsForUuidAsync(GattCharacteristicUuids.BatteryLevel, BluetoothCacheMode.Uncached)
                        .AsTask(ct)
                        .ConfigureAwait(false);

                    if (charResult.Status != GattCommunicationStatus.Success || charResult.Characteristics.Count == 0)
                    {
                        continue;
                    }

                    GattReadResult readResult = await charResult.Characteristics[0]
                        .ReadValueAsync(BluetoothCacheMode.Uncached)
                        .AsTask(ct)
                        .ConfigureAwait(false);

                    if (readResult.Status != GattCommunicationStatus.Success || readResult.Value is null || readResult.Value.Length == 0)
                    {
                        continue;
                    }

                    using DataReader reader = DataReader.FromBuffer(readResult.Value);
                    byte percent = reader.ReadByte();
                    if (percent > 100) continue;

                    string name = !string.IsNullOrWhiteSpace(bleDevice?.Name)
                        ? bleDevice.Name
                        : info.Name;

                    name = CleanDeviceName(string.IsNullOrWhiteSpace(name) ? "Dispositivo BLE" : name);

                    list.Add(new BluetoothBatteryDevice(
                        Id: bleDevice?.DeviceId ?? info.Id,
                        Name: name,
                        BatteryPercent: percent,
                        Category: InferCategory(name),
                        IsBle: true,
                        ConnectionLabel: "BLE"
                    ));
                }
            }
            catch
            {
                // Device may have disconnected mid-query
            }
        }

        return list;
    }

    private static string CleanDeviceName(string name)
    {
        return name
            .Replace(" Hands-Free AG Audio", "", StringComparison.OrdinalIgnoreCase)
            .Replace(" Hands-Free", "", StringComparison.OrdinalIgnoreCase)
            .Replace(" Stereo", "", StringComparison.OrdinalIgnoreCase)
            .Trim();
    }

    private static DeviceCategory InferCategory(string name)
    {
        var lower = name.ToLowerInvariant();

        if (lower.Contains("airpod") || lower.Contains("bud") || lower.Contains("head") ||
            lower.Contains("pod") || lower.Contains("wh-") || lower.Contains("wf-") ||
            lower.Contains("bose") || lower.Contains("jabra") || lower.Contains("beats") ||
            lower.Contains("audio") || lower.Contains("ear"))
        {
            return DeviceCategory.Headphones;
        }

        if (lower.Contains("mouse") || lower.Contains("ratón") || lower.Contains("raton") ||
            lower.Contains("mx master") || lower.Contains("mx anywhere") || lower.Contains("viper") ||
            lower.Contains("deathadder") || lower.Contains("g pro"))
        {
            return DeviceCategory.Mouse;
        }

        if (lower.Contains("keyboard") || lower.Contains("teclado") || lower.Contains("keychron") ||
            lower.Contains("mx keys") || lower.Contains("k380") || lower.Contains("anne pro"))
        {
            return DeviceCategory.Keyboard;
        }

        if (lower.Contains("xbox") || lower.Contains("dualsense") || lower.Contains("dualshock") ||
            lower.Contains("controller") || lower.Contains("gamepad") || lower.Contains("mando") ||
            lower.Contains("8bitdo") || lower.Contains("joy-con"))
        {
            return DeviceCategory.Gamepad;
        }

        return DeviceCategory.Generic;
    }

    #region Win32 SetupAPI P/Invoke

    private const uint DIGCF_PRESENT = 0x00000002;
    private const uint DIGCF_ALLCLASSES = 0x00000004;

    private static readonly DEVPROPKEY DeVpKeyBluetoothBattery = new()
    {
        fmtid = new Guid("104EA319-6EE2-4701-BD47-8DDBF425BBE5"),
        pid = 2
    };

    private static readonly DEVPROPKEY DeVpKeyDeviceIsConnected = new()
    {
        fmtid = new Guid("83DA6326-97A6-4088-9453-A1923F573B29"),
        pid = 15
    };

    private static readonly DEVPROPKEY DeVpKeyDeviceFriendlyName = new()
    {
        fmtid = new Guid("A45C254E-DF1C-4EFD-8020-67D146A850E0"),
        pid = 14
    };

    private static readonly DEVPROPKEY DeVpKeyName = new()
    {
        fmtid = new Guid("B725F130-47EF-101A-A5F1-02608C9EEBAC"),
        pid = 10
    };

    private static readonly DEVPROPKEY DeVpKeyDeviceContainerId = new()
    {
        fmtid = new Guid("8C7ED206-3F8A-4827-B3AB-AE9E1FAEFC6C"),
        pid = 2
    };

    [StructLayout(LayoutKind.Sequential)]
    private struct SP_DEVINFO_DATA
    {
        public uint cbSize;
        public Guid ClassGuid;
        public uint DevInst;
        public IntPtr Reserved;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct DEVPROPKEY
    {
        public Guid fmtid;
        public uint pid;
    }

    [DllImport("setupapi.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern IntPtr SetupDiGetClassDevsW(
        IntPtr ClassGuid,
        IntPtr Enumerator,
        IntPtr hwndParent,
        uint Flags);

    [DllImport("setupapi.dll", SetLastError = true)]
    private static extern bool SetupDiEnumDeviceInfo(
        IntPtr DeviceInfoSet,
        uint MemberIndex,
        ref SP_DEVINFO_DATA DeviceInfoData);

    [DllImport("setupapi.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern bool SetupDiGetDevicePropertyW(
        IntPtr DeviceInfoSet,
        ref SP_DEVINFO_DATA DeviceInfoData,
        ref DEVPROPKEY PropertyKey,
        out uint PropertyType,
        byte[]? PropertyBuffer,
        uint PropertyBufferSize,
        out uint RequiredSize,
        uint Flags);

    [DllImport("setupapi.dll", SetLastError = true)]
    private static extern bool SetupDiDestroyDeviceInfoList(IntPtr DeviceInfoSet);

    private static bool TryGetByteProperty(IntPtr devInfoSet, ref SP_DEVINFO_DATA devInfoData, DEVPROPKEY key, out byte value)
    {
        value = 0;
        var buffer = new byte[1];
        if (SetupDiGetDevicePropertyW(devInfoSet, ref devInfoData, ref key, out _, buffer, 1, out _, 0))
        {
            value = buffer[0];
            return true;
        }
        return false;
    }

    private static bool TryGetBoolProperty(IntPtr devInfoSet, ref SP_DEVINFO_DATA devInfoData, DEVPROPKEY key, out bool value)
    {
        value = false;
        var buffer = new byte[1];
        if (SetupDiGetDevicePropertyW(devInfoSet, ref devInfoData, ref key, out _, buffer, 1, out _, 0))
        {
            value = buffer[0] != 0;
            return true;
        }
        return false;
    }

    private static bool TryGetGuidProperty(IntPtr devInfoSet, ref SP_DEVINFO_DATA devInfoData, DEVPROPKEY key, out Guid value)
    {
        value = Guid.Empty;
        var buffer = new byte[16];
        if (SetupDiGetDevicePropertyW(devInfoSet, ref devInfoData, ref key, out _, buffer, 16, out _, 0))
        {
            value = new Guid(buffer);
            return true;
        }
        return false;
    }

    private static string? GetStringProperty(IntPtr devInfoSet, ref SP_DEVINFO_DATA devInfoData, DEVPROPKEY key)
    {
        SetupDiGetDevicePropertyW(devInfoSet, ref devInfoData, ref key, out _, null, 0, out uint requiredSize, 0);
        if (requiredSize <= 2) return null;

        var buffer = new byte[requiredSize];
        if (!SetupDiGetDevicePropertyW(devInfoSet, ref devInfoData, ref key, out _, buffer, requiredSize, out _, 0))
        {
            return null;
        }

        return Encoding.Unicode.GetString(buffer).TrimEnd('\0');
    }

    #endregion
}
