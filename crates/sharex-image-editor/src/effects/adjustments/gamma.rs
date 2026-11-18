//! `GammaImageEffect` (`Core/ImageEffects/Adjustments/GammaImageEffect.cs`).

use crate::effects::prelude::*;

/// `GammaImageEffect`.
#[derive(Clone, Debug)]
pub struct GammaImageEffect {
    pub amount: f32,
}

impl Default for GammaImageEffect {
    fn default() -> Self {
        GammaImageEffect {
            amount: 1.0,
        }
    }
}

impl EffectImpl for GammaImageEffect {
    fn info() -> Info {
        Info::new("gamma", "Gamma", EffectCategory::Adjustments, "image-effect/gauge")
            .desc("Adjusts the gamma level.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("amount", "Amount", 0.1, 5.0, 1.0, |e: &mut Self, v| e.amount = v).tick(0.1).snap(false).format("{}{0:0.0}"),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
