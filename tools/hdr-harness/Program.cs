// Drives ShareX's own HDR screenshot correction (ShareX.ScreenCaptureLib's
// internal HDRScreenCapture, by reflection) over synthetic HDR frames and
// writes what it produces, for crates/sharex-capture's hdr tests. Nothing is
// captured: the frames are made here (FP16 scRGB and HDR10 PQ, as DXGI
// desktop duplication delivers them once mapped), and so is the "GDI"
// capture they correct.
//
//   HdrHarness <out dir>
//
// Per case (all pixels BGRA, rows packed):
//   <case>.frame     the mapped frame (RowPitch * Height bytes)
//   <case>.gdi       the GDI capture of the capture rectangle, before
//   <case>.srgb      ConvertToSrgb(frame, white, peak)
//   <case>.ref       ConvertToSdrReference(frame, white)
//   <case>.refrot    the reference after RotateOutput
//   <case>.wic       ConvertWithWindowsToneMapper(frame) (or .wic.error.txt)
//   <case>.wicrot    that after RotateOutput
//   <case>.final     the GDI capture after CaptureOutput's steps (Windows'
//                    tone mapper, the reference, RotateOutput, the size
//                    check) and CopyOutputIntersection (or .final.null when
//                    CaptureOutput gives nothing)
//   <case>.fallback  the same when Windows' tone mapper throws (ConvertToSrgb)
// and cases.json describing them.

using System.Drawing;
using System.Reflection;
using System.Runtime.InteropServices;
using System.Text.Json;
using ShareX.ScreenCaptureLib;
using SkiaSharp;
using Vortice.Direct3D11;
using Vortice.DXGI;

static unsafe class Program
{
    const BindingFlags All = BindingFlags.Instance | BindingFlags.Static | BindingFlags.Public | BindingFlags.NonPublic;
    static readonly Type Hdr = typeof(Screenshot).Assembly.GetType("ShareX.ScreenCaptureLib.HDRScreenCapture", true);
    static object Call(string name, params object[] args)
    {
        try { return Hdr.GetMethods(All).Single(m => m.Name == name && m.GetParameters().Length == args.Length).Invoke(null, args); }
        catch (TargetInvocationException e) { throw e.InnerException; }
    }

    record Case(string Name, string Format, int Width, int Height, int RowPitch, float White, float Peak, int Rotation,
        int[] Bounds, int[] Capture, string Pattern);

