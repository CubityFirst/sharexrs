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
        let levels = self.levels.clamp(2, 32);
        let step = 255.0 / (levels - 1) as f32;
        let q = |v: u8| {
            let i = ((v as f32 / step).round_ties_even() as i32).clamp(0, levels - 1);
            ((i as f32 * step) as i32).clamp(0, 255) as u8
        };
        let dst: Vec<SkColor> = pixels(source).iter().map(|c| SkColor::rgba(q(c.r), q(c.g), q(c.b), c.a)).collect();
        Ok(from_pixels(source.width(), source.height(), &dst))
    }
}
