// The ShareX side of tools/scrolling-harness.
//
//   ScrollingHarness --scrolling <cases folder>
//       For each case in <cases folder>/cases.json: the frames are fed to
//       ShareX's own ScrollingCaptureManager (its private CombineImages and
//       state, through reflection) in the order and with the checks of its
//       StartCapture loop (CompareLastTwoImages through
//       SkiaImageHelpers.CompareImages). Writes <case>/result-sharex.png and
//       results-sharex.json.
//   ScrollingHarness --auto <out.json>
//       The auto capture window's formulas (AutoCaptureWindow.UpdateStatus,
//       Start, UpdateRegion) with ShareX.dll's strings.
//
// No window opens and nothing is captured: the frames are synthetic PNGs.

using System.Globalization;
using System.Reflection;
using System.Text.Json;
using System.Text.Json.Nodes;
using ShareX.HelpersLib;
using ShareX.ScreenCaptureLib;
using SkiaSharp;

static class Program
{
    static int Main(string[] args)
    {
        CultureInfo.CurrentCulture = CultureInfo.GetCultureInfo("en-US");
        CultureInfo.CurrentUICulture = CultureInfo.GetCultureInfo("en-US");
        if (args.Length == 2 && args[0] == "--scrolling")
        {
            return Scrolling(args[1]);
        }
        if (args.Length == 2 && args[0] == "--auto")
        {
            return Auto(args[1]);
        }
        Console.Error.WriteLine("usage: ScrollingHarness --scrolling <cases folder> | --auto <out.json>");
        return 2;
    }

    static void Write(string path, JsonNode node)
    {
        File.WriteAllText(path, node.ToJsonString(new JsonSerializerOptions { WriteIndented = true, Encoder = System.Text.Encodings.Web.JavaScriptEncoder.UnsafeRelaxedJsonEscaping }) + "\n");
    }

    static int Scrolling(string folder)
    {
        Assembly lib = typeof(ScrollingCaptureOptions).Assembly;
        Type managerType = lib.GetType("ShareX.ScreenCaptureLib.ScrollingCaptureManager", true);
        MethodInfo combine = managerType.GetMethod("CombineImages", BindingFlags.Instance | BindingFlags.NonPublic)
            ?? throw new MissingMethodException("CombineImages");
        FieldInfo statusField = managerType.GetField("status", BindingFlags.Instance | BindingFlags.NonPublic)
            ?? throw new MissingFieldException("status");

        JsonNode cases = JsonNode.Parse(File.ReadAllText(Path.Combine(folder, "cases.json")));
        JsonObject results = new();
        int count = 0;
        foreach (JsonNode c in cases["cases"].AsArray())
        {
            string name = c["name"].GetValue<string>();
            ScrollingCaptureOptions options = new() { AutoIgnoreBottomEdge = c["auto_ignore_bottom_edge"].GetValue<bool>() };
            object manager = Activator.CreateInstance(managerType, BindingFlags.Instance | BindingFlags.Public | BindingFlags.NonPublic, null, [options, null], null);
            // StartCapture: status = Failed before the loop.
            statusField.SetValue(manager, ScrollingCaptureStatus.Failed);

            List<SKBitmap> frames = c["frames"].AsArray().Select(f => SKBitmap.Decode(Path.Combine(folder, name, f.GetValue<string>()))).ToList();
            SKBitmap result = null, previous = null;
            int captured = 0, scrolls = 0;
            bool stop = false;
            JsonArray steps = new();

            // ScrollingCaptureManager.StartCapture's loop; the frames come in
            // order and stop is requested once the last one is handed out.
            while (!stop)
            {
                SKBitmap last = frames[captured++];
                if (captured == frames.Count) stop = true;

                if (last != null && previous != null && SkiaImageHelpers.CompareImages(last, previous))
                {
                    steps.Add(new JsonObject { ["frame"] = captured - 1, ["outcome"] = "identical" });
                    break;
                }

                scrolls++;

                if (last != null)
                {
                    SKBitmap newResult = (SKBitmap)combine.Invoke(manager, [result, last]);
                    ScrollingCaptureStatus st = (ScrollingCaptureStatus)statusField.GetValue(manager);
                    if (newResult != null)
                    {
                        result = newResult;
                        steps.Add(new JsonObject { ["frame"] = captured - 1, ["outcome"] = "combined", ["width"] = result.Width, ["height"] = result.Height, ["status"] = st.ToString() });
                    }
                    else
                    {
                        steps.Add(new JsonObject { ["frame"] = captured - 1, ["outcome"] = "failed", ["status"] = st.ToString() });
                        break;
                    }
                }

                if (stop) break;

                previous = last;
            }

            ScrollingCaptureStatus status = (ScrollingCaptureStatus)statusField.GetValue(manager);
            JsonObject r = new()
            {
                ["status"] = status.ToString(),
                ["frames"] = captured,
                ["scrolls"] = scrolls,
                ["width"] = result?.Width ?? 0,
                ["height"] = result?.Height ?? 0,
                ["steps"] = steps
            };
            string outPng = Path.Combine(folder, name, "result-sharex.png");
            if (result != null)
            {
                using SKImage image = SKImage.FromBitmap(result);
                using SKData data = image.Encode(SKEncodedImageFormat.Png, 100);
                File.WriteAllBytes(outPng, data.ToArray());
            }
            else if (File.Exists(outPng))
            {
                File.Delete(outPng);
            }
            results[name] = r;
            count++;
        }
        Write(Path.Combine(folder, "results-sharex.json"), results);
        Console.WriteLine($"{count} scrolling cases");
        return 0;
    }

