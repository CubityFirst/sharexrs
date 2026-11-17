// Runs ShareX's own FFmpeg recorder code over cases.json and writes what it
// would run (see README.md). Nothing is recorded and FFmpeg never starts:
// Harmony replaces FFmpegCLIManager.Run (the command line is captured),
// DesktopScreen.AllScreens (the case's monitors), RegistryHelpers'
// CreateRegistry (values captured, the registry untouched) and
// FileHelpers.CreateDirectoryFromFilePath (no folders created).
//
//   RecordingHarness <cases.json> <out.json> <work dir>

using System.Diagnostics;
using System.Drawing;
using System.Globalization;
using System.Reflection;
using System.Runtime.CompilerServices;
using HarmonyLib;
using Microsoft.Win32;
using Newtonsoft.Json;
using Newtonsoft.Json.Converters;
using Newtonsoft.Json.Linq;
using ShareX.HelpersLib;
using ShareX.ScreenCaptureLib;

static class Captured
{
    public static List<string> Runs = new();
    public static List<object[]> Registry = new();
    public static List<string> ListFiles = new();
    public static DesktopScreen[] Screens = Array.Empty<DesktopScreen>();
    public static string CannedOutput;
}

static class Patches
{
    public static bool Run(FFmpegCLIManager __instance, string args, ref bool __result)
    {
        Captured.Runs.Add(args);
        // ConcatenateVideos' list file exists only during the run.
        int i = args.IndexOf("-i \"", StringComparison.Ordinal);
        if (args.StartsWith("-f concat", StringComparison.Ordinal) && i >= 0)
        {
            string list = args.Substring(i + 4, args.IndexOf('"', i + 4) - i - 4);
            Captured.ListFiles.Add(File.Exists(list) ? File.ReadAllText(list) : null);
        }
        if (Captured.CannedOutput != null) __instance.Output.Append(Captured.CannedOutput);
        __result = true;
        return false;
    }

    public static bool AllScreens(ref DesktopScreen[] __result)
    {
        __result = Captured.Screens;
        return false;
    }

    public static bool CreateRegistry(string path, string name, int value)
    {
        Captured.Registry.Add(new object[] { path, name, value });
        return false;
    }

    public static bool NoDirectory() => false;
}

static class Program
{
    static readonly JsonSerializerSettings SettingsLike = new()
    {
        Converters = { new StringEnumConverter() },
        // SettingsBase.Serializer_Error: a value that doesn't convert keeps its default.
        Error = (_, e) => e.ErrorContext.Handled = true
    };

    static int Main(string[] argv)
    {
        CultureInfo.DefaultThreadCurrentCulture = CultureInfo.InvariantCulture;
        string casesPath = argv[0], outPath = argv[1], work = argv[2];
        Directory.CreateDirectory(work);

        Harmony harmony = new("sharexrs.recording-harness");
        harmony.Patch(typeof(FFmpegCLIManager).GetMethod("Run", new[] { typeof(string) }),
            prefix: new HarmonyMethod(typeof(Patches).GetMethod(nameof(Patches.Run))));
        harmony.Patch(typeof(DesktopScreen).GetProperty("AllScreens").GetGetMethod(),
            prefix: new HarmonyMethod(typeof(Patches).GetMethod(nameof(Patches.AllScreens))));
        harmony.Patch(typeof(RegistryHelpers).GetMethod("CreateRegistry", new[] { typeof(string), typeof(string), typeof(int), typeof(RegistryHive) }),
            prefix: new HarmonyMethod(typeof(Patches).GetMethod(nameof(Patches.CreateRegistry))));
        harmony.Patch(typeof(FileHelpers).GetMethod("CreateDirectoryFromFilePath"),
            prefix: new HarmonyMethod(typeof(Patches).GetMethod(nameof(Patches.NoDirectory))));

        JArray cases = JArray.Parse(File.ReadAllText(casesPath));
        JArray results = new();
        foreach (JObject c in cases)
        {
            Captured.Runs.Clear();
            Captured.Registry.Clear();
            Captured.ListFiles.Clear();
            Captured.CannedOutput = null;
            Captured.Screens = (c["screens"] as JArray ?? new JArray()).Select(MakeScreen).ToArray();
            JObject r = new() { ["name"] = c["name"] };
            try
            {
                Run(c, r, work);
            }
            catch (Exception e)
            {
                r["error"] = e.GetType().Name + ": " + e.Message;
            }
            r["runs"] = JArray.FromObject(Captured.Runs);
            r["registry"] = JArray.FromObject(Captured.Registry);
            if (Captured.ListFiles.Count > 0) r["lists"] = JArray.FromObject(Captured.ListFiles);
            results.Add(r);
        }
        File.WriteAllText(outPath, results.ToString(Formatting.Indented));
        Console.WriteLine($"{results.Count} cases -> {outPath}");
        return 0;
    }

