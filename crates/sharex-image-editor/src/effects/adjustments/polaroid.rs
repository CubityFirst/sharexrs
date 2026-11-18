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
        #[rustfmt::skip]
        let m = [
            1.438, -0.062, -0.062, 0.0, 0.0,
            -0.122, 1.378, -0.122, 0.0, 0.0,
            -0.016, -0.016, 1.483, 0.0, 0.0,
            0.0, 0.0, 0.0, 1.0, 0.0,
        ];
        Ok(apply_color_matrix(source, &m))
    }
}
