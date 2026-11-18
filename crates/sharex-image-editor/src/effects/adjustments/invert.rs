//! `InvertImageEffect` (`Core/ImageEffects/Adjustments/InvertImageEffect.cs`).

use crate::effects::prelude::*;

/// `InvertImageEffect`.
#[derive(Clone, Debug)]
pub struct InvertImageEffect {
}

impl Default for InvertImageEffect {
    fn default() -> Self {
        InvertImageEffect {
        }
    }
}

impl EffectImpl for InvertImageEffect {
    fn info() -> Info {
        Info::new("invert", "Invert", EffectCategory::Adjustments, "image-effect/invert")
            .desc("Inverts image colors.")
            .immediate()
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
