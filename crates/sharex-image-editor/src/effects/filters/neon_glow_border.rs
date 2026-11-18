//! `NeonGlowBorderImageEffect` (`Core/ImageEffects/Filters/NeonGlowBorderImageEffect.cs`).

use crate::effects::prelude::*;

/// `NeonGlowBorderImageEffect`.
#[derive(Clone, Debug)]
pub struct NeonGlowBorderImageEffect {
    pub border_size: i32,
    pub neon_color: SkColor,
    pub glow_intensity: f32,
    pub tube_width: f32,
    pub bg_color: SkColor,
    pub double_tube: bool,
}

impl Default for NeonGlowBorderImageEffect {
    fn default() -> Self {
        NeonGlowBorderImageEffect {
            border_size: 50,
            neon_color: SkColor::argb(0xFF00FFC8),
            glow_intensity: 75.0,
            tube_width: 4.0,
            bg_color: SkColor::argb(0xFF0A0A12),
            double_tube: false,
        }
    }
}

impl EffectImpl for NeonGlowBorderImageEffect {
    fn info() -> Info {
        Info::new("neon_glow_border", "Neon glow border", EffectCategory::Filters, "app/zap")
            .desc("Adds a vibrant neon light tube border with a diffused outer glow on a dark background.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_numeric::<Self>("border_size", "Border size", 10, 200, 50, |e: &mut Self, v| e.border_size = v),
            color_param::<Self>("neon_color", "Neon color", SkColor::argb(0xFF00FFC8), |e: &mut Self, v| e.neon_color = v),
            float_slider::<Self>("glow_intensity", "Glow intensity", 10.0, 100.0, 75.0, |e: &mut Self, v| e.glow_intensity = v),
            float_slider::<Self>("tube_width", "Tube width", 1.0, 20.0, 4.0, |e: &mut Self, v| e.tube_width = v),
            color_param::<Self>("bg_color", "Background color", SkColor::argb(0xFF0A0A12), |e: &mut Self, v| e.bg_color = v),
            bool_param::<Self>("double_tube", "Double tube", false, |e: &mut Self, v| e.double_tube = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
