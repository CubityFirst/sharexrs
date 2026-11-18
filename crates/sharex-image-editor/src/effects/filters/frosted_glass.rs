//! `FrostedGlassImageEffect` (`Core/ImageEffects/Filters/FrostedGlassImageEffect.cs`).

use crate::effects::prelude::*;

/// `FrostedGlassImageEffect`.
#[derive(Clone, Debug)]
pub struct FrostedGlassImageEffect {
    pub radius: i32,
}

impl Default for FrostedGlassImageEffect {
    fn default() -> Self {
        FrostedGlassImageEffect {
            radius: 5,
        }
    }
}

impl EffectImpl for FrostedGlassImageEffect {
    fn info() -> Info {
        Info::new("frosted_glass", "Frosted glass", EffectCategory::Filters, "image-effect/snowflake")
            .desc("Simulates looking through frosted glass by randomizing nearby pixels.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("radius", "Radius", 1, 20, 5, |e: &mut Self, v| e.radius = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
