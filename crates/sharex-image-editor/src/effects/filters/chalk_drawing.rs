//! `ChalkDrawingImageEffect` (`Core/ImageEffects/Filters/ChalkDrawingImageEffect.cs`).

use crate::effects::prelude::*;

/// `ChalkDrawingImageEffect`.
#[derive(Clone, Debug)]
pub struct ChalkDrawingImageEffect {
    pub detail: f32,
    pub roughness: f32,
    pub chalk_color: SkColor,
    pub board_color: SkColor,
}

impl Default for ChalkDrawingImageEffect {
    fn default() -> Self {
        ChalkDrawingImageEffect {
            detail: 50.0,
            roughness: 40.0,
            chalk_color: SkColor::argb(0xFFFFFFFF),
            board_color: SkColor::argb(0xFF1E3228),
        }
    }
}

impl EffectImpl for ChalkDrawingImageEffect {
    fn info() -> Info {
        Info::new("chalk_drawing", "Chalk Drawing", EffectCategory::Filters, "image-effect/chalk-brush")
            .desc("Simulates a chalk drawing on a dark surface.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("detail", "Detail", 0.0, 100.0, 50.0, |e: &mut Self, v| e.detail = v),
            float_slider::<Self>("roughness", "Roughness", 0.0, 100.0, 40.0, |e: &mut Self, v| e.roughness = v),
            color_param::<Self>("chalk_color", "Chalk Color", SkColor::argb(0xFFFFFFFF), |e: &mut Self, v| e.chalk_color = v),
            color_param::<Self>("board_color", "Board Color", SkColor::argb(0xFF1E3228), |e: &mut Self, v| e.board_color = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
