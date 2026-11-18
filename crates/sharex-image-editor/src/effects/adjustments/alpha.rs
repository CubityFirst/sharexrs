//! `AlphaImageEffect` (`Core/ImageEffects/Adjustments/AlphaImageEffect.cs`).

use crate::effects::prelude::*;

/// `AlphaImageEffect`.
#[derive(Clone, Debug)]
pub struct AlphaImageEffect {
    pub amount: f32,
}

impl Default for AlphaImageEffect {
    fn default() -> Self {
        AlphaImageEffect {
            amount: 100.0,
        }
    }
}

impl EffectImpl for AlphaImageEffect {
    fn info() -> Info {
        Info::new("alpha", "Alpha", EffectCategory::Adjustments, "image-effect/droplet")
            .desc("Adjusts the alpha transparency.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("amount", "Alpha", 0.0, 100.0, 100.0, |e: &mut Self, v| e.amount = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
