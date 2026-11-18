//! `DehazeImageEffect` (`Core/ImageEffects/Adjustments/DehazeImageEffect.cs`).

use crate::effects::prelude::*;

/// `DehazeImageEffect`.
#[derive(Clone, Debug)]
pub struct DehazeImageEffect {
    pub amount: f32,
}

impl Default for DehazeImageEffect {
    fn default() -> Self {
        DehazeImageEffect {
            amount: 50.0,
        }
    }
}

impl EffectImpl for DehazeImageEffect {
    fn info() -> Info {
        Info::new("dehaze", "Dehaze", EffectCategory::Adjustments, "image-effect/sunset")
            .desc("Removes or adds atmospheric haze.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("amount", "Amount", -100.0, 100.0, 50.0, |e: &mut Self, v| e.amount = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
