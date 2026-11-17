// Files with metadata for the Metadata tool's cases: JPEG/PNG/GIF/TIFF/WebP
// assembled segment by segment (every kind the readers handle), BMP/ICO from
// the other inputs, and videos written by ShareX's ffmpeg.exe with tags.

using System.Diagnostics;
using System.IO.Compression;
using System.Text;

static class MetaInputs
{
    static byte[] BE16(int v) => [(byte)(v >> 8), (byte)v];
    static byte[] BE32(long v) => [(byte)(v >> 24), (byte)(v >> 16), (byte)(v >> 8), (byte)v];
    static byte[] LE16(int v) => [(byte)v, (byte)(v >> 8)];
    static byte[] LE32(long v) => [(byte)v, (byte)(v >> 8), (byte)(v >> 16), (byte)(v >> 24)];
    static byte[] A(string s) => Encoding.ASCII.GetBytes(s);
    static byte[] U8(string s) => Encoding.UTF8.GetBytes(s);

    static byte[] Cat(params byte[][] parts) => parts.SelectMany(p => p).ToArray();

    // A little TIFF writer: IFD entries (tag, type, count, value bytes).
    sealed class Ifd
    {
        public readonly List<(ushort Tag, ushort Type, uint Count, byte[] Value)> Entries = [];
        public Ifd Add(ushort tag, ushort type, uint count, byte[] value) { Entries.Add((tag, type, count, value)); return this; }
    }

    static byte[] Tiff(bool le, Ifd root, Ifd exif, Ifd gps, Ifd thumb)
    {
        byte[] U16(int v) => le ? LE16(v) : BE16(v);
        byte[] U32(long v) => le ? LE32(v) : BE32(v);
        List<byte> data = [.. (le ? A("II") : A("MM")), .. U16(42), .. U32(8)];
        // Layout: IFDs first, values after.
        List<(Ifd Ifd, int Offset)> ifds = [];
        int offset = 8;
        foreach (Ifd ifd in new[] { root, exif, gps, thumb }.Where(x => x != null))
        {
            ifds.Add((ifd, offset));
            offset += 2 + ifd.Entries.Count * 12 + 4;
        }
        int valueOffset = offset;
        int Pointer(Ifd x) => ifds.First(i => i.Ifd == x).Offset;
        byte[] Fix(Ifd ifd, (ushort Tag, ushort Type, uint Count, byte[] Value) e)
        {
            if (e.Tag == 0x8769 && exif != null) return U32(Pointer(exif));
            if (e.Tag == 0x8825 && gps != null) return U32(Pointer(gps));
            return e.Value;
        }
        List<byte> values = [];
        foreach ((Ifd ifd, int at) in ifds)
        {
            data.AddRange(U16(ifd.Entries.Count));
            foreach (var e in ifd.Entries)
            {
                byte[] v = Fix(ifd, e);
                data.AddRange(U16(e.Tag)); data.AddRange(U16(e.Type)); data.AddRange(U32(e.Count));
                if (v.Length <= 4) { data.AddRange(v); data.AddRange(new byte[4 - v.Length]); }
                else { data.AddRange(U32(valueOffset + values.Count)); values.AddRange(v); if ((values.Count & 1) != 0) values.Add(0); }
            }
            data.AddRange(U32(ifd == root && thumb != null ? Pointer(thumb) : 0));
        }
        data.AddRange(values);
        return data.ToArray();
    }

    static byte[] Rational(bool le, params (uint N, uint D)[] v) => v.SelectMany(x => Cat(le ? LE32(x.N) : BE32(x.N), le ? LE32(x.D) : BE32(x.D))).ToArray();
    static byte[] SRational(bool le, params (int N, int D)[] v) => v.SelectMany(x => Cat(le ? LE32(x.N) : BE32(x.N), le ? LE32(x.D) : BE32(x.D))).ToArray();
    static byte[] Short(bool le, params int[] v) => v.SelectMany(x => le ? LE16(x) : BE16(x)).ToArray();
    static byte[] Str(string s) => [.. A(s), 0];

