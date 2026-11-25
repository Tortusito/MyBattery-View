using System.Runtime.InteropServices;
using System.Text;
using Microsoft.Win32.SafeHandles;
using MyBatteryView.Models;

namespace MyBatteryView.Services;

/// <summary>
/// Reads battery levels from standard USB/2.4GHz Wireless HID devices
/// supporting HID Power/Battery System (0x85) or Generic Device Battery Strength (0x06/0x20).
/// </summary>
public static class HidBatteryService
{
    private const ushort UsagePageGenericDevice = 0x06;
    private const ushort UsageBatteryStrength = 0x20;
    private const ushort UsagePageBatterySystem = 0x85;
    private const ushort UsageAbsoluteStateOfCharge = 0x65;
    private const ushort UsageRemainingCapacity = 0x66;

    public static List<BluetoothBatteryDevice> ReadHidBatteries(Func<string, DeviceCategory> inferCategory)
    {
        var list = new List<BluetoothBatteryDevice>();
        var seenPaths = new HashSet<string>(StringComparer.OrdinalIgnoreCase);

        HidD_GetHidGuid(out Guid hidGuid);
        IntPtr devInfoSet = SetupDiGetClassDevsW(ref hidGuid, IntPtr.Zero, IntPtr.Zero, DIGCF_PRESENT | DIGCF_DEVICEINTERFACE);
        if (devInfoSet == IntPtr.Zero || devInfoSet == new IntPtr(-1))
        {
            return list;
        }

        try
        {
            var ifaceData = new SP_DEVICE_INTERFACE_DATA
            {
                cbSize = (uint)Marshal.SizeOf<SP_DEVICE_INTERFACE_DATA>()
            };

            uint index = 0;
            while (SetupDiEnumDeviceInterfaces(devInfoSet, IntPtr.Zero, ref hidGuid, index++, ref ifaceData))
            {
                string? devicePath = GetDeviceInterfacePath(devInfoSet, ref ifaceData);
                if (string.IsNullOrWhiteSpace(devicePath) || !seenPaths.Add(devicePath))
                {
                    continue;
                }

                var dev = TryReadDeviceBattery(devicePath, inferCategory);
                if (dev is not null)
                {
                    list.Add(dev);
                }
            }
        }
        finally
        {
            SetupDiDestroyDeviceInfoList(devInfoSet);
        }

        return list;
    }

    private static BluetoothBatteryDevice? TryReadDeviceBattery(string devicePath, Func<string, DeviceCategory> inferCategory)
    {
        // Open with dwDesiredAccess = 0 (metadata/feature query) so Windows doesn't block mice/keyboards
        using SafeFileHandle handle = CreateFileW(
            devicePath,
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            IntPtr.Zero,
            OPEN_EXISTING,
            0,
            IntPtr.Zero);

        if (handle.IsInvalid) return null;

        if (!HidD_GetPreparsedData(handle, out IntPtr preparsedData) || preparsedData == IntPtr.Zero)
        {
            return null;
        }

        try
        {
            if (HidP_GetCaps(preparsedData, out HIDP_CAPS caps) != HIDP_STATUS_SUCCESS)
            {
                return null;
            }

            // Check Feature reports first, then Input reports
            int? percent = TryExtractBatteryFromReportType(handle, preparsedData, HidP_Feature, caps.NumberFeatureValueCaps, caps.FeatureReportByteLength)
                        ?? TryExtractBatteryFromReportType(handle, preparsedData, HidP_Input, caps.NumberInputValueCaps, caps.InputReportByteLength);

            if (percent is null or < 0 or > 100)
            {
                return null;
            }

            string name = GetProductName(handle) ?? "Periférico Wireless";
            return new BluetoothBatteryDevice(
                Id: devicePath,
                Name: name,
                BatteryPercent: percent.Value,
                Category: inferCategory(name),
                IsBle: false,
                ConnectionLabel: "Wireless"
            );
        }
        finally
        {
            HidD_FreePreparsedData(preparsedData);
        }
    }

