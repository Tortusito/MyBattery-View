using System.Drawing.Drawing2D;
using System.Runtime.InteropServices;
using Microsoft.Win32;

namespace MyBatteryView.UI;

public static class TrayIconRenderer
{
    [DllImport("user32.dll", SetLastError = true)]
    private static extern bool DestroyIcon(IntPtr hIcon);

    public static bool IsSystemLightTheme()
    {
        try
        {
            using var key = Registry.CurrentUser.OpenSubKey(
                @"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
                writable: false);
            return key?.GetValue("SystemUsesLightTheme") is int val && val == 1;
        }
        catch
        {
            return false;
        }
    }

    public static Icon CreateBatteryIcon(int? lowestPercent)
    {
        const int size = 32;
        using var bmp = new Bitmap(size, size);
        using var g = Graphics.FromImage(bmp);

        g.SmoothingMode = SmoothingMode.AntiAlias;
        g.Clear(Color.Transparent);

        bool lightTaskbar = IsSystemLightTheme();
        Color strokeColor = lightTaskbar ? Color.FromArgb(24, 24, 27) : Color.FromArgb(244, 244, 245);
        Color fillColor = lowestPercent switch
        {
            null => Color.FromArgb(140, 140, 150),
            <= 20 => Color.FromArgb(239, 68, 68),
            <= 45 => Color.FromArgb(245, 158, 11),
            _ => Color.FromArgb(34, 197, 94)
        };

        // Battery body outline
        var bodyRect = new RectangleF(4f, 8f, 21f, 16f);
        using (var pen = new Pen(strokeColor, 2.2f))
        using (var path = CreateRoundedRect(bodyRect, 3.2f))
        {
            g.DrawPath(pen, path);
        }

        // Battery positive terminal cap
        var capRect = new RectangleF(26.5f, 12.5f, 2.5f, 7f);
        using (var capBrush = new SolidBrush(strokeColor))
        using (var capPath = CreateRoundedRect(capRect, 1f))
        {
            g.FillPath(capBrush, capPath);
        }

        // Inner fill level
        if (lowestPercent is int pct)
        {
            float clamped = Math.Clamp(pct, 5, 100) / 100f;
            float maxInnerWidth = 15.6f;
            float fillWidth = Math.Max(2.5f, maxInnerWidth * clamped);
            var fillRect = new RectangleF(6.7f, 10.7f, fillWidth, 10.6f);

            using var fillBrush = new SolidBrush(fillColor);
            using var fillPath = CreateRoundedRect(fillRect, 1.5f);
            g.FillPath(fillBrush, fillPath);
        }
        else
        {
            // Small dash when no devices are connected
            using var dashBrush = new SolidBrush(fillColor);
            g.FillRectangle(dashBrush, 10f, 15f, 9f, 2f);
        }

        IntPtr hIcon = bmp.GetHicon();
        try
        {
            using var temp = Icon.FromHandle(hIcon);
            return (Icon)temp.Clone();
        }
        finally
        {
            DestroyIcon(hIcon);
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
