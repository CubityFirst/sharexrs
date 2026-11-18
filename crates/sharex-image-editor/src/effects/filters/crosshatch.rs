//! `CrosshatchImageEffect` (`Core/ImageEffects/Filters/CrosshatchImageEffect.cs`).

use crate::effects::prelude::*;

/// `CrosshatchImageEffect`.
#[derive(Clone, Debug)]
pub struct CrosshatchImageEffect {
    pub line_spacing: i32,
    pub line_thickness: f32,
    pub contrast: f32,
    pub layer_count: i32,
}

impl Default for CrosshatchImageEffect {
    fn default() -> Self {
        CrosshatchImageEffect {
            line_spacing: 8,
            line_thickness: 1.2,
            contrast: 120.0,
            layer_count: 4,
        }
    }
}

impl EffectImpl for CrosshatchImageEffect {
    fn info() -> Info {
        Info::new("crosshatch", "Crosshatch", EffectCategory::Filters, "app/hash")
            .desc("Renders the image as a crosshatch pen drawing.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("lineSpacing", "Line spacing", 4, 24, 8, |e: &mut Self, v| e.line_spacing = v),
            float_slider::<Self>("lineThickness", "Line thickness", 0.5, 4.0, 1.2000000476837158, |e: &mut Self, v| e.line_thickness = v),
            float_slider::<Self>("contrast", "Contrast", 50.0, 200.0, 120.0, |e: &mut Self, v| e.contrast = v),
            int_slider::<Self>("layerCount", "Layer count", 1, 6, 4, |e: &mut Self, v| e.layer_count = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
