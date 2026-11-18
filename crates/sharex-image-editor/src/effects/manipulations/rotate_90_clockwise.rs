//! `Rotate90CWImageEffect` (`Core/ImageEffects/Manipulations/Rotate90CWImageEffect.cs`).

use crate::effects::prelude::*;

/// `Rotate90CWImageEffect`.
#[derive(Clone, Debug)]
pub struct Rotate90CWImageEffect {}

impl Default for Rotate90CWImageEffect {
    fn default() -> Self {
        Rotate90CWImageEffect {}
    }
}

impl EffectImpl for Rotate90CWImageEffect {
    fn info() -> Info {
        Info::new(
            "rotate_90_clockwise",
            "Rotate 90° clockwise",
            EffectCategory::Manipulations,
            "image-effect/rotate-clockwise",
        )
        .desc("Rotates the image 90 degrees clockwise.")
        .immediate()
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        Ok(rotate_orthogonal(source, 90))
    }
}
