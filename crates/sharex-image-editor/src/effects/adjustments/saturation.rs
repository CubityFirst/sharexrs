//! `SaturationImageEffect` (`Core/ImageEffects/Adjustments/SaturationImageEffect.cs`).

use crate::effects::prelude::*;

/// `SaturationImageEffect`.
#[derive(Clone, Debug)]
pub struct SaturationImageEffect {
    pub amount: f32,
}

impl Default for SaturationImageEffect {
    fn default() -> Self {
        SaturationImageEffect { amount: 0.0 }
    }
}

impl EffectImpl for SaturationImageEffect {
    fn info() -> Info {
        Info::new(
            "saturation",
            "Saturation",
            EffectCategory::Adjustments,
            "image-effect/droplets",
        )
        .desc("Adjusts the color saturation.")
    }

    fn params() -> Vec<ParamDef> {
        vec![float_slider::<Self>(
            "amount",
            "Amount",
            -100.0,
            100.0,
            0.0,
            |e: &mut Self, v| e.amount = v,
        )]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let x = 1.0 + (self.amount / 100.0);
        let (lum_r, lum_g, lum_b) = (0.3086f32, 0.6094f32, 0.0820f32);
        let inv = 1.0 - x;
        let (r, g, b) = (inv * lum_r, inv * lum_g, inv * lum_b);
        #[rustfmt::skip]
        let m = [
            r + x, g, b, 0.0, 0.0,
            r, g + x, b, 0.0, 0.0,
            r, g, b + x, 0.0, 0.0,
            0.0, 0.0, 0.0, 1.0, 0.0,
        ];
        Ok(apply_color_matrix(source, &m))
    }
}