    private static int? TryExtractBatteryFromReportType(
        SafeFileHandle handle,
        IntPtr preparsedData,
        int reportType,
        ushort valueCapsCount,
        ushort reportByteLength)
    {
        if (valueCapsCount == 0 || reportByteLength == 0) return null;

        var valueCaps = new HIDP_VALUE_CAPS[valueCapsCount];
        ushort length = valueCapsCount;
        if (HidP_GetValueCaps(reportType, valueCaps, ref length, preparsedData) != HIDP_STATUS_SUCCESS)
        {
            return null;
        }

        for (int i = 0; i < length; i++)
        {
            var cap = valueCaps[i];
            if (cap.IsRange != 0) continue;

            bool isBatteryUsage =
                (cap.UsagePage == UsagePageBatterySystem && (cap.UsageMin == UsageAbsoluteStateOfCharge || cap.UsageMin == UsageRemainingCapacity)) ||
                (cap.UsagePage == UsagePageGenericDevice && cap.UsageMin == UsageBatteryStrength);

            if (!isBatteryUsage) continue;

            var report = new byte[reportByteLength];
            report[0] = cap.ReportID;

            bool gotReport = reportType == HidP_Feature
                ? HidD_GetFeature(handle, report, (uint)report.Length)
                : HidD_GetInputReport(handle, report, (uint)report.Length);

            if (!gotReport) continue;

            if (HidP_GetUsageValue(
                    reportType,
                    cap.UsagePage,
                    cap.LinkCollection,
                    cap.UsageMin,
                    out uint rawValue,
                    preparsedData,
                    report,
                    (uint)report.Length) == HIDP_STATUS_SUCCESS)
            {
                int max = cap.LogicalMax > cap.LogicalMin ? cap.LogicalMax : 100;
                int min = cap.LogicalMin;
                if (max <= min) continue;

                int pct = (int)Math.Round(((rawValue - min) / (double)(max - min)) * 100.0);
                return Math.Clamp(pct, 0, 100);
            }
        }

        return null;
    }

    private static string? GetProductName(SafeFileHandle handle)
    {
        var buffer = new byte[256];
        if (!HidD_GetProductString(handle, buffer, (uint)buffer.Length))
        {
            return null;
        }

        string name = Encoding.Unicode.GetString(buffer).TrimEnd('\0').Trim();
        return string.IsNullOrWhiteSpace(name) ? null : name;
    }

    private static string? GetDeviceInterfacePath(IntPtr devInfoSet, ref SP_DEVICE_INTERFACE_DATA ifaceData)
    {
        SetupDiGetDeviceInterfaceDetailW(devInfoSet, ref ifaceData, IntPtr.Zero, 0, out uint requiredSize, IntPtr.Zero);
        if (requiredSize <= 4) return null;

        IntPtr detailBuffer = Marshal.AllocHGlobal((int)requiredSize);
        try
        {
            // cbSize is 8 on 64-bit, 6 on 32-bit for SP_DEVICE_INTERFACE_DETAIL_DATA_W
            Marshal.WriteInt32(detailBuffer, IntPtr.Size == 8 ? 8 : 6);
            if (!SetupDiGetDeviceInterfaceDetailW(devInfoSet, ref ifaceData, detailBuffer, requiredSize, out _, IntPtr.Zero))
            {
                return null;
            }

            return Marshal.PtrToStringUni(IntPtr.Add(detailBuffer, 4));
        }
        finally
        {
            Marshal.FreeHGlobal(detailBuffer);
        }
    }

    #region P/Invoke

    private const uint DIGCF_PRESENT = 0x00000002;
    private const uint DIGCF_DEVICEINTERFACE = 0x00000010;
    private const uint FILE_SHARE_READ = 0x00000001;
    private const uint FILE_SHARE_WRITE = 0x00000002;
    private const uint OPEN_EXISTING = 3;
    private const int HIDP_STATUS_SUCCESS = 0x00110000;
    private const int HidP_Input = 0;
    private const int HidP_Feature = 2;

