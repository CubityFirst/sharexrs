//! `SoftDiffusionImageEffect` (`Core/ImageEffects/Filters/SoftDiffusionImageEffect.cs`).

use crate::effects::prelude::*;

/// `SoftDiffusionImageEffect`.
#[derive(Clone, Debug)]
pub struct SoftDiffusionImageEffect {
    pub amount: f32,
    pub radius: f32,
    pub highlight_bloom: f32,
    pub contrast_softening: f32,
    pub warmth: f32,
}

impl Default for SoftDiffusionImageEffect {
    fn default() -> Self {
        SoftDiffusionImageEffect {
            amount: 52.0,
            radius: 12.0,
            highlight_bloom: 40.0,
            contrast_softening: 28.0,
            warmth: 22.0,
        }
    }
}

impl EffectImpl for SoftDiffusionImageEffect {
    fn info() -> Info {
        Info::new("soft_diffusion", "Soft diffusion", EffectCategory::Filters, "image-effect/soft-glow")
            .desc("Applies a soft diffusion glow with bloom and warmth.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("amount", "Amount", 0.0, 100.0, 52.0, |e: &mut Self, v| e.amount = v),
            float_slider::<Self>("radius", "Radius", 0.0, 30.0, 12.0, |e: &mut Self, v| e.radius = v),
            float_slider::<Self>("highlight_bloom", "Highlight bloom", 0.0, 100.0, 40.0, |e: &mut Self, v| e.highlight_bloom = v),
            float_slider::<Self>("contrast_softening", "Contrast softening", 0.0, 100.0, 28.0, |e: &mut Self, v| e.contrast_softening = v),
            float_slider::<Self>("warmth", "Warmth", 0.0, 100.0, 22.0, |e: &mut Self, v| e.warmth = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
