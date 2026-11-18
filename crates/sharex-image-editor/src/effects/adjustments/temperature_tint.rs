//! `TemperatureTintImageEffect` (`Core/ImageEffects/Adjustments/TemperatureTintImageEffect.cs`).

use crate::effects::prelude::*;

/// `TemperatureTintImageEffect`.
#[derive(Clone, Debug)]
pub struct TemperatureTintImageEffect {
    pub temperature: f32,
    pub tint: f32,
}

impl Default for TemperatureTintImageEffect {
    fn default() -> Self {
        TemperatureTintImageEffect {
            temperature: 0.0,
            tint: 0.0,
        }
    }
}

impl EffectImpl for TemperatureTintImageEffect {
    fn info() -> Info {
        Info::new(
            "temperature_tint",
            "Temperature / Tint",
            EffectCategory::Adjustments,
            "image-effect/thermometer",
        )
        .desc("Adjusts the color temperature and tint.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>(
                "temperature",
                "Temperature",
                -100.0,
                100.0,
                0.0,
                |e: &mut Self, v| e.temperature = v,
            ),
            float_slider::<Self>("tint", "Tint", -100.0, 100.0, 0.0, |e: &mut Self, v| {
                e.tint = v
            }),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let temperature = clampf(self.temperature, -100.0, 100.0);
        let tint = clampf(self.tint, -100.0, 100.0);
        if temperature.abs() < 0.0001 && tint.abs() < 0.0001 {
            return Ok(source.clone());
        }
        let temp_delta = temperature / 100.0 * 64.0;
        let tint_delta = tint / 100.0 * 64.0;
        Ok(apply_pixel_operation(source, |c| {
            let r = c.r as f32 + temp_delta - tint_delta * 0.25;
            let g = c.g as f32 + tint_delta;
            let b = c.b as f32 - temp_delta - tint_delta * 0.25;
            SkColor::rgba(clamp_to_byte(r), clamp_to_byte(g), clamp_to_byte(b), c.a)
        }))
    }
}
