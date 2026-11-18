//! `NightVisionImageEffect` (`Core/ImageEffects/Filters/NightVisionImageEffect.cs`).

use crate::effects::prelude::*;

/// `NightVisionImageEffect`.
#[derive(Clone, Debug)]
pub struct NightVisionImageEffect {
    pub intensity: f32,
    pub glow: f32,
    pub noise: f32,
    pub vignette: f32,
    pub scanlines: f32,
}

impl Default for NightVisionImageEffect {
    fn default() -> Self {
        NightVisionImageEffect {
            intensity: 78.0,
            glow: 42.0,
            noise: 18.0,
            vignette: 45.0,
            scanlines: 35.0,
        }
    }
}

impl EffectImpl for NightVisionImageEffect {
    fn info() -> Info {
        Info::new("night_vision", "Night vision", EffectCategory::Filters, "app/scan-eye")
            .desc("Simulates a night vision green phosphor display effect.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("intensity", "Intensity", 0.0, 100.0, 78.0, |e: &mut Self, v| e.intensity = v),
            float_slider::<Self>("glow", "Glow", 0.0, 100.0, 42.0, |e: &mut Self, v| e.glow = v),
            float_slider::<Self>("noise", "Noise", 0.0, 100.0, 18.0, |e: &mut Self, v| e.noise = v),
            float_slider::<Self>("vignette", "Vignette", 0.0, 100.0, 45.0, |e: &mut Self, v| e.vignette = v),
            float_slider::<Self>("scanlines", "Scanlines", 0.0, 100.0, 35.0, |e: &mut Self, v| e.scanlines = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let intensity = clampf(self.intensity, 0.0, 100.0) / 100.0;
        let glow = clampf(self.glow, 0.0, 100.0) / 100.0;
        let noise = clampf(self.noise, 0.0, 100.0) / 100.0;
        let vignette = clampf(self.vignette, 0.0, 100.0) / 100.0;
        let scanlines = clampf(self.scanlines, 0.0, 100.0) / 100.0;
        if intensity <= 0.0001 {
            return Ok(source.clone());
        }
        let (width, height) = (source.width() as i32, source.height() as i32);
        let cx = (width - 1) as f32 * 0.5;
        let cy = (height - 1) as f32 * 0.5;
        let max_dist = 1f32.max(((cx * cx) + (cy * cy)).sqrt());
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        let lum = |c: SkColor| (0.2126 * c.r as f32) + (0.7152 * c.g as f32) + (0.0722 * c.b as f32);
        let p = |x: i32, y: i32| src[(y * width + x) as usize];
        for y in 0..height {
            let row = y * width;
            for x in 0..width {
                let s = src[(row + x) as usize];
                let l = lum(s) / 255.0;
                let mut avg = l;
                avg += lum(p(0.max(x - 1), y)) / 255.0;
                avg += lum(p((width - 1).min(x + 1), y)) / 255.0;
                avg += lum(p(x, 0.max(y - 1))) / 255.0;
                avg += lum(p(x, (height - 1).min(y + 1))) / 255.0;
                avg /= 5.0;
                let mut ph = lerp(l, avg, glow * 0.45) + (l.powf(0.82) * glow * 0.26);
                ph *= 0.72 + (intensity * 0.68);
                let dx = x as f32 - cx;
                let dy = y as f32 - cy;
                let dist = ((dx * dx) + (dy * dy)).sqrt() / max_dist;
                let vm = clamp01(1.0 - (vignette * dist * dist)) * (1.02 - (dist * 0.08));
                let scan = 1.0 - (scanlines * 0.16 * (0.5 + (0.5 * (y as f32 * std::f32::consts::PI).sin())));
                let grain = ((hash01(x, y, 1229) * 2.0) - 1.0) * noise * 0.12;
                let r = clamp01(((ph * 0.12) + (grain * 0.35)) * vm * scan);
                let g = clamp01(((ph * 1.08) + grain) * vm * scan);
                let b = clamp01(((ph * 0.18) + (grain * 0.28)) * vm * scan);
                dst[(row + x) as usize] =
                    SkColor::rgba(clamp_to_byte(r * 255.0), clamp_to_byte(g * 255.0), clamp_to_byte(b * 255.0), s.a);
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
