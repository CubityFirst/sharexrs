//! `HolographicFoilShimmerImageEffect` (`Core/ImageEffects/Filters/HolographicFoilShimmerImageEffect.cs`).

use crate::effects::prelude::*;

/// `HolographicFoilShimmerImageEffect`.
#[derive(Clone, Debug)]
pub struct HolographicFoilShimmerImageEffect {
    pub intensity: f32,
    pub scale: f32,
    pub shift: f32,
    pub specular: f32,
    pub grain: f32,
    pub seed: i32,
}

impl Default for HolographicFoilShimmerImageEffect {
    fn default() -> Self {
        HolographicFoilShimmerImageEffect {
            intensity: 65.0,
            scale: 120.0,
            shift: 0.0,
            specular: 45.0,
            grain: 20.0,
            seed: 2026,
        }
    }
}

impl EffectImpl for HolographicFoilShimmerImageEffect {
    fn info() -> Info {
        Info::new("holographic_foil_shimmer", "Holographic foil shimmer", EffectCategory::Filters, "image-effect/rainbow")
            .desc("Overlays a rainbow holographic foil shimmer effect.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("intensity", "Intensity", 0.0, 100.0, 65.0, |e: &mut Self, v| e.intensity = v),
            float_slider::<Self>("scale", "Scale", 40.0, 300.0, 120.0, |e: &mut Self, v| e.scale = v),
            float_slider::<Self>("shift", "Shift", 0.0, 360.0, 0.0, |e: &mut Self, v| e.shift = v),
            float_slider::<Self>("specular", "Specular", 0.0, 100.0, 45.0, |e: &mut Self, v| e.specular = v),
            float_slider::<Self>("grain", "Grain", 0.0, 100.0, 20.0, |e: &mut Self, v| e.grain = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
