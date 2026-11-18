//! The mouse highlighter (`ShareX.Tools/Tools/MouseHighlighter`): its
//! options (`MouseHighlighterOptions`), the clicks it shows and how they
//! age (`MouseHighlighterService`), and drawing a monitor's effects
//! (`MouseHighlighterOverlayWindow.Draw`) with Skia's CPU rasteriser, as
//! ShareX draws them into its layered window.

use serde_json::{Value, json};
use sharex_skia::{Color as SkColor, Matrix, Path, Pixmap, Stroke};

use crate::color::Color;
use crate::pin::{color, set};

/// `MouseHighlightMode`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Circle,
    Ripple,
}

/// `MouseHighlightButton` (also the bit in the pressed-buttons mask).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Primary = 0,
    Secondary = 1,
    Middle = 2,
}

/// `MouseHighlighterOptions`.
#[derive(Clone, Debug, PartialEq)]
pub struct MouseHighlighterOptions {
    pub auto_activate: bool,
    pub mode: Mode,
    pub highlight_primary_clicks: bool,
    pub primary_color: Color,
    pub show_primary_release_crosshairs: bool,
    pub highlight_secondary_clicks: bool,
    pub secondary_color: Color,
    pub show_secondary_release_crosshairs: bool,
    pub highlight_middle_clicks: bool,
    pub middle_color: Color,
    pub show_middle_release_crosshairs: bool,
    pub always_color: Color,
    pub radius: i32,
    pub fade_delay: i32,
    pub fade_duration: i32,
    pub ripple_size: i32,
    pub ripple_intensity: f64,
    pub ripple_duration: i32,
    pub follow_cursor_while_held: bool,
}

impl Default for MouseHighlighterOptions {
    fn default() -> Self {
        Self {
            auto_activate: false,
            mode: Mode::Ripple,
            highlight_primary_clicks: true,
            primary_color: Color::argb(166, 54, 169, 225),
            show_primary_release_crosshairs: false,
            highlight_secondary_clicks: true,
            secondary_color: Color::argb(166, 89, 184, 106),
            show_secondary_release_crosshairs: false,
            highlight_middle_clicks: true,
            middle_color: Color::argb(166, 242, 193, 78),
            show_middle_release_crosshairs: false,
            always_color: Color::transparent(),
            radius: 20,
            fade_delay: 500,
            fade_duration: 250,
            ripple_size: 60,
            ripple_intensity: 0.7,
            ripple_duration: 500,
            follow_cursor_while_held: true,
        }
    }
}

impl MouseHighlighterOptions {
    pub fn from_json(v: &Value) -> Self {
        let d = Self::default();
        let b = |k: &str, d: bool| v.get(k).and_then(Value::as_bool).unwrap_or(d);
        let i = |k: &str, d: i32| v.get(k).and_then(Value::as_i64).map_or(d, |n| n as i32);
        let mode = match v.get("Mode") {
            Some(Value::String(s)) if s.eq_ignore_ascii_case("Circle") => Mode::Circle,
            Some(Value::String(s)) if s.eq_ignore_ascii_case("Ripple") => Mode::Ripple,
            Some(Value::Number(n)) if n.as_i64() == Some(0) => Mode::Circle,
            _ => d.mode,
        };
        Self {
            auto_activate: b("AutoActivate", d.auto_activate),
            mode,
            highlight_primary_clicks: b("HighlightPrimaryClicks", d.highlight_primary_clicks),
            primary_color: color(v, "PrimaryColor", d.primary_color),
            show_primary_release_crosshairs: b("ShowPrimaryReleaseCrosshairs", d.show_primary_release_crosshairs),
            highlight_secondary_clicks: b("HighlightSecondaryClicks", d.highlight_secondary_clicks),
            secondary_color: color(v, "SecondaryColor", d.secondary_color),
            show_secondary_release_crosshairs: b("ShowSecondaryReleaseCrosshairs", d.show_secondary_release_crosshairs),
            highlight_middle_clicks: b("HighlightMiddleClicks", d.highlight_middle_clicks),
            middle_color: color(v, "MiddleColor", d.middle_color),
            show_middle_release_crosshairs: b("ShowMiddleReleaseCrosshairs", d.show_middle_release_crosshairs),
            always_color: color(v, "AlwaysColor", d.always_color),
            radius: i("Radius", d.radius),
            fade_delay: i("FadeDelay", d.fade_delay),
            fade_duration: i("FadeDuration", d.fade_duration),
            ripple_size: i("RippleSize", d.ripple_size),
            ripple_intensity: v.get("RippleIntensity").and_then(Value::as_f64).unwrap_or(d.ripple_intensity),
            ripple_duration: i("RippleDuration", d.ripple_duration),
            follow_cursor_while_held: b("FollowCursorWhileHeld", d.follow_cursor_while_held),
        }
    }

