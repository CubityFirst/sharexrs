// ShareXrs image effects harness: applies ShareX's own ShareX.ImageEffectsLib to
// fixed test images, headlessly, and writes the raw premultiplied pixels so
// ShareXrs's port (crates/sharex-image-effects) can be compared byte for byte.
//
//   EffectsHarness <cases.json> <work dir>
//
// Writes, under <work dir>:
//   inputs/      the test images (raw .sxrb + .png previews) and the files the
//                drawing effects load (watermark.png, background.png, particles/)
//   sharex/      one <case>.sxrb (+ .png preview) per case
//   roundtrip/   each case's preset as ShareX serialises it back (export format)
//   defaults/    every effect's default instance, in the settings format
//                (TaskSettings, full type names) and the export format (.sxie)
//   probe.txt    font metrics Skia reports, for the text effects
//
// .sxrb: "SXRB", u32 width, u32 height, then width*height premultiplied BGRA
// pixels (what ShareX's SKBitmap holds; PNG would unpremultiply and round).

using System.Reflection;
using System.Runtime.InteropServices;
using System.Text;
using Newtonsoft.Json.Linq;
using ShareX.HelpersLib;
using ShareX.ImageEffectsLib;
using SkiaSharp;

static class Program
{
    static int Main(string[] args)
    {
        if (args.Length < 2)
        {
            Console.Error.WriteLine("usage: EffectsHarness <cases.json> <work dir>");
            return 2;
        }

        string casesPath = Path.GetFullPath(args[0]);
        string work = Path.GetFullPath(args[1]);
        string inputs = Path.Combine(work, "inputs");
        string outDir = Path.Combine(work, "sharex");
        string roundtripDir = Path.Combine(work, "roundtrip");
        string defaultsDir = Path.Combine(work, "defaults");
        foreach (string dir in new[] { inputs, outDir, roundtripDir, defaultsDir }) Directory.CreateDirectory(dir);

        Inputs.Generate(inputs);
        DumpDefaults(defaultsDir);
        Probe(Path.Combine(work, "probe.txt"));

        JArray cases = JArray.Parse(File.ReadAllText(casesPath));
        string only = args.Length > 2 ? args[2] : null;
        int failures = 0;
        foreach (JObject c in cases.Cast<JObject>())
        {
            string name = (string)c["name"];
            if (only != null && !name.StartsWith(only, StringComparison.Ordinal)) continue;
            try
            {
                RunCase(c, inputs, outDir, roundtripDir);
            }
            catch (Exception e)
            {
                failures++;
                Console.Error.WriteLine($"{name}: {e}");
            }
        }
        Console.WriteLine($"{cases.Count} cases, {failures} failed");
        return failures == 0 ? 0 : 1;
    }

    static readonly ImageEffectsSerializationBinder Binder = new();

    static void RunCase(JObject c, string inputs, string outDir, string roundtripDir)
    {
        string name = (string)c["name"];
        string input = (string)c["input"];
        int seed = (int?)c["seed"] ?? 1;

        JObject preset = new() { ["Name"] = (string)c["presetName"] ?? "", ["Effects"] = c["effects"].DeepClone() };
        string json = preset.ToString().Replace("{inputs}", inputs.Replace("\\", "\\\\"));

        SeedRandom(seed);
        ImageEffectPreset p = JsonHelpers.DeserializeFromString<ImageEffectPreset>(json, Binder);
        File.WriteAllText(Path.Combine(roundtripDir, name + ".json"),
            JsonHelpers.SerializeToString(p, serializationBinder: Binder).Replace(inputs, "{inputs}"));

        using SKBitmap source = Raw.Read(Path.Combine(inputs, input + ".sxrb"));
        SeedRandom(seed);
        using SKBitmap result = p.ApplyEffects(source);
        Raw.Write(result, Path.Combine(outDir, name + ".sxrb"));
        Raw.WritePng(result, Path.Combine(outDir, name + ".png"));
    }

    /// <summary>Makes RandomFast deterministic: swaps its Random's implementation for
    /// the one <c>new Random(seed)</c> uses (the .NET Framework compatible generator),
    /// which ShareXrs's validation mode reproduces.</summary>
    static void SeedRandom(int seed)
    {
        Random shared = (Random)typeof(RandomFast).GetField("random", BindingFlags.NonPublic | BindingFlags.Static)!.GetValue(null);
        FieldInfo impl = typeof(Random).GetField("_impl", BindingFlags.NonPublic | BindingFlags.Instance)!;
        impl.SetValue(shared, impl.GetValue(new Random(seed)));
    }

