//! `BrightnessImageEffect` (`Core/ImageEffects/Adjustments/BrightnessImageEffect.cs`).

use crate::effects::prelude::*;

/// `BrightnessImageEffect`.
#[derive(Clone, Debug)]
pub struct BrightnessImageEffect {
    pub amount: f32,
}

impl Default for BrightnessImageEffect {
    fn default() -> Self {
        BrightnessImageEffect {
            amount: 0.0,
        }
    }
}

impl EffectImpl for BrightnessImageEffect {
    fn info() -> Info {
        Info::new("brightness", "Brightness", EffectCategory::Adjustments, "app/sun")
            .desc("Adjusts image brightness.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("amount", "Amount", -100.0, 100.0, 0.0, |e: &mut Self, v| e.amount = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let v = self.amount / 100.0;
        #[rustfmt::skip]
        let m = [
            1.0, 0.0, 0.0, 0.0, v,
            0.0, 1.0, 0.0, 0.0, v,
            0.0, 0.0, 1.0, 0.0, v,
            0.0, 0.0, 0.0, 1.0, 0.0,
        ];
        Ok(apply_color_matrix(source, &m))
    }
}
