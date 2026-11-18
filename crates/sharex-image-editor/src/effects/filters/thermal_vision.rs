//! `ThermalVisionImageEffect` (`Core/ImageEffects/Filters/ThermalVisionImageEffect.cs`).

use crate::effects::prelude::*;

/// `ThermalVisionImageEffect`.
#[derive(Clone, Debug)]
pub struct ThermalVisionImageEffect {
    pub levels: i32,
    pub contrast: f32,
    pub glow: f32,
    pub blend: f32,
    pub invert: bool,
}

impl Default for ThermalVisionImageEffect {
    fn default() -> Self {
        ThermalVisionImageEffect {
            levels: 8,
            contrast: 135.0,
            glow: 28.0,
            blend: 100.0,
            invert: false,
        }
    }
}

impl EffectImpl for ThermalVisionImageEffect {
    fn info() -> Info {
        Info::new("thermal_vision", "Thermal vision", EffectCategory::Filters, "image-effect/thermometer-sun")
            .desc("Maps image luminance to a thermal color gradient.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("levels", "Levels", 3, 16, 8, |e: &mut Self, v| e.levels = v),
            float_slider::<Self>("contrast", "Contrast", 50.0, 200.0, 135.0, |e: &mut Self, v| e.contrast = v),
            float_slider::<Self>("glow", "Glow", 0.0, 100.0, 28.0, |e: &mut Self, v| e.glow = v),
            float_slider::<Self>("blend", "Blend", 0.0, 100.0, 100.0, |e: &mut Self, v| e.blend = v),
            bool_param::<Self>("invert", "Invert", false, |e: &mut Self, v| e.invert = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
