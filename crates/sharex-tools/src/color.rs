//! ShareX's colour maths and colour texts: `ShareX.HelpersLib/Helpers/
//! ColorHelpers.cs`, `Colors/HSB.cs`, `Colors/CMYK.cs`, `Colors/MyColor.cs`,
//! `NameParser/CodeMenuEntryPixelInfo.cs` (the screen colour picker's copy
//! formats and the region capture's custom info text) and
//! `ShareX.Avalonia/Windows/ScreenColorPickerTextFormatter.cs` (the screen
//! colour picker's info text).
//!
//! Colours are `System.Drawing.Color`s ([`Color`]): ARGB plus the name of a
//! colour made from one (`Color.Red`), which `Equals` and the settings JSON
//! (`ColorConverter`) tell apart from the same numbers.

pub use sharex_image_effects::dotnet::Color;
use sharex_image_effects::dotnet::KNOWN_COLORS;

use crate::text::{Arg, format_dotnet, format_double_g};

/// `ColorFormat`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorFormat {
    Rgb,
    Rgba,
    Argb,
}

/// `HelpersOptions.RecentColorsMax`.
pub const RECENT_COLORS_MAX: usize = 32;

/// `ColorHelpers.StandardColors`.
pub const STANDARD_COLORS: [(u8, u8, u8); 32] = [
    (0, 0, 0),
    (64, 64, 64),
    (255, 0, 0),
    (255, 106, 0),
    (255, 216, 0),
    (182, 255, 0),
    (76, 255, 0),
    (0, 255, 33),
    (0, 255, 144),
    (0, 255, 255),
    (0, 148, 255),
    (0, 38, 255),
    (72, 0, 255),
    (178, 0, 255),
    (255, 0, 220),
    (255, 0, 110),
    (255, 255, 255),
    (128, 128, 128),
    (127, 0, 0),
    (127, 51, 0),
    (127, 106, 0),
    (91, 127, 0),
    (38, 127, 0),
    (0, 127, 14),
    (0, 127, 70),
    (0, 127, 127),
    (0, 74, 127),
    (0, 19, 127),
    (33, 0, 127),
    (87, 0, 127),
    (127, 0, 110),
    (127, 0, 55),
];

/// `ColorHelpers.StandardColors` as colours (`Color.FromArgb(r, g, b)`).
pub fn standard_colors() -> Vec<Color> {
    STANDARD_COLORS.iter().map(|&(r, g, b)| Color::rgb(r, g, b)).collect()
}

/// `ColorHelpers.ColorToHex`.
pub fn color_to_hex(c: &Color, format: ColorFormat) -> String {
    match format {
        ColorFormat::Rgb => format!("{:02X}{:02X}{:02X}", c.r, c.g, c.b),
        ColorFormat::Rgba => format!("{:02X}{:02X}{:02X}{:02X}", c.r, c.g, c.b, c.a),
        ColorFormat::Argb => format!("{:02X}{:02X}{:02X}{:02X}", c.a, c.r, c.g, c.b),
    }
}

/// `ColorHelpers.ColorToDecimal` (an `int`: RGBA and ARGB can be negative).
pub fn color_to_decimal(c: &Color, format: ColorFormat) -> i32 {
    let (a, r, g, b) = (c.a as u32, c.r as u32, c.g as u32, c.b as u32);
    (match format {
        ColorFormat::Rgb => r << 16 | g << 8 | b,
        ColorFormat::Rgba => r << 24 | g << 16 | b << 8 | a,
        ColorFormat::Argb => a << 24 | r << 16 | g << 8 | b,
    }) as i32
}

/// `ColorHelpers.DecimalToColor`.
pub fn decimal_to_color(dec: i32, format: ColorFormat) -> Color {
    let d = dec as u32;
    let byte = |s: u32| ((d >> s) & 0xFF) as u8;
    match format {
        ColorFormat::Rgb => Color::rgb(byte(16), byte(8), byte(0)),
        ColorFormat::Rgba => Color::argb(byte(0), byte(24), byte(16), byte(8)),
        ColorFormat::Argb => Color::argb(byte(24), byte(16), byte(8), byte(0)),
    }
}

