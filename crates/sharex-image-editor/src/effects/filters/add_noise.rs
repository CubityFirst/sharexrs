//! `AddNoiseImageEffect` (`Core/ImageEffects/Filters/AddNoiseImageEffect.cs`).

use crate::effects::prelude::*;

/// `AddNoiseImageEffect`.
#[derive(Clone, Debug)]
pub struct AddNoiseImageEffect {
    pub amount: f32,
    pub seed: i32,
}

impl Default for AddNoiseImageEffect {
    fn default() -> Self {
        AddNoiseImageEffect {
            amount: 8.0,
            seed: 1337,
        }
    }
}

impl EffectImpl for AddNoiseImageEffect {
    fn info() -> Info {
        Info::new("add_noise", "Add noise", EffectCategory::Filters, "image-effect/spray-can")
            .desc("Adds random noise to the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("amount", "Amount", 0.0, 100.0, 8.0, |e: &mut Self, v| e.amount = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
