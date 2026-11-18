// Analyze image: runs ShareX's own AnalyzeImageService (the four AI providers,
// TestConnectionAsync, LoadModelsAsync) over analyze_cases.json against the
// local mock server (tools/uploaders-harness/mock_server.py), and writes what
// each case sent and what ShareX made of the canned answer.
//
//   ToolsHarness --analyze <analyze_cases.json> <results.json> <port> <work dir>
//
// No request leaves the machine: every HttpClient in the process (the
// providers' HttpClientFactory client and the plain new HttpClient() Gemini and
// OpenRouter make) goes through .NET's HttpHandlerDiagnosticListener, whose
// request event rewrites https://host/path to
// http://127.0.0.1:<port>/__/https/host/path before the request is sent. Trace
// headers are turned off so the requests are as ShareX sends them.

using System.Diagnostics;
using System.Reflection;
using Newtonsoft.Json;
using Newtonsoft.Json.Linq;
using ShareX.HelpersLib;
using ShareX.Tools;
using SkiaSharp;

static class AnalyzeCases
{
    static int Port;
    static readonly HttpClient Control = new HttpClient();

    public static int Run(string[] args)
    {
        string casesPath = args[0], resultsPath = args[1];
        Port = int.Parse(args[2]);
        string work = Path.GetFullPath(args[3]);
        DistributedContextPropagator.Current = DistributedContextPropagator.CreateNoOutputPropagator();
        DiagnosticListener.AllListeners.Subscribe(new ListenerObserver());
        if (!RewriteWorks())
        {
            Console.Error.WriteLine("The request rewrite isn't working: stopping before any case runs.");
            return 1;
        }

        string inputs = Path.Combine(work, "analyze-inputs");
        GenerateInputs(inputs);

        JArray cases = (JArray)JObject.Parse(File.ReadAllText(casesPath))["cases"];
        JArray results = new JArray();
        foreach (JObject c in cases.Cast<JObject>())
        {
            JObject result = RunCase(c, inputs);
            result["id"] = c["id"];
            results.Add(result);
            Console.WriteLine($"{c["id"]}: {(string)result["error"] ?? "ok"}");
        }
        File.WriteAllText(resultsPath, results.ToString(Formatting.Indented));
        return 0;
    }

    /// Both kinds of client must reach the mock before a case may run: a
    /// request to 127.0.0.2 (loopback, so nothing leaves the machine even if
    /// it isn't rewritten) must arrive at the mock.
    static bool RewriteWorks()
    {
        JArray ok = JArray.Parse("[{\"status\":200,\"reason\":\"OK\",\"body\":\"\"},{\"status\":200,\"reason\":\"OK\",\"body\":\"\"}]");
        Reset(ok);
        try
        {
            using HttpClient plain = new HttpClient();
            plain.GetAsync("https://127.0.0.2/canary-plain").GetAwaiter().GetResult();
            HttpClientFactory.Create().GetAsync("https://127.0.0.2/canary-factory").GetAwaiter().GetResult();
        }
        catch (Exception e)
        {
            Console.Error.WriteLine(e.Message);
            return false;
        }
        string[] targets = Requests().Select(r => (string)r["target"]).ToArray();
        return targets.SequenceEqual(new[] { "/__/https/127.0.0.2/canary-plain", "/__/https/127.0.0.2/canary-factory" });
    }

    /// The images the cases send: written with ShareX's own encoders, read by
    /// both harnesses.
    static void GenerateInputs(string folder)
    {
        Directory.CreateDirectory(folder);
        using SKBitmap opaque = SkiaImageHelpers.CreateBitmap(64, 48);
        using SKBitmap alpha = SkiaImageHelpers.CreateBitmap(40, 30);
        for (int y = 0; y < 48; y++)
            for (int x = 0; x < 64; x++)
                opaque.SetPixel(x, y, new SKColor((byte)(x * 4), (byte)(y * 5), (byte)((x * y) % 256), 255));
        for (int y = 0; y < 30; y++)
            for (int x = 0; x < 40; x++)
                alpha.SetPixel(x, y, new SKColor((byte)(x * 6), 90, (byte)(y * 8), (byte)(x * 6 + y)));
        Save(opaque, Path.Combine(folder, "opaque.png"), SKEncodedImageFormat.Png);
        Save(alpha, Path.Combine(folder, "alpha.png"), SKEncodedImageFormat.Png);
        using (FileStream f = File.Create(Path.Combine(folder, "photo.jpg")))
            SkiaImageHelpers.SaveJPEG(opaque, f, 85);
        File.WriteAllBytes(Path.Combine(folder, "not-an-image.bin"), System.Text.Encoding.ASCII.GetBytes("not an image"));
    }

