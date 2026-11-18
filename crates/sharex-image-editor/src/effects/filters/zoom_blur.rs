//! `ZoomBlurImageEffect` (`Core/ImageEffects/Filters/ZoomBlurImageEffect.cs`).

use crate::effects::prelude::*;

/// `ZoomBlurImageEffect`.
#[derive(Clone, Debug)]
pub struct ZoomBlurImageEffect {
    pub strength: f32,
    pub samples: i32,
    pub center_x: f32,
    pub center_y: f32,
}

impl Default for ZoomBlurImageEffect {
    fn default() -> Self {
        ZoomBlurImageEffect {
            strength: 35.0,
            samples: 24,
            center_x: 50.0,
            center_y: 50.0,
        }
    }
}

impl EffectImpl for ZoomBlurImageEffect {
    fn info() -> Info {
        Info::new(
            "zoom_blur",
            "Zoom blur",
            EffectCategory::Filters,
            "app/zoom-in",
        )
        .desc("Applies a radial zoom blur emanating from a center point.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>(
                "strength",
                "Strength",
                0.0,
                100.0,
                35.0,
                |e: &mut Self, v| e.strength = v,
            ),
            int_slider::<Self>("samples", "Samples", 4, 64, 24, |e: &mut Self, v| {
                e.samples = v
            }),
            float_slider::<Self>(
                "center_x",
                "Center X",
                0.0,
                100.0,
                50.0,
                |e: &mut Self, v| e.center_x = v,
            ),
            float_slider::<Self>(
                "center_y",
                "Center Y",
                0.0,
                100.0,
                50.0,
                |e: &mut Self, v| e.center_y = v,
            ),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let strength = clampf(self.strength, 0.0, 100.0) / 100.0;
        let n = self.samples.clamp(4, 64) as usize;
        if strength <= 0.0001 {
            return Ok(source.clone());
        }
        let (width, height) = (source.width() as i32, source.height() as i32);
        let cx = (clampf(self.center_x, 0.0, 100.0) / 100.0) * (width - 1) as f32;
        let cy = (clampf(self.center_y, 0.0, 100.0) / 100.0) * (height - 1) as f32;
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        let inv = if n <= 1 { 1.0 } else { 1.0 / (n - 1) as f32 };
        let factors: Vec<f32> = (0..n)
            .map(|i| 1.0 - ((i as f32 * inv) * strength))
            .collect();
        let weights: Vec<f32> = (0..n).map(|i| 1.0 - ((i as f32 * inv) * 0.6)).collect();
        for y in 0..height {
            for x in 0..width {
                let dx = x as f32 - cx;
                let dy = y as f32 - cy;
                let (mut sr, mut sg, mut sb, mut sa, mut sw) = (0f32, 0f32, 0f32, 0f32, 0f32);
                for i in 0..n {
                    let (f, w) = (factors[i], weights[i]);
                    let p = bilinear_sample(&src, width, height, cx + (dx * f), cy + (dy * f));
                    sr += p.r as f32 * w;
                    sg += p.g as f32 * w;
                    sb += p.b as f32 * w;
                    sa += p.a as f32 * w;
                    sw += w;
                }
                let inv = 1.0 / 0.0001f32.max(sw);
                dst[(y * width + x) as usize] = SkColor::rgba(
                    clamp_to_byte(sr * inv),
                    clamp_to_byte(sg * inv),
                    clamp_to_byte(sb * inv),
                    clamp_to_byte(sa * inv),
                );
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
