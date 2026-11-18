//! `SurfaceBlurImageEffect` (`Core/ImageEffects/Filters/SurfaceBlurImageEffect.cs`).

use crate::effects::prelude::*;

/// `SurfaceBlurImageEffect`.
#[derive(Clone, Debug)]
pub struct SurfaceBlurImageEffect {
    pub radius: i32,
    pub threshold: i32,
}

impl Default for SurfaceBlurImageEffect {
    fn default() -> Self {
        SurfaceBlurImageEffect {
            radius: 3,
            threshold: 24,
        }
    }
}

impl EffectImpl for SurfaceBlurImageEffect {
    fn info() -> Info {
        Info::new(
            "surface_blur",
            "Surface blur",
            EffectCategory::Filters,
            "image-effect/waves",
        )
        .desc("Blurs while preserving edges based on color similarity.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("radius", "Radius", 1, 8, 3, |e: &mut Self, v| e.radius = v),
            int_slider::<Self>("threshold", "Threshold", 1, 100, 24, |e: &mut Self, v| {
                e.threshold = v
            }),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let radius = self.radius.clamp(1, 8);
        let threshold = self.threshold.clamp(1, 100);
        let tsum = threshold * 3;
        let scale = 0.65 / 1.max(tsum) as f32;
        let (width, height) = (source.width() as i32, source.height() as i32);
        let (right, bottom) = (width - 1, height - 1);
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        let mut offsets = Vec::new();
        for ky in -radius..=radius {
            for kx in -radius..=radius {
                if kx != 0 || ky != 0 {
                    offsets.push((kx, ky));
                }
            }
        }
        for y in 0..height {
            for x in 0..width {
                let c0 = src[(y * width + x) as usize];
                let (mut sr, mut sg, mut sb, mut sa, mut sw) =
                    (c0.r as f32, c0.g as f32, c0.b as f32, c0.a as f32, 1f32);
                for &(kx, ky) in &offsets {
                    let sx = (x + kx).clamp(0, right);
                    let sy = (y + ky).clamp(0, bottom);
                    let c = src[(sy * width + sx) as usize];
                    let delta = (c.r as i32 - c0.r as i32).abs()
                        + (c.g as i32 - c0.g as i32).abs()
                        + (c.b as i32 - c0.b as i32).abs();
                    if delta > tsum {
                        continue;
                    }
                    let w = 1.0 - (delta as f32 * scale);
                    sr += c.r as f32 * w;
                    sg += c.g as f32 * w;
                    sb += c.b as f32 * w;
                    sa += c.a as f32 * w;
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
