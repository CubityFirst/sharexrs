//! `PosterizeImageEffect` (`Core/ImageEffects/Adjustments/PosterizeImageEffect.cs`).

use crate::effects::prelude::*;

/// `PosterizeImageEffect`.
#[derive(Clone, Debug)]
pub struct PosterizeImageEffect {
    pub levels: i32,
}

impl Default for PosterizeImageEffect {
    fn default() -> Self {
        PosterizeImageEffect {
            levels: 8,
        }
    }
}

impl EffectImpl for PosterizeImageEffect {
    fn info() -> Info {
        Info::new("posterize", "Posterize", EffectCategory::Adjustments, "app/layers")
            .desc("Reduces the number of colors to create a poster-like effect.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("levels", "Levels", 2, 64, 8, |e: &mut Self, v| e.levels = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
