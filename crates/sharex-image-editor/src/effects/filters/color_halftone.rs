//! `ColorHalftoneImageEffect` (`Core/ImageEffects/Filters/ColorHalftoneImageEffect.cs`).

use crate::effects::prelude::*;

/// `ColorHalftoneImageEffect`.
#[derive(Clone, Debug)]
pub struct ColorHalftoneImageEffect {
    pub dot_size: i32,
}

impl Default for ColorHalftoneImageEffect {
    fn default() -> Self {
        ColorHalftoneImageEffect {
            dot_size: 8,
        }
    }
}

impl EffectImpl for ColorHalftoneImageEffect {
    fn info() -> Info {
        Info::new("color_halftone", "Color halftone", EffectCategory::Filters, "image-effect/circle-dot")
            .desc("Simulates CMYK color halftone printing with rotated dot grids.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("size", "Dot size", 4, 30, 8, |e: &mut Self, v| e.dot_size = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
