//! `SobelEdgeImageEffect` (`Core/ImageEffects/Filters/SobelEdgeImageEffect.cs`).

use crate::effects::prelude::*;

/// `SobelEdgeImageEffect`.
#[derive(Clone, Debug)]
pub struct SobelEdgeImageEffect {
    pub strength: f32,
    pub threshold: i32,
}

impl Default for SobelEdgeImageEffect {
    fn default() -> Self {
        SobelEdgeImageEffect {
            strength: 1.2,
            threshold: 20,
        }
    }
}

impl EffectImpl for SobelEdgeImageEffect {
    fn info() -> Info {
        Info::new("sobel_edge", "Sobel edge", EffectCategory::Filters, "app/region")
            .desc("Detects edges using the Sobel operator.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("strength", "Strength", 0.10000000149011612, 5.0, 1.2000000476837158, |e: &mut Self, v| e.strength = v),
            int_slider::<Self>("threshold", "Threshold", 0, 255, 20, |e: &mut Self, v| e.threshold = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let strength = clampf(self.strength, 0.1, 5.0);
        let threshold = self.threshold.clamp(0, 255);
        let (width, height) = (source.width() as i32, source.height() as i32);
        let (right, bottom) = (width - 1, height - 1);
        let src = pixels(source);
        let luma: Vec<i32> = src
            .iter()
            .map(|c| ((c.r as i32 * 77) + (c.g as i32 * 150) + (c.b as i32 * 29)) >> 8)
            .collect();
        let mut dst = vec![SkColor::default(); src.len()];
        let l = |x: i32, y: i32| luma[(y * width + x) as usize];
        for y in 0..height {
            let (y0, y2) = (clamp_int(y - 1, 0, bottom), clamp_int(y + 1, 0, bottom));
            for x in 0..width {
                let (x0, x2) = (clamp_int(x - 1, 0, right), clamp_int(x + 1, 0, right));
                let (p00, p01, p02) = (l(x0, y0), l(x, y0), l(x2, y0));
                let (p10, p12) = (l(x0, y), l(x2, y));
                let (p20, p21, p22) = (l(x0, y2), l(x, y2), l(x2, y2));
                let gx = (-p00 + p02) + (-2 * p10 + 2 * p12) + (-p20 + p22);
                let gy = (-p00 - 2 * p01 - p02) + (p20 + 2 * p21 + p22);
                let m = (((gx * gx) + (gy * gy)) as f32).sqrt() * strength;
                let e = if m >= threshold as f32 { clamp_to_byte(m) } else { 0 };
                let i = (y * width + x) as usize;
                dst[i] = SkColor::rgba(e, e, e, src[i].a);
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
