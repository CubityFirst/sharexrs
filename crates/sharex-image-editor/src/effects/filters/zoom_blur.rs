//! `ZoomBlurImageEffect` (`Core/ImageEffects/Filters/ZoomBlurImageEffect.cs`).

use crate::effects::prelude::*;

/// `ZoomBlurImageEffect`.
#[derive(Clone, Debug)]
pub struct ZoomBlurImageEffect {
    pub strength: f32,
    pub samples: i32,
    pub center_x: f32,
    pub center_y: f32,
}

impl Default for ZoomBlurImageEffect {
    fn default() -> Self {
        ZoomBlurImageEffect {
            strength: 35.0,
            samples: 24,
            center_x: 50.0,
            center_y: 50.0,
        }
    }
}

impl EffectImpl for ZoomBlurImageEffect {
    fn info() -> Info {
        Info::new("zoom_blur", "Zoom blur", EffectCategory::Filters, "app/zoom-in")
            .desc("Applies a radial zoom blur emanating from a center point.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("strength", "Strength", 0.0, 100.0, 35.0, |e: &mut Self, v| e.strength = v),
            int_slider::<Self>("samples", "Samples", 4, 64, 24, |e: &mut Self, v| e.samples = v),
            float_slider::<Self>("center_x", "Center X", 0.0, 100.0, 50.0, |e: &mut Self, v| e.center_x = v),
            float_slider::<Self>("center_y", "Center Y", 0.0, 100.0, 50.0, |e: &mut Self, v| e.center_y = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