    static void DumpDefaults(string dir)
    {
        File.Delete(Path.Combine(dir, "names.txt"));
        foreach (ImageEffectDefinition definition in ImageEffectCatalog.All)
        {
            SeedRandom(1);
            ImageEffect effect = definition.Create();
            ImageEffectPreset preset = new() { Effects = { effect } };
            string type = definition.EffectType.Name;
            File.WriteAllText(Path.Combine(dir, type + ".json"), JsonHelpers.SerializeToString(preset));
            File.WriteAllText(Path.Combine(dir, type + ".sxie.json"), JsonHelpers.SerializeToString(preset, serializationBinder: Binder));
            File.AppendAllText(Path.Combine(dir, "names.txt"), $"{type}\t{definition.Category}\t{definition.Name}\t{effect}\n");
        }

        SeedRandom(1);
        ImageEffectPreset named = new() { Name = "Named", Effects = { new DrawBackground { Name = "My background", Enabled = false } } };
        File.WriteAllText(Path.Combine(dir, "named.json"), JsonHelpers.SerializeToString(named));
        File.WriteAllText(Path.Combine(dir, "default-preset.json"), JsonHelpers.SerializeToString(ImageEffectPreset.GetDefaultPreset()));
    }

    static void Probe(string path)
    {
        StringBuilder sb = new();
        foreach (string spec in new[] { "Arial, 11.25pt", "Arial, 36pt", "Arial, 20pt, style=Bold", "Arial, 24px, style=Italic", "Segoe UI, 14pt" })
        {
            ImageFont font = (ImageFont)new ImageFontConverter().ConvertFromInvariantString(spec);
            using SKFont sk = font.CreateFont();
            SKFontMetrics m = sk.Metrics;
            sb.AppendLine($"{spec}: family={sk.Typeface.FamilyName} size={sk.Size:R} ascent={m.Ascent:R} descent={m.Descent:R} leading={m.Leading:R} spacing={sk.Spacing:R} " +
                $"underline={m.UnderlinePosition:R}/{m.UnderlineThickness:R} strike={m.StrikeoutPosition:R}/{m.StrikeoutThickness:R} " +
                $"hinting={sk.Hinting} edging={sk.Edging} subpixel={sk.Subpixel} linear={sk.LinearMetrics}");
            foreach (string text in new[] { "Text watermark", "Text", "ShareX 1234", "AVAWAy" })
            {
                sb.AppendLine($"  measure '{text}' = {sk.MeasureText(text):R}");
            }
        }
        File.WriteAllText(path, sb.ToString());
    }
}

static class Raw
{
    public static SKBitmap Read(string path)
    {
        byte[] data = File.ReadAllBytes(path);
        if (Encoding.ASCII.GetString(data, 0, 4) != "SXRB") throw new InvalidDataException(path);
        int width = BitConverter.ToInt32(data, 4), height = BitConverter.ToInt32(data, 8);
        SKBitmap bitmap = new(new SKImageInfo(width, height, SKColorType.Bgra8888, SKAlphaType.Premul));
        Marshal.Copy(data, 12, bitmap.GetPixels(), width * height * 4);
        bitmap.NotifyPixelsChanged();
        return bitmap;
    }

    public static void Write(SKBitmap bitmap, string path)
    {
        SKImageInfo info = new(bitmap.Width, bitmap.Height, SKColorType.Bgra8888, SKAlphaType.Premul);
        byte[] pixels = new byte[info.BytesSize];
        GCHandle handle = GCHandle.Alloc(pixels, GCHandleType.Pinned);
        try
        {
            if (!bitmap.PeekPixels().ReadPixels(info, handle.AddrOfPinnedObject(), info.RowBytes)) throw new InvalidOperationException("ReadPixels");
        }
        finally { handle.Free(); }
        using FileStream stream = File.Create(path);
        stream.Write(Encoding.ASCII.GetBytes("SXRB"));
        stream.Write(BitConverter.GetBytes(bitmap.Width));
        stream.Write(BitConverter.GetBytes(bitmap.Height));
        stream.Write(pixels);
    }

    public static void WritePng(SKBitmap bitmap, string path)
    {
        using SKData data = bitmap.Encode(SKEncodedImageFormat.Png, 100);
        File.WriteAllBytes(path, data.ToArray());
    }

    /// <summary>Premultiplies as Skia does (SkOpts::RGBA_to_rgbA): (c*a+127)/255.</summary>
    public static void WriteStraight(string path, int width, int height, Func<int, int, (int r, int g, int b, int a)> pixel)
    {
        SKBitmap bitmap = new(new SKImageInfo(width, height, SKColorType.Bgra8888, SKAlphaType.Premul));
        byte[] pixels = new byte[width * height * 4];
        for (int y = 0; y < height; y++)
            for (int x = 0; x < width; x++)
            {
                (int r, int g, int b, int a) = pixel(x, y);
                int i = (y * width + x) * 4;
                pixels[i] = (byte)((b * a + 127) / 255);
                pixels[i + 1] = (byte)((g * a + 127) / 255);
                pixels[i + 2] = (byte)((r * a + 127) / 255);
                pixels[i + 3] = (byte)a;
            }
        Marshal.Copy(pixels, 0, bitmap.GetPixels(), pixels.Length);
        bitmap.NotifyPixelsChanged();
        Write(bitmap, path + ".sxrb");
        WritePng(bitmap, path + ".png");
        bitmap.Dispose();
    }

