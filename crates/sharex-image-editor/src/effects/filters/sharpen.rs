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
        if self.strength <= 0 {
            return Ok(source.clone());
        }
        let s = clampf(self.strength as f32 / 100.0, 0.0, 1.0);
        let center = 1.0 + 4.0 * s;
        let edge = -s;
        let k = [0.0, edge, 0.0, edge, center, edge, 0.0, edge, 0.0];
        Ok(matrix_convolution_3x3(source, &k, 1.0, 0.0, true))
    }
}
