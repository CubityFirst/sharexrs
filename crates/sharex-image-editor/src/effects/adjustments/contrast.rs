//! `ContrastImageEffect` (`Core/ImageEffects/Adjustments/ContrastImageEffect.cs`).

use crate::effects::prelude::*;

/// `ContrastImageEffect`.
#[derive(Clone, Debug)]
pub struct ContrastImageEffect {
    pub amount: f32,
}

impl Default for ContrastImageEffect {
    fn default() -> Self {
        ContrastImageEffect {
            amount: 0.0,
        }
    }
}

impl EffectImpl for ContrastImageEffect {
    fn info() -> Info {
        Info::new("contrast", "Contrast", EffectCategory::Adjustments, "app/contrast")
            .desc("Adjusts image contrast.")
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
