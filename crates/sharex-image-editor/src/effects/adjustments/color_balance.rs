//! `ColorBalanceImageEffect` (`Core/ImageEffects/Adjustments/ColorBalanceImageEffect.cs`).

use crate::effects::prelude::*;

/// `ColorBalanceImageEffect`.
#[derive(Clone, Debug)]
pub struct ColorBalanceImageEffect {
    pub shadow_cyan_red: f32,
    pub shadow_magenta_green: f32,
    pub shadow_yellow_blue: f32,
    pub mid_cyan_red: f32,
    pub mid_magenta_green: f32,
    pub mid_yellow_blue: f32,
    pub highlight_cyan_red: f32,
    pub highlight_magenta_green: f32,
    pub highlight_yellow_blue: f32,
}

impl Default for ColorBalanceImageEffect {
    fn default() -> Self {
        ColorBalanceImageEffect {
            shadow_cyan_red: 0.0,
            shadow_magenta_green: 0.0,
            shadow_yellow_blue: 0.0,
            mid_cyan_red: 0.0,
            mid_magenta_green: 0.0,
            mid_yellow_blue: 0.0,
            highlight_cyan_red: 0.0,
            highlight_magenta_green: 0.0,
            highlight_yellow_blue: 0.0,
        }
    }
}

impl EffectImpl for ColorBalanceImageEffect {
    fn info() -> Info {
        Info::new("color_balance", "Color Balance", EffectCategory::Adjustments, "app/sliders")
            .desc("Adjusts color balance in shadows, midtones, and highlights.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("shadow_cyan_red", "Shadows: Cyan-Red", -100.0, 100.0, 0.0, |e: &mut Self, v| e.shadow_cyan_red = v),
            float_slider::<Self>("shadow_magenta_green", "Shadows: Magenta-Green", -100.0, 100.0, 0.0, |e: &mut Self, v| e.shadow_magenta_green = v),
            float_slider::<Self>("shadow_yellow_blue", "Shadows: Yellow-Blue", -100.0, 100.0, 0.0, |e: &mut Self, v| e.shadow_yellow_blue = v),
            float_slider::<Self>("mid_cyan_red", "Midtones: Cyan-Red", -100.0, 100.0, 0.0, |e: &mut Self, v| e.mid_cyan_red = v),
            float_slider::<Self>("mid_magenta_green", "Midtones: Magenta-Green", -100.0, 100.0, 0.0, |e: &mut Self, v| e.mid_magenta_green = v),
            float_slider::<Self>("mid_yellow_blue", "Midtones: Yellow-Blue", -100.0, 100.0, 0.0, |e: &mut Self, v| e.mid_yellow_blue = v),
            float_slider::<Self>("highlight_cyan_red", "Highlights: Cyan-Red", -100.0, 100.0, 0.0, |e: &mut Self, v| e.highlight_cyan_red = v),
            float_slider::<Self>("highlight_magenta_green", "Highlights: Magenta-Green", -100.0, 100.0, 0.0, |e: &mut Self, v| e.highlight_magenta_green = v),
            float_slider::<Self>("highlight_yellow_blue", "Highlights: Yellow-Blue", -100.0, 100.0, 0.0, |e: &mut Self, v| e.highlight_yellow_blue = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
