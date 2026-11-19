//! Avalonia's `ColorView` (`Avalonia.Controls.ColorPicker` 12.1.6, the
//! version ShareX builds with) and the three ways ShareX hosts it.
//!
//! Ported from the assembly's Fluent theme (`Themes/Fluent/ColorView.xaml`,
//! `ColorSpectrum.xaml`, `ColorSlider.xaml`, `ColorPreviewer.xaml`,
//! `ColorPicker.xaml`, `Fluent.xaml`) and its code (`ColorView`,
//! `ColorSpectrum`, `ColorSlider`, `ColorPreviewer`, `ColorPickerHelpers`,
//! `ColorHelper`, `Hsv`, `Rgb`, `FluentColorPalette`, the converters), with
//! `HsvColor`/`Color` from `Avalonia.Base`:
//!
//! - the tab strip (spectrum, palette, components; Fluent UI System Icons),
//! - the spectrum tab: the `ColorSpectrum` box (hue across, saturation
//!   down, `HueSaturation`), the value slider on its left and the alpha
//!   slider on its right (`ColorSlider`s, vertical),
//! - the palette tab: `FluentColorPalette`, 6 columns of 8 shades, with the
//!   contrast selection rectangle,
//! - the components tab: RGB/HSV toggle buttons, the hex box (alpha
//!   trailing, as CSS), and a label, number box and slider per component,
//! - the `ColorPreviewer` (the colour with its value accents and shadow).
//!
//! The hosts, as [`ColorPopup`]: `ColorPickerDropdown`'s popup
//! (`ColorPickerPanel`: the view on its palette tab, the screen colour
//! picker button and the recent colours), a `Flyout` holding a `ColorView`
//! (Task settings' mouse highlighter, Pin to screen, the Monitor test, the
//! image effects), and Avalonia's `ColorPicker` button's own flyout (the
//! image tools' colours).
//!
//! The control's strings ("RGB", "HSV", "R", "G", "B", "A", "H", "S", "V",
//! "#") are written in Avalonia's template, not ShareX's `.resx`.
//!
//! Colour maths follows .NET: `Math.Round` rounds half to even unless the
//! source asks for `AwayFromZero`, and casts to `byte` truncate.

use std::sync::Mutex;

use egui::{Color32, ColorImage, Id, Key, Order, Pos2, Rect, Sense, TextureHandle, TextureOptions, Ui, Vec2, pos2, vec2};

use crate::border::{self, Brush, Part};
use crate::controls::content_font;
use crate::keys::app_icons;
use crate::theme::{Thickness, fluent, palette};
use crate::widgets::{center_y, font, galley, line_height, text_width};
use crate::{focus, icons};

// ---------------------------------------------------------------------------
// Colour maths
// ---------------------------------------------------------------------------

/// `Avalonia.Media.HsvColor`: alpha 0..1, hue 0..360, saturation and value
/// 0..1, in doubles.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hsv {
    pub a: f64,
    pub h: f64,
    pub s: f64,
    pub v: f64,
}

impl Hsv {
    /// `new HsvColor(alpha, hue, saturation, value)`: each clamped, a hue of
    /// 360 made 0.
    pub fn new(a: f64, h: f64, s: f64, v: f64) -> Self {
        let h = h.clamp(0.0, 360.0);
        Self { a: a.clamp(0.0, 1.0), h: if h == 360.0 { 0.0 } else { h }, s: s.clamp(0.0, 1.0), v: v.clamp(0.0, 1.0) }
    }

    /// `Color.ToHsv()`: an `[R, G, B, A]` colour in HSV (unclamped, as
    /// `HsvColor.ToHsv` builds it).
    pub fn from_rgba(c: [u8; 4]) -> Self {
        let [r, g, b, a] = c.map(|v| v as f64 / 255.0);
        let max = if r >= g { if r >= b { r } else { b } } else if g >= b { g } else { b };
        let min = if r <= g { if r <= b { r } else { b } } else if g <= b { g } else { b };
        let delta = max - min;
        let (h, s) = if delta == 0.0 {
            (0.0, 0.0)
        } else {
            let mut h = if r == max {
                60.0 * (g - b) / delta
            } else if g == max {
                120.0 + 60.0 * (b - r) / delta
            } else {
                240.0 + 60.0 * (r - g) / delta
            };
            if h < 0.0 {
                h += 360.0;
            }
            (h, delta / max)
        };
        Self { a, h, s, v: max }
    }

    /// `HsvColor.ToRgb()`, as `[R, G, B, A]`.
    pub fn to_rgba(&self) -> [u8; 4] {
        hsv_to_rgba(self.h, self.s, self.v, self.a)
    }
}

/// .NET's `(byte)Math.Round(x)` of a value already in 0..255.
fn round_byte(x: f64) -> u8 {
    x.round_ties_even().clamp(0.0, 255.0) as u8
}

/// `HsvColor.ToRgb(hue, saturation, value, alpha)`.
pub fn hsv_to_rgba(hue: f64, saturation: f64, value: f64, alpha: f64) -> [u8; 4] {
    let mut hue = hue;
    while hue >= 360.0 {
        hue -= 360.0;
    }
    while hue < 0.0 {
        hue += 360.0;
    }
    let s = saturation.clamp(0.0, 1.0);
    let v = value.clamp(0.0, 1.0);
    let a = alpha.clamp(0.0, 1.0);
    let chroma = s * v;
    let m = v - chroma;
    if chroma == 0.0 {
        let g = round_byte(m * 255.0);
        return [g, g, g, round_byte(a * 255.0)];
    }
    let sector = (hue / 60.0) as i32;
    let f = hue / 60.0 - sector as f64;
    let max = chroma + m;
    let (r, g, b) = match sector {
        0 => (max, m + chroma * f, m),
        1 => (m + chroma * (1.0 - f), max, m),
        2 => (m, max, m + chroma * f),
        3 => (m, m + chroma * (1.0 - f), max),
        4 => (m + chroma * f, m, max),
        5 => (max, m, m + chroma * (1.0 - f)),
        _ => (0.0, 0.0, 0.0),
    };
    [round_byte(r * 255.0), round_byte(g * 255.0), round_byte(b * 255.0), round_byte(a * 255.0)]
}

/// .NET's `Math.Round(value, digits)` (to even).
fn round_digits_even(value: f64, digits: i32) -> f64 {
    let p = 10f64.powi(digits);
    (value * p).round_ties_even() / p
}

/// `Math.Round(value, digits, MidpointRounding.AwayFromZero)`.
fn round_digits_away(value: f64, digits: i32) -> f64 {
    let p = 10f64.powi(digits);
    (value * p).round() / p
}

/// `AccentColorConverter.GetAccent`: the value moved 0.1 a step, rounded
/// to two places.
pub fn accent(hsv: Hsv, step: i32) -> Hsv {
    if step == 0 {
        return hsv;
    }
    let v = round_digits_even(hsv.v + step as f64 * 0.1, 2);
    Hsv::new(hsv.a, hsv.h, hsv.s, v)
}

/// `ColorHelper.GetRelativeLuminance`.
pub fn relative_luminance(c: [u8; 4]) -> f64 {
    let channel = |v: u8| if v <= 10 { v as f64 / 3294.0 } else { (v as f64 / 269.0 + 0.0513).powf(2.4) };
    0.2126 * channel(c[0]) + 0.7152 * channel(c[1]) + 0.0722 * channel(c[2])
}

/// `ContrastBrushConverter` without a default colour: black on a light
/// colour, white on a dark one.
pub fn contrast(c: [u8; 4]) -> Color32 {
    if relative_luminance(c) <= 0.5 { Color32::WHITE } else { Color32::BLACK }
}

/// `ColorToHexConverter.ToHexString` with the alpha trailing (the theme's
/// `HexInputAlphaPosition`): `RRGGBBAA`, or `RRGGBB` without alpha.
pub fn to_hex_string(c: [u8; 4], include_alpha: bool, include_symbol: bool) -> String {
    let [r, g, b, a] = c;
    let s = if include_alpha { format!("{r:02X}{g:02X}{b:02X}{a:02X}") } else { format!("{r:02X}{g:02X}{b:02X}") };
    if include_symbol { format!("#{s}") } else { s }
}

/// `ColorToHexConverter.ParseHexString` with the alpha trailing: `#RGB`,
/// `#RGBA`, `#RRGGBB` or `#RRGGBBAA`, the `#` optional; `None` otherwise.
pub fn parse_hex_string(text: &str) -> Option<[u8; 4]> {
    let t = text.trim();
    let t = t.strip_prefix('#').unwrap_or(t);
    let chars: Vec<char> = t.chars().collect();
    let expanded: Vec<char> = if chars.len() == 3 || chars.len() == 4 { chars.iter().flat_map(|c| [*c, *c]).collect() } else { chars };
    let opaque = match expanded.len() {
        6 => true,
        8 => false,
        _ => return None,
    };
    // `NumberStyles.HexNumber`: white space allowed around the digits.
    let s: String = expanded.iter().collect();
    let digits = s.trim_matches(|c: char| matches!(c, ' ' | '\t' | '\n' | '\r' | '\u{b}' | '\u{c}'));
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let mut v = u32::from_str_radix(digits, 16).ok()?;
    if opaque {
        v = (v << 8) | 0xFF;
    }
    Some([(v >> 24) as u8, (v >> 16) as u8, (v >> 8) as u8, v as u8])
}

/// `ColorSlider.RoundComponentValues` (away from zero).
fn round_components(h: Hsv) -> Hsv {
    Hsv::new(round_digits_away(h.a, 2), round_digits_away(h.h, 0), round_digits_away(h.s, 2), round_digits_away(h.v, 2))
}

/// `FluentColorPalette` as `ColorView` lists it: shade by shade, 6
/// colours each.
pub const FLUENT_PALETTE: [u32; 48] = [
    0xFF4343, 0xE74856, 0x0078D7, 0x0099BC, 0x7A7574, 0x767676, //
    0xD13438, 0xE81123, 0x0063B1, 0x2D7D9A, 0x5D5A50, 0x4C4A48, //
    0xEF6950, 0xEA005E, 0x8E8CD8, 0x00B7C3, 0x68768A, 0x69797E, //
    0xDA3B01, 0xC30052, 0x6B69D6, 0x038387, 0x515C6B, 0x4A5459, //
    0xCA5010, 0xE3008C, 0x8764B8, 0x00B294, 0x567C73, 0x647C64, //
    0xF7630C, 0xBF0077, 0x744DA9, 0x018574, 0x486860, 0x525E54, //
    0xFF8C00, 0xC239B3, 0xB146C2, 0x00CC6A, 0x498205, 0x847545, //
    0xFFB900, 0x9A0089, 0x881798, 0x10893E, 0x107C10, 0x7E735F,
];

/// `FluentColorPalette.ColorCount`: `PaletteColumnCount`.
const PALETTE_COLUMNS: usize = 6;

/// The palette's colours as `[R, G, B, A]`; with `ColorPickerPanel.OnLoaded`'s
/// three replacements (the last, sixth and thirteenth from the end: #EBEBEB,
/// #141414 and `Colors.Transparent`, #00FFFFFF) when `sharex`.
pub fn palette_colors(sharex: bool) -> [[u8; 4]; 48] {
    let mut out = FLUENT_PALETTE.map(|c| [(c >> 16) as u8, (c >> 8) as u8, c as u8, 255]);
    if sharex {
        let n = out.len();
        out[n - 13] = [235, 235, 235, 255];
        out[n - 7] = [20, 20, 20, 255];
        out[n - 1] = [255, 255, 255, 0];
    }
    out
}

// ---------------------------------------------------------------------------
// ColorSlider
// ---------------------------------------------------------------------------

