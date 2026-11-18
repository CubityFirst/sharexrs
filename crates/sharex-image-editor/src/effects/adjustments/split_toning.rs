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
        let strength = clampf(self.strength, 0.0, 100.0) / 100.0;
        if strength < 0.001 {
            return Ok(source.clone());
        }
        let balance = clampf(self.balance, -100.0, 100.0) / 200.0 + 0.5;
        let (s_r, s_g, s_b) = (
            self.shadow_tint.r as f32 / 255.0,
            self.shadow_tint.g as f32 / 255.0,
            self.shadow_tint.b as f32 / 255.0,
        );
        let (h_r, h_g, h_b) = (
            self.highlight_tint.r as f32 / 255.0,
            self.highlight_tint.g as f32 / 255.0,
            self.highlight_tint.b as f32 / 255.0,
        );
        Ok(apply_pixel_operation(source, |c| {
            let mut r = c.r as f32 / 255.0;
            let mut g = c.g as f32 / 255.0;
            let mut b = c.b as f32 / 255.0;
            let lum = 0.299 * r + 0.587 * g + 0.114 * b;
            let sw = 1.0 - split_smooth_step(0.0, balance, lum);
            let hw = split_smooth_step(balance, 1.0, lum);
            let tr = s_r * sw + h_r * hw;
            let tg = s_g * sw + h_g * hw;
            let tb = s_b * sw + h_b * hw;
            r = lerp(r, soft_light(r, tr), strength);
            g = lerp(g, soft_light(g, tg), strength);
            b = lerp(b, soft_light(b, tb), strength);
            SkColor::rgba(clamp_to_byte(r * 255.0), clamp_to_byte(g * 255.0), clamp_to_byte(b * 255.0), c.a)
        }))
    }
}

// ---- port ----

fn soft_light(base: f32, blend: f32) -> f32 {
    (1.0 - 2.0 * blend) * base * base + 2.0 * blend * base
}

fn split_smooth_step(edge0: f32, edge1: f32, x: f32) -> f32 {
    let range = edge1 - edge0;
    if range <= 0.0 {
        return if x >= edge1 { 1.0 } else { 0.0 };
    }
    let t = clampf((x - edge0) / range, 0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
