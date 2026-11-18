//! `LiquifyPushSmudgeImageEffect` (`Core/ImageEffects/Manipulations/LiquifyPushSmudgeImageEffect.cs`).

use crate::effects::prelude::*;

/// `LiquifyPushSmudgeImageEffect`.
#[derive(Clone, Debug)]
pub struct LiquifyPushSmudgeImageEffect {
    pub angle: f32,
    pub distance: f32,
    pub radius_percentage: f32,
    pub smudge: f32,
    pub center_x_percentage: f32,
    pub center_y_percentage: f32,
}

impl Default for LiquifyPushSmudgeImageEffect {
    fn default() -> Self {
        LiquifyPushSmudgeImageEffect {
            angle: 0.0,
            distance: 40.0,
            radius_percentage: 25.0,
            smudge: 35.0,
            center_x_percentage: 50.0,
            center_y_percentage: 50.0,
        }
    }
}

impl EffectImpl for LiquifyPushSmudgeImageEffect {
    fn info() -> Info {
        Info::new("liquify_push_smudge", "Liquify push / smudge", EffectCategory::Manipulations, "image-effect/droplet")
            .desc("Pushes and smudges pixels in a direction.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("angle", "Angle", 0.0, 360.0, 0.0, |e: &mut Self, v| e.angle = v),
            float_slider::<Self>("distance", "Distance", -200.0, 200.0, 40.0, |e: &mut Self, v| e.distance = v),
            float_slider::<Self>("radius_percentage", "Radius percentage", 1.0, 100.0, 25.0, |e: &mut Self, v| e.radius_percentage = v),
            float_slider::<Self>("smudge", "Smudge", 0.0, 100.0, 35.0, |e: &mut Self, v| e.smudge = v),
            float_slider::<Self>("center_x_percentage", "Center X percentage", 0.0, 100.0, 50.0, |e: &mut Self, v| e.center_x_percentage = v),
            float_slider::<Self>("center_y_percentage", "Center Y percentage", 0.0, 100.0, 50.0, |e: &mut Self, v| e.center_y_percentage = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
