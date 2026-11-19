// The System tools' data side, run headless: the Clipboard viewer's previews
// (ClipboardViewerData over a ClipboardData snapshot built here, never the
// real clipboard) and the Inspect window's texts (WindowStyles.ToString,
// FormatRectangle).

using System.Buffers.Binary;
using System.Reflection;
using Newtonsoft.Json.Linq;
using ShareX.HelpersLib;
using SkiaSharp;

static partial class Program
{
    static byte[] DibOf(SKBitmap bmp, int bits, bool topDown = false, int headerSize = 40, int compression = 0, bool alphaMask = false)
    {
        int w = bmp.Width, h = bmp.Height;
        int row = (w * bits + 31) / 32 * 4;
        int masks = headerSize == 40 && compression == 3 ? 12 : 0;
        byte[] d = new byte[headerSize + masks + row * h];
        BinaryPrimitives.WriteInt32LittleEndian(d.AsSpan(0), headerSize);
        BinaryPrimitives.WriteInt32LittleEndian(d.AsSpan(4), w);
        BinaryPrimitives.WriteInt32LittleEndian(d.AsSpan(8), topDown ? -h : h);
        BinaryPrimitives.WriteUInt16LittleEndian(d.AsSpan(12), 1);
        BinaryPrimitives.WriteUInt16LittleEndian(d.AsSpan(14), (ushort)bits);
        BinaryPrimitives.WriteInt32LittleEndian(d.AsSpan(16), compression);
        BinaryPrimitives.WriteInt32LittleEndian(d.AsSpan(20), row * h);
        if (compression == 3)
        {
            BinaryPrimitives.WriteUInt32LittleEndian(d.AsSpan(40), 0x00FF0000);
            BinaryPrimitives.WriteUInt32LittleEndian(d.AsSpan(44), 0x0000FF00);
            BinaryPrimitives.WriteUInt32LittleEndian(d.AsSpan(48), 0x000000FF);
            if (alphaMask) BinaryPrimitives.WriteUInt32LittleEndian(d.AsSpan(52), 0xFF000000);
        }
        int offset = headerSize + masks;
        for (int y = 0; y < h; y++)
        {
            int dy = topDown ? y : h - 1 - y;
            for (int x = 0; x < w; x++)
            {
                SKColor c = bmp.GetPixel(x, y);
                int p = offset + dy * row + x * (bits / 8);
                // Premultiplied, as Windows' 32 bpp clipboard bitmaps are.
                byte pm(byte v) => bits == 32 ? (byte)((v * c.Alpha + 127) / 255) : v;
                d[p] = pm(c.Blue); d[p + 1] = pm(c.Green); d[p + 2] = pm(c.Red);
                if (bits == 32) d[p + 3] = c.Alpha;
            }
        }
        return d;
    }

    static SKBitmap ClipImage(int w, int h, bool alpha)
    {
        SKBitmap b = new(new SKImageInfo(w, h, SKColorType.Bgra8888, SKAlphaType.Premul));
        for (int y = 0; y < h; y++)
            for (int x = 0; x < w; x++)
                b.SetPixel(x, y, new SKColor((byte)(x * 23 + y * 3), (byte)(y * 31), (byte)(200 - x * 7), alpha ? (byte)(x * 255 / Math.Max(1, w - 1)) : (byte)255));
        return b;
    }

