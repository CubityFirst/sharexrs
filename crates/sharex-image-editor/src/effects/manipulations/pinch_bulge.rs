//! `PinchBulgeImageEffect` (`Core/ImageEffects/Manipulations/PinchBulgeImageEffect.cs`).

use crate::effects::prelude::*;

/// `PinchBulgeImageEffect`.
#[derive(Clone, Debug)]
pub struct PinchBulgeImageEffect {
    pub strength: f32,
    pub radius_percentage: f32,
    pub center_x_percentage: f32,
    pub center_y_percentage: f32,
}

impl Default for PinchBulgeImageEffect {
    fn default() -> Self {
        PinchBulgeImageEffect {
            strength: 35.0,
            radius_percentage: 50.0,
            center_x_percentage: 50.0,
            center_y_percentage: 50.0,
        }
    }
}

impl EffectImpl for PinchBulgeImageEffect {
    fn info() -> Info {
        Info::new("pinch_bulge", "Pinch / bulge", EffectCategory::Manipulations, "app/expand")
            .desc("Applies a pinch or bulge distortion to the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("strength", "Strength", -100.0, 100.0, 35.0, |e: &mut Self, v| e.strength = v),
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
