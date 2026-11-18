// The image editor's image effects (ShareX.ImageEditor/Core/ImageEffects),
// run with ShareX's own code, headless (SkiaSharp only, no Avalonia platform).
//
//     EditorHarness --effects <work dir> [case name prefix]
//
// Writes, under <work dir>:
//   catalog.json   every effect ImageEffectCatalog lists, in its order: id,
//                  name, browser label, icon, description, category, whether
//                  it applies immediately, its parameters (kind, key, label,
//                  ranges, defaults, options) and its properties' defaults
//   inputs/        the test images (premultiplied PNGs, see below) and the
//                  files the file path parameters point at
//   cases.json     the cases, generated from the catalogue: each effect's
//                  defaults on both test images, then all parameters at their
//                  minimum, at their maximum, at a third of their range, and
//                  each enum option in turn
//   sharex/        one <case>.png per case (premultiplied, see below)
//   results.json   per case "ok" or the exception ShareX threw
//   probe/         how SkiaSharp converts pixels (SKBitmap.Pixels both ways,
//                  GetPixel) and what Random.Shared draws from a set state
//
// Each case is applied as ShareX's effect dialog applies it: the definition's
// parameter states (EffectParameterState.Create), their values set, then
// EffectDefinition.CreateConfiguredEffect(states).Apply(source).
//
// Random.Shared is made deterministic: before each case its thread's
// generator (Random.ThreadSafeRandom.t_random, a XoshiroImpl) is replaced by
// one whose state is splitmix64 of the case's seed, which ShareXrs reproduces.
//
// Premultiplied PNGs: the bitmap's premultiplied BGRA bytes stored as RGBA
// without conversion (a normal PNG would unpremultiply and round).

using System.Globalization;
using System.Reflection;
using System.Runtime.CompilerServices;
using System.Text.Json;
using System.Text.Json.Nodes;
using ShareX.ImageEditor.Core.ImageEffects;
using ShareX.ImageEditor.Core.ImageEffects.Parameters;
using ShareX.ImageEditor.Presentation.Effects;
using SkiaSharp;

namespace EditorHarness;

internal static class EffectsHarness
{
    private static readonly CultureInfo Inv = CultureInfo.InvariantCulture;

    public static int Run(string[] args)
    {
        string work = Path.GetFullPath(args[1]);
        string? only = args.Length > 2 ? args[2] : null;
        string inputs = Path.Combine(work, "inputs");
        string outDir = Path.Combine(work, "sharex");
        string probeDir = Path.Combine(work, "probe");
        foreach (string dir in new[] { inputs, outDir, probeDir }) Directory.CreateDirectory(dir);

        Inputs(inputs);
        Probe(probeDir);
        JsonArray catalog = DumpCatalog();
        File.WriteAllText(Path.Combine(work, "catalog.json"), catalog.ToJsonString(new JsonSerializerOptions { WriteIndented = true }));

        JsonArray cases = GenerateCases();
        File.WriteAllText(Path.Combine(work, "cases.json"), cases.ToJsonString(new JsonSerializerOptions { WriteIndented = true }));

        JsonObject results = new();
        int failures = 0, count = 0;
        foreach (JsonObject c in cases.Cast<JsonObject>())
        {
            string name = (string)c["name"]!;
            if (only != null && !name.StartsWith(only, StringComparison.Ordinal)) continue;
            count++;
            var sw = System.Diagnostics.Stopwatch.StartNew();
            try
            {
                using SKBitmap result = RunCase(c, inputs);
                SavePremulPng(result, Path.Combine(outDir, FileName(name) + ".png"));
                results[name] = "ok";
            }
            catch (Exception e)
            {
                failures++;
                Exception inner = e is TargetInvocationException { InnerException: { } ie } ? ie : e;
                results[name] = "error: " + inner.GetType().Name + ": " + inner.Message;
                Console.Error.WriteLine($"{name}: {inner.GetType().Name}: {inner.Message}");
            }
            if (sw.ElapsedMilliseconds > 2000) Console.Error.WriteLine($"{name}: slow, {sw.ElapsedMilliseconds} ms");
        }
        File.WriteAllText(Path.Combine(work, "results.json"), results.ToJsonString(new JsonSerializerOptions { WriteIndented = true }));
        Console.WriteLine($"{count} cases, {failures} threw");
        return 0;
    }

    public static string FileName(string caseName) => caseName.Replace('/', '_');