/// `ColorComponent`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Component {
    /// Red or hue.
    C1,
    /// Green or saturation.
    C2,
    /// Blue or value.
    C3,
    Alpha,
}

/// A `ColorSlider`'s settings.
#[derive(Clone, Copy, Debug)]
struct SliderSpec {
    component: Component,
    /// `ColorModel.Hsva`, else `Rgba`.
    hsv: bool,
    vertical: bool,
    /// `IsAlphaVisible` (false by default).
    alpha_visible: bool,
    /// `IsPerceptive` (true by default).
    perceptive: bool,
    /// `IsRoundingEnabled`.
    rounding: bool,
    /// `IsSnapToTickEnabled` with `TickFrequency="1"`.
    snap: bool,
    enabled: bool,
}

impl SliderSpec {
    /// `SetColorToSliderValues`: (minimum, maximum, value).
    fn values(&self, hsv: Hsv) -> (f64, f64, f64) {
        if self.hsv {
            let h = if self.rounding { round_components(hsv) } else { hsv };
            match self.component {
                Component::Alpha => (0.0, 100.0, h.a * 100.0),
                Component::C1 => (0.0, 359.0, h.h),
                Component::C2 => (0.0, 100.0, h.s * 100.0),
                Component::C3 => (0.0, 100.0, h.v * 100.0),
            }
        } else {
            let c = hsv.to_rgba();
            let v = match self.component {
                Component::Alpha => c[3],
                Component::C1 => c[0],
                Component::C2 => c[1],
                Component::C3 => c[2],
            };
            (0.0, 255.0, v as f64)
        }
    }

    /// `GetColorFromSliderValues` and what `OnPropertyChanged` then sets:
    /// the `HsvColor` for slider value `value`.
    fn color_at(&self, value: f64, hsv: Hsv) -> Hsv {
        let (min, max, _) = self.values(hsv);
        let num = value / (max - min);
        if self.hsv {
            let h = match self.component {
                Component::Alpha => Hsv::new(num, hsv.h, hsv.s, hsv.v),
                Component::C1 => Hsv::new(hsv.a, num * 359.0, hsv.s, hsv.v),
                Component::C2 => Hsv::new(hsv.a, hsv.h, num, hsv.v),
                Component::C3 => Hsv::new(hsv.a, hsv.h, hsv.s, num),
            };
            if self.rounding { round_components(h) } else { h }
        } else {
            let mut c = hsv.to_rgba();
            let b = (num * 255.0).clamp(0.0, 255.0).round_ties_even() as u8;
            match self.component {
                Component::Alpha => c[3] = b,
                Component::C1 => c[0] = b,
                Component::C2 => c[1] = b,
                Component::C3 => c[2] = b,
            }
            Hsv::from_rgba(c)
        }
    }

    /// `GetPerceptiveBackgroundColor`, as `[R, G, B, A]`.
    fn perceptive_background(&self, hsv: Hsv) -> [u8; 4] {
        if self.hsv {
            let mut h = hsv;
            if !self.alpha_visible && self.component != Component::Alpha {
                h = Hsv::new(1.0, h.h, h.s, h.v);
            }
            if self.perceptive {
                h = match self.component {
                    Component::C1 => Hsv::new(h.a, h.h, 1.0, 1.0),
                    Component::C2 => Hsv::new(h.a, h.h, h.s, 1.0),
                    Component::C3 => Hsv::new(h.a, h.h, 1.0, h.v),
                    Component::Alpha => h,
                };
            }
            h.to_rgba()
        } else {
            let mut c = hsv.to_rgba();
            if !self.alpha_visible && self.component != Component::Alpha {
                c[3] = 255;
            }
            if self.perceptive {
                c = match self.component {
                    Component::C1 => [c[0], 0, 0, c[3]],
                    Component::C2 => [0, c[1], 0, c[3]],
                    Component::C3 => [0, 0, c[2], c[3]],
                    Component::Alpha => c,
                };
            }
            c
        }
    }

    /// `UpdatePseudoClasses`: `Some(true)` for `:light-selector`,
    /// `Some(false)` for `:dark-selector`, `None` for neither.
    fn light_selector(&self, hsv: Hsv) -> Option<bool> {
        let color = hsv.to_rgba();
        if color[3] < 128 && (self.alpha_visible || self.component == Component::Alpha) {
            return None;
        }
        Some(relative_luminance(self.perceptive_background(hsv)) <= 0.5)
    }

    /// `ColorPickerHelpers.CreateComponentBitmapAsync`: the track's pixels,
    /// premultiplied, `w` x `h`.
    fn bitmap(&self, w: usize, h: usize, base: Hsv) -> Vec<[u8; 4]> {
        let mut base_hsv = base;
        if !self.alpha_visible && self.component != Component::Alpha {
            base_hsv = Hsv::new(1.0, base_hsv.h, base_hsv.s, base_hsv.v);
        }
        let mut base_rgb = [255u8; 4];
        if !self.hsv {
            base_rgb = base_hsv.to_rgba();
        }
        if self.perceptive && self.component != Component::Alpha {
            if self.hsv {
                base_hsv = match self.component {
                    Component::C1 => Hsv::new(base_hsv.a, base_hsv.h, 1.0, 1.0),
                    Component::C2 => Hsv::new(base_hsv.a, base_hsv.h, base_hsv.s, 1.0),
                    _ => Hsv::new(base_hsv.a, base_hsv.h, 1.0, base_hsv.v),
                };
            } else {
                let [r, g, b, a] = base_rgb;
                base_rgb = match self.component {
                    Component::C1 => [r, 0, 0, a],
                    Component::C2 => [0, g, 0, a],
                    _ => [0, 0, b, a],
                };
            }
        }
        let to_byte = |v: f64| v.clamp(0.0, 255.0).round_ties_even() as u8;
        let color = |value: f64| -> [u8; 4] {
            if self.hsv {
                let b = base_hsv;
                match self.component {
                    Component::C1 => hsv_to_rgba(value.clamp(0.0, 360.0), b.s, b.v, b.a),
                    Component::C2 => hsv_to_rgba(b.h, value.clamp(0.0, 1.0), b.v, b.a),
                    Component::C3 => hsv_to_rgba(b.h, b.s, value.clamp(0.0, 1.0), b.a),
                    Component::Alpha => hsv_to_rgba(b.h, b.s, b.v, value.clamp(0.0, 1.0)),
                }
            } else {
                let [r, g, b, a] = base_rgb;
                match self.component {
                    Component::C1 => [to_byte(value), g, b, a],
                    Component::C2 => [r, to_byte(value), b, a],
                    Component::C3 => [r, g, to_byte(value), a],
                    Component::Alpha => [r, g, b, to_byte(value)],
                }
            }
        };
        let length = if self.vertical { h } else { w } as f64;
        let step = if !self.hsv {
            255.0 / length
        } else if self.component == Component::C1 {
            360.0 / length
        } else {
            1.0 / length
        };
        // Premultiplied as the helper writes it (integer division).
        let premul = |c: [u8; 4]| {
            let a = c[3] as u32;
            [(c[0] as u32 * a / 255) as u8, (c[1] as u32 * a / 255) as u8, (c[2] as u32 * a / 255) as u8, c[3]]
        };
        let mut out = vec![[0u8; 4]; w * h];
        for y in 0..h {
            for x in 0..w {
                let t = if self.vertical { (h - 1 - y) as f64 } else { x as f64 };
                out[y * w + x] = premul(color(t * step));
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------
// The control
// ---------------------------------------------------------------------------

/// `ColorViewTabBackgroundCornerRadius`: 3 on `ColorView`, `5,5,0,0` in
/// `ColorPicker`'s flyout (its `OverlayCornerRadius` top corners).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TabCorners {
    All3,
    Top5,
}

/// A `ColorView` and the properties ShareX sets on it.
#[derive(Clone, Copy, Debug)]
pub struct ColorView {
    /// `IsAlphaEnabled`.
    pub alpha_enabled: bool,
    /// `IsAlphaVisible`.
    pub alpha_visible: bool,
    /// `SelectedIndex`: the tab it opens on the first time.
    pub selected_index: usize,
    /// `IsColorPreviewVisible`.
    pub preview_visible: bool,
    /// `ColorPickerPanel.OnLoaded`'s palette replacements.
    pub sharex_palette: bool,
    pub tab_corners: TabCorners,
    /// An explicit `Width`/`Height` (the Monitor test's 320x320): the
    /// template keeps its 350x338 tab control, centred and clipped.
    pub size: Option<Vec2>,
    /// What's behind the view (its host's background): Avalonia clips a
    /// `BoxShadow` to outside its border, so the previewer's shadow never
    /// shows through a translucent colour. The panel background if `None`.
    pub backdrop: Option<Color32>,
}

impl Default for ColorView {
    fn default() -> Self {
        Self {
            alpha_enabled: true,
            alpha_visible: true,
            selected_index: 0,
            preview_visible: true,
            sharex_palette: false,
            tab_corners: TabCorners::All3,
            size: None,
            backdrop: None,
        }
    }
}

/// `PART_TabControl`'s size.
const TAB_CONTROL: Vec2 = vec2(350.0, 338.0);
/// `ColorViewTabStripHeight`.
const STRIP: f32 = 48.0;
/// `TabControlTopPlacementItemMargin`: the strip's 2px bottom margin.
const STRIP_MARGIN: f32 = 2.0;
/// `ColorPreviewer`'s `Height`.
const PREVIEW_HEIGHT: f32 = 50.0;
/// `ColorSliderSize`.
const SLIDER: f32 = 20.0;

/// Avalonia's layout rounding at a scale: a control's own size (its
/// desired size) rounds up to whole device pixels, a margin or an offset
/// half to even (`LayoutHelper.RoundLayoutSizeUp`, `RoundLayoutValue`).
#[derive(Clone, Copy, Debug)]
pub struct Px(pub f32);

impl Px {
    pub fn of(ctx: &egui::Context) -> Self {
        Px(ctx.pixels_per_point())
    }

    /// A size.
    pub fn up(self, v: f32) -> f32 {
        (v * self.0 - 1e-4).ceil() / self.0
    }

    /// A margin or an offset. Avalonia computes in doubles: a tie that
    /// f32 arithmetic leaves a hair off .5 still rounds to even.
    pub fn rd(self, v: f32) -> f32 {
        let x = ((v * self.0) * 1024.0).round() / 1024.0;
        x.round_ties_even() / self.0
    }

    /// A point on the device pixel grid.
    fn pos(self, p: Pos2) -> Pos2 {
        pos2(self.rd(p.x), self.rd(p.y))
    }
}

/// What the control keeps between frames (`ColorView.Color`/`HsvColor`,
/// the selected tab, the colour model, the hex box).
#[derive(Clone, Debug)]
struct State {
    hsv: Hsv,
    rgba: [u8; 4],
    tab: usize,
    hsv_model: bool,
    hex: String,
}

fn state_id(id: Id) -> Id {
    id.with("color-view-state")
}

/// `Color32` stores premultiplied colours; the view keeps the exact
/// `[R, G, B, A]` and syncs only when the caller's colour really changed.
fn unmultiplied(c: Color32) -> [u8; 4] {
    c.to_srgba_unmultiplied()
}

fn to_color32(c: [u8; 4]) -> Color32 {
    Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3])
}

impl ColorView {
    /// The control's size at 100%: [`Self::size_at`].
    pub fn size(&self) -> Vec2 {
        self.size_at(Px(1.0))
    }

