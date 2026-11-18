//! `VibranceImageEffect` (`Core/ImageEffects/Adjustments/VibranceImageEffect.cs`).

use crate::effects::prelude::*;

/// `VibranceImageEffect`.
#[derive(Clone, Debug)]
pub struct VibranceImageEffect {
    pub amount: f32,
}

impl Default for VibranceImageEffect {
    fn default() -> Self {
        VibranceImageEffect { amount: 25.0 }
    }
}

impl EffectImpl for VibranceImageEffect {
    fn info() -> Info {
        Info::new(
            "vibrance",
            "Vibrance",
            EffectCategory::Adjustments,
            "app/sparkles",
        )
        .desc("Adjusts the color vibrance.")
    }

    fn params() -> Vec<ParamDef> {
        vec![float_slider::<Self>(
            "amount",
            "Amount",
            -100.0,
            100.0,
            0.0,
            |e: &mut Self, v| e.amount = v,
        )]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let amount = clampf(self.amount, -100.0, 100.0) / 100.0;
        if amount.abs() < 0.0001 {
            return Ok(source.clone());
        }
        Ok(apply_pixel_operation(source, |c| {
            let (mut r, mut g, mut b) = (c.r as f32, c.g as f32, c.b as f32);
            let max = r.max(g.max(b));
            let min = r.min(g.min(b));
            let saturation = (max - min) / 255.0;
            let gray = (r + g + b) / 3.0;
            let factor = if amount >= 0.0 {
                1.0 + (amount * (1.0 - saturation))
            } else {
                1.0 + amount
            };
            r = gray + ((r - gray) * factor);
            g = gray + ((g - gray) * factor);
            b = gray + ((b - gray) * factor);
            SkColor::rgba(clamp_to_byte(r), clamp_to_byte(g), clamp_to_byte(b), c.a)
        }))
    }
}
