// The synthetic test images: text drawn with SkiaSharp in Windows fonts, in several
// sizes, colours, backgrounds, layouts, angles and languages, plus a made-up UI
// screenshot. Nothing here is a real capture.

using System.Text.Json;
using SkiaSharp;

static class Generate
{
    sealed record Case(string Name, int Width, int Height, string Description, Action<Draw> Paint, bool Transparent = false);

    // A canvas with helpers that remember which fonts were used.
    sealed class Draw(SKCanvas canvas, List<string> fonts)
    {
        public SKCanvas Canvas => canvas;

        public SKTypeface Face(string family, bool bold = false, bool italic = false)
        {
            SKFontStyle style = new(bold ? SKFontStyleWeight.Bold : SKFontStyleWeight.Normal, SKFontStyleWidth.Normal,
                italic ? SKFontStyleSlant.Italic : SKFontStyleSlant.Upright);
            SKTypeface face = SKTypeface.FromFamilyName(family, style) ?? SKTypeface.Default;
            string used = face.FamilyName == family ? family : $"{family} -> {face.FamilyName}";
            if (!fonts.Contains(used)) fonts.Add(used);
            return face;
        }

        public void Text(string text, float x, float y, string family, float size, SKColor color, bool bold = false, bool italic = false)
        {
            using SKFont font = new(Face(family, bold, italic), size) { Edging = SKFontEdging.Antialias, Subpixel = true };
            using SKPaint paint = new() { Color = color, IsAntialias = true };
            canvas.DrawText(text, x, y, font, paint);
        }

        public void Fill(SKColor color) => canvas.Clear(color);

        public void Rect(float x, float y, float w, float h, SKColor color, bool stroke = false)
        {
            using SKPaint paint = new() { Color = color, IsAntialias = true, IsStroke = stroke, StrokeWidth = 1 };
            canvas.DrawRect(SKRect.Create(x, y, w, h), paint);
        }

        public void RoundRect(float x, float y, float w, float h, float r, SKColor fill, SKColor? border = null)
        {
            using SKPaint paint = new() { Color = fill, IsAntialias = true };
            canvas.DrawRoundRect(SKRect.Create(x, y, w, h), r, r, paint);
            if (border is SKColor b)
            {
                using SKPaint p = new() { Color = b, IsAntialias = true, IsStroke = true, StrokeWidth = 1 };
                canvas.DrawRoundRect(SKRect.Create(x + 0.5f, y + 0.5f, w - 1, h - 1), r, r, p);
            }
        }

        public void Circle(float cx, float cy, float r, SKColor color)
        {
            using SKPaint paint = new() { Color = color, IsAntialias = true };
            canvas.DrawCircle(cx, cy, r, paint);
        }
    }

    static readonly SKColor Black = SKColors.Black, White = SKColors.White;

    const string Pangram = "The quick brown fox jumps over the lazy dog.";

