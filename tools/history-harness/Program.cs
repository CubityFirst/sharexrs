// Runs ShareX's own history code (ShareX.HistoryLib.dll) over cases.json
// and dumps what it wrote, read, filtered and counted, for
// crates/sharex-history's harness example to compare with (see README.md).
//
//   HistoryHarness sharex <cases.json> <work dir>   everything ShareX does, into <work>/sharex.json
//   HistoryHarness read <History.db> <out.json>     ShareX reading a database (ShareXrs's)
//   HistoryHarness read-json <History.json> <out.json>
//
// Internal members (HistoryManager.Load, StatsFormat, the charts' helpers)
// are reached by reflection; nothing is patched.

using System.Globalization;
using System.Reflection;
using Microsoft.Data.Sqlite;
using Newtonsoft.Json;
using Newtonsoft.Json.Linq;
using ShareX.HistoryLib;

static class H
{
    static readonly Assembly Lib = typeof(HistoryItem).Assembly;
    const BindingFlags Any = BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Static | BindingFlags.Instance;

    static int Main(string[] args)
    {
        SQLitePCL.Batteries_V2.Init();
        switch (args[0])
        {
            case "sharex": RunShareX(args[1], args[2]); break;
            case "read": File.WriteAllText(args[2], ReadDb(args[1]).ToString()); break;
            case "read-json": File.WriteAllText(args[2], DumpItems(LoadWith(new HistoryManagerJSON(args[1]), args[1])).ToString()); break;
            case "append": Append(args[1], args[2]); break;
            case "hold": Hold(args[1], int.Parse(args[2])); break;
            default: Console.Error.WriteLine("unknown command"); return 1;
        }
        return 0;
    }

    // ---- items in and out -------------------------------------------------

    static DateTime SpecDate(JToken t)
    {
        if (t == null || t.Type == JTokenType.Null) return DateTime.MinValue;
        JProperty p = ((JObject)t).Properties().First();
        string s = (string)p.Value;
        DateTime wall = DateTime.ParseExact(s, "yyyy-MM-ddTHH:mm:ss.fffffff", CultureInfo.InvariantCulture);
        return p.Name switch
        {
            "local" => DateTime.SpecifyKind(wall, DateTimeKind.Local),
            "utc" => DateTime.SpecifyKind(wall, DateTimeKind.Utc),
            "unspecified" => DateTime.SpecifyKind(wall, DateTimeKind.Unspecified),
            "utc_to_local" => DateTime.SpecifyKind(wall, DateTimeKind.Utc).ToLocalTime(),
            _ => throw new Exception("date kind " + p.Name)
        };
    }

    static HistoryItem SpecItem(JObject o)
    {
        HistoryItem item = new HistoryItem
        {
            FileName = (string)o["FileName"],
            FilePath = (string)o["FilePath"],
            DateTime = SpecDate(o["DateTime"]),
            Type = (string)o["Type"],
            Host = (string)o["Host"],
            URL = (string)o["URL"],
            ThumbnailURL = (string)o["ThumbnailURL"],
            DeletionURL = (string)o["DeletionURL"],
            ShortenedURL = (string)o["ShortenedURL"],
        };
        if (o["Tags"] is JArray tags)
        {
            item.Tags = new Dictionary<string, string>();
            foreach (JArray pair in tags) item.Tags[(string)pair[0]] = (string)pair[1];
        }
        return item;
    }

    static JToken Str(string s) => s == null ? JValue.CreateNull() : new JValue(s);

    static JObject DumpDate(DateTime d) => new JObject
    {
        ["Ticks"] = d.Ticks,
        ["Kind"] = d.Kind.ToString(),
        ["O"] = d.ToString("o"),
    };

    static JObject DumpItem(HistoryItem i)
    {
        JToken tags = JValue.CreateNull();
        if (i.Tags != null) tags = new JArray(i.Tags.Select(kv => new JArray(kv.Key, Str(kv.Value))));
        return new JObject
        {
            ["Id"] = i.Id,
            ["FileName"] = Str(i.FileName),
            ["FilePath"] = Str(i.FilePath),
            ["DateTime"] = DumpDate(i.DateTime),
            ["Type"] = Str(i.Type),
            ["Host"] = Str(i.Host),
            ["URL"] = Str(i.URL),
            ["ThumbnailURL"] = Str(i.ThumbnailURL),
            ["DeletionURL"] = Str(i.DeletionURL),
            ["ShortenedURL"] = Str(i.ShortenedURL),
            ["Tags"] = tags,
            ["Favorite"] = i.Favorite,
            ["Tag"] = Str(i.Tag),
            ["TagsWindowTitle"] = Str(i.TagsWindowTitle),
            ["TagsProcessName"] = Str(i.TagsProcessName),
        };
    }

