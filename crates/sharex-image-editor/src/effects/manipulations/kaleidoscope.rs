//! `KaleidoscopeImageEffect` (`Core/ImageEffects/Manipulations/KaleidoscopeImageEffect.cs`).

use crate::effects::prelude::*;

/// `KaleidoscopeImageEffect`.
#[derive(Clone, Debug)]
pub struct KaleidoscopeImageEffect {
    pub segments: i32,
    pub rotation: f32,
    pub zoom: f32,
    pub center_x_percentage: f32,
    pub center_y_percentage: f32,
}

impl Default for KaleidoscopeImageEffect {
    fn default() -> Self {
        KaleidoscopeImageEffect {
            segments: 8,
            rotation: 0.0,
            zoom: 100.0,
            center_x_percentage: 50.0,
            center_y_percentage: 50.0,
        }
    }
}

impl EffectImpl for KaleidoscopeImageEffect {
    fn info() -> Info {
        Info::new("kaleidoscope", "Kaleidoscope", EffectCategory::Manipulations, "image-effect/pinwheel")
            .desc("Creates a kaleidoscope mirror pattern.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("segments", "Segments", 2, 24, 8, |e: &mut Self, v| e.segments = v),
            float_slider::<Self>("rotation", "Rotation", 0.0, 360.0, 0.0, |e: &mut Self, v| e.rotation = v),
            float_slider::<Self>("zoom", "Zoom", 20.0, 300.0, 100.0, |e: &mut Self, v| e.zoom = v),
            float_slider::<Self>("center_x_percentage", "Center X percentage", 0.0, 100.0, 50.0, |e: &mut Self, v| e.center_x_percentage = v),
            float_slider::<Self>("center_y_percentage", "Center Y percentage", 0.0, 100.0, 50.0, |e: &mut Self, v| e.center_y_percentage = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
