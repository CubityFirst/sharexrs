//! `HeatMapImageEffect` (`Core/ImageEffects/Filters/HeatMapImageEffect.cs`).

use crate::effects::prelude::*;

/// `HeatMapImageEffect`.
#[derive(Clone, Debug)]
pub struct HeatMapImageEffect {
}

impl Default for HeatMapImageEffect {
    fn default() -> Self {
        HeatMapImageEffect {
        }
    }
}

impl EffectImpl for HeatMapImageEffect {
    fn info() -> Info {
        Info::new("heat_map", "Heat map", EffectCategory::Filters, "image-effect/flame")
            .desc("Maps image luminance to a fire-like color gradient.")
            .immediate()
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
