//! `PolaroidImageEffect` (`Core/ImageEffects/Adjustments/PolaroidImageEffect.cs`).

use crate::effects::prelude::*;

/// `PolaroidImageEffect`.
#[derive(Clone, Debug)]
pub struct PolaroidImageEffect {
}

impl Default for PolaroidImageEffect {
    fn default() -> Self {
        PolaroidImageEffect {
        }
    }
}

impl EffectImpl for PolaroidImageEffect {
    fn info() -> Info {
        Info::new("polaroid", "Polaroid", EffectCategory::Adjustments, "app/camera")
            .desc("Applies a Polaroid effect.")
            .immediate()
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
