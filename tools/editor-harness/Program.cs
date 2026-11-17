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
            Console.Error.WriteLine("       EditorHarness --text <family> <size> <bold 0|1> <text>");
            return 2;
        }

        if (args[0] == "--text")
        {
            AppBuilder.Configure<HarnessApp>()
                .UseSkia()
                .UseHeadless(new AvaloniaHeadlessPlatformOptions { UseHeadlessDrawing = false })
                .WithInterFont()
                .SetupWithoutStarting();
            return DumpText(args[1], double.Parse(args[2], System.Globalization.CultureInfo.InvariantCulture), args[3] == "1", args[4]);
        }

        if (args[0] == "--fonts")
        {
            // The fonts Avalonia.Fonts.Inter bundles (FontFamily.Default with WithInterFont),
            // and how SKTypeface.Default measures (NumberAnnotation.CalculateRadius).
            AppBuilder.Configure<HarnessApp>()
                .UseSkia()
                .UseHeadless(new AvaloniaHeadlessPlatformOptions { UseHeadlessDrawing = false })
                .WithInterFont()
                .SetupWithoutStarting();
            Directory.CreateDirectory(args[1]);
            foreach (Uri asset in Avalonia.Platform.AssetLoader.GetAssets(new Uri("avares://Avalonia.Fonts.Inter/Assets"), null))
            {
                using Stream s = Avalonia.Platform.AssetLoader.Open(asset);
                using FileStream f = File.Create(Path.Combine(args[1], Path.GetFileName(asset.AbsolutePath)));
                s.CopyTo(f);
                Console.WriteLine(asset);
            }
            var inv = System.Globalization.CultureInfo.InvariantCulture;
            Console.WriteLine("default typeface " + SKTypeface.Default.FamilyName + " weight " + SKTypeface.Default.FontWeight);
            foreach (float size in new[] { 14.4f, 18f, 24f })
            {
                foreach (bool bold in new[] { false, true })
                {
                    using var font = new SKFont(SKTypeface.Default, size) { Embolden = bold };
                    var m = font.Metrics;
                    Console.WriteLine(string.Format(inv, "size {0} embolden {1} ascent {2:R} descent {3:R} 1:{4:R} 12:{5:R} 88:{6:R}",
                        size, bold, m.Ascent, m.Descent, font.MeasureText("1"), font.MeasureText("12"), font.MeasureText("88")));
                }
            }
            return 0;
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

    /// <summary>
    /// Prints how Avalonia lays out and outlines a string, as ShareX's text
    /// annotations do (FormattedText, BuildGeometry): the metrics, each
    /// glyph and its advance, and the Skia path of the geometry.
    /// </summary>
    private static int DumpText(string family, double size, bool bold, string text)
    {
        var inv = System.Globalization.CultureInfo.InvariantCulture;
        var typeface = family == "$Default"
            ? new Avalonia.Media.Typeface(Avalonia.Media.FontFamily.Default, Avalonia.Media.FontStyle.Normal, bold ? Avalonia.Media.FontWeight.Bold : Avalonia.Media.FontWeight.Normal)
            : new Avalonia.Media.Typeface(family, Avalonia.Media.FontStyle.Normal, bold ? Avalonia.Media.FontWeight.Bold : Avalonia.Media.FontWeight.Normal);
        var ft = new Avalonia.Media.FormattedText(text, System.Globalization.CultureInfo.CurrentUICulture,
            Avalonia.Media.FlowDirection.LeftToRight, typeface, size, Avalonia.Media.Brushes.Black);
        Console.WriteLine(string.Format(inv, "width {0:R} height {1:R} baseline {2:R} extent {3:R}", ft.Width, ft.Height, ft.Baseline, ft.Extent));
        var gt = typeface.GlyphTypeface;
        Console.WriteLine(string.Format(inv, "font {0} em {1} ascent {2} descent {3} linegap {4}", gt.FamilyName, gt.Metrics.DesignEmHeight, gt.Metrics.Ascent, gt.Metrics.Descent, gt.Metrics.LineGap));
        var geo = ft.BuildGeometry(new Avalonia.Point(0, 0));
        var impl = geo?.GetType().GetProperty("PlatformImpl", BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Instance)?.GetValue(geo);
        Console.WriteLine("geometry " + geo?.GetType().Name + " impl " + impl?.GetType().Name);
        DumpImpl(impl, inv);
        return 0;
    }

    private static void DumpImpl(object? impl, IFormatProvider inv)
    {
        if (impl == null) return;
        var children = impl.GetType().GetProperty("Children", BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Instance)?.GetValue(impl);
        if (children is System.Collections.IEnumerable list)
        {
            foreach (var c in list) DumpImpl(c, inv);
        }
        var transform = impl.GetType().GetProperty("Transform", BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Instance)?.GetValue(impl);
        if (transform != null) Console.WriteLine("transform " + transform);
        var source = impl.GetType().GetProperty("SourceGeometry", BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Instance)?.GetValue(impl);
        if (source != null) DumpImpl(source, inv);
        foreach (var name in new[] { "StrokePath", "FillPath" })
        {
            if (impl.GetType().GetProperty(name, BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Instance)?.GetValue(impl) is SKPath path)
            {
                Console.WriteLine(name + " filltype " + path.FillType);
                using var it = path.CreateRawIterator();
                var pts = new SKPoint[4];
                SKPathVerb verb;
                while ((verb = it.Next(pts)) != SKPathVerb.Done)
                {
                    int n = verb switch { SKPathVerb.Move => 1, SKPathVerb.Line => 2, SKPathVerb.Quad => 3, SKPathVerb.Conic => 3, SKPathVerb.Cubic => 4, _ => 0 };
                    var s = string.Join(" ", pts.Take(n).Select(p => string.Format(inv, "{0:R},{1:R}", p.X, p.Y)));
                    Console.WriteLine(verb + " " + s + (verb == SKPathVerb.Conic ? string.Format(inv, " w{0:R}", it.ConicWeight()) : ""));
                }
                break;
            }
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
