//! `CloudsImageEffect` (`Core/ImageEffects/Filters/CloudsImageEffect.cs`).

use crate::effects::prelude::*;

/// `CloudsImageEffect`.
#[derive(Clone, Debug)]
pub struct CloudsImageEffect {
    pub coverage: f32,
    pub scale: f32,
    pub height_bias: f32,
    pub softness: f32,
    pub sunlight: f32,
}

impl Default for CloudsImageEffect {
    fn default() -> Self {
        CloudsImageEffect {
            coverage: 55.0,
            scale: 72.0,
            height_bias: 62.0,
            softness: 64.0,
            sunlight: 38.0,
        }
    }
}

impl EffectImpl for CloudsImageEffect {
    fn info() -> Info {
        Info::new("clouds", "Clouds", EffectCategory::Filters, "app/cloud")
            .desc("Overlays procedurally generated clouds onto the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("coverage", "Coverage", 0.0, 100.0, 55.0, |e: &mut Self, v| e.coverage = v),
            float_slider::<Self>("scale", "Scale", 0.0, 100.0, 72.0, |e: &mut Self, v| e.scale = v),
            float_slider::<Self>("heightBias", "Height bias", 0.0, 100.0, 62.0, |e: &mut Self, v| e.height_bias = v),
            float_slider::<Self>("softness", "Softness", 0.0, 100.0, 64.0, |e: &mut Self, v| e.softness = v),
            float_slider::<Self>("sunlight", "Sunlight", 0.0, 100.0, 38.0, |e: &mut Self, v| e.sunlight = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
