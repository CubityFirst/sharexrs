//! `DehazeImageEffect` (`Core/ImageEffects/Adjustments/DehazeImageEffect.cs`).

use crate::effects::prelude::*;

/// `DehazeImageEffect`.
#[derive(Clone, Debug)]
pub struct DehazeImageEffect {
    pub amount: f32,
}

impl Default for DehazeImageEffect {
    fn default() -> Self {
        DehazeImageEffect { amount: 50.0 }
    }
}

impl EffectImpl for DehazeImageEffect {
    fn info() -> Info {
        Info::new(
            "dehaze",
            "Dehaze",
            EffectCategory::Adjustments,
            "image-effect/sunset",
        )
        .desc("Removes or adds atmospheric haze.")
    }

    fn params() -> Vec<ParamDef> {
        vec![float_slider::<Self>(
            "amount",
            "Amount",
            -100.0,
            100.0,
            50.0,
            |e: &mut Self, v| e.amount = v,
        )]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let amount = clampf(self.amount, -100.0, 100.0) / 100.0;
        if amount.abs() < 0.001 {
            return Ok(source.clone());
        }
        let px = pixels(source);
        let count = px.len();
        let (mut sum_r, mut sum_g, mut sum_b) = (0f32, 0f32, 0f32);
        let mut atm_count = 0i32;
        let top_n = 1.max(count as i32 / 1000);
        let brightness: Vec<f32> = px
            .iter()
            .map(|c| (c.r as i32 + c.g as i32 + c.b as i32) as f32 / 3.0)
            .collect();
        let mut sorted = brightness.clone();
        sorted.sort_by(|a, b| a.total_cmp(b));
        let threshold = sorted[0.max(count as i32 - top_n) as usize];
        for (i, c) in px.iter().enumerate() {
            if brightness[i] >= threshold {
                sum_r += c.r as f32;
                sum_g += c.g as f32;
                sum_b += c.b as f32;
                atm_count += 1;
            }
        }
        let atm_r = if atm_count > 0 {
            sum_r / atm_count as f32
        } else {
            200.0
        };
        let atm_g = if atm_count > 0 {
            sum_g / atm_count as f32
        } else {
            200.0
        };
        let atm_b = if atm_count > 0 {
            sum_b / atm_count as f32
        } else {
            200.0
        };
        let result: Vec<SkColor> = px
            .iter()
            .map(|c| {
                let (mut r, mut g, mut b) = (c.r as f32, c.g as f32, c.b as f32);
                if amount > 0.0 {
                    let dark = (r / atm_r).min((g / atm_g).min(b / atm_b));
                    let mut t = 1.0 - (amount * dark);
                    t = t.max(0.1);
                    r = (r - atm_r * (1.0 - t)) / t;
                    g = (g - atm_g * (1.0 - t)) / t;
                    b = (b - atm_b * (1.0 - t)) / t;
                } else {
                    let haze = -amount;
                    r = r + (atm_r - r) * haze;
                    g = g + (atm_g - g) * haze;
                    b = b + (atm_b - b) * haze;
                }
                SkColor::rgba(clamp_to_byte(r), clamp_to_byte(g), clamp_to_byte(b), c.a)
            })
            .collect();
        Ok(from_pixels(source.width(), source.height(), &result))
    }
}