    // Writes the snapshots' inputs (inputs/clipboard/manifest.json: one
    // list of formats per snapshot, and the byte values) and returns them.
    // The first holds one of each kind; each later one a DIB variant under
    // its clipboard name.
    static List<ClipboardData> MakeClipboardSnapshots(string dir)
    {
        Directory.CreateDirectory(dir);
        JArray all = new(), manifest = new();
        List<ClipboardData> snapshots = [];
        ClipboardData data = new();
        void Next()
        {
            all.Add(manifest); snapshots.Add(data);
            manifest = new(); data = new();
        }
        int n = 0;
        void Bytes(string format, byte[] value)
        {
            string file = $"{n++}.bin";
            File.WriteAllBytes(Path.Combine(dir, file), value);
            manifest.Add(new JObject { ["format"] = format, ["bytes"] = file });
            data.SetData(format, value);
        }
        void Text(string format, string value)
        {
            manifest.Add(new JObject { ["format"] = format, ["text"] = value });
            data.SetData(format, value);
        }
        void Files(string format, string[] value)
        {
            manifest.Add(new JObject { ["format"] = format, ["files"] = new JArray(value) });
            data.SetData(format, value);
        }
        Text("UnicodeText", "Hello\r\nWorld \u2713 \U0001F600");
        Files("FileDrop", [@"C:\a.txt", @"D:\folder\b c.png"]);
        Bytes("HTML Format", System.Text.Encoding.UTF8.GetBytes("Version:0.9\r\n<b>x</b>"));
        using (SKBitmap a = ClipImage(17, 9, true))
        {
            Bytes("PNG", a.Encode(SKEncodedImageFormat.Png, 100).ToArray());
            Bytes("Bitmap", a.Encode(SKEncodedImageFormat.Png, 100).ToArray());
            Bytes("DeviceIndependentBitmap", DibOf(a, 32, compression: 3));
            Bytes("Format17", DibOf(a, 32, headerSize: 124, compression: 3, alphaMask: true));
            Bytes("image/png", [1, 2, 3, 4]);
            Bytes("Empty", []);
            Next();
            Bytes("DeviceIndependentBitmap", DibOf(a, 32)); Next();
            Bytes("DeviceIndependentBitmap", DibOf(a, 32, topDown: true, compression: 3)); Next();
            Bytes("Format17", DibOf(a, 32, headerSize: 124, compression: 3)); Next();
        }
        using (SKBitmap o = ClipImage(13, 7, false))
        {
            Bytes("DeviceIndependentBitmap", DibOf(o, 24)); Next();
            Bytes("DeviceIndependentBitmap", DibOf(o, 24, topDown: true)); Next();
            Bytes("DeviceIndependentBitmap", DibOf(o, 32, compression: 0).Select((b, i) => i >= 40 && (i - 40) % 4 == 3 ? (byte)0 : b).ToArray()); Next();
            byte[] truncated = DibOf(o, 24);
            Bytes("DeviceIndependentBitmap", truncated[..(truncated.Length - 5)]); Next();
            Bytes("Format17", DibOf(o, 24, headerSize: 124)); Next();
        }
        // 8 bpp with a palette (Skia's BMP decoder).
        {
            int w = 6, h = 4, row = 8;
            byte[] d = new byte[40 + 16 + row * h];
            BinaryPrimitives.WriteInt32LittleEndian(d.AsSpan(0), 40);
            BinaryPrimitives.WriteInt32LittleEndian(d.AsSpan(4), w);
            BinaryPrimitives.WriteInt32LittleEndian(d.AsSpan(8), h);
            BinaryPrimitives.WriteUInt16LittleEndian(d.AsSpan(12), 1);
            BinaryPrimitives.WriteUInt16LittleEndian(d.AsSpan(14), 8);
            BinaryPrimitives.WriteUInt32LittleEndian(d.AsSpan(32), 4);
            byte[][] pal = [[10, 20, 30, 0], [200, 100, 50, 0], [0, 255, 0, 0], [255, 255, 255, 0]];
            for (int i = 0; i < 4; i++) pal[i].CopyTo(d, 40 + i * 4);
            for (int y = 0; y < h; y++) for (int x = 0; x < w; x++) d[56 + y * row + x] = (byte)((x + y) % 4);
            Bytes("DeviceIndependentBitmap", d); Next();
        }
        File.WriteAllText(Path.Combine(dir, "manifest.json"), all.ToString());
        return snapshots;
    }

