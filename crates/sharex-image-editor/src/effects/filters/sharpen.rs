//! `SharpenImageEffect` (`Core/ImageEffects/Filters/SharpenImageEffect.cs`).

use crate::effects::prelude::*;

/// `SharpenImageEffect`.
#[derive(Clone, Debug)]
pub struct SharpenImageEffect {
    pub strength: i32,
}

impl Default for SharpenImageEffect {
    fn default() -> Self {
        SharpenImageEffect {
            strength: 50,
        }
    }
}

impl EffectImpl for SharpenImageEffect {
    fn info() -> Info {
        Info::new("sharpen", "Sharpen", EffectCategory::Filters, "app/crosshair")
            .desc("Sharpens the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("strength", "Strength", 0, 100, 50, |e: &mut Self, v| e.strength = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