    static void Save(SKBitmap bitmap, string path, SKEncodedImageFormat format)
    {
        using FileStream f = File.Create(path);
        bitmap.Save(f, format);
    }

    static JObject RunCase(JObject c, string inputs)
    {
        Reset(c["responses"]);
        // ShareX's OpenAI providers leave their Authorization on the shared
        // client (a leak ShareXrs doesn't port); clear it between cases unless
        // the case shows the leak.
        if (!(bool?)c["keep_shared_auth"] ?? true)
            HttpClientFactory.Create().DefaultRequestHeaders.Authorization = null;

        AIOptions options = JsonConvert.DeserializeObject<AIOptions>(c["options"].ToString());
        AnalyzeImageService service = new AnalyzeImageService();
        JObject result = new JObject();
        try
        {
            switch ((string)c["kind"])
            {
                case "analyze":
                {
                    string path = c["file"] != null ? Path.Combine(inputs, (string)c["file"]) : null;
                    byte[] data = c["data_file"] != null ? File.ReadAllBytes(Path.Combine(inputs, (string)c["data_file"])) : (c["empty_data"] != null ? Array.Empty<byte>() : null);
                    string text = service.AnalyzeAsync(path, data, options).GetAwaiter().GetResult();
                    result["result"] = text;
                    break;
                }
                case "test":
                {
                    AnalyzeImageConnectionResult r = service.TestConnectionAsync(options).GetAwaiter().GetResult();
                    result["success"] = r.Success;
                    result["result"] = r.Message;
                    break;
                }
                case "models":
                {
                    IReadOnlyList<string> models = service.LoadModelsAsync(options).GetAwaiter().GetResult();
                    result["models"] = new JArray(models);
                    break;
                }
            }
        }
        catch (Exception e)
        {
            result["error"] = e.Message;
            result["error_type"] = e.GetType().Name;
        }
        result["requests"] = Requests();
        return result;
    }

    /// The requests the mock recorded: method, target, headers (names in
    /// lower case, without the user agent) and the body.
    static JArray Requests()
    {
        JArray log = JArray.Parse(Control.GetStringAsync($"http://127.0.0.1:{Port}/__mock/log").GetAwaiter().GetResult());
        JArray outList = new JArray();
        foreach (JObject r in log.Cast<JObject>())
        {
            JArray headers = new JArray();
            foreach (JArray h in r["headers"].Cast<JArray>())
            {
                string name = (string)h[0];
                if (name.Equals("User-Agent", StringComparison.OrdinalIgnoreCase)) continue;
                headers.Add(new JArray(name.ToLowerInvariant(), (string)h[1]));
            }
            outList.Add(new JObject { ["method"] = r["method"], ["target"] = r["target"], ["headers"] = headers, ["body_base64"] = r["body_base64"] });
        }
        return outList;
    }

    static void Reset(JToken responses)
    {
        string body = new JObject { ["responses"] = responses?.DeepClone() ?? new JArray() }.ToString(Formatting.None);
        Control.PostAsync($"http://127.0.0.1:{Port}/__mock/reset", new StringContent(body)).GetAwaiter().GetResult().EnsureSuccessStatusCode();
    }

    sealed class ListenerObserver : IObserver<DiagnosticListener>
    {
        public void OnNext(DiagnosticListener listener)
        {
            if (listener.Name == "HttpHandlerDiagnosticListener")
                listener.Subscribe(new RequestObserver(), name => name == "System.Net.Http.Request");
        }
        public void OnCompleted() { }
        public void OnError(Exception error) { }
    }

    sealed class RequestObserver : IObserver<KeyValuePair<string, object>>
    {
        public void OnNext(KeyValuePair<string, object> e)
        {
            if (e.Key != "System.Net.Http.Request") return;
            HttpRequestMessage request = (HttpRequestMessage)e.Value.GetType().GetProperty("Request").GetValue(e.Value);
            Uri original = request.RequestUri;
            if (original.Host == "127.0.0.1") return;
            Uri rewritten = new Uri($"http://127.0.0.1:{Port}/__/{original.Scheme}/{original.Authority}{original.PathAndQuery}");
            try
            {
                request.RequestUri = rewritten;
            }
            catch (InvalidOperationException)
            {
                typeof(HttpRequestMessage).GetField("_requestUri", BindingFlags.NonPublic | BindingFlags.Instance).SetValue(request, rewritten);
            }
        }
        public void OnCompleted() { }
        public void OnError(Exception error) { }
    }
}