    static JArray DumpItems(List<HistoryItem> items) => new JArray(items.Select(DumpItem));

    // HistoryManager.Load(string) is internal; GetHistoryItems would show a message box on errors.
    static List<HistoryItem> LoadWith(HistoryManager m, string path)
    {
        MethodInfo load = typeof(HistoryManager).GetMethod("Load", Any, new[] { typeof(string) });
        try
        {
            return (List<HistoryItem>)load.Invoke(m, new object[] { path });
        }
        catch (TargetInvocationException e)
        {
            throw e.InnerException;
        }
    }

    static JToken ReadDb(string db)
    {
        try
        {
            using HistoryManagerSQLite m = new HistoryManagerSQLite(db);
            return DumpItems(LoadWith(m, db));
        }
        catch (Exception e)
        {
            return new JObject { ["Error"] = e.GetType().Name };
        }
    }

    // ---- concurrent access -------------------------------------------------

    // A finished task's AppendHistoryItem while another app may hold the
    // database: prints how long it took and whether it succeeded.
    static void Append(string db, string itemJson)
    {
        JObject spec = JsonConvert.DeserializeObject<JObject>(itemJson, new JsonSerializerSettings { DateParseHandling = DateParseHandling.None });
        var watch = System.Diagnostics.Stopwatch.StartNew();
        using HistoryManagerSQLite m = new HistoryManagerSQLite(db);
        bool ok = m.AppendHistoryItem(SpecItem(spec));
        Console.WriteLine($"{(ok ? "ok" : "failed")} {watch.ElapsedMilliseconds}");
    }

    // Holds a write transaction (as ShareX's Append does while it inserts)
    // for the given time, after printing "locked".
    static void Hold(string db, int ms)
    {
        using HistoryManagerSQLite m = new HistoryManagerSQLite(db);
        using SqliteConnection c = new SqliteConnection("Data Source=" + db + ";Pooling=False");
        c.Open();
        using SqliteTransaction tx = c.BeginTransaction();
        using (SqliteCommand cmd = new SqliteCommand("INSERT INTO History (FileName, FilePath, DateTime) VALUES ('held.png', 'C:\\held.png', '2026-11-17T12:00:00.0000000+00:00');", c, tx))
            cmd.ExecuteNonQuery();
        Console.WriteLine("locked");
        Console.Out.Flush();
        Thread.Sleep(ms);
        tx.Commit();
    }

    // ---- the ShareX run ---------------------------------------------------

