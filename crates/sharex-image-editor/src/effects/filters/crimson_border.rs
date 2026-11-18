//! `CrimsonBorderImageEffect` (`Core/ImageEffects/Filters/CrimsonBorderImageEffect.cs`).

use crate::effects::prelude::*;

/// `CrimsonBorderImageEffect`.
#[derive(Clone, Debug)]
pub struct CrimsonBorderImageEffect {
    pub size: i32,
    pub color: SkColor,
    pub glow_strength: f32,
    pub inner_line: bool,
    pub outer_line: bool,
}

impl Default for CrimsonBorderImageEffect {
    fn default() -> Self {
        CrimsonBorderImageEffect {
            size: 12,
            color: SkColor::argb(0xFF990012),
            glow_strength: 50.0,
            inner_line: true,
            outer_line: true,
        }
    }
}

impl EffectImpl for CrimsonBorderImageEffect {
    fn info() -> Info {
        Info::new("crimson_border", "Crimson border", EffectCategory::Filters, "image-effect/sword")
            .desc("Adds a rich crimson/dark red border with an inner glow effect.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("size", "Size", 2, 60, 12, |e: &mut Self, v| e.size = v),
            color_param::<Self>("color", "Color", SkColor::argb(0xFF990012), |e: &mut Self, v| e.color = v),
            float_slider::<Self>("glow_strength", "Glow strength", 0.0, 100.0, 50.0, |e: &mut Self, v| e.glow_strength = v),
            bool_param::<Self>("inner_line", "Inner line", true, |e: &mut Self, v| e.inner_line = v),
            bool_param::<Self>("outer_line", "Outer line", true, |e: &mut Self, v| e.outer_line = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