/// `ColorHelpers.HexToColor`: `None` for `Color.Empty` and for what
/// `Convert.ToInt32(hex, 16)` throws on.
pub fn hex_to_color(hex: &str, format: ColorFormat) -> Option<Color> {
    if hex.is_empty() {
        return None;
    }
    let hex = if let Some(h) = hex.strip_prefix('#') {
        h
    } else if hex.len() >= 2 && hex[..2].eq_ignore_ascii_case("0x") {
        &hex[2..]
    } else {
        hex
    };
    let n = hex.chars().count();
    if (format != ColorFormat::Rgb && n != 8) || (format == ColorFormat::Rgb && n != 6) {
        return None;
    }
    let part = |i: usize| -> Option<u8> {
        let s: String = hex.chars().skip(i).take(2).collect();
        // `Convert.ToInt32(s, 16)` also takes a "0x" prefix and gives
        // values over 255 to `FromArgb` (which throws): two hex digits.
        if s.len() == 2 && s.chars().all(|c| c.is_ascii_hexdigit()) { u8::from_str_radix(&s, 16).ok() } else { None }
    };
    Some(match format {
        ColorFormat::Rgb => Color::rgb(part(0)?, part(2)?, part(4)?),
        ColorFormat::Rgba => Color::argb(part(6)?, part(0)?, part(2)?, part(4)?),
        ColorFormat::Argb => Color::argb(part(0)?, part(2)?, part(4)?, part(6)?),
    })
}

/// `ColorHelpers.ParseColor`: `#RRGGBB`/`0xRRGGBB`/`RRGGBB`, or three
/// 0-255 numbers (`rgb(...)`, commas or spaces), at most 20 characters.
pub fn parse_color(text: &str) -> Option<Color> {
    let text = text.trim();
    if text.is_empty() || text.chars().count() > 20 {
        return None;
    }
    let hex = text.strip_prefix('#').or_else(|| {
        (text.len() >= 2 && text.is_char_boundary(2) && text[..2].eq_ignore_ascii_case("0x")).then(|| &text[2..])
    });
    let digits = hex.unwrap_or(text);
    if digits.len() == 6 && digits.chars().all(|c| c.is_ascii_hexdigit()) {
        return hex_to_color(digits, ColorFormat::Rgb);
    }
    // `^(?:rgb\()?(n)(?:\s|,)+(n)(?:\s|,)+(n)\)?$` with n in 0..=255 (no
    // leading zeros beyond what `[1]?[0-9]?[0-9]` allows).
    let body = text.strip_prefix("rgb(").unwrap_or(text);
    let body = body.strip_suffix(')').unwrap_or(body);
    let parts: Vec<&str> = body.split(|c: char| c == ',' || c.is_whitespace()).filter(|p| !p.is_empty()).collect();
    let separators_ok = {
        // Separators are runs of whitespace and commas only.
        body.chars().all(|c| c.is_ascii_digit() || c == ',' || c.is_whitespace())
            && !body.starts_with(|c: char| c == ',' || c.is_whitespace())
            && !body.ends_with(|c: char| c == ',' || c.is_whitespace())
    };
    if parts.len() != 3 || !separators_ok {
        return None;
    }
    let byte = |s: &str| -> Option<u8> {
        let ok = match s.len() {
            1 | 2 => true,
            3 => s.starts_with('1') || (s.starts_with('2') && s[1..].parse::<u8>().is_ok_and(|v| v <= 55)),
            _ => false,
        };
        if ok { s.parse::<u16>().ok().filter(|v| *v <= 255).map(|v| v as u8) } else { None }
    };
    Some(Color::rgb(byte(parts[0])?, byte(parts[1])?, byte(parts[2])?))
}

/// `ColorHelpers.ValidColor(double)`.
fn valid(v: f64) -> f64 {
    v.clamp(0.0, 1.0)
}

/// `Math.Round(double)`: to even.
fn round_even(v: f64) -> f64 {
    v.round_ties_even()
}

/// `HSB`: hue, saturation and brightness from 0 to 1, and alpha.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Hsb {
    pub hue: f64,
    pub saturation: f64,
    pub brightness: f64,
    pub alpha: i32,
}

impl Hsb {
    /// `new HSB(double, double, double, int)`.
    pub fn new(hue: f64, saturation: f64, brightness: f64, alpha: i32) -> Self {
        Self { hue: valid(hue), saturation: valid(saturation), brightness: valid(brightness), alpha: alpha.clamp(0, 255) }
    }

    /// `new HSB(int hue, int saturation, int brightness, int alpha)`: in
    /// degrees and percent.
    pub fn from_ints(hue: i32, saturation: i32, brightness: i32, alpha: i32) -> Self {
        Self::new(hue as f64 / 360.0, saturation as f64 / 100.0, brightness as f64 / 100.0, alpha)
    }

