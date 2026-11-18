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
        #[rustfmt::skip]
        let m = [
            -1.0, 0.0, 0.0, 0.0, 1.0,
            0.0, -1.0, 0.0, 0.0, 1.0,
            0.0, 0.0, -1.0, 0.0, 1.0,
            0.0, 0.0, 0.0, 1.0, 0.0,
        ];
        Ok(apply_color_matrix(source, &m))
    }
}
