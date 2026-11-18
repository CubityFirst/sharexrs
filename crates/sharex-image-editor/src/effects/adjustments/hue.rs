//! `HueImageEffect` (`Core/ImageEffects/Adjustments/HueImageEffect.cs`).

use crate::effects::prelude::*;

/// `HueImageEffect`.
#[derive(Clone, Debug)]
pub struct HueImageEffect {
    pub amount: f32,
}

impl Default for HueImageEffect {
    fn default() -> Self {
        HueImageEffect {
            amount: 0.0,
        }
    }
}

impl EffectImpl for HueImageEffect {
    fn info() -> Info {
        Info::new("hue", "Hue", EffectCategory::Adjustments, "app/screen-color-picker")
            .desc("Adjusts the hue of the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("amount", "Amount", -180.0, 180.0, 0.0, |e: &mut Self, v| e.amount = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