    pub fn hue360(&self) -> f64 {
        self.hue * 360.0
    }

    pub fn saturation100(&self) -> f64 {
        self.saturation * 100.0
    }

    pub fn brightness100(&self) -> f64 {
        self.brightness * 100.0
    }

    /// `ColorHelpers.ColorToHSB`.
    pub fn from_color(c: &Color) -> Self {
        let (r, g, b) = (c.r as i32, c.g as i32, c.b as i32);
        let (mut max, mut min) = if r > g { (r, g) } else { (g, r) };
        if b > max {
            max = b;
        } else if b < min {
            min = b;
        }
        let diff = max - min;
        let brightness = max as f64 / 255.0;
        let saturation = if max == 0 { 0.0 } else { diff as f64 / max as f64 };
        let q = if diff == 0 { 0.0 } else { 60.0 / diff as f64 };
        let hue = if max == r {
            if g < b { (360.0 + q * (g - b) as f64) / 360.0 } else { q * (g - b) as f64 / 360.0 }
        } else if max == g {
            (120.0 + q * (b - r) as f64) / 360.0
        } else if max == b {
            (240.0 + q * (r - g) as f64) / 360.0
        } else {
            0.0
        };
        // The setters clamp (`ValidColor`).
        Self::new(hue, saturation, brightness, c.a as i32)
    }

    /// `ColorHelpers.HSBToColor`.
    pub fn to_color(&self) -> Color {
        let a = self.alpha.clamp(0, 255) as u8;
        let max = round_even(self.brightness * 255.0) as i32;
        let min = round_even((1.0 - self.saturation) * (self.brightness / 1.0) * 255.0) as i32;
        let q = (max - min) as f64 / 255.0;
        let h = self.hue;
        // `FromArgb` throws past 255; these stay within 0..=255.
        let c = |v: i32| v.clamp(0, 255) as u8;
        let mid = |v: f64| round_even(v) as i32;
        if (0.0..=1.0 / 6.0).contains(&h) {
            let m = mid(((h - 0.0) * q) * 1530.0 + min as f64);
            return Color::argb(a, c(max), c(m), c(min));
        }
        if h <= 1.0 / 3.0 {
            let m = mid(-((h - 1.0 / 6.0) * q) * 1530.0 + max as f64);
            return Color::argb(a, c(m), c(max), c(min));
        }
        if h <= 0.5 {
            let m = mid(((h - 1.0 / 3.0) * q) * 1530.0 + min as f64);
            return Color::argb(a, c(min), c(max), c(m));
        }
        if h <= 2.0 / 3.0 {
            let m = mid(-((h - 0.5) * q) * 1530.0 + max as f64);
            return Color::argb(a, c(min), c(m), c(max));
        }
        if h <= 5.0 / 6.0 {
            let m = mid(((h - 2.0 / 3.0) * q) * 1530.0 + min as f64);
            return Color::argb(a, c(m), c(min), c(max));
        }
        if h <= 1.0 {
            let m = mid(-((h - 5.0 / 6.0) * q) * 1530.0 + max as f64);
            return Color::argb(a, c(max), c(min), c(m));
        }
        Color::argb(a, 0, 0, 0)
    }
}

/// `CMYK`: cyan, magenta, yellow and key from 0 to 1, and alpha.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Cmyk {
    pub cyan: f64,
    pub magenta: f64,
    pub yellow: f64,
    pub key: f64,
    pub alpha: i32,
}

impl Cmyk {
    pub fn new(cyan: f64, magenta: f64, yellow: f64, key: f64, alpha: i32) -> Self {
        Self { cyan: valid(cyan), magenta: valid(magenta), yellow: valid(yellow), key: valid(key), alpha: alpha.clamp(0, 255) }
    }

    /// `new CMYK(int, int, int, int, int)`: in percent.
    pub fn from_ints(c: i32, m: i32, y: i32, k: i32, alpha: i32) -> Self {
        Self::new(c as f64 / 100.0, m as f64 / 100.0, y as f64 / 100.0, k as f64 / 100.0, alpha)
    }

    pub fn cyan100(&self) -> f64 {
        self.cyan * 100.0
    }
    pub fn magenta100(&self) -> f64 {
        self.magenta * 100.0
    }
    pub fn yellow100(&self) -> f64 {
        self.yellow * 100.0
    }
    pub fn key100(&self) -> f64 {
        self.key * 100.0
    }

