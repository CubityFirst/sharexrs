// Lays out Avalonia's ColorView as ShareX configures it, in each of its
// hosts, and prints its visual tree on each tab plus the theme resources its
// templates use; with an output folder, also saves each state as a PNG (the
// headless platform's Skia frame) to compare ShareXrs's port with.
//
//     EditorHarness --control-dump colorview [out dir]
//
// Hosts:
//   flyout:   MouseHighlighterSettingsControl.ColorRow: a Flyout's
//             FlyoutPresenter around ColorView { IsAlphaEnabled, IsAlphaVisible }
//             (Color 128,54,169,225: LIVE-CHECKS HIL2-02).
//   effects:  ImageEffectOptionsPanel / GradientOptionsPanel: ColorView
//             { MinWidth 320, IsAlphaVisible, IsColorPreviewVisible } in a Flyout.
//   dropdown: ColorPickerDropdown's popup: Border (panel, 8 corners, padding 4,
//             1px border) around ColorPickerPanel's ColorView { IsAlphaVisible,
//             IsColorPreviewVisible, SelectedIndex 1 }.

using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.Primitives;
using Avalonia.Headless;
using Avalonia.Layout;
using Avalonia.Media;
using Avalonia.Threading;
using Avalonia.VisualTree;

namespace EditorHarness;

internal static class ColorViewDump
{
    static readonly string[] Resources =
    [
        "ColorControlDarkSelectorBrush", "ColorControlDefaultSelectorBrush", "ColorControlLightSelectorBrush",
        "ColorSliderCornerRadius", "ColorSliderSize", "ColorSliderTrackCornerRadius", "ColorSliderTrackSize",
        "ColorViewComponentLabelWidth", "ColorViewComponentTextInputWidth", "ColorViewContentBackgroundBrush",
        "ColorViewContentBorderBrush", "ColorViewTabBackgroundBrush", "ColorViewTabBackgroundCornerRadius",
        "ColorViewTabBorderBrush", "ColorViewTabStripHeight", "ControlCornerRadius", "OverlayCornerRadius",
        "SystemControlForegroundListLowBrush", "SystemControlHighlightListAccentLowBrush", "SystemControlForegroundBaseHighBrush",
        "TabItemHeaderBackgroundSelected", "TabItemHeaderBackgroundSelectedPointerOver", "TabItemHeaderBackgroundSelectedPressed",
        "TabItemHeaderBackgroundUnselected", "TabItemHeaderBackgroundUnselectedPointerOver", "TabItemHeaderBackgroundUnselectedPressed",
        "TabItemHeaderFontSize", "TabItemHeaderForegroundSelected", "TabItemHeaderForegroundSelectedPointerOver",
        "TabItemHeaderForegroundSelectedPressed", "TabItemHeaderForegroundUnselected", "TabItemHeaderForegroundUnselectedPointerOver",
        "TabItemHeaderForegroundUnselectedPressed", "TabItemHeaderSelectedPipeFill", "TabItemHeaderThemeFontWeight", "TabItemPipeThickness",
        "TextControlBackgroundDisabled", "TextControlBorderBrush", "TextControlForeground", "TextControlForegroundDisabled",
        "TextControlBackground", "TextControlBorderThemeThickness", "TextControlThemePadding",
        "FlyoutPresenterBackground", "FlyoutBorderThemeBrush", "FlyoutBorderThemeThickness", "FlyoutContentPadding",
        "ToggleButtonBackground", "ToggleButtonBackgroundChecked", "ToggleButtonForeground", "ToggleButtonForegroundChecked",
        "ToggleButtonBorderBrush", "ToggleButtonBorderBrushChecked", "ButtonPadding", "SystemAccentColor",
        "SystemAccentColorDark1", "SystemAccentColorDark2", "SystemAccentColorDark3",
        "SystemAccentColorLight1", "SystemAccentColorLight2", "SystemAccentColorLight3",
        "SystemControlBackgroundBaseLowBrush",
    ];