    static byte[] ExifTiff(bool le)
    {
        Ifd root = new Ifd()
            .Add(0x010F, 2, 6, Str("Canon"))
            .Add(0x0110, 2, 13, Str("EOS R5 Mk II"))
            .Add(0x0112, 3, 1, Short(le, 6))
            .Add(0x011A, 5, 1, Rational(le, (300, 1)))
            .Add(0x011B, 5, 1, Rational(le, (72, 1)))
            .Add(0x0128, 3, 1, Short(le, 2))
            .Add(0x0131, 2, 8, Str("ShareXé"))
            .Add(0x0132, 2, 20, Str("2024:05:06 07:08:09"))
            .Add(0x9C9B, 1, 12, [.. Encoding.Unicode.GetBytes("Title"), 0, 0])
            .Add(0x8769, 4, 1, new byte[4])
            .Add(0x8825, 4, 1, new byte[4]);
        Ifd exif = new Ifd()
            .Add(0x829A, 5, 1, Rational(le, (1, 250)))
            .Add(0x829D, 5, 1, Rational(le, (28, 10)))
            .Add(0x8822, 3, 1, Short(le, 3))
            .Add(0x8827, 3, 2, Short(le, 400, 800))
            .Add(0x9000, 7, 4, A("0232"))
            .Add(0x9003, 2, 20, Str("2024:05:06 07:08:09"))
            .Add(0x9101, 7, 4, [1, 2, 3, 0])
            .Add(0x9201, 10, 1, SRational(le, (7965784, 1000000)))
            .Add(0x9202, 5, 1, Rational(le, (2970854, 1000000)))
            .Add(0x9204, 10, 1, SRational(le, (-2, 3)))
            .Add(0x9207, 3, 1, Short(le, 5))
            .Add(0x9208, 3, 1, Short(le, 21))
            .Add(0x9209, 3, 1, Short(le, 0x19))
            .Add(0x920A, 5, 1, Rational(le, (50, 1)))
            .Add(0x927C, 7, 40, new byte[40])
            .Add(0x9286, 7, 18, [.. A("UNICODE\0"), .. Encoding.BigEndianUnicode.GetBytes("Hi é")])
            .Add(0xA000, 7, 4, A("0100"))
            .Add(0xA001, 3, 1, Short(le, 65535))
            .Add(0xA002, 4, 1, le ? LE32(6000) : BE32(6000))
            .Add(0xA402, 3, 1, Short(le, 1))
            .Add(0xA405, 3, 1, Short(le, 35))
            .Add(0xA406, 3, 1, Short(le, 2))
            .Add(0xA432, 5, 4, Rational(le, (24, 1), (70, 1), (28, 10), (28, 10)))
            .Add(0xA434, 2, 15, Str("RF24-70mm F2.8"))
            .Add(0x0001, 3, 1, Short(le, 1));
        Ifd gps = new Ifd()
            .Add(0x0000, 1, 4, [2, 3, 0, 0])
            .Add(0x0001, 2, 2, Str("S"))
            .Add(0x0002, 5, 3, Rational(le, (33, 1), (51, 1), (5432, 100)))
            .Add(0x0003, 2, 2, Str("E"))
            .Add(0x0004, 5, 3, Rational(le, (151, 1), (12, 1), (4048, 100)))
            .Add(0x0006, 5, 1, Rational(le, (12345, 100)))
            .Add(0x0007, 5, 3, Rational(le, (7, 1), (8, 1), (9500, 1000)))
            .Add(0x000D, 5, 1, Rational(le, (0, 0)))
            .Add(0x0011, 5, 1, Rational(le, (27150, 100)))
            .Add(0x001D, 2, 11, Str("2024:05:06"));
        Ifd thumb = new Ifd().Add(0x0103, 3, 1, Short(le, 6)).Add(0x0201, 4, 1, le ? LE32(1234) : BE32(1234));
        return Tiff(le, root, exif, gps, thumb);
    }

    static byte[] Xmp => U8("<?xpacket begin='' id='W5M0MpCehiHzreSzNTczkc9d'?><x:xmpmeta xmlns:x='adobe:ns:meta/'><rdf:RDF xmlns:rdf='http://www.w3.org/1999/02/22-rdf-syntax-ns#'>" +
        "<rdf:Description rdf:about='' xmlns:dc='http://purl.org/dc/elements/1.1/' xmlns:xmp='http://ns.adobe.com/xap/1.0/' xmlns:photoshop='http://ns.adobe.com/photoshop/1.0/' xmp:CreatorTool='ShareX 22.4' photoshop:City='Sydney'>" +
        "<dc:title><rdf:Alt><rdf:li xml:lang='x-default'>A title</rdf:li><rdf:li xml:lang='fr'>Un titre</rdf:li></rdf:Alt></dc:title>" +
        "<dc:subject><rdf:Bag><rdf:li>one</rdf:li><rdf:li>two &amp; three</rdf:li></rdf:Bag></dc:subject><xmp:Rating>4</xmp:Rating><custom xmlns='urn:x'>  spaced\n value </custom>" +
        "</rdf:Description></rdf:RDF></x:xmpmeta><?xpacket end='w'?>");

