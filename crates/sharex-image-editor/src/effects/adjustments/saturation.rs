//! `SaturationImageEffect` (`Core/ImageEffects/Adjustments/SaturationImageEffect.cs`).

use crate::effects::prelude::*;

/// `SaturationImageEffect`.
#[derive(Clone, Debug)]
pub struct SaturationImageEffect {
    pub amount: f32,
}

impl Default for SaturationImageEffect {
    fn default() -> Self {
        SaturationImageEffect {
            amount: 0.0,
        }
    }
}

impl EffectImpl for SaturationImageEffect {
    fn info() -> Info {
        Info::new("saturation", "Saturation", EffectCategory::Adjustments, "image-effect/droplets")
            .desc("Adjusts the color saturation.")
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