    static int Main(string[] args)
    {
        if (args.Length != 1) { Console.Error.WriteLine("usage: HdrHarness <out dir>"); return 2; }
        string outDir = args[0];
        Directory.CreateDirectory(outDir);
        List<Case> cases =
        [
            new("fp16-ramp", "fp16", 64, 8, 64 * 8, 1f, 1000f, 1, [0, 0, 64, 8], [0, 0, 64, 8], "ramp"),
            new("fp16-ramp-white25", "fp16", 64, 8, 64 * 8, 2.5f, 600f, 1, [0, 0, 64, 8], [0, 0, 64, 8], "ramp"),
            new("fp16-ramp-peak-floor", "fp16", 64, 8, 64 * 8, 125f, 1000f, 1, [0, 0, 64, 8], [0, 0, 64, 8], "ramp"),
            new("fp16-ramp-peak-ceiling", "fp16", 64, 8, 64 * 8, 1f, 100000f, 1, [0, 0, 64, 8], [0, 0, 64, 8], "ramp"),
            new("fp16-special", "fp16", 16, 4, 16 * 8, 1f, 1000f, 1, [0, 0, 16, 4], [0, 0, 16, 4], "special"),
            new("fp16-colors", "fp16", 16, 4, 16 * 8, 1.5f, 800f, 1, [0, 0, 16, 4], [0, 0, 16, 4], "colors"),
            new("fp16-pitch", "fp16", 37, 5, 37 * 8 + 24, 1f, 1000f, 1, [0, 0, 37, 5], [0, 0, 37, 5], "ramp"),
            new("fp16-rot90", "fp16", 7, 5, 7 * 8, 1f, 1000f, 2, [-30, 4, 5, 7], [-33, 2, 6, 12], "alpha"),
            new("fp16-rot180", "fp16", 7, 5, 7 * 8, 1f, 1000f, 3, [-30, 4, 7, 5], [-33, 2, 12, 6], "alpha"),
            new("fp16-rot270", "fp16", 7, 5, 7 * 8, 2f, 1000f, 4, [-30, 4, 5, 7], [-33, 2, 6, 12], "alpha"),
            new("fp16-colors-rot90", "fp16", 16, 4, 16 * 8, 1f, 1000f, 2, [0, 0, 4, 16], [0, 0, 4, 16], "colors"),
            new("fp16-colors-rot270", "fp16", 16, 4, 16 * 8, 1f, 1000f, 4, [0, 0, 4, 16], [0, 0, 4, 16], "colors"),
            new("fp16-unspecified", "fp16", 16, 4, 16 * 8, 1f, 1000f, 0, [0, 0, 16, 4], [0, 0, 16, 4], "colors"),
            new("fp16-offset", "fp16", 32, 16, 32 * 8, 1f, 1000f, 1, [100, 50, 32, 16], [90, 60, 30, 20], "ramp"),
            new("fp16-inside", "fp16", 32, 16, 32 * 8, 1f, 1000f, 1, [0, 0, 32, 16], [4, 3, 10, 8], "colors"),
            new("fp16-size-mismatch", "fp16", 16, 4, 16 * 8, 1f, 1000f, 1, [0, 0, 16, 5], [0, 0, 16, 4], "colors"),
            new("hdr10-codes", "hdr10", 64, 16, 64 * 4, 1f, 1000f, 1, [0, 0, 64, 16], [0, 0, 64, 16], "codes"),
            new("hdr10-white2-rot90", "hdr10", 32, 8, 32 * 4, 2f, 1000f, 2, [0, 0, 8, 32], [2, 1, 8, 30], "codes"),
            new("hdr10-pitch", "hdr10", 13, 3, 13 * 4 + 12, 3f, 400f, 1, [0, 0, 13, 3], [0, 0, 13, 3], "codes"),
        ];

        foreach (Case c in cases)
        {
            byte[] frame = MakeFrame(c);
            File.WriteAllBytes(Path.Combine(outDir, c.Name + ".frame"), frame);
            IntPtr buffer = Marshal.AllocHGlobal(frame.Length);
            try
            {
                Marshal.Copy(frame, 0, buffer, frame.Length);
                MappedSubresource mapped = new() { DataPointer = buffer, RowPitch = (uint)c.RowPitch, DepthPitch = (uint)frame.Length };
                Texture2DDescription desc = new()
                {
                    Width = (uint)c.Width, Height = (uint)c.Height, MipLevels = 1, ArraySize = 1,
                    Format = c.Format == "fp16" ? Format.R16G16B16A16_Float : Format.R10G10B10A2_UNorm,
                    SampleDescription = new SampleDescription(1, 0),
                };
                ModeRotation rotation = (ModeRotation)c.Rotation;
                Rectangle bounds = new(c.Bounds[0], c.Bounds[1], c.Bounds[2], c.Bounds[3]);
                Rectangle capture = new(c.Capture[0], c.Capture[1], c.Capture[2], c.Capture[3]);

                using (SKBitmap srgb = (SKBitmap)Call("ConvertToSrgb", mapped, desc, c.White, c.Peak))
                    File.WriteAllBytes(Path.Combine(outDir, c.Name + ".srgb"), srgb.Bytes);
                using (SKBitmap reference = (SKBitmap)Call("ConvertToSdrReference", mapped, desc, c.White))
                {
                    File.WriteAllBytes(Path.Combine(outDir, c.Name + ".ref"), reference.Bytes);
                    Call("RotateOutput", reference, rotation);
                    File.WriteAllBytes(Path.Combine(outDir, c.Name + ".refrot"), reference.Bytes);
                }
                string wicError = Path.Combine(outDir, c.Name + ".wic.error.txt");
                File.Delete(wicError);
                try
                {
                    using SKBitmap wic = (SKBitmap)Call("ConvertWithWindowsToneMapper", mapped, desc);
                    File.WriteAllBytes(Path.Combine(outDir, c.Name + ".wic"), wic.Bytes);
                    Call("RotateOutput", wic, rotation);
                    File.WriteAllBytes(Path.Combine(outDir, c.Name + ".wicrot"), wic.Bytes);
                }
                catch (Exception e) { File.WriteAllText(wicError, e.ToString()); }

                byte[] gdi = MakeGdi(c, mapped, desc, rotation, bounds, capture);
                File.WriteAllBytes(Path.Combine(outDir, c.Name + ".gdi"), gdi);
                Pipeline(outDir, c.Name + ".final", true, mapped, desc, c, rotation, bounds, capture, gdi);
                Pipeline(outDir, c.Name + ".fallback", false, mapped, desc, c, rotation, bounds, capture, gdi);
                Console.WriteLine($"{c.Name}: ok");
            }
            finally { Marshal.FreeHGlobal(buffer); }
        }
        File.WriteAllText(Path.Combine(outDir, "cases.json"), JsonSerializer.Serialize(cases, new JsonSerializerOptions { WriteIndented = true }));
        return 0;
    }

