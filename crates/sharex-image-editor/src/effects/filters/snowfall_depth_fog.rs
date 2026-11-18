//! `SnowfallDepthFogImageEffect` (`Core/ImageEffects/Filters/SnowfallDepthFogImageEffect.cs`).

use crate::effects::prelude::*;

/// `SnowfallDepthFogImageEffect`.
#[derive(Clone, Debug)]
pub struct SnowfallDepthFogImageEffect {
    pub snow_amount: f32,
    pub flake_size: f32,
    pub wind: f32,
    pub fog_amount: f32,
    pub fog_height: f32,
}

impl Default for SnowfallDepthFogImageEffect {
    fn default() -> Self {
        SnowfallDepthFogImageEffect {
            snow_amount: 55.0,
            flake_size: 5.0,
            wind: 20.0,
            fog_amount: 40.0,
            fog_height: 65.0,
        }
    }
}

impl EffectImpl for SnowfallDepthFogImageEffect {
    fn info() -> Info {
        Info::new("snowfall_depth_fog", "Snowfall + depth fog", EffectCategory::Filters, "image-effect/cloud-snow")
            .desc("Simulates snowfall with depth-based fog.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("snow_amount", "Snow amount", 0.0, 100.0, 55.0, |e: &mut Self, v| e.snow_amount = v),
            float_slider::<Self>("flake_size", "Flake size", 1.0, 14.0, 5.0, |e: &mut Self, v| e.flake_size = v),
            float_slider::<Self>("wind", "Wind", -100.0, 100.0, 20.0, |e: &mut Self, v| e.wind = v),
            float_slider::<Self>("fog_amount", "Fog amount", 0.0, 100.0, 40.0, |e: &mut Self, v| e.fog_amount = v),
            float_slider::<Self>("fog_height", "Fog height", 10.0, 100.0, 65.0, |e: &mut Self, v| e.fog_height = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