    /// `ColorHelpers.ColorToCMYK`.
    pub fn from_color(c: &Color) -> Self {
        if c.r == 0 && c.g == 0 && c.b == 0 {
            return Self::new(0.0, 0.0, 0.0, 1.0, c.a as i32);
        }
        let cy = 1.0 - c.r as f64 / 255.0;
        let m = 1.0 - c.g as f64 / 255.0;
        let y = 1.0 - c.b as f64 / 255.0;
        let k = cy.min(m.min(y));
        Self::new((cy - k) / (1.0 - k), (m - k) / (1.0 - k), (y - k) / (1.0 - k), k, c.a as i32)
    }

    /// `ColorHelpers.CMYKToColor`.
    pub fn to_color(&self) -> Color {
        let a = self.alpha.clamp(0, 255) as u8;
        if self.cyan == 0.0 && self.magenta == 0.0 && self.yellow == 0.0 && self.key == 1.0 {
            return Color::argb(a, 0, 0, 0);
        }
        let c = self.cyan * (1.0 - self.key) + self.key;
        let m = self.magenta * (1.0 - self.key) + self.key;
        let y = self.yellow * (1.0 - self.key) + self.key;
        let byte = |v: f64| (round_even((1.0 - v) * 255.0) as i32).clamp(0, 255) as u8;
        Color::argb(a, byte(c), byte(m), byte(y))
    }
}

/// `KnownColor.AliceBlue` to `KnownColor.YellowGreen` in enum order (the
/// web colours, without .NET 8's RebeccaPurple, which comes last).
fn web_colors() -> impl Iterator<Item = (&'static str, u32)> {
    KNOWN_COLORS
        .iter()
        .skip_while(|(n, _)| *n != "AliceBlue")
        .take_while(|(n, _)| *n != "ActiveBorder")
        .filter(|(n, _)| *n != "RebeccaPurple")
        .copied()
}

fn color_of(argb: u32) -> Color {
    Color::argb((argb >> 24) as u8, (argb >> 16) as u8, (argb >> 8) as u8, argb as u8)
}

/// `ColorHelpers.ColorDifference`.
pub fn color_difference(a: &Color, b: &Color) -> i32 {
    (a.r as i32 - b.r as i32).abs() + (a.g as i32 - b.g as i32).abs() + (a.b as i32 - b.b as i32).abs()
}

/// `ColorHelpers.FindClosestKnownColor`: the first web colour nearer than
/// all before it, starting from black.
pub fn find_closest_known_color(c: &Color) -> (&'static str, Color) {
    let mut best = ("Black", Color::rgb(0, 0, 0));
    for (name, argb) in web_colors() {
        let cand = color_of(argb);
        if color_difference(c, &cand) < color_difference(c, &best.1) {
            best = (name, cand);
        }
    }
    best
}

/// `ColorHelpers.GetColorName`: the closest web colour's proper name
/// ("Alice blue").
pub fn get_color_name(c: &Color) -> String {
    get_proper_name(find_closest_known_color(c).0, false)
}

/// `Helpers.GetProperName`: "TopLeft" becomes "Top left", "Rotate180"
/// becomes "Rotate 180".
pub fn get_proper_name(name: &str, keep_case: bool) -> String {
    let mut out = String::new();
    let mut number = false;
    for (i, c) in name.chars().enumerate() {
        if i > 0 && (c.is_uppercase() || (!number && c.is_numeric())) {
            out.push(' ');
            if keep_case {
                out.push(c);
            } else {
                out.extend(c.to_lowercase());
            }
        } else {
            out.push(c);
        }
        number = c.is_numeric();
    }
    out
}

/// `MyColor.ToString()`: every format of the colour (Copy all details).
pub fn my_color_to_string(c: &Color) -> String {
    let hsb = Hsb::from_color(c);
    let cmyk = Cmyk::from_color(c);
    format_dotnet(
        "RGBA (Red, Green, Blue, Alpha) = {0}, {1}, {2}, {3}\r\nHSB (Hue, Saturation, Brightness) = {4:0.0}°, {5:0.0}%, {6:0.0}%\r\nCMYK (Cyan, Magenta, Yellow, Key) = {7:0.0}%, {8:0.0}%, {9:0.0}%, {10:0.0}%\r\nHex (RGB, RGBA, ARGB) = #{11}, #{12}, #{13}\r\nDecimal (RGB, RGBA, ARGB) = {14}, {15}, {16}",
        &[
            Arg::from(c.r as i32),
            Arg::from(c.g as i32),
            Arg::from(c.b as i32),
            Arg::from(c.a as i32),
            Arg::F64(hsb.hue360()),
            Arg::F64(hsb.saturation100()),
            Arg::F64(hsb.brightness100()),
            Arg::F64(cmyk.cyan100()),
            Arg::F64(cmyk.magenta100()),
            Arg::F64(cmyk.yellow100()),
            Arg::F64(cmyk.key100()),
            Arg::from(color_to_hex(c, ColorFormat::Rgb)),
            Arg::from(color_to_hex(c, ColorFormat::Rgba)),
            Arg::from(color_to_hex(c, ColorFormat::Argb)),
            Arg::from(color_to_decimal(c, ColorFormat::Rgb)),
            Arg::from(color_to_decimal(c, ColorFormat::Rgba)),
            Arg::from(color_to_decimal(c, ColorFormat::Argb)),
        ],
    )
}