    static void RunShareX(string casesPath, string work)
    {
        // Strings stay strings (JObject.Parse would turn ISO dates into DateTime values).
        JObject cases = JsonConvert.DeserializeObject<JObject>(File.ReadAllText(casesPath),
            new JsonSerializerSettings { DateParseHandling = DateParseHandling.None });
        Directory.CreateDirectory(work);
        JObject result = new JObject();

        foreach (string f in Directory.GetFiles(work, "sharex*")) File.Delete(f);

        using (SqliteConnection c = new SqliteConnection("Data Source=:memory:"))
        {
            c.Open();
            using SqliteCommand cmd = new SqliteCommand("select sqlite_version()", c);
            result["SqliteVersion"] = (string)cmd.ExecuteScalar();
        }

        // 1. Items written as a finished task does (AppendHistoryItems), then
        //    edited as the windows do (Favorite, Tag, Edit, Delete) on loaded items.
        string db = Path.Combine(work, "sharex.db");
        using (HistoryManagerSQLite m = new HistoryManagerSQLite(db))
        {
            List<HistoryItem> items = cases["write"].Select(t => SpecItem((JObject)t)).ToList();
            m.AppendHistoryItems(items);
            result["AppendedIds"] = new JArray(items.Select(i => i.Id));
            foreach (JObject op in cases["ops"])
            {
                List<HistoryItem> loaded = LoadWith(m, db);
                HistoryItem item = op["id"] != null ? loaded.First(i => i.Id == (long)op["id"]) : null;
                switch ((string)op["op"])
                {
                    case "favorite": item.Favorite = !item.Favorite; m.Edit(item); break;
                    case "tag": item.Tag = (string)op["value"]; m.Edit(item); break;
                    case "set_tags":
                        item.Tags = new Dictionary<string, string>();
                        foreach (JArray pair in op["tags"]) item.Tags[(string)pair[0]] = (string)pair[1];
                        m.Edit(item);
                        break;
                    case "edit":
                        item.FileName = (string)op["FileName"];
                        item.URL = (string)op["URL"];
                        item.Host = (string)op["Host"];
                        m.Edit(item);
                        break;
                    case "delete": m.Delete(loaded.Where(i => op["ids"].Any(x => (long)x == i.Id)).ToArray()); break;
                    case "append":
                        List<HistoryItem> more = op["items"].Select(t => SpecItem((JObject)t)).ToList();
                        m.AppendHistoryItems(more);
                        break;
                }
            }
        }
        result["Read"] = ReadDb(db);

        // 2. Odd stored values, inserted with SQL into a database ShareX made.
        string rawDb = Path.Combine(work, "sharex-raw.db");
        new HistoryManagerSQLite(rawDb).Dispose();
        using (SqliteConnection c = new SqliteConnection("Data Source=" + rawDb + ";Pooling=False"))
        {
            c.Open();
            foreach (string sql in cases["raw"].Values<string>())
            {
                using SqliteCommand cmd = new SqliteCommand(sql, c);
                cmd.ExecuteNonQuery();
            }
        }
        result["ReadRaw"] = ReadDb(rawDb);
        JObject rawErrors = new JObject();
        foreach (JProperty p in ((JObject)cases["raw_errors"]).Properties())
        {
            string errDb = Path.Combine(work, "sharex-raw-" + p.Name + ".db");
            new HistoryManagerSQLite(errDb).Dispose();
            using (SqliteConnection c = new SqliteConnection("Data Source=" + errDb + ";Pooling=False"))
            {
                c.Open();
                using SqliteCommand cmd = new SqliteCommand((string)p.Value, c);
                cmd.ExecuteNonQuery();
            }
            rawErrors[p.Name] = ReadDb(errDb);
        }
        result["RawErrors"] = rawErrors;

        // 3. Legacy files: History.json written by ShareX's JSON manager, and a
        //    hand-written one, each migrated (MigrateFromJSON) into a new database.
        string json = Path.Combine(work, "sharex-History.json");
        new HistoryManagerJSON(json).AppendHistoryItems(cases["json_items"].Select(t => SpecItem((JObject)t)).ToList());
        new HistoryManagerJSON(json).AppendHistoryItems(cases["json_items_2"].Select(t => SpecItem((JObject)t)).ToList());
        result["JsonRead"] = DumpItems(LoadWith(new HistoryManagerJSON(json), json));
        using (HistoryManagerSQLite m = new HistoryManagerSQLite(Path.Combine(work, "sharex-migrated.db"))) m.MigrateFromJSON(json);
        result["JsonMigrated"] = ReadDb(Path.Combine(work, "sharex-migrated.db"));

        string jsonRaw = Path.Combine(work, "sharex-raw-History.json");
        File.WriteAllText(jsonRaw, (string)cases["json_raw"]);
        result["JsonRawRead"] = DumpItems(LoadWith(new HistoryManagerJSON(jsonRaw), jsonRaw));
        using (HistoryManagerSQLite m = new HistoryManagerSQLite(Path.Combine(work, "sharex-migrated-raw.db"))) m.MigrateFromJSON(jsonRaw);
        result["JsonRawMigrated"] = ReadDb(Path.Combine(work, "sharex-migrated-raw.db"));

        string xml = Path.Combine(work, "sharex-History.xml");
        new HistoryManagerXML(xml).AppendHistoryItems(cases["json_items"].Select(t => SpecItem((JObject)t)).ToList());
        new HistoryManagerXML(xml).AppendHistoryItems(cases["json_items_2"].Select(t => SpecItem((JObject)t)).ToList());
        result["XmlRead"] = DumpItems(LoadWith(new HistoryManagerXML(xml), xml));
        string xmlRaw = Path.Combine(work, "sharex-raw-History.xml");
        File.WriteAllText(xmlRaw, (string)cases["xml_raw"]);
        result["XmlRawRead"] = DumpItems(LoadWith(new HistoryManagerXML(xmlRaw), xmlRaw));

        // 4. Queries and stats over what was read back from sharex.db (and the raw one).
        List<HistoryItem> all;
        using (HistoryManagerSQLite m = new HistoryManagerSQLite(db)) all = LoadWith(m, db);
        all.Reverse();
        result["Queries"] = Queries(cases, all);
        List<HistoryItem> rawItems;
        using (HistoryManagerSQLite m = new HistoryManagerSQLite(rawDb)) rawItems = LoadWith(m, rawDb);
        rawItems.Reverse();
        result["QueriesRaw"] = Queries(cases, rawItems);
        result["Formats"] = Formats(cases);
        result["Columns"] = Columns(cases);
        result["Transactions"] = ProbeTransactions(work);

        File.WriteAllText(Path.Combine(work, "sharex.json"), result.ToString());
    }

