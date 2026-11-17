// The harness's fixed test inputs, generated with integer maths so every run
// (and both ports) reads the same files: opaque, transparent and odd-sized
// images in the formats ShareX loads, EXIF-rotated JPEGs, a watermark logo.

using System.Runtime.InteropServices;
using ShareX.HelpersLib;
using SkiaSharp;

static class Inputs
{
    public static void Generate(string dir)
    {
        Directory.CreateDirectory(dir);
        Png(dir, "opaque_97x61.png", Pattern(97, 61, 0, false));
        Png(dir, "opaque_97x61_dots.png", Dots(Pattern(97, 61, 0, false), 7));
        Png(dir, "odd_33x77.png", Pattern(33, 77, 1, false));
        Png(dir, "wide_301x41.png", Pattern(301, 41, 2, false));
        Png(dir, "tall_23x157.png", Pattern(23, 157, 3, false));
        Png(dir, "small_5x3.png", Pattern(5, 3, 4, false));
        Png(dir, "tiny_1x1.png", Pattern(1, 1, 5, false));
        Png(dir, "big_420x300.png", Pattern(420, 300, 6, false));
        Png(dir, "big_420x300_dots.png", Dots(Pattern(420, 300, 6, false), 13));
        Png(dir, "huge_1900x1100.png", Pattern(1900, 1100, 7, false));
        Png(dir, "alpha_64x48.png", Pattern(64, 48, 8, true));
        Png(dir, "alpha_odd_45x29.png", Pattern(45, 29, 9, true));
        Png(dir, "logo_37x23.png", Logo(37, 23));

        using (SKBitmap photo = Pattern(97, 61, 0, false))
        using (SKData data = photo.Encode(SKEncodedImageFormat.Jpeg, 90))
            File.WriteAllBytes(Path.Combine(dir, "photo_q90.jpg"), data.ToArray());
        foreach (int orientation in new[] { 6, 3, 8 })
        {
            using SKBitmap src = Pattern(33, 20, 10, false);
            using SKData data = src.Encode(SKEncodedImageFormat.Jpeg, 95);
            File.WriteAllBytes(Path.Combine(dir, $"exif{orientation}.jpg"), InsertExif(data.ToArray(), orientation));
        }
        using (SKBitmap gifSource = Pattern(40, 30, 11, false))
        using (FileStream fs = File.Create(Path.Combine(dir, "palette.gif")))
            SkiaImageHelpers.SaveGIF(gifSource, fs, GIFQuality.Bit8);
        using (SKBitmap bmpSource = Pattern(31, 17, 12, false))
        using (FileStream fs = File.Create(Path.Combine(dir, "image24.bmp")))
            SkiaImageHelpers.SaveBMP(bmpSource, fs, BMPBitDepth.Bit24);
        using (SKBitmap webpSource = Pattern(29, 21, 13, true))
        using (SKPixmap pm = webpSource.PeekPixels())
        using (SKData data = pm.Encode(new SKWebpEncoderOptions(SKWebpEncoderCompression.Lossless, 75)))
            File.WriteAllBytes(Path.Combine(dir, "lossless.webp"), data.ToArray());
        using (SKBitmap gray = new(new SKImageInfo(26, 19, SKColorType.Gray8, SKAlphaType.Opaque)))
        {
            byte[] px = new byte[gray.ByteCount];
            for (int y = 0; y < 19; y++) for (int x = 0; x < 26; x++) px[y * gray.RowBytes + x] = (byte)((x * 9 + y * 13) % 256);
            Marshal.Copy(px, 0, gray.GetPixels(), px.Length);
            using SKPixmap pm = gray.PeekPixels();
            using SKData data = pm.Encode(SKPngEncoderOptions.Default);
            File.WriteAllBytes(Path.Combine(dir, "gray.png"), data.ToArray());
        }
        File.WriteAllBytes(Path.Combine(dir, "rgb16.png"), Png16(24, 18));
    }

    // Straight colour from position and seed; alpha too when requested (with
    // fully transparent and fully opaque runs).
    static SKBitmap Pattern(int w, int h, int seed, bool alpha)
    {
        SKBitmap bmp = new(new SKImageInfo(w, h, SKColorType.Bgra8888, alpha ? SKAlphaType.Unpremul : SKAlphaType.Premul));
        byte[] px = new byte[w * h * 4];
        for (int y = 0; y < h; y++)
            for (int x = 0; x < w; x++)
            {
                int i = (y * w + x) * 4;
                int r = (x * 7 + y * 3 + seed * 31) % 256;
                int g = (x * 2 + y * 11 + seed * 17) % 256;
                int b = ((x / 4 + y / 3) % 2 == 0 ? 200 : 40) + seed % 7;
                int a = 255;
                if (alpha)
                {
                    a = (x * 13 + y * 5 + seed) % 256;
                    if (x < 3) a = 0;
                    if (y < 2) a = 255;
                }
                px[i] = (byte)b; px[i + 1] = (byte)g; px[i + 2] = (byte)r; px[i + 3] = (byte)a;
            }
        Marshal.Copy(px, 0, bmp.GetPixels(), px.Length);
        return bmp;
    }