    /// The control's size: its template's 350 by 338 plus the previewer
    /// row (50 and its 12px margin), or the explicit size, in whole pixels.
    pub fn size_at(&self, px: Px) -> Vec2 {
        if let Some(s) = self.size {
            return vec2(px.up(s.x), px.up(s.y));
        }
        let preview = if self.preview_visible { px.up(PREVIEW_HEIGHT) + px.rd(12.0) } else { 0.0 };
        vec2(px.up(TAB_CONTROL.x), px.up(TAB_CONTROL.y) + preview)
    }

    /// `OnCoerceColor` / `OnCoerceHsvColor`.
    fn coerce(&self, h: Hsv) -> Hsv {
        if self.alpha_enabled { h } else { Hsv::new(1.0, h.h, h.s, h.v) }
    }

    fn hex_of(&self, c: [u8; 4]) -> String {
        to_hex_string(c, self.alpha_enabled && self.alpha_visible, false)
    }

    fn load(&self, ctx: &egui::Context, id: Id, color: Color32) -> State {
        let existing: Option<State> = ctx.data(|d| d.get_temp(state_id(id)));
        match existing {
            Some(mut s) => {
                if to_color32(s.rgba) != color {
                    s.rgba = unmultiplied(color);
                    s.hsv = Hsv::from_rgba(s.rgba);
                    s.hex = self.hex_of(s.rgba);
                }
                s
            }
            None => {
                let mut rgba = unmultiplied(color);
                if !self.alpha_enabled {
                    rgba[3] = 255;
                }
                State { hsv: Hsv::from_rgba(rgba), rgba, tab: self.selected_index.min(2), hsv_model: false, hex: self.hex_of(rgba) }
            }
        }
    }

    /// Draws the view with its top-left at `origin`, editing `color`.
    /// Returns whether the colour changed.
    pub fn show(&self, ui: &mut Ui, origin: Pos2, id: Id, color: &mut Color32) -> bool {
        let ctx = ui.ctx().clone();
        let px = Px::of(&ctx);
        let mut st = self.load(&ctx, id, *color);
        let before = to_color32(st.rgba);
        let bounds = Rect::from_min_size(px.pos(origin), self.size_at(px));
        let saved_clip = ui.clip_rect();
        if self.size.is_some() {
            // The overflow is cut by the flyout's scroll viewer.
            ui.set_clip_rect(bounds.intersect(saved_clip));
        }
        let mut edit = Edit::default();
        self.paint_view(ui, px, bounds, id, &mut st, &mut edit);
        if let Some(h) = edit.hsv {
            st.hsv = self.coerce(h);
            st.rgba = st.hsv.to_rgba();
            st.hex = self.hex_of(st.rgba);
        }
        if let Some(c) = edit.rgba {
            let mut c = c;
            if !self.alpha_enabled {
                c[3] = 255;
            }
            st.rgba = c;
            st.hsv = Hsv::from_rgba(c);
            st.hex = self.hex_of(c);
        }
        ui.set_clip_rect(saved_clip);
        let after = to_color32(st.rgba);
        ctx.data_mut(|d| d.insert_temp(state_id(id), st));
        if after != before {
            *color = after;
            return true;
        }
        false
    }

    fn paint_view(&self, ui: &mut Ui, px: Px, bounds: Rect, id: Id, st: &mut State, edit: &mut Edit) {
        let f = fluent(ui.ctx());
        let p = palette(ui.ctx());
        let painter = ui.painter().clone();
        // `TabBackgroundBorder`: the grid's width, 48 high.
        let strip_h = px.up(STRIP);
        let strip_bg = Rect::from_min_size(bounds.min, vec2(bounds.width(), strip_h));
        match self.tab_corners {
            TabCorners::All3 => border::border(&painter, strip_bg, 3.0, 0.0, Some(Brush::Solid(f.base_low)), None),
            TabCorners::Top5 => {
                let clip = painter.with_clip_rect(strip_bg.intersect(painter.clip_rect()));
                let tall = Rect::from_min_max(strip_bg.min, pos2(strip_bg.max.x, strip_bg.max.y + 6.0));
                border::border(&clip, tall, 5.0, 0.0, Some(Brush::Solid(f.base_low)), None);
            }
        }
        // `PART_TabControl`: 350 by 338, centred in the grid's column.
        let tc_size = vec2(px.up(TAB_CONTROL.x), px.up(TAB_CONTROL.y));
        let tc = Rect::from_min_size(pos2(bounds.min.x + px.rd((bounds.width() - tc_size.x) / 2.0), bounds.min.y), tc_size);
        // The tab items, a `UniformGrid` of one row.
        let tab_w = tc.width() / 3.0;
        let pad = px.rd(6.0);
        let icon = px.up(20.0);
        let icons = [(SPECTRUM_ICON, "2.5 2 15 16"), (PALETTE_ICON, "1.89915 1.57663 17.29995 16.78697"), (COMPONENTS_ICON, "2 3 16 14")];
        for (i, (data, view_box)) in icons.iter().enumerate() {
            let tr = Rect::from_min_size(pos2(tc.min.x + px.rd(tab_w * i as f32), tc.min.y), vec2(px.rd(tab_w), strip_h));
            let r = ui.interact(tr, id.with(("tab", i)), Sense::click());
            if r.clicked() {
                st.tab = i;
            }
            let selected = st.tab == i;
            // `TabItemHeaderForeground*`: selected base high, unselected
            // base medium, base medium high under the pointer or pressed.
            let fg = if r.hovered() || r.is_pointer_button_down_on() {
                f.base_medium_high
            } else if selected {
                f.base_high
            } else {
                f.base_medium
            };
            // The 20px `PathIcon` centred in the item's 6,0 padding.
            let inner = Rect::from_min_max(pos2(tr.min.x + pad, tr.min.y), pos2(tr.max.x - pad, tr.max.y));
            let ix = inner.min.x + px.rd((inner.width() - icon) / 2.0);
            let iy = inner.min.y + px.rd((strip_h - icon) / 2.0);
            paint_path_icon(&painter, Rect::from_min_size(pos2(ix, iy), vec2(icon, icon)), data, view_box, fg);
            if selected {
                // `PART_SelectedPipe`: `TabItemPipeThickness` 2, 2 up from
                // the bottom, the accent, `ControlCornerRadius`.
                let bottom = tr.max.y - px.rd(2.0);
                let pipe = Rect::from_min_max(pos2(inner.min.x, bottom - px.up(2.0)), pos2(inner.max.x, bottom));
                border::border(&painter, pipe, 3.0, 0.0, Some(Brush::Solid(p.accent_start)), None);
            }
            if focus::visible(ui.ctx(), r.id) {
                focus::paint_adorner(&painter, tr);
            }
        }
        let content = Rect::from_min_max(pos2(tc.min.x, tc.min.y + strip_h + px.rd(STRIP_MARGIN)), tc.max);
        let m = px.rd(12.0);
        let grid = content.shrink(m);
        match st.tab {
            0 => self.spectrum_tab(ui, px, grid, id, st, edit),
            1 => self.palette_tab(ui, grid, id, st, edit),
            _ => self.components_tab(ui, px, grid, id, st, edit),
        }
        if self.preview_visible {
            let pr = Rect::from_min_size(pos2(bounds.min.x + m, tc.max.y), vec2(tc_size.x - 2.0 * m, px.up(PREVIEW_HEIGHT)));
            self.previewer(ui, px, pr, id, st, edit);
        }
    }

    fn spectrum_tab(&self, ui: &mut Ui, px: Px, grid: Rect, id: Id, st: &State, edit: &mut Edit) {
        // Columns `Auto` (min 32), `*`, `Auto` (min 32): a 20px slider and
        // its 12px margin either side.
        let sw = px.up(SLIDER);
        let col = px.up(32.0).max(sw + px.rd(12.0));
        let third = Rect::from_min_size(grid.min, vec2(sw, grid.height()));
        let alpha = Rect::from_min_size(pos2(grid.max.x - sw, grid.min.y), vec2(sw, grid.height()));
        let spec_area = Rect::from_min_max(pos2(grid.min.x + col, grid.min.y), pos2(grid.max.x - col, grid.max.y));
        self.spectrum(ui, px, spec_area, id.with("spectrum"), st, edit);
        let value = SliderSpec { component: Component::C3, hsv: true, vertical: true, alpha_visible: false, perceptive: true, rounding: false, snap: false, enabled: true };
        color_slider(ui, third, id.with("third"), &value, st.hsv, edit);
        if self.alpha_visible {
            let a = SliderSpec { component: Component::Alpha, hsv: true, vertical: true, alpha_visible: false, perceptive: true, rounding: false, snap: false, enabled: self.alpha_enabled };
            color_slider(ui, alpha, id.with("alpha"), &a, st.hsv, edit);
        }
    }

