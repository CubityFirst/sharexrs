//! `TrianglePixelateImageEffect` (`Core/ImageEffects/Filters/TrianglePixelateImageEffect.cs`).

use crate::effects::prelude::*;

/// `TrianglePixelateImageEffect`.
#[derive(Clone, Debug)]
pub struct TrianglePixelateImageEffect {
    pub size: i32,
}

impl Default for TrianglePixelateImageEffect {
    fn default() -> Self {
        TrianglePixelateImageEffect {
            size: 24,
        }
    }
}

impl EffectImpl for TrianglePixelateImageEffect {
    fn info() -> Info {
        Info::new("triangle_pixelate", "Triangle pixelate", EffectCategory::Filters, "image-effect/triangle")
            .desc("Pixelates the image using triangular tiles.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("size", "Size", 8, 100, 24, |e: &mut Self, v| e.size = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
