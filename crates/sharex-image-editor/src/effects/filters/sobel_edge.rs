//! `SobelEdgeImageEffect` (`Core/ImageEffects/Filters/SobelEdgeImageEffect.cs`).

use crate::effects::prelude::*;

/// `SobelEdgeImageEffect`.
#[derive(Clone, Debug)]
pub struct SobelEdgeImageEffect {
    pub strength: f32,
    pub threshold: i32,
}

impl Default for SobelEdgeImageEffect {
    fn default() -> Self {
        SobelEdgeImageEffect {
            strength: 1.2,
            threshold: 20,
        }
    }
}

impl EffectImpl for SobelEdgeImageEffect {
    fn info() -> Info {
        Info::new("sobel_edge", "Sobel edge", EffectCategory::Filters, "app/region")
            .desc("Detects edges using the Sobel operator.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("strength", "Strength", 0.10000000149011612, 5.0, 1.2000000476837158, |e: &mut Self, v| e.strength = v),
            int_slider::<Self>("threshold", "Threshold", 0, 255, 20, |e: &mut Self, v| e.threshold = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