/// The colour picker window's Copy menu (`OnCopy*Click`).
pub fn copy_rgb(c: &Color) -> String {
    format!("{}, {}, {}", c.r, c.g, c.b)
}

pub fn copy_hex(c: &Color) -> String {
    format!("#{}", color_to_hex(c, ColorFormat::Rgb))
}

pub fn copy_decimal(c: &Color) -> String {
    color_to_decimal(c, ColorFormat::Rgb).to_string()
}

pub fn copy_hsb(c: &Color) -> String {
    let h = Hsb::from_color(c);
    format_dotnet("{0:0.0}°, {1:0.0}%, {2:0.0}%", &[Arg::F64(h.hue360()), Arg::F64(h.saturation100()), Arg::F64(h.brightness100())])
}

pub fn copy_cmyk(c: &Color) -> String {
    let k = Cmyk::from_color(c);
    format_dotnet(
        "{0:0.0}%, {1:0.0}%, {2:0.0}%, {3:0.0}%",
        &[Arg::F64(k.cyan100()), Arg::F64(k.magenta100()), Arg::F64(k.yellow100()), Arg::F64(k.key100())],
    )
}

/// `Environment.NewLine`.
pub const NEW_LINE: &str = if cfg!(windows) { "\r\n" } else { "\n" };

/// `string.Replace(old, new, comparison)` with an ASCII-case-insensitive
/// comparison (`OrdinalIgnoreCase`, and `InvariantCultureIgnoreCase` for
/// the ASCII tokens it's used with).
pub fn replace_ignore_case(s: &str, old: &str, new: &str) -> String {
    if old.is_empty() || s.is_empty() {
        return s.to_string();
    }
    let (hay, needle) = (s.to_ascii_lowercase(), old.to_ascii_lowercase());
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while let Some(p) = hay[i..].find(&needle) {
        out.push_str(&s[i..i + p]);
        out.push_str(new);
        i += p + old.len();
    }
    out.push_str(&s[i..]);
    out
}

/// `Math.Round(v, digits, MidpointRounding.AwayFromZero)`: scaled,
/// rounded half away from zero, scaled back (.NET Core's way).
pub fn round_away(v: f64, digits: i32) -> f64 {
    let p = 10f64.powi(digits);
    (v * p).round() / p
}

/// `StringExtensions.ForEachBetween(text, front, back)`: each
/// `front...back` and what's between.
fn for_each_between(text: &str, front: &str, back: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut f = 0;
    while text.len() > f {
        let Some(start) = text[f..].find(front).map(|p| p + f) else { break };
        let Some(b) = text[start + front.len()..].find(back).map(|p| p + start + front.len()) else { break };
        let whole = text[start..b + back.len()].to_string();
        let inner = whole[front.len()..whole.len() - back.len()].to_string();
        out.push((whole, inner));
        f = start + front.len();
        while !text.is_char_boundary(f) {
            f += 1;
        }
    }
    out
}

