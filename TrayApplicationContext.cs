using MyBatteryView.Models;
using MyBatteryView.Services;
using MyBatteryView.UI;

namespace MyBatteryView;

public sealed class TrayApplicationContext : ApplicationContext
{
    private readonly NotifyIcon _trayIcon;
    private readonly FlyoutForm _flyout;
    private readonly System.Windows.Forms.Timer _pollTimer;
    private readonly AppSettings _settings;
    private readonly HashSet<string> _notifiedLowDevices = new(StringComparer.OrdinalIgnoreCase);

    private readonly ToolStripMenuItem _startupMenuItem;
    private readonly ToolStripMenuItem _notifyMenuItem;

    private Icon? _currentIcon;
    private bool _isRefreshing;

    public TrayApplicationContext()
    {
        _settings = SettingsService.Load();

        _flyout = new FlyoutForm();
        _flyout.RefreshRequested += async (_, _) => await RefreshBatteriesAsync();

        _startupMenuItem = new ToolStripMenuItem("Iniciar con Windows")
        {
            Checked = StartupService.IsEnabled(),
            CheckOnClick = true
        };
        _startupMenuItem.CheckedChanged += (_, _) =>
        {
            StartupService.SetEnabled(_startupMenuItem.Checked);
        };

        _notifyMenuItem = new ToolStripMenuItem($"Avisar batería baja (<{_settings.LowBatteryThreshold}%)")
        {
            Checked = _settings.NotifyLowBattery,
            CheckOnClick = true
        };
        _notifyMenuItem.CheckedChanged += (_, _) =>
        {
            _settings.NotifyLowBattery = _notifyMenuItem.Checked;
            SettingsService.Save(_settings);
        };

        var refreshItem = new ToolStripMenuItem("Actualizar ahora", null, async (_, _) => await RefreshBatteriesAsync());
        var exitItem = new ToolStripMenuItem("Salir", null, (_, _) => ExitApplication());

        var menu = new ContextMenuStrip();
        menu.Items.Add(refreshItem);
        menu.Items.Add(new ToolStripSeparator());
        menu.Items.Add(_startupMenuItem);
        menu.Items.Add(_notifyMenuItem);
        menu.Items.Add(new ToolStripSeparator());
        menu.Items.Add(exitItem);

        _currentIcon = TrayIconRenderer.CreateBatteryIcon(null);
        _trayIcon = new NotifyIcon
        {
            Icon = _currentIcon,
            Text = "MyBatteryView · Cargando...",
            ContextMenuStrip = menu,
            Visible = true
        };

        _trayIcon.MouseClick += async (_, e) =>
        {
            if (e.Button == MouseButtons.Left)
            {
                if (_flyout.Visible)
                {
                    _flyout.Hide();
                }
                else
                {
                    _flyout.ShowNearTray();
                    await RefreshBatteriesAsync();
                }
            }
        };

        _pollTimer = new System.Windows.Forms.Timer
        {
            Interval = Math.Max(10, _settings.PollIntervalSeconds) * 1000
        };
        _pollTimer.Tick += async (_, _) => await RefreshBatteriesAsync();
        _pollTimer.Start();

        _ = RefreshBatteriesAsync();
    }

    private async Task RefreshBatteriesAsync()
    {
        if (_isRefreshing) return;
        _isRefreshing = true;

        try
        {
            IReadOnlyList<BluetoothBatteryDevice> devices = await BluetoothBatteryService.GetConnectedDevicesAsync();

            int? lowest = devices.Count > 0 ? devices.Min(d => d.BatteryPercent) : null;
            UpdateTrayIcon(lowest);
            UpdateTooltip(devices);
            _flyout.UpdateDevices(devices, isRefreshing: false);
            CheckLowBatteryAlerts(devices);
        }
        catch
        {
            // Keep previous state on transient Bluetooth error
        }
        finally
        {
            _isRefreshing = false;
        }
    }

    private void UpdateTrayIcon(int? lowestPercent)
    {
        var nextIcon = TrayIconRenderer.CreateBatteryIcon(lowestPercent);
        var oldIcon = _currentIcon;
        _currentIcon = nextIcon;
        _trayIcon.Icon = nextIcon;
        oldIcon?.Dispose();
    }

    private void UpdateTooltip(IReadOnlyList<BluetoothBatteryDevice> devices)
    {
        if (devices.Count == 0)
        {
            _trayIcon.Text = "MyBatteryView · Sin dispositivos";
            return;
        }

        var summary = string.Join(" · ", devices.Select(d => $"{d.Name}: {d.BatteryPercent}%"));
        if (summary.Length > 120)
        {
            summary = summary[..117] + "...";
        }
        _trayIcon.Text = summary;
    }

    private void CheckLowBatteryAlerts(IReadOnlyList<BluetoothBatteryDevice> devices)
    {
        if (!_settings.NotifyLowBattery) return;

        foreach (var dev in devices)
        {
            if (dev.BatteryPercent <= _settings.LowBatteryThreshold)
            {
                if (_notifiedLowDevices.Add(dev.Id))
                {
                    _trayIcon.ShowBalloonTip(
                        4000,
                        "Batería baja",
                        $"{dev.Name} está al {dev.BatteryPercent}%",
                        ToolTipIcon.Warning);
                }
            }
            else if (dev.BatteryPercent > _settings.LowBatteryThreshold + 5)
            {
                _notifiedLowDevices.Remove(dev.Id);
            }
        }
    }

    private void ExitApplication()
    {
        _pollTimer.Stop();
        _pollTimer.Dispose();
        _trayIcon.Visible = false;
        _trayIcon.Dispose();
        _currentIcon?.Dispose();
        _flyout.Dispose();
        ExitThread();
    }
}
