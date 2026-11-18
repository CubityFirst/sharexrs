//! `EmbossImageEffect` (`Core/ImageEffects/Filters/EmbossImageEffect.cs`).

use crate::effects::prelude::*;

/// `EmbossImageEffect`.
#[derive(Clone, Debug)]
pub struct EmbossImageEffect {}

impl Default for EmbossImageEffect {
    fn default() -> Self {
        EmbossImageEffect {}
    }
}

impl EffectImpl for EmbossImageEffect {
    fn info() -> Info {
        Info::new("emboss", "Emboss", EffectCategory::Filters, "app/stamp")
            .desc("Applies an emboss effect.")
            .immediate()
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        #[rustfmt::skip]
        const K: [f32; 9] = [-1.0, 0.0, -1.0, 0.0, 4.0, 0.0, -1.0, 0.0, -1.0];
        Ok(matrix_convolution_3x3(source, &K, 1.0, 127.0, false))
    }
}
