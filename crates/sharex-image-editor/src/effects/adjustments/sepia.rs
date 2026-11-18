//! `SepiaImageEffect` (`Core/ImageEffects/Adjustments/SepiaImageEffect.cs`).

use crate::effects::prelude::*;

/// `SepiaImageEffect`.
#[derive(Clone, Debug)]
pub struct SepiaImageEffect {
    pub strength: f32,
}

impl Default for SepiaImageEffect {
    fn default() -> Self {
        SepiaImageEffect {
            strength: 100.0,
        }
    }
}

impl EffectImpl for SepiaImageEffect {
    fn info() -> Info {
        Info::new("sepia", "Sepia", EffectCategory::Adjustments, "image-effect/coffee")
            .desc("Applies a sepia tone effect.")
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
