//! `PolarWarpImageEffect` (`Core/ImageEffects/Manipulations/PolarWarpImageEffect.cs`).

use crate::effects::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolarWarpMode {
    CartesianToPolar,
    PolarToCartesian,
}

/// `PolarWarpImageEffect`.
#[derive(Clone, Debug)]
pub struct PolarWarpImageEffect {
    pub mode: PolarWarpMode,
    pub rotation: f32,
    pub radius_scale: f32,
    pub center_x_percentage: f32,
    pub center_y_percentage: f32,
}

impl Default for PolarWarpImageEffect {
    fn default() -> Self {
        PolarWarpImageEffect {
            mode: PolarWarpMode::CartesianToPolar,
            rotation: 0.0,
            radius_scale: 100.0,
            center_x_percentage: 50.0,
            center_y_percentage: 50.0,
        }
    }
}

impl EffectImpl for PolarWarpImageEffect {
    fn info() -> Info {
        Info::new("polar_warp", "Polar warp", EffectCategory::Manipulations, "image-effect/orbit")
            .desc("Converts between Cartesian and polar coordinate systems.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            enum_param::<Self, PolarWarpMode>("mode", "Mode", PolarWarpMode::CartesianToPolar, |e: &mut Self, v| e.mode = v, &[("Cartesian to polar", PolarWarpMode::CartesianToPolar), ("Polar to Cartesian", PolarWarpMode::PolarToCartesian)]),
            float_slider::<Self>("rotation", "Rotation", -360.0, 360.0, 0.0, |e: &mut Self, v| e.rotation = v),
            float_slider::<Self>("radius_scale", "Radius scale", 20.0, 200.0, 100.0, |e: &mut Self, v| e.radius_scale = v),
            float_slider::<Self>("center_x_percentage", "Center X %", 0.0, 100.0, 50.0, |e: &mut Self, v| e.center_x_percentage = v),
            float_slider::<Self>("center_y_percentage", "Center Y %", 0.0, 100.0, 50.0, |e: &mut Self, v| e.center_y_percentage = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
