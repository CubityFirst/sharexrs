//! `RetroSunsetImageEffect` (`Core/ImageEffects/Filters/RetroSunsetImageEffect.cs`).

use crate::effects::prelude::*;

/// `RetroSunsetImageEffect`.
#[derive(Clone, Debug)]
pub struct RetroSunsetImageEffect {
    pub strength: f32,
    pub horizontal_lines: bool,
}

impl Default for RetroSunsetImageEffect {
    fn default() -> Self {
        RetroSunsetImageEffect {
            strength: 50.0,
            horizontal_lines: true,
        }
    }
}

impl EffectImpl for RetroSunsetImageEffect {
    fn info() -> Info {
        Info::new("retro_sunset", "Retro sunset", EffectCategory::Filters, "image-effect/sunset")
            .desc("Overlays retro 80s-style sunset gradient bands.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("strength", "Strength", 0.0, 100.0, 50.0, |e: &mut Self, v| e.strength = v),
            bool_param::<Self>("horizontal_lines", "Horizontal lines", true, |e: &mut Self, v| e.horizontal_lines = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
