// ShareX's pointer drawing (ShareX.HelpersLib's CursorData), headless: each
// system cursor drawn into a generated frame the way a capture draws the
// pointer, for crates/sharex-capture's cursor tests to compare with.
//
//     ToolsHarness --cursors <out dir>
//
// For each cursor (LoadCursor of a standard IDC_*), with its hotspot at
// (32, 32) of a 64x64 frame of the test pattern:
//   <name>-bitmap.png  CursorData.DrawCursor(SKBitmap, offset): the region
//                      capture's way (WindowsImageInterop.FromIcon over the
//                      background, drawn with SkiaSharp)
//   <name>-gdi.png     CursorData.DrawCursor(hdc, offset): a screenshot's way
//                      (DrawIconEx on the capture's DC)
//   background.png     the frame without the pointer

using System.Drawing;
using System.Reflection;
using System.Runtime.InteropServices;
using ShareX.HelpersLib;
using SkiaSharp;

static class CursorCases
{
    [DllImport("user32.dll")] static extern IntPtr LoadCursorW(IntPtr instance, IntPtr name);
    [DllImport("gdi32.dll")] static extern IntPtr CreateCompatibleDC(IntPtr dc);
    [DllImport("gdi32.dll")] static extern IntPtr CreateDIBSection(IntPtr dc, ref BITMAPINFOHEADER info, uint usage, out IntPtr bits, IntPtr section, uint offset);
    [DllImport("gdi32.dll")] static extern IntPtr SelectObject(IntPtr dc, IntPtr obj);
    [DllImport("gdi32.dll")] static extern bool DeleteObject(IntPtr obj);
    [DllImport("gdi32.dll")] static extern bool DeleteDC(IntPtr dc);
    [DllImport("gdi32.dll")] static extern bool GdiFlush();

    [StructLayout(LayoutKind.Sequential)]
    struct BITMAPINFOHEADER
    {
        public uint biSize; public int biWidth, biHeight; public ushort biPlanes, biBitCount;
        public uint biCompression, biSizeImage; public int biXPelsPerMeter, biYPelsPerMeter; public uint biClrUsed, biClrImportant;
    }

    static readonly (string Name, int Id)[] Cursors =
    [
        ("arrow", 32512), ("ibeam", 32513), ("cross", 32515), ("hand", 32649), ("sizeall", 32646), ("no", 32648)
    ];

    const int Size = 64;

    // sharexrs --test-pattern's picture (crates/sharexrs/src/test_pattern.rs).
    static SKColor Pattern(int x, int y)
    {
        if (x % 64 == 0 || y % 64 == 0) return new SKColor(255, 255, 255);
        bool checker = ((x / 16) + (y / 16)) % 2 == 0;
        return new SKColor((byte)(x % 256), (byte)(y % 256), (byte)(checker ? 200 : 70));
    }

    static SKBitmap Background()
    {
        SKBitmap b = new(new SKImageInfo(Size, Size, SKColorType.Bgra8888, SKAlphaType.Premul));
        for (int y = 0; y < Size; y++) for (int x = 0; x < Size; x++) b.SetPixel(x, y, Pattern(x + 100, y + 100));
        return b;
    }

    static void Set(object target, string property, object value) =>
        target.GetType().GetProperty(property, BindingFlags.Public | BindingFlags.Instance)!.SetValue(target, value);

    public static int Run(string outDir)
    {
        Directory.CreateDirectory(outDir);
        using (SKBitmap bg = Background()) Save(bg, Path.Combine(outDir, "background.png"));
        foreach ((string name, int id) in Cursors)
        {
            IntPtr handle = LoadCursorW(IntPtr.Zero, id);
            // A CursorData for this cursor at screen (32, 32): what
            // UpdateCursorData reads from GetCursorInfo, set directly.
            CursorData data = new();
            Set(data, "Handle", handle);
            Set(data, "Position", new Point(32, 32));
            Set(data, "IsVisible", true);
            Set(data, "SizeMultiplier", CursorData.GetCursorSizeMultiplier());
            Set(data, "Size", System.Drawing.Size.Empty);
            NativeMethods.GetIconInfo(handle, out IconInfo info);
            Set(data, "Hotspot", new Point(info.xHotspot, info.yHotspot));
            if (info.hbmColor != IntPtr.Zero) DeleteObject(info.hbmColor);
            if (info.hbmMask != IntPtr.Zero) DeleteObject(info.hbmMask);
            Console.WriteLine($"{name}: hotspot {info.xHotspot},{info.yHotspot}");

            using (SKBitmap bmp = Background())
            {
                data.DrawCursor(bmp, Point.Empty);
                Save(bmp, Path.Combine(outDir, $"{name}-bitmap.png"));
            }

            // DrawCursor(hdc): DrawIconEx on a DC holding the frame.
            IntPtr dc = CreateCompatibleDC(IntPtr.Zero);
            BITMAPINFOHEADER bi = new() { biSize = 40, biWidth = Size, biHeight = -Size, biPlanes = 1, biBitCount = 32 };
            IntPtr dib = CreateDIBSection(dc, ref bi, 0, out IntPtr bits, IntPtr.Zero, 0);
            IntPtr old = SelectObject(dc, dib);
            using (SKBitmap bg = Background())
            {
                Marshal.Copy(bg.Bytes, 0, bits, Size * Size * 4);
            }
            data.DrawCursor(dc, Point.Empty);
            GdiFlush();
            byte[] raw = new byte[Size * Size * 4];
            Marshal.Copy(bits, raw, 0, raw.Length);
            SelectObject(dc, old);
            DeleteObject(dib);
            DeleteDC(dc);
            using (SKBitmap gdi = new(new SKImageInfo(Size, Size, SKColorType.Bgra8888, SKAlphaType.Premul)))
            {
                // A screen capture is opaque (WindowsImageInterop.Capture sets alpha 255).
                for (int i = 3; i < raw.Length; i += 4) raw[i] = 255;
                Marshal.Copy(raw, 0, gdi.GetPixels(), raw.Length);
                Save(gdi, Path.Combine(outDir, $"{name}-gdi.png"));
            }
        }
        return 0;
    }

    static void Save(SKBitmap bitmap, string path)
    {
        using SKData data = bitmap.Encode(SKEncodedImageFormat.Png, 100);
        File.WriteAllBytes(path, data.ToArray());
    }
}