    static SKBitmap Dots(SKBitmap bmp, int step)
    {
        for (int y = 0; y < bmp.Height; y += step)
            for (int x = (y / step) % 3; x < bmp.Width; x += step)
                bmp.SetPixel(x, y, new SKColor(255, 0, 255));
        return bmp;
    }

    static SKBitmap Logo(int w, int h)
    {
        SKBitmap bmp = new(new SKImageInfo(w, h, SKColorType.Bgra8888, SKAlphaType.Unpremul));
        byte[] px = new byte[w * h * 4];
        for (int y = 0; y < h; y++)
            for (int x = 0; x < w; x++)
            {
                int i = (y * w + x) * 4;
                int dx = x * 2 - w, dy = y * 2 - h;
                bool inside = dx * dx * h * h + dy * dy * w * w <= w * w * h * h;
                px[i] = (byte)(x * 255 / w); px[i + 1] = 80; px[i + 2] = (byte)(y * 255 / h);
                px[i + 3] = (byte)(inside ? 160 + (x + y) % 96 : 0);
            }
        Marshal.Copy(px, 0, bmp.GetPixels(), px.Length);
        return bmp;
    }

    static void Png(string dir, string name, SKBitmap bmp)
    {
        using (bmp)
        using (SKPixmap pm = bmp.PeekPixels())
        using (SKData data = pm.Encode(SKPngEncoderOptions.Default))
            File.WriteAllBytes(Path.Combine(dir, name), data.ToArray());
    }

    // An APP1 Exif segment (little-endian TIFF, IFD0 with Orientation) after SOI.
    static byte[] InsertExif(byte[] jpeg, int orientation)
    {
        List<byte> tiff = [.. "II*\0"u8.ToArray(), 8, 0, 0, 0, 1, 0, 0x12, 0x01, 3, 0, 1, 0, 0, 0, (byte)orientation, 0, 0, 0, 0, 0, 0, 0];
        List<byte> seg = [.. "Exif\0\0"u8.ToArray(), .. tiff];
        List<byte> result = [0xFF, 0xD8, 0xFF, 0xE1, (byte)((seg.Count + 2) >> 8), (byte)((seg.Count + 2) & 0xFF), .. seg];
        result.AddRange(jpeg.Skip(2));
        return result.ToArray();
    }

    // A 16-bit RGBA PNG written by hand (unfiltered, stored deflate).
    static byte[] Png16(int w, int h)
    {
        using MemoryStream raw = new();
        for (int y = 0; y < h; y++)
        {
            raw.WriteByte(0);
            for (int x = 0; x < w; x++)
            {
                ushort r = (ushort)(x * 2731 + y * 17), g = (ushort)(y * 3637 + x * 5), b = (ushort)((x * y * 997) % 65536), a = (ushort)(65535 - x * 1500);
                foreach (ushort v in new[] { r, g, b, a }) { raw.WriteByte((byte)(v >> 8)); raw.WriteByte((byte)v); }
            }
        }
        using MemoryStream z = new();
        using (System.IO.Compression.ZLibStream zs = new(z, System.IO.Compression.CompressionLevel.Optimal, true)) raw.WriteTo(zs);
        using MemoryStream png = new();
        png.Write([137, 80, 78, 71, 13, 10, 26, 10]);
        byte[] ihdr = [.. BE(w), .. BE(h), 16, 6, 0, 0, 0];
        Chunk(png, "IHDR", ihdr);
        Chunk(png, "IDAT", z.ToArray());
        Chunk(png, "IEND", []);
        return png.ToArray();
    }

    static byte[] BE(int v) => [(byte)(v >> 24), (byte)(v >> 16), (byte)(v >> 8), (byte)v];

    static void Chunk(Stream s, string type, byte[] data)
    {
        s.Write(BE(data.Length));
        byte[] t = System.Text.Encoding.ASCII.GetBytes(type);
        s.Write(t); s.Write(data);
        uint crc = 0xFFFFFFFF;
        foreach (byte v in t.Concat(data)) { crc ^= v; for (int k = 0; k < 8; k++) crc = (crc & 1) != 0 ? 0xEDB88320 ^ (crc >> 1) : crc >> 1; }
        s.Write(BE((int)~crc));
    }
}