    static byte[] Icc()
    {
        byte[] p = new byte[132 + 12 * 3 + 96];
        Array.Copy(BE32(p.Length), p, 4); Array.Copy(A("lcms"), 0, p, 4, 4); p[8] = 0x43; p[9] = 0x20;
        Array.Copy(A("mntr"), 0, p, 12, 4); Array.Copy(A("RGB "), 0, p, 16, 4); Array.Copy(A("XYZ "), 0, p, 20, 4);
        int[] date = [2023, 2, 28, 13, 14, 15]; for (int i = 0; i < 6; i++) Array.Copy(BE16(date[i]), 0, p, 24 + i * 2, 2);
        Array.Copy(A("acsp"), 0, p, 36, 4); Array.Copy(A("MSFT"), 0, p, 40, 4); Array.Copy(A("SHRX"), 0, p, 48, 4);
        Array.Copy(BE32(1), 0, p, 64, 4); Array.Copy(A("shx "), 0, p, 80, 4);
        Array.Copy(BE32(3), 0, p, 128, 4);
        int at = 168;
        void Tag(int i, string sig, byte[] body) { Array.Copy(A(sig), 0, p, 132 + i * 12, 4); Array.Copy(BE32(at), 0, p, 136 + i * 12, 4); Array.Copy(BE32(body.Length), 0, p, 140 + i * 12, 4); Array.Copy(body, 0, p, at, body.Length); at += body.Length; }
        Tag(0, "desc", [.. A("desc"), 0, 0, 0, 0, .. BE32(9), .. A("Test sRGB")]);
        Tag(1, "cprt", [.. A("text"), 0, 0, 0, 0, .. A("No copyright\0")]);
        Tag(2, "dmnd", [.. A("mluc"), 0, 0, 0, 0, .. BE32(1), .. BE32(12), .. A("enUS"), .. BE32(10), .. BE32(28), .. Encoding.BigEndianUnicode.GetBytes("Maker")]);
        return p;
    }

    static byte[] JpegSegment(int marker, byte[] data) => [0xFF, (byte)marker, .. BE16(data.Length + 2), .. data];

    static byte[] Jpeg(byte[] baseline, int sof, params byte[][] segments)
    {
        // Keep SOI, insert the segments, then the rest of a real JPEG (from its first DQT).
        int dqt = 2;
        while (!(baseline[dqt] == 0xFF && baseline[dqt + 1] == 0xDB)) dqt++;
        byte[] rest = baseline[dqt..];
        if (sof != 0xC0) for (int i = 0; i < rest.Length - 1; i++) if (rest[i] == 0xFF && rest[i + 1] == 0xC0) { rest[i + 1] = (byte)sof; break; }
        return [0xFF, 0xD8, .. segments.SelectMany(s => s), .. rest];
    }

    static byte[] PngChunk(string type, byte[] data)
    {
        byte[] t = A(type);
        uint crc = 0xFFFFFFFF;
        foreach (byte v in t.Concat(data)) { crc ^= v; for (int k = 0; k < 8; k++) crc = (crc & 1) != 0 ? 0xEDB88320 ^ (crc >> 1) : crc >> 1; }
        return [.. BE32(data.Length), .. t, .. data, .. BE32(~crc)];
    }

    static byte[] Zlib(byte[] data)
    {
        using MemoryStream m = new();
        using (ZLibStream z = new(m, CompressionLevel.Optimal, true)) z.Write(data);
        return m.ToArray();
    }

    static byte[] RiffChunk(string type, byte[] data) => [.. A(type), .. LE32(data.Length), .. data, .. (data.Length % 2 == 1 ? new byte[] { 0 } : [])];

