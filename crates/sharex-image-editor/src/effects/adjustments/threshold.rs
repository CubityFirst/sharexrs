//! `ThresholdImageEffect` (`Core/ImageEffects/Adjustments/ThresholdImageEffect.cs`).

use crate::effects::prelude::*;

/// `ThresholdImageEffect`.
#[derive(Clone, Debug)]
pub struct ThresholdImageEffect {
    pub value: i32,
}

impl Default for ThresholdImageEffect {
    fn default() -> Self {
        ThresholdImageEffect {
            value: 128,
        }
    }
}

impl EffectImpl for ThresholdImageEffect {
    fn info() -> Info {
        Info::new("threshold", "Threshold", EffectCategory::Adjustments, "image-effect/binary")
            .desc("Applies a contrast threshold.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("value", "Threshold", 0, 255, 128, |e: &mut Self, v| e.value = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
