//! `FrostedGlassIceEdgesImageEffect` (`Core/ImageEffects/Filters/FrostedGlassIceEdgesImageEffect.cs`).

use crate::effects::prelude::*;

/// `FrostedGlassIceEdgesImageEffect`.
#[derive(Clone, Debug)]
pub struct FrostedGlassIceEdgesImageEffect {
    pub distortion: f32,
    pub blur: f32,
    pub edge_threshold: i32,
    pub ice_strength: f32,
    pub tint: f32,
    pub seed: i32,
}

impl Default for FrostedGlassIceEdgesImageEffect {
    fn default() -> Self {
        FrostedGlassIceEdgesImageEffect {
            distortion: 10.0,
            blur: 35.0,
            edge_threshold: 28,
            ice_strength: 75.0,
            tint: 35.0,
            seed: 1337,
        }
    }
}

impl EffectImpl for FrostedGlassIceEdgesImageEffect {
    fn info() -> Info {
        Info::new("frosted_glass_ice_edges", "Frosted glass + ice edges", EffectCategory::Filters, "image-effect/snowflake")
            .desc("Combines frosted glass distortion with icy edge highlights and cool tinting.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("distortion", "Distortion", 0.0, 30.0, 10.0, |e: &mut Self, v| e.distortion = v),
            float_slider::<Self>("blur", "Blur", 0.0, 100.0, 35.0, |e: &mut Self, v| e.blur = v),
            int_slider::<Self>("edge_threshold", "Edge threshold", 1, 120, 28, |e: &mut Self, v| e.edge_threshold = v),
            float_slider::<Self>("ice_strength", "Ice strength", 0.0, 100.0, 75.0, |e: &mut Self, v| e.ice_strength = v),
            float_slider::<Self>("tint", "Tint", 0.0, 100.0, 35.0, |e: &mut Self, v| e.tint = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