    /// `ColorSpectrum` (box, `HueSaturation`, hue 0..359, saturation and
    /// value 0..100).
    fn spectrum(&self, ui: &mut Ui, px: Px, area: Rect, id: Id, st: &State, edit: &mut Edit) {
        let ctx = ui.ctx().clone();
        let ppp = px.0;
        // `PART_SizingPanel`: a square as wide as the shorter side, centred.
        let side = px.up(area.width().min(area.height()));
        let sq = Rect::from_min_size(pos2(area.min.x + px.rd((area.width() - side) / 2.0), area.min.y + px.rd((area.height() - side) / 2.0)), vec2(side, side));
        let pd = (area.width().min(area.height()) * ppp).round_ties_even() as usize;
        if pd < 2 {
            return;
        }
        let over = ui.rect_contains_pointer(area);
        let r = ui.interact(sq, id, Sense::click_and_drag());
        if (r.is_pointer_button_down_on() || r.dragged())
            && let Some(at) = r.interact_pointer_pos()
        {
            // `UpdateColorFromPoint`: the pixel under the pointer.
            let x = ((at.x - sq.min.x) * ppp) as f64;
            let y = ((at.y - sq.min.y) * ppp) as f64;
            let col = (x.round_ties_even() as i64).clamp(0, pd as i64 - 1) as f64;
            let row = (y.round_ties_even() as i64).clamp(0, pd as i64 - 1) as f64;
            let n = (pd - 1) as f64;
            let h = col / n * 359.0;
            let s = 1.0 - row / n;
            let mut v = st.hsv.v;
            let mut a = st.hsv.a;
            // `UpdateColor`: a transparent black picks up full value and alpha.
            if a == 0.0 && v == 0.0 {
                a = 1.0;
                v = 1.0;
            }
            edit.hsv = Some(Hsv::new(a, h, s, v));
        }
        let hsv = edit.hsv.unwrap_or(st.hsv);
        let tex = spectrum_texture(&ctx, pd);
        let painter = ui.painter().clone();
        painter.image(tex.id(), sq, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        // The selection ellipse, 16x16 with a 2px stroke, clipped to the box:
        // `Canvas.SetLeft(x - 8)` of the bitmap's pixel, then rounded.
        let n = pd as f32;
        let t_x = (hsv.h.clamp(0.0, 359.0) / 359.0) as f32;
        let t_y = (1.0 - hsv.s.clamp(0.0, 1.0)) as f32;
        let left = px.rd(n * t_x / ppp - 8.0);
        let top = px.rd(n * t_y / ppp - 8.0);
        let ell = Rect::from_min_size(pos2(sq.min.x + left, sq.min.y + top), vec2(px.up(16.0), px.up(16.0)));
        let light = relative_luminance(Hsv::new(hsv.a, hsv.h, hsv.s, 1.0).to_rgba()) <= 0.5;
        let (stroke, opacity) = if light {
            (Color32::WHITE, if over { 0.8 } else { 1.0 })
        } else {
            (Color32::from_rgba_unmultiplied(0, 0, 0, 0xE4), if over { 0.7 } else { 1.0 })
        };
        let clip = painter.with_clip_rect(sq.intersect(painter.clip_rect()));
        border::paint(&clip, &[Part::Stroke { rect: ell.shrink(1.0), radius: 7.0, thickness: 2.0, brush: Brush::Solid(stroke) }], opacity);
        // `BorderRectangle`: 1px of `SystemControlForegroundListLowBrush`.
        let f = fluent(&ctx);
        border::paint(&painter, &[Part::Stroke { rect: sq.shrink(0.5), radius: 0.0, thickness: 1.0, brush: Brush::Solid(f.list_low) }], 1.0);
        if focus::visible(&ctx, id) {
            focus::paint_adorner(&painter, sq);
        }
    }

    fn palette_tab(&self, ui: &mut Ui, area: Rect, id: Id, st: &State, edit: &mut Edit) {
        let colors = palette_colors(self.sharex_palette);
        let rows = colors.len().div_ceil(PALETTE_COLUMNS);
        // `UseLayoutRounding="False"`: cells of 54.33 by 33.
        let (cw, ch) = (area.width() / PALETTE_COLUMNS as f32, area.height() / rows as f32);
        let painter = ui.painter().clone();
        let current = edit.rgba.unwrap_or(st.rgba);
        let selected = colors.iter().position(|c| *c == current);
        for (i, c) in colors.iter().enumerate() {
            let cell = Rect::from_min_size(pos2(area.min.x + cw * (i % PALETTE_COLUMNS) as f32, area.min.y + ch * (i / PALETTE_COLUMNS) as f32), vec2(cw, ch));
            let r = ui.interact(cell, id.with(("palette", i)), Sense::click());
            if c[3] < 255 {
                border::border(&painter, cell, 0.0, 0.0, Some(checkered()), None);
            }
            painter.rect_filled(cell, 0.0, to_color32(*c));
            // `BorderRectangle`: a 3px stroke in the contrasting colour, at
            // half opacity under the pointer.
            let is_selected = selected == Some(i);
            if is_selected || r.hovered() {
                let opacity = if is_selected { 1.0 } else { 0.5 };
                border::paint(&painter, &[Part::Stroke { rect: cell.shrink(1.5), radius: 0.0, thickness: 3.0, brush: Brush::Solid(contrast(*c)) }], opacity);
            }
            if r.clicked() && current != *c {
                edit.rgba = Some(*c);
            }
        }
    }

    fn components_tab(&self, ui: &mut Ui, px: Px, grid: Rect, id: Id, st: &mut State, edit: &mut Edit) {
        let f = fluent(ui.ctx());
        let p = palette(ui.ctx());
        let painter = ui.painter().clone();
        let h32 = px.up(32.0);
        // The hex `TextBox` and the `NumericUpDown`s ask `Height="32"`, but
        // ShareX's `TextBox` and `NumericUpDown` styles set `MinHeight="40"`,
        // which wins: they're 40 tall (`--control-dump colorview`: 127x40,
        // 80x40), and what binds to their height follows (the toggles, the
        // component labels).
        let h_in = px.up(40.0);
        // Row 0: the colour model toggles (`1*`) and the hex input (`1*`), 12 apart.
        let gap = px.rd(12.0);
        let half = px.rd((grid.width() - gap) / 2.0);
        let model = Rect::from_min_size(grid.min, vec2(half, h_in));
        let rgb_w = px.rd(half / 2.0);
        let rgb = Rect::from_min_max(model.min, pos2(model.min.x + rgb_w, model.max.y));
        let hsvr = Rect::from_min_max(pos2(rgb.max.x, model.min.y), model.max);
        for (rect, is_hsv, text, side) in [(rgb, false, "RGB", Side::Left), (hsvr, true, "HSV", Side::Right)] {
            let r = ui.interact(rect, id.with(("model", is_hsv)), Sense::click());
            if r.clicked() {
                st.hsv_model = is_hsv;
            }
            let checked = st.hsv_model == is_hsv;
            let (over, pressed) = (r.hovered() || r.is_pointer_button_down_on(), r.is_pointer_button_down_on());
            // Fluent's `ToggleButton` brushes.
            let (bg, fg) = match (checked, pressed, over) {
                (true, true, _) => (f.base_medium_low, Color32::WHITE),
                // ShareX's accent foreground on the accent (`#FF111111` in the dump).
                (true, false, true) => (p.accent_fill_light[0], p.accent_foreground),
                (true, false, false) => (p.accent_start, p.accent_foreground),
                (false, true, _) => (f.base_medium_low, f.base_high),
                (false, false, true) => (f.base_high.gamma_multiply(0.1), f.base_high),
                (false, false, false) => (f.base_low, f.base_high),
            };
            // `BorderThickness` 1,1,0,1 / 0,1,1,1 of a transparent brush:
            // the background stops at the middle of those sides.
            side_box(&painter, rect, side, 4.0, px.rd(1.0), Some(Brush::Solid(bg)), None, false);
            let cf = content_font(14.0, 400);
            let tw = text_width(ui, text, cf.clone());
            let x = rect.min.x + px.rd((rect.width() - tw) / 2.0);
            let g = galley(ui, text, cf.clone(), fg, f32::INFINITY);
            painter.galley(pos2(x, center_y(rect, line_height(ui, &cf))), g, fg);
            if focus::visible(ui.ctx(), r.id) {
                focus::paint_adorner(&painter, rect);
            }
        }
        // `HexInputGrid`: the "#" label (30) and the text box.
        let hex = Rect::from_min_max(pos2(grid.min.x + half + gap, grid.min.y), pos2(grid.max.x, grid.min.y + h_in));
        // The "#" `Border` keeps its own `Height="32"`, centred in the row.
        let hash = Rect::from_min_size(pos2(hex.min.x, hex.min.y + px.rd((h_in - h32) / 2.0)), vec2(px.up(30.0), h32));
        label_box(ui, px, &painter, hash, "#", true);
        let tb = Rect::from_min_max(pos2(hex.min.x + px.up(30.0), hex.min.y), hex.max);
        let hex_id = id.with("hex");
        let mut text = st.hex.clone();
        let (committed, _) = edit_box(ui, px, tb, hex_id, &mut text, false, true);
        st.hex = text;
        if committed {
            // `GetColorFromHexTextBox`: a valid colour is taken, the box
            // always shows the colour afterwards.
            if let Some(c) = parse_hex_string(&st.hex) {
                edit.rgba = Some(c);
            }
            st.hex = self.hex_of(edit.rgba.unwrap_or(st.rgba));
        }
        // Rows 2..5 (`1*` each) after the 24px gap, 12 under them.
        let fixed = h_in + px.up(24.0);
        let star = (grid.height() - fixed - px.up(12.0)) / 4.0;
        let names: [&str; 4] = if st.hsv_model { ["H", "S", "V", "A"] } else { ["R", "G", "B", "A"] };
        let components = [Component::C1, Component::C2, Component::C3, Component::Alpha];
        let sw = px.up(SLIDER);
        for (i, comp) in components.iter().enumerate() {
            let alpha = *comp == Component::Alpha;
            if alpha && !self.alpha_visible {
                continue;
            }
            let enabled = !alpha || self.alpha_enabled;
            let top = grid.min.y + fixed + px.rd(star * i as f32);
            let row_h = px.rd(star * (i + 1) as f32) - px.rd(star * i as f32);
            let row = Rect::from_min_size(pos2(grid.min.x, top), vec2(grid.width(), row_h));
            let by = row.min.y + px.rd((row_h - h_in) / 2.0);
            let lr = Rect::from_min_size(pos2(row.min.x, by), vec2(px.up(30.0), h_in));
            label_box(ui, px, &painter, lr, names[i], enabled);
            let spec = SliderSpec { component: *comp, hsv: st.hsv_model, vertical: false, alpha_visible: false, perceptive: true, rounding: true, snap: true, enabled };
            let hsv = edit.hsv.unwrap_or(st.hsv);
            let (min, max, value) = spec.values(hsv);
            let nr = Rect::from_min_size(pos2(lr.max.x, by), vec2(px.up(80.0), h_in));
            if let Some(v) = number_box(ui, px, nr, id.with(("number", i)), value, min, max, enabled) {
                edit.hsv = Some(spec.color_at(v, hsv));
            }
            let sy = row.min.y + px.rd((row_h - sw) / 2.0);
            let sr = Rect::from_min_max(pos2(nr.max.x + px.rd(12.0), sy), pos2(row.max.x, sy + sw));
            color_slider(ui, sr, id.with(("component", i)), &spec, hsv, edit);
        }
    }

    /// `ColorPreviewer`: the accents either side (80x40, a click takes
    /// one) and the colour between them with its `0 0 10 2 #BF000000`
    /// shadow.
    fn previewer(&self, ui: &mut Ui, px: Px, r: Rect, id: Id, st: &State, edit: &mut Edit) {
        let painter = ui.painter().clone();
        let hsv = edit.hsv.unwrap_or(st.hsv);
        let color = hsv.to_rgba();
        let (aw, ah) = (px.up(80.0), px.up(40.0));
        let cy = r.min.y + px.rd((r.height() - ah) / 2.0);
        let left = Rect::from_min_size(pos2(r.min.x, cy), vec2(aw, ah));
        let right = Rect::from_min_size(pos2(r.max.x - aw, cy), vec2(aw, ah));
        let half = px.rd(aw / 2.0);
        for (grid, steps) in [(left, [-2, -1]), (right, [1, 2])] {
            border::border(&painter, grid, 0.0, 0.0, Some(checkered()), None);
            for (k, step) in steps.iter().enumerate() {
                let cell = if k == 0 { Rect::from_min_max(grid.min, pos2(grid.min.x + half, grid.max.y)) } else { Rect::from_min_max(pos2(grid.min.x + half, grid.min.y), grid.max) };
                let c = accent(hsv, *step).to_rgba();
                let outer = (*step == -2) || (*step == 2);
                let side = if *step < 0 { Side::Left } else { Side::Right };
                if outer {
                    side_box(&painter, cell, side, 3.0, 0.0, Some(Brush::Solid(to_color32(c))), None, false);
                } else {
                    painter.rect_filled(cell, 0.0, to_color32(c));
                }
                if ui.interact(cell, id.with(("accent", *step)), Sense::click()).is_pointer_button_down_on()
                    && ui.input(|i| i.pointer.primary_pressed())
                {
                    edit.hsv = Some(accent(hsv, *step));
                }
            }
        }
        let main = Rect::from_min_max(pos2(left.max.x, r.min.y), pos2(right.min.x, r.max.y));
        let shadow = egui::epaint::Shadow { offset: [0, 0], blur: 10, spread: 2, color: Color32::from_black_alpha(0xBF) };
        painter.add(shadow.as_shape(main, egui::CornerRadius::same(3)));
        border::paint(
            &painter,
            &[
                Part::Border { rect: main, radius: 3.0, thickness: 0.0, background: Some(Brush::Solid(self.backdrop.unwrap_or(palette(ui.ctx()).background_panel))), border: None },
                Part::Border { rect: main, radius: 3.0, thickness: 0.0, background: Some(checkered()), border: None },
                Part::Border { rect: main, radius: 3.0, thickness: 0.0, background: Some(Brush::Solid(to_color32(color))), border: None },
            ],
            1.0,
        );
    }
}

/// What a frame's input changed.
#[derive(Default)]
struct Edit {
    hsv: Option<Hsv>,
    rgba: Option<[u8; 4]>,
}

/// `ColorControlCheckeredBackgroundBrush`: 4px squares of #19808080 at the
/// top-left and bottom-right of each 8px tile, transparent between.
fn checkered() -> Brush {
    Brush::Checkered { colors: [Color32::TRANSPARENT, Color32::from_rgba_unmultiplied(0x80, 0x80, 0x80, 0x19)], cell: 4.0 }
}

/// The rounded side of a box whose other side is square.
#[derive(Clone, Copy, PartialEq)]
enum Side {
    Left,
    Right,
}

/// A `Border` with `radius` on the corners of `side` only: the shape
/// drawn wider and cut at the square side. `square_line` draws the border
/// along the square side too (a text box's full border).
#[allow(clippy::too_many_arguments)]
fn side_box(painter: &egui::Painter, rect: Rect, side: Side, radius: f32, thickness: f32, bg: Option<Brush>, stroke: Option<Color32>, square_line: bool) {
    let ext = radius + thickness + 2.0;
    let tall = match side {
        Side::Left => Rect::from_min_max(rect.min, pos2(rect.max.x + ext, rect.max.y)),
        Side::Right => Rect::from_min_max(pos2(rect.min.x - ext, rect.min.y), rect.max),
    };
    let clip = painter.with_clip_rect(rect.intersect(painter.clip_rect()));
    border::paint(&clip, &[Part::Border { rect: tall, radius, thickness, background: bg, border: stroke }], 1.0);
    if square_line && let Some(c) = stroke {
        let x = if side == Side::Left { rect.max.x - thickness } else { rect.min.x };
        painter.rect_filled(Rect::from_min_size(pos2(x, rect.min.y), vec2(thickness, rect.height())), 0.0, c);
    }
}

/// A component label: `TextControlBackgroundDisabled` in a
/// `TextControlBorderBrush` border (1,1,0,1, 4px left corners), the letter
/// SemiBold in `TextControlForegroundDisabled`, centred.
fn label_box(ui: &Ui, px: Px, painter: &egui::Painter, rect: Rect, text: &str, enabled: bool) {
    let f = fluent(ui.ctx());
    side_box(painter, rect, Side::Left, 4.0, px.rd(1.0), Some(Brush::Solid(f.base_low)), Some(f.base_medium), false);
    let lf = font(13.0, 600);
    let color = if enabled { f.chrome_disabled_low } else { f.chrome_disabled_low.gamma_multiply(0.5) };
    let tw = text_width(ui, text, lf.clone());
    let x = rect.min.x + px.rd((rect.width() - tw) / 2.0);
    let g = galley(ui, text, lf.clone(), color, f32::INFINITY);
    painter.galley(pos2(x, center_y(rect, line_height(ui, &lf))), g, color);
}

/// A text box with `CornerRadius="0,4,4,0"` (ShareX's `TextBox` look: the
/// control background, hover background, 1px border, 2px when focused).
/// Returns whether the text was committed (Enter, or focus lost) and
/// whether it has focus.
fn edit_box(ui: &mut Ui, px: Px, rect: Rect, id: Id, text: &mut String, centred: bool, enabled: bool) -> (bool, bool) {
    let p = palette(ui.ctx());
    let focused = ui.ctx().memory(|m| m.has_focus(id));
    let hover = enabled && ui.rect_contains_pointer(rect);
    let fill = if !enabled {
        p.control_background_disabled
    } else if hover && !focused {
        p.control_background_hover
    } else {
        p.control_background
    };
    let thickness = px.rd(if focused { 2.0 } else { 1.0 });
    side_box(ui.painter(), rect, Side::Right, 4.0, thickness, Some(Brush::Solid(fill)), Some(p.border), true);
    let tf = crate::controls::TextBox::font();
    let lh = line_height(ui, &tf);
    let pad = Thickness::hv(8.0, 6.0);
    let b = px.rd(1.0);
    let inner = Rect::from_min_max(pos2(rect.min.x + b + px.rd(pad.left), rect.min.y + b), pos2(rect.max.x - b - px.rd(pad.right), rect.max.y - b));
    let er = Rect::from_min_size(pos2(inner.min.x, center_y(rect, lh)), vec2(inner.width().max(0.0), lh));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(er));
    child.set_clip_rect(inner.intersect(ui.clip_rect()));
    let v = child.visuals_mut();
    v.selection.bg_fill = p.accent_start;
    v.selection.stroke.color = p.accent_foreground;
    v.text_cursor.stroke = egui::Stroke::new(1.0, p.text);
    let mut te = egui::TextEdit::singleline(text)
        .id(id)
        .font(tf)
        .text_color(p.text)
        .frame(egui::Frame::NONE)
        .margin(egui::Margin::ZERO)
        .desired_width(er.width())
        .char_limit(9)
        .interactive(enabled);
    if centred {
        te = te.horizontal_align(egui::Align::Center);
    }
    let r = child.add_sized(er.size(), te);
    let outer = ui.interact(rect, id.with("box"), Sense::click());
    if enabled && outer.clicked() {
        ui.ctx().memory_mut(|m| m.request_focus(id));
    }
    let committed = r.lost_focus() || (r.has_focus() && ui.input(|i| i.key_pressed(Key::Enter)));
    (committed, r.has_focus())
}

/// A `NumericUpDown` with `ShowButtonSpinner="False"`, `AllowSpin`,
/// `NumberDecimalDigits=0` and `CornerRadius="0,4,4,0"`: the slider's value
/// as .NET prints the decimal, typed values and the wheel set it.
#[allow(clippy::too_many_arguments)]
fn number_box(ui: &mut Ui, px: Px, rect: Rect, id: Id, value: f64, min: f64, max: f64, enabled: bool) -> Option<f64> {
    let buf_id = id.with("text");
    let focused = ui.ctx().memory(|m| m.has_focus(buf_id));
    let mut text = if focused { ui.ctx().data(|d| d.get_temp::<String>(buf_id)).unwrap_or_else(|| format_decimal(value)) } else { format_decimal(value) };
    let (committed, has_focus) = edit_box(ui, px, rect, buf_id, &mut text, true, enabled);
    let mut out = None;
    if has_focus {
        ui.ctx().data_mut(|d| d.insert_temp(buf_id, text.clone()));
    }
    if committed {
        if let Ok(v) = text.trim().parse::<f64>() {
            let v = v.clamp(min, max);
            if v != value {
                out = Some(v);
            }
        }
        ui.ctx().data_mut(|d| d.remove::<String>(buf_id));
    }
    // `AllowSpin`: the wheel steps by the `Increment` (1).
    if enabled && ui.rect_contains_pointer(rect) {
        let dy: f32 = ui.input(|i| i.events.iter().map(|e| if let egui::Event::MouseWheel { delta, .. } = e { delta.y } else { 0.0 }).sum());
        if dy != 0.0 {
            let v = (value + dy.signum() as f64).clamp(min, max);
            if v != value {
                out = Some(v);
            }
        }
    }
    out
}

/// A slider value as its `decimal` prints: 15 significant digits, no
/// trailing zeros.
fn format_decimal(v: f64) -> String {
    if v == 0.0 {
        return "0".into();
    }
    let digits = 15 - (v.abs().log10().floor() as i32 + 1);
    let digits = digits.clamp(0, 15) as usize;
    let s = format!("{v:.digits$}");
    let s = if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s };
    if s == "-0" { "0".into() } else { s }
}

