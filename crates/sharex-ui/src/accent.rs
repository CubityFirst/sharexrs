//! Colours derived from the user's accent colour: a port of
//! `ShareX.Avalonia/Theming/AccentPalette.cs`.
//!
//! Text meets WCAG AA contrast whatever accent is picked: text on an accent
//! fill reaches 4.5:1 and accent coloured text on a theme surface reaches
//! 4.5:1. Accent coloured indicators (borders, rings) on a theme surface
//! reach 3:1. Accent coloured icons on a theme surface use the accent as
//! picked.

use egui::Color32;

pub const TEXT_CONTRAST: f64 = 4.5;
pub const NON_TEXT_CONTRAST: f64 = 3.0;
pub const GRADIENT_DARKEN_AMOUNT: f64 = 0.10;

pub const LIGHT_TEXT: Color32 = Color32::from_rgb(0xFF, 0xFF, 0xFF);
pub const DARK_TEXT: Color32 = Color32::from_rgb(0x11, 0x11, 0x11);

const STEPS: i32 = 100;

/// `AccentPalette`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AccentPalette {
    /// The user's accent colour, made opaque.
    pub accent: Color32,
    /// Start of the accent fill (`ShareX.Color.Accent.Start`,
    /// `SystemAccentColor`).
    pub fill_start: Color32,
    /// End of the fill gradient, slightly darker (`ShareX.Color.Accent.End`).
    pub fill_end: Color32,
    /// Text and icons on the fill (`ShareX.Color.Accent.Foreground`).
    pub on_fill: Color32,
    /// Accent text on a theme surface (`ShareX.Color.Accent.Text`).
    pub text: Color32,
    /// Accent borders and rings on a theme surface (`ShareX.Color.Accent.Border`).
    pub border: Color32,
    /// `SystemAccentColorLight1..3`.
    pub fill_light: [Color32; 3],
    /// `SystemAccentColorDark1..3`.
    pub fill_dark: [Color32; 3],
}

impl AccentPalette {
    /// Icons on a theme surface use the accent as picked.
    pub fn icon(&self) -> Color32 {
        self.accent
    }

    /// `AccentPalette.Create`: `surfaces` are the theme backgrounds accent
    /// text and indicators are drawn on (`ThemeManager.AccentSurfaceKeys`).
    pub fn create(accent: Color32, surfaces: &[Color32]) -> Self {
        assert!(!surfaces.is_empty(), "at least one surface colour is required");
        let accent = opaque(accent);
        let (fill_start, on_fill) = get_fill(accent);
        let fill_end = darken(fill_start, GRADIENT_DARKEN_AMOUNT);
        let readable = |fill: Color32| contrast_ratio(on_fill, fill) >= TEXT_CONTRAST;
        let white = Color32::WHITE;
        let black = Color32::BLACK;
        let fill_light = [0.10, 0.20, 0.30].map(|a| approach(fill_start, white, a, readable));
        let fill_dark = [0.10, 0.20, 0.30].map(|a| approach(fill_start, black, a, readable));
        let surfaces: Vec<Color32> = surfaces.iter().copied().map(opaque).collect();
        let average = surfaces.iter().map(|&c| relative_luminance(c)).sum::<f64>() / surfaces.len() as f64;
        let away = if average < 0.5 { white } else { black };
        let text = reach(accent, away, |c| meets_all(c, &surfaces, TEXT_CONTRAST));
        let border = reach(fill_start, away, |c| meets_all(c, &surfaces, NON_TEXT_CONTRAST));
        Self { accent, fill_start, fill_end, on_fill, text, border, fill_light, fill_dark }
    }
}

/// `GetTextColorOn`: white or near-black text, whichever reads better.
pub fn text_color_on(background: Color32) -> Color32 {
    if contrast_ratio(LIGHT_TEXT, background) >= contrast_ratio(DARK_TEXT, background) { LIGHT_TEXT } else { DARK_TEXT }
}

