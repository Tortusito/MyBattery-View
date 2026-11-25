namespace MyBatteryView.Models;

public enum DeviceCategory
{
    Headphones,
    Mouse,
    Keyboard,
    Gamepad,
    Generic
}

public sealed record BluetoothBatteryDevice(
    string Id,
    string Name,
    int BatteryPercent,
    DeviceCategory Category,
    bool IsBle
);