    private static SKBitmap RunCase(JsonObject c, string inputs)
    {
        string id = (string)c["effect"]!;
        if (!ImageEffectCatalog.TryGetDefinition(id, out EffectDefinition? definition) || definition == null)
            throw new InvalidOperationException("no effect " + id);
        using SKBitmap source = LoadPremulPng(Path.Combine(inputs, (string)c["input"]! + ".png"));
        List<EffectParameterState> states = definition.CoreParameters.Select(EffectParameterState.Create).ToList();
        JsonObject values = (JsonObject?)c["params"] ?? new JsonObject();
        foreach (EffectParameterState state in states)
        {
            if (!values.TryGetPropertyValue(state.Key, out JsonNode? v) || v == null) continue;
            switch (state)
            {
                case SliderParameterState s: s.Value = (double)v; break;
                case NumericParameterState n: n.Value = decimal.Parse(v.ToJsonString(), NumberStyles.Float, Inv); break;
                case CheckboxParameterState b: b.Value = (bool)v; break;
                case EnumParameterState e: e.SelectedOption = e.Options[(int)v]; break;
                case ColorParameterState col:
                    {
                        uint argb = uint.Parse(((string)v!).TrimStart('#'), NumberStyles.HexNumber, Inv);
                        col.Value = Avalonia.Media.Color.FromUInt32(argb);
                        break;
                    }
                case TextParameterState t: t.Value = (string)v!; break;
                case FilePathParameterState f: f.Value = ((string)v!).Replace("{inputs}", inputs); break;
                default: throw new NotSupportedException(state.GetType().Name);
            }
        }
        SeedShared((ulong)(long)c["seed"]!);
        ImageEffect effect = definition.CreateConfiguredEffect(states);
        SKBitmap result = effect.Apply(source);
        if (ReferenceEquals(result, source)) return source.Copy();
        return result;
    }

    // ---- Random.Shared ----

    private static readonly FieldInfo TRandom = typeof(Random)
        .GetNestedType("ThreadSafeRandom", BindingFlags.NonPublic)!
        .GetField("t_random", BindingFlags.NonPublic | BindingFlags.Static)!;
    private static readonly Type XoshiroType = typeof(Random).GetNestedType("XoshiroImpl", BindingFlags.NonPublic)!;

    /// <summary>splitmix64, as ShareXrs seeds its Xoshiro256** for validation.</summary>
    private static ulong SplitMix(ref ulong x)
    {
        ulong z = x += 0x9E3779B97F4A7C15UL;
        z = (z ^ (z >> 30)) * 0xBF58476D1CE4E5B9UL;
        z = (z ^ (z >> 27)) * 0x94D049BB133111EBUL;
        return z ^ (z >> 31);
    }

    public static void SeedShared(ulong seed)
    {
        object impl = RuntimeHelpers.GetUninitializedObject(XoshiroType);
        ulong x = seed;
        foreach (string f in new[] { "_s0", "_s1", "_s2", "_s3" })
            XoshiroType.GetField(f, BindingFlags.NonPublic | BindingFlags.Instance)!.SetValue(impl, SplitMix(ref x));
        _ = Random.Shared.Next(); // makes sure the thread's generator exists before it is replaced
        TRandom.SetValue(null, impl);
    }

    // ---- Catalogue ----

    private static JsonArray DumpCatalog()
    {
        JsonArray list = new();
        foreach (EffectDefinition d in ImageEffectCatalog.Definitions)
        {
            JsonObject o = new()
            {
                ["id"] = d.Id,
                ["name"] = d.Name,
                ["browserLabel"] = d.BrowserLabel,
                ["icon"] = d.Icon,
                ["description"] = d.Description,
                ["category"] = d.Category.ToString(),
                ["applyImmediately"] = d.ApplyImmediately,
            };
            JsonArray ps = new();
            foreach (EffectParameter p in d.CoreParameters) ps.Add(DumpParameter(p));
            o["parameters"] = ps;
            o["type"] = d.CreateEffect().GetType().FullName;
            o["properties"] = DumpProperties(d.CreateEffect());
            o["configured"] = DumpProperties(d.CreateConfiguredEffect(d.CoreParameters.Select(EffectParameterState.Create).ToList()));
            list.Add(o);
        }
        return list;
    }

