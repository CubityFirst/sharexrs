//! `ClaymationTextureImageEffect` (`Core/ImageEffects/Filters/ClaymationTextureImageEffect.cs`).

use crate::effects::prelude::*;

/// `ClaymationTextureImageEffect`.
#[derive(Clone, Debug)]
pub struct ClaymationTextureImageEffect {
    pub chunk_size: i32,
    pub smoothness: f32,
    pub relief: f32,
    pub saturation: f32,
    pub texture_grain: f32,
}

impl Default for ClaymationTextureImageEffect {
    fn default() -> Self {
        ClaymationTextureImageEffect {
            chunk_size: 8,
            smoothness: 55.0,
            relief: 35.0,
            saturation: 20.0,
            texture_grain: 30.0,
        }
    }
}

impl EffectImpl for ClaymationTextureImageEffect {
    fn info() -> Info {
        Info::new("claymation_texture", "Claymation texture", EffectCategory::Filters, "image-effect/paint-roller")
            .desc("Gives the image a sculpted claymation look with chunky quantization and relief texture.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("chunkSize", "Chunk size", 3, 18, 8, |e: &mut Self, v| e.chunk_size = v),
            float_slider::<Self>("smoothness", "Smoothness", 0.0, 100.0, 55.0, |e: &mut Self, v| e.smoothness = v),
            float_slider::<Self>("relief", "Relief", 0.0, 100.0, 35.0, |e: &mut Self, v| e.relief = v),
            float_slider::<Self>("saturation", "Saturation", -100.0, 100.0, 20.0, |e: &mut Self, v| e.saturation = v),
            float_slider::<Self>("textureGrain", "Texture grain", 0.0, 100.0, 30.0, |e: &mut Self, v| e.texture_grain = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