    pub fn write_json(&self, target: &mut Value) {
        set(target, "AutoActivate", json!(self.auto_activate));
        set(target, "Mode", json!(if self.mode == Mode::Circle { "Circle" } else { "Ripple" }));
        set(target, "HighlightPrimaryClicks", json!(self.highlight_primary_clicks));
        set(target, "PrimaryColor", json!(self.primary_color.to_string()));
        set(target, "ShowPrimaryReleaseCrosshairs", json!(self.show_primary_release_crosshairs));
        set(target, "HighlightSecondaryClicks", json!(self.highlight_secondary_clicks));
        set(target, "SecondaryColor", json!(self.secondary_color.to_string()));
        set(target, "ShowSecondaryReleaseCrosshairs", json!(self.show_secondary_release_crosshairs));
        set(target, "HighlightMiddleClicks", json!(self.highlight_middle_clicks));
        set(target, "MiddleColor", json!(self.middle_color.to_string()));
        set(target, "ShowMiddleReleaseCrosshairs", json!(self.show_middle_release_crosshairs));
        set(target, "AlwaysColor", json!(self.always_color.to_string()));
        set(target, "Radius", json!(self.radius));
        set(target, "FadeDelay", json!(self.fade_delay));
        set(target, "FadeDuration", json!(self.fade_duration));
        set(target, "RippleSize", json!(self.ripple_size));
        set(target, "RippleIntensity", json!(self.ripple_intensity));
        set(target, "RippleDuration", json!(self.ripple_duration));
        set(target, "FollowCursorWhileHeld", json!(self.follow_cursor_while_held));
    }

    /// `Validate`: settings read from disk kept within the UI's limits.
    pub fn validate(&mut self) {
        self.radius = self.radius.clamp(5, 500);
        self.fade_delay = self.fade_delay.clamp(0, 10000);
        self.fade_duration = self.fade_duration.clamp(0, 10000);
        self.ripple_size = self.ripple_size.clamp(10, 300);
        self.ripple_intensity = if self.ripple_intensity.is_finite() { self.ripple_intensity.clamp(0.15, 1.35) } else { 0.7 };
        self.ripple_duration = self.ripple_duration.clamp(60, 2000);
    }

    pub fn is_button_enabled(&self, b: Button) -> bool {
        match b {
            Button::Primary => self.highlight_primary_clicks,
            Button::Secondary => self.highlight_secondary_clicks,
            Button::Middle => self.highlight_middle_clicks,
        }
    }

    pub fn color(&self, b: Button) -> &Color {
        match b {
            Button::Primary => &self.primary_color,
            Button::Secondary => &self.secondary_color,
            Button::Middle => &self.middle_color,
        }
    }

    pub fn show_release_crosshairs(&self, b: Button) -> bool {
        match b {
            Button::Primary => self.highlight_primary_clicks && self.show_primary_release_crosshairs,
            Button::Secondary => self.highlight_secondary_clicks && self.show_secondary_release_crosshairs,
            Button::Middle => self.highlight_middle_clicks && self.show_middle_release_crosshairs,
        }
    }
}

/// `MouseHighlight`: a click, in desktop pixels and milliseconds since the
/// highlighter started.
#[derive(Clone, Debug, PartialEq)]
pub struct Highlight {
    pub position: (i32, i32),
    pub button: Button,
    pub started: f64,
    pub released: Option<f64>,
    pub crosshairs: bool,
}

/// `MouseHighlighterService`'s state: the cursor, the clicks shown, the
/// ones still held.
#[derive(Clone, Debug)]
pub struct Highlighter {
    pub options: MouseHighlighterOptions,
    pub cursor: (i32, i32),
    pub highlights: Vec<Highlight>,
    /// Indices into `highlights` of each button's held click.
    held: [Option<usize>; 3],
}

