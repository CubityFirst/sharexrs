// The ShareX side of tools/actions-harness: ShareX's own ExternalProgram,
// CodeMenuEntryActions and WatchFolder over the cases ShareXrs's tests replay.
//
//   ActionsHarness --actions <cases.json> <probe.exe> <scratch> <results.json>
//   ActionsHarness --watch <cases.json> <scratch> <results.json>
//
// Every file the cases touch is made in <scratch>; the only program run is
// the probe (crates/sharex-settings/examples/sharex_settings_action_probe.rs).

using System.Diagnostics;
using System.Text.Json;
using System.Text.Json.Nodes;
using Newtonsoft.Json;
using ShareX;
using ShareX.HelpersLib;

static class Program
{
    static int Main(string[] args)
    {
        if (args.Length == 5 && args[0] == "--actions")
        {
            return Actions(args[1], args[2], args[3], args[4]);
        }
        if (args.Length == 4 && args[0] == "--watch")
        {
            // The cases run side by side and sleep on pool threads; without
            // spare threads the watchers' own callbacks would queue behind
            // them (ShareX's pool isn't busy like that).
            ThreadPool.SetMinThreads(256, 256);
            return Watch(args[1], args[2], args[3]);
        }
        Console.Error.WriteLine("usage: ActionsHarness --actions <cases> <probe> <scratch> <out> | --watch <cases> <scratch> <out>");
        return 2;
    }

    static string Str(JsonNode n) => n == null ? null : n.GetValue<string>();

    static void Write(string path, JsonNode node)
    {
        File.WriteAllText(path, node.ToJsonString(new JsonSerializerOptions { WriteIndented = true, Encoder = System.Text.Encodings.Web.JavaScriptEncoder.UnsafeRelaxedJsonEscaping }) + "\n");
    }

    // ---------------------------------------------------------------- actions

    static int Actions(string casesPath, string probe, string scratch, string outPath)
    {
        JsonNode cases = JsonNode.Parse(File.ReadAllText(casesPath));
        JsonObject results = new();

        JsonArray parse = new();
        foreach (JsonNode c in cases["parse"].AsArray())
        {
            parse.Add(CodeMenuEntryActions.Parse(Str(c["pattern"]), Str(c["input"]), Str(c["output"])));
        }
        results["parse"] = parse;

        JsonArray check = new();
        ExternalProgram checker = new();
        foreach (JsonNode c in cases["check_extension"].AsArray())
        {
            check.Add(checker.CheckExtension(Str(c["path"]), Str(c["extensions"])));
        }
        results["check_extension"] = check;

        JsonArray json = new();
        foreach (JsonNode c in cases["json"].AsArray())
        {
            object value = Str(c["kind"]) switch
            {
                "ExternalProgram" => new ExternalProgram(),
                _ => new WatchFolderSettings(),
            };
            if (c["value"] is JsonObject v)
            {
                JsonConvert.PopulateObject(v.ToJsonString(), value);
            }
            json.Add(JsonConvert.SerializeObject(value));
        }
        results["json"] = json;

        string probeDir = Path.GetDirectoryName(probe);
        Environment.SetEnvironmentVariable("SHAREXRS_ACTION_PROBE_DIR", probeDir);
        JsonArray runs = new();
        foreach (JsonNode c in cases["runs"].AsArray())
        {
            string name = Str(c["name"]);
            string dir = Path.Combine(scratch, name);
            if (Directory.Exists(dir)) Directory.Delete(dir, true);
            Directory.CreateDirectory(dir);
            foreach (JsonNode f in c["files"].AsArray())
            {
                string file = Path.Combine(dir, Str(f));
                Directory.CreateDirectory(Path.GetDirectoryName(file));
                File.WriteAllText(file, "input");
            }
            string log = Path.Combine(scratch, name + ".log");
            if (File.Exists(log)) File.Delete(log);
            Environment.SetEnvironmentVariable("SHAREXRS_ACTION_PROBE_LOG", log);

            string Expand(string s) => s?.Replace("<dir>", dir).Replace("<probename>", Path.GetFileName(probe)).Replace("<probe>", probe);
            ExternalProgram program = new() { IsActive = true, Name = name, Path = probe };
            JsonObject p = c["program"].AsObject();
            if (p.ContainsKey("Path")) program.Path = Expand(Str(p["Path"]));
            if (p.ContainsKey("Args")) program.Args = Str(p["Args"]);
            if (p.ContainsKey("OutputExtension")) program.OutputExtension = Str(p["OutputExtension"]);
            if (p.ContainsKey("Extensions")) program.Extensions = Str(p["Extensions"]);
            if (p.ContainsKey("HiddenWindow")) program.HiddenWindow = p["HiddenWindow"].GetValue<bool>();
            if (p.ContainsKey("DeleteInputFile")) program.DeleteInputFile = p["DeleteInputFile"].GetValue<bool>();

            Stopwatch sw = Stopwatch.StartNew();
            string result = program.Run(Expand(Str(c["input"])));
            long elapsed = sw.ElapsedMilliseconds;
            program.DeletePendingInputFile();

            string Normalize(string s) => s?.Replace(dir, "<dir>").Replace(probe, "<probe>");
            JsonObject r = new()
            {
                ["name"] = name,
                ["result"] = Normalize(result),
                ["files"] = new JsonArray(Directory.GetFiles(dir, "*", SearchOption.AllDirectories)
                    .Select(x => Path.GetRelativePath(dir, x)).OrderBy(x => x, StringComparer.Ordinal).Select(x => (JsonNode)x).ToArray()),
                ["log"] = new JsonArray((File.Exists(log) ? File.ReadAllLines(log) : [])
                    .Select(l => JsonNode.Parse(l))
                    .Select(l => (JsonNode)new JsonObject
                    {
                        ["cmdline"] = Normalize(Str(l["cmdline"])),
                        ["args"] = new JsonArray(l["args"].AsArray().Select(a => (JsonNode)Normalize(Str(a))).ToArray()),
                    }).ToArray()),
                ["elapsed_ms"] = elapsed,
            };
            runs.Add(r);
        }
        results["runs"] = runs;
        Write(outPath, results);
        Console.WriteLine($"{parse.Count} parse, {check.Count} extension, {json.Count} JSON, {runs.Count} runs -> {outPath}");
        return 0;
    }

