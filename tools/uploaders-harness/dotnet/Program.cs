// Runs ShareX's own uploaders (ShareX.UploadersLib from the reference build)
// for every case in cases.json against the local mock server, and writes
// what each sent and what ShareX made of the canned responses.
//
//   UploadersHarness <cases.json> <results.json> [port] [--fixture <UploadersConfig.json>]
//   UploadersHarness --after-upload <after_upload_cases.json> <out.json>  (AfterUpload.cs)
//   UploadersHarness --destination-pages <out.json>  (DestinationPages.cs)
//
// Every request goes to 127.0.0.1: the shared HttpClients ShareX's
// HttpClientFactory hands out are swapped (by reflection) for ones whose
// handler rewrites https://host/path to http://127.0.0.1:<port>/__/https/host/path
// in front of ShareX's own SocketsHttpHandler.

using System.Collections;
using System.Globalization;
using System.Net.Http;
using System.Reflection;
using System.Runtime.Loader;
using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;
using ShareX.HelpersLib;
using ShareX.UploadersLib;

static class Program
{
    static string ShareXBuild = Environment.GetEnvironmentVariable("SHAREX_BUILD") ?? @"G:\scripts\sharexrs-ref\ShareX-build";
    internal static int Port = 18765;

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
        if (args.Length == 3 && args[0] == "--after-upload") return AfterUpload.Run(args[1], args[2]);
        if (args.Length == 2 && args[0] == "--destination-pages") return DestinationPages.Run(args[1]);
        string casesPath = args[0];
        string resultsPath = args[1];
        if (args.Length > 2 && int.TryParse(args[2], out int p)) Port = p;
        string fixturePath = null;
        for (int i = 0; i < args.Length - 1; i++) if (args[i] == "--fixture") fixturePath = args[i + 1];

        Harness.Rewire(Port);
        // The mock SMTP server's certificate is self-signed (ShareX checks it).
#pragma warning disable SYSLIB0014
        System.Net.ServicePointManager.ServerCertificateValidationCallback = (sender, cert, chain, errors) => true;
#pragma warning restore SYSLIB0014

        JsonArray cases = JsonNode.Parse(File.ReadAllText(casesPath))!["cases"]!.AsArray();
        JsonArray results = new JsonArray();
        string only = Environment.GetEnvironmentVariable("HARNESS_ONLY");

        foreach (JsonNode c in cases)
        {
            string id = (string)c!["id"];
            if (!string.IsNullOrEmpty(only) && id != only) continue;
            JsonObject result = Harness.RunCase(c.AsObject());
            result["id"] = id;
            results.Add(result);
            Console.WriteLine($"{id}: {(result["exception"] != null ? "exception" : "ok")}");
        }

        File.WriteAllText(resultsPath, results.ToJsonString(new JsonSerializerOptions { WriteIndented = true }));

        if (fixturePath != null) Harness.WriteFixture(cases, fixturePath);
        return 0;
    }
}

sealed class RewriteHandler : DelegatingHandler
{
    private readonly int port;

    public RewriteHandler(HttpMessageHandler inner, int port) : base(inner) { this.port = port; }

    protected override async Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken cancellationToken)
    {
        Uri original = request.RequestUri;
        string rewritten = $"http://127.0.0.1:{port}/__/{original.Scheme}/{original.Authority}{original.PathAndQuery}";
        request.RequestUri = new Uri(rewritten);
        HttpResponseMessage response = await base.SendAsync(request, cancellationToken).ConfigureAwait(false);
        // ResponseInfo.ResponseURL is RequestMessage.RequestUri.OriginalString: put back what ShareX asked for
        // (or, after a redirect, the original form of where it ended).
        if (request.RequestUri.OriginalString == rewritten) request.RequestUri = original;
        else request.RequestUri = Unrewrite(request.RequestUri);
        return response;
    }

    private Uri Unrewrite(Uri uri)
    {
        string prefix = $"http://127.0.0.1:{port}/__/";
        string s = uri.OriginalString;
        if (!s.StartsWith(prefix)) return uri;
        string rest = s.Substring(prefix.Length);
        int slash = rest.IndexOf('/');
        return new Uri(rest.Substring(0, slash) + "://" + rest.Substring(slash + 1));
    }
}

static class Harness
{
    static readonly HttpClient Control = new HttpClient();

