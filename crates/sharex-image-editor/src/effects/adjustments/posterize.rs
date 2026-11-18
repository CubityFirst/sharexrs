//! `PosterizeImageEffect` (`Core/ImageEffects/Adjustments/PosterizeImageEffect.cs`).

use crate::effects::prelude::*;

/// `PosterizeImageEffect`.
#[derive(Clone, Debug)]
pub struct PosterizeImageEffect {
    pub levels: i32,
}

impl Default for PosterizeImageEffect {
    fn default() -> Self {
        PosterizeImageEffect { levels: 8 }
    }
}

impl EffectImpl for PosterizeImageEffect {
    fn info() -> Info {
        Info::new(
            "posterize",
            "Posterize",
            EffectCategory::Adjustments,
            "app/layers",
        )
        .desc("Reduces the number of colors to create a poster-like effect.")
    }

    fn params() -> Vec<ParamDef> {
        vec![int_slider::<Self>(
            "levels",
            "Levels",
            2,
            64,
            8,
            |e: &mut Self, v| e.levels = v,
        )]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let levels = self.levels.clamp(2, 64);
        let scale = (levels - 1) as f32;
        let q = |v: u8| -> u8 {
            let bucket = (v as f32 * scale / 255.0).round_ties_even();
            let mapped = bucket * 255.0 / scale;
            if mapped <= 0.0 {
                return 0;
            }
            if mapped >= 255.0 {
                return 255;
            }
            mapped.round_ties_even() as u8
        };
        Ok(apply_pixel_operation(source, |c| {
            SkColor::rgba(q(c.r), q(c.g), q(c.b), c.a)
        }))
    }
}
