//! `BloodSplashImageEffect` (`Core/ImageEffects/Filters/BloodSplashImageEffect.cs`).

use crate::effects::prelude::*;

/// `BloodSplashImageEffect`.
#[derive(Clone, Debug)]
pub struct BloodSplashImageEffect {
    pub splash_amount: f32,
    pub drip_length: f32,
    pub spread: f32,
    pub darkness: f32,
    pub wet_shine: f32,
}

impl Default for BloodSplashImageEffect {
    fn default() -> Self {
        BloodSplashImageEffect {
            splash_amount: 52.0,
            drip_length: 48.0,
            spread: 42.0,
            darkness: 58.0,
            wet_shine: 30.0,
        }
    }
}

impl EffectImpl for BloodSplashImageEffect {
    fn info() -> Info {
        Info::new("blood_splash", "Blood splash", EffectCategory::Filters, "image-effect/droplet")
            .desc("Overlays procedural blood splash and drip effects.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("splash_amount", "Splash amount", 0.0, 100.0, 52.0, |e: &mut Self, v| e.splash_amount = v),
            float_slider::<Self>("drip_length", "Drip length", 0.0, 100.0, 48.0, |e: &mut Self, v| e.drip_length = v),
            float_slider::<Self>("spread", "Spread", 0.0, 100.0, 42.0, |e: &mut Self, v| e.spread = v),
            float_slider::<Self>("darkness", "Darkness", 0.0, 100.0, 58.0, |e: &mut Self, v| e.darkness = v),
            float_slider::<Self>("wet_shine", "Wet shine", 0.0, 100.0, 30.0, |e: &mut Self, v| e.wet_shine = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
