//! `PerspectiveWarpImageEffect` (`Core/ImageEffects/Manipulations/PerspectiveWarpImageEffect.cs`).

use crate::effects::prelude::*;

/// `PerspectiveWarpImageEffect`.
#[derive(Clone, Debug)]
pub struct PerspectiveWarpImageEffect {
    pub top_left_x: f32,
    pub top_left_y: f32,
    pub top_right_x: f32,
    pub top_right_y: f32,
    pub bottom_right_x: f32,
    pub bottom_right_y: f32,
    pub bottom_left_x: f32,
    pub bottom_left_y: f32,
}

impl Default for PerspectiveWarpImageEffect {
    fn default() -> Self {
        PerspectiveWarpImageEffect {
            top_left_x: 0.0,
            top_left_y: 0.0,
            top_right_x: 0.0,
            top_right_y: 0.0,
            bottom_right_x: 0.0,
            bottom_right_y: 0.0,
            bottom_left_x: 0.0,
            bottom_left_y: 0.0,
        }
    }
}

impl EffectImpl for PerspectiveWarpImageEffect {
    fn info() -> Info {
        Info::new("perspective_warp", "Perspective warp", EffectCategory::Manipulations, "image-effect/move-diagonal")
            .desc("Applies a perspective warp transformation.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_numeric::<Self>("top_left_x", "Top left X", -500, 500, 0, |e: &mut Self, v| e.top_left_x = v as f32),
            int_numeric::<Self>("top_left_y", "Top left Y", -500, 500, 0, |e: &mut Self, v| e.top_left_y = v as f32),
            int_numeric::<Self>("top_right_x", "Top right X", -500, 500, 0, |e: &mut Self, v| e.top_right_x = v as f32),
            int_numeric::<Self>("top_right_y", "Top right Y", -500, 500, 0, |e: &mut Self, v| e.top_right_y = v as f32),
            int_numeric::<Self>("bottom_right_x", "Bottom right X", -500, 500, 0, |e: &mut Self, v| e.bottom_right_x = v as f32),
            int_numeric::<Self>("bottom_right_y", "Bottom right Y", -500, 500, 0, |e: &mut Self, v| e.bottom_right_y = v as f32),
            int_numeric::<Self>("bottom_left_x", "Bottom left X", -500, 500, 0, |e: &mut Self, v| e.bottom_left_x = v as f32),
            int_numeric::<Self>("bottom_left_y", "Bottom left Y", -500, 500, 0, |e: &mut Self, v| e.bottom_left_y = v as f32),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
