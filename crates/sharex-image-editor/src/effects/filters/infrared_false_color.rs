//! `InfraredFalseColorImageEffect` (`Core/ImageEffects/Filters/InfraredFalseColorImageEffect.cs`).

use crate::effects::prelude::*;

/// `InfraredFalseColorImageEffect`.
#[derive(Clone, Debug)]
pub struct InfraredFalseColorImageEffect {
    pub intensity: f32,
    pub foliage_shift: f32,
    pub sky_darkening: f32,
    pub glow: f32,
    pub contrast: f32,
}

impl Default for InfraredFalseColorImageEffect {
    fn default() -> Self {
        InfraredFalseColorImageEffect {
            intensity: 68.0,
            foliage_shift: 78.0,
            sky_darkening: 44.0,
            glow: 20.0,
            contrast: 118.0,
        }
    }
}

impl EffectImpl for InfraredFalseColorImageEffect {
    fn info() -> Info {
        Info::new("infrared_false_color", "Infrared false color", EffectCategory::Filters, "image-effect/flame")
            .desc("Simulates infrared false color photography.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("intensity", "Intensity", 0.0, 100.0, 68.0, |e: &mut Self, v| e.intensity = v),
            float_slider::<Self>("foliage_shift", "Foliage shift", 0.0, 100.0, 78.0, |e: &mut Self, v| e.foliage_shift = v),
            float_slider::<Self>("sky_darkening", "Sky darkening", 0.0, 100.0, 44.0, |e: &mut Self, v| e.sky_darkening = v),
            float_slider::<Self>("glow", "Glow", 0.0, 100.0, 20.0, |e: &mut Self, v| e.glow = v),
            float_slider::<Self>("contrast", "Contrast", 50.0, 200.0, 118.0, |e: &mut Self, v| e.contrast = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
