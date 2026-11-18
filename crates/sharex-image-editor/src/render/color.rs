//! `Avalonia.Media.Color.Parse` (and `SKColor.Parse`, which reads the same
//! hex forms): `#RGB`, `#ARGB`, `#RRGGBB`, `#AARRGGBB` and named colours.

use sharex_skia::Color;

/// Parses a colour as ShareX's annotations store it; unparseable colours
/// are transparent.
pub fn parse_color(s: &str) -> Color {
    try_parse(s).unwrap_or(Color::argb(0, 0, 0, 0))
}

pub fn try_parse(s: &str) -> Option<Color> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix('#') {
        let v = u32::from_str_radix(hex, 16).ok()?;
        let nib = |i: u32| ((v >> (i * 4)) & 0xF) as u8 * 17;
        return match hex.len() {
            3 => Some(Color::argb(255, nib(2), nib(1), nib(0))),
            4 => Some(Color::argb(nib(3), nib(2), nib(1), nib(0))),
            6 => Some(Color::argb(255, (v >> 16) as u8, (v >> 8) as u8, v as u8)),
            8 => Some(Color::argb(
                (v >> 24) as u8,
                (v >> 16) as u8,
                (v >> 8) as u8,
                v as u8,
            )),
            _ => None,
        };
    }
    // KnownColors (the ones ShareX's editor uses; Avalonia knows all of CSS's).
    let named: u32 = match s.to_ascii_lowercase().as_str() {
        "transparent" => 0x00FF_FFFF,
        "black" => 0xFF00_0000,
        "white" => 0xFFFF_FFFF,
        "red" => 0xFFFF_0000,
        "yellow" => 0xFFFF_FF00,
        "lime" => 0xFF00_FF00,
        "green" => 0xFF00_8000,
        "blue" => 0xFF00_00FF,
        "gray" | "grey" => 0xFF80_8080,
        _ => return None,
    };
    Some(Color::argb(
        (named >> 24) as u8,
        (named >> 16) as u8,
        (named >> 8) as u8,
        named as u8,
    ))
}
