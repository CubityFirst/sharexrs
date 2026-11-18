// Runs ShareX's own update check code (ShareX.HelpersLib's GitHubUpdateChecker,
// FileDownloader, System.Version and Helpers.CompareVersion, from the reference
// build) over cases.json against the local mock server, and writes what each
// decided and sent.
//
//   UpdateHarness <cases.json> <results.json> <port> <work dir>
//
// Every request goes to 127.0.0.1: the shared HttpClients HttpClientFactory
// hands out are swapped (by reflection) for ones whose handler rewrites
// https://host/path to http://127.0.0.1:<port>/__/https/host/path, as in
// tools/uploaders-harness.

using System.Collections;
using System.Globalization;
using System.Reflection;
using System.Runtime.Loader;
using System.Security.Cryptography;
using System.Text.Json;
using System.Text.Json.Nodes;
using ShareX.HelpersLib;

static class Program
{
    static readonly string ShareXBuild = Environment.GetEnvironmentVariable("SHAREX_BUILD") ?? @"G:\scripts\sharexrs-ref\ShareX-build";
    static int Port;
    static readonly HttpClient Control = new HttpClient();

    static int Main(string[] args)
    {
        AssemblyLoadContext.Default.Resolving += (ctx, name) =>
        {
            string path = Path.Combine(ShareXBuild, name.Name + ".dll");
            return File.Exists(path) ? ctx.LoadFromAssemblyPath(path) : null;
        };
        CultureInfo.DefaultThreadCurrentCulture = CultureInfo.InvariantCulture;
        CultureInfo.DefaultThreadCurrentUICulture = CultureInfo.InvariantCulture;
        CultureInfo.CurrentCulture = CultureInfo.InvariantCulture;
        CultureInfo.CurrentUICulture = CultureInfo.InvariantCulture;
        return Run(args);
    }

    static int Run(string[] args)
    {
        string casesPath = args[0], resultsPath = args[1];
        Port = int.Parse(args[2]);
        string work = args[3];
        Directory.CreateDirectory(work);
        Rewire(Port);

        JsonArray cases = JsonNode.Parse(File.ReadAllText(casesPath))!["cases"]!.AsArray();
        JsonArray results = new JsonArray();
        foreach (JsonNode c in cases)
        {
            JsonObject o = c!.AsObject();
            string id = (string)o["id"];
            JsonObject result = (string)o["kind"] switch
            {
                "check" => Check(o),
                "download" => Download(o, work),
                "version" => Versions(o),
                _ => throw new InvalidOperationException()
            };
            result["id"] = id;
            results.Add(result);
            Console.WriteLine($"{id}: {(string)result["status"] ?? (string)result["exception"] ?? "ok"}");
        }
        File.WriteAllText(resultsPath, results.ToJsonString(new JsonSerializerOptions { WriteIndented = true }));
        return 0;
    }

    static JsonObject Check(JsonObject c)
    {
        Reset(c["responses"]);
        GitHubUpdateChecker checker = new GitHubUpdateChecker("CubityFirst", "sharexrs")
        {
            IsPortable = (bool)c["portable"],
            IncludePreRelease = (bool)c["include_pre_release"],
            IgnoreRevision = (bool)c["ignore_revision"],
            IsDev = (bool)c["is_dev"],
            CurrentVersion = Version.Parse((string)c["current"])
        };
        JsonObject result = new JsonObject();
        try
        {
            checker.CheckUpdateAsync().GetAwaiter().GetResult();
        }
        catch (Exception e)
        {
            result["exception"] = e.GetType().Name + ": " + e.Message;
        }
        result["status"] = checker.Status.ToString();
        result["current_version"] = checker.CurrentVersion?.ToString();
        result["latest_version"] = checker.LatestVersion?.ToString();
        result["download_url"] = checker.DownloadURL;
        string fileName = null;
        try { fileName = checker.FileName; } catch (Exception e) { fileName = "<" + e.GetType().Name + ">"; }
        result["file_name"] = fileName;
        result["is_pre_release"] = checker.IsPreRelease;
        result["requests"] = Requests();
        return result;
    }

