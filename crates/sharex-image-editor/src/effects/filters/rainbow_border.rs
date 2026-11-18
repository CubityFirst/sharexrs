//! `RainbowBorderImageEffect` (`Core/ImageEffects/Filters/RainbowBorderImageEffect.cs`).

use crate::effects::prelude::*;

/// `RainbowBorderImageEffect`.
#[derive(Clone, Debug)]
pub struct RainbowBorderImageEffect {
    pub border_size: i32,
    pub saturation: f32,
    pub brightness: f32,
    pub hue_offset: f32,
    pub glow_strength: f32,
    pub inner_line: bool,
}

impl Default for RainbowBorderImageEffect {
    fn default() -> Self {
        RainbowBorderImageEffect {
            border_size: 24,
            saturation: 100.0,
            brightness: 95.0,
            hue_offset: 0.0,
            glow_strength: 40.0,
            inner_line: true,
        }
    }
}

impl EffectImpl for RainbowBorderImageEffect {
    fn info() -> Info {
        Info::new("rainbow_border", "Rainbow border", EffectCategory::Filters, "image-effect/rainbow")
            .desc("Adds a vivid rainbow gradient border that cycles through the full color spectrum.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_numeric::<Self>("border_size", "Border size", 2, 300, 24, |e: &mut Self, v| e.border_size = v),
            float_slider::<Self>("saturation", "Saturation", 0.0, 100.0, 100.0, |e: &mut Self, v| e.saturation = v),
            float_slider::<Self>("brightness", "Brightness", 0.0, 100.0, 95.0, |e: &mut Self, v| e.brightness = v),
            float_slider::<Self>("hue_offset", "Hue offset", 0.0, 360.0, 0.0, |e: &mut Self, v| e.hue_offset = v),
            float_slider::<Self>("glow_strength", "Glow", 0.0, 100.0, 40.0, |e: &mut Self, v| e.glow_strength = v),
            bool_param::<Self>("inner_line", "Inner line", true, |e: &mut Self, v| e.inner_line = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
