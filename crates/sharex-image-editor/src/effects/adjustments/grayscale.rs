//! `GrayscaleImageEffect` (`Core/ImageEffects/Adjustments/GrayscaleImageEffect.cs`).

use crate::effects::prelude::*;

/// `GrayscaleImageEffect`.
#[derive(Clone, Debug)]
pub struct GrayscaleImageEffect {
    pub strength: f32,
}

impl Default for GrayscaleImageEffect {
    fn default() -> Self {
        GrayscaleImageEffect {
            strength: 100.0,
        }
    }
}

impl EffectImpl for GrayscaleImageEffect {
    fn info() -> Info {
        Info::new("grayscale", "Grayscale", EffectCategory::Adjustments, "image-effect/eclipse")
            .desc("Converts the image to grayscale.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("strength", "Strength", 0.0, 100.0, 100.0, |e: &mut Self, v| e.strength = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
