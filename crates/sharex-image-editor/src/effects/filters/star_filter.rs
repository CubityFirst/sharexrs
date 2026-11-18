//! `StarFilterImageEffect` (`Core/ImageEffects/Filters/StarFilterImageEffect.cs`).

use crate::effects::prelude::*;

/// `StarFilterImageEffect`.
#[derive(Clone, Debug)]
pub struct StarFilterImageEffect {
    pub threshold: f32,
    pub length: f32,
    pub strength: f32,
    pub rotation: f32,
    pub warmth: f32,
}

impl Default for StarFilterImageEffect {
    fn default() -> Self {
        StarFilterImageEffect {
            threshold: 74.0,
            length: 40.0,
            strength: 58.0,
            rotation: 0.0,
            warmth: 48.0,
        }
    }
}

impl EffectImpl for StarFilterImageEffect {
    fn info() -> Info {
        Info::new("star_filter", "Star filter", EffectCategory::Filters, "app/star")
            .desc("Adds star-shaped light streaks to bright areas.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("threshold", "Threshold", 0.0, 100.0, 74.0, |e: &mut Self, v| e.threshold = v),
            float_slider::<Self>("length", "Length", 4.0, 80.0, 40.0, |e: &mut Self, v| e.length = v),
            float_slider::<Self>("strength", "Strength", 0.0, 100.0, 58.0, |e: &mut Self, v| e.strength = v),
            float_slider::<Self>("rotation", "Rotation", 0.0, 360.0, 0.0, |e: &mut Self, v| e.rotation = v),
            float_slider::<Self>("warmth", "Warmth", 0.0, 100.0, 48.0, |e: &mut Self, v| e.warmth = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
