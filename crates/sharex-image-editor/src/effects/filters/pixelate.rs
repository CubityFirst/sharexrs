//! `PixelateImageEffect` (`Core/ImageEffects/Filters/PixelateImageEffect.cs`).

use crate::effects::prelude::*;

/// `PixelateImageEffect`.
#[derive(Clone, Debug)]
pub struct PixelateImageEffect {
    pub size: i32,
}

impl Default for PixelateImageEffect {
    fn default() -> Self {
        PixelateImageEffect {
            size: 10,
        }
    }
}

impl EffectImpl for PixelateImageEffect {
    fn info() -> Info {
        Info::new("pixelate", "Pixelate", EffectCategory::Filters, "app/split")
            .desc("Pixelates the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("size", "Size", 1, 200, 10, |e: &mut Self, v| e.size = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
