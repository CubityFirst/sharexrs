// Lays out a Fluent control as ShareX's theme styles it (ShareXTheme.axaml,
// FluentTheme, ControlStyles.axaml, as HarnessApp loads them) and prints its
// visual tree: every Border, ContentPresenter, TextBlock and the like with its
// bounds (relative to its root), padding, margin, corner radius, brushes and
// fonts, so ShareXrs can port the values instead of guessing them.
//
//     EditorHarness --control-dump autocomplete|datepicker|datepicker-presenter
//
// autocomplete: the History window's search box (an AutoCompleteBox 250 wide
// with FilterMode ContainsOrdinal and Padding 12,7,38,7) over a few process
// names, the text "c" typed, the drop-down open: the popup's tree.
// datepicker: the advanced filters' DatePicker (2026-11-19).
// datepicker-presenter: the DatePicker's flyout (DatePickerPresenter) as it
// opens for that date.

using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.Presenters;
using Avalonia.Controls.Primitives;
using Avalonia.Layout;
using Avalonia.Media;
using Avalonia.Threading;
using Avalonia.VisualTree;

namespace EditorHarness;

internal static class ControlDump
{
    public static int Run(string kind)
    {
        foreach (Avalonia.Styling.ThemeVariant variant in new[] { Avalonia.Styling.ThemeVariant.Dark, Avalonia.Styling.ThemeVariant.Light })
        {
            Console.WriteLine($"=== {variant}");
            switch (kind)
            {
                case "autocomplete":
                    AutoComplete(variant);
                    break;
                case "datepicker":
                    DatePickerButton(variant);
                    break;
                case "datepicker-presenter":
                    Presenter(variant);
                    break;
                default:
                    Console.Error.WriteLine($"unknown control {kind}");
                    return 2;
            }
        }
        return 0;
    }

    static void Pump()
    {
        for (int i = 0; i < 5; i++)
        {
            Dispatcher.UIThread.RunJobs();
        }
    }

    static void AutoComplete(Avalonia.Styling.ThemeVariant variant)
    {
        AutoCompleteBox box = new()
        {
            Width = 250,
            FilterMode = AutoCompleteFilterMode.ContainsOrdinal,
            Padding = new Thickness(12, 7, 38, 7),
            ItemsSource = new[] { "chrome", "Code", "explorer", "notepad", "ShareX" },
            PlaceholderText = "Search file names and tags",
            HorizontalAlignment = HorizontalAlignment.Left,
            VerticalAlignment = VerticalAlignment.Top,
        };
        Window window = new() { Content = box, Width = 600, Height = 500, RequestedThemeVariant = variant };
        window.Show();
        window.UpdateLayout();
        Dump(box, box, 0);
        box.Text = "c";
        Pump();
        box.IsDropDownOpen = true;
        Pump();
        window.UpdateLayout();
        Popup? popup = box.GetVisualDescendants().OfType<Popup>().FirstOrDefault();
        Console.WriteLine($"popup open {popup?.IsOpen} placement {popup?.Placement} offset {popup?.HorizontalOffset},{popup?.VerticalOffset} child {popup?.Child?.GetType().Name}");
        if (popup?.Child is Control child)
        {
            Visual? root = TopLevel.GetTopLevel(child);
            if (root is Layoutable l)
            {
                l.UpdateLayout();
            }
            Console.WriteLine($"popup root {root?.GetType().Name} bounds {root?.Bounds}");
            Dump(child, child, 0);
        }
        window.Close();
    }

    static void DatePickerButton(Avalonia.Styling.ThemeVariant variant)
    {
        DatePicker picker = new()
        {
            SelectedDate = new DateTimeOffset(new DateTime(2026, 11, 19)),
            HorizontalAlignment = HorizontalAlignment.Left,
            VerticalAlignment = VerticalAlignment.Top,
        };
        Window window = new() { Content = picker, Width = 600, Height = 500, RequestedThemeVariant = variant };
        window.Show();
        window.UpdateLayout();
        Dump(picker, picker, 0);
        // Its button's click, as a user's opens the flyout.
        var click = typeof(DatePicker).GetMethod("OnFlyoutButtonClicked", System.Reflection.BindingFlags.NonPublic | System.Reflection.BindingFlags.Instance);
        Console.WriteLine($"OnFlyoutButtonClicked {(click == null ? "missing" : "found")}");
        click?.Invoke(picker, [null, new Avalonia.Interactivity.RoutedEventArgs()]);
        Pump();
        window.UpdateLayout();
        foreach (Popup popup in picker.GetVisualDescendants().OfType<Popup>())
        {
            Console.WriteLine($"popup open {popup.IsOpen} placement {popup.Placement} anchor {popup.PlacementAnchor} gravity {popup.PlacementGravity} offset {popup.HorizontalOffset},{popup.VerticalOffset} child {popup.Child?.GetType().Name}");
            if (popup.Child is Control c)
            {
                Point? at = c.TranslatePoint(new Point(0, 0), picker);
                Console.WriteLine($"  child at {at} bounds {c.Bounds}");
            }
        }
        foreach (var m in typeof(DatePickerPresenter).GetMethods(System.Reflection.BindingFlags.NonPublic | System.Reflection.BindingFlags.Instance).Where(m => m.Name.Contains("Offset")))
        {
            Console.WriteLine($"presenter method {m.Name}");
        }
        window.Close();
    }

