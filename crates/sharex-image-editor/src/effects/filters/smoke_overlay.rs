//! `SmokeOverlayImageEffect` (`Core/ImageEffects/Filters/SmokeOverlayImageEffect.cs`).

use crate::effects::prelude::*;

/// `SmokeOverlayImageEffect`.
#[derive(Clone, Debug)]
pub struct SmokeOverlayImageEffect {
    pub density: f32,
    pub scale: f32,
    pub drift: f32,
    pub softness: f32,
    pub contrast: f32,
}

impl Default for SmokeOverlayImageEffect {
    fn default() -> Self {
        SmokeOverlayImageEffect {
            density: 42.0,
            scale: 68.0,
            drift: 18.0,
            softness: 55.0,
            contrast: 48.0,
        }
    }
}

impl EffectImpl for SmokeOverlayImageEffect {
    fn info() -> Info {
        Info::new(
            "smoke_overlay",
            "Smoke overlay",
            EffectCategory::Filters,
            "image-effect/fog",
        )
        .desc("Adds a soft procedural smoke overlay to the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("density", "Density", 0.0, 100.0, 42.0, |e: &mut Self, v| {
                e.density = v
            }),
            float_slider::<Self>("scale", "Scale", 0.0, 100.0, 68.0, |e: &mut Self, v| {
                e.scale = v
            }),
            float_slider::<Self>("drift", "Drift", -100.0, 100.0, 18.0, |e: &mut Self, v| {
                e.drift = v
            }),
            float_slider::<Self>(
                "softness",
                "Softness",
                0.0,
                100.0,
                55.0,
                |e: &mut Self, v| e.softness = v,
            ),
            float_slider::<Self>(
                "contrast",
                "Contrast",
                0.0,
                100.0,
                48.0,
                |e: &mut Self, v| e.contrast = v,
            ),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let density = clampf(self.density, 0.0, 100.0) / 100.0;
        let scale = clampf(self.scale, 0.0, 100.0) / 100.0;
        let drift = clampf(self.drift, -100.0, 100.0) / 100.0;
        let softness = clampf(self.softness, 0.0, 100.0) / 100.0;
        let contrast = clampf(self.contrast, 0.0, 100.0) / 100.0;
        if density <= 0.0001 {
            return Ok(source.clone());
        }
        let (width, height) = (source.width() as i32, source.height() as i32);
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        let cf = 0.012 + ((1.0 - scale) * 0.035);
        let mf = cf * 2.2;
        let ff = cf * 4.4;
        for y in 0..height {
            let row = y * width;
            let yf = y as f32;
            let drift_off = drift * yf * 0.014;
            for x in 0..width {
                let xf = x as f32;
                let s = src[(row + x) as usize];
                let (sr, sg, sb) = (s.r as f32 / 255.0, s.g as f32 / 255.0, s.b as f32 / 255.0);
                let px = xf + drift_off;
                let nx = px * cf;
                let ny = yf * cf;
                let wx =
                    ((fractal_noise((nx * 0.85) + 7.2, (ny * 0.85) - 3.1, 3, 2.05, 0.56, 911)
                        * 2.0)
                        - 1.0)
                        * (0.75 + (scale * 1.45));
                let wy =
                    ((fractal_noise((nx * 0.90) - 5.4, (ny * 0.90) + 11.8, 3, 2.00, 0.56, 1201)
                        * 2.0)
                        - 1.0)
                        * (0.60 + (scale * 1.15));
                let (sx, sy) = (nx + wx, ny + wy);
                let coarse = fractal_noise(sx, sy, 4, 2.00, 0.55, 1559);
                let medium = fractal_noise(
                    (sx * (mf / cf)) + 13.1,
                    (sy * (mf / cf)) - 7.6,
                    3,
                    2.10,
                    0.52,
                    2081,
                );
                let fine = fractal_noise(
                    (sx * (ff / cf)) - 9.4,
                    (sy * (ff / cf)) + 4.7,
                    2,
                    2.20,
                    0.58,
                    2593,
                );
                let fa = ((xf * cf * 16.0) + (yf * cf * 6.0) + 0.7).sin();
                let fb = ((xf * cf * 9.0) - (yf * cf * 13.0) - 1.4).cos();
                let wisps = 1.0 - ((medium * 2.0) - 1.0).abs();
                let mut smoke = (coarse * 0.58) + (wisps * 0.24) + (fine * 0.18);
                smoke = clamp01(smoke + fa * 0.08 + fb * 0.06);
                smoke = clamp01(((smoke - 0.5) * (0.92 + (contrast * 1.5))) + 0.5);
                let threshold = 0.44 - (density * 0.16);
                let mut alpha = smooth_step(threshold, 0.96, smoke);
                alpha = alpha.powf(1.55 - (softness * 1.10)) * density * 0.68;
                let r = lerp(sr, 0.82 + (smoke * 0.10), alpha);
                let g = lerp(sg, 0.84 + (smoke * 0.10), alpha);
                let b = lerp(sb, 0.86 + (smoke * 0.12), alpha);
                dst[(row + x) as usize] = SkColor::rgba(
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
