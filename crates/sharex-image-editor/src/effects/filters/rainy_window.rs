//! `RainyWindowImageEffect` (`Core/ImageEffects/Filters/RainyWindowImageEffect.cs`).

use crate::effects::prelude::*;

/// `RainyWindowImageEffect`.
#[derive(Clone, Debug)]
pub struct RainyWindowImageEffect {
    pub distortion: f32,
    pub streak_density: f32,
    pub mist_amount: f32,
    pub droplet_amount: f32,
    pub seed: i32,
}

impl Default for RainyWindowImageEffect {
    fn default() -> Self {
        RainyWindowImageEffect {
            distortion: 8.0,
            streak_density: 45.0,
            mist_amount: 25.0,
            droplet_amount: 35.0,
            seed: 8723,
        }
    }
}

impl EffectImpl for RainyWindowImageEffect {
    fn info() -> Info {
        Info::new("rainy_window", "Rainy window", EffectCategory::Filters, "image-effect/cloud-rain")
            .desc("Simulates looking through a rain-streaked window with distortion and mist.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("distortion", "Distortion", 0.0, 30.0, 8.0, |e: &mut Self, v| e.distortion = v),
            float_slider::<Self>("streak_density", "Streak density", 0.0, 100.0, 45.0, |e: &mut Self, v| e.streak_density = v),
            float_slider::<Self>("mist_amount", "Mist amount", 0.0, 100.0, 25.0, |e: &mut Self, v| e.mist_amount = v),
            float_slider::<Self>("droplet_amount", "Droplet amount", 0.0, 100.0, 35.0, |e: &mut Self, v| e.droplet_amount = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
