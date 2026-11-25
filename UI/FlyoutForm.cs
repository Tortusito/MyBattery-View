using System.Drawing.Drawing2D;
using System.Drawing.Text;
using System.Runtime.InteropServices;
using Microsoft.Win32;
using MyBatteryView.Models;

namespace MyBatteryView.UI;

public sealed class FlyoutForm : Form
{
    private const int DWMWA_WINDOW_CORNER_PREFERENCE = 33;
    private const int DWMWCP_ROUND = 2;

    [DllImport("dwmapi.dll", PreserveSig = true)]
    private static extern int DwmSetWindowAttribute(IntPtr hwnd, int attr, ref int attrValue, int attrSize);

    private IReadOnlyList<BluetoothBatteryDevice> _devices = Array.Empty<BluetoothBatteryDevice>();
    private bool _isRefreshing;
    private Rectangle _refreshBtnRect;
    private bool _refreshHovered;

    public event EventHandler? RefreshRequested;

    public FlyoutForm()
    {
        FormBorderStyle = FormBorderStyle.None;
        ShowInTaskbar = false;
        TopMost = true;
        StartPosition = FormStartPosition.Manual;
        DoubleBuffered = true;
        Width = 320;
        Height = 140;

        Deactivate += (_, _) => Hide();
    }

    protected override CreateParams CreateParams
    {
        get
        {
            var cp = base.CreateParams;
            cp.ExStyle |= 0x00000080; // WS_EX_TOOLWINDOW (hide from Alt+Tab)
            return cp;
        }
    }

    protected override void OnHandleCreated(EventArgs e)
    {
        base.OnHandleCreated(e);
        int preference = DWMWCP_ROUND;
        _ = DwmSetWindowAttribute(Handle, DWMWA_WINDOW_CORNER_PREFERENCE, ref preference, sizeof(int));
    }

    public void UpdateDevices(IReadOnlyList<BluetoothBatteryDevice> devices, bool isRefreshing = false)
    {
        _devices = devices;
        _isRefreshing = isRefreshing;

        int rowCount = Math.Max(1, _devices.Count);
        Height = Math.Min(420, 56 + (rowCount * 54) + 12);

        if (Visible)
        {
            PositionNearTray();
            Invalidate();
        }
    }

    public void ShowNearTray()
    {
        PositionNearTray();
        Show();
        Activate();
        Invalidate();
    }

    private void PositionNearTray()
    {
        var screen = Screen.FromPoint(Cursor.Position);
        var workArea = screen.WorkingArea;
        const int margin = 10;

        int x = Math.Clamp(Cursor.Position.X - (Width / 2), workArea.Left + margin, workArea.Right - Width - margin);
        int y = workArea.Bottom - Height - margin;

        // Handle top taskbar edge
        if (workArea.Top > screen.Bounds.Top)
        {
            y = workArea.Top + margin;
        }

        Location = new Point(x, y);
    }

    protected override void OnMouseMove(MouseEventArgs e)
    {
        base.OnMouseMove(e);
        bool hovered = _refreshBtnRect.Contains(e.Location);
        if (hovered != _refreshHovered)
        {
            _refreshHovered = hovered;
            Cursor = hovered ? Cursors.Hand : Cursors.Default;
            Invalidate(_refreshBtnRect);
        }
    }

    protected override void OnMouseClick(MouseEventArgs e)
    {
        base.OnMouseClick(e);
        if (e.Button == MouseButtons.Left && _refreshBtnRect.Contains(e.Location) && !_isRefreshing)
        {
            RefreshRequested?.Invoke(this, EventArgs.Empty);
        }
    }

