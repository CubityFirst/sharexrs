//! `BlackAndWhiteImageEffect` (`Core/ImageEffects/Adjustments/BlackAndWhiteImageEffect.cs`).

use crate::effects::prelude::*;

/// `BlackAndWhiteImageEffect`.
#[derive(Clone, Debug)]
pub struct BlackAndWhiteImageEffect {
}

impl Default for BlackAndWhiteImageEffect {
    fn default() -> Self {
        BlackAndWhiteImageEffect {
        }
    }
}

impl EffectImpl for BlackAndWhiteImageEffect {
    fn info() -> Info {
        Info::new("black_and_white", "Black & White", EffectCategory::Adjustments, "app/contrast")
            .desc("Converts the image to black and white.")
            .immediate()
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
