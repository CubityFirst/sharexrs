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
        Info::new("oil_paint", "Oil paint", EffectCategory::Filters, "image-effect/paint-brush")
            .desc("Simulates an oil painting by quantizing color intensity levels.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("radius", "Radius", 1, 6, 3, |e: &mut Self, v| e.radius = v),
            int_slider::<Self>("levels", "Levels", 8, 64, 24, |e: &mut Self, v| e.levels = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
