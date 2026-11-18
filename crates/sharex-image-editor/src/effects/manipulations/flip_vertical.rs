//! `FlipVerticalImageEffect` (`Core/ImageEffects/Manipulations/FlipVerticalImageEffect.cs`).

use crate::effects::prelude::*;

/// `FlipVerticalImageEffect`.
#[derive(Clone, Debug)]
pub struct FlipVerticalImageEffect {
}

impl Default for FlipVerticalImageEffect {
    fn default() -> Self {
        FlipVerticalImageEffect {
        }
    }
}

impl EffectImpl for FlipVerticalImageEffect {
    fn info() -> Info {
        Info::new("flip_vertical", "Flip vertical", EffectCategory::Manipulations, "image-effect/flip-vertical")
            .desc("Flips the image vertically.")
            .immediate()
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