/// `ColorSlider` in `rect`: the checkered track, its gradient, the 20px
/// ring thumb. Pressing moves the thumb to the pointer and drags it.
fn color_slider(ui: &mut Ui, rect: Rect, id: Id, spec: &SliderSpec, hsv: Hsv, edit: &mut Edit) {
    let ctx = ui.ctx().clone();
    let px = Px::of(&ctx);
    let ppp = px.0;
    // The thumb is the slider's own 20 square.
    let thumb_size = px.up(SLIDER);
    let hsv = edit.hsv.unwrap_or(hsv);
    let (min, max, mut value) = spec.values(hsv);
    let sense = if spec.enabled { Sense::click_and_drag() } else { Sense::hover() };
    let r = ui.interact(rect, id, sense);
    let length = if spec.vertical { rect.height() } else { rect.width() };
    if spec.enabled
        && (r.is_pointer_button_down_on() || r.dragged())
        && let Some(at) = r.interact_pointer_pos()
    {
        // `Slider.MoveToPoint`.
        let pos = if spec.vertical { at.y - rect.min.y } else { at.x - rect.min.x };
        let frac = (((pos - thumb_size / 2.0) / (length - thumb_size)) as f64).clamp(0.0, 1.0);
        let frac = if spec.vertical { 1.0 - frac } else { frac };
        let mut v = frac * (max - min) + min;
        if spec.snap {
            // `SnapToTick` with `TickFrequency="1"`.
            let lo = min + ((v - min) / 1.0).round_ties_even();
            let hi = (lo + 1.0).min(max);
            v = if v >= (lo + hi) * 0.5 { hi } else { lo };
        }
        if v != value {
            let new = spec.color_at(v, hsv);
            edit.hsv = Some(new);
            value = spec.values(new).2;
        }
    }
    let hsv = edit.hsv.unwrap_or(hsv);
    // The track: the checkered `Border` and the gradient, `CornerRadius` 10.
    let painter = ui.painter().clone();
    border::border(&painter, rect, 10.0, 0.0, Some(checkered()), None);
    let (w, h) = ((rect.width() * ppp).round_ties_even() as usize, (rect.height() * ppp).round_ties_even() as usize);
    if w > 0 && h > 0 {
        let tex = slider_texture(&ctx, id, spec, w, h, hsv, 10.0 * ppp);
        painter.image(tex.id(), rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    }
    // `Track.ArrangeOverride`: the thumb's offset along what's left.
    let t = ((value - min) / (max - min)).clamp(0.0, 1.0) as f32;
    let offset = if spec.vertical { (length - thumb_size) * (1.0 - t) } else { (length - thumb_size) * t };
    let offset = px.rd(offset);
    let thumb = if spec.vertical {
        Rect::from_min_size(pos2(rect.min.x, rect.min.y + offset), vec2(thumb_size, thumb_size))
    } else {
        Rect::from_min_size(pos2(rect.min.x + offset, rect.min.y), vec2(thumb_size, thumb_size))
    };
    let f = fluent(&ctx);
    let light = spec.light_selector(hsv);
    let color = match light {
        None => f.base_high,
        Some(true) => Color32::WHITE,
        Some(false) => Color32::from_rgba_unmultiplied(0, 0, 0, 0xE4),
    };
    let over = spec.enabled && (r.hovered() || r.dragged());
    let opacity = match (over, light) {
        (false, _) => 1.0,
        (true, None) => 0.75,
        (true, Some(true)) => 0.8,
        (true, Some(false)) => 0.7,
    };
    border::paint(&painter, &[Part::Border { rect: thumb, radius: 10.0, thickness: px.rd(3.0), background: None, border: Some(color) }], opacity);
    if focus::visible(&ctx, id) {
        focus::paint_adorner(&painter, rect);
    }
}

/// The GPU-style coverage of a rounded rectangle at a pixel (see `border`).
fn rrect_coverage(w: f32, h: f32, radius: f32, x: f32, y: f32) -> f32 {
    let rad = radius.min(w / 2.0).min(h / 2.0);
    let (px, py) = (x + 0.5, y + 0.5);
    let (hw, hh) = (w / 2.0 - rad, h / 2.0 - rad);
    let (qx, qy) = ((px - w / 2.0).abs() - hw, (py - h / 2.0).abs() - hh);
    let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
    let sd = outside + qx.max(qy).min(0.0) - rad;
    (0.5 - sd).clamp(0.0, 1.0)
}

/// The slider's gradient (`ImageBrush` of the helper's bitmap) in its
/// rounded track, cached while the colour and size stay.
fn slider_texture(ctx: &egui::Context, id: Id, spec: &SliderSpec, w: usize, h: usize, hsv: Hsv, radius: f32) -> TextureHandle {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    (spec.component, spec.hsv, spec.vertical, spec.alpha_visible, spec.perceptive, w, h, radius.to_bits()).hash(&mut hasher);
    for v in [hsv.a, hsv.h, hsv.s, hsv.v] {
        v.to_bits().hash(&mut hasher);
    }
    let key = hasher.finish();
    let slot = id.with("texture");
    if let Some((k, tex)) = ctx.data(|d| d.get_temp::<(u64, TextureHandle)>(slot))
        && k == key
    {
        return tex;
    }
    let pixels = spec.bitmap(w, h, hsv);
    let image = ColorImage {
        size: [w, h],
        pixels: pixels
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let cov = rrect_coverage(w as f32, h as f32, radius, (i % w) as f32, (i / w) as f32);
                let m = |v: u8| (v as f32 * cov).round() as u8;
                Color32::from_rgba_premultiplied(m(c[0]), m(c[1]), m(c[2]), m(c[3]))
            })
            .collect(),
        source_size: vec2(w as f32, h as f32),
    };
    let tex = ctx.load_texture(format!("color-slider-{key:x}"), image, TextureOptions::NEAREST);
    ctx.data_mut(|d| d.insert_temp(slot, (key, tex.clone())));
    tex
}

