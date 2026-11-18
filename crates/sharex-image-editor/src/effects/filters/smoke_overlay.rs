//! `SmokeOverlayImageEffect` (`Core/ImageEffects/Filters/SmokeOverlayImageEffect.cs`).

use crate::effects::prelude::*;

/// `SmokeOverlayImageEffect`.
#[derive(Clone, Debug)]
pub struct SmokeOverlayImageEffect {
    pub density: f32,
    pub scale: f32,
    pub drift: f32,
    pub softness: f32,
    pub contrast: f32,
}

impl Default for SmokeOverlayImageEffect {
    fn default() -> Self {
        SmokeOverlayImageEffect {
            density: 42.0,
            scale: 68.0,
            drift: 18.0,
            softness: 55.0,
            contrast: 48.0,
        }
    }
}

impl EffectImpl for SmokeOverlayImageEffect {
    fn info() -> Info {
        Info::new("smoke_overlay", "Smoke overlay", EffectCategory::Filters, "image-effect/fog")
            .desc("Adds a soft procedural smoke overlay to the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("density", "Density", 0.0, 100.0, 42.0, |e: &mut Self, v| e.density = v),
            float_slider::<Self>("scale", "Scale", 0.0, 100.0, 68.0, |e: &mut Self, v| e.scale = v),
            float_slider::<Self>("drift", "Drift", -100.0, 100.0, 18.0, |e: &mut Self, v| e.drift = v),
            float_slider::<Self>("softness", "Softness", 0.0, 100.0, 55.0, |e: &mut Self, v| e.softness = v),
            float_slider::<Self>("contrast", "Contrast", 0.0, 100.0, 48.0, |e: &mut Self, v| e.contrast = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
