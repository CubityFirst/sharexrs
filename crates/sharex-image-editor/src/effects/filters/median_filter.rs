//! `MedianFilterImageEffect` (`Core/ImageEffects/Filters/MedianFilterImageEffect.cs`).

use crate::effects::prelude::*;

/// `MedianFilterImageEffect`.
#[derive(Clone, Debug)]
pub struct MedianFilterImageEffect {
    pub radius: i32,
}

impl Default for MedianFilterImageEffect {
    fn default() -> Self {
        MedianFilterImageEffect { radius: 1 }
    }
}

impl EffectImpl for MedianFilterImageEffect {
    fn info() -> Info {
        Info::new(
            "median_filter",
            "Median filter",
            EffectCategory::Filters,
            "app/filter",
        )
        .desc("Applies a median filter to reduce noise while preserving edges.")
    }

    fn params() -> Vec<ParamDef> {
        vec![int_slider::<Self>(
            "radius",
            "Radius",
            1,
            5,
            1,
            |e: &mut Self, v| e.radius = v,
        )]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let radius = self.radius.clamp(1, 5);
        let (width, height) = (source.width() as i32, source.height() as i32);
        let (right, bottom) = (width - 1, height - 1);
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        let n = ((radius * 2 + 1) * (radius * 2 + 1)) as usize;
        let (mut rv, mut gv, mut bv, mut av) =
            (vec![0u8; n], vec![0u8; n], vec![0u8; n], vec![0u8; n]);
        for y in 0..height {
            for x in 0..width {
                let mut count = 0;
                for ky in -radius..=radius {
                    let sy = (y + ky).clamp(0, bottom);
                    for kx in -radius..=radius {
                        let sx = (x + kx).clamp(0, right);
                        let c = src[(sy * width + sx) as usize];
                        rv[count] = c.r;
                        gv[count] = c.g;
                        bv[count] = c.b;
                        av[count] = c.a;
                        count += 1;
                    }
                }
                rv[..count].sort_unstable();
                gv[..count].sort_unstable();
                bv[..count].sort_unstable();
                av[..count].sort_unstable();
                let mid = count >> 1;
                dst[(y * width + x) as usize] = SkColor::rgba(rv[mid], gv[mid], bv[mid], av[mid]);
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
