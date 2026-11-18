//! `GoldenBorderImageEffect` (`Core/ImageEffects/Filters/GoldenBorderImageEffect.cs`).

use crate::effects::prelude::*;

/// `GoldenBorderImageEffect`.
#[derive(Clone, Debug)]
pub struct GoldenBorderImageEffect {
    pub size: i32,
    pub bevel_strength: f32,
    pub color: SkColor,
    pub inner_border: bool,
    pub outer_border: bool,
}

impl Default for GoldenBorderImageEffect {
    fn default() -> Self {
        GoldenBorderImageEffect {
            size: 25,
            bevel_strength: 60.0,
            color: SkColor::argb(0xFFD4AF37),
            inner_border: true,
            outer_border: true,
        }
    }
}

impl EffectImpl for GoldenBorderImageEffect {
    fn info() -> Info {
        Info::new("golden_border", "Golden border", EffectCategory::Filters, "image-effect/crown")
            .desc("Adds a luxurious picture frame border to the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("size", "Size", 5, 80, 25, |e: &mut Self, v| e.size = v),
            float_slider::<Self>("bevel_strength", "Bevel strength", 0.0, 100.0, 60.0, |e: &mut Self, v| e.bevel_strength = v),
            color_param::<Self>("color", "Color", SkColor::argb(0xFFD4AF37), |e: &mut Self, v| e.color = v),
            bool_param::<Self>("inner_border", "Inner border", true, |e: &mut Self, v| e.inner_border = v),
            bool_param::<Self>("outer_border", "Outer border", true, |e: &mut Self, v| e.outer_border = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
