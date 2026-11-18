//! `VibranceImageEffect` (`Core/ImageEffects/Adjustments/VibranceImageEffect.cs`).

use crate::effects::prelude::*;

/// `VibranceImageEffect`.
#[derive(Clone, Debug)]
pub struct VibranceImageEffect {
    pub amount: f32,
}

impl Default for VibranceImageEffect {
    fn default() -> Self {
        VibranceImageEffect {
            amount: 25.0,
        }
    }
}

impl EffectImpl for VibranceImageEffect {
    fn info() -> Info {
        Info::new("vibrance", "Vibrance", EffectCategory::Adjustments, "app/sparkles")
            .desc("Adjusts the color vibrance.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("amount", "Amount", -100.0, 100.0, 0.0, |e: &mut Self, v| e.amount = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
