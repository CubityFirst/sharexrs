//! `WatercolorBorderImageEffect` (`Core/ImageEffects/Filters/WatercolorBorderImageEffect.cs`).

use crate::effects::prelude::*;

/// `WatercolorBorderImageEffect`.
#[derive(Clone, Debug)]
pub struct WatercolorBorderImageEffect {
    pub border_size: i32,
    pub wash_color: SkColor,
    pub edge_roughness: f32,
    pub bleed_amount: f32,
    pub opacity: f32,
}

impl Default for WatercolorBorderImageEffect {
    fn default() -> Self {
        WatercolorBorderImageEffect {
            border_size: 60,
            wash_color: SkColor::argb(0xFFB9A082),
            edge_roughness: 65.0,
            bleed_amount: 40.0,
            opacity: 90.0,
        }
    }
}

impl EffectImpl for WatercolorBorderImageEffect {
    fn info() -> Info {
        Info::new("watercolor_border", "Watercolor border", EffectCategory::Filters, "image-effect/droplets")
            .desc("Adds a soft watercolor wash border with organic, painted edges that bleed into the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_numeric::<Self>("border_size", "Border size", 10, 300, 60, |e: &mut Self, v| e.border_size = v),
            color_param::<Self>("wash_color", "Wash color", SkColor::argb(0xFFB9A082), |e: &mut Self, v| e.wash_color = v),
            float_slider::<Self>("edge_roughness", "Edge roughness", 0.0, 100.0, 65.0, |e: &mut Self, v| e.edge_roughness = v),
            float_slider::<Self>("bleed_amount", "Bleed amount", 0.0, 100.0, 40.0, |e: &mut Self, v| e.bleed_amount = v),
            float_slider::<Self>("opacity", "Opacity", 10.0, 100.0, 90.0, |e: &mut Self, v| e.opacity = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
