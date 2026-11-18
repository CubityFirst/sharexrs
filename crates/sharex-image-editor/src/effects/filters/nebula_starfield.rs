//! `NebulaStarfieldImageEffect` (`Core/ImageEffects/Filters/NebulaStarfieldImageEffect.cs`).

use crate::effects::prelude::*;

/// `NebulaStarfieldImageEffect`.
#[derive(Clone, Debug)]
pub struct NebulaStarfieldImageEffect {
    pub intensity: f32,
    pub scale: f32,
    pub hue_shift: f32,
    pub star_density: f32,
    pub star_size: f32,
    pub twinkle: f32,
    pub vignette_strength: f32,
    pub seed: i32,
}

impl Default for NebulaStarfieldImageEffect {
    fn default() -> Self {
        NebulaStarfieldImageEffect {
            intensity: 70.0,
            scale: 80.0,
            hue_shift: -15.0,
            star_density: 55.0,
            star_size: 10.0,
            twinkle: 40.0,
            vignette_strength: 18.0,
            seed: 1337,
        }
    }
}

impl EffectImpl for NebulaStarfieldImageEffect {
    fn info() -> Info {
        Info::new("nebula_starfield", "Nebula starfield", EffectCategory::Filters, "app/sparkles")
            .desc("Generates a procedural nebula and starfield overlay.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("intensity", "Intensity", 0.0, 100.0, 70.0, |e: &mut Self, v| e.intensity = v),
            float_slider::<Self>("scale", "Scale", 0.0, 100.0, 80.0, |e: &mut Self, v| e.scale = v),
            float_slider::<Self>("hue_shift", "Hue shift", -180.0, 180.0, -15.0, |e: &mut Self, v| e.hue_shift = v),
            float_slider::<Self>("star_density", "Star density", 0.0, 100.0, 55.0, |e: &mut Self, v| e.star_density = v),
            float_slider::<Self>("star_size", "Star size", 0.0, 200.0, 10.0, |e: &mut Self, v| e.star_size = v),
            float_slider::<Self>("twinkle", "Twinkle", 0.0, 100.0, 40.0, |e: &mut Self, v| e.twinkle = v),
            float_slider::<Self>("vignette_strength", "Vignette strength", 0.0, 100.0, 18.0, |e: &mut Self, v| e.vignette_strength = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
