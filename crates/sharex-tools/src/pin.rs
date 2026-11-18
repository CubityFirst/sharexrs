//! Pin to screen's options and sizes: `ShareX.Tools/Tools/PinToScreen/
//! PinToScreenContracts.cs` (`PinToScreenOptions`), the sizes and places
//! `PinToScreenWindow.axaml.cs` works out in physical pixels, and
//! `Helpers.GetPosition`.

use serde_json::{Value, json};

use crate::color::Color;

/// `ImageContentAlignment`, in `PinToScreenOptionsViewModel.Placements` order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placement {
    TopLeft,
    TopCenter,
    TopRight,
    MiddleLeft,
    MiddleCenter,
    MiddleRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

impl Placement {
    pub const ALL: [Placement; 9] = [
        Placement::TopLeft,
        Placement::TopCenter,
        Placement::TopRight,
        Placement::MiddleLeft,
        Placement::MiddleCenter,
        Placement::MiddleRight,
        Placement::BottomLeft,
        Placement::BottomCenter,
        Placement::BottomRight,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Placement::TopLeft => "TopLeft",
            Placement::TopCenter => "TopCenter",
            Placement::TopRight => "TopRight",
            Placement::MiddleLeft => "MiddleLeft",
            Placement::MiddleCenter => "MiddleCenter",
            Placement::MiddleRight => "MiddleRight",
            Placement::BottomLeft => "BottomLeft",
            Placement::BottomCenter => "BottomCenter",
            Placement::BottomRight => "BottomRight",
        }
    }

    /// Json.NET's `StringEnumConverter` (names, case-insensitive) or the
    /// number.
    pub fn parse(v: &Value) -> Option<Self> {
        match v {
            Value::String(s) => Self::ALL.into_iter().find(|p| p.name().eq_ignore_ascii_case(s.trim())),
            Value::Number(n) => n.as_u64().and_then(|i| Self::ALL.get(i as usize).copied()),
            _ => None,
        }
    }
}

/// `PinToScreenOptions`.
#[derive(Clone, Debug, PartialEq)]
pub struct PinToScreenOptions {
    pub initial_scale: i32,
    pub scale_step: i32,
    pub high_quality_scale: bool,
    pub initial_opacity: i32,
    pub opacity_step: i32,
    pub placement: Placement,
    pub placement_offset: i32,
    pub top_most: bool,
    pub keep_center_location: bool,
    pub background_color: Color,
    pub shadow: bool,
    pub border: bool,
    pub border_size: i32,
    pub border_color: Color,
    pub minimize_size: (i32, i32),
}

impl Default for PinToScreenOptions {
    fn default() -> Self {
        Self {
            initial_scale: 100,
            scale_step: 10,
            high_quality_scale: true,
            initial_opacity: 100,
            opacity_step: 10,
            placement: Placement::BottomRight,
            placement_offset: 10,
            top_most: true,
            keep_center_location: true,
            background_color: Color::named("White"),
            shadow: true,
            border: true,
            border_size: 2,
            border_color: Color::named("CornflowerBlue"),
            minimize_size: (100, 100),
        }
    }
}

fn int(v: &Value, key: &str, default: i32) -> i32 {
    v.get(key).and_then(Value::as_i64).map_or(default, |n| n as i32)
}

fn boolean(v: &Value, key: &str, default: bool) -> bool {
    v.get(key).and_then(Value::as_bool).unwrap_or(default)
}

pub(crate) fn color(v: &Value, key: &str, default: Color) -> Color {
    v.get(key).and_then(Value::as_str).and_then(Color::parse).unwrap_or(default)
}

/// `System.Drawing.Size` as Json.NET writes it: "W, H".
fn size(v: &Value, key: &str, default: (i32, i32)) -> (i32, i32) {
    let Some(s) = v.get(key).and_then(Value::as_str) else { return default };
    let parts: Vec<i32> = s.split(',').filter_map(|p| p.trim().parse().ok()).collect();
    match parts.as_slice() {
        [w, h] => (*w, *h),
        _ => default,
    }
}

