//! `ClarityImageEffect` (`Core/ImageEffects/Adjustments/ClarityImageEffect.cs`).

use crate::effects::prelude::*;

/// `ClarityImageEffect`.
#[derive(Clone, Debug)]
pub struct ClarityImageEffect {
    pub amount: f32,
}

impl Default for ClarityImageEffect {
    fn default() -> Self {
        ClarityImageEffect { amount: 0.0 }
    }
}

impl EffectImpl for ClarityImageEffect {
    fn info() -> Info {
        Info::new("clarity", "Clarity", EffectCategory::Adjustments, "app/eye")
            .desc("Enhances midtone contrast for added depth and punch.")
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
        if amount.abs() < 0.001 {
            return Ok(source.clone());
        }
        let (width, height) = (source.width() as i32, source.height() as i32);
        let blur_radius = (width.max(height) as f32 * 0.02).max(3.0);
        let blurred = blur(source, blur_radius, blur_radius);
        let src = pixels(source);
        let bl = pixels(&blurred);
        let result: Vec<SkColor> = src
            .iter()
            .zip(&bl)
            .map(|(s, b)| {
                let (sr, sg, sb) = (s.r as f32 / 255.0, s.g as f32 / 255.0, s.b as f32 / 255.0);
                let (br, bg, bb) = (b.r as f32 / 255.0, b.g as f32 / 255.0, b.b as f32 / 255.0);
                let lum = 0.299 * sr + 0.587 * sg + 0.114 * sb;
                let diff = lum - 0.5;
                let midtone = (-8.0 * diff * diff).exp();
                let (dr, dg, db) = (sr - br, sg - bg, sb - bb);
                let strength = amount * midtone * 1.5;
                let out_r = clampf(sr + dr * strength, 0.0, 1.0);
                let out_g = clampf(sg + dg * strength, 0.0, 1.0);
                let out_b = clampf(sb + db * strength, 0.0, 1.0);
                SkColor::rgba(
                    (out_r * 255.0).round_ties_even() as u8,
                    (out_g * 255.0).round_ties_even() as u8,
                    (out_b * 255.0).round_ties_even() as u8,
                    s.a,
                )
            })
            .collect();
        Ok(from_pixels(width as u32, height as u32, &result))
    }
}
