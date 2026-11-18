//! `JpegArtifactImageEffect` (`Core/ImageEffects/Filters/JpegArtifactImageEffect.cs`).

use crate::effects::prelude::*;

/// `JpegArtifactImageEffect`.
#[derive(Clone, Debug)]
pub struct JpegArtifactImageEffect {
    pub compression: f32,
    pub block_size: i32,
    pub chroma_bleed: f32,
    pub ringing: f32,
}

impl Default for JpegArtifactImageEffect {
    fn default() -> Self {
        JpegArtifactImageEffect {
            compression: 62.0,
            block_size: 8,
            chroma_bleed: 45.0,
            ringing: 28.0,
        }
    }
}

impl EffectImpl for JpegArtifactImageEffect {
    fn info() -> Info {
        Info::new("jpeg_artifact", "JPEG artifact", EffectCategory::Filters, "app/file-image")
            .desc("Simulates JPEG compression artifacts with blocking and ringing.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("compression", "Compression", 0.0, 100.0, 62.0, |e: &mut Self, v| e.compression = v),
            int_slider::<Self>("block_size", "Block size", 4, 32, 8, |e: &mut Self, v| e.block_size = v),
            float_slider::<Self>("chroma_bleed", "Chroma bleed", 0.0, 100.0, 45.0, |e: &mut Self, v| e.chroma_bleed = v),
            float_slider::<Self>("ringing", "Ringing", 0.0, 100.0, 28.0, |e: &mut Self, v| e.ringing = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