/// `CodeMenuEntryPixelInfo.Parse`: the screen colour picker's copy
/// formats (`Format`, `FormatCtrl`) and the region capture's custom info
/// text.
pub fn pixel_info_parse(input: &str, c: &Color, (x, y): (i32, i32)) -> String {
    let cmyk = Cmyk::from_color(c);
    let hex = color_to_hex(c, ColorFormat::Rgb);
    let pct = |v: f64| format_double_g(round_away(v, 2));
    // `InvariantCulture` (case-sensitive) for the upper-case hex tokens.
    let replace_exact = |s: &str, old: &str, new: &str| s.replace(old, new);
    let mut s = input.to_string();
    s = replace_ignore_case(&s, "$r255", &c.r.to_string());
    s = replace_ignore_case(&s, "$g255", &c.g.to_string());
    s = replace_ignore_case(&s, "$b255", &c.b.to_string());
    s = replace_exact(&s, "$rHEX", &format!("{:02X}", c.r));
    s = replace_exact(&s, "$gHEX", &format!("{:02X}", c.g));
    s = replace_exact(&s, "$bHEX", &format!("{:02X}", c.b));
    s = replace_ignore_case(&s, "$rhex", &format!("{:02x}", c.r));
    s = replace_ignore_case(&s, "$ghex", &format!("{:02x}", c.g));
    s = replace_ignore_case(&s, "$bhex", &format!("{:02x}", c.b));
    s = replace_exact(&s, "$HEX", &hex);
    s = replace_ignore_case(&s, "$hex", &hex.to_lowercase());
    s = replace_ignore_case(&s, "$c100", &pct(cmyk.cyan100()));
    s = replace_ignore_case(&s, "$m100", &pct(cmyk.magenta100()));
    s = replace_ignore_case(&s, "$y100", &pct(cmyk.yellow100()));
    s = replace_ignore_case(&s, "$k100", &pct(cmyk.key100()));
    s = replace_ignore_case(&s, "$name", &get_color_name(c));
    s = replace_ignore_case(&s, "$x", &x.to_string());
    s = replace_ignore_case(&s, "$y", &y.to_string());
    s = replace_ignore_case(&s, "$n", NEW_LINE);
    for (token, value) in [("$r1", c.r), ("$g1", c.g), ("$b1", c.b)] {
        // `ListEntryWithValue(input, "$r1")` over the text as it was.
        let entries = for_each_between(&s, &format!("{token}{{"), "}");
        for (whole, inner) in entries {
            let digits = inner.split(',').next().and_then(|v| v.trim().parse::<i32>().ok()).unwrap_or(0);
            let v = format_double_g(round_away(value as f64 / 255.0, digits.clamp(0, 15)));
            s = replace_ignore_case(&s, &whole, &v);
        }
    }
    for (token, value) in [("$r1", c.r), ("$g1", c.g), ("$b1", c.b)] {
        s = replace_ignore_case(&s, token, &format_double_g(round_away(value as f64 / 255.0, 3)));
    }
    s = replace_ignore_case(&s, "$r", &c.r.to_string());
    s = replace_ignore_case(&s, "$g", &c.g.to_string());
    s = replace_ignore_case(&s, "$b", &c.b.to_string());
    s
}

/// `int.TryParse` of the `\d+` group (an `int` overflow fails).
fn parse_precision(s: &str) -> Option<i32> {
    s.parse::<i32>().ok()
}

/// `ScreenColorPickerTextFormatter.Format`: the screen colour picker's
/// info text (`InfoText`).
pub fn screen_picker_format(format: &str, c: &Color, (x, y): (i32, i32)) -> String {
    if format.is_empty() {
        return String::new();
    }
    let upper = format!("{:02X}{:02X}{:02X}", c.r, c.g, c.b);
    let (cy, m, ye, k) = to_cmyk(c);
    let pct = |v: f64| format_double_g(round_away(v * 100.0, 2));
    let mut s = format.to_string();
    s = replace_ignore_case(&s, "$r255", &c.r.to_string());
    s = replace_ignore_case(&s, "$g255", &c.g.to_string());
    s = replace_ignore_case(&s, "$b255", &c.b.to_string());
    s = s.replace("$rHEX", &format!("{:02X}", c.r));
    s = s.replace("$gHEX", &format!("{:02X}", c.g));
    s = s.replace("$bHEX", &format!("{:02X}", c.b));
    s = replace_ignore_case(&s, "$rhex", &format!("{:02x}", c.r));
    s = replace_ignore_case(&s, "$ghex", &format!("{:02x}", c.g));
    s = replace_ignore_case(&s, "$bhex", &format!("{:02x}", c.b));
    s = s.replace("$HEX", &upper);
    s = replace_ignore_case(&s, "$hex", &upper.to_lowercase());
    s = replace_ignore_case(&s, "$c100", &pct(cy));
    s = replace_ignore_case(&s, "$m100", &pct(m));
    s = replace_ignore_case(&s, "$y100", &pct(ye));
    s = replace_ignore_case(&s, "$k100", &pct(k));
    // `GetColorName` (ShareX #9322, 8a46e17, fixing #9318): the closest web
    // colour's name, as `ColorHelpers.GetColorName` gives it ("Alice blue"),
    // where it was the name of a `KnownColor` with exactly this value
    // (usually none).
    s = replace_ignore_case(&s, "$name", &get_color_name(c));
    s = replace_ignore_case(&s, "$x", &x.to_string());
    s = replace_ignore_case(&s, "$y", &y.to_string());
    s = replace_ignore_case(&s, "$n", NEW_LINE);
    s = replace_normalized_components(&s, c);
    s = replace_ignore_case(&s, "$r", &c.r.to_string());
    s = replace_ignore_case(&s, "$g", &c.g.to_string());
    replace_ignore_case(&s, "$b", &c.b.to_string())
}

