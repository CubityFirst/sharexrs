//! `ExposureImageEffect` (`Core/ImageEffects/Adjustments/ExposureImageEffect.cs`).

use crate::effects::prelude::*;

/// `ExposureImageEffect`.
#[derive(Clone, Debug)]
pub struct ExposureImageEffect {
    pub amount: f32,
}

impl Default for ExposureImageEffect {
    fn default() -> Self {
        ExposureImageEffect {
            amount: 0.0,
        }
    }
}

impl EffectImpl for ExposureImageEffect {
    fn info() -> Info {
        Info::new("exposure", "Exposure", EffectCategory::Adjustments, "image-effect/aperture")
            .desc("Adjusts the exposure level.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("amount", "Exposure", -100.0, 100.0, 0.0, |e: &mut Self, v| e.amount = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
