// ShareXrs tools harness: runs ShareX's own tool code (ShareX.Tools.dll and the
// HelpersLib/ShareX helpers it calls) over the cases in cases.json, headlessly,
// and writes what each tool produces so crates/sharex-tools can be compared
// with it (examples/tools_validate.rs).
//
//   ToolsHarness <cases.json> <work dir> [case prefix]
//
// Writes, under <work dir>:
//   inputs/   the generated test inputs (Inputs.cs; folders, videos too)
//   sharex/   per case: the tool's output files (<case>.<ext> or <case>/...),
//             <case>.sxrb for the bitmap before encoding, <case>.json for
//             values (hashes, counts, strings, command lines)
//
// .sxrb: "SXRB", u32 width, u32 height, then premultiplied BGRA pixels.

using System.Reflection;
using System.Runtime.InteropServices;
using System.Text;
using Newtonsoft.Json.Linq;
using ShareX.HelpersLib;
using ShareX.Tools;
using SkiaSharp;

static partial class Program
{
    static string Inputs_;
    static string Out;

    static int Main(string[] args)
    {
        if (args.Length < 2)
        {
            Console.Error.WriteLine("usage: ToolsHarness <cases.json> <work dir> [case prefix]");
            return 2;
        }
        string work = Path.GetFullPath(args[1]);
        Inputs_ = Path.Combine(work, "inputs");
        Out = Path.Combine(work, "sharex");
        Directory.CreateDirectory(Out);
        Inputs.Generate(Inputs_);
        GenerateExtraInputs(work);

        JArray cases = JArray.Parse(File.ReadAllText(args[0]));
        string only = args.Length > 2 ? args[2] : null;
        int failures = 0, count = 0;
        foreach (JObject c in cases.Cast<JObject>())
        {
            string name = (string)c["name"];
            if (only != null && !name.StartsWith(only, StringComparison.Ordinal)) continue;
            count++;
            File.Delete(Path.Combine(Out, name + ".error.txt"));
            try
            {
                RunCase(c);
            }
            catch (Exception e)
            {
                failures++;
                File.WriteAllText(Path.Combine(Out, name + ".error.txt"), e.ToString());
                Console.Error.WriteLine($"{name}: {e.GetBaseException().Message}");
            }
        }
        Console.WriteLine($"{count} cases, {failures} failed");
        return failures == 0 ? 0 : 1;
    }

    static string In(JObject c, string key) => Path.Combine(Inputs_, (string)c[key]);

    static string[] InList(JObject c, string key) => c[key].Select(t => Path.Combine(Inputs_, (string)t)).ToArray();

    static T Enum<T>(JObject c, string key) where T : struct => System.Enum.Parse<T>((string)c[key]);

    static System.Drawing.Color Color(JToken t) =>
        t == null ? System.Drawing.Color.White : System.Drawing.Color.FromArgb((int)t[0], (int)t[1], (int)t[2]);

