//! `FlipHorizontalImageEffect` (`Core/ImageEffects/Manipulations/FlipHorizontalImageEffect.cs`).

use crate::effects::prelude::*;

/// `FlipHorizontalImageEffect`.
#[derive(Clone, Debug)]
pub struct FlipHorizontalImageEffect {
}

impl Default for FlipHorizontalImageEffect {
    fn default() -> Self {
        FlipHorizontalImageEffect {
        }
    }
}

impl EffectImpl for FlipHorizontalImageEffect {
    fn info() -> Info {
        Info::new("flip_horizontal", "Flip horizontal", EffectCategory::Manipulations, "image-effect/flip-horizontal")
            .desc("Flips the image horizontally.")
            .immediate()
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