    static bool RunSystemDataCase(JObject c, string o)
    {
        switch ((string)c["tool"])
        {
            case "clipboard-preview":
            {
                Type t = typeof(ShareX.Tools.ClipboardViewerData);
                MethodInfo get = t.GetMethod("GetPreview")!;
                JArray snapshots = new();
                Directory.CreateDirectory(o);
                int i = 0;
                foreach (ClipboardData data in MakeClipboardSnapshots(Path.Combine(Inputs_, "clipboard")))
                {
                object viewer = t.GetConstructor(BindingFlags.NonPublic | BindingFlags.Instance, [typeof(ClipboardData)])!.Invoke([data]);
                JArray results = new();
                foreach (string format in data.GetFormats().Append("Missing"))
                {
                    JObject r = new() { ["format"] = format };
                    try
                    {
                        var p = (ShareX.Tools.ClipboardViewerPreview)get.Invoke(viewer, [format])!;
                        if (p.IsImage)
                        {
                            File.WriteAllBytes(Path.Combine(o, $"{i}.png"), p.ImageData!);
                            r["image"] = $"{i}.png"; r["width"] = p.ImageWidth; r["height"] = p.ImageHeight;
                        }
                        else r["text"] = p.Text;
                    }
                    catch (TargetInvocationException e) { r["error"] = e.InnerException!.GetType().Name; }
                    results.Add(r);
                    i++;
                }
                snapshots.Add(new JObject { ["formats"] = new JArray(data.GetFormats()), ["previews"] = results });
                }
                File.WriteAllText(o + ".json", new JObject { ["snapshots"] = snapshots }.ToString());
                return true;
            }
            case "bg-remove":
            {
                // ShareX's BackgroundRemovalService on the CPU device, over the
                // synthetic models make_bg_models.py writes (no real model).
                string modelPath = Path.Combine(Inputs_, (string)c["model"]);
                if (!File.Exists(modelPath)) throw new FileNotFoundException("run make_bg_models.py first", modelPath);
                FileInfo fi = new(modelPath);
                var model = new ShareX.Tools.BackgroundRemovalModel { FilePath = fi.FullName, FileName = fi.Name, FileSize = fi.Length };
                using var service = new ShareX.Tools.BackgroundRemovalService();
                JArray runs = new();
                Directory.CreateDirectory(o);
                int i = 0;
                foreach (JToken img in (JArray)c["images"])
                {
                    using SKBitmap source = SKBitmap.Decode(Path.Combine(Inputs_, (string)img));
                    var result = service.RemoveBackground(source, model, ShareX.Tools.BackgroundRemovalDevice.CPU);
                    Sxrb(Path.Combine(o, $"{i++}.sxrb"), result.Image);
                    runs.Add(new JObject { ["cached"] = result.IsSessionCached, ["device"] = result.ExecutionDevice });
                    result.Image.Dispose();
                }
                File.WriteAllText(o + ".json", new JObject { ["display"] = model.DisplayName, ["runs"] = runs }.ToString());
                return true;
            }
            case "inspect-styles":
            {
                JObject r = new()
                {
                    ["names"] = new JArray(System.Enum.GetNames(typeof(WindowStyles))),
                    ["values"] = new JArray(System.Enum.GetValues(typeof(WindowStyles)).Cast<WindowStyles>().Select(v => (long)(uint)v)),
                };
                r["strings"] = new JArray(((JArray)c["values"]).Select(v => ((WindowStyles)(uint)(long)v).ToString()));
                MethodInfo fmt = typeof(ShareX.Tools.InspectWindowViewModel).GetMethod("FormatRectangle", BindingFlags.NonPublic | BindingFlags.Static)!;
                r["rects"] = new JArray(((JArray)c["rects"]).Select(a => (string)fmt.Invoke(null, [new System.Drawing.Rectangle((int)a[0], (int)a[1], (int)a[2], (int)a[3])])!));
                File.WriteAllText(o + ".json", r.ToString());
                return true;
            }
        }
        return false;
    }
}
