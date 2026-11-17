// The Data, Video and System tools' cases.

using System.Reflection;
using Newtonsoft.Json.Linq;
using ShareX.HelpersLib;
using ShareX.Tools;
using SkiaSharp;
using ZXing;
using ZXing.Common;

static partial class Program
{
    static string Work;

    static readonly int[] HashFileSizes = [0, 1, 3, 8191, 8192, 8193, 65543, 1048579];

    static void GenerateExtraInputs(string work)
    {
        Work = work;
        string hashDir = Path.Combine(Inputs_, "hash");
        Directory.CreateDirectory(hashDir);
        foreach (int size in HashFileSizes)
        {
            byte[] data = new byte[size];
            uint x = 2463534242u ^ (uint)size;
            for (int i = 0; i < size; i++) { x ^= x << 13; x ^= x >> 17; x ^= x << 5; data[i] = (byte)x; }
            File.WriteAllBytes(Path.Combine(hashDir, $"file{size}.bin"), data);
        }
        MakeFolders(Path.Combine(Inputs_, "folders"));
        MakeQRInputs(Path.Combine(Inputs_, "qr"));
        File.WriteAllText(Path.Combine(Inputs_, "custom.css"), "body { color: #123456; }\r\nh1 { color: red; }\n", new System.Text.UTF8Encoding(true));
    }

    static void Touch(string path, int size, bool hidden = false)
    {
        Directory.CreateDirectory(Path.GetDirectoryName(path));
        if (File.Exists(path)) File.SetAttributes(path, FileAttributes.Normal);
        using (FileStream fs = File.Create(path)) fs.SetLength(size);
        if (hidden) File.SetAttributes(path, FileAttributes.Hidden);
    }

    static void HiddenDir(string path)
    {
        Directory.CreateDirectory(path);
        new DirectoryInfo(path).Attributes |= FileAttributes.Hidden;
    }

    static void MakeFolders(string root)
    {
        if (Directory.Exists(root))
        {
            foreach (string f in Directory.EnumerateFileSystemEntries(root, "*", SearchOption.AllDirectories))
                File.SetAttributes(f, File.GetAttributes(f) & ~(FileAttributes.Hidden | FileAttributes.ReadOnly));
            Directory.Delete(root, true);
        }
        string t = Path.Combine(root, "tree");
        Touch(Path.Combine(t, "b.txt"), 10);
        Touch(Path.Combine(t, "A.txt"), 1500);
        Touch(Path.Combine(t, "_under.md"), 0);
        Touch(Path.Combine(t, "ä-umlaut.txt"), 2048);
        Touch(Path.Combine(t, "日本.txt"), 5);
        Touch(Path.Combine(t, "space name.log"), 123456);
        Touch(Path.Combine(t, "file10.txt"), 1000);
        Touch(Path.Combine(t, "file2.txt"), 1024);
        Touch(Path.Combine(t, "file1.txt"), 999);
        Touch(Path.Combine(t, "co-op.txt"), 7);
        Touch(Path.Combine(t, "coop.txt"), 8);
        Touch(Path.Combine(t, "it's & more.txt"), 1048576);
        Touch(Path.Combine(t, ".dotfile"), 3, hidden: true);
        Touch(Path.Combine(t, "hidden.txt"), 4, hidden: true);
        Touch(Path.Combine(t, "sub1", "a.png"), 999);
        Touch(Path.Combine(t, "sub1", "deep", "x.bin"), 1000000);
        Touch(Path.Combine(t, "sub1", "deep", "deeper", "y"), 1);
        Directory.CreateDirectory(Path.Combine(t, "Sub2"));
        HiddenDir(Path.Combine(t, "_hiddenDir"));
        Touch(Path.Combine(t, "_hiddenDir", "inside.txt"), 77);
        Touch(Path.Combine(t, "zeta", "z.txt"), 123456789);
        Touch(Path.Combine(t, "R&D's", "Ünïcödé — file.txt"), 1536);
        Directory.CreateDirectory(Path.Combine(root, "empty"));
        string sizes = Path.Combine(root, "sizes");
        int[] s = [999, 1000, 1023, 1024, 1535, 1536, 9999, 10240, 999999, 1000000, 1048575, 1048576, 1500000, 99999999];
        for (int i = 0; i < s.Length; i++) Touch(Path.Combine(sizes, $"f{i:D2}"), s[i]);
    }

    static SKBitmap QR(string text, int size) => ShareX.TaskHelpers.GenerateQRCode(text, size);

    static void SavePng(SKBitmap bmp, string path)
    {
        using SKData d = bmp.Encode(SKEncodedImageFormat.Png, 100);
        File.WriteAllBytes(path, d.ToArray());
    }

    static SKBitmap Barcode(BarcodeFormat format, string text, int w, int h)
    {
        BarcodeWriterGeneric writer = new() { Format = format, Options = new EncodingOptions { Width = w, Height = h, Margin = 10 } };
        BitMatrix m = writer.Encode(text);
        SKBitmap bmp = SkiaImageHelpers.CreateBitmap(m.Width, m.Height);
        for (int y = 0; y < m.Height; y++) for (int x = 0; x < m.Width; x++) bmp.SetPixel(x, y, m[x, y] ? SKColors.Black : SKColors.White);
        return bmp;
    }

