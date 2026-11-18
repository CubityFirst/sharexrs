//! `SurfaceBlurImageEffect` (`Core/ImageEffects/Filters/SurfaceBlurImageEffect.cs`).

use crate::effects::prelude::*;

/// `SurfaceBlurImageEffect`.
#[derive(Clone, Debug)]
pub struct SurfaceBlurImageEffect {
    pub radius: i32,
    pub threshold: i32,
}

impl Default for SurfaceBlurImageEffect {
    fn default() -> Self {
        SurfaceBlurImageEffect {
            radius: 3,
            threshold: 24,
        }
    }
}

impl EffectImpl for SurfaceBlurImageEffect {
    fn info() -> Info {
        Info::new("surface_blur", "Surface blur", EffectCategory::Filters, "image-effect/waves")
            .desc("Blurs while preserving edges based on color similarity.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("radius", "Radius", 1, 8, 3, |e: &mut Self, v| e.radius = v),
            int_slider::<Self>("threshold", "Threshold", 1, 100, 24, |e: &mut Self, v| e.threshold = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