    protected override void OnPaint(PaintEventArgs e)
    {
        base.OnPaint(e);
        var g = e.Graphics;
        g.SmoothingMode = SmoothingMode.AntiAlias;
        g.TextRenderingHint = TextRenderingHint.ClearTypeGridFit;

        bool light = IsAppLightTheme();
        Color bg = light ? Color.FromArgb(250, 250, 252) : Color.FromArgb(28, 28, 32);
        Color cardBg = light ? Color.FromArgb(241, 242, 246) : Color.FromArgb(40, 40, 46);
        Color border = light ? Color.FromArgb(224, 224, 230) : Color.FromArgb(58, 58, 66);
        Color textPrimary = light ? Color.FromArgb(24, 24, 27) : Color.FromArgb(244, 244, 245);
        Color textMuted = light ? Color.FromArgb(113, 113, 122) : Color.FromArgb(161, 161, 170);
        Color trackBg = light ? Color.FromArgb(220, 220, 226) : Color.FromArgb(63, 63, 70);

        g.Clear(bg);
        using (var borderPen = new Pen(border, 1f))
        {
            g.DrawRectangle(borderPen, 0, 0, Width - 1, Height - 1);
        }

        // Header
        using var titleFont = new Font("Segoe UI", 10f, FontStyle.Bold);
        using var textBrush = new SolidBrush(textPrimary);
        using var mutedBrush = new SolidBrush(textMuted);
        g.DrawString("Batería Bluetooth", titleFont, textBrush, 16, 15);

        // Refresh button
        _refreshBtnRect = new Rectangle(Width - 42, 11, 28, 28);
        if (_refreshHovered)
        {
            using var hoverBrush = new SolidBrush(cardBg);
            using var hoverPath = CreateRoundedRect(_refreshBtnRect, 6);
            g.FillPath(hoverBrush, hoverPath);
        }
        DrawRefreshIcon(g, _refreshBtnRect, _isRefreshing ? textMuted : textPrimary);

        // Divider
        using (var divPen = new Pen(border, 1f))
        {
            g.DrawLine(divPen, 16, 46, Width - 16, 46);
        }

        if (_devices.Count == 0)
        {
            using var emptyFont = new Font("Segoe UI", 9f, FontStyle.Regular);
            var emptyRect = new RectangleF(16, 62, Width - 32, 48);
            var sf = new StringFormat { Alignment = StringAlignment.Center, LineAlignment = StringAlignment.Center };
            g.DrawString("No hay dispositivos Bluetooth con batería detectados.", emptyFont, mutedBrush, emptyRect, sf);
            return;
        }

        using var nameFont = new Font("Segoe UI", 9.5f, FontStyle.Regular);
        using var pctFont = new Font("Segoe UI", 9.5f, FontStyle.Bold);

        int y = 56;
        foreach (var dev in _devices)
        {
            var rowRect = new RectangleF(12, y, Width - 24, 46);
            using (var rowBrush = new SolidBrush(cardBg))
            using (var rowPath = CreateRoundedRect(rowRect, 8))
            {
                g.FillPath(rowBrush, rowPath);
            }

            // Category icon
            DrawCategoryIcon(g, dev.Category, new RectangleF(22, y + 13, 20, 20), textMuted);

            // Device name
            var nameRect = new RectangleF(50, y + 7, Width - 118, 18);
            var nameFormat = new StringFormat { Trimming = StringTrimming.EllipsisCharacter, FormatFlags = StringFormatFlags.NoWrap };
            g.DrawString(dev.Name, nameFont, textBrush, nameRect, nameFormat);

            // Percent text
            Color levelColor = dev.BatteryPercent switch
            {
                <= 20 => Color.FromArgb(239, 68, 68),
                <= 45 => Color.FromArgb(245, 158, 11),
                _ => Color.FromArgb(34, 197, 94)
            };

            using var levelBrush = new SolidBrush(levelColor);
            var pctRect = new RectangleF(Width - 68, y + 7, 46, 18);
            var pctFormat = new StringFormat { Alignment = StringAlignment.Far };
            g.DrawString($"{dev.BatteryPercent}%", pctFont, levelBrush, pctRect, pctFormat);

            // Progress bar
            var barTrack = new RectangleF(50, y + 30, Width - 74, 5);
            using (var trackBrush = new SolidBrush(trackBg))
            using (var trackPath = CreateRoundedRect(barTrack, 2.5f))
            {
                g.FillPath(trackBrush, trackPath);
            }

            float fillWidth = Math.Max(5f, barTrack.Width * (Math.Clamp(dev.BatteryPercent, 0, 100) / 100f));
            var barFill = new RectangleF(barTrack.X, barTrack.Y, fillWidth, barTrack.Height);
            using (var fillPath = CreateRoundedRect(barFill, 2.5f))
            {
                g.FillPath(levelBrush, fillPath);
            }

            y += 54;
        }
    }

