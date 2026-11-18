//! `DoubleExposureImageEffect` (`Core/ImageEffects/Filters/DoubleExposureImageEffect.cs`).

use crate::effects::prelude::*;

/// `DoubleExposureImageEffect`.
#[derive(Clone, Debug)]
pub struct DoubleExposureImageEffect {
    pub blend_amount: f32,
    pub offset_x: i32,
    pub offset_y: i32,
    pub ghost_blur: f32,
    pub highlight_bias: f32,
    pub contrast: f32,
}

impl Default for DoubleExposureImageEffect {
    fn default() -> Self {
        DoubleExposureImageEffect {
            blend_amount: 62.0,
            offset_x: 22,
            offset_y: -14,
            ghost_blur: 8.0,
            highlight_bias: 55.0,
            contrast: 112.0,
        }
    }
}

impl EffectImpl for DoubleExposureImageEffect {
    fn info() -> Info {
        Info::new("double_exposure", "Double exposure", EffectCategory::Filters, "app/images")
            .desc("Blends a blurred ghost of the image over itself to simulate double exposure.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("blend_amount", "Blend amount", 0.0, 100.0, 62.0, |e: &mut Self, v| e.blend_amount = v),
            int_numeric::<Self>("offset_x", "Offset X", -500, 500, 22, |e: &mut Self, v| e.offset_x = v),
            int_numeric::<Self>("offset_y", "Offset Y", -500, 500, -14, |e: &mut Self, v| e.offset_y = v),
            float_slider::<Self>("ghost_blur", "Ghost blur", 0.0, 30.0, 8.0, |e: &mut Self, v| e.ghost_blur = v),
            float_slider::<Self>("highlight_bias", "Highlight bias", 0.0, 100.0, 55.0, |e: &mut Self, v| e.highlight_bias = v),
            float_slider::<Self>("contrast", "Contrast", 50.0, 200.0, 112.0, |e: &mut Self, v| e.contrast = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