/// Sets `key` on the object (keeping its place when it's there).
pub(crate) fn set(target: &mut Value, key: &str, value: Value) {
    if !target.is_object() {
        *target = json!({});
    }
    if let Some(m) = target.as_object_mut() {
        m.insert(key.to_string(), value);
    }
}

impl PinToScreenOptions {
    pub fn from_json(v: &Value) -> Self {
        let d = Self::default();
        Self {
            initial_scale: int(v, "InitialScale", d.initial_scale),
            scale_step: int(v, "ScaleStep", d.scale_step),
            high_quality_scale: boolean(v, "HighQualityScale", d.high_quality_scale),
            initial_opacity: int(v, "InitialOpacity", d.initial_opacity),
            opacity_step: int(v, "OpacityStep", d.opacity_step),
            placement: v.get("Placement").and_then(Placement::parse).unwrap_or(d.placement),
            placement_offset: int(v, "PlacementOffset", d.placement_offset),
            top_most: boolean(v, "TopMost", d.top_most),
            keep_center_location: boolean(v, "KeepCenterLocation", d.keep_center_location),
            background_color: color(v, "BackgroundColor", d.background_color),
            shadow: boolean(v, "Shadow", d.shadow),
            border: boolean(v, "Border", d.border),
            border_size: int(v, "BorderSize", d.border_size),
            border_color: color(v, "BorderColor", d.border_color),
            minimize_size: size(v, "MinimizeSize", d.minimize_size),
        }
    }

    /// Writes every property into `target` in ShareX's order.
    pub fn write_json(&self, target: &mut Value) {
        set(target, "InitialScale", json!(self.initial_scale));
        set(target, "ScaleStep", json!(self.scale_step));
        set(target, "HighQualityScale", json!(self.high_quality_scale));
        set(target, "InitialOpacity", json!(self.initial_opacity));
        set(target, "OpacityStep", json!(self.opacity_step));
        set(target, "Placement", json!(self.placement.name()));
        set(target, "PlacementOffset", json!(self.placement_offset));
        set(target, "TopMost", json!(self.top_most));
        set(target, "KeepCenterLocation", json!(self.keep_center_location));
        set(target, "BackgroundColor", json!(self.background_color.to_string()));
        set(target, "Shadow", json!(self.shadow));
        set(target, "Border", json!(self.border));
        set(target, "BorderSize", json!(self.border_size));
        set(target, "BorderColor", json!(self.border_color.to_string()));
        set(target, "MinimizeSize", json!(format!("{}, {}", self.minimize_size.0, self.minimize_size.1)));
    }
}

/// `Helpers.GetPosition(placement, offset, background, objectSize)`.
pub fn get_position(placement: Placement, offset: i32, bg: (i32, i32, i32, i32), obj: (i32, i32)) -> (i32, i32) {
    let (bx, by, bw, bh) = bg;
    let mid_x = ((bw as f32 / 2.0) - (obj.0 as f32 / 2.0)).round_ties_even() as i32;
    let mid_y = ((bh as f32 / 2.0) - (obj.1 as f32 / 2.0)).round_ties_even() as i32;
    let right = bw - obj.0;
    let bottom = bh - obj.1;
    let (x, y) = match placement {
        Placement::TopLeft => (offset, offset),
        Placement::TopCenter => (mid_x, offset),
        Placement::TopRight => (right - offset, offset),
        Placement::MiddleLeft => (offset, mid_y),
        Placement::MiddleCenter => (mid_x, mid_y),
        Placement::MiddleRight => (right - offset, mid_y),
        Placement::BottomLeft => (offset, bottom - offset),
        Placement::BottomCenter => (mid_x, bottom - offset),
        Placement::BottomRight => (right - offset, bottom - offset),
    };
    (bx + x, by + y)
}

/// `PinToScreenWindow.ShadowMargin`.
pub const SHADOW_MARGIN: i32 = 10;

/// A pin's state that sizes its window (`_imageScale`, `_imageOpacity`,
/// `_minimized`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PinState {
    pub scale: i32,
    pub opacity: i32,
    pub minimized: bool,
}

impl PinState {
    /// The window's starting state: the options' scale (20-500%) and
    /// opacity (10-100%).
    pub fn new(o: &PinToScreenOptions) -> Self {
        Self { scale: o.initial_scale.clamp(20, 500), opacity: o.initial_opacity.clamp(10, 100), minimized: false }
    }

