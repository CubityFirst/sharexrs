//! `OilSlickInterferenceImageEffect` (`Core/ImageEffects/Filters/OilSlickInterferenceImageEffect.cs`).

use crate::effects::prelude::*;

/// `OilSlickInterferenceImageEffect`.
#[derive(Clone, Debug)]
pub struct OilSlickInterferenceImageEffect {
    pub intensity: f32,
    pub scale: f32,
    pub darkness: f32,
    pub gloss: f32,
    pub shift: f32,
}

impl Default for OilSlickInterferenceImageEffect {
    fn default() -> Self {
        OilSlickInterferenceImageEffect {
            intensity: 58.0,
            scale: 70.0,
            darkness: 45.0,
            gloss: 60.0,
            shift: 0.0,
        }
    }
}

impl EffectImpl for OilSlickInterferenceImageEffect {
    fn info() -> Info {
        Info::new("oil_slick_interference", "Oil slick interference", EffectCategory::Filters, "image-effect/rainbow")
            .desc("Simulates iridescent oil slick interference patterns.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("intensity", "Intensity", 0.0, 100.0, 58.0, |e: &mut Self, v| e.intensity = v),
            float_slider::<Self>("scale", "Scale", 0.0, 100.0, 70.0, |e: &mut Self, v| e.scale = v),
            float_slider::<Self>("darkness", "Darkness", 0.0, 100.0, 45.0, |e: &mut Self, v| e.darkness = v),
            float_slider::<Self>("gloss", "Gloss", 0.0, 100.0, 60.0, |e: &mut Self, v| e.gloss = v),
            float_slider::<Self>("shift", "Shift", 0.0, 360.0, 0.0, |e: &mut Self, v| e.shift = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
