//! `CyanotypeImageEffect` (`Core/ImageEffects/Filters/CyanotypeImageEffect.cs`).

use crate::effects::prelude::*;

/// `CyanotypeImageEffect`.
#[derive(Clone, Debug)]
pub struct CyanotypeImageEffect {
    pub contrast: f32,
    pub paper_texture: f32,
    pub stain: f32,
    pub grain: f32,
    pub vignette: f32,
    pub seed: i32,
}

impl Default for CyanotypeImageEffect {
    fn default() -> Self {
        CyanotypeImageEffect {
            contrast: 118.0,
            paper_texture: 34.0,
            stain: 28.0,
            grain: 12.0,
            vignette: 20.0,
            seed: 1842,
        }
    }
}

impl EffectImpl for CyanotypeImageEffect {
    fn info() -> Info {
        Info::new("cyanotype", "Cyanotype", EffectCategory::Filters, "app/swatches")
            .desc("Simulates a cyanotype photographic printing process with blue-toned output.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("contrast", "Contrast", 50.0, 200.0, 118.0, |e: &mut Self, v| e.contrast = v),
            float_slider::<Self>("paper_texture", "Paper texture", 0.0, 100.0, 34.0, |e: &mut Self, v| e.paper_texture = v),
            float_slider::<Self>("stain", "Stain", 0.0, 100.0, 28.0, |e: &mut Self, v| e.stain = v),
            float_slider::<Self>("grain", "Grain", 0.0, 100.0, 12.0, |e: &mut Self, v| e.grain = v),
            float_slider::<Self>("vignette", "Vignette", 0.0, 100.0, 20.0, |e: &mut Self, v| e.vignette = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let contrast = clampf(self.contrast, 50.0, 200.0) / 100.0;
        let paper_tex = clampf(self.paper_texture, 0.0, 100.0) / 100.0;
        let stain = clampf(self.stain, 0.0, 100.0) / 100.0;
        let grain = clampf(self.grain, 0.0, 100.0) / 100.0;
        let vignette = clampf(self.vignette, 0.0, 100.0) / 100.0;
        let (width, height) = (source.width() as i32, source.height() as i32);
        let cx = (width - 1) as f32 * 0.5;
        let cy = (height - 1) as f32 * 0.5;
        let max_dist = 1f32.max(((cx * cx) + (cy * cy)).sqrt());
        let paper = SkColor::rgb(241, 236, 220);
        let mid = SkColor::rgb(87, 136, 170);
        let deep = SkColor::rgb(11, 45, 92);
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        let seed = self.seed;
        for y in 0..height {
            let row = y * width;
            for x in 0..width {
                let (xf, yf) = (x as f32, y as f32);
                let s = src[(row + x) as usize];
                let lum = apply_contrast(analog_luminance01(s), contrast);
                let ink = 1.0 - lum;
                let tone = if ink < 0.36 {
                    lerp_color(paper, mid, ink / 0.36)
                } else {
                    lerp_color(mid, deep, (ink - 0.36) / 0.64)
                };
                let pn = (fractal_noise(xf * 0.022, yf * 0.022, 4, 2.1, 0.55, seed) - 0.5) * paper_tex;
                let sn = fractal_noise(xf * 0.006, yf * 0.006, 5, 2.0, 0.5, seed ^ 901);
                let sm = smooth_step(0.56, 0.92, sn) * stain;
                let gn = ((hash01(x, y, seed ^ 2111) * 2.0) - 1.0) * grain * 0.08;
                let radial = ((xf - cx).powf(2.0) + (yf - cy).powf(2.0)).sqrt() / max_dist;
                let vm = clamp01(1.0 - (vignette * radial.powf(1.7) * 0.24));
                let r = clamp01(((tone.r as f32 / 255.0) + (pn * 0.08) + (sm * 0.06) + gn) * vm);
                let g = clamp01(((tone.g as f32 / 255.0) + (pn * 0.10) + (sm * 0.08) + gn) * vm);
                let b = clamp01(((tone.b as f32 / 255.0) + (pn * 0.14) + (sm * 0.10) + gn) * vm);
                dst[(row + x) as usize] =
                    SkColor::rgba(clamp_to_byte(r * 255.0), clamp_to_byte(g * 255.0), clamp_to_byte(b * 255.0), s.a);
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
