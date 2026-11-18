//! `SepiaImageEffect` (`Core/ImageEffects/Adjustments/SepiaImageEffect.cs`).

use crate::effects::prelude::*;

/// `SepiaImageEffect`.
#[derive(Clone, Debug)]
pub struct SepiaImageEffect {
    pub strength: f32,
}

impl Default for SepiaImageEffect {
    fn default() -> Self {
        SepiaImageEffect {
            strength: 100.0,
        }
    }
}

impl EffectImpl for SepiaImageEffect {
    fn info() -> Info {
        Info::new("sepia", "Sepia", EffectCategory::Adjustments, "image-effect/coffee")
            .desc("Applies a sepia tone effect.")
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
        let sepia: [f32; 20] = [
            0.393, 0.769, 0.189, 0.0, 0.0,
            0.349, 0.686, 0.168, 0.0, 0.0,
            0.272, 0.534, 0.131, 0.0, 0.0,
            0.0, 0.0, 0.0, 1.0, 0.0,
        ];
        Ok(apply_color_matrix(source, &blend_with_identity(&sepia, s)))
    }
}