    static ScreenRecordingOptions MakeOptions(JObject c)
    {
        JObject o = (JObject)c["options"];
        JToken area = o["area"];
        return new ScreenRecordingOptions
        {
            IsRecording = (bool?)o["is_recording"] ?? false,
            IsLossless = (bool?)o["is_lossless"] ?? false,
            InputPath = (string)o["input_path"],
            OutputPath = (string)o["output_path"],
            FPS = (int?)o["fps"] ?? 0,
            CaptureArea = area == null ? Rectangle.Empty : new Rectangle((int)area[0], (int)area[1], (int)area[2], (int)area[3]),
            Duration = (float?)o["duration"] ?? 0f,
            DrawCursor = (bool?)o["draw_cursor"] ?? false,
            FFmpeg = JsonConvert.DeserializeObject<FFmpegOptions>((o["ffmpeg"] ?? new JObject()).ToString(), SettingsLike)
        };
    }

    static DesktopScreen MakeScreen(JToken s)
    {
        DesktopScreen screen = (DesktopScreen)RuntimeHelpers.GetUninitializedObject(typeof(DesktopScreen));
        JArray b = (JArray)s["bounds"];
        SetField(screen, "<Bounds>k__BackingField", new Rectangle((int)b[0], (int)b[1], (int)b[2], (int)b[3]));
        SetField(screen, "<Primary>k__BackingField", (bool)s["primary"]);
        return screen;
    }

    static void SetField(object o, string name, object value) =>
        o.GetType().GetField(name, BindingFlags.Instance | BindingFlags.NonPublic).SetValue(o, value);

    static void Run(JObject c, JObject r, string work)
    {
        string kind = (string)c["kind"];
        switch (kind)
        {
            case "record":
            {
                // What ScreenRecordManager does per segment: a ScreenRecorder, then StartRecording.
                ScreenRecordingOptions options = MakeOptions(c);
                r["extension"] = options.FFmpeg.Extension;
                r["even"] = options.FFmpeg.IsEvenSizeRequired;
                r["source_selected"] = options.FFmpeg.IsSourceSelected;
                ScreenRecorder recorder = new(ScreenRecordOutput.FFmpeg, options, null, options.CaptureArea);
                recorder.StartRecording();
                break;
            }
            case "commands":
                r["result"] = MakeOptions(c).GetFFmpegCommands();
                break;
            case "custom_args":
                r["result"] = MakeOptions(c).GetFFmpegArgs(true);
                break;
            case "encode_video":
            {
                ScreenRecordingOptions options = MakeOptions(c);
                ScreenRecorder recorder = new(ScreenRecordOutput.FFmpeg, options, null, options.CaptureArea);
                recorder.FFmpegEncodeVideo((string)c["input"], (string)c["output"]);
                break;
            }
            case "encode_gif":
            {
                ScreenRecordingOptions options = MakeOptions(c);
                ScreenRecorder recorder = new(ScreenRecordOutput.FFmpeg, options, null, options.CaptureArea);
                recorder.FFmpegEncodeAsGIF((string)c["input"], (string)c["output"]);
                break;
            }
            case "concat":
            {
                using FFmpegCLIManager ffmpeg = new("ffmpeg.exe");
                string output = Path.Combine(work, (string)c["output"]);
                string[] inputs = ((JArray)c["inputs"]).Select(x => (string)x).ToArray();
                ffmpeg.ConcatenateVideos(inputs, output, false);
                r["list_file"] = output + ".txt";
                break;
            }
            case "dshow":
            {
                using FFmpegCLIManager ffmpeg = new("ffmpeg.exe");
                Captured.CannedOutput = (string)c["output"];
                DirectShowDevices d = ffmpeg.GetDirectShowDevices();
                r["video"] = JArray.FromObject(d.VideoDevices);
                r["audio"] = JArray.FromObject(d.AudioDevices);
                break;
            }
            case "progress":
            {
                // The lines FFmpeg writes during a second pass, through ScreenRecorder's events.
                ScreenRecordingOptions options = MakeOptions(c);
                ScreenRecorder recorder = new(ScreenRecordOutput.FFmpeg, options, null, options.CaptureArea);
                FFmpegCLIManager ffmpeg = (FFmpegCLIManager)typeof(ScreenRecorder)
                    .GetField("ffmpeg", BindingFlags.Instance | BindingFlags.NonPublic).GetValue(recorder);
                ffmpeg.TrackEncodeProgress = (bool?)c["track"] ?? true;
                JArray events = new();
                recorder.RecordingStarted += () => events.Add("started");
                recorder.EncodingProgressChanged += p => events.Add("progress " + p);
                ffmpeg.EncodeProgressChanged += p => events.Add("raw " + p.ToString("R", CultureInfo.InvariantCulture));
                MethodInfo received = typeof(FFmpegCLIManager).GetMethod("FFmpeg_DataReceived", BindingFlags.Instance | BindingFlags.NonPublic);
                ConstructorInfo ctor = typeof(DataReceivedEventArgs).GetConstructor(BindingFlags.Instance | BindingFlags.NonPublic, new[] { typeof(string) });
                foreach (JToken line in (JArray)c["lines"])
                {
                    received.Invoke(ffmpeg, new object[] { null, ctor.Invoke(new object[] { (string)line }) });
                }
                r["events"] = events;
                r["output"] = ffmpeg.Output.ToString();
                r["duration_ticks"] = ffmpeg.VideoDuration.Ticks;
                break;
            }
            default:
                throw new InvalidOperationException("unknown kind " + kind);
        }
    }
}
