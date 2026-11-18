//! `Rotate90CCWImageEffect` (`Core/ImageEffects/Manipulations/Rotate90CCWImageEffect.cs`).

use crate::effects::prelude::*;

/// `Rotate90CCWImageEffect`.
#[derive(Clone, Debug)]
pub struct Rotate90CCWImageEffect {}

impl Default for Rotate90CCWImageEffect {
    fn default() -> Self {
        Rotate90CCWImageEffect {}
    }
}

impl EffectImpl for Rotate90CCWImageEffect {
    fn info() -> Info {
        Info::new(
            "rotate_90_counter_clockwise",
            "Rotate 90° counter clockwise",
            EffectCategory::Manipulations,
            "app/reset",
        )
        .desc("Rotates the image 90 degrees counter-clockwise.")
        .immediate()
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        Ok(rotate_orthogonal(source, 270))
    }
}
