//! `MeanRemovalImageEffect` (`Core/ImageEffects/Filters/MeanRemovalImageEffect.cs`).

use crate::effects::prelude::*;

/// `MeanRemovalImageEffect`.
#[derive(Clone, Debug)]
pub struct MeanRemovalImageEffect {}

impl Default for MeanRemovalImageEffect {
    fn default() -> Self {
        MeanRemovalImageEffect {}
    }
}

impl EffectImpl for MeanRemovalImageEffect {
    fn info() -> Info {
        Info::new(
            "mean_removal",
            "Mean removal",
            EffectCategory::Filters,
            "image-effect/sigma",
        )
        .desc("Removes the mean value from colors.")
        .immediate()
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        #[rustfmt::skip]
        const K: [f32; 9] = [-1.0, -1.0, -1.0, -1.0, 9.0, -1.0, -1.0, -1.0, -1.0];
        Ok(matrix_convolution_3x3(source, &K, 1.0, 0.0, false))
    }
}
