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
        Info::new("temperature_tint", "Temperature / Tint", EffectCategory::Adjustments, "image-effect/thermometer")
            .desc("Adjusts the color temperature and tint.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("temperature", "Temperature", -100.0, 100.0, 0.0, |e: &mut Self, v| e.temperature = v),
            float_slider::<Self>("tint", "Tint", -100.0, 100.0, 0.0, |e: &mut Self, v| e.tint = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
