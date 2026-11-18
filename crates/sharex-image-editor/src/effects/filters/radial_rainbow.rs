//! `RadialRainbowImageEffect` (`Core/ImageEffects/Filters/RadialRainbowImageEffect.cs`).

use crate::effects::prelude::*;

/// `RadialRainbowImageEffect`.
#[derive(Clone, Debug)]
pub struct RadialRainbowImageEffect {
    pub strength: f32,
    pub rotations: i32,
}

impl Default for RadialRainbowImageEffect {
    fn default() -> Self {
        RadialRainbowImageEffect {
            strength: 50.0,
            rotations: 1,
        }
    }
}

impl EffectImpl for RadialRainbowImageEffect {
    fn info() -> Info {
        Info::new("radial_rainbow", "Radial rainbow", EffectCategory::Filters, "image-effect/rainbow")
            .desc("Applies a rainbow hue shift based on angle from center.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("strength", "Strength", 0.0, 100.0, 50.0, |e: &mut Self, v| e.strength = v),
            int_slider::<Self>("rotations", "Rotations", 1, 5, 1, |e: &mut Self, v| e.rotations = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