    public static void Generate(string dir, string inputs, string ffmpeg)
    {
        Directory.CreateDirectory(dir);
        byte[] baseline = File.ReadAllBytes(Path.Combine(inputs, "photo_q90.jpg"));
        byte[] iptc = [0x1C, 2, 5, .. BE16(9), .. A("Obj name\0"), 0x1C, 2, 25, .. BE16(3), .. A("key"), 0x1C, 2, 120, .. BE16(5), .. U8("Capté")[..5], 0x1C, 1, 90, .. BE16(3), 0x1B, 0x25, 0x47];
        byte[] photoshop = [.. A("Photoshop 3.0\0"), .. A("8BIM"), .. BE16(0x0404), 0, 0, .. BE32(iptc.Length), .. iptc, .. (iptc.Length % 2 == 1 ? new byte[] { 0 } : []), .. A("8BIM"), .. BE16(0x0424), 3, .. A("abc"), .. BE32(Xmp.Length), .. Xmp];
        byte[] icc = Icc();
        File.WriteAllBytes(Path.Combine(dir, "full.jpg"), Jpeg(baseline, 0xC0,
            JpegSegment(0xE1, [.. A("Exif\0\0"), .. ExifTiff(false)]),
            JpegSegment(0xE1, [.. A("http://ns.adobe.com/xap/1.0/\0"), .. Xmp]),
            JpegSegment(0xE2, [.. A("ICC_PROFILE\0"), 2, 2, .. icc[100..]]),
            JpegSegment(0xE2, [.. A("ICC_PROFILE\0"), 1, 2, .. icc[..100]]),
            JpegSegment(0xED, photoshop),
            JpegSegment(0xEE, [.. A("Adobe"), 0, 100, 0, 0, 0, 0, 1]),
            JpegSegment(0xFE, U8("A comment\twith  spaces\0\0")),
            JpegSegment(0xE5, A("Unknown APP5"))));
        File.WriteAllBytes(Path.Combine(dir, "progressive.jpg"), Jpeg(baseline, 0xC2, JpegSegment(0xE1, [.. A("Exif\0\0"), .. ExifTiff(true)]), JpegSegment(0xFE, [0xC0, 0xFF, 0x41])));
        File.WriteAllBytes(Path.Combine(dir, "plain.jpg"), baseline);

        byte[] png = File.ReadAllBytes(Path.Combine(inputs, "alpha_64x48.png"));
        int iend = png.Length - 12;
        List<byte> fullPng = [.. png[..33]];
        fullPng.AddRange(PngChunk("pHYs", [.. BE32(3780), .. BE32(3779), 1]));
        fullPng.AddRange(PngChunk("gAMA", BE32(45455)));
        fullPng.AddRange(PngChunk("sRGB", [1]));
        fullPng.AddRange(PngChunk("cHRM", [.. BE32(31270), .. BE32(32900), .. BE32(64000), .. BE32(33000), .. BE32(30000), .. BE32(60000), .. BE32(15000), .. BE32(6000)]));
        fullPng.AddRange(PngChunk("cICP", [1, 13, 0, 1]));
        fullPng.AddRange(PngChunk("tIME", [.. BE16(2025), 12, 31, 23, 59, 58]));
        fullPng.AddRange(PngChunk("tEXt", [.. A("Software"), 0, .. Encoding.Latin1.GetBytes("ShareX édition")]));
        fullPng.AddRange(PngChunk("zTXt", [.. A("Comment"), 0, 0, .. Zlib(U8("compressed comment"))]));
        fullPng.AddRange(PngChunk("iTXt", [.. A("Title"), 0, 0, 0, .. A("fr"), 0, .. U8("Titre"), 0, .. U8("Le titre é")]));
        fullPng.AddRange(PngChunk("iTXt", [.. A("XML:com.adobe.xmp"), 0, 1, 0, 0, 0, .. Zlib(Xmp)]));
        fullPng.AddRange(PngChunk("eXIf", ExifTiff(true)));
        fullPng.AddRange(PngChunk("iCCP", [.. A("Test profile"), 0, 0, .. Zlib(icc)]));
        fullPng.AddRange(PngChunk("acTL", [.. BE32(3), .. BE32(0)]));
        fullPng.AddRange(PngChunk("prVt", A("private")));
        fullPng.AddRange(png[33..]);
        File.WriteAllBytes(Path.Combine(dir, "full.png"), fullPng.ToArray());

        byte[] gif = File.ReadAllBytes(Path.Combine(inputs, "palette.gif"));
        List<byte> fullGif = [.. gif[..13]];
        int table = (gif[10] & 0x80) != 0 ? 3 << ((gif[10] & 7) + 1) : 0;
        fullGif.AddRange(gif[13..(13 + table)]);
        fullGif.AddRange([0x21, 0xFF, 11, .. A("NETSCAPE2.0"), 3, 1, .. LE16(5), 0]);
        fullGif.AddRange([0x21, 0xFE, 9, .. A("A comment"), 0]);
        fullGif.AddRange([0x21, 0xFF, 11, .. A("XMP DataXMP"), .. Xmp.Chunk(255).SelectMany(c => (byte[])[(byte)c.Length, .. c]), 0]);
        fullGif.AddRange([0x21, 0xFF, 11, .. A("ICCRGBG1012"), 2, 1, 2, 0]);
        fullGif.AddRange([0x21, 0x01, 12, .. new byte[12], 3, .. A("txt"), 0]);
        byte[] body = gif[(13 + table)..];
        // Two frames, each with a graphic control extension (125 ms, then 2.5 s).
        fullGif.AddRange([0x21, 0xF9, 4, 0, .. LE16(12), 0, 0]);
        fullGif.AddRange(body[..^1]);
        fullGif.AddRange([0x21, 0xF9, 4, 0, .. LE16(250), 0, 0]);
        fullGif.AddRange(body);
        File.WriteAllBytes(Path.Combine(dir, "full.gif"), fullGif.ToArray());

        File.WriteAllBytes(Path.Combine(dir, "be.tif"), ExifTiff(false));
        File.WriteAllBytes(Path.Combine(dir, "le.tif"), ExifTiff(true));
        File.Copy(Path.Combine(inputs, "image24.bmp"), Path.Combine(dir, "image.bmp"), true);

        byte[] webp = File.ReadAllBytes(Path.Combine(inputs, "lossless.webp"));
        byte[] vp8l = webp[12..];
        byte[] vp8x = [0x3E, 0, 0, 0, .. LE32(28)[..3], .. LE32(20)[..3]];
        byte[] webpBody = [.. RiffChunk("VP8X", vp8x), .. RiffChunk("ICCP", icc), .. RiffChunk("ANIM", [0, 0, 0, 0, .. LE16(3)]), .. RiffChunk("ANMF", new byte[17]), .. vp8l, .. RiffChunk("EXIF", [.. A("Exif\0\0"), .. ExifTiff(true)]), .. RiffChunk("XMP ", Xmp)];
        File.WriteAllBytes(Path.Combine(dir, "full.webp"), [.. A("RIFF"), .. LE32(webpBody.Length + 4), .. A("WEBP"), .. webpBody]);
        byte[] lossy = [.. RiffChunk("VP8 ", [0x10, 0x02, 0, 0x9D, 0x01, 0x2A, .. LE16(321), .. LE16(0x4000 | 123), 0, 0])];
        File.WriteAllBytes(Path.Combine(dir, "lossy.webp"), [.. A("RIFF"), .. LE32(lossy.Length + 4), .. A("WEBP"), .. lossy]);

        if (File.Exists(ffmpeg))
        {
            string src = "-f lavfi -i testsrc=duration=2:size=160x90:rate=10 -f lavfi -i sine=frequency=440:duration=2";
            string meta = "-metadata title=\"Clip title\" -metadata artist=\"Ann Artist\" -metadata comment=\"Long comment é\" -metadata date=2024 -metadata:s:v:0 language=eng -metadata:s:a:0 title=\"Audio name\"";
            Ffmpeg(ffmpeg, $"{src} -c:v libx264 -pix_fmt yuv420p -c:a aac -shortest {meta} -y \"{Path.Combine(dir, "clip.mp4")}\"");
            Ffmpeg(ffmpeg, $"{src} -c:v libx264 -pix_fmt yuv420p -c:a aac -shortest -movflags use_metadata_tags {meta} -metadata:s:v:0 rotate=90 -y \"{Path.Combine(dir, "clip.mov")}\"");
            Ffmpeg(ffmpeg, $"{src} -c:v libvpx -b:v 200k -c:a libvorbis -shortest {meta} -y \"{Path.Combine(dir, "clip.mkv")}\"");
            Ffmpeg(ffmpeg, $"{src} -c:v libvpx -b:v 200k -c:a libvorbis -shortest {meta} -y \"{Path.Combine(dir, "clip.webm")}\"");
            Ffmpeg(ffmpeg, $"{src} -c:v mpeg4 -c:a libmp3lame -shortest {meta} -y \"{Path.Combine(dir, "clip.avi")}\"");
            Ffmpeg(ffmpeg, $"-f lavfi -i testsrc=duration=12:size=320x180:rate=10 -c:v libx264 -pix_fmt yuv420p -y \"{Path.Combine(dir, "long.mp4")}\"");
            Ffmpeg(ffmpeg, $"{src} -c:v wmv2 -c:a wmav2 -shortest {meta} -y \"{Path.Combine(dir, "clip.wmv")}\"");
        }
    }

    static void Ffmpeg(string ffmpeg, string args)
    {
        using Process p = Process.Start(new ProcessStartInfo(ffmpeg, "-hide_banner -loglevel error -fflags +bitexact -flags:v +bitexact -flags:a +bitexact " + args)
        { UseShellExecute = false, CreateNoWindow = true, RedirectStandardError = true });
        string err = p.StandardError.ReadToEnd();
        p.WaitForExit();
        if (p.ExitCode != 0) throw new InvalidOperationException(err);
    }
}