    static JObject Queries(JObject cases, List<HistoryItem> items)
    {
        JObject q = new JObject();
        // HistoryWindow.RefreshHistoryAsync's lists (window code, copied).
        q["Types"] = new JArray(items.Select(item => item.Type).Where(type => !string.IsNullOrWhiteSpace(type))
            .Distinct(StringComparer.OrdinalIgnoreCase).OrderBy(type => type).ToArray());
        q["Hosts"] = new JArray(items.Select(item => item.Host).Where(host => !string.IsNullOrWhiteSpace(host))
            .Distinct(StringComparer.OrdinalIgnoreCase).OrderBy(host => host).ToArray());
        q["ProcessNames"] = new JArray(HistoryHelpers.GetProcessNames(items));

        JArray filters = new JArray();
        foreach (JObject f in cases["filters"])
        {
            HistoryFilter filter = new HistoryFilter
            {
                Filename = (string)f["Filename"],
                URL = (string)f["URL"],
                FilterDate = (bool?)f["FilterDate"] ?? false,
                FromDate = f["FromDate"] != null ? SpecDate(f["FromDate"]) : default,
                ToDate = f["ToDate"] != null ? SpecDate(f["ToDate"]) : default,
                FilterType = (bool?)f["FilterType"] ?? false,
                Type = (string)f["Type"],
                FilterHost = (bool?)f["FilterHost"] ?? false,
                Host = (string)f["Host"],
                FilterFavorites = (bool?)f["FilterFavorites"] ?? false,
                MaxItemCount = (int?)f["MaxItemCount"] ?? 0,
                SearchInTags = (bool?)f["SearchInTags"] ?? true,
            };
            filters.Add(new JArray(filter.ApplyFilter(items).Select(i => i.Id)));
        }
        q["Filters"] = filters;

        // ImageHistoryWindow.ApplyFilterAsync (window code, copied), files "existing" by the case's list.
        JArray imageFilters = new JArray();
        foreach (JObject f in cases["image_filters"])
        {
            string search = ((string)f["search"])?.Trim() ?? string.Empty;
            bool favorites = (bool)f["favorites"], mediaOnly = (bool)f["media_only"], filterMissing = (bool)f["filter_missing"];
            HashSet<string> existing = new HashSet<string>(cases["existing_files"].Values<string>());
            System.Text.RegularExpressions.Regex regex = null;
            if (!string.IsNullOrEmpty(search))
            {
                string pattern = System.Text.RegularExpressions.Regex.Escape(search).Replace("\\?", ".").Replace("\\*", ".*");
                regex = new System.Text.RegularExpressions.Regex(pattern, System.Text.RegularExpressions.RegexOptions.IgnoreCase | System.Text.RegularExpressions.RegexOptions.CultureInvariant);
            }
            List<long> ids = new List<long>();
            foreach (HistoryItem item in items)
            {
                if (string.IsNullOrWhiteSpace(item.FilePath)) continue;
                if (favorites && !item.Favorite) continue;
                if (mediaOnly && !ShareX.HelpersLib.FileHelpers.IsImageFile(item.FilePath) && !ShareX.HelpersLib.FileHelpers.IsVideoFile(item.FilePath)) continue;
                if (filterMissing && !existing.Contains(item.FilePath)) continue;
                if (regex != null && !regex.IsMatch(item.FileName ?? string.Empty) &&
                    !(item.Tags?.Values.Any(value => !string.IsNullOrEmpty(value) && regex.IsMatch(value)) == true)) continue;
                ids.Add(item.Id);
            }
            imageFilters.Add(new JArray(ids));
        }
        q["ImageFilters"] = imageFilters;

        q["OutputStats"] = HistoryHelpers.OutputStats(items);

        JArray stats = new JArray();
        foreach (JObject s in cases["stats"])
        {
            DateTime now = SpecDate(s["now"]);
            HistoryStats st = HistoryStats.Calculate(items, now, (DayOfWeek)(int)s["first_day"]);
            stats.Add(DumpStats(st));
        }
        q["Stats"] = stats;
        return q;
    }

