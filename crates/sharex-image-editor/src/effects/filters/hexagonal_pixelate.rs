//! `HexagonalPixelateImageEffect` (`Core/ImageEffects/Filters/HexagonalPixelateImageEffect.cs`).

use crate::effects::prelude::*;

/// `HexagonalPixelateImageEffect`.
#[derive(Clone, Debug)]
pub struct HexagonalPixelateImageEffect {
    pub size: i32,
}

impl Default for HexagonalPixelateImageEffect {
    fn default() -> Self {
        HexagonalPixelateImageEffect {
            size: 16,
        }
    }
}

impl EffectImpl for HexagonalPixelateImageEffect {
    fn info() -> Info {
        Info::new("hexagonal_pixelate", "Hexagonal pixelate", EffectCategory::Filters, "image-effect/hexagon")
            .desc("Pixelates the image using hexagonal tiles.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("size", "Size", 4, 100, 16, |e: &mut Self, v| e.size = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
