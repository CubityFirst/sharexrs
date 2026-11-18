//! `NeonEdgeGlowImageEffect` (`Core/ImageEffects/Filters/NeonEdgeGlowImageEffect.cs`).

use crate::effects::prelude::*;

/// `NeonEdgeGlowImageEffect`.
#[derive(Clone, Debug)]
pub struct NeonEdgeGlowImageEffect {
    pub edge_strength: f32,
    pub threshold: i32,
    pub glow_radius: f32,
    pub glow_intensity: f32,
    pub base_dim: f32,
    pub neon_color: SkColor,
}

impl Default for NeonEdgeGlowImageEffect {
    fn default() -> Self {
        NeonEdgeGlowImageEffect {
            edge_strength: 2.2,
            threshold: 36,
            glow_radius: 8.0,
            glow_intensity: 120.0,
            base_dim: 30.0,
            neon_color: SkColor::argb(0xFF00F0FF),
        }
    }
}

impl EffectImpl for NeonEdgeGlowImageEffect {
    fn info() -> Info {
        Info::new("neon_edge_glow", "Neon edge glow", EffectCategory::Filters, "app/zap")
            .desc("Detects edges and applies a neon glow effect.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("edge_strength", "Edge strength", 0.1, 6.0, 2.2, |e: &mut Self, v| e.edge_strength = v),
            int_slider::<Self>("threshold", "Threshold", 0, 255, 36, |e: &mut Self, v| e.threshold = v),
            float_slider::<Self>("glow_radius", "Glow radius", 0.0, 40.0, 8.0, |e: &mut Self, v| e.glow_radius = v),
            float_slider::<Self>("glow_intensity", "Glow intensity", 0.0, 250.0, 120.0, |e: &mut Self, v| e.glow_intensity = v),
            float_slider::<Self>("base_dim", "Base dim", 0.0, 100.0, 30.0, |e: &mut Self, v| e.base_dim = v),
            color_param::<Self>("neon_color", "Neon color", SkColor::argb(0xFF00F0FF), |e: &mut Self, v| e.neon_color = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
