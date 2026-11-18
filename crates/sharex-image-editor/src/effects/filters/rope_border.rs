//! `RopeBorderImageEffect` (`Core/ImageEffects/Filters/RopeBorderImageEffect.cs`).

use crate::effects::prelude::*;

/// `RopeBorderImageEffect`.
#[derive(Clone, Debug)]
pub struct RopeBorderImageEffect {
    pub border_size: i32,
    pub rope_color: SkColor,
    pub bg_color: SkColor,
    pub twist_density: f32,
    pub fiber_detail: f32,
}

impl Default for RopeBorderImageEffect {
    fn default() -> Self {
        RopeBorderImageEffect {
            border_size: 40,
            rope_color: SkColor::argb(0xFFB4915A),
            bg_color: SkColor::argb(0xFF3C2D1E),
            twist_density: 50.0,
            fiber_detail: 55.0,
        }
    }
}

impl EffectImpl for RopeBorderImageEffect {
    fn info() -> Info {
        Info::new("rope_border", "Rope border", EffectCategory::Filters, "app/link")
            .desc("Adds a twisted rope/cord border with braided fiber detail and shading.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_numeric::<Self>("border_size", "Border size", 12, 200, 40, |e: &mut Self, v| e.border_size = v),
            color_param::<Self>("rope_color", "Rope color", SkColor::argb(0xFFB4915A), |e: &mut Self, v| e.rope_color = v),
            color_param::<Self>("bg_color", "Background color", SkColor::argb(0xFF3C2D1E), |e: &mut Self, v| e.bg_color = v),
            float_slider::<Self>("twist_density", "Twist density", 10.0, 100.0, 50.0, |e: &mut Self, v| e.twist_density = v),
            float_slider::<Self>("fiber_detail", "Fiber detail", 0.0, 100.0, 55.0, |e: &mut Self, v| e.fiber_detail = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