    private static JsonObject DumpParameter(EffectParameter p)
    {
        JsonObject o = new() { ["key"] = p.Key, ["label"] = p.Label };
        if (p.Description != null) o["description"] = p.Description;
        switch (p)
        {
            case SliderEffectParameter s:
                o["kind"] = "slider";
                o["min"] = s.Minimum; o["max"] = s.Maximum; o["default"] = s.DefaultValue;
                o["tick"] = s.TickFrequency; o["snap"] = s.IsSnapToTickEnabled; o["format"] = s.ValueStringFormat;
                break;
            case NumericEffectParameter n:
                o["kind"] = "numeric";
                o["min"] = n.Minimum; o["max"] = n.Maximum; o["default"] = n.DefaultValue;
                o["increment"] = n.Increment; o["format"] = n.FormatString;
                break;
            case CheckboxEffectParameter b:
                o["kind"] = "checkbox"; o["default"] = b.DefaultValue;
                break;
            case EnumEffectParameter e:
                o["kind"] = "enum"; o["default"] = e.DefaultIndex;
                JsonArray opts = new();
                foreach (EffectOption opt in e.Options) opts.Add(new JsonObject { ["label"] = opt.Label, ["value"] = opt.Value.ToString() });
                o["options"] = opts;
                break;
            case ColorEffectParameter c:
                o["kind"] = "color"; o["default"] = Hex(c.DefaultValue);
                break;
            case TextEffectParameter t:
                o["kind"] = "text"; o["default"] = t.DefaultValue;
                break;
            case FilePathEffectParameter f:
                o["kind"] = "file"; o["default"] = f.DefaultValue;
                if (f.FileFilter != null) o["filter"] = f.FileFilter;
                break;
            default:
                throw new NotSupportedException(p.GetType().Name);
        }
        return o;
    }

    private static JsonObject DumpProperties(object effect)
    {
        JsonObject o = new();
        foreach (PropertyInfo prop in effect.GetType().GetProperties(BindingFlags.Public | BindingFlags.Instance).OrderBy(p => p.Name, StringComparer.Ordinal))
        {
            if (prop.GetIndexParameters().Length > 0 || !prop.CanRead) continue;
            if (prop.Name is "Parameters" or "Name" or "Category" or "IconKey" or "Id" or "Description" or "BrowserLabel" or "ExecutionMode" or "HasParameters") continue;
            object? v;
            try { v = prop.GetValue(effect); } catch { continue; }
            o[prop.Name] = v switch
            {
                null => null,
                SKColor c => Hex(c),
                float f => f.ToString("R", Inv),
                double dd => dd.ToString("R", Inv),
                bool b => b,
                IFormattable fm => fm.ToString(null, Inv),
                _ => v.ToString(),
            };
        }
        return o;
    }

    private static string Hex(SKColor c) => $"#{c.Alpha:X2}{c.Red:X2}{c.Green:X2}{c.Blue:X2}";

    // ---- Cases ----

    private static JsonArray GenerateCases()
    {
        JsonArray cases = new();
        long seed = 1;
        void Add(string name, string id, string input, JsonObject values)
        {
            cases.Add(new JsonObject { ["name"] = name, ["effect"] = id, ["input"] = input, ["seed"] = seed++, ["params"] = values });
        }

        foreach (EffectDefinition d in ImageEffectCatalog.Definitions)
        {
            Add($"{d.Id}/default-a", d.Id, "a", new JsonObject());
            Add($"{d.Id}/default-b", d.Id, "b", new JsonObject());
            if (d.CoreParameters.Count == 0) continue;
            Add($"{d.Id}/min", d.Id, "a", Values(d, 0));
            Add($"{d.Id}/max", d.Id, "a", Values(d, 1));
            Add($"{d.Id}/third", d.Id, "a", Values(d, 2));
            foreach (EffectParameter p in d.CoreParameters)
            {
                if (p is not EnumEffectParameter e) continue;
                for (int i = 0; i < e.Options.Count; i++)
                {
                    if (i == e.DefaultIndex) continue;
                    Add($"{d.Id}/{p.Key}-{i}", d.Id, "a", new JsonObject { [p.Key] = i });
                }
            }
        }
        return cases;
    }

    /// <summary>Numeric boxes can go to thousands (sizes, counts); the cases keep them small.</summary>
    private const decimal NumericCap = 100;

