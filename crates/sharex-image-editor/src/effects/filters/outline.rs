//! `OutlineImageEffect` (`Core/ImageEffects/Filters/OutlineImageEffect.cs`).

use crate::effects::prelude::*;

/// `OutlineImageEffect`.
#[derive(Clone, Debug)]
pub struct OutlineImageEffect {
    pub size: i32,
    pub padding: i32,
    pub outline_only: bool,
    pub color: SkColor,
}

impl Default for OutlineImageEffect {
    fn default() -> Self {
        OutlineImageEffect {
            size: 3,
            padding: 0,
            outline_only: false,
            color: SkColor::argb(0xFF000000),
        }
    }
}

impl EffectImpl for OutlineImageEffect {
    fn info() -> Info {
        Info::new("outline", "Outline", EffectCategory::Filters, "image-effect/square-dashed")
            .desc("Applies an outline effect.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("size", "Size", 1, 50, 3, |e: &mut Self, v| e.size = v).snap(false),
            int_slider::<Self>("padding", "Padding", 0, 100, 0, |e: &mut Self, v| e.padding = v).snap(false),
            bool_param::<Self>("outline_only", "Outline only", false, |e: &mut Self, v| e.outline_only = v),
            color_param::<Self>("color", "Color", SkColor::argb(0xFF000000), |e: &mut Self, v| e.color = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
