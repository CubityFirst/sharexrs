//! `HologramScanImageEffect` (`Core/ImageEffects/Filters/HologramScanImageEffect.cs`).

use crate::effects::prelude::*;

/// `HologramScanImageEffect`.
#[derive(Clone, Debug)]
pub struct HologramScanImageEffect {
    pub scanline_strength: f32,
    pub glitch_amount: f32,
    pub chroma_shift: i32,
    pub glow_amount: f32,
    pub seed: i32,
}

impl Default for HologramScanImageEffect {
    fn default() -> Self {
        HologramScanImageEffect {
            scanline_strength: 45.0,
            glitch_amount: 25.0,
            chroma_shift: 2,
            glow_amount: 30.0,
            seed: 6006,
        }
    }
}

impl EffectImpl for HologramScanImageEffect {
    fn info() -> Info {
        Info::new("hologram_scan", "Hologram scan", EffectCategory::Filters, "image-effect/scan-face")
            .desc("Applies a holographic scanline effect with glitch and glow.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("scanline_strength", "Scanline strength", 0.0, 100.0, 45.0, |e: &mut Self, v| e.scanline_strength = v),
            float_slider::<Self>("glitch_amount", "Glitch amount", 0.0, 100.0, 25.0, |e: &mut Self, v| e.glitch_amount = v),
            int_slider::<Self>("chroma_shift", "Chroma shift", 0, 12, 2, |e: &mut Self, v| e.chroma_shift = v),
            float_slider::<Self>("glow_amount", "Glow amount", 0.0, 100.0, 30.0, |e: &mut Self, v| e.glow_amount = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let scanline = clampf(self.scanline_strength, 0.0, 100.0) / 100.0;
        let glitch = clampf(self.glitch_amount, 0.0, 100.0) / 100.0;
        let chroma = self.chroma_shift.clamp(0, 12);
        let glow = clampf(self.glow_amount, 0.0, 100.0) / 100.0;
        if scanline <= 0.0 && glitch <= 0.0 && chroma <= 0 && glow <= 0.0 {
            return Ok(source.clone());
        }
        let (width, height) = (source.width() as i32, source.height() as i32);
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        let seed = self.seed;
        for y in 0..height {
            let row = y * width;
            let band = y / 6;
            let band_noise = hash01(band, 13, seed);
            let band_sign = (hash01(band, 31, seed) * 2.0) - 1.0;
            let max_shift = glitch * 18.0;
            let burst = if band_noise > 0.60 { (band_noise - 0.60) / 0.40 } else { 0.0 };
            let row_shift =
                (burst * max_shift * band_sign) + (((y as f32 * 0.11) + (seed as f32 * 0.001)).sin() * glitch * 2.5);
            let line_sine = 0.5 + (0.5 * (y as f32 * std::f32::consts::PI).sin());
            let factor = 1.0 - (scanline * (0.30 + (0.70 * line_sine)));
            for x in 0..width {
                let fx = x as f32 + row_shift;
                let sr = bilinear_sample(&src, width, height, fx + chroma as f32, y as f32);
                let sg = bilinear_sample(&src, width, height, fx, y as f32);
                let sb = bilinear_sample(&src, width, height, fx - chroma as f32, y as f32);
                let (r, g, b) = (sr.r as f32 / 255.0, sg.g as f32 / 255.0, sb.b as f32 / 255.0);
                let lum = (0.2126 * r) + (0.7152 * g) + (0.0722 * b);
                let mut hr = (lum * 0.18) + (r * 0.20);
                let mut hg = (lum * 0.85) + (g * 0.35);
                let mut hb = (lum * 1.25) + (b * 0.45);
                let pulse = 0.85 + (0.15 * ((y as f32 * 0.45) + (x as f32 * 0.03) + (seed as f32 * 0.0003)).sin());
                hr *= factor * pulse;
                hg *= factor * pulse;
                hb *= factor * pulse;
                let boost = glow * lum.powf(2.2);
                hr += boost * 0.08;
                hg += boost * 0.25;
                hb += boost * 0.45;
                if band_noise > 0.92 && hash01(x, y, seed ^ 99) > 0.97 {
                    hr += 0.35;
                    hg += 0.35;
                    hb += 0.35;
                }
                dst[(row + x) as usize] =
                    SkColor::rgba(clamp_to_byte(hr * 255.0), clamp_to_byte(hg * 255.0), clamp_to_byte(hb * 255.0), sg.a);
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
