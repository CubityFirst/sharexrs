//! `LiquidGlassImageEffect` (`Core/ImageEffects/Filters/LiquidGlassImageEffect.cs`).

use crate::effects::prelude::*;

/// `LiquidGlassImageEffect`.
#[derive(Clone, Debug)]
pub struct LiquidGlassImageEffect {
    pub distortion: f32,
    pub refraction: f32,
    pub chroma_shift: i32,
    pub gloss: f32,
    pub flow_scale: f32,
    pub seed: i32,
}

impl Default for LiquidGlassImageEffect {
    fn default() -> Self {
        LiquidGlassImageEffect {
            distortion: 9.0,
            refraction: 45.0,
            chroma_shift: 1,
            gloss: 40.0,
            flow_scale: 100.0,
            seed: 9421,
        }
    }
}

impl EffectImpl for LiquidGlassImageEffect {
    fn info() -> Info {
        Info::new("liquid_glass", "Liquid glass", EffectCategory::Filters, "image-effect/glass")
            .desc("Distorts the image as if viewed through liquid glass.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("distortion", "Distortion", 0.0, 35.0, 9.0, |e: &mut Self, v| e.distortion = v),
            float_slider::<Self>("refraction", "Refraction", 0.0, 100.0, 45.0, |e: &mut Self, v| e.refraction = v),
            int_slider::<Self>("chroma_shift", "Chroma shift", 0, 8, 1, |e: &mut Self, v| e.chroma_shift = v),
            float_slider::<Self>("gloss", "Gloss", 0.0, 100.0, 40.0, |e: &mut Self, v| e.gloss = v),
            float_slider::<Self>("flow_scale", "Flow scale", 40.0, 220.0, 100.0, |e: &mut Self, v| e.flow_scale = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