/// `ColorSpectrum`'s value bitmap for `HueSaturation` (`FillPixelForBox`
/// with value 1): hue across, saturation down. The same at every colour.
fn spectrum_texture(ctx: &egui::Context, pd: usize) -> TextureHandle {
    let slot = Id::new(("color-spectrum-texture", pd));
    if let Some(tex) = ctx.data(|d| d.get_temp::<TextureHandle>(slot)) {
        return tex;
    }
    let pixels = spectrum_pixels(pd);
    let image = ColorImage {
        size: [pd, pd],
        pixels: pixels.iter().map(|c| Color32::from_rgb(c[0], c[1], c[2])).collect(),
        source_size: vec2(pd as f32, pd as f32),
    };
    let tex = ctx.load_texture(format!("color-spectrum-{pd}"), image, TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(slot, tex.clone()));
    tex
}

/// The spectrum's pixels, row by row.
pub fn spectrum_pixels(pd: usize) -> Vec<[u8; 4]> {
    let n = (pd - 1) as f64;
    let mut out = Vec::with_capacity(pd * pd);
    for row in 0..pd {
        for col in 0..pd {
            let h = 0.0 + col as f64 / n * 359.0;
            let s0 = row as f64 / n;
            let s = 1.0 - s0;
            // `Hsv.ToRgb()` then the pixel's `(byte)Math.Round(c * 255)`.
            out.push(hsv_to_rgba(h, s, 1.0, 1.0));
        }
    }
    out
}

// ---------------------------------------------------------------------------
// ColorPickerPanel and the popups
// ---------------------------------------------------------------------------

/// `EditorRecentColors`: newest first, shared by every `ColorPickerPanel`.
static RECENT_COLORS: Mutex<Vec<Color32>> = Mutex::new(Vec::new());
/// `EditorRecentColors.MaxCount`.
const MAX_RECENT_COLORS: usize = 32;
/// `ColorPickerPanel.RecentColorCount`.
const RECENT_COLOR_COUNT: usize = 5;

/// `EditorRecentColors.Add`.
pub fn add_recent_color(c: Color32) {
    let mut list = RECENT_COLORS.lock().unwrap();
    list.retain(|x| *x != c);
    list.insert(0, c);
    list.truncate(MAX_RECENT_COLORS);
}

/// `EditorRecentColors.Colors`.
pub fn recent_colors() -> Vec<Color32> {
    RECENT_COLORS.lock().unwrap().clone()
}

/// `ColorPickerPanel`'s view.
const PANEL_VIEW: ColorView = ColorView {
    alpha_enabled: true,
    alpha_visible: true,
    selected_index: 1,
    preview_visible: true,
    sharex_palette: true,
    tab_corners: TabCorners::All3,
    size: None,
    backdrop: None,
};

/// `ColorPickerPanel`'s size: the view (350x400) and the bottom row (the
/// 32px button in a 4px margin).
pub fn panel_size(px: Px) -> Vec2 {
    let view = PANEL_VIEW.size_at(px);
    vec2(view.x, view.y + 2.0 * px.rd(4.0) + px.up(32.0))
}

/// `ColorPickerPanel`: the `ColorView` (alpha, preview, `SelectedIndex="1"`,
/// ShareX's palette), then, centred 12 apart in a 4px margin, the screen
/// colour picker button (40x32) and the five recent colours (28x28, 6
/// apart). Returns whether the colour changed and whether the screen
/// colour picker was asked for.
pub fn color_picker_panel(ui: &mut Ui, origin: Pos2, id: Id, color: &mut Color32) -> (bool, bool) {
    let p = palette(ui.ctx());
    let px = Px::of(ui.ctx());
    let origin = px.pos(origin);
    let view = PANEL_VIEW;
    let mut changed = view.show(ui, origin, id, color);
    let painter = ui.painter().clone();
    let size = view.size_at(px);
    let row_y = origin.y + size.y + px.rd(4.0);
    let (bw, bh, cell, spacing, gap) = (px.up(40.0), px.up(32.0), px.up(28.0), 6.0, 12.0);
    let row_w = px.up(40.0 + gap + RECENT_COLOR_COUNT as f32 * 28.0 + (RECENT_COLOR_COUNT as f32 - 1.0) * spacing);
    let x0 = origin.x + px.rd((size.x - row_w) / 2.0);
    let br = Rect::from_min_size(pos2(x0, row_y), vec2(bw, bh));
    let bresp = ui.interact(br, id.with("screen-picker"), Sense::click());
    let bg = if bresp.hovered() { p.control_background_hover } else { p.control_background };
    border::border(&painter, br, 4.0, px.rd(1.0), Some(Brush::Solid(bg)), Some(p.border));
    let icon = px.up(16.0);
    let ir = Rect::from_min_size(pos2(br.min.x + px.rd((bw - icon) / 2.0), br.min.y + px.rd((bh - icon) / 2.0)), vec2(icon, icon));
    icons::paint(&painter, ir, app_icons::ScreenColorPicker, p.text);
    if focus::visible(ui.ctx(), bresp.id) {
        focus::paint_adorner(&painter, br);
    }
    let recent = recent_colors();
    for i in 0..RECENT_COLOR_COUNT {
        let cr = Rect::from_min_size(pos2(x0 + px.rd(40.0 + gap + i as f32 * (28.0 + spacing)), row_y + px.rd((bh - cell) / 2.0)), vec2(cell, cell));
        match recent.get(i) {
            Some(rc) => {
                let r = ui.interact(cr, id.with(("recent", i)), Sense::click());
                // `checkered-surface` under `recent-color-fill` (1px border,
                // 2px in the secondary text colour under the pointer).
                let (w, k) = if r.hovered() { (px.rd(2.0), p.text_secondary) } else { (px.rd(1.0), p.border) };
                border::paint(
                    &painter,
                    &[
                        Part::Border { rect: cr, radius: 4.0, thickness: 0.0, background: Some(Brush::Checkered { colors: p.checkered, cell: 4.0 }), border: None },
                        Part::Border { rect: cr, radius: 4.0, thickness: w, background: Some(Brush::Solid(*rc)), border: Some(k) },
                    ],
                    1.0,
                );
                if r.clicked() && *color != *rc {
                    *color = *rc;
                    changed = true;
                }
            }
            None => {
                // `recent-color-empty`: its border at 60%.
                border::paint(&painter, &[Part::Border { rect: cr, radius: 4.0, thickness: px.rd(1.0), background: None, border: Some(p.border) }], 0.6);
            }
        }
    }
    (changed, bresp.clicked())
}

/// How a colour popup is hosted.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ColorPopup {
    /// `ColorPickerDropdown`'s `Popup` (`Placement="Bottom"`, 4px down):
    /// the panel background, 1px border, 8px corners, 4px padding, holding
    /// a `ColorPickerPanel`.
    Dropdown,
    /// A `Flyout` (`Placement="Bottom"`) holding a `ColorView` with
    /// `IsAlphaEnabled`/`IsAlphaVisible` = `alpha` and an optional
    /// explicit size: Fluent's `FlyoutPresenter` (chrome medium low,
    /// transient border, 5px corners, 12,11,12,12 padding).
    Flyout { alpha: bool, size: Option<Vec2> },
    /// Avalonia's `ColorPicker` button's flyout (`Placement="Top"`, no
    /// padding, the tab background's top corners at 5).
    Picker { alpha: bool },
}

/// The popup's frame, content offset and view.
struct Host {
    size: Vec2,
    content: Vec2,
    border: f32,
    radius: f32,
    top: bool,
    gap: f32,
}

impl ColorPopup {
    fn view(&self) -> ColorView {
        match *self {
            ColorPopup::Dropdown => ColorView { selected_index: 1, sharex_palette: true, ..Default::default() },
            ColorPopup::Flyout { alpha, size } => ColorView { alpha_enabled: alpha, alpha_visible: alpha, size, ..Default::default() },
            ColorPopup::Picker { alpha } => ColorView { alpha_enabled: alpha, alpha_visible: alpha, tab_corners: TabCorners::Top5, ..Default::default() },
        }
    }

    fn host(&self, px: Px) -> Host {
        let b = px.rd(1.0);
        match self {
            ColorPopup::Dropdown => {
                let pad = px.rd(4.0);
                Host { size: panel_size(px) + Vec2::splat(2.0 * (b + pad)), content: Vec2::splat(b + pad), border: b, radius: 8.0, top: false, gap: px.rd(4.0) }
            }
            ColorPopup::Flyout { .. } => {
                // `FlyoutContentThemePadding` 12,11,12,12.
                let view = self.view().size_at(px);
                let (l, t, r, bt) = (px.rd(12.0), px.rd(11.0), px.rd(12.0), px.rd(12.0));
                Host { size: view + vec2(2.0 * b + l + r, 2.0 * b + t + bt), content: vec2(b + l, b + t), border: b, radius: 5.0, top: false, gap: 0.0 }
            }
            ColorPopup::Picker { .. } => {
                let view = self.view().size_at(px);
                Host { size: view + Vec2::splat(2.0 * b), content: Vec2::splat(b), border: b, radius: 5.0, top: true, gap: 0.0 }
            }
        }
    }

    /// The popup's size at 100%, frame included.
    pub fn size(&self) -> Vec2 {
        self.size_at(Px(1.0))
    }

    /// The popup's size at a scale, in whole device pixels.
    pub fn size_at(&self, px: Px) -> Vec2 {
        self.host(px).size
    }
}

fn pick_key(id: Id) -> Id {
    id.with("screen-pick-requested")
}

/// Whether the `Dropdown` popup `id`'s screen colour picker button was
/// pressed since the last call (the popup closed for it, as
/// `OnScreenColorPickerClick` closes its host popup).
pub fn take_screen_pick(ctx: &egui::Context, id: Id) -> bool {
    ctx.data_mut(|d| d.remove_temp::<bool>(pick_key(id))).unwrap_or(false)
}

/// Where Avalonia's popup positioner puts a popup of `size` against
/// `anchor`: centred on it, under it (`top`: over it) `gap` away, flipped
/// when it doesn't fit and slid along to stay on `screen`.
pub fn place(anchor: Rect, size: Vec2, top: bool, gap: f32, screen: Rect) -> Pos2 {
    let x = (anchor.center().x - size.x / 2.0).round();
    let below = anchor.max.y + gap;
    let above = anchor.min.y - gap - size.y;
    let mut y = if top { above } else { below };
    if top && y < screen.min.y && below + size.y <= screen.max.y {
        y = below;
    } else if !top && y + size.y > screen.max.y && above >= screen.min.y {
        y = above;
    }
    let x = x.clamp(screen.min.x, (screen.max.x - size.x).max(screen.min.x));
    let y = y.clamp(screen.min.y, (screen.max.y - size.y).max(screen.min.y));
    pos2(x, y)
}

