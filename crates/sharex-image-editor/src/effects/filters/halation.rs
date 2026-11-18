//! `HalationImageEffect` (`Core/ImageEffects/Filters/HalationImageEffect.cs`).

use crate::effects::prelude::*;

/// `HalationImageEffect`.
#[derive(Clone, Debug)]
pub struct HalationImageEffect {
    pub threshold: f32,
    pub radius: f32,
    pub strength: f32,
    pub warmth: f32,
}

impl Default for HalationImageEffect {
    fn default() -> Self {
        HalationImageEffect {
            threshold: 72.0,
            radius: 18.0,
            strength: 60.0,
            warmth: 70.0,
        }
    }
}

impl EffectImpl for HalationImageEffect {
    fn info() -> Info {
        Info::new("halation", "Halation", EffectCategory::Filters, "image-effect/sunrise")
            .desc("Simulates film halation where bright areas bleed warm light into surrounding regions.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("threshold", "Threshold", 0.0, 100.0, 72.0, |e: &mut Self, v| e.threshold = v),
            float_slider::<Self>("radius", "Radius", 1.0, 100.0, 18.0, |e: &mut Self, v| e.radius = v),
            float_slider::<Self>("strength", "Strength", 0.0, 150.0, 60.0, |e: &mut Self, v| e.strength = v),
            float_slider::<Self>("warmth", "Warmth", 0.0, 100.0, 70.0, |e: &mut Self, v| e.warmth = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
