//! `SmoothImageEffect` (`Core/ImageEffects/Filters/SmoothImageEffect.cs`).

use crate::effects::prelude::*;

/// `SmoothImageEffect`.
#[derive(Clone, Debug)]
pub struct SmoothImageEffect {
}

impl Default for SmoothImageEffect {
    fn default() -> Self {
        SmoothImageEffect {
        }
    }
}

impl EffectImpl for SmoothImageEffect {
    fn info() -> Info {
        Info::new("smooth", "Smooth", EffectCategory::Filters, "image-effect/waves")
            .desc("Applies a smoothing effect.")
            .immediate()
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
