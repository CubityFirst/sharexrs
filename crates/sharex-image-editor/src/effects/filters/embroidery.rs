//! `EmbroideryImageEffect` (`Core/ImageEffects/Filters/EmbroideryImageEffect.cs`).

use crate::effects::prelude::*;

/// `EmbroideryImageEffect`.
#[derive(Clone, Debug)]
pub struct EmbroideryImageEffect {
    pub stitch_size: i32,
    pub color_count: i32,
    pub show_grid: bool,
    pub fabric_color: SkColor,
}

impl Default for EmbroideryImageEffect {
    fn default() -> Self {
        EmbroideryImageEffect {
            stitch_size: 10,
            color_count: 16,
            show_grid: true,
            fabric_color: SkColor::argb(0xFFF0EBDC),
        }
    }
}

impl EffectImpl for EmbroideryImageEffect {
    fn info() -> Info {
        Info::new("embroidery", "Embroidery", EffectCategory::Filters, "app/stamp")
            .desc("Simulates a cross-stitch embroidery pattern.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("stitch_size", "Stitch Size", 4, 40, 10, |e: &mut Self, v| e.stitch_size = v),
            int_slider::<Self>("color_count", "Color Count", 2, 64, 16, |e: &mut Self, v| e.color_count = v),
            bool_param::<Self>("show_grid", "Show Grid", true, |e: &mut Self, v| e.show_grid = v),
            color_param::<Self>("fabric_color", "Fabric Color", SkColor::argb(0xFFF0EBDC), |e: &mut Self, v| e.fabric_color = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
