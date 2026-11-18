//! `GaussianBlurImageEffect` (`Core/ImageEffects/Filters/GaussianBlurImageEffect.cs`).

use crate::effects::prelude::*;

/// `GaussianBlurImageEffect`.
#[derive(Clone, Debug)]
pub struct GaussianBlurImageEffect {
    pub radius: i32,
}

impl Default for GaussianBlurImageEffect {
    fn default() -> Self {
        GaussianBlurImageEffect { radius: 15 }
    }
}

impl EffectImpl for GaussianBlurImageEffect {
    fn info() -> Info {
        Info::new(
            "gaussian_blur",
            "Gaussian blur",
            EffectCategory::Filters,
            "image-effect/gauge",
        )
        .desc("Applies a Gaussian blur effect.")
    }

    fn params() -> Vec<ParamDef> {
        vec![int_slider::<Self>(
            "radius",
            "Radius",
            1,
            200,
            15,
            |e: &mut Self, v| e.radius = v,
        )]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let radius = 1.max(self.radius);
        Ok(blur_clamped(source, radius * 2, radius as f32 / 3.0))
    }
}
