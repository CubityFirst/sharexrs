//! `PerlinNoiseImageEffect` (`Core/ImageEffects/Filters/PerlinNoiseImageEffect.cs`).

use crate::effects::prelude::*;

/// `PerlinNoiseImageEffect`.
#[derive(Clone, Debug)]
pub struct PerlinNoiseImageEffect {
    pub scale: f32,
    pub octaves: i32,
    pub persistence: f32,
    pub strength: f32,
    pub monochrome: bool,
}

impl Default for PerlinNoiseImageEffect {
    fn default() -> Self {
        PerlinNoiseImageEffect {
            scale: 64.0,
            octaves: 4,
            persistence: 0.5,
            strength: 50.0,
            monochrome: true,
        }
    }
}

impl EffectImpl for PerlinNoiseImageEffect {
    fn info() -> Info {
        Info::new("perlin_noise", "Perlin noise", EffectCategory::Filters, "image-effect/waves")
            .desc("Generates a Perlin noise pattern and blends it with the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("scale", "Scale", 1.0, 500.0, 64.0, |e: &mut Self, v| e.scale = v).snap(false),
            int_slider::<Self>("octaves", "Octaves", 1, 8, 4, |e: &mut Self, v| e.octaves = v),
            float_slider::<Self>("persistence", "Persistence", 0.0, 1.0, 0.5, |e: &mut Self, v| e.persistence = v).tick(0.01).snap(false).format("{}{0:0.00}"),
            float_slider::<Self>("strength", "Strength", 0.0, 100.0, 50.0, |e: &mut Self, v| e.strength = v).snap(false),
            bool_param::<Self>("monochrome", "Monochrome", true, |e: &mut Self, v| e.monochrome = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