    public static void Run(Avalonia.Styling.ThemeVariant variant, string? outDir)
    {
        foreach (string host in new[] { "flyout", "effects", "dropdown" })
        {
            for (int tab = 0; tab < 3; tab++)
            {
                foreach (bool rgb in tab == 2 ? new[] { true, false } : new[] { true })
                {
                    Case(variant, host, tab, rgb, outDir);
                }
            }
        }
    }

    static void Pump()
    {
        for (int i = 0; i < 5; i++)
        {
            Dispatcher.UIThread.RunJobs();
        }
    }

    static void Case(Avalonia.Styling.ThemeVariant variant, string host, int tab, bool rgb, string? outDir)
    {
        ColorView view = host switch
        {
            "flyout" => new ColorView { Color = Color.FromArgb(128, 54, 169, 225), IsAlphaEnabled = true, IsAlphaVisible = true },
            "effects" => new ColorView { MinWidth = 320, Color = Color.FromArgb(255, 54, 169, 225), IsAlphaVisible = true, IsColorPreviewVisible = true },
            _ => new ColorView { Color = Color.FromArgb(255, 54, 169, 225), IsAlphaVisible = true, IsColorPreviewVisible = true },
        };
        view.SelectedIndex = tab;
        view.ColorModel = rgb ? ColorModel.Rgba : ColorModel.Hsva;
        Control root;
        if (host == "dropdown")
        {
            root = new Border
            {
                Child = view,
                CornerRadius = new CornerRadius(8),
                Padding = new Thickness(4),
                BorderThickness = new Thickness(1),
                [!Border.BackgroundProperty] = new Avalonia.Markup.Xaml.MarkupExtensions.DynamicResourceExtension("ShareX.Brush.Background.Panel"),
                [!Border.BorderBrushProperty] = new Avalonia.Markup.Xaml.MarkupExtensions.DynamicResourceExtension("ShareX.Brush.Border"),
            };
        }
        else
        {
            root = new FlyoutPresenter { Content = view };
        }
        root.HorizontalAlignment = HorizontalAlignment.Left;
        root.VerticalAlignment = VerticalAlignment.Top;
        Window window = new() { Content = root, Width = 520, Height = 640, RequestedThemeVariant = variant, Background = Brushes.Black };
        window.Show();
        window.UpdateLayout();
        // ColorSpectrum and ColorSlider build their bitmaps on the thread
        // pool (Task.Run) and post them back: give them time to land.
        for (int i = 0; i < 20; i++)
        {
            Thread.Sleep(25);
            Pump();
            window.UpdateLayout();
        }
        string name = $"{host}-tab{tab}{(tab == 2 ? (rgb ? "-rgb" : "-hsv") : "")}-{variant.ToString().ToLowerInvariant()}";
        Console.WriteLine($"--- {name} root {root.Bounds} view {view.Bounds} color {view.Color} hsv {view.HsvColor} palette columns {view.PaletteColumnCount} colors {view.PaletteColors?.Count()}");
        if (tab == 1 && view.PaletteColors is { } colors)
        {
            Console.WriteLine("palette " + string.Join(" ", colors.Select(c => c.ToString())));
        }
        if (host == "flyout" && tab == 0 && rgb)
        {
            foreach (string key in Resources)
            {
                bool found = Application.Current!.TryGetResource(key, variant, out object? value);
                Console.WriteLine($"resource {key} = {(found ? Format(value) : "(none)")}");
            }
        }
        DumpTree(root, root, 0);
        if (outDir != null)
        {
            Directory.CreateDirectory(outDir);
            var frame = window.CaptureRenderedFrame();
            if (frame != null)
            {
                frame.Save(Path.Combine(outDir, name + "-window.png"));
                Console.WriteLine($"saved {name}-window.png (crop to {root.Bounds})");
            }
        }
        window.Close();
    }

