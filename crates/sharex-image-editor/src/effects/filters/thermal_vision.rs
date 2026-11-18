//! `ThermalVisionImageEffect` (`Core/ImageEffects/Filters/ThermalVisionImageEffect.cs`).

use crate::effects::prelude::*;

/// `ThermalVisionImageEffect`.
#[derive(Clone, Debug)]
pub struct ThermalVisionImageEffect {
    pub levels: i32,
    pub contrast: f32,
    pub glow: f32,
    pub blend: f32,
    pub invert: bool,
}

impl Default for ThermalVisionImageEffect {
    fn default() -> Self {
        ThermalVisionImageEffect {
            levels: 8,
            contrast: 135.0,
            glow: 28.0,
            blend: 100.0,
            invert: false,
        }
    }
}

impl EffectImpl for ThermalVisionImageEffect {
    fn info() -> Info {
        Info::new(
            "thermal_vision",
            "Thermal vision",
            EffectCategory::Filters,
            "image-effect/thermometer-sun",
        )
        .desc("Maps image luminance to a thermal color gradient.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("levels", "Levels", 3, 16, 8, |e: &mut Self, v| e.levels = v),
            float_slider::<Self>(
                "contrast",
                "Contrast",
                50.0,
                200.0,
                135.0,
                |e: &mut Self, v| e.contrast = v,
            ),
            float_slider::<Self>("glow", "Glow", 0.0, 100.0, 28.0, |e: &mut Self, v| {
                e.glow = v
            }),
            float_slider::<Self>("blend", "Blend", 0.0, 100.0, 100.0, |e: &mut Self, v| {
                e.blend = v
            }),
            bool_param::<Self>("invert", "Invert", false, |e: &mut Self, v| e.invert = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let levels = self.levels.clamp(3, 16);
        let contrast = clampf(self.contrast, 50.0, 200.0) / 100.0;
        let glow = clampf(self.glow, 0.0, 100.0) / 100.0;
        let blend = clampf(self.blend, 0.0, 100.0) / 100.0;
        if source.is_empty() {
            return Ok(source.clone());
        }
        let dst: Vec<SkColor> = pixels(source)
            .iter()
            .map(|s| {
                let mut l =
                    ((0.2126 * s.r as f32) + (0.7152 * s.g as f32) + (0.0722 * s.b as f32)) / 255.0;
                l = clampf(((l - 0.5) * contrast) + 0.5, 0.0, 1.0);
                if self.invert {
                    l = 1.0 - l;
                }
                let q = (l * (levels - 1) as f32).round_ties_even() / 1f32.max(levels as f32 - 1.0);
                let t = thermal_gradient(q);
                let gb = glow * q.powf(2.2);
                let tr = t.r as f32 + (t.r as f32 * gb * 0.20);
                let tg = t.g as f32 + (t.g as f32 * gb * 0.25);
                let tb = t.b as f32 + (t.b as f32 * gb * 0.35);
                SkColor::rgba(
                    clamp_to_byte(lerp(s.r as f32, tr, blend)),
                    clamp_to_byte(lerp(s.g as f32, tg, blend)),
                    clamp_to_byte(lerp(s.b as f32, tb, blend)),
                    s.a,
                )
            })
            .collect();
        Ok(from_pixels(source.width(), source.height(), &dst))
    }
}

// ---- port ----

fn thermal_gradient(t: f32) -> SkColor {
    let t = clampf(t, 0.0, 1.0);
    let l = |a: (u8, u8, u8), b: (u8, u8, u8), t: f32| {
        SkColor::rgba(
            clamp_to_byte(lerp(a.0 as f32, b.0 as f32, t)),
            clamp_to_byte(lerp(a.1 as f32, b.1 as f32, t)),
            clamp_to_byte(lerp(a.2 as f32, b.2 as f32, t)),
            255,
        )
    };
    if t <= 0.20 {
        return l((0, 0, 52), (0, 34, 170), t / 0.20);
    }
    if t <= 0.40 {
        return l((0, 34, 170), (0, 160, 255), (t - 0.20) / 0.20);
    }
    if t <= 0.60 {
        return l((0, 160, 255), (0, 255, 210), (t - 0.40) / 0.20);
    }
    if t <= 0.76 {
        return l((0, 255, 210), (255, 255, 0), (t - 0.60) / 0.16);
    }
    if t <= 0.90 {
        return l((255, 255, 0), (255, 80, 0), (t - 0.76) / 0.14);
    }
    l((255, 80, 0), (255, 255, 255), (t - 0.90) / 0.10)
}
