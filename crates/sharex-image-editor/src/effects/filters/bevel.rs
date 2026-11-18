//! `BevelImageEffect` (`Core/ImageEffects/Filters/BevelImageEffect.cs`).

use crate::effects::prelude::*;

/// `BevelImageEffect`.
#[derive(Clone, Debug)]
pub struct BevelImageEffect {
    pub size: i32,
    pub strength: f32,
    pub light_angle: f32,
    pub highlight_color: SkColor,
    pub shadow_color: SkColor,
}

impl Default for BevelImageEffect {
    fn default() -> Self {
        BevelImageEffect {
            size: 10,
            strength: 70.0,
            light_angle: 225.0,
            highlight_color: SkColor::argb(0xD9FFFFFF),
            shadow_color: SkColor::argb(0xB0000000),
        }
    }
}

impl EffectImpl for BevelImageEffect {
    fn info() -> Info {
        Info::new("bevel", "Bevel", EffectCategory::Filters, "app/layers")
            .desc("Adds a beveled edge effect with highlights and shadows.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("size", "Size", 1, 40, 10, |e: &mut Self, v| e.size = v),
            float_slider::<Self>("strength", "Strength", 0.0, 100.0, 70.0, |e: &mut Self, v| e.strength = v),
            float_slider::<Self>("light_angle", "Light angle", 0.0, 360.0, 225.0, |e: &mut Self, v| e.light_angle = v),
            color_param::<Self>("highlight_color", "Highlight color", SkColor::argb(0xD9FFFFFF), |e: &mut Self, v| e.highlight_color = v),
            color_param::<Self>("shadow_color", "Shadow color", SkColor::argb(0xB0000000), |e: &mut Self, v| e.shadow_color = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
