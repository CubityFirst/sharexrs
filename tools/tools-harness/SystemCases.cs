// The System tools' cases: Monitor test patterns rendered by ShareX's own
// MonitorTestPatternControl through Avalonia.Headless (Skia drawing, no window).

using System.Reflection;
using Avalonia;
using Avalonia.Headless;
using Avalonia.Media;
using Avalonia.Media.Imaging;
using Avalonia.Threading;
using Newtonsoft.Json.Linq;
using ShareX.Tools;
using ShareX.Tools.Controls;

static partial class Program
{
    static bool _avalonia;

    static void EnsureAvalonia()
    {
        if (_avalonia) return;
        AppBuilder.Configure<Application>()
            .UseSkia()
            .UseHeadless(new AvaloniaHeadlessPlatformOptions { UseHeadlessDrawing = false })
            .SetupWithoutStarting();
        _avalonia = true;
    }

    static bool RunSystemCase(JObject c, string o)
    {
        if ((string)c["tool"] != "monitor-test") return false;
        EnsureAvalonia();
        Dispatcher.UIThread.Invoke(() =>
        {
            int w = (int)c["width"], h = (int)c["height"];
            MonitorTestPatternControl control = new()
            {
                Mode = System.Enum.Parse<MonitorTestMode>((string)c["mode"]),
                GrayValue = (double?)c["gray"] ?? 255,
                Red = (double?)c["red"] ?? 255,
                Green = (double?)c["green"] ?? 0,
                Blue = (double?)c["blue"] ?? 0,
                GradientStart = c["start"] is JArray s ? Avalonia.Media.Color.FromRgb((byte)(int)s[0], (byte)(int)s[1], (byte)(int)s[2]) : Colors.White,
                GradientEnd = c["end"] is JArray e ? Avalonia.Media.Color.FromRgb((byte)(int)e[0], (byte)(int)e[1], (byte)(int)e[2]) : Colors.Black,
                GradientDirection = System.Enum.Parse<MonitorGradientDirection>((string)c["direction"] ?? "Vertical"),
                Pattern = System.Enum.Parse<MonitorPattern>((string)c["pattern"] ?? "HorizontalLines"),
                PatternSize = (double?)c["patternSize"] ?? 5,
                MotionDirection = System.Enum.Parse<MonitorMotionDirection>((string)c["motion"] ?? "VerticalBars"),
            };
            typeof(MonitorTestPatternControl).GetField("_motionOffset", BindingFlags.NonPublic | BindingFlags.Instance)
                .SetValue(control, (double?)c["offset"] ?? 0);
            control.Measure(new Size(w, h));
            control.Arrange(new Rect(0, 0, w, h));
            using RenderTargetBitmap rtb = new(new PixelSize(w, h), new Vector(96, 96));
            rtb.Render(control);
            rtb.Save(o + ".png");
        });
        return true;
    }
}
