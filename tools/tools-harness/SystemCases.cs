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

    // GIFs for the trimmer's frame compositing: transparent and opaque,
    // partial and full frames, with every disposal (patched into the graphic
    // control blocks AnimatedGifCreator writes).
    public static void MakeDisposalGifs(string dir)
    {
        // (left, top, width, height, transparent hole, disposal)
        (int, int, int, int, bool, int)[][] sets =
        [
            [(0, 0, 48, 36, false, 1), (4, 4, 20, 14, true, 1), (10, 8, 24, 20, false, 2), (2, 18, 30, 16, true, 3),
             (0, 0, 48, 36, true, 1), (20, 2, 26, 30, false, 1), (6, 6, 12, 12, true, 0)],
            [(8, 6, 30, 22, true, 3), (0, 0, 48, 36, false, 2), (5, 5, 20, 20, true, 1), (0, 0, 48, 36, true, 2),
             (12, 10, 20, 12, true, 3), (12, 10, 30, 20, false, 1), (0, 0, 48, 36, true, 0)],
            [(0, 0, 48, 36, true, 2), (3, 3, 40, 28, true, 2), (0, 0, 48, 36, true, 3), (10, 10, 10, 10, false, 3),
             (10, 10, 28, 20, true, 1), (0, 0, 48, 36, false, 1)],
        ];
        for (int s = 0; s < sets.Length; s++)
        {
            string path = Path.Combine(dir, $"disp{s}.gif");
            if (File.Exists(path)) continue;
            using (ShareX.HelpersLib.AnimatedGifCreator creator = new(path, 100, 0, true))
            {
                for (int i = 0; i < sets[s].Length; i++)
                {
                    var (l, t, w, h, hole, _) = sets[s][i];
                    using SkiaSharp.SKBitmap bmp = new(w, h);
                    using (SkiaSharp.SKCanvas cv = new(bmp))
                    {
                        cv.Clear(new SkiaSharp.SKColor((byte)(40 + i * 30 + s * 7), (byte)(200 - i * 25), (byte)(90 + s * 40)));
                        using SkiaSharp.SKPaint p = new() { Color = new SkiaSharp.SKColor((byte)(i * 37), 20, (byte)(250 - i * 30)) };
                        cv.DrawRect(1, h / 2, w - 2, 2, p);
                        if (hole)
                        {
                            using SkiaSharp.SKPaint clear = new() { BlendMode = SkiaSharp.SKBlendMode.Clear };
                            cv.DrawRect(w / 4, h / 4, w / 2, h / 3, clear);
                        }
                    }
                    var image = ShareX.HelpersLib.SkiaImageHelpers.Quantize(bmp, ShareX.HelpersLib.GIFQuality.Adaptive);
                    creator.AddFrame(image, 10 * (i + 2), l, t, true);
                }
            }
            byte[] b = File.ReadAllBytes(path);
            // The logical screen is 48x36 even when the first frame is smaller.
            b[6] = 48; b[7] = 0; b[8] = 36; b[9] = 0;
            int frame = 0, pos = 13;
            if ((b[10] & 0x80) != 0) pos += 3 << ((b[10] & 7) + 1);
            void SkipSubBlocks() { while (b[pos] != 0) pos += b[pos] + 1; pos++; }
            while (b[pos] != 0x3B)
            {
                if (b[pos] == 0x21)
                {
                    if (b[pos + 1] == 0xF9)
                        b[pos + 3] = (byte)((b[pos + 3] & ~0x1C) | (sets[s][frame].Item6 << 2));
                    pos += 2;
                    SkipSubBlocks();
                }
                else
                {
                    byte packed = b[pos + 9];
                    pos += 10;
                    if ((packed & 0x80) != 0) pos += 3 << ((packed & 7) + 1);
                    pos++;
                    SkipSubBlocks();
                    frame++;
                }
            }
            File.WriteAllBytes(path, b);
        }
    }

    // The trimmer's frames (SkiaAnimatedImage.GetFrame), its preview
    // (RenderFrame) and Export: lossless, re-encoded, cropped and resized.
    static bool RunGifReencodeCase(JObject c, string o)
    {
        if ((string)c["tool"] != "gif-reencode") return false;
        string path = Path.Combine(Inputs_, (string)c["file"]);
        Assembly asm = typeof(MonitorTestPatternControl).Assembly;
        Type doc = asm.GetType("ShareX.Tools.AnimatedGifTrimmerDocument");
        JObject r = new();
        Directory.CreateDirectory(o);
        using (ShareX.HelpersLib.SkiaAnimatedImage image = new(path))
        {
            r["frames"] = image.FrameCount;
            for (int i = 0; i < image.FrameCount; i++)
            {
                using SkiaSharp.SKBitmap f = image.GetFrame(i);
                Sxrb(Path.Combine(o, $"frame{i}.sxrb"), f);
            }
        }
        using (IDisposable d = (IDisposable)doc.GetMethod("Open").Invoke(null, [path, CancellationToken.None]))
        {
            int count = (int)doc.GetProperty("FrameCount").GetValue(d);
            int preview = (int?)c["preview"] ?? 640;
            for (int i = 0; i < count; i++)
                File.WriteAllBytes(Path.Combine(o, $"preview{i}.png"),
                    (byte[])doc.GetMethod("RenderFrame").Invoke(d, [i, CancellationToken.None, preview]));
            JArray results = new();
            int n = 0;
            foreach (JArray e in (JArray)c["exports"])
            {
                int first = (int)e[0], end = (int)e[1];
                bool reencode = (bool)e[2];
                JArray cr = (JArray)e[3], sz = (JArray)e[4];
                SkiaSharp.SKRectI crop = SkiaSharp.SKRectI.Create((int)cr[0], (int)cr[1], (int)cr[2], (int)cr[3]);
                SkiaSharp.SKSizeI size = new((int)sz[0], (int)sz[1]);
                string outPath = Path.Combine(o, $"export{n++}.gif");
                File.Delete(outPath);
                try
                {
                    bool ok = (bool)doc.GetMethod("Export").Invoke(d, [outPath, first, end, reencode, crop, size, null, CancellationToken.None]);
                    results.Add(ok);
                }
                catch (TargetInvocationException ex) { results.Add(ex.InnerException.GetType().Name); }
            }
            r["exports"] = results;
        }
        File.WriteAllText(o + ".json", r.ToString());
        return true;
    }

    static bool RunGifTrimCase(JObject c, string o)
    {
        if (RunGifReencodeCase(c, o)) return true;
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
