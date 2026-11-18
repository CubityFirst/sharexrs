//! `ShadowsHighlightsImageEffect` (`Core/ImageEffects/Adjustments/ShadowsHighlightsImageEffect.cs`).

use crate::effects::prelude::*;

/// `ShadowsHighlightsImageEffect`.
#[derive(Clone, Debug)]
pub struct ShadowsHighlightsImageEffect {
    pub shadows: f32,
    pub highlights: f32,
}

impl Default for ShadowsHighlightsImageEffect {
    fn default() -> Self {
        ShadowsHighlightsImageEffect {
            shadows: 0.0,
            highlights: 0.0,
        }
    }
}

impl EffectImpl for ShadowsHighlightsImageEffect {
    fn info() -> Info {
        Info::new("shadows_highlights", "Shadows / Highlights", EffectCategory::Adjustments, "app/theme")
            .desc("Adjusts shadows and highlights.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("shadows", "Shadows", -100.0, 100.0, 0.0, |e: &mut Self, v| e.shadows = v),
            float_slider::<Self>("highlights", "Highlights", -100.0, 100.0, 0.0, |e: &mut Self, v| e.highlights = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