    // CaptureOutput after Map, then ApplyAdapterColorCorrection's
    // CopyOutputIntersection, with ShareX's own methods; `useWic: false` is
    // the catch branch (Windows' tone mapper threw).
    static void Pipeline(string outDir, string file, bool useWic, MappedSubresource mapped, Texture2DDescription desc, Case c,
        ModeRotation rotation, Rectangle bounds, Rectangle capture, byte[] gdi)
    {
        SKBitmap toneMapped = null;
        SKBitmap reference = null;
        try
        {
            if (!useWic) throw new NotSupportedException("no tone mapper");
            toneMapped = (SKBitmap)Call("ConvertWithWindowsToneMapper", mapped, desc);
            reference = (SKBitmap)Call("ConvertToSdrReference", mapped, desc, c.White);
        }
        catch
        {
            toneMapped?.Dispose();
            reference?.Dispose();
            reference = null;
            toneMapped = (SKBitmap)Call("ConvertToSrgb", mapped, desc, c.White, c.Peak);
        }
        Call("RotateOutput", toneMapped, rotation);
        if (reference != null) Call("RotateOutput", reference, rotation);
        File.Delete(Path.Combine(outDir, file + ".null"));
        if (toneMapped.Width != bounds.Width || toneMapped.Height != bounds.Height)
        {
            toneMapped.Dispose();
            reference?.Dispose();
            File.WriteAllText(Path.Combine(outDir, file + ".null"), "CaptureOutput returned null");
            File.WriteAllBytes(Path.Combine(outDir, file), gdi);
            return;
        }
        Type capturedType = Hdr.GetNestedType("CapturedOutput", All);
        using IDisposable captured = (IDisposable)Activator.CreateInstance(capturedType, All, null, [toneMapped, reference], null);
        using SKBitmap destination = FromBytes(gdi, capture.Width, capture.Height);
        Call("CopyOutputIntersection", destination, capture, captured, bounds);
        File.WriteAllBytes(Path.Combine(outDir, file), destination.Bytes);
    }

    static SKBitmap FromBytes(byte[] bgra, int width, int height)
    {
        SKBitmap b = new(new SKImageInfo(width, height, SKColorType.Bgra8888, SKAlphaType.Premul));
        Marshal.Copy(bgra, 0, b.GetPixels(), bgra.Length);
        b.NotifyPixelsChanged();
        return b;
    }

