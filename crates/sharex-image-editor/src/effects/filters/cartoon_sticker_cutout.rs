//! `CartoonStickerCutoutImageEffect` (`Core/ImageEffects/Filters/CartoonStickerCutoutImageEffect.cs`).

use crate::effects::prelude::*;

/// `CartoonStickerCutoutImageEffect`.
#[derive(Clone, Debug)]
pub struct CartoonStickerCutoutImageEffect {
    pub color_levels: i32,
    pub edge_threshold: i32,
    pub ink_strength: f32,
    pub sticker_border: i32,
    pub border_strength: f32,
}

impl Default for CartoonStickerCutoutImageEffect {
    fn default() -> Self {
        CartoonStickerCutoutImageEffect {
            color_levels: 8,
            edge_threshold: 32,
            ink_strength: 85.0,
            sticker_border: 3,
            border_strength: 90.0,
        }
    }
}

impl EffectImpl for CartoonStickerCutoutImageEffect {
    fn info() -> Info {
        Info::new("cartoon_sticker_cutout", "Cartoon sticker cutout", EffectCategory::Filters, "image-effect/sticker")
            .desc("Applies a cartoon sticker cutout effect with quantized colors and ink edges.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("colorLevels", "Color levels", 2, 20, 8, |e: &mut Self, v| e.color_levels = v),
            int_slider::<Self>("edgeThreshold", "Edge threshold", 5, 120, 32, |e: &mut Self, v| e.edge_threshold = v),
            float_slider::<Self>("inkStrength", "Ink strength", 0.0, 100.0, 85.0, |e: &mut Self, v| e.ink_strength = v),
            int_slider::<Self>("stickerBorder", "Sticker border", 0, 10, 3, |e: &mut Self, v| e.sticker_border = v),
            float_slider::<Self>("borderStrength", "Border strength", 0.0, 100.0, 90.0, |e: &mut Self, v| e.border_strength = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
