//! `RotateImageEffect` (`Core/ImageEffects/Manipulations/RotateImageEffect.cs`).

use crate::effects::prelude::*;

/// `RotateImageEffect`.
#[derive(Clone, Debug)]
pub struct RotateImageEffect {
    pub angle: f32,
    pub auto_resize: bool,
}

impl Default for RotateImageEffect {
    fn default() -> Self {
        RotateImageEffect {
            angle: 0.0,
            auto_resize: true,
        }
    }
}

impl EffectImpl for RotateImageEffect {
    fn info() -> Info {
        Info::new("rotate_custom_angle", "Rotate", EffectCategory::Manipulations, "image-effect/rotate-clockwise")
            .desc("Rotates the image by a custom angle.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("angle", "Angle", -360.0, 360.0, 0.0, |e: &mut Self, v| e.angle = v),
            bool_param::<Self>("auto_resize", "Auto resize", true, |e: &mut Self, v| e.auto_resize = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
