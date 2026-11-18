//! `MedianFilterImageEffect` (`Core/ImageEffects/Filters/MedianFilterImageEffect.cs`).

use crate::effects::prelude::*;

/// `MedianFilterImageEffect`.
#[derive(Clone, Debug)]
pub struct MedianFilterImageEffect {
    pub radius: i32,
}

impl Default for MedianFilterImageEffect {
    fn default() -> Self {
        MedianFilterImageEffect {
            radius: 1,
        }
    }
}

impl EffectImpl for MedianFilterImageEffect {
    fn info() -> Info {
        Info::new("median_filter", "Median filter", EffectCategory::Filters, "app/filter")
            .desc("Applies a median filter to reduce noise while preserving edges.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("radius", "Radius", 1, 5, 1, |e: &mut Self, v| e.radius = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