    static IEnumerable<Case> Cases()
    {
        // Fonts, sizes and colours.
        yield return new("en_segoe_16", 420, 40, "Segoe UI 16, black on white", d => { d.Fill(White); d.Text(Pangram, 10, 26, "Segoe UI", 16, Black); });
        yield return new("en_arial_11", 300, 30, "Arial 11 (small text)", d => { d.Fill(White); d.Text(Pangram, 6, 19, "Arial", 11, Black); });
        yield return new("en_times_28", 640, 110, "Times New Roman 28, two lines", d =>
        {
            d.Fill(White);
            d.Text("It was the best of times,", 14, 42, "Times New Roman", 28, Black);
            d.Text("it was the worst of times.", 14, 88, "Times New Roman", 28, Black);
        });
        yield return new("en_consolas_code", 520, 120, "Consolas 14, code lines", d =>
        {
            d.Fill(new SKColor(0xFA, 0xFA, 0xFA));
            string[] lines = ["fn main() {", "    let total = 42 * 7;", "    println!(\"total = {}\", total);", "}"];
            for (int i = 0; i < lines.Length; i++) d.Text(lines[i], 12, 26 + i * 24, "Consolas", 14, new SKColor(0x20, 0x20, 0x60));
        });
        yield return new("en_georgia_bold_italic", 560, 50, "Georgia 22 bold italic", d => { d.Fill(White); d.Text("Bold and italic headlines read well", 12, 34, "Georgia", 22, Black, bold: true, italic: true); });
        yield return new("white_on_dark", 460, 44, "Segoe UI 18, white on #1E1E1E", d => { d.Fill(new SKColor(0x1E, 0x1E, 0x1E)); d.Text("Dark mode text is common in apps", 12, 29, "Segoe UI", 18, White); });
        yield return new("red_on_yellow", 420, 46, "Verdana 20, red on yellow", d => { d.Fill(new SKColor(0xFF, 0xE0, 0x40)); d.Text("Warning: wet floor", 14, 31, "Verdana", 20, new SKColor(0xC0, 0x10, 0x10)); });
        yield return new("low_contrast", 400, 40, "Segoe UI 16, #8A8A8A on #B0B0B0", d => { d.Fill(new SKColor(0xB0, 0xB0, 0xB0)); d.Text("Low contrast placeholder text", 10, 26, "Segoe UI", 16, new SKColor(0x8A, 0x8A, 0x8A)); });
        yield return new("gradient_bg", 520, 70, "Segoe UI Semibold 24, white on a gradient", d =>
        {
            using SKPaint bg = new() { Shader = SKShader.CreateLinearGradient(new SKPoint(0, 0), new SKPoint(520, 70), [new SKColor(0x20, 0x40, 0xC0), new SKColor(0xA0, 0x20, 0xA0)], SKShaderTileMode.Clamp) };
            d.Canvas.DrawRect(0, 0, 520, 70, bg);
            d.Text("Gradients behind white text", 18, 44, "Segoe UI Semibold", 24, White);
        });
        yield return new("large_72", 900, 120, "Segoe UI 72 bold (lines taller than the model, so crops shrink)", d => { d.Fill(White); d.Text("Big Headline", 20, 88, "Segoe UI", 72, Black, bold: true); });
        yield return new("huge_image", 4600, 400, "4600 px wide (detection input shrinks with mipmaps)", d =>
        {
            d.Fill(White);
            d.Text("A very wide banner with large lettering that keeps going across the whole image", 40, 240, "Arial", 110, new SKColor(0x10, 0x30, 0x10));
        });
        yield return new("very_wide_odd", 9001, 501, "9001x501 (detection input shrinks to 0.44: two mipmap levels blended, odd sizes)", d =>
        {
            d.Fill(White);
            d.Text("An enormous odd sized poster whose detection input is shrunk by more than half", 60, 300, "Arial", 170, Black);
        });
        yield return new("very_wide_short", 9001, 41, "9001x41 (the same shrink in a small file)", d =>
        {
            d.Fill(White);
            d.Text(string.Join("   ", Enumerable.Repeat("Ticker tape headline", 24)), 10, 30, "Arial", 28, Black);
        });
        yield return new("giant_text", 1500, 280, "Arial 200 (line crops shrink past the first mipmap level)", d => { d.Fill(White); d.Text("Giant words", 30, 210, "Arial", 200, new SKColor(0x30, 0x30, 0x80)); });
        yield return new("long_line", 3800, 40, "One 3800 px line of small text (recognition input capped at 3200)", d =>
        {
            d.Fill(White);
            d.Text(string.Join(" ", Enumerable.Repeat("a long line of small text that keeps going", 9)), 8, 27, "Segoe UI", 18, Black);
        });
        yield return new("tiny_image", 44, 22, "44x22, Segoe UI 12 (detection input grows)", d => { d.Fill(White); d.Text("OK", 12, 16, "Segoe UI", 12, Black); });

        // Layout.
        yield return new("paragraphs", 560, 300, "Heading, two paragraphs", d =>
        {
            d.Fill(White);
            d.Text("Release notes", 16, 40, "Segoe UI", 28, Black, bold: true);
            string[] p1 = ["This version adds text recognition that runs", "entirely on your computer, with no network", "access and no account."];
            for (int i = 0; i < p1.Length; i++) d.Text(p1[i], 16, 82 + i * 24, "Segoe UI", 16, Black);
            string[] p2 = ["Recognition works best on clear screenshots", "of printed text."];
            for (int i = 0; i < p2.Length; i++) d.Text(p2[i], 16, 196 + i * 24, "Segoe UI", 16, Black);
        });
        yield return new("columns", 700, 200, "Two columns", d =>
        {
            d.Fill(White);
            string[] left = ["Left column starts here", "and continues on this", "third line of the column."];
            string[] right = ["Right column has its", "own lines that should", "not be joined with left."];
            for (int i = 0; i < 3; i++)
            {
                d.Text(left[i], 20, 50 + i * 30, "Arial", 18, Black);
                d.Text(right[i], 380, 50 + i * 30, "Arial", 18, Black);
            }
        });

        // Angles.
        foreach (int angle in new[] { 5, 15, 30 })
        {
            int a = angle;
            yield return new($"rotated_{a}", 560, 260, $"Segoe UI 22 rotated {a} degrees", d =>
            {
                d.Fill(White);
                d.Canvas.Save();
                d.Canvas.RotateDegrees(-a, 280, 130);
                d.Text("Rotated text is still readable", 90, 138, "Segoe UI", 22, Black);
                d.Canvas.Restore();
            });
        }
        yield return new("rotated_90", 80, 420, "Segoe UI 22 rotated 90 degrees (vertical line)", d =>
        {
            d.Fill(White);
            d.Canvas.Save();
            d.Canvas.RotateDegrees(-90, 40, 210);
            d.Text("Sideways label text", -70, 218, "Segoe UI", 22, Black);
            d.Canvas.Restore();
        });
        yield return new("skewed", 560, 80, "Segoe UI 24 skewed (faux italic)", d =>
        {
            d.Fill(White);
            d.Canvas.Save();
            d.Canvas.Concat(SKMatrix.CreateSkew(-0.3f, 0));
            d.Text("Skewed letters lean forward", 40, 50, "Segoe UI", 24, Black);
            d.Canvas.Restore();
        });
        yield return new("vertical_cjk", 70, 360, "Chinese written top to bottom", d =>
        {
            d.Fill(White);
            string text = "竖排中文文字测试";
            for (int i = 0; i < text.Length; i++) d.Text(text[i].ToString(), 20, 50 + i * 38, "Microsoft YaHei", 30, Black);
        });

        // Languages.
        yield return new("lang_fr", 620, 44, "French", d => { d.Fill(White); d.Text("Ça a été très agréable, à bientôt au château !", 10, 30, "Segoe UI", 20, Black); });
        yield return new("lang_de", 560, 44, "German", d => { d.Fill(White); d.Text("Größe, Übung, Straße und schöne Mädchen.", 10, 30, "Segoe UI", 20, Black); });
        yield return new("lang_es", 560, 44, "Spanish", d => { d.Fill(White); d.Text("¿Dónde está la niña? ¡Mañana será otro día!", 10, 30, "Segoe UI", 20, Black); });
        yield return new("lang_zh_hans", 520, 50, "Simplified Chinese", d => { d.Fill(White); d.Text("今天天气很好，我们去公园散步吧。", 10, 34, "Microsoft YaHei", 24, Black); });
        yield return new("lang_zh_hant", 440, 50, "Traditional Chinese", d => { d.Fill(White); d.Text("繁體中文測試，臺灣與香港。", 10, 34, "Microsoft JhengHei", 24, Black); });
        yield return new("lang_ja", 620, 50, "Japanese", d => { d.Fill(White); d.Text("日本語の文字認識テストです。カタカナとひらがな。", 10, 34, "Yu Gothic", 22, Black); });
        yield return new("lang_el", 560, 44, "Greek", d => { d.Fill(White); d.Text("Καλημέρα κόσμε, αυτή είναι μια δοκιμή.", 10, 30, "Segoe UI", 20, Black); });
        yield return new("lang_ko", 360, 50, "Korean (not in the model's dictionary)", d => { d.Fill(White); d.Text("안녕하세요 세계 여러분", 10, 34, "Malgun Gothic", 24, Black); });
        yield return new("lang_ru", 380, 44, "Russian (not in the model's dictionary)", d => { d.Fill(White); d.Text("Привет, мир! Как дела?", 10, 30, "Segoe UI", 20, Black); });
        yield return new("numbers_symbols", 720, 44, "Numbers, currency, dates, punctuation", d => { d.Fill(White); d.Text("Invoice #2026-0042  Total: $1,234.56  Due 17/11/2026  (ID: A7-x9)", 10, 29, "Segoe UI", 17, Black); });

        // A made-up UI screenshot.
        yield return new("ui_screenshot", 820, 520, "Synthetic settings window", DrawUi);

        // Edge cases.
        yield return new("transparent", 420, 44, "Black text on a transparent background", d => { d.Fill(SKColors.Transparent); d.Text("Transparent background text", 10, 30, "Segoe UI", 20, Black); }, Transparent: true);
        yield return new("shapes_and_text", 520, 160, "Icons and shapes beside a little text", d =>
        {
            d.Fill(White);
            d.Circle(40, 40, 18, new SKColor(0x30, 0x90, 0xE0));
            d.Rect(90, 22, 36, 36, new SKColor(0xE0, 0x60, 0x20));
            d.RoundRect(150, 22, 60, 36, 8, new SKColor(0x40, 0xB0, 0x60));
            for (int i = 0; i < 12; i++) d.Rect(240 + i * 20, 30 + (i % 3) * 6, 12, 12, new SKColor((byte)(i * 20), 0x80, 0xC0));
            d.Text("Toolbar with icons", 20, 120, "Segoe UI", 18, Black);
        });
        yield return new("empty", 200, 100, "A blank image", d => d.Fill(White));
    }