    static void Presenter(Avalonia.Styling.ThemeVariant variant)
    {
        DatePickerPresenter presenter = new()
        {
            Date = new DateTimeOffset(new DateTime(2026, 11, 19)),
            Width = 296,
            HorizontalAlignment = HorizontalAlignment.Left,
            VerticalAlignment = VerticalAlignment.Top,
        };
        Window window = new() { Content = presenter, Width = 600, Height = 600, RequestedThemeVariant = variant };
        window.Show();
        window.UpdateLayout();
        Pump();
        window.UpdateLayout();
        Console.WriteLine($"presenter desired {presenter.DesiredSize} bounds {presenter.Bounds}");
        foreach (string key in new[] { "DatePickerFlyoutPresenterHighlightFill", "DatePickerFlyoutPresenterSpacerFill", "DatePickerFlyoutPresenterBackground",
            "DatePickerFlyoutPresenterBorderBrush", "DatePickerFlyoutPresenterItemPadding", "DatePickerFlyoutPresenterMonthPadding",
            "DatePickerFlyoutPresenterItemHeight", "DatePickerFlyoutPresenterAcceptDismissHostGridHeight", "DatePickerFlyoutPresenterHighlightHeight",
            "SystemAccentColor", "ListBoxItemForegroundPointerOver", "ListBoxItemBackgroundPointerOver", "ListBoxItemBackgroundPressed",
            "ListBoxItemBackgroundSelected", "ListBoxItemForegroundSelected", "ListBoxItemBackgroundSelectedPointerOver",
            "AutoCompleteListMaxHeight", "SystemControlHighlightListLowBrush", "SystemControlHighlightListMediumBrush",
            "SystemControlHighlightAltBaseHighBrush", "SystemControlHighlightBaseMediumLowBrush", "ButtonBackgroundPointerOver", "ButtonBackgroundPressed" })
        {
            bool found = Application.Current!.TryGetResource(key, presenter.ActualThemeVariant, out object? value);
            Console.WriteLine($"resource {key} = {(found ? (value is IBrush br ? B(br) : value?.ToString()) : "(none)")}");
        }
        foreach (Visual v in presenter.GetVisualDescendants())
        {
            if (v is ListBoxItem item && item.IsSelected)
            {
                Console.WriteLine($"selected item fg {B(item.Foreground)} bg {B(item.Background)}");
            }
        }
        Dump(presenter, presenter, 0);
        window.Close();
    }

    static string B(IBrush? b) => b switch
    {
        null => "-",
        ISolidColorBrush s => $"{s.Color}@{s.Opacity:0.##}",
        _ => b.GetType().Name,
    };

    static void Dump(Visual v, Visual root, int depth)
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
            Border b => $"bg {B(b.Background)} border {B(b.BorderBrush)} {b.BorderThickness} radius {b.CornerRadius} padding {b.Padding}",
            ContentPresenter c => $"bg {B(c.Background)} border {B(c.BorderBrush)} {c.BorderThickness} radius {c.CornerRadius} padding {c.Padding} fg {B(c.Foreground)} font {c.FontFamily} {c.FontSize} {c.FontWeight}",
            TextBlock t => $"'{t.Text}' fg {B(t.Foreground)} font {t.FontFamily} {t.FontSize} {t.FontWeight} align {t.TextAlignment}",
            TemplatedControl tc => $"bg {B(tc.Background)} border {B(tc.BorderBrush)} {tc.BorderThickness} padding {tc.Padding} fg {B(tc.Foreground)} font {tc.FontSize} {tc.FontWeight}",
            Avalonia.Controls.Shapes.Path path => $"fill {B(path.Fill)} stroke {B(path.Stroke)} {path.StrokeThickness} cap {path.StrokeLineCap} stretch {path.Stretch} geometry bounds {path.Data?.Bounds}",
            Avalonia.Controls.Shapes.Shape s => $"fill {B(s.Fill)} stroke {B(s.Stroke)} {s.StrokeThickness}",
            Panel p => $"bg {B(p.Background)}",
            _ => "",
        };
        string margin = v is Layoutable lay && lay.Margin != default ? $" margin {lay.Margin}" : "";
        string classes = v is StyledElement se && se.Classes.Count > 0 ? $" .{string.Join(".", se.Classes)}" : "";
        Console.WriteLine($"{pad}{v.GetType().Name}{name}{classes} [{at}]{margin} {extra}");
        foreach (Visual child in v.GetVisualChildren())
        {
            Dump(child, root, depth + 1);
        }
    }
}