    /// <summary>A straight-alpha PNG file, for the effects that load images from disk.</summary>
    public static void WritePngFile(string path, int width, int height, Func<int, int, (int r, int g, int b, int a)> pixel)
    {
        using SKBitmap bitmap = new(new SKImageInfo(width, height, SKColorType.Rgba8888, SKAlphaType.Unpremul));
        for (int y = 0; y < height; y++)
            for (int x = 0; x < width; x++)
            {
                (int r, int g, int b, int a) = pixel(x, y);
                bitmap.SetPixel(x, y, new SKColor((byte)r, (byte)g, (byte)b, (byte)a));
            }
        using SKData data = bitmap.Encode(SKEncodedImageFormat.Png, 100);
        File.WriteAllBytes(path, data.ToArray());
    }
}

/// <summary>The test images: integer maths only, so they're the same everywhere.</summary>
static class Inputs
{
    sealed class Lcg
    {
        uint state;
        public Lcg(uint seed) => state = seed;
        public int Next() { state = state * 1103515245 + 12345; return (int)((state >> 16) & 0xFF); }
    }

    static int Clamp(int v) => Math.Clamp(v, 0, 255);

    public static void Generate(string dir)
    {
        // Opaque, photo-like: gradients, a soft disc, hard edges, pure white and black
        // blocks (Replace color, Auto crop), noise.
        Lcg noise = new(7);
        Raw.WriteStraight(Path.Combine(dir, "photo"), 157, 103, (x, y) =>
        {
            int n = noise.Next() / 8 - 16;
            if (x >= 10 && x < 30 && y >= 10 && y < 25) return (255, 255, 255, 255);
            if (x >= 120 && x < 140 && y >= 70 && y < 90) return (0, 0, 0, 255);
            int dx = x - 80, dy = y - 50, d2 = dx * dx + dy * dy;
            int disc = d2 < 900 ? 120 : d2 < 1600 ? 120 * (1600 - d2) / 700 : 0;
            int r = Clamp(x * 255 / 156 + n + disc / 2);
            int g = Clamp(y * 255 / 102 - disc / 3 + n);
            int b = Clamp(((x * 3 + y * 5) % 97) * 2 + 40 + disc + n);
            if ((x / 13 + y / 11) % 5 == 0) { r = 255 - r; g = 200; }
            return (r, g, b, 255);
        });

        // Transparency: an opaque centre fading to transparent, a half transparent
        // band, fully transparent corners.
        Lcg noise2 = new(11);
        Raw.WriteStraight(Path.Combine(dir, "alpha"), 121, 87, (x, y) =>
        {
            int n = noise2.Next() / 16;
            int dx = x - 60, dy = y - 43, d2 = dx * dx + dy * dy;
            int a = d2 <= 625 ? 255 : d2 >= 1600 ? 0 : 255 * (1600 - d2) / 975;
            if (y >= 5 && y < 15 && x >= 5) a = 128;
            int r = Clamp(x * 2 + n), g = Clamp(255 - y * 2 - n), b = Clamp((x ^ y) * 3 % 256);
            return (r, g, b, a);
        });

        Lcg random = new(3);
        Raw.WriteStraight(Path.Combine(dir, "tiny"), 9, 7, (x, y) =>
        {
            int a = random.Next();
            if (a < 40) a = 0; else if (a > 215) a = 255;
            return (random.Next(), random.Next(), random.Next(), a);
        });

        // Auto crop: a solid border around content, and a transparent one.
        Raw.WriteStraight(Path.Combine(dir, "crop-solid"), 64, 48, (x, y) =>
            x >= 13 && x < 43 && y >= 9 && y < 29 ? (x * 8 % 256, y * 12 % 256, 90, 255) : (40, 60, 80, 255));
        Raw.WriteStraight(Path.Combine(dir, "crop-alpha"), 64, 48, (x, y) =>
            x >= 7 && x < 48 && y >= 11 && y < 36 ? (200, x * 5 % 256, y * 7 % 256, 255) : (0, 0, 0, 0));

        Raw.WritePngFile(Path.Combine(dir, "watermark.png"), 40, 24, (x, y) =>
            (255 - x * 6, 60 + y * 8, 200, x < 4 || y < 3 ? 255 : Clamp(80 + x * 4 + y * 3)));
        Raw.WritePngFile(Path.Combine(dir, "background.png"), 50, 37, (x, y) =>
            ((x * 5) % 256, (y * 7) % 256, (x + y) * 3 % 256, 255));
        string particles = Path.Combine(dir, "particles");
        Directory.CreateDirectory(particles);
        Raw.WritePngFile(Path.Combine(particles, "a.png"), 12, 12, (x, y) =>
            (255, x * 20, y * 20, (x - 6) * (x - 6) + (y - 6) * (y - 6) <= 36 ? 255 : 0));
        Raw.WritePngFile(Path.Combine(particles, "b.png"), 16, 10, (x, y) => (40, 200, x * 15, 180));
    }
}
