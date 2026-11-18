//! `WoodcutPrintImageEffect` (`Core/ImageEffects/Filters/WoodcutPrintImageEffect.cs`).

use crate::effects::prelude::*;

/// `WoodcutPrintImageEffect`.
#[derive(Clone, Debug)]
pub struct WoodcutPrintImageEffect {
    pub threshold: f32,
    pub edge_strength: f32,
    pub grain_amount: f32,
    pub ink_color: SkColor,
    pub paper_color: SkColor,
}

impl Default for WoodcutPrintImageEffect {
    fn default() -> Self {
        WoodcutPrintImageEffect {
            threshold: 50.0,
            edge_strength: 60.0,
            grain_amount: 30.0,
            ink_color: SkColor::argb(0xFF0F0A05),
            paper_color: SkColor::argb(0xFFEBE1C8),
        }
    }
}

impl EffectImpl for WoodcutPrintImageEffect {
    fn info() -> Info {
        Info::new("woodcut_print", "Woodcut Print", EffectCategory::Filters, "image-effect/axe")
            .desc("Simulates a woodcut or linocut print style.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("threshold", "Threshold", 0.0, 100.0, 50.0, |e: &mut Self, v| e.threshold = v),
            float_slider::<Self>("edge_strength", "Edge Strength", 0.0, 100.0, 60.0, |e: &mut Self, v| e.edge_strength = v),
            float_slider::<Self>("grain_amount", "Wood Grain", 0.0, 100.0, 30.0, |e: &mut Self, v| e.grain_amount = v),
            color_param::<Self>("ink_color", "Ink Color", SkColor::argb(0xFF0F0A05), |e: &mut Self, v| e.ink_color = v),
            color_param::<Self>("paper_color", "Paper Color", SkColor::argb(0xFFEBE1C8), |e: &mut Self, v| e.paper_color = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
