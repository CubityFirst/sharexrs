//! `PointillismImageEffect` (`Core/ImageEffects/Filters/PointillismImageEffect.cs`).

use crate::effects::prelude::*;

/// `PointillismImageEffect`.
#[derive(Clone, Debug)]
pub struct PointillismImageEffect {
    pub dot_size: i32,
    pub density: f32,
    pub jitter: f32,
    pub color_boost: f32,
    pub background_mix: f32,
    pub seed: i32,
}

impl Default for PointillismImageEffect {
    fn default() -> Self {
        PointillismImageEffect {
            dot_size: 7,
            density: 72.0,
            jitter: 65.0,
            color_boost: 20.0,
            background_mix: 20.0,
            seed: 2026,
        }
    }
}

impl EffectImpl for PointillismImageEffect {
    fn info() -> Info {
        Info::new("pointillism", "Pointillism", EffectCategory::Filters, "image-effect/circle-dot")
            .desc("Renders the image as a pointillist painting with colored dots.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("dot_size", "Dot size", 2, 24, 7, |e: &mut Self, v| e.dot_size = v),
            float_slider::<Self>("density", "Density", 10.0, 100.0, 72.0, |e: &mut Self, v| e.density = v),
            float_slider::<Self>("jitter", "Jitter", 0.0, 100.0, 65.0, |e: &mut Self, v| e.jitter = v),
            float_slider::<Self>("color_boost", "Color boost", 0.0, 100.0, 20.0, |e: &mut Self, v| e.color_boost = v),
            float_slider::<Self>("background_mix", "Background mix", 0.0, 100.0, 20.0, |e: &mut Self, v| e.background_mix = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
