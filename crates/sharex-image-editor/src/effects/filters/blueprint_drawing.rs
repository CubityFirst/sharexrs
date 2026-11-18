//! `BlueprintDrawingImageEffect` (`Core/ImageEffects/Filters/BlueprintDrawingImageEffect.cs`).

use crate::effects::prelude::*;

/// `BlueprintDrawingImageEffect`.
#[derive(Clone, Debug)]
pub struct BlueprintDrawingImageEffect {
    pub line_strength: f32,
    pub detail: f32,
    pub grid_intensity: f32,
    pub texture: f32,
    pub glow: f32,
}

impl Default for BlueprintDrawingImageEffect {
    fn default() -> Self {
        BlueprintDrawingImageEffect {
            line_strength: 75.0,
            detail: 45.0,
            grid_intensity: 30.0,
            texture: 25.0,
            glow: 35.0,
        }
    }
}

impl EffectImpl for BlueprintDrawingImageEffect {
    fn info() -> Info {
        Info::new("blueprint_drawing", "Blueprint drawing", EffectCategory::Filters, "image-effect/compass")
            .desc("Transforms the image into a blueprint-style drawing.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("line_strength", "Line strength", 0.0, 100.0, 75.0, |e: &mut Self, v| e.line_strength = v),
            float_slider::<Self>("detail", "Detail", 0.0, 100.0, 45.0, |e: &mut Self, v| e.detail = v),
            float_slider::<Self>("grid_intensity", "Grid intensity", 0.0, 100.0, 30.0, |e: &mut Self, v| e.grid_intensity = v),
            float_slider::<Self>("texture", "Texture", 0.0, 100.0, 25.0, |e: &mut Self, v| e.texture = v),
            float_slider::<Self>("glow", "Glow", 0.0, 100.0, 35.0, |e: &mut Self, v| e.glow = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