    public static void Rewire(int port)
    {
        // Make the factory create its clients, then wrap each one's handler.
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

    static void Reset(JsonNode responses)
    {
        string body = new JsonObject { ["responses"] = responses?.DeepClone() ?? new JsonArray() }.ToJsonString();
        Control.PostAsync($"http://127.0.0.1:{Program.Port}/__mock/reset", new StringContent(body)).GetAwaiter().GetResult().EnsureSuccessStatusCode();
    }

    static JsonNode Log()
    {
        string text = Control.GetStringAsync($"http://127.0.0.1:{Program.Port}/__mock/log").GetAwaiter().GetResult();
        return JsonNode.Parse(text);
    }

    // The FTP/SFTP/SMTP mocks (sharex_uploaders_mock_servers): their control port.
    static readonly int MockServers = int.Parse(Environment.GetEnvironmentVariable("MOCK_SERVERS_PORT") ?? "18820");

    static void ResetServers(JsonNode spec)
    {
        Control.PostAsync($"http://127.0.0.1:{MockServers}/reset", new StringContent(spec.ToJsonString())).GetAwaiter().GetResult().EnsureSuccessStatusCode();
    }

    static JsonNode ServersLog()
    {
        return JsonNode.Parse(Control.GetStringAsync($"http://127.0.0.1:{MockServers}/log").GetAwaiter().GetResult());
    }

    public static JsonObject RunCase(JsonObject c)
    {
        Reset(c["responses"]);
        if (c["servers"] != null) ResetServers(c["servers"]);
        JsonObject output = new JsonObject();
        Uploader uploader = null;
        try
        {
            if ((string)c["kind"] == "oauth")
            {
                OAuthCases.Run(c, output, ref uploader);
            }
            else if ((string)c["kind"] == "remote")
            {
                RemoteCases.Run(c, output);
            }
            else if ((string)c["kind"] == "listing")
            {
                RemoteCases.RunListing(c, output, ref uploader);
            }
            else
            {
                UploadResult result = Execute(c, ref uploader, output);
                output["result"] = ResultToJson(result);
            }
        }
        catch (Exception e)
        {
            Exception inner = e is AggregateException ae && ae.InnerException != null ? ae.InnerException : e;
            output["exception"] = inner.Message;
            output["exception_type"] = inner.GetType().FullName;
        }
        if (uploader != null) output["uploader_errors"] = ErrorsToJson(uploader.Errors);
        string checkFile = (string)c["input"]?["check_file"];
        if (checkFile != null)
        {
            output["written_file_base64"] = File.Exists(checkFile) ? Convert.ToBase64String(File.ReadAllBytes(checkFile)) : null;
            if (File.Exists(checkFile)) File.Delete(checkFile);
        }
        output["requests"] = Log();
        if (c["servers"] != null) output["servers"] = ServersLog();
        return output;
    }

    static UploadResult Execute(JsonObject c, ref Uploader uploader, JsonObject output)
    {
        string kind = (string)c["kind"];
        JsonObject input = c["input"]?.AsObject() ?? new JsonObject();
        string fileName = (string)input["file_name"] ?? "";
        string text = (string)input["text"];
        string url = (string)input["url"];
        byte[] data = input["content_base64"] != null ? Convert.FromBase64String((string)input["content_base64"]) : Encoding.UTF8.GetBytes(text ?? "");

        if (kind == "custom")
        {
            // {inputbox}, {outputbox} and {select} open windows: never here.
            string sxcuText = c["sxcu"]!.ToJsonString().ToLowerInvariant();
            if (new[] { "{inputbox", "{prompt", "{outputbox", "{select" }.Any(sxcuText.Contains))
                throw new Exception("harness: cases must not use dialog functions");
            CustomUploaderItem item = JsonHelpers.DeserializeFromString<CustomUploaderItem>(c["sxcu"]!.ToJsonString());
            item.CheckBackwardCompatibility();
            output["sxcu_export"] = JsonHelpers.SerializeToString(item, Newtonsoft.Json.DefaultValueHandling.Ignore, Newtonsoft.Json.NullValueHandling.Ignore);
            output["request_url"] = TryGet(() => item.GetRequestURL(new CustomUploaderInput(fileName, text ?? url ?? "")));
            string dest = (string)c["dest"];
            switch (dest)
            {
                case "image":
                {
                    var u = new ShareX.UploadersLib.ImageUploaders.CustomImageUploader(item); uploader = u;
                    using MemoryStream ms = new MemoryStream(data);
                    return u.UploadAsync(ms, fileName).GetAwaiter().GetResult();
                }
                case "file":
                {
                    var u = new ShareX.UploadersLib.FileUploaders.CustomFileUploader(item); uploader = u;
                    using MemoryStream ms = new MemoryStream(data);
                    return u.UploadAsync(ms, fileName).GetAwaiter().GetResult();
                }
                case "text":
                {
                    var u = new ShareX.UploadersLib.TextUploaders.CustomTextUploader(item); uploader = u;
                    return u.UploadTextAsync(text, fileName).GetAwaiter().GetResult();
                }
                case "url_shortener":
                {
                    var u = new ShareX.UploadersLib.URLShorteners.CustomURLShortener(item); uploader = u;
                    return u.ShortenURLAsync(url).GetAwaiter().GetResult();
                }
                case "url_sharing":
                {
                    var u = new ShareX.UploadersLib.SharingServices.CustomURLSharer(item); uploader = u;
                    return u.ShareURLAsync(url).GetAwaiter().GetResult();
                }
            }
            throw new Exception("unknown dest " + dest);
        }

        // Built-in destination: through UploaderFactory, as WorkerTask does.
        UploadersConfig config = JsonHelpers.DeserializeFromString<UploadersConfig>(c["config"]?.ToJsonString() ?? "{}") ?? new UploadersConfig();
        TaskReferenceHelper task = new TaskReferenceHelper { TextFormat = "text" };
        string[] service = ((string)c["service"]).Split(':');
        IUploaderService svc;
        switch (service[0])
        {
            case "image": svc = UploaderFactory.ImageUploaderServices[Enum.Parse<ImageDestination>(service[1])]; break;
            case "text": svc = UploaderFactory.TextUploaderServices[Enum.Parse<TextDestination>(service[1])]; break;
            case "file": svc = UploaderFactory.FileUploaderServices[Enum.Parse<FileDestination>(service[1])]; break;
            case "shortener": svc = UploaderFactory.URLShortenerServices[Enum.Parse<UrlShortenerType>(service[1])]; break;
            case "sharing": svc = UploaderFactory.URLSharingServices[Enum.Parse<URLSharingServices>(service[1])]; break;
            default: throw new Exception("unknown service " + c["service"]);
        }
        output["check_config"] = svc.CheckConfig(config);
        if (svc is IGenericUploaderService g)
        {
            GenericUploader u = g.CreateUploader(config, task); uploader = u;
            if (u is TextUploader tu) return tu.UploadTextAsync(text, fileName).GetAwaiter().GetResult();
            using MemoryStream ms = new MemoryStream(data);
            return u.UploadAsync(ms, fileName).GetAwaiter().GetResult();
        }
        if (svc is URLShortenerService s)
        {
            URLShortener u = s.CreateShortener(config, task); uploader = u;
            return u.ShortenURLAsync(url).GetAwaiter().GetResult();
        }
        if (svc is URLSharingService sh)
        {
            URLSharer u = sh.CreateSharer(config, task); uploader = u;
            return u.ShareURLAsync(url).GetAwaiter().GetResult();
        }
        throw new Exception("unhandled service");
    }

    static string TryGet(Func<string> f)
    {
        try { return f(); } catch (Exception e) { return "EXCEPTION: " + e.Message; }
    }

    static JsonNode ResultToJson(UploadResult r)
    {
        if (r == null) return null;
        return new JsonObject
        {
            ["url"] = r.URL,
            ["thumbnail_url"] = r.ThumbnailURL,
            ["deletion_url"] = r.DeletionURL,
            ["shortened_url"] = r.ShortenedURL,
            ["is_success"] = r.IsSuccess,
            ["is_error"] = r.IsError,
            ["is_url_expected"] = r.IsURLExpected,
            ["response"] = r.Response,
            ["errors"] = ErrorsToJson(r.Errors),
            ["response_url"] = r.ResponseInfo?.ResponseURL,
            ["status_code"] = r.ResponseInfo == null ? null : (int)r.ResponseInfo.StatusCode,
        };
    }

    static JsonArray ErrorsToJson(UploaderErrorManager errors)
    {
        JsonArray a = new JsonArray();
        if (errors != null) foreach (UploaderErrorInfo e in errors.Errors) a.Add(e.Text);
        return a;
    }

    /// <summary>An UploadersConfig.json written by ShareX's own Save, with every case's custom uploader
    /// and the built-in cases' settings, for the Rust round-trip test.</summary>
    public static void WriteFixture(JsonArray cases, string path)
    {
        UploadersConfig config = new UploadersConfig();
        foreach (JsonNode c in cases)
        {
            if ((string)c["kind"] == "custom")
            {
                CustomUploaderItem item = JsonHelpers.DeserializeFromString<CustomUploaderItem>(c["sxcu"]!.ToJsonString());
                config.CustomUploadersList.Add(item);
            }
            else if (c["config"] is JsonObject o)
            {
                Newtonsoft.Json.JsonConvert.PopulateObject(o.ToJsonString(), config);
            }
        }
        config.CustomTextUploaderSelected = 1;
        config.Save(path);
    }
}