impl Highlighter {
    pub fn new(mut options: MouseHighlighterOptions, cursor: (i32, i32)) -> Self {
        options.validate();
        Self { options, cursor, highlights: Vec::new(), held: [None; 3] }
    }

    /// `UpdateOptions`: the effects so far are dropped.
    pub fn update_options(&mut self, mut options: MouseHighlighterOptions) {
        options.validate();
        self.options = options;
        self.highlights.clear();
        self.held = [None; 3];
    }

    /// `MoveCursor`: held clicks follow, except ripples that don't.
    pub fn move_cursor(&mut self, p: (i32, i32)) {
        self.cursor = p;
        if self.options.mode != Mode::Ripple || self.options.follow_cursor_while_held {
            for i in self.held.iter().flatten() {
                self.highlights[*i].position = p;
            }
        }
    }

    /// `Press`.
    pub fn press(&mut self, b: Button, p: (i32, i32), time: f64) {
        if !self.options.is_button_enabled(b) {
            return;
        }
        if let Some(i) = self.held[b as usize] {
            self.highlights[i].released = Some(time);
        }
        self.highlights.push(Highlight { position: p, button: b, started: time, released: None, crosshairs: false });
        self.held[b as usize] = Some(self.highlights.len() - 1);
    }

    /// `Release`.
    pub fn release(&mut self, b: Button, p: (i32, i32), time: f64) {
        let Some(i) = self.held[b as usize].take() else { return };
        self.highlights[i].released = Some(time);
        if self.options.mode == Mode::Ripple && self.options.show_release_crosshairs(b) {
            self.highlights.push(Highlight { position: p, button: b, started: time, released: Some(time), crosshairs: true });
        }
    }

    /// The input buffer overflowed: everything restarts from the buttons
    /// held now.
    pub fn reset(&mut self, cursor: (i32, i32), pressed: u8, time: f64) {
        self.highlights.clear();
        self.held = [None; 3];
        self.move_cursor(cursor);
        for b in [Button::Primary, Button::Secondary, Button::Middle] {
            if pressed & (1 << b as u8) != 0 {
                self.press(b, self.cursor, time);
            }
        }
    }

    /// `OnTick`'s clean-up: finished effects go, and at most 64 are kept.
    pub fn tick(&mut self, time: f64) {
        let life = if self.options.mode == Mode::Ripple {
            self.options.ripple_duration as f64
        } else {
            (self.options.fade_delay + self.options.fade_duration) as f64
        };
        let keep: Vec<bool> = self.highlights.iter().map(|h| !h.released.is_some_and(|r| time - r >= life)).collect();
        self.retain(&keep);
        while self.highlights.len() > 64 {
            let Some(i) = self.highlights.iter().position(|h| h.released.is_some()) else { break };
            let mut keep = vec![true; self.highlights.len()];
            keep[i] = false;
            self.retain(&keep);
        }
    }

    fn retain(&mut self, keep: &[bool]) {
        let mut map = vec![None; keep.len()];
        let mut next = 0;
        for (i, k) in keep.iter().enumerate() {
            if *k {
                map[i] = Some(next);
                next += 1;
            }
        }
        let mut i = 0;
        self.highlights.retain(|_| {
            let k = keep[i];
            i += 1;
            k
        });
        for h in &mut self.held {
            *h = h.and_then(|i| map[i]);
        }
    }

    /// Whether anything is drawn at all (the overlays hide otherwise).
    pub fn is_idle(&self) -> bool {
        self.highlights.is_empty() && !(self.options.mode == Mode::Circle && self.options.always_color.a > 0)
    }

    /// `GetEffectBounds` on a monitor: what this frame draws there, in
    /// desktop pixels (`None` when nothing).
    pub fn effect_bounds(&self, screen: (i32, i32, i32, i32)) -> Option<(i32, i32, i32, i32)> {
        let o = &self.options;
        let around = |(x, y): (i32, i32), r: i32| (x - r, y - r, r * 2 + 1, r * 2 + 1);
        let mut bounds: Option<(i32, i32, i32, i32)> = None;
        let mut add = |r: (i32, i32, i32, i32)| {
            if let Some(i) = intersect(r, screen) {
                bounds = Some(match bounds {
                    Some(b) => union(b, i),
                    None => i,
                });
            }
        };
        if o.mode == Mode::Circle && o.always_color.a > 0 {
            add(around(self.cursor, o.radius + 2));
        }
        for h in &self.highlights {
            if o.color(h.button).a == 0 {
                continue;
            }
            let r = if o.mode == Mode::Ripple { o.ripple_size / 2 + 12 } else { o.radius + 2 };
            add(around(h.position, r));
        }
        bounds.and_then(|b| intersect(b, screen))
    }

