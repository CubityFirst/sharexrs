//! `NightVisionImageEffect` (`Core/ImageEffects/Filters/NightVisionImageEffect.cs`).

use crate::effects::prelude::*;

/// `NightVisionImageEffect`.
#[derive(Clone, Debug)]
pub struct NightVisionImageEffect {
    pub intensity: f32,
    pub glow: f32,
    pub noise: f32,
    pub vignette: f32,
    pub scanlines: f32,
}

impl Default for NightVisionImageEffect {
    fn default() -> Self {
        NightVisionImageEffect {
            intensity: 78.0,
            glow: 42.0,
            noise: 18.0,
            vignette: 45.0,
            scanlines: 35.0,
        }
    }
}

impl EffectImpl for NightVisionImageEffect {
    fn info() -> Info {
        Info::new("night_vision", "Night vision", EffectCategory::Filters, "app/scan-eye")
            .desc("Simulates a night vision green phosphor display effect.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("intensity", "Intensity", 0.0, 100.0, 78.0, |e: &mut Self, v| e.intensity = v),
            float_slider::<Self>("glow", "Glow", 0.0, 100.0, 42.0, |e: &mut Self, v| e.glow = v),
            float_slider::<Self>("noise", "Noise", 0.0, 100.0, 18.0, |e: &mut Self, v| e.noise = v),
            float_slider::<Self>("vignette", "Vignette", 0.0, 100.0, 45.0, |e: &mut Self, v| e.vignette = v),
            float_slider::<Self>("scanlines", "Scanlines", 0.0, 100.0, 35.0, |e: &mut Self, v| e.scanlines = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
