//! `ExposureImageEffect` (`Core/ImageEffects/Adjustments/ExposureImageEffect.cs`).

use crate::effects::prelude::*;

/// `ExposureImageEffect`.
#[derive(Clone, Debug)]
pub struct ExposureImageEffect {
    pub amount: f32,
}

impl Default for ExposureImageEffect {
    fn default() -> Self {
        ExposureImageEffect {
            amount: 0.0,
        }
    }
}

impl EffectImpl for ExposureImageEffect {
    fn info() -> Info {
        Info::new("exposure", "Exposure", EffectCategory::Adjustments, "image-effect/aperture")
            .desc("Adjusts the exposure level.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("amount", "Exposure", -100.0, 100.0, 0.0, |e: &mut Self, v| e.amount = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let amount = clampf(self.amount, -10.0, 10.0);
        if amount.abs() < 0.0001 {
            return Ok(source.clone());
        }
        let gain = 2f32.powf(amount);
        Ok(apply_pixel_operation(source, |c| {
            SkColor::rgba(
                clamp_to_byte(c.r as f32 * gain),
                clamp_to_byte(c.g as f32 * gain),
                clamp_to_byte(c.b as f32 * gain),
                c.a,
            )
        }))
    }
}
