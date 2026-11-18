//! `LevelsImageEffect` (`Core/ImageEffects/Adjustments/LevelsImageEffect.cs`).

use crate::effects::prelude::*;

/// `LevelsImageEffect`.
#[derive(Clone, Debug)]
pub struct LevelsImageEffect {
    pub input_black: i32,
    pub input_white: i32,
    pub gamma: f32,
    pub output_black: i32,
    pub output_white: i32,
}

impl Default for LevelsImageEffect {
    fn default() -> Self {
        LevelsImageEffect {
            input_black: 0,
            input_white: 255,
            gamma: 1.0,
            output_black: 0,
            output_white: 255,
        }
    }
}

impl EffectImpl for LevelsImageEffect {
    fn info() -> Info {
        Info::new("levels", "Levels", EffectCategory::Adjustments, "image-effect/sliders-vertical")
            .desc("Adjusts image color levels.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("input_black", "Input black", 0, 255, 0, |e: &mut Self, v| e.input_black = v),
            int_slider::<Self>("input_white", "Input white", 0, 255, 255, |e: &mut Self, v| e.input_white = v),
            float_slider::<Self>("gamma", "Gamma", 0.1, 5.0, 1.0, |e: &mut Self, v| e.gamma = v).tick(0.1).snap(false).format("{}{0:0.0}"),
            int_slider::<Self>("output_black", "Output black", 0, 255, 0, |e: &mut Self, v| e.output_black = v),
            int_slider::<Self>("output_white", "Output white", 0, 255, 255, |e: &mut Self, v| e.output_white = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