    /// `Draw`: this frame's effects into `pm`, whose top-left is the
    /// desktop pixel `origin`.
    pub fn draw(&self, pm: &mut Pixmap, origin: (i32, i32), time: f64) {
        let o = &self.options;
        let (ox, oy) = (origin.0 as f32, origin.1 as f32);
        let at = |(x, y): (i32, i32)| (x as f32 - ox, y as f32 - oy);
        if o.mode == Mode::Circle {
            let (cx, cy) = at(self.cursor);
            fill_circle(pm, cx, cy, o.radius as f32, paint(&o.always_color, 1.0));
        }
        for h in &self.highlights {
            let c = o.color(h.button);
            let (cx, cy) = at(h.position);
            if o.mode == Mode::Circle {
                fill_circle(pm, cx, cy, o.radius as f32, paint(c, fade_opacity(h, o, time)));
                continue;
            }
            let progress = ((time - h.started) / o.ripple_duration as f64).clamp(0.0, 1.0);
            let fade = match h.released {
                Some(r) => 1.0 - ((time - r) / o.ripple_duration as f64).clamp(0.0, 1.0),
                None => 1.0,
            };
            let radius = (o.ripple_size as f64 / 2.0 * (0.35 + 0.65 * progress)) as f32;
            let ring = paint(c, fade * o.ripple_intensity);
            let stroke = Stroke { width: (3.0 * o.ripple_intensity) as f32, cap: Default::default(), join: Default::default(), miter_limit: 4.0, dash: None };
            if h.crosshairs {
                let gap = radius * 0.4;
                for (x0, y0, x1, y1) in [
                    (cx - radius, cy, cx - gap, cy),
                    (cx + gap, cy, cx + radius, cy),
                    (cx, cy - radius, cx, cy - gap),
                    (cx, cy + gap, cx, cy + radius),
                ] {
                    let mut p = Path::new();
                    p.move_to(x0, y0).line_to(x1, y1);
                    sharex_skia::draw::stroke_path(pm, &p, &stroke, &Matrix::IDENTITY, ring, None);
                }
            } else {
                let glow = paint(c, fade * o.ripple_intensity * 0.3);
                blurred_circle(pm, cx, cy, radius, glow, 3.0);
                let mut p = Path::new();
                p.add_circle(cx, cy, radius);
                sharex_skia::draw::stroke_path(pm, &p, &stroke, &Matrix::IDENTITY, ring, None);
            }
        }
    }
}

/// `FadeOpacity` (circle mode).
pub fn fade_opacity(h: &Highlight, o: &MouseHighlighterOptions, time: f64) -> f64 {
    let Some(r) = h.released else { return 1.0 };
    let age = time - r - o.fade_delay as f64;
    if age < 0.0 {
        return 1.0;
    }
    if o.fade_duration == 0 { 0.0 } else { (1.0 - age / o.fade_duration as f64).clamp(0.0, 1.0) }
}

/// `Paint(color, opacity)`: the colour's alpha scaled and rounded.
fn paint(c: &Color, opacity: f64) -> SkColor {
    let a = (c.a as f64 * opacity).round().clamp(0.0, 255.0) as u8;
    SkColor::argb(a, c.r, c.g, c.b)
}

fn fill_circle(pm: &mut Pixmap, cx: f32, cy: f32, r: f32, color: SkColor) {
    if color.a == 0 {
        return;
    }
    let mut p = Path::new();
    p.add_circle(cx, cy, r);
    sharex_skia::draw::fill_path(pm, &p, color, None);
}