    private static bool IsAppLightTheme()
    {
        try
        {
            using var key = Registry.CurrentUser.OpenSubKey(
                @"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
                writable: false);
            return key?.GetValue("AppsUseLightTheme") is int val && val == 1;
        }
        catch
        {
            return false;
        }
    }

    private static void DrawRefreshIcon(Graphics g, Rectangle rect, Color color)
    {
        using var pen = new Pen(color, 1.7f) { StartCap = LineCap.Round, EndCap = LineCap.Round };
        var arcRect = new RectangleF(rect.X + 7, rect.Y + 7, 14, 14);
        g.DrawArc(pen, arcRect, 35, 290);
        // Arrowhead
        g.DrawLine(pen, rect.X + 18, rect.Y + 7, rect.X + 21, rect.Y + 10);
        g.DrawLine(pen, rect.X + 21, rect.Y + 10, rect.X + 17, rect.Y + 11);
    }

    private static void DrawCategoryIcon(Graphics g, DeviceCategory category, RectangleF bounds, Color color)
    {
        using var pen = new Pen(color, 1.6f) { StartCap = LineCap.Round, EndCap = LineCap.Round };
        float x = bounds.X;
        float y = bounds.Y;

        switch (category)
        {
            case DeviceCategory.Headphones:
                g.DrawArc(pen, x + 3, y + 3, 14, 13, 180, 180);
                g.DrawRectangle(pen, x + 2, y + 10, 3.5f, 6);
                g.DrawRectangle(pen, x + 14.5f, y + 10, 3.5f, 6);
                break;

            case DeviceCategory.Mouse:
                using (var p = CreateRoundedRect(new RectangleF(x + 4.5f, y + 2, 11, 16), 5))
                {
                    g.DrawPath(pen, p);
                }
                g.DrawLine(pen, x + 10, y + 5, x + 10, y + 8);
                break;

            case DeviceCategory.Keyboard:
                using (var p = CreateRoundedRect(new RectangleF(x + 1.5f, y + 5, 17, 10), 2))
                {
                    g.DrawPath(pen, p);
                }
                g.DrawLine(pen, x + 4.5f, y + 8.5f, x + 15.5f, y + 8.5f);
                g.DrawLine(pen, x + 6, y + 12, x + 14, y + 12);
                break;

            case DeviceCategory.Gamepad:
                using (var p = CreateRoundedRect(new RectangleF(x + 2, y + 5, 16, 10), 4))
                {
                    g.DrawPath(pen, p);
                }
                g.DrawLine(pen, x + 5.5f, y + 10, x + 8.5f, y + 10);
                g.DrawLine(pen, x + 7, y + 8.5f, x + 7, y + 11.5f);
                g.DrawEllipse(pen, x + 12.5f, y + 9, 2, 2);
                break;

            default:
                // Bluetooth rune
                g.DrawLine(pen, x + 10, y + 2, x + 10, y + 18);
                g.DrawLine(pen, x + 10, y + 2, x + 15, y + 6);
                g.DrawLine(pen, x + 15, y + 6, x + 5.5f, y + 13.5f);
                g.DrawLine(pen, x + 5.5f, y + 6.5f, x + 15, y + 14);
                g.DrawLine(pen, x + 15, y + 14, x + 10, y + 18);
                break;
        }
    }

    private static GraphicsPath CreateRoundedRect(RectangleF rect, float radius)
    {
        var path = new GraphicsPath();
        float d = radius * 2f;
        path.AddArc(rect.X, rect.Y, d, d, 180, 90);
        path.AddArc(rect.Right - d, rect.Y, d, d, 270, 90);
        path.AddArc(rect.Right - d, rect.Bottom - d, d, d, 0, 90);
        path.AddArc(rect.X, rect.Bottom - d, d, d, 90, 90);
        path.CloseFigure();
        return path;
    }
}
