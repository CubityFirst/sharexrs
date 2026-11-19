// The time zone half of tools/settings-harness: what Task settings › File
// naming › Time zone lists (TimeZoneInfo.GetSystemTimeZones()), the JSON
// ShareX writes for the chosen zone (SettingsBase.SaveToStream's Newtonsoft
// settings), what loading that JSON gives back (LoadInternal's settings), and
// each zone's UTC offsets over 1970-2040 (TimeZoneInfo.ConvertTimeFromUtc, as
// NameParser's ConvertTime uses it), for ShareXrs's tests.
//
//   SettingsHarness --time-zones <out.json>
//
// Reads the registry through .NET only; writes only <out.json>.

using System.Globalization;
using System.Text.Json.Nodes;
using Microsoft.Win32;
using Newtonsoft.Json;
using Newtonsoft.Json.Converters;
using ShareX.HelpersLib;

/// The field ShareX's TaskSettingsUpload has.
sealed class TimeZoneHolder
{
    public TimeZoneInfo CustomTimeZone = TimeZoneInfo.Utc;
}

static class TimeZoneHarness
{
    static readonly DateTime ScanStart = new(1970, 1, 1, 0, 0, 0, DateTimeKind.Utc);
    static readonly DateTime ScanEnd = new(2040, 1, 1, 0, 0, 0, DateTimeKind.Utc);

    /// SettingsBase.SaveToStream (without DPAPI): the holder's JSON.
    static string Save(TimeZoneHolder holder)
    {
        JsonSerializer serializer = new()
        {
            ContractResolver = new WritablePropertiesOnlyResolver(),
            DateTimeZoneHandling = DateTimeZoneHandling.Utc,
            Formatting = Formatting.Indented,
        };
        serializer.Converters.Add(new StringEnumConverter());
        using StringWriter sw = new();
        using (JsonTextWriter jw = new(sw))
        {
            serializer.Serialize(jw, holder);
        }
        return sw.ToString();
    }

    /// SettingsBase.LoadInternal's serializer (its error handler included).
    static TimeZoneHolder Load(string json)
    {
        JsonSerializer serializer = new()
        {
            ContractResolver = new DPAPIEncryptedStringPropertyResolver(),
            DateTimeZoneHandling = DateTimeZoneHandling.Local,
            ObjectCreationHandling = ObjectCreationHandling.Replace,
        };
        serializer.Converters.Add(new StringEnumConverter());
        serializer.Error += (_, e) =>
        {
            if (e.ErrorContext.Error.Message.StartsWith("Error converting value"))
            {
                e.ErrorContext.Handled = true;
            }
        };
        using JsonTextReader reader = new(new StringReader(json));
        return serializer.Deserialize<TimeZoneHolder>(reader);
    }

    static long Offset(TimeZoneInfo zone, DateTime utc) => (long)(TimeZoneInfo.ConvertTimeFromUtc(utc, zone) - utc).TotalMinutes;

    /// The offset at the scan's start, then each change: the first tick
    /// (UTC, DateTime ticks) with the new offset, found hourly, then to the tick.
    static JsonArray Transitions(TimeZoneInfo zone)
    {
        JsonArray list = new();
        long previous = Offset(zone, ScanStart);
        list.Add(new JsonArray(ScanStart.Ticks, previous));
        for (DateTime t = ScanStart.AddHours(1); t <= ScanEnd; t = t.AddHours(1))
        {
            long offset = Offset(zone, t);
            if (offset == previous)
            {
                continue;
            }
            long lo = t.Ticks - TimeSpan.TicksPerHour, hi = t.Ticks;
            while (hi - lo > 1)
            {
                long mid = lo + (hi - lo) / 2;
                if (Offset(zone, new DateTime(mid, DateTimeKind.Utc)) == previous) lo = mid; else hi = mid;
            }
            list.Add(new JsonArray(hi, offset));
            previous = offset;
        }
        return list;
    }

