//! `FisheyeLensImageEffect` (`Core/ImageEffects/Manipulations/FisheyeLensImageEffect.cs`).

use crate::effects::prelude::*;

/// `FisheyeLensImageEffect`.
#[derive(Clone, Debug)]
pub struct FisheyeLensImageEffect {
    pub strength: f32,
    pub radius_percentage: f32,
    pub center_x_percentage: f32,
    pub center_y_percentage: f32,
}

impl Default for FisheyeLensImageEffect {
    fn default() -> Self {
        FisheyeLensImageEffect {
            strength: 58.0,
            radius_percentage: 100.0,
            center_x_percentage: 50.0,
            center_y_percentage: 50.0,
        }
    }
}

impl EffectImpl for FisheyeLensImageEffect {
    fn info() -> Info {
        Info::new("fisheye_lens", "Fisheye lens", EffectCategory::Manipulations, "image-effect/gauge")
            .desc("Applies a fisheye lens distortion effect.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("strength", "Strength", 0.0, 100.0, 58.0, |e: &mut Self, v| e.strength = v),
            float_slider::<Self>("radius_percentage", "Radius percentage", 1.0, 100.0, 100.0, |e: &mut Self, v| e.radius_percentage = v),
            float_slider::<Self>("center_x_percentage", "Center X percentage", 0.0, 100.0, 50.0, |e: &mut Self, v| e.center_x_percentage = v),
            float_slider::<Self>("center_y_percentage", "Center Y percentage", 0.0, 100.0, 50.0, |e: &mut Self, v| e.center_y_percentage = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
