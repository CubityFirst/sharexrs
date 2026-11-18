//! `EmbossImageEffect` (`Core/ImageEffects/Filters/EmbossImageEffect.cs`).

use crate::effects::prelude::*;

/// `EmbossImageEffect`.
#[derive(Clone, Debug)]
pub struct EmbossImageEffect {
}

impl Default for EmbossImageEffect {
    fn default() -> Self {
        EmbossImageEffect {
        }
    }
}

impl EffectImpl for EmbossImageEffect {
    fn info() -> Info {
        Info::new("emboss", "Emboss", EffectCategory::Filters, "app/stamp")
            .desc("Applies an emboss effect.")
            .immediate()
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