    static void MakeQRInputs(string dir)
    {
        Directory.CreateDirectory(dir);
        using (SKBitmap q = QR("https://getsharex.com", 200)) SavePng(q, Path.Combine(dir, "plain.png"));
        using (SKBitmap q = QR("Rotated text", 160))
        using (SKBitmap r = new(q.Height, q.Width))
        using (SKCanvas c = new(r)) { c.Translate(r.Width, 0); c.RotateDegrees(90); c.DrawBitmap(q, 0, 0); c.Flush(); SavePng(r, Path.Combine(dir, "rotated90.png")); }
        using (SKBitmap q = QR("Inverted", 150))
        {
            for (int y = 0; y < q.Height; y++) for (int x = 0; x < q.Width; x++) { SKColor p = q.GetPixel(x, y); q.SetPixel(x, y, new SKColor((byte)(255 - p.Red), (byte)(255 - p.Green), (byte)(255 - p.Blue))); }
            SavePng(q, Path.Combine(dir, "inverted.png"));
        }
        using (SKBitmap q = QR("Translucent é日本", 180))
        using (SKBitmap t = SkiaImageHelpers.CreateBitmap(q.Width, q.Height))
        using (SKCanvas c = new(t))
        using (SKPaint p = new() { Color = SKColors.White.WithAlpha(180) })
        { c.DrawBitmap(q, 0, 0, p); c.Flush(); SavePng(t, Path.Combine(dir, "translucent.png")); }
        using (SKBitmap a = QR("First code", 150))
        using (SKBitmap b = QR("Second code", 150))
        using (SKBitmap both = SkiaImageHelpers.CreateBitmap(a.Width + b.Width + 40, Math.Max(a.Height, b.Height) + 20))
        using (SKCanvas c = new(both)) { c.Clear(SKColors.White); c.DrawBitmap(a, 10, 10); c.DrawBitmap(b, a.Width + 30, 10); c.Flush(); SavePng(both, Path.Combine(dir, "two.png")); }
        using (SKBitmap b = Barcode(BarcodeFormat.CODE_128, "SHAREX-128", 300, 100)) SavePng(b, Path.Combine(dir, "code128.png"));
        using (SKBitmap b = Barcode(BarcodeFormat.EAN_13, "5901234123457", 300, 120)) SavePng(b, Path.Combine(dir, "ean13.png"));
        using (SKBitmap b = Barcode(BarcodeFormat.DATA_MATRIX, "Data matrix", 120, 120)) SavePng(b, Path.Combine(dir, "datamatrix.png"));
        using (SKBitmap q = QR(new string('x', 600), 400)) SavePng(q, Path.Combine(dir, "dense.png"));
        using (SKBitmap q = QR("Small", 100))
        using (SKBitmap small = SkiaImageHelpers.ResizeImage(q.Copy(), 45, 45)) SavePng(small, Path.Combine(dir, "small.png"));
    }

    static IndexerSettings IndexerSettingsFrom(JObject c)
    {
        IndexerSettings s = new();
        JObject o = (JObject)c["settings"];
        foreach (JProperty p in o.Properties())
        {
            if (p.Name == "BinaryUnits") { s.BinaryUnits = (bool)p.Value; continue; }
            PropertyInfo pi = typeof(IndexerSettings).GetProperty(p.Name);
            object v = pi.PropertyType.IsEnum ? System.Enum.Parse(pi.PropertyType, (string)p.Value) : p.Value.ToObject(pi.PropertyType);
            if (p.Name == "CustomCSSFilePath") v = Path.Combine(Inputs_, (string)p.Value);
            pi.SetValue(s, v);
        }
        return s;
    }

    static bool RunDataCase(JObject c, string o)
    {
        switch ((string)c["tool"])
        {
            case "hash":
            {
                JObject r = new();
                foreach (int size in HashFileSizes)
                {
                    string path = Path.Combine(Inputs_, "hash", $"file{size}.bin");
                    r[$"file{size}.bin"] = new HashChecker().Start(path, System.Enum.Parse<HashType>((string)c["algorithm"])).GetAwaiter().GetResult();
                }
                r["missing"] = new HashChecker().Start(Path.Combine(Inputs_, "hash", "nope.bin"), HashType.MD5).GetAwaiter().GetResult();
                File.WriteAllText(o + ".json", r.ToString());
                return true;
            }
            case "qr":
            {
                string text = (string)c["text"];
                int size = (int)c["size"];
                using SKBitmap bmp = QR(text, size);
                JObject r = new() { ["null"] = bmp == null };
                if (bmp != null)
                {
                    Sxrb(o + ".sxrb", bmp);
                    MethodInfo save = typeof(ShareX.TaskHelpers).GetMethod("SaveQRCodeAsync", BindingFlags.NonPublic | BindingFlags.Static);
                    ((Task)save.Invoke(null, [text, size, o + ".svg"])).GetAwaiter().GetResult();
                }
                File.WriteAllText(o + ".json", r.ToString());
                return true;
            }
            case "qr-scan":
            {
                using SKBitmap bmp = SkiaImageHelpers.LoadImage(Path.Combine(Inputs_, "qr", (string)c["file"]));
                string[] r = ShareX.TaskHelpers.BarcodeScan(bmp, (bool)c["qrOnly"]);
                File.WriteAllText(o + ".json", r == null ? "null" : new JArray(r).ToString());
                return true;
            }
            case "indexer":
            {
                IndexerSettings s = IndexerSettingsFrom(c);
                string folder = Path.Combine(Inputs_, "folders", (string)c["folder"]);
                File.WriteAllText(o + ".txt", Indexer.Index(folder, s), new System.Text.UTF8Encoding(false));
                return true;
            }
        }
        return false;
    }
}
