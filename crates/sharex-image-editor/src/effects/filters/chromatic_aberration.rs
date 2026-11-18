//! `ChromaticAberrationImageEffect` (`Core/ImageEffects/Filters/ChromaticAberrationImageEffect.cs`).

use crate::effects::prelude::*;

/// `ChromaticAberrationImageEffect`.
#[derive(Clone, Debug)]
pub struct ChromaticAberrationImageEffect {
    pub amount: f32,
    pub edge_start: f32,
    pub strength: f32,
}

impl Default for ChromaticAberrationImageEffect {
    fn default() -> Self {
        ChromaticAberrationImageEffect {
            amount: 8.0,
            edge_start: 0.2,
            strength: 75.0,
        }
    }
}

impl EffectImpl for ChromaticAberrationImageEffect {
    fn info() -> Info {
        Info::new("chromatic_aberration", "Chromatic aberration", EffectCategory::Filters, "image-effect/aperture")
            .desc("Simulates lens chromatic aberration with RGB channel splitting.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("amount", "Amount", 0.0, 40.0, 8.0, |e: &mut Self, v| e.amount = v),
            float_slider::<Self>("edgeStart", "Edge start", 0.0, 0.949999988079071, 0.20000000298023224, |e: &mut Self, v| e.edge_start = v),
            float_slider::<Self>("strength", "Strength", 0.0, 100.0, 75.0, |e: &mut Self, v| e.strength = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
