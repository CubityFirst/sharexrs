//! `ClarityImageEffect` (`Core/ImageEffects/Adjustments/ClarityImageEffect.cs`).

use crate::effects::prelude::*;

/// `ClarityImageEffect`.
#[derive(Clone, Debug)]
pub struct ClarityImageEffect {
    pub amount: f32,
}

impl Default for ClarityImageEffect {
    fn default() -> Self {
        ClarityImageEffect {
            amount: 0.0,
        }
    }
}

impl EffectImpl for ClarityImageEffect {
    fn info() -> Info {
        Info::new("clarity", "Clarity", EffectCategory::Adjustments, "app/eye")
            .desc("Enhances midtone contrast for added depth and punch.")
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
