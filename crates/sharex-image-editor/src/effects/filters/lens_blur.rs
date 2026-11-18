//! `LensBlurImageEffect` (`Core/ImageEffects/Filters/LensBlurImageEffect.cs`).

use crate::effects::prelude::*;

/// `LensBlurImageEffect`.
#[derive(Clone, Debug)]
pub struct LensBlurImageEffect {
    pub radius: i32,
    pub highlight_threshold: f32,
    pub highlight_boost: f32,
}

impl Default for LensBlurImageEffect {
    fn default() -> Self {
        LensBlurImageEffect {
            radius: 8,
            highlight_threshold: 70.0,
            highlight_boost: 85.0,
        }
    }
}

impl EffectImpl for LensBlurImageEffect {
    fn info() -> Info {
        Info::new("lens_blur", "Lens blur (bokeh)", EffectCategory::Filters, "image-effect/aperture")
            .desc("Applies a disc-shaped lens blur with highlight boost.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("radius", "Radius", 1, 15, 8, |e: &mut Self, v| e.radius = v),
            float_slider::<Self>("highlight_threshold", "Highlight threshold", 0.0, 100.0, 70.0, |e: &mut Self, v| e.highlight_threshold = v),
            float_slider::<Self>("highlight_boost", "Highlight boost", 0.0, 200.0, 85.0, |e: &mut Self, v| e.highlight_boost = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
