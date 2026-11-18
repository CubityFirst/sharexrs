//! `GhostTrailImageEffect` (`Core/ImageEffects/Filters/GhostTrailImageEffect.cs`).

use crate::effects::prelude::*;

/// `GhostTrailImageEffect`.
#[derive(Clone, Debug)]
pub struct GhostTrailImageEffect {
    pub copies: i32,
    pub offset_x: i32,
    pub offset_y: i32,
    pub fade: f32,
}

impl Default for GhostTrailImageEffect {
    fn default() -> Self {
        GhostTrailImageEffect {
            copies: 3,
            offset_x: 15,
            offset_y: 5,
            fade: 50.0,
        }
    }
}

impl EffectImpl for GhostTrailImageEffect {
    fn info() -> Info {
        Info::new("ghost_trail", "Ghost trail", EffectCategory::Filters, "image-effect/ghost")
            .desc("Creates fading ghost copies offset from the original image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("copies", "Copies", 1, 8, 3, |e: &mut Self, v| e.copies = v),
            int_slider::<Self>("offset_x", "Offset X", -50, 50, 15, |e: &mut Self, v| e.offset_x = v),
            int_slider::<Self>("offset_y", "Offset Y", -50, 50, 5, |e: &mut Self, v| e.offset_y = v),
            float_slider::<Self>("fade", "Fade", 10.0, 90.0, 50.0, |e: &mut Self, v| e.fade = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
