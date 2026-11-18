//! `ColorizeImageEffect` (`Core/ImageEffects/Adjustments/ColorizeImageEffect.cs`).

use crate::effects::prelude::*;

/// `ColorizeImageEffect`.
#[derive(Clone, Debug)]
pub struct ColorizeImageEffect {
    pub color: SkColor,
    pub strength: f32,
}

impl Default for ColorizeImageEffect {
    fn default() -> Self {
        ColorizeImageEffect {
            color: SkColor::argb(0xFFFF0000),
            strength: 50.0,
        }
    }
}

impl EffectImpl for ColorizeImageEffect {
    fn info() -> Info {
        Info::new("colorize", "Colorize", EffectCategory::Adjustments, "app/palette")
            .desc("Colorizes the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            color_param::<Self>("color", "Color", SkColor::argb(0xFFFFA500), |e: &mut Self, v| e.color = v),
            float_slider::<Self>("strength", "Strength", 0.0, 100.0, 50.0, |e: &mut Self, v| e.strength = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
