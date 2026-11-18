//! `TileRepeatImageEffect` (`Core/ImageEffects/Manipulations/TileRepeatImageEffect.cs`).

use crate::effects::prelude::*;

/// `TileRepeatImageEffect`.
#[derive(Clone, Debug)]
pub struct TileRepeatImageEffect {
    pub columns: i32,
    pub rows: i32,
}

impl Default for TileRepeatImageEffect {
    fn default() -> Self {
        TileRepeatImageEffect {
            columns: 3,
            rows: 3,
        }
    }
}

impl EffectImpl for TileRepeatImageEffect {
    fn info() -> Info {
        Info::new("tile_repeat", "Tile repeat", EffectCategory::Manipulations, "app/split")
            .desc("Tiles the image into a grid of smaller copies.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("columns", "Columns", 2, 10, 3, |e: &mut Self, v| e.columns = v),
            int_slider::<Self>("rows", "Rows", 2, 10, 3, |e: &mut Self, v| e.rows = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
