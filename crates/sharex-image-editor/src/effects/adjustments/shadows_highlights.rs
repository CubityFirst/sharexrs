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
        Info::new(
            "shadows_highlights",
            "Shadows / Highlights",
            EffectCategory::Adjustments,
            "app/theme",
        )
        .desc("Adjusts shadows and highlights.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>(
                "shadows",
                "Shadows",
                -100.0,
                100.0,
                0.0,
                |e: &mut Self, v| e.shadows = v,
            ),
            float_slider::<Self>(
                "highlights",
                "Highlights",
                -100.0,
                100.0,
                0.0,
                |e: &mut Self, v| e.highlights = v,
            ),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let shadows = clampf(self.shadows, -100.0, 100.0);
        let highlights = clampf(self.highlights, -100.0, 100.0);
        if shadows.abs() < 0.0001 && highlights.abs() < 0.0001 {
            return Ok(source.clone());
        }
        let (ss, hs) = (shadows / 100.0, highlights / 100.0);
        Ok(apply_pixel_operation(source, |c| {
            let luma = (0.2126 * c.r as f32 + 0.7152 * c.g as f32 + 0.0722 * c.b as f32) / 255.0;
            let sw = (1.0 - luma) * (1.0 - luma);
            let hw = luma * luma;
            let delta = (ss * sw - hs * hw) * 255.0;
            SkColor::rgba(
                clamp_to_byte(c.r as f32 + delta),
                clamp_to_byte(c.g as f32 + delta),
                clamp_to_byte(c.b as f32 + delta),
                c.a,
            )
        }))
    }
}
