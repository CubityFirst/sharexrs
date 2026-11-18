//! `LuminanceContourLinesImageEffect` (`Core/ImageEffects/Filters/LuminanceContourLinesImageEffect.cs`).

use crate::effects::prelude::*;

/// `LuminanceContourLinesImageEffect`.
#[derive(Clone, Debug)]
pub struct LuminanceContourLinesImageEffect {
    pub levels: i32,
    pub line_width: f32,
    pub line_strength: f32,
    pub background_strength: f32,
    pub threshold: f32,
    pub invert: bool,
    pub line_color: SkColor,
}

impl Default for LuminanceContourLinesImageEffect {
    fn default() -> Self {
        LuminanceContourLinesImageEffect {
            levels: 12,
            line_width: 6.0,
            line_strength: 65.0,
            background_strength: 20.0,
            threshold: 0.0,
            invert: false,
            line_color: SkColor::argb(0xFF000000),
        }
    }
}

impl EffectImpl for LuminanceContourLinesImageEffect {
    fn info() -> Info {
        Info::new("luminance_contour_lines", "Luminance contour lines", EffectCategory::Filters, "image-effect/contours")
            .desc("Draws contour lines based on luminance quantization levels.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("levels", "Levels", 2, 64, 12, |e: &mut Self, v| e.levels = v),
            float_slider::<Self>("line_width", "Line width", 0.0, 200.0, 6.0, |e: &mut Self, v| e.line_width = v),
            float_slider::<Self>("line_strength", "Line strength", 0.0, 100.0, 65.0, |e: &mut Self, v| e.line_strength = v),
            float_slider::<Self>("background_strength", "Background strength", 0.0, 100.0, 20.0, |e: &mut Self, v| e.background_strength = v),
            float_slider::<Self>("threshold", "Threshold", 0.0, 255.0, 0.0, |e: &mut Self, v| e.threshold = v),
            bool_param::<Self>("invert", "Invert", false, |e: &mut Self, v| e.invert = v),
            color_param::<Self>("line_color", "Line color", SkColor::argb(0xFF000000), |e: &mut Self, v| e.line_color = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