    static string Format(object? value) => value switch
    {
        ISolidColorBrush s => $"{s.Color}@{s.Opacity:0.##}",
        IBrush b => b.GetType().Name,
        _ => value?.ToString() ?? "null",
    };

    static string B(IBrush? b) => b switch
    {
        null => "-",
        ISolidColorBrush s => $"{s.Color}@{s.Opacity:0.##}",
        _ => b.GetType().Name,
    };

    static void DumpTree(Visual v, Visual root, int depth)
    {
        if (!v.IsVisible)
        {
            return;
        }
        Point? origin = v.TranslatePoint(new Point(0, 0), root);
        string at = origin is Point o ? $"{o.X:0.##},{o.Y:0.##} {v.Bounds.Width:0.##}x{v.Bounds.Height:0.##}" : v.Bounds.ToString();
        string pad = new(' ', depth * 2);
        string name = string.IsNullOrEmpty((v as StyledElement)?.Name) ? "" : $"#{((StyledElement)v).Name}";
        string extra = v switch
        {
            Border b => $"bg {B(b.Background)} border {B(b.BorderBrush)} {b.BorderThickness} radius {b.CornerRadius} padding {b.Padding} shadow {b.BoxShadow}",
            Avalonia.Controls.Presenters.ContentPresenter c => $"bg {B(c.Background)} border {B(c.BorderBrush)} {c.BorderThickness} radius {c.CornerRadius} padding {c.Padding} fg {B(c.Foreground)} font {c.FontSize} {c.FontWeight}",
            TextBlock t => $"'{t.Text}' fg {B(t.Foreground)} font {t.FontFamily} {t.FontSize} {t.FontWeight}",
            Avalonia.Controls.Primitives.ColorSlider s => $"component {s.ColorComponent} model {s.ColorModel} orientation {s.Orientation} value {s.Value} min {s.Minimum} max {s.Maximum} perceptive {s.IsPerceptive} alpha {s.IsAlphaVisible} enabled {s.IsEffectivelyEnabled} radius {s.CornerRadius}",
            Avalonia.Controls.Primitives.ColorSpectrum sp => $"components {sp.Components} shape {sp.Shape} hsv {sp.HsvColor} third {sp.ThirdComponent} radius {sp.CornerRadius}",
            NumericUpDown n => $"value {n.Value} text '{n.Text}' padding {n.Padding} radius {n.CornerRadius}",
            TextBox tb => $"text '{tb.Text}' padding {tb.Padding} radius {tb.CornerRadius} font {tb.FontSize} fg {B(tb.Foreground)} bg {B(tb.Background)}",
            TemplatedControl tc => $"bg {B(tc.Background)} border {B(tc.BorderBrush)} {tc.BorderThickness} padding {tc.Padding} fg {B(tc.Foreground)} font {tc.FontSize} {tc.FontWeight} radius {tc.CornerRadius}",
            Avalonia.Controls.Shapes.Path path => $"fill {B(path.Fill)} stroke {B(path.Stroke)} {path.StrokeThickness} stretch {path.Stretch}",
            Avalonia.Controls.Shapes.Shape s => $"fill {B(s.Fill)} stroke {B(s.Stroke)} {s.StrokeThickness} opacity {s.Opacity}",
            Panel p => $"bg {B(p.Background)}",
            _ => "",
        };
        string margin = v is Layoutable lay && lay.Margin != default ? $" margin {lay.Margin}" : "";
        string classes = v is StyledElement se && se.Classes.Count > 0 ? $" .{string.Join(".", se.Classes)}" : "";
        string opacity = v.Opacity != 1 ? $" opacity {v.Opacity}" : "";
        Console.WriteLine($"{pad}{v.GetType().Name}{name}{classes} [{at}]{margin}{opacity} {extra}");
        foreach (Visual child in v.GetVisualChildren())
        {
            DumpTree(child, root, depth + 1);
        }
    }
}
