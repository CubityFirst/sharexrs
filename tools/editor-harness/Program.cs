// Renders image editor annotations with ShareX's own code, headless.
//
//     EditorHarness <cases.json> <out dir> [case name filter]
//
// For each case in cases.json it opens ShareX's EditorWindow on Avalonia's
// headless platform (Skia drawing, no OS window), loads the base image, adds
// the case's annotations to the editor's EditorCore as ShareX's own undo/redo
// does (AnnotationsRestored rebuilds the annotation layer through
// AnnotationVisualFactory), and saves what the editor exports
// (EditorView.GetSnapshot, which renders the visual tree with
// RenderTargetBitmap) as <out dir>/<case>.png.
//
// The annotations are ShareX's own JSON (System.Text.Json with the
// [JsonDerivedType] discriminators on Annotation, "$type": "Rectangle", ...).

using System.Reflection;
using System.Text.Json;
using System.Text.Json.Nodes;
using System.Text.Json.Serialization;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Headless;
using Avalonia.Markup.Xaml.Styling;
using Avalonia.Themes.Fluent;
using Avalonia.Threading;
using Avalonia.VisualTree;
using ShareX.AvaloniaUI.Theming;
using ShareX.ImageEditor.Core.Annotations;
using ShareX.ImageEditor.Core.Editor;
using ShareX.ImageEditor.Integration;
using ShareX.ImageEditor.Presentation.ViewModels;
using ShareX.ImageEditor.Presentation.Views;
using SkiaSharp;

namespace EditorHarness;

internal sealed class HarnessApp : Application
{
    public override void Initialize()
    {
        // ShareX.ImageEditor.App's App.axaml, in code.
        Uri baseUri = new("avares://EditorHarness/");
        Resources.MergedDictionaries.Add(new ResourceInclude(baseUri)
        {
            Source = new Uri("avares://ShareX.Avalonia/Theming/ShareXTheme.axaml")
        });
        Styles.Add(new FluentTheme());
        Styles.Add(new StyleInclude(baseUri) { Source = new Uri("avares://ShareX.Avalonia/Theming/ControlStyles.axaml") });
        Styles.Add(new StyleInclude(baseUri) { Source = new Uri("avares://ShareX.Avalonia/Theming/ToolStyles.axaml") });
        Styles.Add(new StyleInclude(baseUri) { Source = new Uri("avares://ShareX.ImageEditor/Presentation/Theming/EditorStyles.axaml") });
    }

    public override void OnFrameworkInitializationCompleted()
    {
        ThemeManager.Configure(new ApplicationThemeOptions());
        base.OnFrameworkInitializationCompleted();
    }
}

internal static class Program
{
    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        PropertyNameCaseInsensitive = true,
        Converters = { new JsonStringEnumConverter(), new SKPointConverter() },
        AllowOutOfOrderMetadataProperties = true,
    };

    [STAThread]
    public static int Main(string[] args)
    {
        if (args.Length < 2)
        {
            Console.Error.WriteLine("usage: EditorHarness <cases.json> <out dir> [case name filter]");
            return 2;
        }

        string casesPath = Path.GetFullPath(args[0]);
        string outDir = Path.GetFullPath(args[1]);
        string? filter = args.Length > 2 ? args[2] : null;
        Directory.CreateDirectory(outDir);

        AppBuilder.Configure<HarnessApp>()
            .UseSkia()
            .UseHeadless(new AvaloniaHeadlessPlatformOptions { UseHeadlessDrawing = false })
            .WithInterFont()
            .SetupWithoutStarting();

        JsonObject root = JsonNode.Parse(File.ReadAllText(casesPath))!.AsObject();
        string basePath = Path.Combine(Path.GetDirectoryName(casesPath)!, root["base"]!.GetValue<string>());

        int failures = 0;
        foreach (JsonNode? node in root["cases"]!.AsArray())
        {
            string name = node!["name"]!.GetValue<string>();
            if (filter != null && !name.Contains(filter, StringComparison.OrdinalIgnoreCase))
            {
                continue;
            }

            try
            {
                using SKBitmap baseImage = SKBitmap.Decode(basePath) ?? throw new InvalidOperationException($"can't read {basePath}");
                List<Annotation> annotations = node["annotations"]!.AsArray()
                    .Select(a => a!.Deserialize<Annotation>(JsonOptions)!)
                    .ToList();

                using SKBitmap snapshot = Render(baseImage, annotations);
                using FileStream file = File.Create(Path.Combine(outDir, name + ".png"));
                snapshot.Encode(file, SKEncodedImageFormat.Png, 100);
                Console.WriteLine($"{name}: {snapshot.Width}x{snapshot.Height}");
            }
            catch (Exception ex)
            {
                failures++;
                Console.Error.WriteLine($"{name}: FAILED {ex}");
            }
        }

        return failures == 0 ? 0 : 1;
    }

    private static SKBitmap Render(SKBitmap baseImage, List<Annotation> annotations)
    {
        ImageEditorOptions options = new() { ShowExitConfirmation = false, RememberWindowState = false };
        EditorWindow window = new(options);
        try
        {
            window.Show();
            Pump();

            if (window.DataContext is MainViewModel vm)
            {
                vm.ShowStartScreen = false;
            }

            window.LoadImage(baseImage.Copy());
            Pump();

            EditorView view = window.GetVisualDescendants().OfType<EditorView>().First();
            EditorCore core = (EditorCore)typeof(EditorView)
                .GetField("_editorCore", BindingFlags.NonPublic | BindingFlags.Instance)!
                .GetValue(view)!;

            foreach (Annotation annotation in annotations)
            {
                core.AddAnnotation(annotation);
            }

            // What undo/redo does: rebuild the annotation layer from EditorCore
            // (AnnotationVisualFactory, effect bitmaps, the spotlight overlay).
            typeof(EditorView)
                .GetMethod("OnAnnotationsRestored", BindingFlags.NonPublic | BindingFlags.Instance)!
                .Invoke(view, null);
            Pump();

            return view.GetSnapshot() ?? throw new InvalidOperationException("GetSnapshot returned null");
        }
        finally
        {
            window.Close();
            Pump();
        }
    }

    private static void Pump()
    {
        for (int i = 0; i < 4; i++)
        {
            Dispatcher.UIThread.RunJobs();
            AvaloniaHeadlessPlatform.ForceRenderTimerTick();
        }
        Dispatcher.UIThread.RunJobs();
    }
}

/// <summary>SKPoint as {"X": .., "Y": ..}.</summary>
internal sealed class SKPointConverter : JsonConverter<SKPoint>
{
    public override SKPoint Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options)
    {
        if (reader.TokenType == JsonTokenType.StartArray)
        {
            reader.Read();
            float ax = reader.GetSingle();
            reader.Read();
            float ay = reader.GetSingle();
            reader.Read();
            return new SKPoint(ax, ay);
        }

        float x = 0, y = 0;
        while (reader.Read() && reader.TokenType != JsonTokenType.EndObject)
        {
            string prop = reader.GetString()!;
            reader.Read();
            if (prop.Equals("X", StringComparison.OrdinalIgnoreCase)) x = reader.GetSingle();
            else if (prop.Equals("Y", StringComparison.OrdinalIgnoreCase)) y = reader.GetSingle();
            else reader.Skip();
        }
        return new SKPoint(x, y);
    }

    public override void Write(Utf8JsonWriter writer, SKPoint value, JsonSerializerOptions options)
    {
        writer.WriteStartObject();
        writer.WriteNumber("X", value.X);
        writer.WriteNumber("Y", value.Y);
        writer.WriteEndObject();
    }
}
