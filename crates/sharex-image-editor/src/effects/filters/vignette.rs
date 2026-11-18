//! `VignetteImageEffect` (`Core/ImageEffects/Filters/VignetteImageEffect.cs`).

use crate::effects::prelude::*;

/// `VignetteImageEffect`.
#[derive(Clone, Debug)]
pub struct VignetteImageEffect {
    pub strength: f32,
    pub radius: f32,
}

impl Default for VignetteImageEffect {
    fn default() -> Self {
        VignetteImageEffect {
            strength: 0.5,
            radius: 0.75,
        }
    }
}

impl EffectImpl for VignetteImageEffect {
    fn info() -> Info {
        Info::new("vignette", "Vignette", EffectCategory::Filters, "app/circle-dashed")
            .desc("Applies a vignette effect.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("strength", "Strength", 0.0, 1.0, 0.5, |e: &mut Self, v| e.strength = v).tick(0.05).snap(false).format("{}{0:0.00}"),
            float_slider::<Self>("radius", "Radius", 0.05, 0.999, 0.75, |e: &mut Self, v| e.radius = v).tick(0.01).snap(false).format("{}{0:0.00}"),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
