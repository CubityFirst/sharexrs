//! `AnamorphicLensFlareImageEffect` (`Core/ImageEffects/Filters/AnamorphicLensFlareImageEffect.cs`).

use crate::effects::prelude::*;

/// `AnamorphicLensFlareImageEffect`.
#[derive(Clone, Debug)]
pub struct AnamorphicLensFlareImageEffect {
    pub intensity: f32,
    pub threshold: f32,
    pub streak_length: f32,
    pub warmth: f32,
    pub ghosting: f32,
}

impl Default for AnamorphicLensFlareImageEffect {
    fn default() -> Self {
        AnamorphicLensFlareImageEffect {
            intensity: 60.0,
            threshold: 72.0,
            streak_length: 55.0,
            warmth: 45.0,
            ghosting: 35.0,
        }
    }
}

impl EffectImpl for AnamorphicLensFlareImageEffect {
    fn info() -> Info {
        Info::new("anamorphic_lens_flare", "Anamorphic lens flare", EffectCategory::Filters, "image-effect/sparkle")
            .desc("Adds cinematic horizontal lens flare streaks to bright areas.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("intensity", "Intensity", 0.0, 100.0, 60.0, |e: &mut Self, v| e.intensity = v),
            float_slider::<Self>("threshold", "Threshold", 0.0, 100.0, 72.0, |e: &mut Self, v| e.threshold = v),
            float_slider::<Self>("streak_length", "Streak length", 0.0, 100.0, 55.0, |e: &mut Self, v| e.streak_length = v),
            float_slider::<Self>("warmth", "Warmth", 0.0, 100.0, 45.0, |e: &mut Self, v| e.warmth = v),
            float_slider::<Self>("ghosting", "Ghosting", 0.0, 100.0, 35.0, |e: &mut Self, v| e.ghosting = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
