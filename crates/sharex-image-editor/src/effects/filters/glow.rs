//! `GlowImageEffect` (`Core/ImageEffects/Filters/GlowImageEffect.cs`).

use crate::effects::prelude::*;

/// `GlowImageEffect`.
#[derive(Clone, Debug)]
pub struct GlowImageEffect {
    pub size: i32,
    pub strength: f32,
    pub color: SkColor,
    pub offset_x: i32,
    pub offset_y: i32,
    pub auto_resize: bool,
}

impl Default for GlowImageEffect {
    fn default() -> Self {
        GlowImageEffect {
            size: 20,
            strength: 80.0,
            color: SkColor::argb(0xFFFFFFFF),
            offset_x: 0,
            offset_y: 0,
            auto_resize: true,
        }
    }
}

impl EffectImpl for GlowImageEffect {
    fn info() -> Info {
        Info::new("glow", "Glow", EffectCategory::Filters, "app/lightbulb")
            .desc("Applies a glowing effect.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("size", "Size", 1, 100, 20, |e: &mut Self, v| e.size = v).snap(false),
            float_slider::<Self>("strength", "Strength", 1.0, 100.0, 80.0, |e: &mut Self, v| e.strength = v).snap(false).format("{}{0:0}%"),
            int_slider::<Self>("offset_x", "Offset X", -100, 100, 0, |e: &mut Self, v| e.offset_x = v).snap(false),
            int_slider::<Self>("offset_y", "Offset Y", -100, 100, 0, |e: &mut Self, v| e.offset_y = v).snap(false),
            color_param::<Self>("color", "Color", SkColor::argb(0xFFFFFFFF), |e: &mut Self, v| e.color = v),
            bool_param::<Self>("auto_resize", "Auto resize", true, |e: &mut Self, v| e.auto_resize = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
