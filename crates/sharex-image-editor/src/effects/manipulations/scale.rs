//! `ScaleImageEffect` (`Core/ImageEffects/Manipulations/ScaleImageEffect.cs`).

use crate::effects::prelude::*;

/// `ScaleImageEffect`.
#[derive(Clone, Debug)]
pub struct ScaleImageEffect {
    pub width_percentage: f32,
    pub height_percentage: f32,
}

impl Default for ScaleImageEffect {
    fn default() -> Self {
        ScaleImageEffect {
            width_percentage: 100.0,
            height_percentage: 0.0,
        }
    }
}

impl EffectImpl for ScaleImageEffect {
    fn info() -> Info {
        Info::new("scale", "Scale", EffectCategory::Manipulations, "app/resize")
            .desc("Scales the image by width and height percentages.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("width_percentage", "Width %", 0.0, 500.0, 100.0, |e: &mut Self, v| e.width_percentage = v),
            float_slider::<Self>("height_percentage", "Height %", 0.0, 500.0, 0.0, |e: &mut Self, v| e.height_percentage = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
