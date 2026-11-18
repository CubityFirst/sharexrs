//! `SpinBlurImageEffect` (`Core/ImageEffects/Filters/SpinBlurImageEffect.cs`).

use crate::effects::prelude::*;

/// `SpinBlurImageEffect`.
#[derive(Clone, Debug)]
pub struct SpinBlurImageEffect {
    pub angle: f32,
    pub samples: i32,
    pub center_x: f32,
    pub center_y: f32,
}

impl Default for SpinBlurImageEffect {
    fn default() -> Self {
        SpinBlurImageEffect {
            angle: 20.0,
            samples: 24,
            center_x: 50.0,
            center_y: 50.0,
        }
    }
}

impl EffectImpl for SpinBlurImageEffect {
    fn info() -> Info {
        Info::new("spin_blur", "Spin blur", EffectCategory::Filters, "image-effect/rotate-clockwise")
            .desc("Applies a rotational motion blur around a center point.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("angle", "Angle", 0.0, 180.0, 20.0, |e: &mut Self, v| e.angle = v),
            int_slider::<Self>("samples", "Samples", 4, 64, 24, |e: &mut Self, v| e.samples = v),
            float_slider::<Self>("center_x", "Center X", 0.0, 100.0, 50.0, |e: &mut Self, v| e.center_x = v),
            float_slider::<Self>("center_y", "Center Y", 0.0, 100.0, 50.0, |e: &mut Self, v| e.center_y = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
