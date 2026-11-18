//! `OilPaintImageEffect` (`Core/ImageEffects/Filters/OilPaintImageEffect.cs`).

use crate::effects::prelude::*;

/// `OilPaintImageEffect`.
#[derive(Clone, Debug)]
pub struct OilPaintImageEffect {
    pub radius: i32,
    pub levels: i32,
}

impl Default for OilPaintImageEffect {
    fn default() -> Self {
        OilPaintImageEffect {
            radius: 3,
            levels: 24,
        }
    }
}

impl EffectImpl for OilPaintImageEffect {
    fn info() -> Info {
        Info::new(
            "oil_paint",
            "Oil paint",
            EffectCategory::Filters,
            "image-effect/paint-brush",
        )
        .desc("Simulates an oil painting by quantizing color intensity levels.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("radius", "Radius", 1, 6, 3, |e: &mut Self, v| e.radius = v),
            int_slider::<Self>("levels", "Levels", 8, 64, 24, |e: &mut Self, v| {
                e.levels = v
            }),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let radius = self.radius.clamp(1, 6);
        let levels = self.levels.clamp(8, 64);
        let (width, height) = (source.width() as i32, source.height() as i32);
        let (right, bottom) = (width - 1, height - 1);
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        for y in 0..height {
            for x in 0..width {
                let mut count = [0i32; 64];
                let mut sum = [[0i32; 4]; 64];
                for ky in -radius..=radius {
                    let sy = (y + ky).clamp(0, bottom);
                    for kx in -radius..=radius {
                        let sx = (x + kx).clamp(0, right);
                        let c = src[(sy * width + sx) as usize];
                        let mut i = ((c.r as i32 + c.g as i32 + c.b as i32) * levels) / 768;
                        if i >= levels {
                            i = levels - 1;
                        }
                        let i = i as usize;
                        count[i] += 1;
                        sum[i][0] += c.r as i32;
                        sum[i][1] += c.g as i32;
                        sum[i][2] += c.b as i32;
                        sum[i][3] += c.a as i32;
                    }
                }
                let mut best = 0usize;
                for i in 1..levels as usize {
                    if count[i] > count[best] {
                        best = i;
                    }
                }
                let bc = 1.max(count[best]);
                let s = sum[best];
                dst[(y * width + x) as usize] = SkColor::rgba(
                    (s[0] / bc) as u8,
                    (s[1] / bc) as u8,
                    (s[2] / bc) as u8,
                    (s[3] / bc) as u8,
                );
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