    /// <summary>All parameters at their minimum (0), maximum (1) or a third of the way (2).</summary>
    private static JsonObject Values(EffectDefinition d, int which)
    {
        JsonObject o = new();
        foreach (EffectParameter p in d.CoreParameters)
        {
            switch (p)
            {
                case SliderEffectParameter s:
                    {
                        double v = which switch
                        {
                            0 => s.Minimum,
                            1 => s.Maximum,
                            _ => Third(s.Minimum, s.Maximum, s.TickFrequency, s.IsSnapToTickEnabled),
                        };
                        o[p.Key] = v;
                        break;
                    }
                case NumericEffectParameter n:
                    {
                        decimal min = Math.Max(n.Minimum, -NumericCap), max = Math.Min(n.Maximum, NumericCap);
                        if (max < min) max = min;
                        decimal v = which switch
                        {
                            0 => min,
                            1 => max,
                            _ => min + Math.Round((max - min) / 3m / n.Increment, MidpointRounding.AwayFromZero) * n.Increment,
                        };
                        o[p.Key] = JsonNode.Parse(v.ToString(Inv));
                        break;
                    }
                case CheckboxEffectParameter b:
                    o[p.Key] = which switch { 0 => false, 1 => true, _ => !b.DefaultValue };
                    break;
                case EnumEffectParameter e:
                    o[p.Key] = which switch { 0 => 0, 1 => e.Options.Count - 1, _ => (e.DefaultIndex + 1) % e.Options.Count };
                    break;
                case ColorEffectParameter:
                    o[p.Key] = which switch { 0 => "#00000000", 1 => "#FFFFFFFF", _ => "#FF2080E0" };
                    break;
                case TextEffectParameter t:
                    o[p.Key] = which switch { 0 => "", 1 => "The quick brown fox", _ => t.Key == "font_family" ? "Segoe UI" : "ShareXrs 123" };
                    break;
                case FilePathEffectParameter f:
                    o[p.Key] = which == 0 ? "" : f.Key == "image_folder" ? "{inputs}/particles" : f.Key == "map_file_path" ? "{inputs}/map.png" : "{inputs}/overlay.png";
                    break;
            }
        }
        return o;
    }

    private static double Third(double min, double max, double tick, bool snap)
    {
        double v = min + (max - min) / 3;
        if (snap && tick > 0) v = min + Math.Round((v - min) / tick) * tick;
        return Math.Round(v, 6);
    }

    // ---- Images ----

    private static void Inputs(string dir)
    {
        // a: 48x36 opaque, smooth gradients, hard edges, a checker patch and fine detail.
        int w = 48, h = 36;
        byte[] a = new byte[w * h * 4];
        for (int y = 0; y < h; y++)
        {
            for (int x = 0; x < w; x++)
            {
                int r = (x * 255) / (w - 1);
                int g = (y * 255) / (h - 1);
                int b = (int)Math.Round(128 + 100 * Math.Sin(x * 0.35) * Math.Cos(y * 0.27));
                if (x >= 4 && x < 16 && y >= 4 && y < 13) { r = 250; g = 250; b = 245; }
                int dx = x - 33, dy = y - 23;
                if (dx * dx + dy * dy <= 49) { r = 20; g = 30; b = 60; }
                if (x >= 22 && x < 29 && y >= 3 && y < 9 && ((x + y) & 1) == 0) { r = 0; g = 0; b = 0; }
                if (y >= 30 && x < 15) { int v = ((x * 37 + y * 91) % 17) * 15; r = v; g = 255 - v; b = (v * 3) % 256; }
                Set(a, w, x, y, r, g, b, 255);
            }
        }
        SavePremulPngBytes(a, w, h, Path.Combine(dir, "a.png"));

        // b: 32x24 translucent: alpha ramps from 0 to 255, a fully transparent band and an opaque block.
        w = 32; h = 24;
        byte[] bpx = new byte[w * h * 4];
        for (int y = 0; y < h; y++)
        {
            for (int x = 0; x < w; x++)
            {
                int alpha = Math.Min(255, (x * 255) / (w - 1) + (y % 7) * 9);
                if (y >= 10 && y < 12) alpha = 0;
                if (x >= 22 && y >= 17) alpha = 255;
                int r = 200 - x * 4, g = 40 + y * 8, bb = (x * y) % 256;
                Set(bpx, w, x, y, r, g, bb, alpha);
            }
        }
        SavePremulPngBytes(bpx, w, h, Path.Combine(dir, "b.png"));

        // Files for the file path parameters (ordinary straight-alpha PNGs, decoded by ShareX's code).
        SaveStraightPng(20, 16, (x, y) => ((x * 12) % 256, 255 - y * 15, 90, x < 3 ? 0 : 160 + y * 5), Path.Combine(dir, "overlay.png"));
        SaveStraightPng(32, 24, (x, y) => ((x * 8) % 256, (y * 10) % 256, 128, 255), Path.Combine(dir, "map.png"));
        string particles = Path.Combine(dir, "particles");
        Directory.CreateDirectory(particles);
        SaveStraightPng(8, 8, (x, y) => (255, x * 30, 0, (x + y) < 4 ? 0 : 255), Path.Combine(particles, "p1.png"));
        SaveStraightPng(6, 10, (x, y) => (0, 120, y * 25, 200), Path.Combine(particles, "p2.png"));
    }

