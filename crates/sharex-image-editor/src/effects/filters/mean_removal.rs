//! `MeanRemovalImageEffect` (`Core/ImageEffects/Filters/MeanRemovalImageEffect.cs`).

use crate::effects::prelude::*;

/// `MeanRemovalImageEffect`.
#[derive(Clone, Debug)]
pub struct MeanRemovalImageEffect {
}

impl Default for MeanRemovalImageEffect {
    fn default() -> Self {
        MeanRemovalImageEffect {
        }
    }
}

impl EffectImpl for MeanRemovalImageEffect {
    fn info() -> Info {
        Info::new("mean_removal", "Mean removal", EffectCategory::Filters, "image-effect/sigma")
            .desc("Removes the mean value from colors.")
            .immediate()
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
