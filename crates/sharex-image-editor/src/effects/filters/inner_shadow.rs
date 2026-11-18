//! `InnerShadowImageEffect` (`Core/ImageEffects/Filters/InnerShadowImageEffect.cs`).

use crate::effects::prelude::*;

/// `InnerShadowImageEffect`.
#[derive(Clone, Debug)]
pub struct InnerShadowImageEffect {
    pub opacity: f32,
    pub size: i32,
    pub color: SkColor,
    pub offset_x: i32,
    pub offset_y: i32,
}

impl Default for InnerShadowImageEffect {
    fn default() -> Self {
        InnerShadowImageEffect {
            opacity: 70.0,
            size: 20,
            color: SkColor::argb(0xFF000000),
            offset_x: 0,
            offset_y: 0,
        }
    }
}

impl EffectImpl for InnerShadowImageEffect {
    fn info() -> Info {
        Info::new("inner_shadow", "Inner shadow", EffectCategory::Filters, "image-effect/square-dashed")
            .desc("Adds an inner shadow along the edges of opaque regions.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("opacity", "Opacity", 0.0, 100.0, 70.0, |e: &mut Self, v| e.opacity = v),
            int_slider::<Self>("size", "Size", 0, 300, 20, |e: &mut Self, v| e.size = v),
            color_param::<Self>("color", "Color", SkColor::argb(0xFF000000), |e: &mut Self, v| e.color = v),
            int_numeric::<Self>("offset_x", "Offset X", -300, 300, 0, |e: &mut Self, v| e.offset_x = v),
            int_numeric::<Self>("offset_y", "Offset Y", -300, 300, 0, |e: &mut Self, v| e.offset_y = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