    // The GDI capture: inside the output, the (turned) SDR reference with a
    // per-channel offset of -7..7, so both sides of the 4-level tolerance
    // show; elsewhere a pattern. Opaque, as WindowsImageInterop.Capture is.
    static byte[] MakeGdi(Case c, MappedSubresource mapped, Texture2DDescription desc, ModeRotation rotation, Rectangle bounds, Rectangle capture)
    {
        using SKBitmap reference = (SKBitmap)Call("ConvertToSdrReference", mapped, desc, c.White);
        Call("RotateOutput", reference, rotation);
        byte[] r = reference.Bytes;
        byte[] gdi = new byte[capture.Width * capture.Height * 4];
        for (int y = 0; y < capture.Height; y++)
        {
            for (int x = 0; x < capture.Width; x++)
            {
                int i = (y * capture.Width + x) * 4;
                int sx = capture.X + x - bounds.X, sy = capture.Y + y - bounds.Y;
                bool inside = sx >= 0 && sy >= 0 && sx < Math.Min(bounds.Width, reference.Width) && sy < Math.Min(bounds.Height, reference.Height);
                for (int ch = 0; ch < 3; ch++)
                {
                    if (inside)
                    {
                        int delta = ((x * 7 + y * 13 + ch * 5) % 15) - 7;
                        gdi[i + ch] = (byte)Math.Clamp(r[(sy * reference.Width + sx) * 4 + ch] + delta, 0, 255);
                    }
                    else
                    {
                        gdi[i + ch] = (byte)((x * 31 + y * 17 + ch * 70) & 255);
                    }
                }
                gdi[i + 3] = 255;
            }
        }
        return gdi;
    }

    static readonly ushort[] Specials =
    [
        0x7E00, 0x7C00, 0xFC00, 0x8000, 0x0000, 0x0001, 0x03FF, 0x0400, 0x3BFF, 0x3C00, 0x3C01, 0x4000, 0x4900, 0x5640, 0x7BFF, 0xBC00,
        0x3555, 0x2E66, 0x1C00, 0xFE00, 0x7D00, 0x4500, 0x3E00, 0x3A00,
    ];

    static ushort H(float v) => BitConverter.HalfToUInt16Bits((Half)v);

    static byte[] MakeFrame(Case c)
    {
        byte[] data = new byte[c.RowPitch * c.Height];
        // Padding past each row's pixels: garbage the conversion must skip.
        for (int i = 0; i < data.Length; i++) data[i] = (byte)(i * 97 + 13);
        for (int y = 0; y < c.Height; y++)
        {
            for (int x = 0; x < c.Width; x++)
            {
                int n = y * c.Width + x;
                int at = y * c.RowPitch + x * (c.Format == "fp16" ? 8 : 4);
                if (c.Format == "fp16")
                {
                    ushort[] px = new ushort[4];
                    for (int ch = 0; ch < 3; ch++)
                    {
                        px[ch] = c.Pattern switch
                        {
                            "ramp" => H(((float)x / (c.Width - 1) * 12f - 0.5f + ch * 0.3f) * (1f + y / 4f)),
                            "special" => Specials[(n * 3 + ch * 7) % Specials.Length],
                            "colors" => H(Colors[(n + ch * 0) % Colors.Length][ch]),
                            "alpha" => H(((n * 5 + ch * 3) % 11) * 0.75f - 0.5f),
                            _ => throw new ArgumentException(c.Pattern),
                        };
                    }
                    px[3] = c.Pattern == "alpha" ? H(new[] { 1f, 0.5f, 0f, 0.25f, 1f }[n % 5]) : H(1f);
                    for (int ch = 0; ch < 4; ch++) BitConverter.TryWriteBytes(data.AsSpan(at + ch * 2, 2), px[ch]);
                }
                else
                {
                    uint r = (uint)(n % 1024), g = (uint)((n * 37 + y) % 1024), b = (uint)((n * 101 + 7) % 1024), a = (uint)(n % 4);
                    BitConverter.TryWriteBytes(data.AsSpan(at, 4), r | (g << 10) | (b << 20) | (a << 30));
                }
            }
        }
        return data;
    }

    static readonly float[][] Colors =
    [
        [8f, 0f, 0f], [0f, 8f, 0f], [0f, 0f, 8f], [4f, 4f, 0f], [2f, 0.1f, 0.1f], [0.2f, 0.4f, 0.6f], [1f, 1f, 1f], [1.2f, 1.1f, 1f],
        [30f, 20f, 10f], [0.9f, 1.5f, 0.3f], [-0.2f, 0.5f, 3f], [0.001f, 0.002f, 0.003f], [60f, 60f, 60f], [5f, -1f, 2f], [0.5f, 0.5f, 0.5f], [1.01f, 0f, 0f],
    ];
}
