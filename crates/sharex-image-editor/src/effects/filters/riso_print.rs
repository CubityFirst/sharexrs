//! `RisoPrintImageEffect` (`Core/ImageEffects/Filters/RisoPrintImageEffect.cs`).

use crate::effects::prelude::*;

/// `RisoPrintImageEffect`.
#[derive(Clone, Debug)]
pub struct RisoPrintImageEffect {
    pub ink_strength: f32,
    pub paper_fade: f32,
    pub offset: f32,
    pub dot_scale: f32,
    pub ink_noise: f32,
    pub seed: i32,
    pub ink_color_a: SkColor,
    pub ink_color_b: SkColor,
}

impl Default for RisoPrintImageEffect {
    fn default() -> Self {
        RisoPrintImageEffect {
            ink_strength: 70.0,
            paper_fade: 25.0,
            offset: 3.0,
            dot_scale: 18.0,
            ink_noise: 35.0,
            seed: 2026,
            ink_color_a: SkColor::argb(0xFFDC4646),
            ink_color_b: SkColor::argb(0xFF46C8D2),
        }
    }
}

impl EffectImpl for RisoPrintImageEffect {
    fn info() -> Info {
        Info::new("riso_print", "Riso print", EffectCategory::Filters, "app/printer")
            .desc("Simulates a risograph print with halftone dots and ink colors.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("ink_strength", "Ink strength", 0.0, 100.0, 70.0, |e: &mut Self, v| e.ink_strength = v),
            float_slider::<Self>("paper_fade", "Paper fade", 0.0, 100.0, 25.0, |e: &mut Self, v| e.paper_fade = v),
            float_slider::<Self>("offset", "Offset", -200.0, 200.0, 3.0, |e: &mut Self, v| e.offset = v),
            float_slider::<Self>("dot_scale", "Dot scale", 0.0, 100.0, 18.0, |e: &mut Self, v| e.dot_scale = v),
            float_slider::<Self>("ink_noise", "Ink noise", 0.0, 100.0, 35.0, |e: &mut Self, v| e.ink_noise = v),
            color_param::<Self>("ink_color_a", "Ink color A", SkColor::argb(0xFFDC4646), |e: &mut Self, v| e.ink_color_a = v),
            color_param::<Self>("ink_color_b", "Ink color B", SkColor::argb(0xFF46C8D2), |e: &mut Self, v| e.ink_color_b = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
