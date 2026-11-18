//! `BlackAndWhiteImageEffect` (`Core/ImageEffects/Adjustments/BlackAndWhiteImageEffect.cs`).

use crate::effects::prelude::*;

/// `BlackAndWhiteImageEffect`.
#[derive(Clone, Debug)]
pub struct BlackAndWhiteImageEffect {}

impl Default for BlackAndWhiteImageEffect {
    fn default() -> Self {
        BlackAndWhiteImageEffect {}
    }
}

impl EffectImpl for BlackAndWhiteImageEffect {
    fn info() -> Info {
        Info::new(
            "black_and_white",
            "Black & White",
            EffectCategory::Adjustments,
            "app/contrast",
        )
        .desc("Converts the image to black and white.")
        .immediate()
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        Ok(apply_pixel_operation(source, |c| {
            let lum = 0.2126 * c.r as f32 + 0.7152 * c.g as f32 + 0.0722 * c.b as f32;
            if lum > 127.0 {
                color::WHITE
            } else {
                color::BLACK
            }
        }))
    }
}
