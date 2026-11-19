// How ShareX encodes a capture for saving and uploading: TaskHelpers.PrepareImage
// (SaveImageAsStream with the task's TaskSettingsImage: format, PNG depth,
// level and filter, JPEG quality and subsampling, GIF quality, BMP depth, WebP,
// the automatic switch to JPEG) over the bitmaps ShareX gives it.
//
// Case: {"tool": "capture", "file", "kind": "capture"|"premul", "settings": {TaskSettingsImage fields},
// "strip": PNGStripColorSpaceInformation}. Writes <case>.<ext> and <case>.json
// ({"Format", "Length"}).

using System.Reflection;
using System.Runtime.InteropServices;
using Newtonsoft.Json;
using Newtonsoft.Json.Linq;
using ShareX.HelpersLib;
using SkiaSharp;

static partial class Program
{
    static bool RunCaptureCase(JObject c, string o)
    {
        if ((string)c["tool"] != "capture") return false;
        // ApplicationState.Settings (internal): PrepareImage reads
        // PNGStripColorSpaceInformation from it.
        Type state = typeof(ShareX.TaskHelpers).Assembly.GetType("ShareX.ApplicationState", true);
        ShareX.ApplicationConfig config = new() { PNGStripColorSpaceInformation = (bool?)c["strip"] ?? false };
        state.GetProperty("Settings", BindingFlags.NonPublic | BindingFlags.Static).SetValue(null, config);

        ShareX.TaskSettings task = new();
        JsonConvert.PopulateObject(c["settings"].ToString(), task.ImageSettings);

        using SKBitmap source = SkiaImageHelpers.LoadImage(In(c, "file"));
        using SKBitmap image = AsTaskImage(source, (string)c["kind"]);
        using ShareX.ImageData data = ShareX.TaskHelpers.PrepareImage(image, task);
        byte[] bytes = data.ImageStream.ToArray();
        File.WriteAllBytes(o + "." + data.ImageFormat.GetFileExtension(), bytes);
        File.WriteAllText(o + ".json", new JObject { ["Format"] = data.ImageFormat.ToString(), ["Length"] = bytes.Length }.ToString());
        return true;
    }

    // "capture": what WindowsImageInterop.Capture hands the task (Bgra8888,
    // Unpremul, no colour space, every alpha 255: SetOpaque). "premul": a
    // tool's image (CreateBitmap: Bgra8888, Premul, no colour space).
    static SKBitmap AsTaskImage(SKBitmap source, string kind)
    {
        bool capture = kind == "capture";
        SKImageInfo info = new(source.Width, source.Height, SKColorType.Bgra8888, capture ? SKAlphaType.Unpremul : SKAlphaType.Premul);
        SKBitmap result = new(info);
        using (SKPixmap pixmap = source.PeekPixels())
            if (!pixmap.ReadPixels(info, result.GetPixels(), result.RowBytes)) throw new InvalidOperationException("ReadPixels");
        if (capture)
        {
            byte[] px = new byte[result.RowBytes * result.Height];
            Marshal.Copy(result.GetPixels(), px, 0, px.Length);
            for (int i = 3; i < px.Length; i += 4) px[i] = 255;
            Marshal.Copy(px, 0, result.GetPixels(), px.Length);
        }
        return result;
    }
}
