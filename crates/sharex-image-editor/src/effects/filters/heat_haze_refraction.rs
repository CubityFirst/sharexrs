//! `HeatHazeRefractionImageEffect` (`Core/ImageEffects/Filters/HeatHazeRefractionImageEffect.cs`).

use crate::effects::prelude::*;

/// `HeatHazeRefractionImageEffect`.
#[derive(Clone, Debug)]
pub struct HeatHazeRefractionImageEffect {
    pub strength: f32,
    pub frequency: f32,
    pub blur_radius: f32,
    pub offset: f32,
    pub luminance_influence: f32,
    pub seed: i32,
}

impl Default for HeatHazeRefractionImageEffect {
    fn default() -> Self {
        HeatHazeRefractionImageEffect {
            strength: 45.0,
            frequency: 40.0,
            blur_radius: 10.0,
            offset: 6.0,
            luminance_influence: 55.0,
            seed: 2026,
        }
    }
}

impl EffectImpl for HeatHazeRefractionImageEffect {
    fn info() -> Info {
        Info::new("heat_haze_refraction", "Heat haze refraction", EffectCategory::Filters, "image-effect/thermometer-sun")
            .desc("Simulates heat haze refraction with wavy distortion.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("strength", "Strength", 0.0, 100.0, 45.0, |e: &mut Self, v| e.strength = v),
            float_slider::<Self>("frequency", "Frequency", 0.0, 100.0, 40.0, |e: &mut Self, v| e.frequency = v),
            float_slider::<Self>("blur_radius", "Blur radius", 0.0, 200.0, 10.0, |e: &mut Self, v| e.blur_radius = v),
            float_slider::<Self>("offset", "Offset", -200.0, 200.0, 6.0, |e: &mut Self, v| e.offset = v),
            float_slider::<Self>("luminance_influence", "Luminance influence", 0.0, 100.0, 55.0, |e: &mut Self, v| e.luminance_influence = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
