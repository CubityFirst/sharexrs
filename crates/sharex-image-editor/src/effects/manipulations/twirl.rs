//! `TwirlImageEffect` (`Core/ImageEffects/Manipulations/TwirlImageEffect.cs`).

use crate::effects::prelude::*;

/// `TwirlImageEffect`.
#[derive(Clone, Debug)]
pub struct TwirlImageEffect {
    pub angle: f32,
    pub radius_percentage: f32,
    pub center_x_percentage: f32,
    pub center_y_percentage: f32,
}

impl Default for TwirlImageEffect {
    fn default() -> Self {
        TwirlImageEffect {
            angle: 90.0,
            radius_percentage: 50.0,
            center_x_percentage: 50.0,
            center_y_percentage: 50.0,
        }
    }
}

impl EffectImpl for TwirlImageEffect {
    fn info() -> Info {
        Info::new("twirl", "Twirl", EffectCategory::Manipulations, "image-effect/pinwheel")
            .desc("Applies a twirl distortion to the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("angle", "Angle", -360.0, 360.0, 90.0, |e: &mut Self, v| e.angle = v),
            float_slider::<Self>("radius_percentage", "Radius %", 1.0, 100.0, 50.0, |e: &mut Self, v| e.radius_percentage = v),
            float_slider::<Self>("center_x_percentage", "Center X %", 0.0, 100.0, 50.0, |e: &mut Self, v| e.center_x_percentage = v),
            float_slider::<Self>("center_y_percentage", "Center Y %", 0.0, 100.0, 50.0, |e: &mut Self, v| e.center_y_percentage = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