/// Draws the popup `id` with its top-left at `pos`: whether the colour
/// changed, whether the screen colour picker was asked for, and the
/// popup's rectangle.
pub fn popup_at(ctx: &egui::Context, id: Id, pos: Pos2, color: &mut Color32, style: ColorPopup) -> (bool, bool, Rect) {
    let p = palette(ctx);
    let f = fluent(ctx);
    let host = style.host(Px::of(ctx));
    let view_id = id.with("view");
    let mut changed = false;
    let mut pick = false;
    let area = egui::Area::new(id.with("color-popup")).order(Order::Foreground).fixed_pos(pos).fade_in(false).show(ctx, |ui| {
        let (r, _) = ui.allocate_exact_size(host.size, Sense::hover());
        match style {
            ColorPopup::Dropdown => border::border(ui.painter(), r, host.radius, host.border, Some(Brush::Solid(p.background_panel)), Some(p.border)),
            _ => border::border(ui.painter(), r, host.radius, host.border, Some(Brush::Solid(f.chrome_medium_low)), Some(f.transient_border)),
        }
        let origin = r.min + host.content;
        match style {
            ColorPopup::Dropdown => {
                let (c, k) = color_picker_panel(ui, origin, view_id, color);
                changed = c;
                pick = k;
            }
            _ => {
                let view = ColorView { backdrop: Some(f.chrome_medium_low), ..style.view() };
                changed = view.show(ui, origin, view_id, color);
            }
        }
    });
    crate::popup::register(ctx, area.response.layer_id, area.response.rect, host.radius);
    (changed, pick, area.response.rect)
}

/// Sets popup `id`'s view to `tab` and the colour model (HSV when
/// `hsv_model`), as a click on them would: for renders.
/// `rgba` is the exact colour (`Color32` premultiplies it); the popup's
/// colour should then be `Color32::from_rgba_unmultiplied` of it.
pub fn preset(ctx: &egui::Context, id: Id, style: ColorPopup, rgba: [u8; 4], tab: usize, hsv_model: bool) {
    let st = State { hsv: Hsv::from_rgba(rgba), rgba, tab: tab.min(2), hsv_model, hex: style.view().hex_of(rgba) };
    ctx.data_mut(|d| d.insert_temp(state_id(id.with("view")), st));
}

/// The colour popup `id` against `anchor`, hosted as `style`. Returns
/// whether the colour changed and whether the popup stays open (Escape or
/// a press outside it and `anchor` closes it, but not the press that
/// opened it, `just_opened`).
pub fn popup(ui: &Ui, id: Id, anchor: Rect, color: &mut Color32, just_opened: bool, style: ColorPopup) -> (bool, bool) {
    let ctx = ui.ctx().clone();
    let host = style.host(Px::of(&ctx));
    let view_id = id.with("view");
    if just_opened && style == ColorPopup::Dropdown {
        // `OnHostPopupOpened`.
        let opened = unmultiplied(*color);
        ctx.data_mut(|d| d.insert_temp(state_id(view_id).with("opened-with"), opened));
    }
    let pos = place(anchor, host.size, host.top, host.gap, crate::popup::bounds(&ctx, ctx.content_rect()));
    let (changed, pick, area) = popup_at(&ctx, id, pos, color, style);
    let mut keep = !ctx.input(|i| i.key_pressed(Key::Escape));
    if !keep {
        crate::popup::closed_by_escape(&ctx);
    }
    if !just_opened
        && ctx.input(|i| i.pointer.any_pressed())
        && let Some(at) = ctx.input(|i| i.pointer.interact_pos())
        && !area.contains(at)
        && !anchor.contains(at)
    {
        keep = false;
    }
    if pick {
        keep = false;
        ctx.data_mut(|d| d.insert_temp(pick_key(id), true));
    }
    if !keep && style == ColorPopup::Dropdown {
        // `OnHostPopupClosed`: the colour it closes with is used.
        let opened: Option<[u8; 4]> = ctx.data(|d| d.get_temp(state_id(view_id).with("opened-with")));
        if opened.is_some_and(|o| to_color32(o) != *color) {
            add_recent_color(*color);
        }
    }
    (changed, keep)
}

// ---------------------------------------------------------------------------
// Icons
// ---------------------------------------------------------------------------

/// `ColorViewSpectrumIconGeometry` (Fluent UI System Icons,
/// ic_fluent_inking_tool_20_regular).
const SPECTRUM_ICON: &str = "M3 2C3.27614 2 3.5 2.22386 3.5 2.5V5.5C3.5 5.77614 3.72386 6 4 6H16C16.2761 6 16.5 5.77614 16.5 5.5V2.5C16.5 2.22386 16.7239 2 17 2C17.2761 2 17.5 2.22386 17.5 2.5V5.5C17.5 6.32843 16.8284 7 16 7H15.809L12.2236 14.1708C12.0615 14.4951 11.7914 14.7431 11.4695 14.8802C11.4905 15.0808 11.5 15.2891 11.5 15.5C11.5 16.0818 11.4278 16.6623 11.2268 17.1165C11.019 17.5862 10.6266 18 10 18C9.37343 18 8.98105 17.5862 8.77323 17.1165C8.57222 16.6623 8.5 16.0818 8.5 15.5C8.5 15.2891 8.50952 15.0808 8.53051 14.8802C8.20863 14.7431 7.93851 14.4951 7.77639 14.1708L4.19098 7H4C3.17157 7 2.5 6.32843 2.5 5.5V2.5C2.5 2.22386 2.72386 2 3 2ZM9.11803 14H10.882C11.0714 14 11.2445 13.893 11.3292 13.7236L14.691 7H5.30902L8.67082 13.7236C8.75552 13.893 8.92865 14 9.11803 14ZM9.52346 15C9.50787 15.1549 9.5 15.3225 9.5 15.5C9.5 16.0228 9.56841 16.4423 9.6877 16.7119C9.8002 16.9661 9.90782 17 10 17C10.0922 17 10.1998 16.9661 10.3123 16.7119C10.4316 16.4423 10.5 16.0228 10.5 15.5C10.5 15.3225 10.4921 15.1549 10.4765 15H9.52346Z";
/// `ColorViewPaletteIconGeometry` (ic_fluent_color_20_regular).
const PALETTE_ICON: &str = "M9.75003 6.5C10.1642 6.5 10.5 6.16421 10.5 5.75C10.5 5.33579 10.1642 5 9.75003 5C9.33582 5 9.00003 5.33579 9.00003 5.75C9.00003 6.16421 9.33582 6.5 9.75003 6.5ZM12.75 7.5C13.1642 7.5 13.5 7.16421 13.5 6.75C13.5 6.33579 13.1642 6 12.75 6C12.3358 6 12 6.33579 12 6.75C12 7.16421 12.3358 7.5 12.75 7.5ZM15.25 9C15.25 9.41421 14.9142 9.75 14.5 9.75C14.0858 9.75 13.75 9.41421 13.75 9C13.75 8.58579 14.0858 8.25 14.5 8.25C14.9142 8.25 15.25 8.58579 15.25 9ZM14.5 12.75C14.9142 12.75 15.25 12.4142 15.25 12C15.25 11.5858 14.9142 11.25 14.5 11.25C14.0858 11.25 13.75 11.5858 13.75 12C13.75 12.4142 14.0858 12.75 14.5 12.75ZM13.25 14C13.25 14.4142 12.9142 14.75 12.5 14.75C12.0858 14.75 11.75 14.4142 11.75 14C11.75 13.5858 12.0858 13.25 12.5 13.25C12.9142 13.25 13.25 13.5858 13.25 14ZM13.6972 2.99169C10.9426 1.57663 8.1432 1.7124 5.77007 3.16636C4.55909 3.9083 3.25331 5.46925 2.51605 7.05899C2.14542 7.85816 1.89915 8.70492 1.90238 9.49318C1.90566 10.2941 2.16983 11.0587 2.84039 11.6053C3.45058 12.1026 3.98165 12.353 4.49574 12.3784C5.01375 12.404 5.41804 12.1942 5.73429 12.0076C5.80382 11.9666 5.86891 11.927 5.93113 11.8892C6.17332 11.7421 6.37205 11.6214 6.62049 11.5426C6.90191 11.4534 7.2582 11.4205 7.77579 11.5787C7.96661 11.637 8.08161 11.7235 8.16212 11.8229C8.24792 11.9289 8.31662 12.0774 8.36788 12.2886C8.41955 12.5016 8.44767 12.7527 8.46868 13.0491C8.47651 13.1594 8.48379 13.2855 8.49142 13.4176C8.50252 13.6098 8.51437 13.8149 8.52974 14.0037C8.58435 14.6744 8.69971 15.4401 9.10362 16.1357C9.51764 16.8488 10.2047 17.439 11.307 17.8158C12.9093 18.3636 14.3731 17.9191 15.5126 17.0169C16.6391 16.125 17.4691 14.7761 17.8842 13.4272C19.1991 9.15377 17.6728 5.03394 13.6972 2.99169ZM6.29249 4.01905C8.35686 2.75426 10.7844 2.61959 13.2403 3.88119C16.7473 5.68275 18.1135 9.28161 16.9284 13.1332C16.5624 14.3227 15.8338 15.4871 14.8919 16.2329C13.963 16.9684 12.8486 17.286 11.6305 16.8696C10.7269 16.5607 10.2467 16.1129 9.96842 15.6336C9.68001 15.1369 9.57799 14.5556 9.52644 13.9225C9.51101 13.733 9.50132 13.5621 9.49147 13.3884C9.48399 13.2564 9.47642 13.1229 9.46618 12.9783C9.44424 12.669 9.41175 12.3499 9.33968 12.0529C9.26719 11.7541 9.14902 11.4527 8.93935 11.1937C8.72439 10.9282 8.43532 10.7346 8.06801 10.6223C7.36648 10.408 6.80266 10.4359 6.31839 10.5893C5.94331 10.7082 5.62016 10.9061 5.37179 11.0582C5.31992 11.0899 5.2713 11.1197 5.22616 11.1463C4.94094 11.3146 4.75357 11.39 4.54514 11.3796C4.33279 11.3691 4.00262 11.2625 3.47218 10.8301C3.0866 10.5158 2.90473 10.0668 2.90237 9.48908C2.89995 8.89865 3.08843 8.20165 3.42324 7.47971C4.09686 6.0272 5.28471 4.63649 6.29249 4.01905Z";
/// `ColorViewComponentsIconGeometry` (ic_fluent_options_20_regular).
const COMPONENTS_ICON: &str = "M14.95 5C14.7184 3.85888 13.7095 3 12.5 3C11.2905 3 10.2816 3.85888 10.05 5H2.5C2.22386 5 2 5.22386 2 5.5C2 5.77614 2.22386 6 2.5 6H10.05C10.2816 7.14112 11.2905 8 12.5 8C13.7297 8 14.752 7.11217 14.961 5.94254C14.9575 5.96177 14.9539 5.98093 14.95 6H17.5C17.7761 6 18 5.77614 18 5.5C18 5.22386 17.7761 5 17.5 5H14.95ZM12.5 7C11.6716 7 11 6.32843 11 5.5C11 4.67157 11.6716 4 12.5 4C13.3284 4 14 4.67157 14 5.5C14 6.32843 13.3284 7 12.5 7ZM9.94999 14C9.71836 12.8589 8.70948 12 7.5 12C6.29052 12 5.28164 12.8589 5.05001 14H2.5C2.22386 14 2 14.2239 2 14.5C2 14.7761 2.22386 15 2.5 15H5.05001C5.28164 16.1411 6.29052 17 7.5 17C8.70948 17 9.71836 16.1411 9.94999 15H17.5C17.7761 15 18 14.7761 18 14.5C18 14.2239 17.7761 14 17.5 14H9.94999ZM7.5 16C6.67157 16 6 15.3284 6 14.5C6 13.6716 6.67157 13 7.5 13C8.32843 13 9 13.6716 9 14.5C9 15.3284 8.32843 16 7.5 16Z";

