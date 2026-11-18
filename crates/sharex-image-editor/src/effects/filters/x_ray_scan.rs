//! `XRayScanImageEffect` (`Core/ImageEffects/Filters/XRayScanImageEffect.cs`).

use crate::effects::prelude::*;

/// `XRayScanImageEffect`.
#[derive(Clone, Debug)]
pub struct XRayScanImageEffect {
    pub contrast: f32,
    pub glow: f32,
    pub edge_boost: f32,
    pub scanlines: f32,
    pub noise: f32,
}

impl Default for XRayScanImageEffect {
    fn default() -> Self {
        XRayScanImageEffect {
            contrast: 70.0,
            glow: 60.0,
            edge_boost: 68.0,
            scanlines: 40.0,
            noise: 14.0,
        }
    }
}

impl EffectImpl for XRayScanImageEffect {
    fn info() -> Info {
        Info::new("x_ray_scan", "X-ray scan", EffectCategory::Filters, "app/scan-eye")
            .desc("Simulates an X-ray scan with edge detection and glow effects.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("contrast", "Contrast", 0.0, 100.0, 70.0, |e: &mut Self, v| e.contrast = v),
            float_slider::<Self>("glow", "Glow", 0.0, 100.0, 60.0, |e: &mut Self, v| e.glow = v),
            float_slider::<Self>("edge_boost", "Edge boost", 0.0, 100.0, 68.0, |e: &mut Self, v| e.edge_boost = v),
            float_slider::<Self>("scanlines", "Scanlines", 0.0, 100.0, 40.0, |e: &mut Self, v| e.scanlines = v),
            float_slider::<Self>("noise", "Noise", 0.0, 100.0, 14.0, |e: &mut Self, v| e.noise = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let contrast = clampf(self.contrast, 0.0, 100.0) / 100.0;
        let glow = clampf(self.glow, 0.0, 100.0) / 100.0;
        let edge_boost = clampf(self.edge_boost, 0.0, 100.0) / 100.0;
        let scanlines = clampf(self.scanlines, 0.0, 100.0) / 100.0;
        let noise = clampf(self.noise, 0.0, 100.0) / 100.0;
        let (width, height) = (source.width() as i32, source.height() as i32);
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        let lum = |c: SkColor| (0.2126 * c.r as f32) + (0.7152 * c.g as f32) + (0.0722 * c.b as f32);
        let p = |x: i32, y: i32| src[(y * width + x) as usize];
        for y in 0..height {
            for x in 0..width {
                let s = p(x, y);
                let l = lum(s) / 255.0;
                let ll = lum(p(0.max(x - 1), y)) / 255.0;
                let lr = lum(p((width - 1).min(x + 1), y)) / 255.0;
                let lt = lum(p(x, 0.max(y - 1))) / 255.0;
                let lb = lum(p(x, (height - 1).min(y + 1))) / 255.0;
                let edge = clamp01(((lr - ll).abs() + (lb - lt).abs()) * (0.85 + (edge_boost * 1.45)));
                let density = clamp01(1.0 - l).powf(1.85 - (contrast * 1.25));
                let energy = clamp01((density * 0.48) + (edge * 0.92));
                let gm = clamp01((edge * (0.25 + (glow * 0.95))) + (energy * glow * 0.18));
                let scan = 1.0 - (scanlines * 0.16 * (0.5 + (0.5 * (y as f32 * std::f32::consts::PI).sin())));
                let grain = ((hash01(x, y, 8849) * 2.0) - 1.0) * noise * 0.08;
                let r = clamp01(((energy * 0.14) + (gm * 0.08) + (grain * 0.30)) * scan);
                let g = clamp01(((energy * 0.72) + (gm * 0.16) + (grain * 0.40)) * scan);
                let b = clamp01(((energy * 1.00) + (gm * 0.26) + (grain * 0.48)) * scan);
                dst[(y * width + x) as usize] =
                    SkColor::rgba(clamp_to_byte(r * 255.0), clamp_to_byte(g * 255.0), clamp_to_byte(b * 255.0), s.a);
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
