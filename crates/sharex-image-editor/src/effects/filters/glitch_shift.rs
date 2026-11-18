//! `GlitchShiftImageEffect` (`Core/ImageEffects/Filters/GlitchShiftImageEffect.cs`).

use crate::effects::prelude::*;

/// `GlitchShiftImageEffect`.
#[derive(Clone, Debug)]
pub struct GlitchShiftImageEffect {
    pub intensity: i32,
    pub bands: i32,
    pub seed: i32,
}

impl Default for GlitchShiftImageEffect {
    fn default() -> Self {
        GlitchShiftImageEffect {
            intensity: 30,
            bands: 12,
            seed: 42,
        }
    }
}

impl EffectImpl for GlitchShiftImageEffect {
    fn info() -> Info {
        Info::new("glitch_shift", "Glitch shift", EffectCategory::Filters, "app/zap")
            .desc("Randomly shifts horizontal bands of the image for a digital glitch look.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("intensity", "Intensity", 1, 100, 30, |e: &mut Self, v| e.intensity = v),
            int_slider::<Self>("bands", "Band count", 2, 50, 12, |e: &mut Self, v| e.bands = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