    static void RunCase(JObject c)
    {
        string name = (string)c["name"];
        string tool = (string)c["tool"];
        string o = Path.Combine(Out, name);
        switch (tool)
        {
            case "combiner":
            {
                JObject opt = (JObject)c["options"];
                using SKBitmap bmp = SkiaImageHelpers.CombineImages(InList(c, "files"),
                    System.Enum.Parse<ImageOrientation>((string)opt["Orientation"]),
                    System.Enum.Parse<ShareX.HelpersLib.ImageCombinerAlignment>((string)opt["Alignment"]),
                    (int)opt["Space"], (int)opt["WrapAfter"], (bool)opt["AutoFillBackground"]);
                Sxrb(o + ".sxrb", bmp);
                using FileStream fs = File.Create(o + ".png");
                bmp.Save(fs, SKEncodedImageFormat.Png);
                break;
            }
            case "splitter":
            {
                Directory.CreateDirectory(o);
                MethodInfo m = typeof(ImageSplitterViewModel).GetMethod("SplitImage", BindingFlags.NonPublic | BindingFlags.Static);
                var files = (List<string>)m.Invoke(null, [In(c, "file"), (int)c["rows"], (int)c["columns"], o]);
                File.WriteAllText(o + ".json", new JArray(files.Select(Path.GetFileName)).ToString());
                using SKBitmap src = SkiaImageHelpers.LoadImage(In(c, "file"));
                List<SKBitmap> parts = SkiaImageHelpers.SplitImage(src, (int)c["rows"], (int)c["columns"]);
                for (int i = 0; i < parts.Count; i++) { Sxrb(Path.Combine(o, $"{i + 1}.sxrb"), parts[i]); parts[i].Dispose(); }
                break;
            }
            case "thumbnailer":
            {
                Directory.CreateDirectory(o);
                ImageThumbnailerViewModel vm = new();
                vm.Images.Add(In(c, "file"));
                vm.Width = (int)c["width"]; vm.Height = (int)c["height"]; vm.Quality = (int)c["quality"];
                vm.OutputFolderPath = o; vm.OutputFileName = "$filename_th";
                MethodInfo m = typeof(ImageThumbnailerViewModel).GetMethod("GenerateThumbnails", BindingFlags.NonPublic | BindingFlags.Instance);
                var files = (List<string>)m.Invoke(vm, null);
                File.WriteAllText(o + ".json", new JArray(files.Select(Path.GetFileName)).ToString());
                SKBitmap src = SkiaImageHelpers.LoadImage(In(c, "file"));
                using SKBitmap thumb = SkiaImageHelpers.CreateThumbnail(src, (int)c["width"], (int)c["height"]);
                using SKBitmap flat = SkiaImageHelpers.FillBackground(thumb, System.Drawing.Color.White);
                Sxrb(o + ".sxrb", flat);
                break;
            }
            case "resizer":
            {
                using SKBitmap src = SkiaImageHelpers.LoadImage(In(c, "file"));
                using SKBitmap bmp = ImageResizerService.Resize(src, (int)c["width"], (int)c["height"], Enum<ImageResizeMode>(c, "mode"));
                Sxrb(o + ".sxrb", bmp);
                var format = Enum<ImageResizeOutputFormat>(c, "format");
                ImageResizerService.Save(bmp, o + (format == ImageResizeOutputFormat.Jpeg ? ".jpg" : ".png"), format, (int)c["quality"], Color(c["background"]));
                byte[] preview = ImageResizerService.CreatePreview(In(c, "file"), (int)c["width"], (int)c["height"], Enum<ImageResizeMode>(c, "mode"), format, (int)c["quality"], Color(c["background"]));
                File.WriteAllBytes(o + ".preview" + (format == ImageResizeOutputFormat.Jpeg ? ".jpg" : ".png"), preview);
                break;
            }
            case "converter":
            {
                using SKBitmap src = ImageConverterService.LoadImage(In(c, "file"));
                Sxrb(o + ".loaded.sxrb", src);
                var format = Enum<ImageConverterOutputFormat>(c, "format");
                var bg = c["background"] is JArray a ? new SKColor((byte)(int)a[0], (byte)(int)a[1], (byte)(int)a[2]) : SKColors.White;
                ImageConverterService.Save(src, o + "." + ImageConverterService.GetFileExtension(format), format, (int?)c["quality"] ?? 90, bg,
                    null,
                    c["jpegSubsampling"] != null ? Enum<SKJpegEncoderDownsample>(c, "jpegSubsampling") : SKJpegEncoderDownsample.Downsample420,
                    c["gifQuality"] != null ? Enum<GIFQuality>(c, "gifQuality") : GIFQuality.Adaptive,
                    c["pngBitDepth"] != null ? Enum<PNGBitDepth>(c, "pngBitDepth") : PNGBitDepth.Automatic,
                    c["bmpBitDepth"] != null ? Enum<BMPBitDepth>(c, "bmpBitDepth") : BMPBitDepth.Bit24,
                    (bool?)c["webpLossless"] == true ? SKWebpEncoderCompression.Lossless : SKWebpEncoderCompression.Lossy);
                break;
            }
            case "converter-preview":
            {
                var format = Enum<ImageConverterOutputFormat>(c, "format");
                ImageConverterPreview p = ImageConverterService.CreatePreview(In(c, "file"), format, 90, SKColors.White);
                File.WriteAllBytes(o + ".png", p.Data);
                File.WriteAllText(o + ".json", new JObject { ["Width"] = p.Width, ["Height"] = p.Height }.ToString());
                break;
            }
            case "watermark":
            case "watermark-preview":
            {
                ImageWatermarkOptions opt = new(Enum<ImageWatermarkType>(c, "type"), (string)c["text"], Enum<ImageWatermarkPosition>(c, "position"),
                    (int)c["margin"], (int)c["opacity"], (float)c["textSize"], Color(c["textColor"]), (int)c["imageScale"], (float)c["rotation"]);
                var format = Enum<ImageWatermarkOutputFormat>(c, "format");
                string ext = format == ImageWatermarkOutputFormat.Jpeg ? ".jpg" : ".png";
                string wmPath = string.IsNullOrEmpty((string)c["watermark"]) ? "" : In(c, "watermark");
                if (tool == "watermark-preview")
                {
                    ImageWatermarkPreview p = ImageWatermarkService.CreatePreview(In(c, "file"), wmPath, opt, format, (int?)c["quality"] ?? 90, Color(c["background"]));
                    File.WriteAllBytes(o + ext, p.Data);
                    File.WriteAllText(o + ".json", new JObject { ["Width"] = p.Width, ["Height"] = p.Height }.ToString());
                    break;
                }
                using SKBitmap src = SkiaImageHelpers.LoadImage(In(c, "file"));
                using SKBitmap wm = wmPath == "" ? null : SkiaImageHelpers.LoadImage(wmPath);
                using SKBitmap bmp = ImageWatermarkService.Apply(src, wm, opt);
                Sxrb(o + ".sxrb", bmp);
                ImageWatermarkService.Save(bmp, o + ext, format, (int?)c["quality"] ?? 90, Color(c["background"]));
                break;
            }
            case "icon":
            {
                using SKBitmap src = SKBitmap.Decode(In(c, "file"));
                byte[] ico = IconConverterService.Convert(src, c["sizes"].Select(t => (int)t), Enum<IconBitDepth>(c, "depth"));
                File.WriteAllBytes(o + ".ico", ico);
                break;
            }
            case "comparer":
            {
                using SKBitmap a = SKBitmap.Decode(In(c, "a"));
                using SKBitmap b = SKBitmap.Decode(In(c, "b"));
                using ImageComparisonResult r = new ImageComparisonService().Compare(a, b, (int)c["maxPreview"]);
                Sxrb(o + ".sxrb", r.DiffBitmap);
                File.WriteAllText(o + ".json", new JObject
                {
                    ["MatchingPixels"] = r.MatchingPixels,
                    ["TotalPixels"] = r.TotalPixels,
                    ["Similarity"] = string.Format(ShareX.Tools.Localization.Strings.ImageComparerViewModel_Similarity_value, r.SimilarityPercentage),
                    ["Status"] = string.Format(ShareX.Tools.Localization.Strings.ImageComparerViewModel_Pixels_match, r.MatchingPixels, r.TotalPixels),
                }.ToString());
                break;
            }
            case "gifmaker":
                AnimatedGifMakerService.Create(o + ".gif", InList(c, "files"), (int)c["delay"], (bool)c["loop"], (int)c["repeat"]);
                break;
            default:
                if (!RunDataCase(c, o) && !RunSystemCase(c, o)) throw new NotSupportedException(tool);
                break;
        }
    }

    public static void Sxrb(string path, SKBitmap bmp)
    {
        using SKBitmap conv = new(new SKImageInfo(bmp.Width, bmp.Height, SKColorType.Bgra8888, SKAlphaType.Premul));
        using (SKPixmap src = bmp.PeekPixels())
            if (!src.ReadPixels(conv.Info, conv.GetPixels(), conv.RowBytes)) throw new InvalidOperationException("ReadPixels");
        byte[] px = new byte[conv.Width * conv.Height * 4];
        for (int y = 0; y < conv.Height; y++)
            Marshal.Copy(conv.GetPixels() + y * conv.RowBytes, px, y * conv.Width * 4, conv.Width * 4);
        using FileStream fs = File.Create(path);
        fs.Write(Encoding.ASCII.GetBytes("SXRB"));
        fs.Write(BitConverter.GetBytes((uint)conv.Width));
        fs.Write(BitConverter.GetBytes((uint)conv.Height));
        fs.Write(px);
    }
}