    /// `Opacity`: 1 while minimized.
    pub fn window_opacity(&self) -> f64 {
        if self.minimized { 1.0 } else { self.opacity as f64 / 100.0 }
    }

    /// `ChangeScale`: true when it changed.
    pub fn change_scale(&mut self, delta: i32) -> bool {
        if self.minimized {
            return false;
        }
        let v = (self.scale + delta).clamp(20, 500);
        let changed = v != self.scale;
        self.scale = v;
        changed
    }

    /// `ChangeOpacity`.
    pub fn change_opacity(&mut self, delta: i32) {
        if !self.minimized {
            self.opacity = (self.opacity + delta).clamp(10, 100);
        }
    }

    /// `ResetImage`.
    pub fn reset(&mut self) {
        self.scale = 100;
        self.opacity = 100;
    }
}

/// The pin window's parts in physical pixels (`ResizeWindow`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PinLayout {
    /// The image as shown.
    pub image: (i32, i32),
    pub border: i32,
    pub shadow: i32,
    /// The window.
    pub window: (i32, i32),
}

impl PinLayout {
    /// `GetBorderPixels + GetShadowPixels`.
    pub fn inset(&self) -> i32 {
        self.border + self.shadow
    }
}

/// `ResizeWindow`'s sizes for an image of `bitmap` pixels at `scaling`.
pub fn layout(o: &PinToScreenOptions, s: &PinState, bitmap: (u32, u32), scaling: f64) -> PinLayout {
    let scaling = if scaling.is_finite() && scaling > 0.0 { scaling } else { 1.0 };
    let image = if s.minimized {
        (
            ((o.minimize_size.0 as f64 * scaling).round() as i32).max(1),
            ((o.minimize_size.1 as f64 * scaling).round() as i32).max(1),
        )
    } else {
        let scale = s.scale as f64 / 100.0;
        (((bitmap.0 as f64 * scale).round() as i32).max(1), ((bitmap.1 as f64 * scale).round() as i32).max(1))
    };
    let border = if o.border { ((o.border_size.clamp(1, 30) as f64 * scaling).round() as i32).max(1) } else { 0 };
    let shadow = if o.shadow { (SHADOW_MARGIN as f64 * scaling).round() as i32 } else { 0 };
    let inset = border + shadow;
    PinLayout { image, border, shadow, window: (image.0 + inset * 2, image.1 + inset * 2) }
}

/// `UpdateToolbarVisibility`: shown while the pointer is over a window at
/// least 155x55 (DIPs).
pub fn toolbar_visible(pointer_inside: bool, window_dips: (f64, f64)) -> bool {
    pointer_inside && window_dips.0 >= 155.0 && window_dips.1 >= 55.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_round_trip_in_sharex_order() {
        let v: Value = serde_json::from_str(
            r#"{"InitialScale":100,"ScaleStep":10,"HighQualityScale":true,"InitialOpacity":100,"OpacityStep":10,"Placement":"BottomRight","PlacementOffset":10,"TopMost":true,"KeepCenterLocation":true,"BackgroundColor":"White","Shadow":true,"Border":true,"BorderSize":2,"BorderColor":"CornflowerBlue","MinimizeSize":"100, 100"}"#,
        )
        .unwrap();
        let o = PinToScreenOptions::from_json(&v);
        assert_eq!(o, PinToScreenOptions::default());
        let mut out = v.clone();
        o.write_json(&mut out);
        assert_eq!(serde_json::to_string(&out).unwrap(), serde_json::to_string(&v).unwrap());
    }

    #[test]
    fn sizes_and_places() {
        let o = PinToScreenOptions::default();
        let s = PinState::new(&o);
        let l = layout(&o, &s, (200, 100), 1.0);
        assert_eq!(l, PinLayout { image: (200, 100), border: 2, shadow: 10, window: (224, 124) });
        assert_eq!(get_position(Placement::BottomRight, 10, (0, 0, 1920, 1040), (224, 124)), (1686, 906));
        assert_eq!(get_position(Placement::MiddleCenter, 10, (0, 0, 1920, 1040), (225, 125)), (848, 458));
    }
}
