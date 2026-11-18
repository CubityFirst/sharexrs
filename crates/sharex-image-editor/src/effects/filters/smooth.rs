//! `SmoothImageEffect` (`Core/ImageEffects/Filters/SmoothImageEffect.cs`).

use crate::effects::prelude::*;

/// `SmoothImageEffect`.
#[derive(Clone, Debug)]
pub struct SmoothImageEffect {}

impl Default for SmoothImageEffect {
    fn default() -> Self {
        SmoothImageEffect {}
    }
}

impl EffectImpl for SmoothImageEffect {
    fn info() -> Info {
        Info::new(
            "smooth",
            "Smooth",
            EffectCategory::Filters,
            "image-effect/waves",
        )
        .desc("Applies a smoothing effect.")
        .immediate()
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        const N: f32 = 1.0 / 9.0;
        Ok(matrix_convolution_3x3(source, &[N; 9], 1.0, 0.0, false))
    }
}