    static int Auto(string outPath)
    {
        string status = ShareX.Localization.Strings.AutoCaptureWindow_Status;
        string region = ShareX.Localization.Strings.AutoCaptureWindow_Region;
        JsonObject o = new();

        // UpdateStatus: secondsLeft = (timeLeft / 1000f).ToString("0.0").
        IEnumerable<int> times = Enumerable.Range(0, 20001)
            .Concat(Enumerable.Range(0, 2000).Select(i => 20001 + i * 43201))
            .Append(86400000);
        o["seconds_left"] = string.Join(" ", times.Select(t => $"{t}={(t / 1000f).ToString("0.0")}"));

        // percentage = (int)(100 - (double)timeLeft / _delay * 100).
        JsonArray percentages = new();
        foreach (int delay in new[] { 1000, 1100, 1234, 2500, 3300, 60000, 86400000 })
        {
            for (int k = 0; k <= 200; k++)
            {
                int timeLeft = (int)((long)delay * k / 200);
                int percentage = delay > 0 ? (int)(100 - (double)timeLeft / delay * 100) : 100;
                percentages.Add(new JsonArray(delay, timeLeft, percentage));
            }
        }
        o["percentages"] = percentages;

        // Start: _delay = (int)(AutoCaptureRepeatTime * 1000), a decimal.
        JsonArray repeat = new();
        foreach (string s in new[] { "1", "1.1", "1.5", "2.3", "4.35", "4.349", "60", "0.1", "86400", "1.0001", "33.3333", "7.77" })
        {
            decimal d = decimal.Parse(s, CultureInfo.InvariantCulture);
            repeat.Add(new JsonArray(s, (int)(d * 1000)));
        }
        o["repeat_ms"] = repeat;

        JsonArray texts = new();
        foreach ((int t, int p, int n) in new[] { (60000, 0, 1), (1250, 97, 12), (350, 64, 3), (0, 100, 999) })
        {
            texts.Add(new JsonArray(t, p, n, string.Format(status, (t / 1000f).ToString("0.0"), p, n)));
        }
        o["status_text"] = texts;

        JsonArray regions = new();
        foreach ((int x, int y, int w, int h) in new[] { (0, 0, 1024, 768), (-1920, 12, 800, 600), (5, 6, 7, 8) })
        {
            regions.Add(new JsonArray(x, y, w, h, string.Format(region, x, y, w, h)));
        }
        o["region_text"] = regions;

        Write(outPath, o);
        Console.WriteLine("auto capture formulas written");
        return 0;
    }
}
