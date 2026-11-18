// ShareX's print page rendering (PrintHelper.RenderImagePage, internal):
// the image placed on a white page three times the page's size in
// hundredths of an inch, as ShareX prints it.
//
//     ToolsHarness --print-page <image.png> <width> <height> <settings json> <out.png>

using System.Reflection;
using System.Text.Json;
using ShareX.HelpersLib;
using SkiaSharp;

static class PrintCases
{
    public static int Run(string[] a)
    {
        using SKBitmap image = SKBitmap.Decode(a[0]);
        var size = new System.Drawing.Size(int.Parse(a[1]), int.Parse(a[2]));
        using JsonDocument doc = JsonDocument.Parse(a[3]);
        JsonElement j = doc.RootElement;
        PrintSettings s = new()
        {
            Margin = j.GetProperty("Margin").GetInt32(),
            AutoRotateImage = j.GetProperty("AutoRotateImage").GetBoolean(),
            AutoScaleImage = j.GetProperty("AutoScaleImage").GetBoolean(),
            AllowEnlargeImage = j.GetProperty("AllowEnlargeImage").GetBoolean(),
            CenterImage = j.GetProperty("CenterImage").GetBoolean(),
        };
        MethodInfo m = typeof(PrintHelper).GetMethod("RenderImagePage", BindingFlags.NonPublic | BindingFlags.Static)!;
        using SKBitmap page = (SKBitmap)m.Invoke(null, [image, size, s])!;
        using FileStream f = File.Create(a[4]);
        page.Encode(f, SKEncodedImageFormat.Png, 100);
        Console.WriteLine($"{page.Width}x{page.Height}");
        return 0;
    }
}
