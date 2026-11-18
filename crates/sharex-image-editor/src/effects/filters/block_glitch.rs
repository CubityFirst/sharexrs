//! `BlockGlitchImageEffect` (`Core/ImageEffects/Filters/BlockGlitchImageEffect.cs`).

use crate::effects::prelude::*;

/// `BlockGlitchImageEffect`.
#[derive(Clone, Debug)]
pub struct BlockGlitchImageEffect {
    pub block_count: i32,
    pub min_block_width: i32,
    pub max_block_width: i32,
    pub min_block_height: i32,
    pub max_block_height: i32,
    pub max_displacement: i32,
    pub channel_shift: i32,
    pub noise_amount: f32,
    pub seed: i32,
}

impl Default for BlockGlitchImageEffect {
    fn default() -> Self {
        BlockGlitchImageEffect {
            block_count: 36,
            min_block_width: 24,
            max_block_width: 200,
            min_block_height: 6,
            max_block_height: 50,
            max_displacement: 50,
            channel_shift: 4,
            noise_amount: 10.0,
            seed: 1945,
        }
    }
}

impl EffectImpl for BlockGlitchImageEffect {
    fn info() -> Info {
        Info::new("block_glitch", "Block glitch / Databending", EffectCategory::Filters, "image-effect/cpu")
            .desc("Simulates digital glitch artifacts with displaced color blocks.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("block_count", "Block count", 1, 240, 36, |e: &mut Self, v| e.block_count = v),
            int_slider::<Self>("min_block_width", "Min block width", 4, 400, 24, |e: &mut Self, v| e.min_block_width = v),
            int_slider::<Self>("max_block_width", "Max block width", 4, 900, 200, |e: &mut Self, v| e.max_block_width = v),
            int_slider::<Self>("min_block_height", "Min block height", 2, 200, 6, |e: &mut Self, v| e.min_block_height = v),
            int_slider::<Self>("max_block_height", "Max block height", 2, 500, 50, |e: &mut Self, v| e.max_block_height = v),
            int_slider::<Self>("max_displacement", "Max displacement", 0, 500, 50, |e: &mut Self, v| e.max_displacement = v),
            int_slider::<Self>("channel_shift", "Channel shift", 0, 64, 4, |e: &mut Self, v| e.channel_shift = v),
            float_slider::<Self>("noise_amount", "Noise amount", 0.0, 100.0, 10.0, |e: &mut Self, v| e.noise_amount = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
