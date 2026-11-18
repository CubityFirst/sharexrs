//! `HueImageEffect` (`Core/ImageEffects/Adjustments/HueImageEffect.cs`).

use crate::effects::prelude::*;

/// `HueImageEffect`.
#[derive(Clone, Debug)]
pub struct HueImageEffect {
    pub amount: f32,
}

impl Default for HueImageEffect {
    fn default() -> Self {
        HueImageEffect { amount: 0.0 }
    }
}

impl EffectImpl for HueImageEffect {
    fn info() -> Info {
        Info::new(
            "hue",
            "Hue",
            EffectCategory::Adjustments,
            "app/screen-color-picker",
        )
        .desc("Adjusts the hue of the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![float_slider::<Self>(
            "amount",
            "Amount",
            -180.0,
            180.0,
            0.0,
            |e: &mut Self, v| e.amount = v,
        )]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let radians = (self.amount as f64 * std::f64::consts::PI / 180.0) as f32;
        let c = (radians as f64).cos() as f32;
        let s = (radians as f64).sin() as f32;
        #[rustfmt::skip]
        let m = [
            0.213 + c * 0.787 - s * 0.213, 0.715 - c * 0.715 - s * 0.715, 0.072 - c * 0.072 + s * 0.928, 0.0, 0.0,
            0.213 - c * 0.213 + s * 0.143, 0.715 + c * 0.285 + s * 0.140, 0.072 - c * 0.072 - s * 0.283, 0.0, 0.0,
            0.213 - c * 0.213 - s * 0.787, 0.715 - c * 0.715 + s * 0.715, 0.072 + c * 0.928 + s * 0.072, 0.0, 0.0,
            0.0, 0.0, 0.0, 1.0, 0.0,
        ];
        Ok(apply_color_matrix(source, &m))
    }
}