    static JsonObject Download(JsonObject c, string work)
    {
        Reset(c["responses"]);
        string path = Path.Combine(work, "sharex-" + (string)c["id"] + ".bin");
        if (File.Exists(path)) File.Delete(path);
        FileDownloader downloader = new FileDownloader((string)c["url"], path) { AcceptHeader = (string)c["accept"] };
        int sizeEvents = 0;
        downloader.FileSizeReceived += () => sizeEvents++;
        JsonObject result = new JsonObject();
        try
        {
            result["returned"] = downloader.StartDownload().GetAwaiter().GetResult();
        }
        catch (Exception e)
        {
            result["exception"] = e.Message;
        }
        result["file_size"] = downloader.FileSize;
        result["downloaded_size"] = downloader.DownloadedSize;
        result["percentage"] = downloader.DownloadPercentage.ToString("0.0", CultureInfo.InvariantCulture);
        result["size_events"] = sizeEvents;
        result["file_exists"] = File.Exists(path);
        result["file_sha256"] = File.Exists(path) ? Convert.ToHexString(SHA256.HashData(File.ReadAllBytes(path))).ToLowerInvariant() : null;
        result["requests"] = Requests();
        return result;
    }

    static JsonObject Versions(JsonObject c)
    {
        JsonArray parsed = new JsonArray();
        foreach (JsonNode s in c["parse"]!.AsArray())
        {
            try { parsed.Add(new Version((string)s).ToString()); }
            catch (Exception e) { parsed.Add("<" + e.GetType().Name + ">"); }
        }
        JsonArray compared = new JsonArray();
        foreach (JsonNode p in c["compare"]!.AsArray())
        {
            JsonArray a = p!.AsArray();
            compared.Add(Helpers.CompareVersion(Version.Parse((string)a[0]), Version.Parse((string)a[1]), (bool)a[2]));
        }
        return new JsonObject { ["parse"] = parsed, ["compare"] = compared };
    }

    /// The requests the mock recorded: method, target and headers, without the user agent.
    static JsonArray Requests()
    {
        JsonArray log = JsonNode.Parse(Control.GetStringAsync($"http://127.0.0.1:{Port}/__mock/log").GetAwaiter().GetResult())!.AsArray();
        JsonArray outList = new JsonArray();
        foreach (JsonNode r in log)
        {
            JsonArray headers = new JsonArray();
            foreach (JsonNode h in r!["headers"]!.AsArray())
            {
                string name = (string)h![0];
                if (name.Equals("User-Agent", StringComparison.OrdinalIgnoreCase)) continue;
                headers.Add(new JsonArray(name.ToLowerInvariant(), (string)h[1]));
            }
            outList.Add(new JsonObject { ["method"] = (string)r["method"], ["target"] = (string)r["target"], ["headers"] = headers });
        }
        return outList;
    }

    static void Reset(JsonNode responses)
    {
        string body = new JsonObject { ["responses"] = responses?.DeepClone() ?? new JsonArray() }.ToJsonString();
        Control.PostAsync($"http://127.0.0.1:{Port}/__mock/reset", new StringContent(body)).GetAwaiter().GetResult().EnsureSuccessStatusCode();
    }

    static void Rewire(int port)
    {
        foreach (bool redirect in new[] { true, false })
            foreach (bool infinite in new[] { true, false })
                HttpClientFactory.Create(redirect, infinite);
        FieldInfo clientsField = typeof(HttpClientFactory).GetField("clients", BindingFlags.NonPublic | BindingFlags.Static);
        IDictionary clients = (IDictionary)clientsField.GetValue(null);
        FieldInfo handlerField = typeof(HttpMessageInvoker).GetField("_handler", BindingFlags.NonPublic | BindingFlags.Instance);
        foreach (object key in clients.Keys.Cast<object>().ToList())
        {
            HttpClient original = (HttpClient)clients[key];
            HttpMessageHandler inner = (HttpMessageHandler)handlerField.GetValue(original);
            HttpClient client = new HttpClient(new RewriteHandler(inner, port), false) { Timeout = original.Timeout };
            foreach (var header in original.DefaultRequestHeaders)
                client.DefaultRequestHeaders.TryAddWithoutValidation(header.Key, header.Value);
            clients[key] = client;
        }
    }
}

sealed class RewriteHandler : DelegatingHandler
{
    private readonly int port;

    public RewriteHandler(HttpMessageHandler inner, int port) : base(inner) { this.port = port; }

    protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken cancellationToken)
    {
        Uri original = request.RequestUri;
        request.RequestUri = new Uri($"http://127.0.0.1:{port}/__/{original.Scheme}/{original.Authority}{original.PathAndQuery}");
        return base.SendAsync(request, cancellationToken);
    }
}
