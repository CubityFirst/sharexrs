//! `MosaicTileBorderImageEffect` (`Core/ImageEffects/Filters/MosaicTileBorderImageEffect.cs`).

use crate::effects::prelude::*;

/// `MosaicTileBorderImageEffect`.
#[derive(Clone, Debug)]
pub struct MosaicTileBorderImageEffect {
    pub border_size: i32,
    pub tile_size: f32,
    pub grout_color: SkColor,
    pub grout_width: f32,
    pub color_variety: f32,
    pub saturation: f32,
}

impl Default for MosaicTileBorderImageEffect {
    fn default() -> Self {
        MosaicTileBorderImageEffect {
            border_size: 48,
            tile_size: 14.0,
            grout_color: SkColor::argb(0xFFD2CDC3),
            grout_width: 2.0,
            color_variety: 70.0,
            saturation: 85.0,
        }
    }
}

impl EffectImpl for MosaicTileBorderImageEffect {
    fn info() -> Info {
        Info::new("mosaic_tile_border", "Mosaic tile border", EffectCategory::Filters, "app/grid")
            .desc("Adds a colorful mosaic tile border with individual square tiles, grout lines, and subtle per-tile variation.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_numeric::<Self>("border_size", "Border size", 12, 300, 48, |e: &mut Self, v| e.border_size = v),
            float_slider::<Self>("tile_size", "Tile size", 4.0, 60.0, 14.0, |e: &mut Self, v| e.tile_size = v),
            color_param::<Self>("grout_color", "Grout color", SkColor::argb(0xFFD2CDC3), |e: &mut Self, v| e.grout_color = v),
            float_slider::<Self>("grout_width", "Grout width", 0.5, 8.0, 2.0, |e: &mut Self, v| e.grout_width = v),
            float_slider::<Self>("color_variety", "Color variety", 0.0, 100.0, 70.0, |e: &mut Self, v| e.color_variety = v),
            float_slider::<Self>("saturation", "Saturation", 0.0, 100.0, 85.0, |e: &mut Self, v| e.saturation = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
