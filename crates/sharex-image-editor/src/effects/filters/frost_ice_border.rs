//! `FrostIceBorderImageEffect` (`Core/ImageEffects/Filters/FrostIceBorderImageEffect.cs`).

use crate::effects::prelude::*;

/// `FrostIceBorderImageEffect`.
#[derive(Clone, Debug)]
pub struct FrostIceBorderImageEffect {
    pub border_size: i32,
    pub ice_color: SkColor,
    pub crystallization: f32,
    pub transparency: f32,
    pub sparkle: bool,
}

impl Default for FrostIceBorderImageEffect {
    fn default() -> Self {
        FrostIceBorderImageEffect {
            border_size: 55,
            ice_color: SkColor::argb(0xFFC8E1FF),
            crystallization: 65.0,
            transparency: 25.0,
            sparkle: true,
        }
    }
}

impl EffectImpl for FrostIceBorderImageEffect {
    fn info() -> Info {
        Info::new("frost_ice_border", "Frost ice border", EffectCategory::Filters, "image-effect/snowflake")
            .desc("Adds a frosted ice crystal border with rime buildup and icy blue highlights.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_numeric::<Self>("border_size", "Border size", 10, 300, 55, |e: &mut Self, v| e.border_size = v),
            color_param::<Self>("ice_color", "Ice color", SkColor::argb(0xFFC8E1FF), |e: &mut Self, v| e.ice_color = v),
            float_slider::<Self>("crystallization", "Crystallization", 0.0, 100.0, 65.0, |e: &mut Self, v| e.crystallization = v),
            float_slider::<Self>("transparency", "Transparency", 0.0, 80.0, 25.0, |e: &mut Self, v| e.transparency = v),
            bool_param::<Self>("sparkle", "Ice sparkle", true, |e: &mut Self, v| e.sparkle = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
