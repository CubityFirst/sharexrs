//! `CrystalizeShardsImageEffect` (`Core/ImageEffects/Filters/CrystalizeShardsImageEffect.cs`).

use crate::effects::prelude::*;

/// `CrystalizeShardsImageEffect`.
#[derive(Clone, Debug)]
pub struct CrystalizeShardsImageEffect {
    pub shard_size: i32,
    pub jitter: f32,
    pub edge_strength: f32,
    pub shine: f32,
    pub seed: i32,
}

impl Default for CrystalizeShardsImageEffect {
    fn default() -> Self {
        CrystalizeShardsImageEffect {
            shard_size: 22,
            jitter: 65.0,
            edge_strength: 75.0,
            shine: 30.0,
            seed: 4242,
        }
    }
}

impl EffectImpl for CrystalizeShardsImageEffect {
    fn info() -> Info {
        Info::new("crystalize_shards", "Crystalize shards", EffectCategory::Filters, "image-effect/diamond")
            .desc("Breaks the image into crystalline Voronoi shards with edge highlights.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("shardSize", "Shard size", 6, 80, 22, |e: &mut Self, v| e.shard_size = v),
            float_slider::<Self>("jitter", "Jitter", 0.0, 100.0, 65.0, |e: &mut Self, v| e.jitter = v),
            float_slider::<Self>("edgeStrength", "Edge strength", 0.0, 100.0, 75.0, |e: &mut Self, v| e.edge_strength = v),
            float_slider::<Self>("shine", "Shine", 0.0, 100.0, 30.0, |e: &mut Self, v| e.shine = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
