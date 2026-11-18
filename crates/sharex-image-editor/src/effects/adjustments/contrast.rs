//! `ContrastImageEffect` (`Core/ImageEffects/Adjustments/ContrastImageEffect.cs`).

use crate::effects::prelude::*;

/// `ContrastImageEffect`.
#[derive(Clone, Debug)]
pub struct ContrastImageEffect {
    pub amount: f32,
}

impl Default for ContrastImageEffect {
    fn default() -> Self {
        ContrastImageEffect {
            amount: 0.0,
        }
    }
}

impl EffectImpl for ContrastImageEffect {
    fn info() -> Info {
        Info::new("contrast", "Contrast", EffectCategory::Adjustments, "app/contrast")
            .desc("Adjusts image contrast.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("amount", "Amount", -100.0, 100.0, 0.0, |e: &mut Self, v| e.amount = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let mut scale = (100.0 + self.amount) / 100.0;
        scale = scale * scale;
        let shift = 0.5 * (1.0 - scale);
        #[rustfmt::skip]
        let m = [
            scale, 0.0, 0.0, 0.0, shift,
            0.0, scale, 0.0, 0.0, shift,
            0.0, 0.0, scale, 0.0, shift,
            0.0, 0.0, 0.0, 1.0, 0.0,
        ];
        Ok(apply_color_matrix(source, &m))
    }
}