    [StructLayout(LayoutKind.Sequential)]
    private struct SP_DEVICE_INTERFACE_DATA
    {
        public uint cbSize;
        public Guid InterfaceClassGuid;
        public uint Flags;
        public IntPtr Reserved;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct HIDP_CAPS
    {
        public ushort Usage;
        public ushort UsagePage;
        public ushort InputReportByteLength;
        public ushort OutputReportByteLength;
        public ushort FeatureReportByteLength;
        [MarshalAs(UnmanagedType.ByValArray, SizeConst = 17)]
        public ushort[] Reserved;
        public ushort NumberLinkCollectionNodes;
        public ushort NumberInputButtonCaps;
        public ushort NumberInputValueCaps;
        public ushort NumberInputDataIndices;
        public ushort NumberOutputButtonCaps;
        public ushort NumberOutputValueCaps;
        public ushort NumberOutputDataIndices;
        public ushort NumberFeatureButtonCaps;
        public ushort NumberFeatureValueCaps;
        public ushort NumberFeatureDataIndices;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct HIDP_VALUE_CAPS
    {
        public ushort UsagePage;
        public byte ReportID;
        public byte IsAlias;
        public ushort BitField;
        public ushort LinkCollection;
        public ushort LinkUsage;
        public ushort LinkUsagePage;
        public byte IsRange;
        public byte IsStringRange;
        public byte IsDesignatorRange;
        public byte IsAbsolute;
        public byte HasNull;
        public byte Reserved;
        public ushort BitSize;
        public ushort ReportCount;
        [MarshalAs(UnmanagedType.ByValArray, SizeConst = 5)]
        public ushort[] Reserved2;
        public uint UnitsExp;
        public uint Units;
        public int LogicalMin;
        public int LogicalMax;
        public int PhysicalMin;
        public int PhysicalMax;
        public ushort UsageMin;
        public ushort UsageMax;
        public ushort StringMin;
        public ushort StringMax;
        public ushort DesignatorMin;
        public ushort DesignatorMax;
        public ushort DataIndexMin;
        public ushort DataIndexMax;
    }

    [DllImport("hid.dll")]
    private static extern void HidD_GetHidGuid(out Guid HidGuid);

    [DllImport("hid.dll", SetLastError = true)]
    private static extern bool HidD_GetPreparsedData(SafeFileHandle HidDeviceObject, out IntPtr PreparsedData);

    [DllImport("hid.dll")]
    private static extern bool HidD_FreePreparsedData(IntPtr PreparsedData);

    [DllImport("hid.dll", SetLastError = true)]
    private static extern bool HidD_GetProductString(SafeFileHandle HidDeviceObject, byte[] Buffer, uint BufferLength);

    [DllImport("hid.dll", SetLastError = true)]
    private static extern bool HidD_GetFeature(SafeFileHandle HidDeviceObject, byte[] ReportBuffer, uint ReportBufferLength);

    [DllImport("hid.dll", SetLastError = true)]
    private static extern bool HidD_GetInputReport(SafeFileHandle HidDeviceObject, byte[] ReportBuffer, uint ReportBufferLength);

    [DllImport("hid.dll")]
    private static extern int HidP_GetCaps(IntPtr PreparsedData, out HIDP_CAPS Capabilities);

    [DllImport("hid.dll")]
    private static extern int HidP_GetValueCaps(int ReportType, [Out] HIDP_VALUE_CAPS[] ValueCaps, ref ushort ValueCapsLength, IntPtr PreparsedData);

    [DllImport("hid.dll")]
    private static extern int HidP_GetUsageValue(int ReportType, ushort UsagePage, ushort LinkCollection, ushort Usage, out uint UsageValue, IntPtr PreparsedData, byte[] Report, uint ReportLength);

    [DllImport("setupapi.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern IntPtr SetupDiGetClassDevsW(ref Guid ClassGuid, IntPtr Enumerator, IntPtr hwndParent, uint Flags);

    [DllImport("setupapi.dll", SetLastError = true)]
    private static extern bool SetupDiEnumDeviceInterfaces(IntPtr DeviceInfoSet, IntPtr DeviceInfoData, ref Guid InterfaceClassGuid, uint MemberIndex, ref SP_DEVICE_INTERFACE_DATA DeviceInterfaceData);

    [DllImport("setupapi.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern bool SetupDiGetDeviceInterfaceDetailW(IntPtr DeviceInfoSet, ref SP_DEVICE_INTERFACE_DATA DeviceInterfaceData, IntPtr DeviceInterfaceDetailData, uint DeviceInterfaceDetailDataSize, out uint RequiredSize, IntPtr DeviceInfoData);

    [DllImport("setupapi.dll", SetLastError = true)]
    private static extern bool SetupDiDestroyDeviceInfoList(IntPtr DeviceInfoSet);

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern SafeFileHandle CreateFileW(string lpFileName, uint dwDesiredAccess, uint dwShareMode, IntPtr lpSecurityAttributes, uint dwCreationDisposition, uint dwFlagsAndAttributes, IntPtr hTemplateFile);

    #endregion
}
