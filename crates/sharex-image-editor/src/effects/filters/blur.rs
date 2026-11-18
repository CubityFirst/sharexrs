//! `BlurImageEffect` (`Core/ImageEffects/Filters/BlurImageEffect.cs`).

use crate::effects::prelude::*;

/// `BlurImageEffect`.
#[derive(Clone, Debug)]
pub struct BlurImageEffect {
    pub radius: i32,
}

impl Default for BlurImageEffect {
    fn default() -> Self {
        BlurImageEffect {
            radius: 5,
        }
    }
}

impl EffectImpl for BlurImageEffect {
    fn info() -> Info {
        Info::new("blur", "Blur", EffectCategory::Filters, "image-editor/blur")
            .desc("Applies a blur effect.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("radius", "Radius", 1, 200, 10, |e: &mut Self, v| e.radius = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
