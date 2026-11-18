//! `MiniatureImageEffect` (`Core/ImageEffects/Filters/MiniatureImageEffect.cs`).

use crate::effects::prelude::*;

/// `MiniatureImageEffect`.
#[derive(Clone, Debug)]
pub struct MiniatureImageEffect {
    pub blur_radius: i32,
    pub focus_position: f32,
    pub focus_band: f32,
    pub saturation_boost: f32,
}

impl Default for MiniatureImageEffect {
    fn default() -> Self {
        MiniatureImageEffect {
            blur_radius: 15,
            focus_position: 50.0,
            focus_band: 20.0,
            saturation_boost: 20.0,
        }
    }
}

impl EffectImpl for MiniatureImageEffect {
    fn info() -> Info {
        Info::new("miniature", "Miniature", EffectCategory::Filters, "image-effect/cube")
            .desc("Creates a toy miniature look by blurring top and bottom with boosted saturation.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("blur_radius", "Blur radius", 2, 40, 15, |e: &mut Self, v| e.blur_radius = v),
            float_slider::<Self>("focus_position", "Focus position %", 10.0, 90.0, 50.0, |e: &mut Self, v| e.focus_position = v),
            float_slider::<Self>("focus_band", "Focus band %", 5.0, 50.0, 20.0, |e: &mut Self, v| e.focus_band = v),
            float_slider::<Self>("saturation", "Saturation boost", 0.0, 50.0, 20.0, |e: &mut Self, v| e.saturation_boost = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
