//! `EdgeDetectImageEffect` (`Core/ImageEffects/Filters/EdgeDetectImageEffect.cs`).

use crate::effects::prelude::*;

/// `EdgeDetectImageEffect`.
#[derive(Clone, Debug)]
pub struct EdgeDetectImageEffect {
}

impl Default for EdgeDetectImageEffect {
    fn default() -> Self {
        EdgeDetectImageEffect {
        }
    }
}

impl EffectImpl for EdgeDetectImageEffect {
    fn info() -> Info {
        Info::new("edge_detect", "Edge detect", EffectCategory::Filters, "app/inspect-window")
            .desc("Detects visible edges in the image.")
            .immediate()
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
