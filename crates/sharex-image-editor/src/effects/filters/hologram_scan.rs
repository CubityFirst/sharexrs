//! `HologramScanImageEffect` (`Core/ImageEffects/Filters/HologramScanImageEffect.cs`).

use crate::effects::prelude::*;

/// `HologramScanImageEffect`.
#[derive(Clone, Debug)]
pub struct HologramScanImageEffect {
    pub scanline_strength: f32,
    pub glitch_amount: f32,
    pub chroma_shift: i32,
    pub glow_amount: f32,
    pub seed: i32,
}

impl Default for HologramScanImageEffect {
    fn default() -> Self {
        HologramScanImageEffect {
            scanline_strength: 45.0,
            glitch_amount: 25.0,
            chroma_shift: 2,
            glow_amount: 30.0,
            seed: 6006,
        }
    }
}

impl EffectImpl for HologramScanImageEffect {
    fn info() -> Info {
        Info::new("hologram_scan", "Hologram scan", EffectCategory::Filters, "image-effect/scan-face")
            .desc("Applies a holographic scanline effect with glitch and glow.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("scanline_strength", "Scanline strength", 0.0, 100.0, 45.0, |e: &mut Self, v| e.scanline_strength = v),
            float_slider::<Self>("glitch_amount", "Glitch amount", 0.0, 100.0, 25.0, |e: &mut Self, v| e.glitch_amount = v),
            int_slider::<Self>("chroma_shift", "Chroma shift", 0, 12, 2, |e: &mut Self, v| e.chroma_shift = v),
            float_slider::<Self>("glow_amount", "Glow amount", 0.0, 100.0, 30.0, |e: &mut Self, v| e.glow_amount = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
