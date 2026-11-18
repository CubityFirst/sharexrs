//! `CRTImageEffect` (`Core/ImageEffects/Filters/CRTImageEffect.cs`).

use crate::effects::prelude::*;

/// `CRTImageEffect`.
#[derive(Clone, Debug)]
pub struct CRTImageEffect {
    pub scanline_strength: f32,
    pub rgb_shift: i32,
    pub noise_amount: f32,
    pub vignette_strength: f32,
    pub seed: i32,
}

impl Default for CRTImageEffect {
    fn default() -> Self {
        CRTImageEffect {
            scanline_strength: 35.0,
            rgb_shift: 1,
            noise_amount: 8.0,
            vignette_strength: 18.0,
            seed: 1337,
        }
    }
}

impl EffectImpl for CRTImageEffect {
    fn info() -> Info {
        Info::new("crt", "CRT", EffectCategory::Filters, "app/monitor")
            .desc("Simulates a CRT monitor with scanlines, RGB shift, noise, and vignette.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("scanlineStrength", "Scanline strength", 0.0, 100.0, 35.0, |e: &mut Self, v| e.scanline_strength = v),
            int_slider::<Self>("rgbShift", "RGB shift", 0, 8, 1, |e: &mut Self, v| e.rgb_shift = v),
            float_slider::<Self>("noiseAmount", "Noise amount", 0.0, 100.0, 8.0, |e: &mut Self, v| e.noise_amount = v),
            float_slider::<Self>("vignetteStrength", "Vignette strength", 0.0, 100.0, 18.0, |e: &mut Self, v| e.vignette_strength = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