    // ------------------------------------------------------------------ watch

    static int Watch(string casesPath, string scratch, string outPath)
    {
        JsonNode cases = JsonNode.Parse(File.ReadAllText(casesPath));
        List<Task<JsonObject>> tasks = new();
        foreach (JsonNode c in cases["cases"].AsArray())
        {
            tasks.Add(Task.Run(() => WatchCase(c, scratch)));
        }
        JsonArray results = new(tasks.Select(t => (JsonNode)t.Result).ToArray());
        Write(outPath, new JsonObject { ["cases"] = results });
        Console.WriteLine($"{results.Count} watch cases -> {outPath}");
        return 0;
    }

    static JsonObject WatchCase(JsonNode c, string scratch)
    {
        string name = Str(c["name"]);
        string dir = Path.Combine(scratch, "watch-" + name);
        string outside = Path.Combine(scratch, "outside-" + name);
        foreach (string d in new[] { dir, outside })
        {
            if (Directory.Exists(d)) Directory.Delete(d, true);
            Directory.CreateDirectory(d);
        }
        foreach (JsonNode d in c["dirs"]?.AsArray() ?? new JsonArray())
        {
            Directory.CreateDirectory(Path.Combine(dir, Str(d)));
        }

        JsonObject s = c["settings"].AsObject();
        WatchFolderSettings settings = new()
        {
            FolderPath = dir,
            Filter = Str(s["Filter"]),
            IncludeSubdirectories = s["IncludeSubdirectories"]?.GetValue<bool>() ?? false,
        };
        List<(string, long)> triggers = new();
        Stopwatch sw = new();
        object gate = new();
        WatchFolder watch = new() { Settings = settings };
        watch.FileWatcherTrigger += path =>
        {
            lock (gate) triggers.Add((Path.GetRelativePath(dir, path), sw.ElapsedMilliseconds));
        };
        watch.Enable();
        Thread.Sleep(200);
        sw.Start();

        List<Task> held = new();
        foreach (JsonNode step in c["steps"].AsArray())
        {
            int at = step["at_ms"]?.GetValue<int>() ?? 0;
            int wait = at - (int)sw.ElapsedMilliseconds;
            if (wait > 0) Thread.Sleep(wait);
            string path = Path.Combine(dir, Str(step["path"]));
            switch (Str(step["op"]))
            {
                case "write":
                    File.WriteAllBytes(path, new byte[step["bytes"]?.GetValue<int>() ?? 10]);
                    break;
                case "append":
                    using (FileStream f = new(path, FileMode.Append)) f.WriteByte(1);
                    break;
                case "delete":
                    File.Delete(path);
                    break;
                case "write_held":
                {
                    // Written and kept open (others may read) for hold_ms.
                    FileStream f = new(path, FileMode.Create, FileAccess.Write, FileShare.Read);
                    f.Write(new byte[10]);
                    f.Flush();
                    int hold = step["hold_ms"].GetValue<int>();
                    held.Add(Task.Run(() => { Thread.Sleep(hold); f.Dispose(); }));
                    break;
                }
                case "move_in":
                {
                    string from = Path.Combine(outside, Path.GetFileName(path));
                    File.WriteAllBytes(from, new byte[10]);
                    File.Move(from, path);
                    break;
                }
                case "rename":
                    File.Move(Path.Combine(dir, Str(step["from"])), path);
                    break;
            }
        }
        Thread.Sleep(c["wait_ms"].GetValue<int>());
        Task.WaitAll(held.ToArray());
        watch.Dispose();
        lock (gate)
        {
            return new JsonObject
            {
                ["name"] = name,
                ["triggers"] = new JsonArray(triggers.OrderBy(t => t.Item1, StringComparer.Ordinal)
                    .Select(t => (JsonNode)new JsonObject { ["path"] = t.Item1, ["ms"] = t.Item2 }).ToArray()),
            };
        }
    }
}
