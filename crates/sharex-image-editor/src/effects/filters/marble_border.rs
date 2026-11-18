//! `MarbleBorderImageEffect` (`Core/ImageEffects/Filters/MarbleBorderImageEffect.cs`).

use crate::effects::prelude::*;

/// `MarbleBorderImageEffect`.
#[derive(Clone, Debug)]
pub struct MarbleBorderImageEffect {
    pub border_size: i32,
    pub base_color: SkColor,
    pub vein_color: SkColor,
    pub vein_strength: f32,
    pub vein_scale: f32,
    pub inner_bevel: bool,
}

impl Default for MarbleBorderImageEffect {
    fn default() -> Self {
        MarbleBorderImageEffect {
            border_size: 50,
            base_color: SkColor::argb(0xFFEBE6DC),
            vein_color: SkColor::argb(0xFF5A5046),
            vein_strength: 60.0,
            vein_scale: 30.0,
            inner_bevel: true,
        }
    }
}

impl EffectImpl for MarbleBorderImageEffect {
    fn info() -> Info {
        Info::new("marble_border", "Marble border", EffectCategory::Filters, "image-effect/gem")
            .desc("Adds a procedural marble-veined texture border around the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_numeric::<Self>("border_size", "Border size", 2, 300, 50, |e: &mut Self, v| e.border_size = v),
            color_param::<Self>("base_color", "Base color", SkColor::argb(0xFFEBE6DC), |e: &mut Self, v| e.base_color = v),
            color_param::<Self>("vein_color", "Vein color", SkColor::argb(0xFF5A5046), |e: &mut Self, v| e.vein_color = v),
            float_slider::<Self>("vein_strength", "Vein strength", 0.0, 100.0, 60.0, |e: &mut Self, v| e.vein_strength = v),
            float_slider::<Self>("vein_scale", "Vein scale", 5.0, 100.0, 30.0, |e: &mut Self, v| e.vein_scale = v),
            bool_param::<Self>("inner_bevel", "Inner bevel", true, |e: &mut Self, v| e.inner_bevel = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
