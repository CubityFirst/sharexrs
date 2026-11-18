//! `DisposableCameraImageEffect` (`Core/ImageEffects/Filters/DisposableCameraImageEffect.cs`).

use crate::effects::prelude::*;

/// `DisposableCameraImageEffect`.
#[derive(Clone, Debug)]
pub struct DisposableCameraImageEffect {
    pub flash: f32,
    pub softness: f32,
    pub grain: f32,
    pub vignette: f32,
    pub warmth: f32,
    pub seed: i32,
}

impl Default for DisposableCameraImageEffect {
    fn default() -> Self {
        DisposableCameraImageEffect {
            flash: 55.0,
            softness: 8.0,
            grain: 24.0,
            vignette: 34.0,
            warmth: 42.0,
            seed: 1996,
        }
    }
}

impl EffectImpl for DisposableCameraImageEffect {
    fn info() -> Info {
        Info::new("disposable_camera", "Disposable camera", EffectCategory::Filters, "app/camera")
            .desc("Simulates the look of a disposable film camera with flash, grain, and warmth.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("flash", "Flash", 0.0, 100.0, 55.0, |e: &mut Self, v| e.flash = v),
            float_slider::<Self>("softness", "Softness", 0.0, 24.0, 8.0, |e: &mut Self, v| e.softness = v),
            float_slider::<Self>("grain", "Grain", 0.0, 100.0, 24.0, |e: &mut Self, v| e.grain = v),
            float_slider::<Self>("vignette", "Vignette", 0.0, 100.0, 34.0, |e: &mut Self, v| e.vignette = v),
            float_slider::<Self>("warmth", "Warmth", 0.0, 100.0, 42.0, |e: &mut Self, v| e.warmth = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
