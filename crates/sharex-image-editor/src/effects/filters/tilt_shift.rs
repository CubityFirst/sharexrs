//! `TiltShiftImageEffect` (`Core/ImageEffects/Filters/TiltShiftImageEffect.cs`).

use crate::effects::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TiltShiftMode {
    Linear,
    Radial,
}

/// `TiltShiftImageEffect`.
#[derive(Clone, Debug)]
pub struct TiltShiftImageEffect {
    pub mode: TiltShiftMode,
    pub blur_radius: f32,
    pub focus_size: f32,
    pub focus_position_x: f32,
    pub focus_position_y: f32,
    pub falloff: f32,
    pub saturation_boost: f32,
}

impl Default for TiltShiftImageEffect {
    fn default() -> Self {
        TiltShiftImageEffect {
            mode: TiltShiftMode::Linear,
            blur_radius: 12.0,
            focus_size: 30.0,
            focus_position_x: 50.0,
            focus_position_y: 50.0,
            falloff: 24.0,
            saturation_boost: 35.0,
        }
    }
}

impl EffectImpl for TiltShiftImageEffect {
    fn info() -> Info {
        Info::new("tilt_shift", "Tilt-shift", EffectCategory::Filters, "image-editor/blur")
            .desc("Simulates a tilt-shift lens with selective focus.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            enum_param::<Self, TiltShiftMode>("mode", "Mode", TiltShiftMode::Linear, |e: &mut Self, v| e.mode = v, &[("Linear", TiltShiftMode::Linear), ("Radial", TiltShiftMode::Radial)]),
            float_slider::<Self>("blur_radius", "Blur radius", 0.0, 30.0, 12.0, |e: &mut Self, v| e.blur_radius = v),
            float_slider::<Self>("focus_size", "Focus size", 5.0, 90.0, 30.0, |e: &mut Self, v| e.focus_size = v),
            float_slider::<Self>("focus_position_x", "Focus position X", 0.0, 100.0, 50.0, |e: &mut Self, v| e.focus_position_x = v),
            float_slider::<Self>("focus_position_y", "Focus position Y", 0.0, 100.0, 50.0, |e: &mut Self, v| e.focus_position_y = v),
            float_slider::<Self>("falloff", "Falloff", 1.0, 60.0, 24.0, |e: &mut Self, v| e.falloff = v),
            float_slider::<Self>("saturation_boost", "Saturation boost", 0.0, 100.0, 35.0, |e: &mut Self, v| e.saturation_boost = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