    private static void Set(byte[] px, int w, int x, int y, int r, int g, int b, int alpha)
    {
        static int Pm(int c, int a) => (Math.Clamp(c, 0, 255) * a + 127) / 255;
        int i = (y * w + x) * 4;
        px[i] = (byte)Pm(r, alpha); px[i + 1] = (byte)Pm(g, alpha); px[i + 2] = (byte)Pm(b, alpha); px[i + 3] = (byte)alpha;
    }

    private static void SaveStraightPng(int w, int h, Func<int, int, (int, int, int, int)> f, string path)
    {
        using SKBitmap bmp = new(new SKImageInfo(w, h, SKColorType.Rgba8888, SKAlphaType.Unpremul));
        Span<byte> span = bmp.GetPixelSpan();
        for (int y = 0; y < h; y++)
            for (int x = 0; x < w; x++)
            {
                var (r, g, b, a) = f(x, y);
                int i = (y * w + x) * 4;
                span[i] = (byte)Math.Clamp(r, 0, 255); span[i + 1] = (byte)Math.Clamp(g, 0, 255);
                span[i + 2] = (byte)Math.Clamp(b, 0, 255); span[i + 3] = (byte)Math.Clamp(a, 0, 255);
            }
        using SKData data = bmp.Encode(SKEncodedImageFormat.Png, 100);
        File.WriteAllBytes(path, data.ToArray());
    }

    /// <summary>Premultiplied RGBA bytes stored in a PNG as they are.</summary>
    private static void SavePremulPngBytes(byte[] rgba, int w, int h, string path)
    {
        using SKBitmap bmp = new(new SKImageInfo(w, h, SKColorType.Rgba8888, SKAlphaType.Unpremul));
        rgba.CopyTo(bmp.GetPixelSpan());
        using SKData data = bmp.Encode(SKEncodedImageFormat.Png, 100);
        File.WriteAllBytes(path, data.ToArray());
    }

    public static void SavePremulPng(SKBitmap bitmap, string path)
    {
        int w = Math.Max(bitmap.Width, 0), h = Math.Max(bitmap.Height, 0);
        if (w == 0 || h == 0)
        {
            File.WriteAllText(path + ".empty", $"{bitmap.Width}x{bitmap.Height}");
            return;
        }
        using SKBitmap bgra = new(new SKImageInfo(w, h, SKColorType.Bgra8888, SKAlphaType.Premul));
        if (bitmap.ColorType == SKColorType.Bgra8888 && bitmap.AlphaType == SKAlphaType.Premul)
        {
            // Raw bytes: a pixel operation can leave colour above alpha, which a conversion would clamp.
            for (int y = 0; y < h; y++)
                unsafe
                {
                    Buffer.MemoryCopy((byte*)bitmap.GetPixels() + (long)y * bitmap.RowBytes, (byte*)bgra.GetPixels() + (long)y * bgra.RowBytes, w * 4, w * 4);
                }
        }
        else if (!bitmap.CopyTo(bgra)) throw new InvalidOperationException("CopyTo " + bitmap.ColorType + " " + bitmap.AlphaType);
        byte[] rgba = new byte[w * h * 4];
        ReadOnlySpan<byte> src = bgra.GetPixelSpan();
        for (int i = 0; i < w * h; i++)
        {
            rgba[i * 4] = src[i * 4 + 2]; rgba[i * 4 + 1] = src[i * 4 + 1]; rgba[i * 4 + 2] = src[i * 4]; rgba[i * 4 + 3] = src[i * 4 + 3];
        }
        SavePremulPngBytes(rgba, w, h, path);
    }

