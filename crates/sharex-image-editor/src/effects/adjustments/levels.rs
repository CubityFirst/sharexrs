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
        let in_black = self.input_black.clamp(0, 255);
        let mut in_white = self.input_white.clamp(0, 255);
        let mut out_black = self.output_black.clamp(0, 255);
        let mut out_white = self.output_white.clamp(0, 255);
        let gamma = clampf(self.gamma, 0.1, 5.0);
        if in_white <= in_black {
            in_white = 255.min(in_black + 1);
        }
        if out_white < out_black {
            std::mem::swap(&mut out_black, &mut out_white);
        }
        if in_black == 0 && in_white == 255 && (gamma - 1.0).abs() < 0.0001 && out_black == 0 && out_white == 255 {
            return Ok(source.clone());
        }
        let in_range = (in_white - in_black) as f32;
        let out_range = (out_white - out_black) as f32;
        let map = |v: u8| -> u8 {
            let mut n = (v as i32 - in_black) as f32 / in_range;
            n = clampf(n, 0.0, 1.0);
            let corrected = n.powf(gamma);
            let output = out_black as f32 + corrected * out_range;
            if output <= 0.0 {
                return 0;
            }
            if output >= 255.0 {
                return 255;
            }
            output.round_ties_even() as u8
        };
        Ok(apply_pixel_operation(source, |c| SkColor::rgba(map(c.r), map(c.g), map(c.b), c.a)))
    }
}
