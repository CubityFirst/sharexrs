//! `PaletteMapImageEffect` (`Core/ImageEffects/Filters/PaletteMapImageEffect.cs`).

use crate::effects::prelude::*;

/// `PaletteMapImageEffect`.
#[derive(Clone, Debug)]
pub struct PaletteMapImageEffect {
    pub levels: i32,
}

impl Default for PaletteMapImageEffect {
    fn default() -> Self {
        PaletteMapImageEffect {
            levels: 6,
        }
    }
}

impl EffectImpl for PaletteMapImageEffect {
    fn info() -> Info {
        Info::new("palette_map", "Palette map", EffectCategory::Filters, "app/swatches")
            .desc("Reduces the image to a limited color palette by quantization.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("levels", "Color levels", 2, 32, 6, |e: &mut Self, v| e.levels = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
