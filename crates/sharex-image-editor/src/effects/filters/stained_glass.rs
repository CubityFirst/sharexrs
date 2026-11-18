//! `StainedGlassImageEffect` (`Core/ImageEffects/Filters/StainedGlassImageEffect.cs`).

use crate::effects::prelude::*;

/// `StainedGlassImageEffect`.
#[derive(Clone, Debug)]
pub struct StainedGlassImageEffect {
    pub tile_size: i32,
    pub irregularity: f32,
    pub lead_width: f32,
    pub lead_opacity: f32,
    pub color_boost: f32,
    pub seed: i32,
}

impl Default for StainedGlassImageEffect {
    fn default() -> Self {
        StainedGlassImageEffect {
            tile_size: 22,
            irregularity: 55.0,
            lead_width: 1.8,
            lead_opacity: 85.0,
            color_boost: 20.0,
            seed: 1337,
        }
    }
}

impl EffectImpl for StainedGlassImageEffect {
    fn info() -> Info {
        Info::new("stained_glass", "Stained glass", EffectCategory::Filters, "image-effect/hexagon")
            .desc("Creates a stained glass mosaic effect with lead borders.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("tile_size", "Tile size", 6, 120, 22, |e: &mut Self, v| e.tile_size = v),
            float_slider::<Self>("irregularity", "Irregularity", 0.0, 100.0, 55.0, |e: &mut Self, v| e.irregularity = v),
            float_slider::<Self>("lead_width", "Lead width", 0.0, 12.0, 1.7999999523162842, |e: &mut Self, v| e.lead_width = v),
            float_slider::<Self>("lead_opacity", "Lead opacity", 0.0, 100.0, 85.0, |e: &mut Self, v| e.lead_opacity = v),
            float_slider::<Self>("color_boost", "Color boost", 0.0, 100.0, 20.0, |e: &mut Self, v| e.color_boost = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