    static JObject Tally(HistoryStatsTally t) => new JObject { ["Images"] = t.Images, ["Recordings"] = t.Recordings, ["Other"] = t.Other };
    static JArray Entries(IReadOnlyList<HistoryStatsEntry> e) => new JArray(e.Select(x => new JArray(x.Name, x.Count)));
    static JToken Ticks(DateTime? d) => d is DateTime v ? new JValue(v.Ticks) : JValue.CreateNull();

    static JObject DumpStats(HistoryStats st)
    {
        Type chart = Lib.GetType("ShareX.HistoryLib.StatsHourChart");
        Type calendar = Lib.GetType("ShareX.HistoryLib.StatsCalendar");
        JObject o = new JObject
        {
            ["Today"] = st.Today.Ticks,
            ["Total"] = Tally(st.Total),
            ["Daily"] = new JArray(st.Daily.Select(kv => new JArray(kv.Key.Ticks, Tally(kv.Value)))),
            ["Hours"] = new JArray(st.Hours.Select(Tally)),
            ["Weekdays"] = new JArray(st.Weekdays.Select(Tally)),
            ["FirstDay"] = Ticks(st.FirstDay),
            ["Apps"] = Entries(st.Apps),
            ["Destinations"] = Entries(st.Destinations),
            ["FileTypes"] = Entries(st.FileTypes),
            ["TodayTally"] = Tally(st.TodayTally),
            ["Past7Days"] = st.Past7Days,
            ["Previous7Days"] = st.Previous7Days,
            ["Streak"] = st.Streak,
            ["LongestStreak"] = st.LongestStreak,
            ["LongestStreakEnd"] = st.LongestStreakEnd.Ticks,
            ["BusiestDay"] = Ticks(st.BusiestDay),
            ["BusiestDayCount"] = st.BusiestDayCount,
            ["DaysCounted"] = st.DaysCounted,
        };
        MethodInfo peak = chart.GetMethod("GetPeakHour", Any);
        o["PeakHour"] = new JArray(new[] { HistoryStatsFilter.All, HistoryStatsFilter.Images, HistoryStatsFilter.Recordings }
            .Select(f => peak.Invoke(null, new object[] { st, f }) is int h ? new JValue(h) : JValue.CreateNull()));
        // StatsWindow.ShowCalendarSpans' spans and GetRange for each.
        List<int> spans = new List<int> { 0 };
        int firstYear = st.FirstDay?.Year ?? st.Today.Year;
        if (firstYear < st.Today.Year) { spans.Add(-1); for (int y = st.Today.Year; y >= firstYear; y--) spans.Add(y); }
        MethodInfo range = calendar.GetMethod("GetRange", Any);
        o["Ranges"] = new JArray(spans.Select(span =>
        {
            object r = range.Invoke(null, new object[] { st, span });
            if (r == null) return new JArray(span, JValue.CreateNull());
            (DateTime from, DateTime to) = ((DateTime, DateTime))r;
            int active = st.CountDays(from, to, t => t.Total > 0);
            return new JArray(span, new JArray(from.Ticks, to.Ticks, active));
        }));
        o["WeekStart"] = st.GetWeekStart(st.Today).Ticks;
        return o;
    }

