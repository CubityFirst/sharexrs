//! `StarFilterImageEffect` (`Core/ImageEffects/Filters/StarFilterImageEffect.cs`).

use crate::effects::prelude::*;

/// `StarFilterImageEffect`.
#[derive(Clone, Debug)]
pub struct StarFilterImageEffect {
    pub threshold: f32,
    pub length: f32,
    pub strength: f32,
    pub rotation: f32,
    pub warmth: f32,
}

impl Default for StarFilterImageEffect {
    fn default() -> Self {
        StarFilterImageEffect {
            threshold: 74.0,
            length: 40.0,
            strength: 58.0,
            rotation: 0.0,
            warmth: 48.0,
        }
    }
}

impl EffectImpl for StarFilterImageEffect {
    fn info() -> Info {
        Info::new(
            "star_filter",
            "Star filter",
            EffectCategory::Filters,
            "app/star",
        )
        .desc("Adds star-shaped light streaks to bright areas.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>(
                "threshold",
                "Threshold",
                0.0,
                100.0,
                74.0,
                |e: &mut Self, v| e.threshold = v,
            ),
            float_slider::<Self>("length", "Length", 4.0, 80.0, 40.0, |e: &mut Self, v| {
                e.length = v
            }),
            float_slider::<Self>(
                "strength",
                "Strength",
                0.0,
                100.0,
                58.0,
                |e: &mut Self, v| e.strength = v,
            ),
            float_slider::<Self>(
                "rotation",
                "Rotation",
                0.0,
                360.0,
                0.0,
                |e: &mut Self, v| e.rotation = v,
            ),
            float_slider::<Self>("warmth", "Warmth", 0.0, 100.0, 48.0, |e: &mut Self, v| {
                e.warmth = v
            }),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let threshold = clampf(self.threshold, 0.0, 100.0) / 100.0;
        let length = clampf(self.length, 4.0, 80.0);
        let strength = clampf(self.strength, 0.0, 100.0) / 100.0;
        let warmth = clampf(self.warmth, 0.0, 100.0) / 100.0;
        if strength <= 0.0001 {
            return Ok(source.clone());
        }
        let (width, height) = (source.width() as i32, source.height() as i32);
        let n = ((length / 4.0).round_ties_even() as i32).clamp(3, 18);
        let step = 1f32.max(length / n as f32);
        let pi = std::f32::consts::PI;
        let base = self.rotation * (pi / 180.0);
        let angles = [
            base,
            base + (pi * 0.5),
            base + (pi * 0.25),
            base + (pi * 0.75),
        ];
        let weights = [1f32, 1.0, 0.68, 0.68];
        let dirs: Vec<(f32, f32)> = angles.iter().map(|a| (a.cos(), a.sin())).collect();
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        let denom = 0.0001f32.max(1.0 - threshold);
        for y in 0..height {
            for x in 0..width {
                let mut streak = 0f32;
                for d in 0..4 {
                    let (dx, dy) = dirs[d];
                    for i in 1..=n {
                        let falloff = 1.0 - (i as f32 / (n + 1) as f32);
                        let off = i as f32 * step;
                        let p = bilinear_sample(
                            &src,
                            width,
                            height,
                            x as f32 + (dx * off),
                            y as f32 + (dy * off),
                        );
                        let q = bilinear_sample(
                            &src,
                            width,
                            height,
                            x as f32 - (dx * off),
                            y as f32 - (dy * off),
                        );
                        let la = 0f32.max(analog_luminance01(p) - threshold) / denom;
                        let lb = 0f32.max(analog_luminance01(q) - threshold) / denom;
                        streak += ((la * la) + (lb * lb)) * falloff * weights[d];
                    }
                }
                let star = strength * (streak / (n as f32 * 3.6));
                let s = src[(y * width + x) as usize];
                let r = screen(s.r as f32 / 255.0, star * (0.90 + (warmth * 0.18)));
                let g = screen(s.g as f32 / 255.0, star * (0.74 + (warmth * 0.08)));
                let b = screen(s.b as f32 / 255.0, star * (0.52 - (warmth * 0.16)));
                dst[(y * width + x) as usize] = SkColor::rgba(
                    clamp_to_byte(r * 255.0),
                    clamp_to_byte(g * 255.0),
                    clamp_to_byte(b * 255.0),
                    s.a,
                );
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