pub fn contrast_ratio(a: Color32, b: Color32) -> f64 {
    let (la, lb) = (relative_luminance(a), relative_luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

pub fn relative_luminance(c: Color32) -> f64 {
    fn linearize(channel: u8) -> f64 {
        let v = channel as f64 / 255.0;
        if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    }
    0.2126 * linearize(c.r()) + 0.7152 * linearize(c.g()) + 0.0722 * linearize(c.b())
}

/// White or near-black text, whichever needs the smaller change to the
/// accent to reach 4.5:1 against both gradient stops.
fn get_fill(accent: Color32) -> (Color32, Color32) {
    let light = nudge(accent, LIGHT_TEXT, Color32::BLACK);
    let dark = nudge(accent, DARK_TEXT, Color32::WHITE);
    if light.1 != dark.1 {
        return if light.1 < dark.1 { (light.0, LIGHT_TEXT) } else { (dark.0, DARK_TEXT) };
    }
    if fill_contrast(LIGHT_TEXT, light.0) >= fill_contrast(DARK_TEXT, dark.0) {
        (light.0, LIGHT_TEXT)
    } else {
        (dark.0, DARK_TEXT)
    }
}

fn nudge(accent: Color32, text: Color32, target: Color32) -> (Color32, i32) {
    for step in 0..=STEPS {
        let fill = blend(accent, target, step as f64 / STEPS as f64);
        if fill_contrast(text, fill) >= TEXT_CONTRAST {
            return (fill, step);
        }
    }
    (target, STEPS + 1)
}

fn fill_contrast(text: Color32, fill_start: Color32) -> f64 {
    contrast_ratio(text, fill_start).min(contrast_ratio(text, darken(fill_start, GRADIENT_DARKEN_AMOUNT)))
}

/// The closest blend from `color` toward `target` that satisfies `ok`.
fn reach(color: Color32, target: Color32, ok: impl Fn(Color32) -> bool) -> Color32 {
    (0..=STEPS).map(|s| blend(color, target, s as f64 / STEPS as f64)).find(|&c| ok(c)).unwrap_or(target)
}

/// The furthest blend toward `target`, up to `max`, that still satisfies `ok`.
fn approach(color: Color32, target: Color32, max: f64, ok: impl Fn(Color32) -> bool) -> Color32 {
    let mut result = color;
    let max_step = (max * STEPS as f64).round() as i32;
    for step in 1..=max_step {
        let c = blend(color, target, step as f64 / STEPS as f64);
        if !ok(c) {
            break;
        }
        result = c;
    }
    result
}

fn meets_all(c: Color32, surfaces: &[Color32], ratio: f64) -> bool {
    surfaces.iter().all(|&s| contrast_ratio(c, s) >= ratio)
}

fn opaque(c: Color32) -> Color32 {
    Color32::from_rgb(c.r(), c.g(), c.b())
}

/// C#'s `Math.Round` (banker's rounding) to a byte.
fn round_byte(v: f64) -> u8 {
    v.round_ties_even().clamp(0.0, 255.0) as u8
}

fn darken(c: Color32, amount: f64) -> Color32 {
    let f = (1.0 - amount).clamp(0.0, 1.0);
    Color32::from_rgb(round_byte(c.r() as f64 * f), round_byte(c.g() as f64 * f), round_byte(c.b() as f64 * f))
}

fn blend(from: Color32, to: Color32, amount: f64) -> Color32 {
    let ch = |a: u8, b: u8| round_byte(a as f64 + (b as f64 - a as f64) * amount);
    Color32::from_rgb(ch(from.r(), to.r()), ch(from.g(), to.g()), ch(from.b(), to.b()))
}

/// Parses `#RGB`, `#RRGGBB` or `#AARRGGBB` as Avalonia's `Color.TryParse`
/// does for hex, to paint with ([`parse_hex_rgba`] keeps the exact colour).
pub fn parse_hex(s: &str) -> Option<Color32> {
    parse_hex_rgba(s).map(crate::color_view::to_color32)
}

/// [`parse_hex`] as unmultiplied `[R, G, B, A]` (lossless).
pub fn parse_hex_rgba(s: &str) -> Option<[u8; 4]> {
    let h = s.trim().strip_prefix('#')?;
    let v = u32::from_str_radix(h, 16).ok()?;
    let (a, r, g, b) = match h.len() {
        3 => (255, ((v >> 8) & 0xF) * 17, ((v >> 4) & 0xF) * 17, (v & 0xF) * 17),
        4 => (((v >> 12) & 0xF) * 17, ((v >> 8) & 0xF) * 17, ((v >> 4) & 0xF) * 17, (v & 0xF) * 17),
        6 => (255, v >> 16, (v >> 8) & 0xFF, v & 0xFF),
        8 => (v >> 24, (v >> 16) & 0xFF, (v >> 8) & 0xFF, v & 0xFF),
        _ => return None,
    };
    Some([r as u8, g as u8, b as u8, a as u8])
}

/// `ApplicationThemeOptions.AccentColor`'s setter: `#AARRGGBB` of an
/// unmultiplied `[R, G, B, A]`.
pub fn to_hex(c: [u8; 4]) -> String {
    let [r, g, b, a] = c;
    format!("#{a:02X}{r:02X}{g:02X}{b:02X}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{DARK, LIGHT};

    /// The default accent reproduces the colours ShareXTheme.axaml lists.
    #[test]
    fn default_accent_matches_the_theme_files() {
        let accent = parse_hex("#09AAFF").unwrap();
        let dark = AccentPalette::create(accent, &crate::theme::accent_surfaces(&DARK));
        assert_eq!(dark.fill_start, DARK.accent_start);
        assert_eq!(dark.fill_end, DARK.accent_end);
        assert_eq!(dark.on_fill, DARK.accent_foreground);
        assert_eq!(dark.text, DARK.accent_text);
        assert_eq!(dark.border, DARK.accent_border);
        let light = AccentPalette::create(accent, &crate::theme::accent_surfaces(&LIGHT));
        assert_eq!(light.text, LIGHT.accent_text);
        assert_eq!(light.border, LIGHT.accent_border);
    }

    #[test]
    fn hex_round_trip() {
        let c = parse_hex_rgba("#6366F1").unwrap();
        assert_eq!(to_hex(c), "#FF6366F1");
        assert_eq!(parse_hex_rgba(&to_hex(c)), Some(c));
    }

    /// Partial alpha survives: `Color32` would premultiply it away.
    #[test]
    fn hex_round_trip_keeps_partial_alpha() {
        for a in [0u8, 1, 64, 128, 166, 254] {
            for v in 0..=255u8 {
                let c = [v, 255 - v, v / 2, a];
                assert_eq!(parse_hex_rgba(&to_hex(c)), Some(c));
            }
        }
        assert_eq!(parse_hex_rgba("#A636A9E1"), Some([54, 169, 225, 166]));
    }
}