    static JObject Formats(JObject cases)
    {
        Type fmt = Lib.GetType("ShareX.HistoryLib.StatsFormat");
        Type window = Lib.GetType("ShareX.HistoryLib.StatsWindow");
        Type chart = Lib.GetType("ShareX.HistoryLib.StatsHourChart");
        Type calendar = Lib.GetType("ShareX.HistoryLib.StatsCalendar");
        object Call(Type t, string name, params object[] a)
        {
            MethodInfo m = t.GetMethods(Any).First(x => x.Name == name && x.GetParameters().Length == a.Length &&
                x.GetParameters().Select(p => p.ParameterType).SequenceEqual(a.Select(v => v.GetType())));
            return m.Invoke(null, a);
        }
        JObject o = new JObject();
        o["Share"] = new JArray(cases["formats"]["share"].Select(p => (string)Call(fmt, "Share", (int)p[0], (int)p[1])));
        o["Number"] = new JArray(cases["formats"]["number"].Select(n => (string)Call(fmt, "Number", (int)n)));
        o["Compact"] = new JArray(cases["formats"]["compact"].Select(n => (string)Call(fmt, "Compact", (double)n)));
        o["Hour"] = new JArray(Enumerable.Range(0, 25).Select(h => (string)Call(fmt, "HourSpan", h)));
        o["Rate"] = new JArray(cases["formats"]["rate"].Select(p => (string)Call(window, "GetRate", (int)p[0], (int)p[1])));
        o["Scale"] = new JArray(cases["formats"]["scale"].Select(n =>
        {
            (double step, double top) = ((double, double))Call(chart, "GetScale", (int)n);
            return new JArray(step, top);
        }));
        o["Level"] = new JArray(cases["formats"]["level"].Select(p =>
            (int)Call(calendar, "GetLevel", (int)p[0], p[1].Values<int>().ToArray())));
        // StatsWindow.UpdateWeekChange's text (instance code, copied).
        o["WeekChange"] = new JArray(cases["formats"]["week_change"].Select(p =>
        {
            int past = (int)p[0], previous = (int)p[1];
            if (previous == 0) return JValue.CreateNull();
            double change = (past - previous) * 100.0 / previous;
            bool up = change >= 0;
            return new JValue((up ? "▲ " : "▼ ") + Math.Abs(change).ToString("0", CultureInfo.CurrentCulture) + "%");
        }));
        return o;
    }

    static JArray Columns(JObject cases)
    {
        JArray results = new JArray();
        foreach (JToken c in cases["columns"])
        {
            HistoryColumnLayout layout = new HistoryColumnLayout();
            layout.Load(c.Type == JTokenType.Null ? null : c.ToObject<List<HistoryColumnSettings>>());
            results.Add(JArray.FromObject(layout.ToSettings()));
        }
        return results;
    }

    // Microsoft.Data.Sqlite's BeginTransaction: BEGIN IMMEDIATE (another
    // writer is refused at once) or BEGIN (deferred, a reader until it writes)?
    static JObject ProbeTransactions(string work)
    {
        string probe = Path.Combine(work, "sharex-probe.db");
        new HistoryManagerSQLite(probe).Dispose();
        using SqliteConnection a = new SqliteConnection("Data Source=" + probe + ";Pooling=False");
        using SqliteConnection b = new SqliteConnection("Data Source=" + probe + ";Pooling=False;Default Timeout=1");
        a.Open();
        b.Open();
        using (SqliteCommand t = new SqliteCommand("PRAGMA busy_timeout = 0;", b)) t.ExecuteNonQuery();
        using SqliteTransaction tx = a.BeginTransaction();
        string other;
        try
        {
            using SqliteCommand cmd = new SqliteCommand("BEGIN IMMEDIATE; ROLLBACK;", b);
            cmd.ExecuteNonQuery();
            other = "acquired";
        }
        catch (SqliteException e)
        {
            other = "busy:" + e.SqliteErrorCode;
        }
        tx.Rollback();
        string journal;
        using (SqliteCommand j = new SqliteCommand("PRAGMA journal_mode;", a)) journal = (string)j.ExecuteScalar();
        return new JObject { ["SecondWriterDuringBeginTransaction"] = other, ["JournalMode"] = journal };
    }
}
