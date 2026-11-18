//! `CrossProcessImageEffect` (`Core/ImageEffects/Adjustments/CrossProcessImageEffect.cs`).

use crate::effects::prelude::*;

/// `CrossProcessImageEffect`.
#[derive(Clone, Debug)]
pub struct CrossProcessImageEffect {
    pub strength: f32,
}

impl Default for CrossProcessImageEffect {
    fn default() -> Self {
        CrossProcessImageEffect {
            strength: 100.0,
        }
    }
}

impl EffectImpl for CrossProcessImageEffect {
    fn info() -> Info {
        Info::new("cross_process", "Cross process", EffectCategory::Adjustments, "app/camera")
            .desc("Simulates film cross-processing with shifted colors and high contrast.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("strength", "Strength", 0.0, 100.0, 100.0, |e: &mut Self, v| e.strength = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let s = clampf(self.strength / 100.0, 0.0, 1.0);
        if s <= 0.0 {
            return Ok(source.clone());
        }
        #[rustfmt::skip]
        let cross: [f32; 20] = [
            1.2, 0.1, -0.1, 0.0, 0.05,
            -0.1, 1.3, 0.0, 0.0, 0.02,
            -0.1, 0.0, 0.8, 0.0, 0.1,
            0.0, 0.0, 0.0, 1.0, 0.0,
        ];
        Ok(apply_color_matrix(source, &blend_with_identity(&cross, s)))
    }
}
