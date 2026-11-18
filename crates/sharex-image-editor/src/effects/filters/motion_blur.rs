//! `MotionBlurImageEffect` (`Core/ImageEffects/Filters/MotionBlurImageEffect.cs`).

use crate::effects::prelude::*;

/// `MotionBlurImageEffect`.
#[derive(Clone, Debug)]
pub struct MotionBlurImageEffect {
    pub distance: i32,
    pub angle: f32,
}

impl Default for MotionBlurImageEffect {
    fn default() -> Self {
        MotionBlurImageEffect {
            distance: 12,
            angle: 0.0,
        }
    }
}

impl EffectImpl for MotionBlurImageEffect {
    fn info() -> Info {
        Info::new("motion_blur", "Motion blur", EffectCategory::Filters, "app/move-horizontal")
            .desc("Applies a directional motion blur at a specified angle.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("distance", "Distance", 1, 200, 12, |e: &mut Self, v| e.distance = v),
            float_slider::<Self>("angle", "Angle", 0.0, 360.0, 0.0, |e: &mut Self, v| e.angle = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
