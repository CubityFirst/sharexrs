//! `DiamondPixelateImageEffect` (`Core/ImageEffects/Filters/DiamondPixelateImageEffect.cs`).

use crate::effects::prelude::*;

/// `DiamondPixelateImageEffect`.
#[derive(Clone, Debug)]
pub struct DiamondPixelateImageEffect {
    pub size: i32,
}

impl Default for DiamondPixelateImageEffect {
    fn default() -> Self {
        DiamondPixelateImageEffect {
            size: 16,
        }
    }
}

impl EffectImpl for DiamondPixelateImageEffect {
    fn info() -> Info {
        Info::new("diamond_pixelate", "Diamond pixelate", EffectCategory::Filters, "image-effect/diamond")
            .desc("Pixelates the image using diamond-shaped blocks.")
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