    /// The json under "CustomTimeZone", as the settings file has it.
    static string ZoneJson(string holderJson)
    {
        Newtonsoft.Json.Linq.JObject o = Newtonsoft.Json.Linq.JObject.Parse(holderJson);
        return o["CustomTimeZone"].ToString(Formatting.None);
    }

    static string Machine()
    {
        using RegistryKey cv = Registry.LocalMachine.OpenSubKey(@"SOFTWARE\Microsoft\Windows NT\CurrentVersion");
        return $"{cv?.GetValue("CurrentBuild")}.{cv?.GetValue("UBR")} {CultureInfo.CurrentUICulture.Name}";
    }

    /// `HARNESS_LOCAL_TZ=<id>`: the reload as on a machine in that zone
    /// (TimeZoneInfo's cached local zone replaced in this process only; the
    /// system's zone isn't touched).
    static void OverrideLocal()
    {
        string id = Environment.GetEnvironmentVariable("HARNESS_LOCAL_TZ");
        if (string.IsNullOrEmpty(id))
        {
            return;
        }
        TimeZoneInfo zone = TimeZoneInfo.FindSystemTimeZoneById(id);
        _ = TimeZoneInfo.Local;
        var flags = System.Reflection.BindingFlags.NonPublic | System.Reflection.BindingFlags.Static | System.Reflection.BindingFlags.Instance;
        object cached = typeof(TimeZoneInfo).GetField("s_cachedData", flags).GetValue(null);
        cached.GetType().GetField("_localTimeZone", flags).SetValue(cached, zone);
        Console.WriteLine($"local zone for the reload: {TimeZoneInfo.Local.Id}");
    }

    public static int Run(string outPath)
    {
        OverrideLocal();
        JsonArray zones = new();
        var system = TimeZoneInfo.GetSystemTimeZones();
        foreach (TimeZoneInfo zone in system)
        {
            string saved = Save(new TimeZoneHolder { CustomTimeZone = zone });
            JsonObject z = new()
            {
                ["id"] = zone.Id,
                ["displayName"] = zone.DisplayName,
                ["json"] = ZoneJson(saved),
                ["transitions"] = Transitions(zone),
            };
            try
            {
                TimeZoneInfo loaded = Load(saved).CustomTimeZone;
                string again = ZoneJson(Save(new TimeZoneHolder { CustomTimeZone = loaded }));
                JsonArray loadedTransitions = Transitions(loaded);
                bool sameOffsets = loadedTransitions.ToJsonString() == z["transitions"].ToJsonString();
                z["loaded"] = new JsonObject
                {
                    ["sameJson"] = again == z["json"].GetValue<string>(),
                    ["json"] = again,
                    // ObjectCombo's EqualityComparer<TimeZoneInfo>.Default: the
                    // list item a reloaded zone selects.
                    ["selects"] = system.FirstOrDefault(x => EqualityComparer<TimeZoneInfo>.Default.Equals(x, loaded))?.Id,
                    ["sameOffsets"] = sameOffsets,
                    ["transitions"] = sameOffsets ? null : loadedTransitions,
                };
            }
            catch (Exception e)
            {
                z["loaded"] = new JsonObject { ["error"] = e.GetType().Name, ["message"] = e.Message };
            }
            zones.Add(z);
        }
        JsonObject result = new()
        {
            ["machine"] = Machine(),
            ["local"] = ZoneJson(Save(new TimeZoneHolder { CustomTimeZone = TimeZoneInfo.Local })),
            ["utc"] = ZoneJson(Save(new TimeZoneHolder())),
            ["zones"] = zones,
        };
        File.WriteAllText(outPath, result.ToJsonString(new System.Text.Json.JsonSerializerOptions { WriteIndented = false, Encoder = System.Text.Encodings.Web.JavaScriptEncoder.UnsafeRelaxedJsonEscaping }) + "\n");
        Console.WriteLine($"{zones.Count} time zones -> {outPath}");
        return 0;
    }
}
