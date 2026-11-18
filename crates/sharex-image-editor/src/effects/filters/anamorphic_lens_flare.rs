//! `AnamorphicLensFlareImageEffect` (`Core/ImageEffects/Filters/AnamorphicLensFlareImageEffect.cs`).

use crate::effects::prelude::*;

/// `AnamorphicLensFlareImageEffect`.
#[derive(Clone, Debug)]
pub struct AnamorphicLensFlareImageEffect {
    pub intensity: f32,
    pub threshold: f32,
    pub streak_length: f32,
    pub warmth: f32,
    pub ghosting: f32,
}

impl Default for AnamorphicLensFlareImageEffect {
    fn default() -> Self {
        AnamorphicLensFlareImageEffect {
            intensity: 60.0,
            threshold: 72.0,
            streak_length: 55.0,
            warmth: 45.0,
            ghosting: 35.0,
        }
    }
}

impl EffectImpl for AnamorphicLensFlareImageEffect {
    fn info() -> Info {
        Info::new(
            "anamorphic_lens_flare",
            "Anamorphic lens flare",
            EffectCategory::Filters,
            "image-effect/sparkle",
        )
        .desc("Adds cinematic horizontal lens flare streaks to bright areas.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>(
                "intensity",
                "Intensity",
                0.0,
                100.0,
                60.0,
                |e: &mut Self, v| e.intensity = v,
            ),
            float_slider::<Self>(
                "threshold",
                "Threshold",
                0.0,
                100.0,
                72.0,
                |e: &mut Self, v| e.threshold = v,
            ),
            float_slider::<Self>(
                "streak_length",
                "Streak length",
                0.0,
                100.0,
                55.0,
                |e: &mut Self, v| e.streak_length = v,
            ),
            float_slider::<Self>("warmth", "Warmth", 0.0, 100.0, 45.0, |e: &mut Self, v| {
                e.warmth = v
            }),
            float_slider::<Self>(
                "ghosting",
                "Ghosting",
                0.0,
                100.0,
                35.0,
                |e: &mut Self, v| e.ghosting = v,
            ),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let intensity = clampf(self.intensity, 0.0, 100.0) / 100.0;
        let threshold = clampf(self.threshold, 0.0, 100.0) / 100.0;
        let streak = clampf(self.streak_length, 0.0, 100.0) / 100.0;
        let warmth = clampf(self.warmth, 0.0, 100.0) / 100.0;
        let ghosting = clampf(self.ghosting, 0.0, 100.0) / 100.0;
        if intensity <= 0.0001 {
            return Ok(source.clone());
        }
        let (width, height) = (source.width() as i32, source.height() as i32);
        let cx = (width - 1) as f32 * 0.5;
        let cy = (height - 1) as f32 * 0.5;
        let sigma = 0.14 + (streak * 0.22);
        let sigma_inv = 1.0 / 0.0001f32.max(2.0 * sigma * sigma);
        let max_offset = 10.0 + (streak * 120.0);
        let steps = 5 + (streak * 4.0).round_ties_even() as i32;
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        let lum =
            |c: SkColor| (0.2126 * c.r as f32) + (0.7152 * c.g as f32) + (0.0722 * c.b as f32);
        let screen = |s: f32, o: f32| {
            let o = 1f32.min(0f32.max(o));
            1.0 - ((1.0 - s) * (1.0 - o))
        };
        for y in 0..height {
            let row = y * width;
            let y_norm = (y as f32 - cy) / 1f32.max((height - 1) as f32);
            let profile = 0.62 + (0.38 * (-(y_norm * y_norm) * sigma_inv).exp());
            for x in 0..width {
                let s = src[(row + x) as usize];
                let (sr, sg, sb) = (s.r as f32 / 255.0, s.g as f32 / 255.0, s.b as f32 / 255.0);
                let mut flare = 0f32;
                for i in -steps..=steps {
                    let t = i as f32 / 1.max(steps) as f32;
                    let offset = t * max_offset;
                    let sample = bilinear_sample(&src, width, height, x as f32 + offset, y as f32);
                    let l = lum(sample) / 255.0;
                    let bright = clamp01((l - threshold) / 0.0001f32.max(1.0 - threshold));
                    let weight = 1.0 / (1.0 + ((i as f32).abs() * 0.7));
                    flare += bright * weight;
                }
                let ghost_x =
                    ((width - 1) - x) as f32 + ((x as f32 - cx) * (0.40 + (ghosting * 0.85)));
                let gs = bilinear_sample(&src, width, height, ghost_x, y as f32);
                let gl = lum(gs) / 255.0;
                let mut ghost =
                    clamp01((gl - (threshold * 0.82)) / 0.0001f32.max(1.0 - (threshold * 0.82)));
                ghost *= intensity * ghosting * 0.55 * profile;
                let fs = flare * intensity * 0.12 * profile;
                let warm_r = 1.0f32;
                let warm_g = lerp(1.0, 0.86, warmth);
                let warm_b = lerp(1.0, 0.58, warmth);
                let ghost_r = lerp(0.88, 1.0, warmth);
                let ghost_g = 0.82f32;
                let ghost_b = lerp(1.0, 0.74, warmth);
                let or = (fs * warm_r) + (ghost * ghost_r);
                let og = (fs * warm_g) + (ghost * ghost_g);
                let ob = (fs * warm_b) + (ghost * ghost_b);
                dst[(row + x) as usize] = SkColor::rgba(
                    clamp_to_byte(screen(sr, or) * 255.0),
                    clamp_to_byte(screen(sg, og) * 255.0),
                    clamp_to_byte(screen(sb, ob) * 255.0),
                    s.a,
                );
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
