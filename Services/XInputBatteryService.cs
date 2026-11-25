using System.Runtime.InteropServices;
using MyBatteryView.Models;

namespace MyBatteryView.Services;

/// <summary>
/// Reads battery status for Xbox Wireless / Bluetooth controllers via XInput.
/// </summary>
public static class XInputBatteryService
{
    private const byte BatteryDevTypeGamepad = 0x00;
    private const byte BatteryTypeDisconnected = 0x00;
    private const byte BatteryTypeWired = 0x01;

    public static List<BluetoothBatteryDevice> ReadXboxControllers()
    {
        var list = new List<BluetoothBatteryDevice>();

        try
        {
            for (uint userIndex = 0; userIndex < 4; userIndex++)
            {
                if (XInputGetBatteryInformation(userIndex, BatteryDevTypeGamepad, out XINPUT_BATTERY_INFORMATION info) != 0)
                {
                    continue;
                }

                if (info.BatteryType is BatteryTypeDisconnected or BatteryTypeWired)
                {
                    continue;
                }

                int percent = info.BatteryLevel switch
                {
                    0 => 10,  // BATTERY_LEVEL_EMPTY
                    1 => 35,  // BATTERY_LEVEL_LOW
                    2 => 70,  // BATTERY_LEVEL_MEDIUM
                    _ => 100  // BATTERY_LEVEL_FULL
                };

                list.Add(new BluetoothBatteryDevice(
                    Id: $"xinput-{userIndex}",
                    Name: $"Mando Xbox #{userIndex + 1}",
                    BatteryPercent: percent,
                    Category: DeviceCategory.Gamepad,
                    IsBle: false,
                    ConnectionLabel: "Xbox"
                ));
            }
        }
        catch
        {
            // xinput1_4.dll may not be present on minimal Windows builds
        }

        return list;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct XINPUT_BATTERY_INFORMATION
    {
        public byte BatteryType;
        public byte BatteryLevel;
    }

    [DllImport("xinput1_4.dll")]
    private static extern uint XInputGetBatteryInformation(
        uint dwUserIndex,
        byte devType,
        out XINPUT_BATTERY_INFORMATION pBatteryInformation);
}
