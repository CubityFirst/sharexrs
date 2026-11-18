//! `TileShuffleImageEffect` (`Core/ImageEffects/Manipulations/TileShuffleImageEffect.cs`).

use crate::effects::prelude::*;

/// `TileShuffleImageEffect`.
#[derive(Clone, Debug)]
pub struct TileShuffleImageEffect {
    pub columns: i32,
    pub rows: i32,
    pub seed: i32,
    pub gap: f32,
    pub gap_color: SkColor,
}

impl Default for TileShuffleImageEffect {
    fn default() -> Self {
        TileShuffleImageEffect {
            columns: 4,
            rows: 4,
            seed: 42,
            gap: 0.0,
            gap_color: SkColor::argb(0xFF000000),
        }
    }
}

impl EffectImpl for TileShuffleImageEffect {
    fn info() -> Info {
        Info::new("tile_shuffle", "Tile Shuffle", EffectCategory::Manipulations, "image-effect/shuffle")
            .desc("Divides the image into a grid and randomly rearranges the tiles.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("columns", "Columns", 2, 20, 4, |e: &mut Self, v| e.columns = v),
            int_slider::<Self>("rows", "Rows", 2, 20, 4, |e: &mut Self, v| e.rows = v),
            float_slider::<Self>("gap", "Gap", 0.0, 10.0, 0.0, |e: &mut Self, v| e.gap = v),
            color_param::<Self>("gap_color", "Gap Color", SkColor::argb(0xFF000000), |e: &mut Self, v| e.gap_color = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
