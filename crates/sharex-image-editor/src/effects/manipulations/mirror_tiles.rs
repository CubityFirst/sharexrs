//! `MirrorTilesImageEffect` (`Core/ImageEffects/Manipulations/MirrorTilesImageEffect.cs`).

use crate::effects::prelude::*;

/// `MirrorTilesImageEffect`.
#[derive(Clone, Debug)]
pub struct MirrorTilesImageEffect {
    pub columns: i32,
    pub rows: i32,
    pub mirror_alternate_columns: bool,
    pub mirror_alternate_rows: bool,
}

impl Default for MirrorTilesImageEffect {
    fn default() -> Self {
        MirrorTilesImageEffect {
            columns: 3,
            rows: 3,
            mirror_alternate_columns: true,
            mirror_alternate_rows: true,
        }
    }
}

impl EffectImpl for MirrorTilesImageEffect {
    fn info() -> Info {
        Info::new("mirror_tiles", "Mirror tiles", EffectCategory::Manipulations, "image-effect/flip-horizontal")
            .desc("Tiles the image with alternating mirrored reflections.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("columns", "Columns", 1, 16, 3, |e: &mut Self, v| e.columns = v),
            int_slider::<Self>("rows", "Rows", 1, 16, 3, |e: &mut Self, v| e.rows = v),
            bool_param::<Self>("mirror_alternate_columns", "Mirror alternate columns", true, |e: &mut Self, v| e.mirror_alternate_columns = v),
            bool_param::<Self>("mirror_alternate_rows", "Mirror alternate rows", true, |e: &mut Self, v| e.mirror_alternate_rows = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
