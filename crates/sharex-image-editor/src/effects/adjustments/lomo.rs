//! `LomoImageEffect` (`Core/ImageEffects/Adjustments/LomoImageEffect.cs`).

use crate::effects::prelude::*;

/// `LomoImageEffect`.
#[derive(Clone, Debug)]
pub struct LomoImageEffect {
    pub strength: f32,
}

impl Default for LomoImageEffect {
    fn default() -> Self {
        LomoImageEffect {
            strength: 100.0,
        }
    }
}

impl EffectImpl for LomoImageEffect {
    fn info() -> Info {
        Info::new("lomo", "Lomo", EffectCategory::Adjustments, "image-effect/aperture")
            .desc("Applies a lomography look with high contrast, warm tones, and vignette.")
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