/// `ReplaceNormalizedComponents`: `\$(r|g|b)1(?:\{(\d+)\})?`, ignoring case.
fn replace_normalized_components(text: &str, c: &Color) -> String {
    let b = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'$' && matches!(b.get(i + 1).map(u8::to_ascii_lowercase), Some(b'r' | b'g' | b'b')) && b.get(i + 2) == Some(&b'1') {
            let component = b[i + 1].to_ascii_lowercase();
            let mut end = i + 3;
            let mut precision = 3;
            if b.get(end) == Some(&b'{') {
                let digits_end = (end + 1..b.len()).find(|&j| !b[j].is_ascii_digit()).unwrap_or(b.len());
                if digits_end > end + 1 && b.get(digits_end) == Some(&b'}') {
                    if let Some(p) = parse_precision(&text[end + 1..digits_end]) {
                        precision = p.clamp(0, 15);
                    }
                    end = digits_end + 1;
                }
            }
            let v = match component {
                b'r' => c.r,
                b'g' => c.g,
                _ => c.b,
            };
            out.push_str(&format_double_g(round_away(v as f64 / 255.0, precision)));
            i = end;
            continue;
        }
        let ch = text[i..].chars().next().unwrap_or('\0');
        out.push(ch);
        i += ch.len_utf8().max(1);
    }
    out
}

/// `ScreenColorPickerTextFormatter.ToCmyk`.
fn to_cmyk(c: &Color) -> (f64, f64, f64, f64) {
    let (r, g, b) = (c.r as f64 / 255.0, c.g as f64 / 255.0, c.b as f64 / 255.0);
    let key = 1.0 - r.max(g.max(b));
    if key >= 1.0 {
        return (0.0, 0.0, 0.0, 1.0);
    }
    let d = 1.0 - key;
    ((1.0 - r - key) / d, (1.0 - g - key) / d, (1.0 - b - key) / d, key)
}

/// `HelpersOptions.RecentColors` (`AddRecentColor`): the colour first,
/// once (`List.Remove` uses `Color.Equals`, so a named colour and the same
/// numbers are different entries), at most [`RECENT_COLORS_MAX`].
pub fn add_recent_color(recent: &mut Vec<Color>, c: &Color) {
    if let Some(i) = recent.iter().position(|r| r == c) {
        recent.remove(i);
    }
    recent.insert(0, c.clone());
    recent.truncate(RECENT_COLORS_MAX);
}

/// `ApplicationConfig.RecentColors` as Json.NET reads it (`ColorConverter`).
pub fn recent_colors_from_json(values: &[String]) -> Vec<Color> {
    values.iter().filter_map(|v| Color::parse(v)).collect()
}

