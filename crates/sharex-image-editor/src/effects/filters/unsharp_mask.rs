//! `UnsharpMaskImageEffect` (`Core/ImageEffects/Filters/UnsharpMaskImageEffect.cs`).

use crate::effects::prelude::*;

/// `UnsharpMaskImageEffect`.
#[derive(Clone, Debug)]
pub struct UnsharpMaskImageEffect {
    pub radius: f32,
    pub amount: f32,
    pub threshold: i32,
}

impl Default for UnsharpMaskImageEffect {
    fn default() -> Self {
        UnsharpMaskImageEffect {
            radius: 5.0,
            amount: 150.0,
            threshold: 0,
        }
    }
}

impl EffectImpl for UnsharpMaskImageEffect {
    fn info() -> Info {
        Info::new("unsharp_mask", "Unsharp mask", EffectCategory::Filters, "app/magic-wand")
            .desc("Sharpens the image using an unsharp mask technique.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("radius", "Radius", 1.0, 100.0, 5.0, |e: &mut Self, v| e.radius = v),
            float_slider::<Self>("amount", "Amount", 0.0, 500.0, 150.0, |e: &mut Self, v| e.amount = v),
            int_slider::<Self>("threshold", "Threshold", 0, 255, 0, |e: &mut Self, v| e.threshold = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let radius = clampf(self.radius, 1.0, 100.0);
        let amount = clampf(self.amount, 0.0, 500.0) / 100.0;
        let threshold = self.threshold.clamp(0, 255);
        if amount <= 0.0 {
            return Ok(source.clone());
        }
        let blurred = blur(source, radius / 3.0, radius / 3.0);
        let ch = |o: u8, b: u8| -> u8 {
            let diff = o as i32 - b as i32;
            if diff.abs() < threshold {
                return o;
            }
            let v = o as f32 + (diff as f32 * amount);
            if v <= 0.0 {
                0
            } else if v >= 255.0 {
                255
            } else {
                v.round_ties_even() as u8
            }
        };
        let dst: Vec<SkColor> = pixels(source)
            .iter()
            .zip(pixels(&blurred).iter())
            .map(|(o, b)| SkColor::rgba(ch(o.r, b.r), ch(o.g, b.g), ch(o.b, b.b), o.a))
            .collect();
        Ok(from_pixels(source.width(), source.height(), &dst))
    }
}
