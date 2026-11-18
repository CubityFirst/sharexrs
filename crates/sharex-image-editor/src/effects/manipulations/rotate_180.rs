//! `Rotate180ImageEffect` (`Core/ImageEffects/Manipulations/Rotate180ImageEffect.cs`).

use crate::effects::prelude::*;

/// `Rotate180ImageEffect`.
#[derive(Clone, Debug)]
pub struct Rotate180ImageEffect {
}

impl Default for Rotate180ImageEffect {
    fn default() -> Self {
        Rotate180ImageEffect {
        }
    }
}

impl EffectImpl for Rotate180ImageEffect {
    fn info() -> Info {
        Info::new("rotate_180", "Rotate 180°", EffectCategory::Manipulations, "image-effect/rotate-clockwise")
            .desc("Rotates the image by 180 degrees.")
            .immediate()
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        Ok(rotate_orthogonal(source, 180))
    }
}