/// `ApplicationConfig.RecentColors` as Json.NET writes it.
pub fn recent_colors_to_json(colors: &[Color]) -> Vec<String> {
    colors.iter().map(|c| c.to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hsb_round_trips_sharex_colours() {
        let c = Color::rgb(54, 169, 225);
        let h = Hsb::from_color(&c);
        assert!((h.hue360() - 199.64912280701756).abs() < 1e-9);
        assert_eq!(h.to_color(), c);
        let red = Hsb::from_ints(0, 100, 100, 255).to_color();
        assert_eq!((red.r, red.g, red.b), (255, 0, 0));
    }

    #[test]
    fn cmyk_of_black_is_key_only() {
        let k = Cmyk::from_color(&Color::rgb(0, 0, 0));
        assert_eq!((k.cyan, k.key), (0.0, 1.0));
        assert_eq!(Cmyk::from_ints(0, 100, 100, 0, 255).to_color(), Color::rgb(255, 0, 0));
    }

    #[test]
    fn closest_names() {
        assert_eq!(get_color_name(&Color::rgb(240, 248, 255)), "Alice blue");
        assert_eq!(get_color_name(&Color::rgb(1, 1, 1)), "Black");
        assert_eq!(get_proper_name("Rotate180", false), "Rotate 180");
    }

    #[test]
    fn my_color_text() {
        let s = my_color_to_string(&Color::rgb(255, 0, 0));
        assert!(s.starts_with("RGBA (Red, Green, Blue, Alpha) = 255, 0, 0, 255\r\nHSB (Hue, Saturation, Brightness) = 0.0°, 100.0%, 100.0%"));
        assert!(s.ends_with("Decimal (RGB, RGBA, ARGB) = 16711680, -16776961, -65536"));
    }

    /// ShareX's colour picker window's Copy menu for (76, 255, 0), read off
    /// the clipboard of the reference build.
    #[test]
    fn copy_formats_match_sharex() {
        let c = Color::rgb(76, 255, 0);
        assert_eq!(
            my_color_to_string(&c),
            "RGBA (Red, Green, Blue, Alpha) = 76, 255, 0, 255\r\nHSB (Hue, Saturation, Brightness) = 102.1°, 100.0%, 100.0%\r\nCMYK (Cyan, Magenta, Yellow, Key) = 70.2%, 0.0%, 100.0%, 0.0%\r\nHex (RGB, RGBA, ARGB) = #4CFF00, #4CFF00FF, #FF4CFF00\r\nDecimal (RGB, RGBA, ARGB) = 5046016, 1291780351, -11731200"
        );
        assert_eq!(copy_rgb(&c), "76, 255, 0");
        assert_eq!(copy_hex(&c), "#4CFF00");
        assert_eq!(copy_hsb(&c), "102.1°, 100.0%, 100.0%");
        assert_eq!(copy_cmyk(&c), "70.2%, 0.0%, 100.0%, 0.0%");
        assert_eq!(copy_decimal(&c), "5046016");
        assert_eq!(get_color_name(&c), "Chartreuse");
    }

    #[test]
    fn pixel_info() {
        let c = Color::rgb(54, 169, 225);
        assert_eq!(pixel_info_parse("$HEX", &c, (1, 2)), "36A9E1");
        assert_eq!(pixel_info_parse("$r255, $g255, $b255", &c, (1, 2)), "54, 169, 225");
        assert_eq!(pixel_info_parse("$hex $x,$y $r1 $g1{2} $c100", &c, (10, 20)), "36a9e1 10,20 0.212 0.66 76");
        assert_eq!(screen_picker_format("#$HEX", &c, (1, 2)), "#36A9E1");
        assert_eq!(screen_picker_format("$R1{1} $b $y", &c, (3, 4)), "0.2 225 4");
        // ShareX #9322: the closest named colour, also for colours without a name.
        assert_eq!(screen_picker_format("$name", &c, (0, 0)), "Royal blue");
        assert_eq!(screen_picker_format("$NAME", &Color::rgb(240, 248, 255), (0, 0)), "Alice blue");
        assert_eq!(screen_picker_format("$name", &Color::rgb(0, 255, 255), (0, 0)), "Aqua");
        assert_eq!(screen_picker_format("$name", &Color::rgb(2, 1, 0), (0, 0)), "Black");
        // ShareX ca3f01f's picker over the test pattern, `#$HEX $name`.
        for (c, text) in [((44, 44, 200), "#2C2CC8 Medium blue"), ((232, 88, 70), "#E85846 Tomato"), ((129, 188, 70), "#81BC46 Yellow green")] {
            assert_eq!(screen_picker_format("#$HEX $name", &Color::rgb(c.0, c.1, c.2), (0, 0)), text);
        }
    }

    #[test]
    fn parsing() {
        assert_eq!(parse_color("#ff0000"), Some(Color::rgb(255, 0, 0)));
        assert_eq!(parse_color("rgb(1, 2,3)"), Some(Color::rgb(1, 2, 3)));
        assert_eq!(parse_color("256 0 0"), None);
        assert_eq!(hex_to_color("#11223344", ColorFormat::Rgba), Some(Color::argb(0x44, 0x11, 0x22, 0x33)));
        assert_eq!(decimal_to_color(0x123456, ColorFormat::Rgb), Color::rgb(0x12, 0x34, 0x56));
    }

    #[test]
    fn recent_colours_keep_named_and_numbered_apart() {
        let mut recent = vec![Color::named("Red")];
        add_recent_color(&mut recent, &Color::rgb(255, 0, 0));
        assert_eq!(recent_colors_to_json(&recent), ["255, 0, 0", "Red"]);
        assert_eq!(recent_colors_from_json(&["255, 0, 0".into()])[0], Color::named("Red"));
    }
}