    public static SKBitmap LoadPremulPng(string path)
    {
        using SKCodec codec = SKCodec.Create(path);
        SKImageInfo info = new(codec.Info.Width, codec.Info.Height, SKColorType.Rgba8888, SKAlphaType.Unpremul);
        using SKBitmap raw = new(info);
        if (codec.GetPixels(info, raw.GetPixels()) != SKCodecResult.Success) throw new InvalidDataException(path);
        SKBitmap bmp = new(new SKImageInfo(info.Width, info.Height, SKColorType.Bgra8888, SKAlphaType.Premul));
        ReadOnlySpan<byte> s = raw.GetPixelSpan();
        Span<byte> d = bmp.GetPixelSpan();
        for (int i = 0; i < info.Width * info.Height; i++)
        {
            d[i * 4] = s[i * 4 + 2]; d[i * 4 + 1] = s[i * 4 + 1]; d[i * 4 + 2] = s[i * 4]; d[i * 4 + 3] = s[i * 4 + 3];
        }
        bmp.NotifyPixelsChanged();
        return bmp;
    }

    // ---- Probes ----

    private static void Probe(string dir)
    {
        // Pixels getter: premultiplied (c, c, c, a) for every c <= a at (c, a).
        using (SKBitmap bmp = new(new SKImageInfo(256, 256, SKColorType.Bgra8888, SKAlphaType.Premul)))
        {
            Span<byte> d = bmp.GetPixelSpan();
            for (int a = 0; a < 256; a++)
                for (int c = 0; c < 256; c++)
                {
                    int i = (a * 256 + c) * 4;
                    byte v = (byte)Math.Min(c, a);
                    d[i] = v; d[i + 1] = v; d[i + 2] = v; d[i + 3] = (byte)a;
                }
            bmp.NotifyPixelsChanged();
            SKColor[] px = bmp.Pixels;
            byte[] o = new byte[256 * 256];
            for (int i = 0; i < px.Length; i++) o[i] = px[i].Red;
            File.WriteAllBytes(Path.Combine(dir, "pixels-get.bin"), o);
            byte[] g = new byte[256 * 256];
            for (int a = 0; a < 256; a++)
                for (int c = 0; c < 256; c++) g[a * 256 + c] = bmp.GetPixel(c, a).Red;
            File.WriteAllBytes(Path.Combine(dir, "getpixel.bin"), g);
        }
        // Pixels setter: straight (c, c, c, a) for every c, a.
        using (SKBitmap bmp = new(new SKImageInfo(256, 256, SKColorType.Bgra8888, SKAlphaType.Premul)))
        {
            SKColor[] px = new SKColor[256 * 256];
            for (int a = 0; a < 256; a++)
                for (int c = 0; c < 256; c++) px[a * 256 + c] = new SKColor((byte)c, (byte)(255 - c), (byte)(c / 2), (byte)a);
            bmp.Pixels = px;
            ReadOnlySpan<byte> d = bmp.GetPixelSpan();
            byte[] o = new byte[256 * 256 * 3];
            for (int i = 0; i < 256 * 256; i++) { o[i * 3] = d[i * 4 + 2]; o[i * 3 + 1] = d[i * 4 + 1]; o[i * 3 + 2] = d[i * 4]; }
            File.WriteAllBytes(Path.Combine(dir, "pixels-set.bin"), o);
        }
        // Random.Shared from splitmix64(12345).
        using (StreamWriter w = new(Path.Combine(dir, "random.txt")))
        {
            SeedShared(12345);
            for (int i = 0; i < 20; i++) w.WriteLine("Next() " + Random.Shared.Next().ToString(Inv));
            foreach (int max in new[] { 1, 2, 3, 7, 10, 100, 360, 1000, int.MaxValue })
                for (int i = 0; i < 5; i++) w.WriteLine($"Next({max}) " + Random.Shared.Next(max).ToString(Inv));
            foreach ((int lo, int hi) in new[] { (0, 1), (0, 64), (-5, 5), (3, 4), (10, 11), (-1000, 1000), (0, 360), (int.MinValue, int.MaxValue) })
                for (int i = 0; i < 5; i++) w.WriteLine($"Next({lo},{hi}) " + Random.Shared.Next(lo, hi).ToString(Inv));
            for (int i = 0; i < 10; i++) w.WriteLine("NextSingle() " + Random.Shared.NextSingle().ToString("R", Inv));
            for (int i = 0; i < 10; i++) w.WriteLine("NextDouble() " + Random.Shared.NextDouble().ToString("R", Inv));
        }
    }
}