/// A `PathIcon` (`Stretch="Uniform"`): the geometry's bounds fitted into
/// `r`.
fn paint_path_icon(painter: &egui::Painter, r: Rect, data: &str, view_box: &str, color: Color32) {
    let svg = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{view_box}"><path d="{data}" fill="currentColor"/></svg>"#);
    let key = format!("colorview-tab-{}", data.len());
    icons::paint_svg(painter, r, &key, &svg, color);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hsv_round_trips_every_grey_and_primary() {
        for v in 0..=255u8 {
            for c in [[v, v, v, 255], [v, 0, 0, 255], [0, v, 0, 255], [0, 0, v, 255], [v, 255 - v, 128, 200]] {
                assert_eq!(Hsv::from_rgba(c).to_rgba(), c, "{c:?}");
            }
        }
    }

    #[test]
    fn hsv_round_trips_a_spread_of_colours() {
        let mut n = 0u32;
        for r in (0..=255u16).step_by(15) {
            for g in (0..=255u16).step_by(17) {
                for b in (0..=255u16).step_by(13) {
                    let c = [r as u8, g as u8, b as u8, 255];
                    assert_eq!(Hsv::from_rgba(c).to_rgba(), c);
                    n += 1;
                }
            }
        }
        assert!(n > 4000);
    }

    #[test]
    fn hsv_matches_avalonias_values() {
        // `Color.ToHsv()` of #FF4343: hue 0, saturation 0.737, value 1.
        let h = Hsv::from_rgba([255, 67, 67, 255]);
        assert_eq!(h.h, 0.0);
        assert!((h.s - 188.0 / 255.0).abs() < 1e-12);
        assert_eq!(h.v, 1.0);
        // Hue sectors and .NET's half-to-even rounding: 0.5 * 255 = 127.5 -> 128.
        assert_eq!(hsv_to_rgba(120.0, 1.0, 0.5, 1.0), [0, 128, 0, 255]);
        assert_eq!(hsv_to_rgba(30.0, 1.0, 1.0, 0.5), [255, 128, 0, 128]);
        assert_eq!(hsv_to_rgba(-60.0, 1.0, 1.0, 1.0), [255, 0, 255, 255]);
        assert_eq!(hsv_to_rgba(720.0, 1.0, 1.0, 1.0), [255, 0, 0, 255]);
        // `HsvColor`'s constructor clamps and makes 360 hue 0.
        assert_eq!(Hsv::new(2.0, 360.0, -1.0, 0.5), Hsv { a: 1.0, h: 0.0, s: 0.0, v: 0.5 });
    }

    #[test]
    fn alpha_survives_the_round_trip() {
        for a in 0..=255u8 {
            let c = [10, 200, 90, a];
            let h = Hsv::from_rgba(c);
            assert!((h.a - a as f64 / 255.0).abs() < 1e-12);
            assert_eq!(h.to_rgba(), c);
        }
    }

    #[test]
    fn hex_formats_as_the_converter() {
        assert_eq!(to_hex_string([0xFF, 0x43, 0x43, 0xFF], true, false), "FF4343FF");
        assert_eq!(to_hex_string([0xFF, 0x43, 0x43, 0x80], false, false), "FF4343");
        assert_eq!(to_hex_string([1, 2, 3, 4], true, true), "#01020304");
    }

    #[test]
    fn hex_parses_as_the_converter() {
        assert_eq!(parse_hex_string("FF4343"), Some([0xFF, 0x43, 0x43, 0xFF]));
        assert_eq!(parse_hex_string("#ff434380"), Some([0xFF, 0x43, 0x43, 0x80]));
        assert_eq!(parse_hex_string("  #abc "), Some([0xAA, 0xBB, 0xCC, 0xFF]));
        assert_eq!(parse_hex_string("abcd"), Some([0xAA, 0xBB, 0xCC, 0xDD]));
        assert_eq!(parse_hex_string("#12345"), None);
        assert_eq!(parse_hex_string("GG0000"), None);
        assert_eq!(parse_hex_string(""), None);
        assert_eq!(parse_hex_string("#"), None);
        assert_eq!(parse_hex_string("123456789"), None);
    }

    #[test]
    fn accents_step_the_value() {
        let base = Hsv::from_rgba([255, 67, 67, 255]);
        // V 1.0 - 0.2 = 0.8: #CC3636 (0.8 * 0.263 = 0.21 -> 53.6 -> 54).
        assert_eq!(accent(base, -2).to_rgba(), [204, 54, 54, 255]);
        assert_eq!(accent(base, 2).to_rgba(), [255, 67, 67, 255]);
        assert_eq!(accent(base, 0), base);
        let red = Hsv::from_rgba([242, 60, 60, 255]);
        // ShareX's shot: the accents of #F23C3C are #BF2F2F, #D93636, #FF3F3F.
        assert_eq!(accent(red, -2).to_rgba(), [191, 47, 47, 255]);
        assert_eq!(accent(red, -1).to_rgba(), [217, 54, 54, 255]);
        assert_eq!(accent(red, 1).to_rgba(), [255, 63, 63, 255]);
    }

    #[test]
    fn contrast_and_luminance() {
        assert_eq!(contrast([255, 255, 255, 255]), Color32::BLACK);
        assert_eq!(contrast([0, 0, 0, 255]), Color32::WHITE);
        assert_eq!(contrast([0, 120, 215, 255]), Color32::WHITE);
        assert!(relative_luminance([255, 255, 255, 255]) > 0.99);
    }

    #[test]
    fn sliders_map_values_as_avalonia() {
        let hsv = Hsv::from_rgba([255, 67, 67, 255]);
        let h = SliderSpec { component: Component::C1, hsv: true, vertical: false, alpha_visible: false, perceptive: true, rounding: true, snap: true, enabled: true };
        assert_eq!(h.values(hsv), (0.0, 359.0, 0.0));
        // Hue 180 of 359: 180 / 359 * 359.
        let c = h.color_at(180.0, hsv);
        assert_eq!(c.h, 180.0);
        let s = SliderSpec { component: Component::C2, ..h };
        assert_eq!(s.values(hsv).2, 74.0);
        let r = SliderSpec { hsv: false, ..h };
        assert_eq!(r.values(hsv), (0.0, 255.0, 255.0));
        assert_eq!(r.color_at(100.0, hsv).to_rgba(), [100, 67, 67, 255]);
        let a = SliderSpec { component: Component::Alpha, hsv: false, ..h };
        assert_eq!(a.color_at(128.0, hsv).to_rgba(), [255, 67, 67, 128]);
        let ah = SliderSpec { component: Component::Alpha, ..h };
        assert_eq!(ah.color_at(50.0, hsv).to_rgba()[3], 128);
    }

    #[test]
    fn slider_bitmaps_as_the_helper() {
        // The value slider of the spectrum tab: black at the bottom, the
        // hue at full saturation at the top (row 0 is value 19/20).
        let spec = SliderSpec { component: Component::C3, hsv: true, vertical: true, alpha_visible: false, perceptive: true, rounding: false, snap: false, enabled: true };
        let px = spec.bitmap(2, 20, Hsv::new(1.0, 240.0, 0.3, 0.4));
        assert_eq!(px[19 * 2], [0, 0, 0, 255]);
        assert_eq!(px[0], [0, 0, 242, 255]);
        assert_eq!(px[1], px[0]);
        // An alpha slider is premultiplied, transparent at the bottom.
        let a = SliderSpec { component: Component::Alpha, ..spec };
        let px = a.bitmap(1, 10, Hsv::new(1.0, 0.0, 1.0, 1.0));
        assert_eq!(px[9], [0, 0, 0, 0]);
        assert_eq!(px[0], [230, 0, 0, 230]); // 0.9 * 255 = 229.5, to even
        // RGB red: 0..255 across.
        let r = SliderSpec { component: Component::C1, hsv: false, vertical: false, ..spec };
        let px = r.bitmap(4, 1, Hsv::from_rgba([10, 20, 30, 255]));
        assert_eq!(px.iter().map(|c| c[0]).collect::<Vec<_>>(), vec![0, 64, 128, 191]);
        assert!(px.iter().all(|c| c[1] == 0 && c[2] == 0));
    }

    #[test]
    fn spectrum_pixels_hue_across_saturation_down() {
        let px = spectrum_pixels(3);
        assert_eq!(px[0], [255, 0, 0, 255]);
        assert_eq!(px[2], hsv_to_rgba(359.0, 1.0, 1.0, 1.0));
        assert_eq!(px[6], [255, 255, 255, 255]);
        assert_eq!(px[1], hsv_to_rgba(179.5, 1.0, 1.0, 1.0));
    }

    #[test]
    fn sharex_palette_replacements() {
        let p = palette_colors(true);
        assert_eq!(p[35], [235, 235, 235, 255]);
        assert_eq!(p[41], [20, 20, 20, 255]);
        assert_eq!(p[47], [255, 255, 255, 0]);
        assert_eq!(palette_colors(false)[47], [126, 115, 95, 255]);
    }

    #[test]
    fn decimals_print_as_dotnet() {
        assert_eq!(format_decimal(0.57 * 100.0), "57");
        assert_eq!(format_decimal(255.0), "255");
        assert_eq!(format_decimal(0.0), "0");
        assert_eq!(format_decimal(12.5), "12.5");
    }

    #[test]
    fn hosts_are_sharex_sizes() {
        // 100%: the dropdown 360x450, a flyout 376x425, the picker 352x402.
        assert_eq!(ColorPopup::Dropdown.size(), vec2(360.0, 450.0));
        assert_eq!(ColorPopup::Flyout { alpha: true, size: None }.size(), vec2(376.0, 425.0));
        assert_eq!(ColorPopup::Picker { alpha: false }.size(), vec2(352.0, 402.0));
        // 125%: the highlighter's flyout is ShareX's 470x532 pixel window.
        let px = Px(1.25);
        let s = ColorPopup::Flyout { alpha: true, size: None }.size_at(px) * 1.25;
        assert_eq!((s.x.round(), s.y.round()), (470.0, 532.0));
        // Sizes round up, offsets half to even (52.5 -> 52).
        assert_eq!(px.up(20.0) * 1.25, 25.0);
        assert_eq!((px.up(2.0) * 1.25).round(), 3.0);
        assert_eq!(px.rd(42.0) * 1.25, 52.0);
        assert_eq!(px.rd(2.0) * 1.25, 2.0);
    }

    #[test]
    fn popups_flip_and_slide() {
        let screen = Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0));
        let anchor = Rect::from_min_size(pos2(100.0, 500.0), vec2(60.0, 32.0));
        // No room under it: above.
        let p = place(anchor, vec2(376.0, 425.0), false, 0.0, screen);
        assert_eq!(p, pos2(0.0, 75.0));
        let anchor = Rect::from_min_size(pos2(300.0, 20.0), vec2(64.0, 32.0));
        // `Placement="Top"` with no room over it: under it.
        assert_eq!(place(anchor, vec2(352.0, 402.0), true, 0.0, screen), pos2(156.0, 52.0));
    }
}
