//! `AutoContrastImageEffect` (`Core/ImageEffects/Adjustments/AutoContrastImageEffect.cs`).

use crate::effects::prelude::*;

/// `AutoContrastImageEffect`.
#[derive(Clone, Debug)]
pub struct AutoContrastImageEffect {
    pub clip_percent: f32,
}

impl Default for AutoContrastImageEffect {
    fn default() -> Self {
        AutoContrastImageEffect {
            clip_percent: 0.5,
        }
    }
}

impl EffectImpl for AutoContrastImageEffect {
    fn info() -> Info {
        Info::new("auto_contrast", "Auto contrast", EffectCategory::Adjustments, "app/magic-wand")
            .desc("Automatically adjusts the contrast.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("clip_percent", "Clip percent", 0.0, 20.0, 0.5, |e: &mut Self, v| e.clip_percent = v).tick(0.1).snap(false).format("{}{0:0.0}"),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
