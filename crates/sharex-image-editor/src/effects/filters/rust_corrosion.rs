//! `RustCorrosionImageEffect` (`Core/ImageEffects/Filters/RustCorrosionImageEffect.cs`).

use crate::effects::prelude::*;

/// `RustCorrosionImageEffect`.
#[derive(Clone, Debug)]
pub struct RustCorrosionImageEffect {
    pub corrosion: f32,
    pub pitting: f32,
    pub streaks: f32,
    pub dirt: f32,
    pub edge_wear: f32,
}

impl Default for RustCorrosionImageEffect {
    fn default() -> Self {
        RustCorrosionImageEffect {
            corrosion: 58.0,
            pitting: 44.0,
            streaks: 36.0,
            dirt: 26.0,
            edge_wear: 52.0,
        }
    }
}

impl EffectImpl for RustCorrosionImageEffect {
    fn info() -> Info {
        Info::new("rust_corrosion", "Rust / corrosion", EffectCategory::Filters, "app/clear-all")
            .desc("Simulates rust and corrosion effects with pitting and streaks.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("corrosion", "Corrosion", 0.0, 100.0, 58.0, |e: &mut Self, v| e.corrosion = v),
            float_slider::<Self>("pitting", "Pitting", 0.0, 100.0, 44.0, |e: &mut Self, v| e.pitting = v),
            float_slider::<Self>("streaks", "Streaks", 0.0, 100.0, 36.0, |e: &mut Self, v| e.streaks = v),
            float_slider::<Self>("dirt", "Dirt", 0.0, 100.0, 26.0, |e: &mut Self, v| e.dirt = v),
            float_slider::<Self>("edge_wear", "Edge wear", 0.0, 100.0, 52.0, |e: &mut Self, v| e.edge_wear = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
