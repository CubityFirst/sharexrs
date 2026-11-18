//! `DisposableCameraImageEffect` (`Core/ImageEffects/Filters/DisposableCameraImageEffect.cs`).

use crate::effects::prelude::*;

/// `DisposableCameraImageEffect`.
#[derive(Clone, Debug)]
pub struct DisposableCameraImageEffect {
    pub flash: f32,
    pub softness: f32,
    pub grain: f32,
    pub vignette: f32,
    pub warmth: f32,
    pub seed: i32,
}

impl Default for DisposableCameraImageEffect {
    fn default() -> Self {
        DisposableCameraImageEffect {
            flash: 55.0,
            softness: 8.0,
            grain: 24.0,
            vignette: 34.0,
            warmth: 42.0,
            seed: 1996,
        }
    }
}

impl EffectImpl for DisposableCameraImageEffect {
    fn info() -> Info {
        Info::new("disposable_camera", "Disposable camera", EffectCategory::Filters, "app/camera")
            .desc("Simulates the look of a disposable film camera with flash, grain, and warmth.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("flash", "Flash", 0.0, 100.0, 55.0, |e: &mut Self, v| e.flash = v),
            float_slider::<Self>("softness", "Softness", 0.0, 24.0, 8.0, |e: &mut Self, v| e.softness = v),
            float_slider::<Self>("grain", "Grain", 0.0, 100.0, 24.0, |e: &mut Self, v| e.grain = v),
            float_slider::<Self>("vignette", "Vignette", 0.0, 100.0, 34.0, |e: &mut Self, v| e.vignette = v),
            float_slider::<Self>("warmth", "Warmth", 0.0, 100.0, 42.0, |e: &mut Self, v| e.warmth = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let flash = clampf(self.flash, 0.0, 100.0) / 100.0;
        let softness = clampf(self.softness, 0.0, 24.0);
        let grain = clampf(self.grain, 0.0, 100.0) / 100.0;
        let vignette = clampf(self.vignette, 0.0, 100.0) / 100.0;
        let warmth = clampf(self.warmth, 0.0, 100.0) / 100.0;
        if flash <= 0.0001 && softness <= 0.0001 && grain <= 0.0001 && vignette <= 0.0001 && warmth <= 0.0001 {
            return Ok(source.clone());
        }
        let (width, height) = (source.width() as i32, source.height() as i32);
        let cx = (width - 1) as f32 * 0.56;
        let cy = (height - 1) as f32 * 0.44;
        let max_dist = 1f32.max(((cx * cx) + (cy * cy)).sqrt());
        let src = pixels(source);
        let blur_px = (softness > 0.01).then(|| pixels(&create_blurred_clamp(source, softness)));
        let soft_px = blur_px.as_ref().unwrap_or(&src);
        let mut dst = vec![SkColor::default(); src.len()];
        let sigma_inv = 1.0 / 0.0001f32.max(2.0 * (0.22 + (flash * 0.20)).powf(2.0));
        let soft_mix = (softness / 24.0) * 0.34;
        let contrast = 1.08 + (flash * 0.16) + (warmth * 0.08);
        let (hw, hh) = ((width - 1) as f32 * 0.5, (height - 1) as f32 * 0.5);
        for y in 0..height {
            let row = y * width;
            for x in 0..width {
                let i = (row + x) as usize;
                let (s, so) = (src[i], soft_px[i]);
                let mut r = apply_contrast(lerp(s.r as f32 / 255.0, so.r as f32 / 255.0, soft_mix), contrast);
                let mut g = apply_contrast(lerp(s.g as f32 / 255.0, so.g as f32 / 255.0, soft_mix), contrast);
                let mut b = apply_contrast(lerp(s.b as f32 / 255.0, so.b as f32 / 255.0, soft_mix), contrast);
                let dx = (x as f32 - cx) / 1.max(width - 1) as f32;
                let dy = (y as f32 - cy) / 1.max(height - 1) as f32;
                let dist = ((dx * dx) + (dy * dy)).sqrt();
                let fm = flash * (-(dist * dist) * sigma_inv).exp();
                r = screen(r, fm * (0.22 + (warmth * 0.10)));
                g = screen(g, fm * 0.18);
                b = screen(b, fm * (0.11 - (warmth * 0.02)));
                if warmth > 0.0001 {
                    let wm = warmth * (0.14 + (fm * 0.30));
                    r = lerp(r, 1f32.min(r + 0.22), wm);
                    g = lerp(g, 1f32.min(g + 0.10), wm * 0.75);
                    b = lerp(b, 0f32.max(b - 0.05), wm * 0.70);
                }
                let radial = ((x as f32 - hw).powf(2.0) + (y as f32 - hh).powf(2.0)).sqrt() / max_dist;
                let vm = clamp01(1.0 - (vignette * radial.powf(1.85) * 0.62));
                let noise = ((hash01(x, y, self.seed) * 2.0) - 1.0) * grain * 0.11;
                r = clamp01((r * vm) + (noise * 1.02));
                g = clamp01((g * vm) + noise);
                b = clamp01((b * vm) + (noise * 0.92));
                dst[i] = SkColor::rgba(clamp_to_byte(r * 255.0), clamp_to_byte(g * 255.0), clamp_to_byte(b * 255.0), s.a);
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