/// A filled circle under `SKMaskFilter.CreateBlur(Normal, sigma)`: the
/// coverage mask blurred, then the colour drawn through it.
fn blurred_circle(pm: &mut Pixmap, cx: f32, cy: f32, r: f32, color: SkColor, sigma: f32) {
    if color.a == 0 || r <= 0.0 {
        return;
    }
    let pad = (sigma * 3.0).ceil() as i32 + 1;
    let (x0, y0) = ((cx - r).floor() as i32 - pad, (cy - r).floor() as i32 - pad);
    let size = ((r * 2.0).ceil() as i32 + 2 * pad + 2).max(1) as u32;
    let mut mask = Pixmap::new(size, size);
    let mut p = Path::new();
    p.add_circle(cx - x0 as f32, cy - y0 as f32, r);
    sharex_skia::draw::fill_path(&mut mask, &p, SkColor::argb(255, 255, 255, 255), None);
    let mask = sharex_skia::blur::blur(&mask, sigma, sigma);
    let (w, h) = (pm.width() as i32, pm.height() as i32);
    for my in 0..size as i32 {
        let y = y0 + my;
        if y < 0 || y >= h {
            continue;
        }
        for mx in 0..size as i32 {
            let x = x0 + mx;
            if x < 0 || x >= w {
                continue;
            }
            let coverage = sharex_skia::pixmap::get_a(mask.pixel(mx, my));
            if coverage == 0 {
                continue;
            }
            let a = (color.a as u32 * coverage + 127) / 255;
            blend(pm.pixel_mut(x, y), a, color);
        }
    }
}

/// Source-over of a straight colour at alpha `a` onto a premultiplied pixel.
fn blend(dst: &mut u32, a: u32, c: SkColor) {
    let mul = |v: u32| (v * a + 127) / 255;
    let (sr, sg, sb) = (mul(c.r as u32), mul(c.g as u32), mul(c.b as u32));
    let inv = 255 - a;
    let d = *dst;
    let ch = |s: u32, sh: u32| s + (((d >> sh) & 0xFF) * inv + 127) / 255;
    *dst = sharex_skia::pixmap::pack(a + ((d >> 24) * inv + 127) / 255, ch(sr, 16), ch(sg, 8), ch(sb, 0));
}

fn intersect(a: (i32, i32, i32, i32), b: (i32, i32, i32, i32)) -> Option<(i32, i32, i32, i32)> {
    let l = a.0.max(b.0);
    let t = a.1.max(b.1);
    let r = (a.0 + a.2).min(b.0 + b.2);
    let bt = (a.1 + a.3).min(b.1 + b.3);
    (r > l && bt > t).then_some((l, t, r - l, bt - t))
}

fn union(a: (i32, i32, i32, i32), b: (i32, i32, i32, i32)) -> (i32, i32, i32, i32) {
    let l = a.0.min(b.0);
    let t = a.1.min(b.1);
    let r = (a.0 + a.2).max(b.0 + b.2);
    let bt = (a.1 + a.3).max(b.1 + b.3);
    (l, t, r - l, bt - t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_round_trip() {
        let d = MouseHighlighterOptions::default();
        let mut v = json!({});
        d.write_json(&mut v);
        assert_eq!(v["PrimaryColor"], "166, 54, 169, 225");
        assert_eq!(v["AlwaysColor"], "Transparent");
        assert_eq!(MouseHighlighterOptions::from_json(&v), d);
    }

    #[test]
    fn clicks_age_and_follow() {
        let o = MouseHighlighterOptions { mode: Mode::Circle, ..Default::default() };
        let mut h = Highlighter::new(o, (0, 0));
        h.press(Button::Primary, (10, 10), 0.0);
        h.move_cursor((20, 20));
        assert_eq!(h.highlights[0].position, (20, 20));
        h.release(Button::Primary, (20, 20), 100.0);
        h.tick(849.0);
        assert_eq!(h.highlights.len(), 1);
        h.tick(850.0);
        assert!(h.highlights.is_empty());
        assert_eq!(h.effect_bounds((0, 0, 100, 100)), None);
        h.press(Button::Secondary, (50, 50), 900.0);
        assert_eq!(h.effect_bounds((0, 0, 100, 100)), Some((28, 28, 45, 45)));
        let mut pm = Pixmap::new(45, 45);
        h.draw(&mut pm, (28, 28), 900.0);
        // The centre is the secondary colour at 166/255.
        assert_eq!(sharex_skia::pixmap::get_a(pm.pixel(22, 22)), 166);
    }
}
