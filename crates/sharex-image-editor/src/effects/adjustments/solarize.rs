//! `SolarizeImageEffect` (`Core/ImageEffects/Adjustments/SolarizeImageEffect.cs`).

use crate::effects::prelude::*;

/// `SolarizeImageEffect`.
#[derive(Clone, Debug)]
pub struct SolarizeImageEffect {
    pub threshold: i32,
}

impl Default for SolarizeImageEffect {
    fn default() -> Self {
        SolarizeImageEffect {
            threshold: 128,
        }
    }
}

impl EffectImpl for SolarizeImageEffect {
    fn info() -> Info {
        Info::new("solarize", "Solarize", EffectCategory::Adjustments, "app/sun")
            .desc("Applies a solarize effect.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("threshold", "Threshold", 0, 255, 128, |e: &mut Self, v| e.threshold = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
