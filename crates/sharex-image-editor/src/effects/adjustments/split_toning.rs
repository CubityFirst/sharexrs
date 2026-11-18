//! `SplitToningImageEffect` (`Core/ImageEffects/Adjustments/SplitToningImageEffect.cs`).

use crate::effects::prelude::*;

/// `SplitToningImageEffect`.
#[derive(Clone, Debug)]
pub struct SplitToningImageEffect {
    pub shadow_tint: SkColor,
    pub highlight_tint: SkColor,
    pub strength: f32,
    pub balance: f32,
}

impl Default for SplitToningImageEffect {
    fn default() -> Self {
        SplitToningImageEffect {
            shadow_tint: SkColor::argb(0xFF283278),
            highlight_tint: SkColor::argb(0xFFC8AA64),
            strength: 30.0,
            balance: 0.0,
        }
    }
}

impl EffectImpl for SplitToningImageEffect {
    fn info() -> Info {
        Info::new("split_toning", "Split Toning", EffectCategory::Adjustments, "app/blend")
            .desc("Tints shadows and highlights with different colors.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            color_param::<Self>("shadow_tint", "Shadow Tint", SkColor::argb(0xFF283278), |e: &mut Self, v| e.shadow_tint = v),
            color_param::<Self>("highlight_tint", "Highlight Tint", SkColor::argb(0xFFC8AA64), |e: &mut Self, v| e.highlight_tint = v),
            float_slider::<Self>("strength", "Strength", 0.0, 100.0, 30.0, |e: &mut Self, v| e.strength = v),
            float_slider::<Self>("balance", "Balance", -100.0, 100.0, 0.0, |e: &mut Self, v| e.balance = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
