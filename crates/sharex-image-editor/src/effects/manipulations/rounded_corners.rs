//! `RoundedCornersImageEffect` (`Core/ImageEffects/Manipulations/RoundedCornersImageEffect.cs`).

use crate::effects::prelude::*;

/// `RoundedCornersImageEffect`.
#[derive(Clone, Debug)]
pub struct RoundedCornersImageEffect {
    pub corner_radius: i32,
}

impl Default for RoundedCornersImageEffect {
    fn default() -> Self {
        RoundedCornersImageEffect {
            corner_radius: 20,
        }
    }
}

impl EffectImpl for RoundedCornersImageEffect {
    fn info() -> Info {
        Info::new("rounded_corners", "Rounded Corners", EffectCategory::Manipulations, "image-effect/rounded-corner")
            .desc("Rounds the corners of the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("corner_radius", "Corner radius", 0, 500, 20, |e: &mut Self, v| e.corner_radius = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
