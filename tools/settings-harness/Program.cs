// The ShareX side of tools/settings-harness: ShareX.HelpersLib's own
// ZipManager, as SettingManager.Export and Import use it, over the archives
// ShareXrs's tests replay.
//
//   SettingsHarness --extract <cases.json> <fixtures> <scratch> <results.json>
//   SettingsHarness --compress <scratch> <results.json> [<sharexrs-export.sxb>]
//
// No window opens. --extract writes only under <scratch>; --compress writes
// one archive and one file there.

using System.IO.Compression;
using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;
using ShareX.HelpersLib;

static class Program
{
    static int Main(string[] args)
    {
        if (args.Length == 5 && args[0] == "--extract")
        {
            return Extract(args[1], args[2], args[3], args[4]);
        }
        if ((args.Length == 3 || args.Length == 4) && args[0] == "--compress")
        {
            return Compress(args[1], args[2], args.Length == 4 ? args[3] : null);
        }
        Console.Error.WriteLine("usage: SettingsHarness --extract <cases> <fixtures> <scratch> <out> | --compress <scratch> <out> [<sharexrs archive>]");
        return 2;
    }

    static void Write(string path, JsonNode node)
    {
        File.WriteAllText(path, node.ToJsonString(new JsonSerializerOptions { WriteIndented = true, Encoder = System.Text.Encodings.Web.JavaScriptEncoder.UnsafeRelaxedJsonEscaping }) + "\n");
    }

    static string Stamp(DateTime t) => t.ToString("yyyy-MM-dd HH:mm:ss");

    /// Every file and folder under `root`, relative with '/', sorted ordinally.
    static JsonArray Tree(string root)
    {
        JsonArray items = new();
        if (!Directory.Exists(root))
        {
            return items;
        }
        var entries = Directory.GetFileSystemEntries(root, "*", SearchOption.AllDirectories)
            .Select(p => (path: p, rel: Path.GetRelativePath(root, p).Replace('\\', '/')))
            .OrderBy(x => x.rel, StringComparer.Ordinal);
        foreach (var (path, rel) in entries)
        {
            if (Directory.Exists(path))
            {
                items.Add(new JsonObject { ["path"] = rel + "/" });
            }
            else
            {
                items.Add(new JsonObject
                {
                    ["path"] = rel,
                    ["data"] = Convert.ToBase64String(File.ReadAllBytes(path)),
                    ["mtime"] = Stamp(File.GetLastWriteTime(path)),
                });
            }
        }
        return items;
    }

    static int Extract(string casesPath, string fixtures, string scratch, string outPath)
    {
        JsonNode cases = JsonNode.Parse(File.ReadAllText(casesPath));
        JsonArray results = new();
        foreach (JsonNode c in cases["cases"].AsArray())
        {
            string name = c["name"].GetValue<string>();
            string archive = Path.Combine(fixtures, c["archive"].GetValue<string>());
            // The destination is a folder inside the case's own folder, so a
            // name that climbs one level out stays in scratch and shows up.
            string caseRoot = Path.Combine(scratch, name);
            if (Directory.Exists(caseRoot))
            {
                Directory.Delete(caseRoot, true);
            }
            string dest = Path.Combine(caseRoot, "dest");
            Func<ZipArchiveEntry, bool> filter = c["filter"]?.GetValue<string>() == "json-xml"
                ? entry => FileHelpers.CheckExtension(entry.Name, new string[] { "json", "xml" })
                : null;
            JsonObject r = new() { ["name"] = name };
            if (!File.Exists(archive))
            {
                r["skipped"] = "no archive";
                results.Add(r);
                continue;
            }
            try
            {
                ZipManager.Extract(archive, dest, c["retain"].GetValue<bool>(), filter, c["max"].GetValue<long>());
                r["ok"] = true;
            }
            catch (Exception e)
            {
                r["ok"] = false;
                r["error"] = e.GetType().Name;
                r["message"] = e.Message;
            }
            r["tree"] = Tree(caseRoot);
            results.Add(r);
        }
        Write(outPath, new JsonObject { ["results"] = results });
        Console.WriteLine($"{results.Count} extract cases -> {outPath}");
        return 0;
    }

    static JsonArray List(string archive)
    {
        JsonArray list = new();
        using ZipArchive zip = ZipFile.OpenRead(archive);
        foreach (ZipArchiveEntry e in zip.Entries)
        {
            using Stream s = e.Open();
            using MemoryStream ms = new();
            s.CopyTo(ms);
            list.Add(new JsonObject
            {
                ["name"] = e.FullName,
                ["length"] = e.Length,
                ["crc32"] = e.Crc32,
                ["data"] = Convert.ToBase64String(ms.ToArray()),
                ["mtime"] = Stamp(e.LastWriteTime.DateTime),
            });
        }
        return list;
    }

    /// SettingManager.Export's archive: the three settings files from memory
    /// streams, then the history file (a missing one is left out).
    static int Compress(string scratch, string outPath, string sharexrsArchive)
    {
        Directory.CreateDirectory(scratch);
        string history = Path.Combine(scratch, "History.db");
        File.WriteAllBytes(history, Encoding.ASCII.GetBytes("SQLite format 3\0").Concat(new byte[48]).ToArray());
        DateTime historyTime = new(2026, 11, 19, 10, 20, 30, DateTimeKind.Local);
        File.SetLastWriteTime(history, historyTime);
        string archive = Path.Combine(scratch, "sharex-export.sxb");
        DateTime before = DateTime.Now;
        ZipManager.Compress(archive, new List<ZipEntryInfo>
        {
            new(new MemoryStream(Encoding.UTF8.GetBytes("{\r\n  \"ShowTray\": true\r\n}")), "ApplicationConfig.json"),
            new(new MemoryStream(Encoding.UTF8.GetBytes("{\r\n  \"ImgurAccountType\": \"Anonymous\"\r\n}")), "UploadersConfig.json"),
            new(new MemoryStream(Encoding.UTF8.GetBytes("{\r\n  \"Hotkeys\": []\r\n}")), "HotkeysConfig.json"),
            new(history),
            new(Path.Combine(scratch, "Missing.db")),
        });
        JsonObject result = new()
        {
            ["sharex"] = List(archive),
            ["sharexStarted"] = Stamp(before),
            ["historyTime"] = Stamp(historyTime),
        };
        if (sharexrsArchive != null && File.Exists(sharexrsArchive))
        {
            result["sharexrs"] = List(sharexrsArchive);
        }
        Write(outPath, result);
        Console.WriteLine($"export -> {outPath}");
        return 0;
    }
}
