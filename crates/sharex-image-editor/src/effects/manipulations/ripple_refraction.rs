//! `RippleRefractionImageEffect` (`Core/ImageEffects/Manipulations/RippleRefractionImageEffect.cs`).

use crate::effects::prelude::*;

/// `RippleRefractionImageEffect`.
#[derive(Clone, Debug)]
pub struct RippleRefractionImageEffect {
    pub amplitude: f32,
    pub wavelength: f32,
    pub phase: f32,
    pub refraction: f32,
    pub center_x_percentage: f32,
    pub center_y_percentage: f32,
}

impl Default for RippleRefractionImageEffect {
    fn default() -> Self {
        RippleRefractionImageEffect {
            amplitude: 12.0,
            wavelength: 32.0,
            phase: 0.0,
            refraction: 35.0,
            center_x_percentage: 50.0,
            center_y_percentage: 50.0,
        }
    }
}

impl EffectImpl for RippleRefractionImageEffect {
    fn info() -> Info {
        Info::new("ripple_refraction", "Ripple refraction", EffectCategory::Manipulations, "image-effect/waves")
            .desc("Applies a ripple refraction distortion effect.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("amplitude", "Amplitude", 0.0, 80.0, 12.0, |e: &mut Self, v| e.amplitude = v),
            float_slider::<Self>("wavelength", "Wavelength", 4.0, 400.0, 32.0, |e: &mut Self, v| e.wavelength = v),
            float_slider::<Self>("phase", "Phase", -360.0, 360.0, 0.0, |e: &mut Self, v| e.phase = v),
            float_slider::<Self>("refraction", "Refraction", 0.0, 100.0, 35.0, |e: &mut Self, v| e.refraction = v),
            float_slider::<Self>("center_x_percentage", "Center X %", 0.0, 100.0, 50.0, |e: &mut Self, v| e.center_x_percentage = v),
            float_slider::<Self>("center_y_percentage", "Center Y %", 0.0, 100.0, 50.0, |e: &mut Self, v| e.center_y_percentage = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
