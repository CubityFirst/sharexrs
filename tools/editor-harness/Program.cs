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

/// <summary>
/// A wallpaper service that hands the editor a generated image (never the
/// desktop's), so the Wallpaper background's layouts can be compared.
/// </summary>
internal sealed class FakeWallpaper(string? path, DesktopWallpaperLayout layout) : IDesktopWallpaperService
{
    public bool IsSupported => path != null;
    public bool RequiresDesktopWallpaperPrewarm => false;

    public bool TryGetDesktopWallpaper(out DesktopWallpaperInfo? wallpaper)
    {
        wallpaper = path == null ? null : new DesktopWallpaperInfo { Path = path, Layout = layout };
        return wallpaper != null;
    }

    public void PrewarmDesktopWallpaper()
    {
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
    static int DumpToolTipTheme()
    {
        string[] keys =
        [
            "ToolTipBackground", "ToolTipForeground", "ToolTipBorderBrush", "ToolTipBorderThemeThickness",
            "ToolTipBorderThemePadding", "ToolTipContentThemeFontSize", "ToolTipContentMaxWidth", "OverlayCornerRadius",
            "ControlCornerRadius", "ToolTipBackgroundOpacity"
        ];
        foreach (Avalonia.Styling.ThemeVariant variant in new[] { Avalonia.Styling.ThemeVariant.Dark, Avalonia.Styling.ThemeVariant.Light })
        {
            foreach (string key in keys)
            {
                bool found = Application.Current!.TryGetResource(key, variant, out object? value);
                Console.WriteLine($"{variant} {key} = {(found ? Describe(value) : "(none)")}");
            }
        }
        if (Application.Current!.TryGetResource(typeof(ToolTip), null, out object? theme) && theme is Avalonia.Styling.ControlTheme ct)
        {
            foreach (var setter in ct.Setters.OfType<Avalonia.Styling.Setter>())
            {
                Console.WriteLine($"setter {setter.Property?.Name} = {Describe(setter.Value)}");
            }
            foreach (var child in ct.Children.OfType<Avalonia.Styling.Style>())
            {
                foreach (var setter in child.Setters.OfType<Avalonia.Styling.Setter>())
                {
                    Console.WriteLine($"style {child.Selector} {setter.Property?.Name} = {Describe(setter.Value)}");
                }
            }
        }
        Console.WriteLine($"default Placement {ToolTip.PlacementProperty.GetDefaultValue(typeof(Control))}");
        Console.WriteLine($"default HorizontalOffset {ToolTip.HorizontalOffsetProperty.GetDefaultValue(typeof(Control))}");
        Console.WriteLine($"default VerticalOffset {ToolTip.VerticalOffsetProperty.GetDefaultValue(typeof(Control))}");
        Console.WriteLine($"default ShowDelay {ToolTip.ShowDelayProperty.GetDefaultValue(typeof(Control))}");
        Console.WriteLine($"default BetweenShowDelay {ToolTip.BetweenShowDelayProperty.GetDefaultValue(typeof(Control))}");
        foreach (Avalonia.Styling.ThemeVariant variant in new[] { Avalonia.Styling.ThemeVariant.Dark, Avalonia.Styling.ThemeVariant.Light })
        {
            foreach (string text in new[] { "Rectangle (R, NumPad1)", "Rectangle region (Tab, NumPad0)" })
            {
                // A ToolTip laid out in a window, as its popup lays it out.
                ToolTip tip = new() { Content = text };
                Window window = new() { Content = tip, RequestedThemeVariant = variant, SizeToContent = SizeToContent.WidthAndHeight };
                window.Show();
                tip.Opacity = 1;
                window.UpdateLayout();
                Console.WriteLine($"{variant} tooltip '{text}' size {tip.Bounds.Size} desired {tip.DesiredSize}");
                foreach (Visual v in tip.GetVisualDescendants())
                {
                    if (v is TextBlock tb)
                    {
                        Console.WriteLine($"  TextBlock family {tb.FontFamily} size {tb.FontSize} weight {tb.FontWeight} fg {Describe(tb.Foreground)} bounds {tb.Bounds}");
                    }
                    else if (v is Avalonia.Controls.Presenters.ContentPresenter cp)
                    {
                        Console.WriteLine($"  ContentPresenter family {cp.FontFamily} size {cp.FontSize} weight {cp.FontWeight} bounds {cp.Bounds}");
                    }
                    else if (v is Border b)
                    {
                        Console.WriteLine($"  Border bounds {b.Bounds} padding {b.Padding} radius {b.CornerRadius} bg {Describe(b.Background)}");
                    }
                }
                window.Close();
            }
        }
        return 0;

    }

    static string Describe(object? value) => value switch
    {
        Avalonia.Media.ISolidColorBrush b => $"brush {b.Color} opacity {b.Opacity}",
        Avalonia.Markup.Xaml.MarkupExtensions.DynamicResourceExtension d => $"dynamic {d.ResourceKey}",
        null => "null",
        _ => $"{value.GetType().Name} {value}"
    };

    public static int Main(string[] args)

    {
        if (args.Length < 2)
        {
            Console.Error.WriteLine("usage: EditorHarness <cases.json> <out dir> [case name filter]");
            Console.Error.WriteLine("       EditorHarness --text <family> <size> <bold 0|1> <text>");
            return 2;
        }

        if (args[0] == "--effects")
        {
            return EffectsHarness.Run(args);
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

        if (args[0] == "--probe-picture")
        {
            return ProbePicture(args[1]);
        }

        if (args[0] == "--emoji-raw")
        {
            // WindowsEmojiBitmapRenderer.RenderWithDirect2D(glyph, canvasSize, fontSize)
            MethodInfo raw = typeof(ShareX.ImageEditor.Presentation.Emoji.WindowsEmojiBitmapRenderer)
                .GetMethod("RenderWithDirect2D", BindingFlags.NonPublic | BindingFlags.Static)!;
            string glyph = ShareX.ImageEditor.Presentation.Emoji.EmojiCatalogService.ToGlyph(args[1]);
            var inv = System.Globalization.CultureInfo.InvariantCulture;
            using SKBitmap? bitmap = (SKBitmap?)raw.Invoke(null, [glyph, int.Parse(args[2], inv), float.Parse(args[3], inv)]);
            if (bitmap == null) return 1;
            SaveRaw(bitmap, args[4]);
            return 0;
        }

        if (args[0] == "--cursors" || args[0] == "--emoji")
        {
            AppBuilder.Configure<HarnessApp>()
                .UseSkia()
                .UseHeadless(new AvaloniaHeadlessPlatformOptions { UseHeadlessDrawing = false })
                .WithInterFont()
                .SetupWithoutStarting();
            return args[0] == "--cursors"
                ? DumpCursors(args[1])
                : DumpEmoji(args[1], int.Parse(args[2], System.Globalization.CultureInfo.InvariantCulture), args[3]);
        }

        if (args[0] == "--control-dump")
        {
            AppBuilder.Configure<HarnessApp>()
                .UseSkia()
                .UseHeadless(new AvaloniaHeadlessPlatformOptions { UseHeadlessDrawing = false })
                .WithInterFont()
                .SetupWithoutStarting();
            return ControlDump.Run(args[1], args.Length > 2 ? args[2] : null);
        }

        if (args[0] == "--tooltip-theme")
        {
            // The resources and ControlTheme setters ShareX's tooltips get from
            // FluentTheme (ShareX's theme doesn't restyle ToolTip), per theme.
            AppBuilder.Configure<HarnessApp>()
                .UseSkia()
                .UseHeadless(new AvaloniaHeadlessPlatformOptions { UseHeadlessDrawing = false })
                .WithInterFont()
                .SetupWithoutStarting();
            return DumpToolTipTheme();
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

        // Never the desktop's wallpaper: a case's own generated image or none.
        EditorServices.DesktopWallpaper = new FakeWallpaper(null, DesktopWallpaperLayout.Fill);

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
                string caseBase = node["base"] is JsonNode b
                    ? Path.Combine(Path.GetDirectoryName(casesPath)!, b.GetValue<string>())
                    : basePath;
                using SKBitmap baseImage = SKBitmap.Decode(caseBase) ?? throw new InvalidOperationException($"can't read {caseBase}");
                List<Annotation> annotations = node["annotations"]!.AsArray()
                    .Select(a => a!.Deserialize<Annotation>(JsonOptions)!)
                    .ToList();
                foreach (Annotation a in annotations)
                {
                    // An inserted image: ImageAnnotation.LoadImage (ImagePath relative to cases.json).
                    if (a.GetType() == typeof(ImageAnnotation) && a is ImageAnnotation image && image.ImagePath != "")
                    {
                        image.LoadImage(Path.Combine(Path.GetDirectoryName(casesPath)!, image.ImagePath));
                    }
                }
                bool configureFill = node["configureFill"]?.GetValue<bool>() ?? false;
                JsonArray? ops = node["ops"]?.AsArray();
                JsonArray? snapshotRect = node["snapshotRect"]?.AsArray();
                JsonObject? background = node["background"]?.AsObject();

                using SKBitmap snapshot = Render(baseImage, annotations, configureFill, ops, snapshotRect, background, Path.GetDirectoryName(casesPath)!);
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

    private static SKBitmap Render(SKBitmap baseImage, List<Annotation> annotations, bool configureFill = false, JsonArray? ops = null,
        JsonArray? snapshotRect = null, JsonObject? background = null, string? casesDir = null)
    {
        ImageEditorOptions options = new() { ShowExitConfirmation = false, RememberWindowState = false };
        string? ratio = null;
        if (background != null)
        {
            // The Background panel's settings, as ImageEditorOptions keeps
            // them (InitializeBackgroundSettingsFromOptions reads them).
            double D(string name, double fallback) => background[name]?.GetValue<double>() ?? fallback;
            string? S(string name) => background[name]?.GetValue<string>();
            options.BackgroundMargin = D("margin", options.BackgroundMargin);
            options.BackgroundPadding = D("padding", options.BackgroundPadding);
            options.BackgroundSmartPadding = background["smartPadding"]?.GetValue<bool>() ?? options.BackgroundSmartPadding;
            options.BackgroundRoundedCorner = D("corner", options.BackgroundRoundedCorner);
            options.BackgroundShadowRadius = D("shadow", options.BackgroundShadowRadius);
            options.BackgroundType = S("type") ?? options.BackgroundType;
            options.BackgroundGradientColor1Hex = S("color1") ?? options.BackgroundGradientColor1Hex;
            options.BackgroundGradientColor2Hex = S("color2") ?? options.BackgroundGradientColor2Hex;
            options.BackgroundColorHex = S("color") ?? options.BackgroundColorHex;
            options.ShadowColorHex = S("shadowColor") ?? options.ShadowColorHex;
            if (S("image") is string image)
            {
                options.BackgroundImagePath = Path.Combine(casesDir!, image);
            }
            string? wallpaper = S("wallpaper");
            EditorServices.DesktopWallpaper = new FakeWallpaper(
                wallpaper == null ? null : Path.Combine(casesDir!, wallpaper),
                Enum.Parse<DesktopWallpaperLayout>(S("layout") ?? "Fill"));
            ratio = S("ratio");
        }
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

            if (background != null && window.DataContext is MainViewModel bvm)
            {
                if (ratio != null)
                {
                    bvm.SetOutputRatioCommand.Execute(ratio);
                }
                // The Background button (ToggleSettingsPanel): the background
                // effects apply while the panel is open.
                bvm.IsSettingsPanelOpen = true;
                Pump();
            }

            EditorView view = window.GetVisualDescendants().OfType<EditorView>().First();
            EditorCore core = (EditorCore)typeof(EditorView)
                .GetField("_editorCore", BindingFlags.NonPublic | BindingFlags.Instance)!
                .GetValue(view)!;

            foreach (Annotation annotation in annotations)
            {
                if (configureFill && annotation is SmartEraserAnnotation eraser)
                {
                    // EditorInputController.UpdateSmartEraserFill, when the eraser is drawn:
                    // its bounds clamped to the canvas, then the fill chosen from the image.
                    var rectangle = (Avalonia.Controls.Shapes.Rectangle)eraser.CreateVisual();
                    ShareX.ImageEditor.Presentation.Rendering.AnnotationVisualFactory.UpdateVisualControl(rectangle, eraser);
                    typeof(SmartEraserAnnotation)
                        .GetMethod("ClampVisualBounds", BindingFlags.NonPublic | BindingFlags.Instance)!
                        .Invoke(eraser, [rectangle, new Size(core.SourceImage!.Width, core.SourceImage.Height)]);
                    eraser.ConfigureFill(core.SourceImage);
                }
                core.AddAnnotation(annotation);
            }

            // What undo/redo does: rebuild the annotation layer from EditorCore
            // (AnnotationVisualFactory, effect bitmaps, the spotlight overlay).
            RestoreAnnotations(view);

            foreach (JsonNode? op in ops ?? [])
            {
                RunOp(view, core, op!.AsObject());
                Pump();
                RestoreAnnotations(view);
            }

            if (snapshotRect != null)
            {
                // Region capture's export (RegionCaptureWindow: GetSnapshot(Rect), a VisualBrush).
                double V(int i) => snapshotRect[i]!.GetValue<double>();
                return view.GetSnapshot(new Rect(V(0), V(1), V(2), V(3))) ?? throw new InvalidOperationException("GetSnapshot(Rect) returned null");
            }

            return view.GetSnapshot() ?? throw new InvalidOperationException("GetSnapshot returned null");
        }
        finally
        {
            window.Close();
            Pump();
        }
    }

    private static void RestoreAnnotations(EditorView view)
    {
        typeof(EditorView)
            .GetMethod("OnAnnotationsRestored", BindingFlags.NonPublic | BindingFlags.Instance)!
            .Invoke(view, null);
        Pump();
    }

    private static float F(JsonObject o, string name) => o[name]!.GetValue<float>();

    private static SKRect R(JsonObject o) =>
        new(o["rect"]![0]!.GetValue<float>(), o["rect"]![1]!.GetValue<float>(),
            o["rect"]![2]!.GetValue<float>(), o["rect"]![3]!.GetValue<float>());

    /// <summary>
    /// One of EditorCore's image operations (or EditorView's, for a clone),
    /// as the editor's toolbar and tools call them.
    /// </summary>
    private static void RunOp(EditorView view, EditorCore core, JsonObject op)
    {
        string name = op["op"]!.GetValue<string>();
        bool ok = true;
        switch (name)
        {
            case "crop": ok = core.Crop(R(op)); break;
            case "performCrop": core.PerformCrop(); break;
            case "cutOut": ok = core.CutOut((int)F(op, "start"), (int)F(op, "end"), op["vertical"]!.GetValue<bool>()); break;
            case "performCutOut":
            {
                SKRect r = R(op);
                var cut = new CutOutAnnotation
                {
                    StartPoint = new SKPoint(r.Left, r.Top),
                    EndPoint = new SKPoint(r.Right, r.Bottom),
                    IsVertical = op["vertical"]!.GetValue<bool>(),
                };
                // The cut-out tool's annotation is in the list while it's drawn (no history).
                var list = (List<Annotation>)typeof(EditorCore)
                    .GetField("_annotations", BindingFlags.NonPublic | BindingFlags.Instance)!.GetValue(core)!;
                list.Add(cut);
                core.PerformCutOut();
                break;
            }
            case "rotate90cw": ok = core.Rotate90Clockwise(); break;
            case "rotate90ccw": ok = core.Rotate90CounterClockwise(); break;
            case "rotate180": ok = core.Rotate180(); break;
            case "rotate": ok = core.RotateCustomAngle(F(op, "angle"), op["autoResize"]?.GetValue<bool>() ?? true); break;
            case "flipH": ok = core.FlipHorizontal(); break;
            case "flipV": ok = core.FlipVertical(); break;
            case "resize": ok = core.ResizeImage((int)F(op, "width"), (int)F(op, "height")); break;
            case "resizeCanvas":
                ok = core.ResizeCanvas((int)F(op, "top"), (int)F(op, "right"), (int)F(op, "bottom"), (int)F(op, "left"),
                    SKColor.Parse(op["color"]!.GetValue<string>()));
                break;
            case "autoCrop": ok = core.AutoCrop((int)F(op, "tolerance")); break;
            case "flatten":
            {
                SKBitmap composite = view.GetSnapshot() ?? throw new InvalidOperationException("no snapshot");
                ok = core.FlattenImage(composite);
                break;
            }
            case "autoExpand":
                ok = core.AutoExpandCanvasToFit(core.Annotations.ToList(), F(op, "margin"),
                    SKColor.Parse(op["color"]!.GetValue<string>()), out _, out _);
                break;
            case "undo": core.Undo(); break;
            case "redo": core.Redo(); break;
            case "clone":
            {
                SKRect r = R(op);
                typeof(EditorView)
                    .GetMethod("CloneRegion", BindingFlags.NonPublic | BindingFlags.Instance)!
                    .Invoke(view, [new Rect(r.Left, r.Top, r.Width, r.Height)]);
                break;
            }
            default: throw new InvalidOperationException($"unknown op {name}");
        }
        Console.WriteLine($"  {name}: {(ok ? "done" : "false")} -> {core.SourceImage?.Width}x{core.SourceImage?.Height}, {core.Annotations.Count} annotations");
    }

    /// <summary>
    /// Saves the bitmaps ShareX makes for cursor annotations
    /// (WindowsCursorBitmapRenderer.CreateAnnotationBitmap), one per CursorType.
    /// </summary>
    private static int DumpCursors(string outDir)
    {
        Directory.CreateDirectory(outDir);
        Type renderer = typeof(EditorCore).Assembly.GetType("ShareX.ImageEditor.Presentation.Rendering.WindowsCursorBitmapRenderer")!;
        MethodInfo create = renderer.GetMethod("CreateAnnotationBitmap", BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Static)!;
        foreach (CursorType type in Enum.GetValues<CursorType>())
        {
            using SKBitmap? bitmap = (SKBitmap?)create.Invoke(null, [type]);
            if (bitmap == null)
            {
                Console.WriteLine($"{type}: none");
                continue;
            }
            SaveRaw(bitmap, Path.Combine(outDir, type + ".bgra"));
            Console.WriteLine($"{type}: {bitmap.Width}x{bitmap.Height} {bitmap.AlphaType}");
        }
        return 0;
    }

    /// <summary>
    /// Saves WindowsEmojiBitmapRenderer.RenderStickerBitmap(sequence, size), the bitmap an
    /// emoji annotation shows.
    /// </summary>
    private static int DumpEmoji(string sequence, int size, string outFile)
    {
        using SKBitmap? bitmap = ShareX.ImageEditor.Presentation.Emoji.WindowsEmojiBitmapRenderer.RenderStickerBitmap(sequence, size);
        if (bitmap == null)
        {
            Console.WriteLine("none");
            return 1;
        }
        SaveRaw(bitmap, outFile);
        Console.WriteLine($"{bitmap.Width}x{bitmap.Height}");
        return 0;
    }

    /// <summary>
    /// Draws the same anti-aliased shapes over an image directly and through an SKPicture
    /// shader (what a VisualBrush becomes), and saves both, to see where they differ.
    /// </summary>
    private static int ProbePicture(string outDir)
    {
        Directory.CreateDirectory(outDir);
        const int w = 200, h = 150;
        using var image = new SKBitmap(w, h);
        for (int y = 0; y < h; y++)
            for (int x = 0; x < w; x++)
                image.SetPixel(x, y, new SKColor((byte)(x * 255 / w), (byte)(y * 255 / h), 128));
        void DrawContent(SKCanvas c)
        {
            c.DrawBitmap(image, 0, 0);
            using var p = new SKPaint { IsAntialias = true, Color = new SKColor(0xEF, 0x44, 0x44), Style = SKPaintStyle.Stroke, StrokeWidth = 4 };
            c.DrawOval(new SKRect(20.5f, 15.25f, 160.5f, 120.75f), p);
            using var f = new SKPaint { IsAntialias = true, Color = new SKColor(0x20, 0x80, 0xF0, 0xA0) };
            c.DrawCircle(120, 90, 37.3f, f);
        }
        using (var direct = new SKBitmap(w, h))
        {
            using (var c = new SKCanvas(direct)) { c.Clear(); DrawContent(c); }
            SaveRaw(direct, Path.Combine(outDir, "direct.bgra"));
        }
        using var recorder = new SKPictureRecorder();
        DrawContent(recorder.BeginRecording(new SKRect(0, 0, w, h)));
        using SKPicture picture = recorder.EndRecording();
        foreach (var (name, info) in new[] {
            ("picture", new SKImageInfo(w, h)),
            ("picture-srgb", new SKImageInfo(w, h, SKImageInfo.PlatformColorType, SKAlphaType.Premul, SKColorSpace.CreateSrgb())) })
        {
            using var target = new SKBitmap(info);
            using (var c = new SKCanvas(target))
            {
                c.Clear();
                using var shader = picture.ToShader(SKShaderTileMode.Decal, SKShaderTileMode.Decal, SKMatrix.Identity, new SKRect(0, 0, w, h));
                using var paint = new SKPaint { Shader = shader };
                c.DrawRect(new SKRect(0, 0, w, h), paint);
            }
            SaveRaw(target, Path.Combine(outDir, name + ".bgra"));
        }
        using (var played = new SKBitmap(w, h))
        {
            using (var c = new SKCanvas(played)) { c.Clear(); c.DrawPicture(picture); }
            SaveRaw(played, Path.Combine(outDir, "drawpicture.bgra"));
        }
        return 0;
    }

    /// <summary>Premultiplied BGRA bytes, after an 8-byte width/height header.</summary>
    private static void SaveRaw(SKBitmap bitmap, string path)
    {
        using SKBitmap bgra = new(new SKImageInfo(bitmap.Width, bitmap.Height, SKColorType.Bgra8888, SKAlphaType.Premul));
        bitmap.CopyTo(bgra);
        using FileStream f = File.Create(path);
        f.Write(BitConverter.GetBytes(bitmap.Width));
        f.Write(BitConverter.GetBytes(bitmap.Height));
        f.Write(bgra.GetPixelSpan());
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