    static void DrawUi(Draw d)
    {
        SKColor text = new(0x1A, 0x1A, 0x1A), sub = new(0x60, 0x60, 0x60), accent = new(0x00, 0x67, 0xC0);
        d.Fill(new SKColor(0xF3, 0xF3, 0xF3));
        // Title bar.
        d.Rect(0, 0, 820, 32, White);
        d.Text("Settings - Example App", 12, 21, "Segoe UI", 12, text);
        d.Text("—", 712, 21, "Segoe UI", 12, text);
        d.Text("☐", 750, 21, "Segoe UI Symbol", 12, text);
        d.Text("✕", 790, 21, "Segoe UI Symbol", 12, text);
        // Sidebar.
        d.Rect(0, 32, 200, 456, new SKColor(0xEB, 0xEB, 0xEB));
        string[] items = ["General", "Capture", "Upload", "Hotkeys", "History", "Advanced"];
        for (int i = 0; i < items.Length; i++)
        {
            if (i == 1) d.RoundRect(8, 48 + i * 36, 184, 30, 4, new SKColor(0xDD, 0xDD, 0xDD));
            d.Text(items[i], 24, 68 + i * 36, "Segoe UI", 14, text);
        }
        // Content.
        d.Text("Capture", 224, 70, "Segoe UI Semibold", 22, text);
        d.Text("Choose what happens after a screenshot is taken.", 224, 96, "Segoe UI", 13, sub);
        string[] checks = ["Copy image to clipboard", "Save image to file", "Upload image to host", "Recognize text (OCR)"];
        for (int i = 0; i < checks.Length; i++)
        {
            float y = 130 + i * 34;
            d.RoundRect(224, y, 18, 18, 3, i < 3 ? accent : White, i < 3 ? null : sub);
            if (i < 3) d.Text("✓", 227, y + 14, "Segoe UI Symbol", 12, White);
            d.Text(checks[i], 252, y + 14, "Segoe UI", 14, text);
        }
        d.Text("File name pattern", 224, 290, "Segoe UI", 13, text);
        d.RoundRect(224, 300, 360, 30, 4, White, new SKColor(0xC8, 0xC8, 0xC8));
        d.Text("Screenshot_%y-%mo-%d_%h-%mi-%s", 234, 320, "Consolas", 13, text);
        // A small table.
        d.Rect(224, 350, 570, 26, new SKColor(0xE6, 0xE6, 0xE6));
        d.Text("Name", 234, 368, "Segoe UI Semibold", 12, text);
        d.Text("Size", 460, 368, "Segoe UI Semibold", 12, text);
        d.Text("Date", 600, 368, "Segoe UI Semibold", 12, text);
        string[,] rows = { { "holiday.png", "1.2 MB", "2026-11-14" }, { "report.pdf", "384 KB", "2026-11-16" } };
        for (int r = 0; r < 2; r++)
        {
            d.Text(rows[r, 0], 234, 398 + r * 26, "Segoe UI", 12, text);
            d.Text(rows[r, 1], 460, 398 + r * 26, "Segoe UI", 12, text);
            d.Text(rows[r, 2], 600, 398 + r * 26, "Segoe UI", 12, text);
        }
        // Buttons.
        d.RoundRect(574, 446, 70, 28, 4, accent);
        d.Text("OK", 600, 465, "Segoe UI", 13, White);
        d.RoundRect(652, 446, 70, 28, 4, White, new SKColor(0xC8, 0xC8, 0xC8));
        d.Text("Cancel", 667, 465, "Segoe UI", 13, text);
        d.RoundRect(730, 446, 70, 28, 4, White, new SKColor(0xC8, 0xC8, 0xC8));
        d.Text("Apply", 748, 465, "Segoe UI", 13, text);
        // Status bar.
        d.Rect(0, 488, 820, 32, new SKColor(0xE0, 0xE0, 0xE0));
        d.Text("Ready", 12, 509, "Segoe UI", 12, sub);
        d.Text("3 items selected", 690, 509, "Segoe UI", 12, sub);
    }

    public static void Run(string outDir)
    {
        Directory.CreateDirectory(outDir);
        List<object> manifest = [];
        foreach (Case c in Cases())
        {
            List<string> fonts = [];
            SKImageInfo info = new(c.Width, c.Height, SKColorType.Bgra8888, c.Transparent ? SKAlphaType.Premul : SKAlphaType.Opaque);
            using SKBitmap bitmap = new(info);
            using (SKCanvas canvas = new(bitmap))
            {
                c.Paint(new Draw(canvas, fonts));
            }
            using SKImage image = SKImage.FromBitmap(bitmap);
            using SKData png = image.Encode(SKEncodedImageFormat.Png, 100);
            File.WriteAllBytes(Path.Combine(outDir, c.Name + ".png"), png.ToArray());
            manifest.Add(new { name = c.Name, file = c.Name + ".png", width = c.Width, height = c.Height, description = c.Description, fonts });
            Console.WriteLine($"{c.Name}: {c.Width}x{c.Height} {string.Join(", ", fonts)}");
        }
        File.WriteAllText(Path.Combine(outDir, "cases.json"), JsonSerializer.Serialize(manifest, new JsonSerializerOptions { WriteIndented = true }));
    }
}
