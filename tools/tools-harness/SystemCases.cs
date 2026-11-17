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

    // A five-frame GIF with varied delays, partial frames and disposals, written
    // by ShareX's own AnimatedGifCreator (frames quantized adaptively).
    public static void MakeTrimGif(string path)
    {
        if (File.Exists(path)) return;
        using ShareX.HelpersLib.AnimatedGifCreator creator = new(path, 100, 2, true);
        int[] delays = [30, 70, 100, 10, 250];
        for (int i = 0; i < 5; i++)
        {
            int w = i % 2 == 0 ? 60 : 30, h = i % 2 == 0 ? 40 : 20;
            using SkiaSharp.SKBitmap bmp = new(w, h);
            using (SkiaSharp.SKCanvas c = new(bmp)) { c.Clear(new SkiaSharp.SKColor((byte)(i * 50), (byte)(255 - i * 40), (byte)(i * 20))); c.DrawRect(2, 2, w / 3, h / 3, new SkiaSharp.SKPaint { Color = SkiaSharp.SKColors.Black }); }
            var image = ShareX.HelpersLib.SkiaImageHelpers.Quantize(bmp, ShareX.HelpersLib.GIFQuality.Adaptive);
            creator.AddFrame(image, delays[i], i % 2 == 0 ? 0 : 15, i % 2 == 0 ? 0 : 10, i != 3);
        }
    }

    static bool RunGifTrimCase(JObject c, string o)
    {
        if ((string)c["tool"] != "gif-trim") return false;
        string path = Path.Combine(Inputs_, (string)c["file"]);
        Assembly asm = typeof(MonitorTestPatternControl).Assembly;
        Type t = asm.GetType("ShareX.Tools.GifLosslessTrimmer");
        JObject r = new();
        object trimmer;
        try { trimmer = t.GetMethod("Parse").Invoke(null, [path, CancellationToken.None]); }
        catch (TargetInvocationException e) { r["error"] = e.InnerException.GetType().Name; File.WriteAllText(o + ".json", r.ToString()); return true; }
        int count = (int)t.GetProperty("FrameCount").GetValue(trimmer);
        r["frames"] = count;
        r["loop"] = (bool)t.GetProperty("Loop").GetValue(trimmer);
        r["repeat"] = (int)t.GetProperty("RepeatCount").GetValue(trimmer);
        r["delays"] = new JArray(Enumerable.Range(0, count).Select(i => (int)t.GetMethod("GetDelay").Invoke(trimmer, [i])));
        r["needsValidation"] = new JArray(Enumerable.Range(0, count).Select(i => (bool)t.GetMethod("NeedsVisualValidation").Invoke(trimmer, [i])));
        JArray copies = new();
        Directory.CreateDirectory(o);
        for (int first = 0; first < count; first++)
            for (int end = first + 1; end <= count; end++)
            {
                bool can = (bool)t.GetMethod("CanCopySelection").Invoke(trimmer, [first, end]);
                copies.Add(new JArray(first, end, can));
                if (!can) continue;
                string outPath = Path.Combine(o, $"{first}-{end}.gif");
                File.Delete(outPath);
                t.GetMethod("Copy").Invoke(trimmer, [path, outPath, first, end, null, CancellationToken.None]);
            }
        r["copies"] = copies;
        Type doc = asm.GetType("ShareX.Tools.AnimatedGifTrimmerDocument");
        using (IDisposable d = (IDisposable)doc.GetMethod("Open").Invoke(null, [path, CancellationToken.None]))
        {
            double[] times = [0, 0.05, 0.29, 0.3, 0.31, 1.0, 1.995, 2.11, 4.6, 99];
            r["frameAt"] = new JArray(times.Select(x => (int)doc.GetMethod("FrameAt").Invoke(d, [x])));
            r["nearest"] = new JArray(times.Select(x => (int)doc.GetMethod("NearestBoundary").Invoke(d, [x, false])));
            r["nearestEnd"] = new JArray(times.Select(x => (int)doc.GetMethod("NearestBoundary").Invoke(d, [x, true])));
        }
        File.WriteAllText(o + ".json", r.ToString());
        return true;
    }

    static bool RunSystemCase(JObject c, string o)
    {
        if (RunGifTrimCase(c, o)) return true;
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
