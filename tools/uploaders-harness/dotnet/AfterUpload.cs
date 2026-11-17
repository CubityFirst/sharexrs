// What WorkerTask.DoAfterUploadJobs makes of a result, through ShareX's own
// code (ShareX.dll from the reference build): UploadInfoParser.Parse on a
// TaskInfo (the text copied with ClipboardContentFormat, the URL opened
// with OpenURLFormat) and Regex.Replace (URLRegexReplace).
//
//   UploadersHarness --after-upload <after_upload_cases.json> <out.json>
//
// The output is the cases with ShareX's "output" added; ShareXrs's tests
// compare against it (crates/sharexrs/src/upload/fixtures).

using System.Text.Json;
using System.Text.Json.Nodes;
using System.Text.RegularExpressions;

static class AfterUpload
{
    public static int Run(string casesPath, string outPath)
    {
        JsonObject root = JsonNode.Parse(File.ReadAllText(casesPath))!.AsObject();
        JsonArray parse = new JsonArray();
        foreach (JsonNode c in root["parse"]!.AsArray())
        {
            ShareX.TaskInfo info = new ShareX.TaskInfo(new ShareX.TaskSettings());
            info.Result.URL = (string)c!["url"];
            info.Result.ThumbnailURL = (string)c["thumbnail_url"];
            info.Result.DeletionURL = (string)c["deletion_url"];
            info.Result.ShortenedURL = (string)c["shortened_url"];
            // The FilePath setter sets FileName; a given FileName comes after.
            info.FilePath = (string)c["file_path"];
            if (c["file_name"] != null) info.FileName = (string)c["file_name"];
            JsonObject o = c.DeepClone().AsObject();
            o["output"] = new ShareX.UploadInfoParser().Parse(info, (string)c["pattern"]);
            parse.Add(o);
        }
        JsonArray regex = new JsonArray();
        foreach (JsonNode c in root["regex"]!.AsArray())
        {
            JsonObject o = c!.DeepClone().AsObject();
            o["output"] = Regex.Replace((string)c["input"], (string)c["pattern"], (string)c["replacement"]);
            regex.Add(o);
        }
        JsonObject result = new JsonObject { ["parse"] = parse, ["regex"] = regex };
        File.WriteAllText(outPath, result.ToJsonString(new JsonSerializerOptions { WriteIndented = true, Encoder = System.Text.Encodings.Web.JavaScriptEncoder.UnsafeRelaxedJsonEscaping }) + "\n");
        Console.WriteLine($"after upload: {parse.Count} parse cases, {regex.Count} regex cases");
        return 0;
    }
}
