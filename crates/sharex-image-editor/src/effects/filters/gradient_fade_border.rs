//! `GradientFadeBorderImageEffect` (`Core/ImageEffects/Filters/GradientFadeBorderImageEffect.cs`).

use crate::effects::prelude::*;

/// `GradientFadeBorderImageEffect`.
#[derive(Clone, Debug)]
pub struct GradientFadeBorderImageEffect {
    pub border_size: i32,
    pub outer_color: SkColor,
    pub mid_color: SkColor,
    pub inner_color: SkColor,
    pub mid_position: f32,
    pub glow_edge: bool,
}

impl Default for GradientFadeBorderImageEffect {
    fn default() -> Self {
        GradientFadeBorderImageEffect {
            border_size: 50,
            outer_color: SkColor::argb(0xFF0A0A23),
            mid_color: SkColor::argb(0xFF3C1478),
            inner_color: SkColor::argb(0xFFB450C8),
            mid_position: 50.0,
            glow_edge: true,
        }
    }
}

impl EffectImpl for GradientFadeBorderImageEffect {
    fn info() -> Info {
        Info::new("gradient_fade_border", "Gradient fade border", EffectCategory::Filters, "app/blend")
            .desc("Adds a multi-stop gradient border that smoothly fades between configurable colors.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_numeric::<Self>("border_size", "Border size", 10, 300, 50, |e: &mut Self, v| e.border_size = v),
            color_param::<Self>("outer_color", "Outer color", SkColor::argb(0xFF0A0A23), |e: &mut Self, v| e.outer_color = v),
            color_param::<Self>("mid_color", "Mid color", SkColor::argb(0xFF3C1478), |e: &mut Self, v| e.mid_color = v),
            color_param::<Self>("inner_color", "Inner color", SkColor::argb(0xFFB450C8), |e: &mut Self, v| e.inner_color = v),
            float_slider::<Self>("mid_position", "Mid-color position %", 10.0, 90.0, 50.0, |e: &mut Self, v| e.mid_position = v),
            bool_param::<Self>("glow_edge", "Inner glow edge", true, |e: &mut Self, v| e.glow_edge = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
