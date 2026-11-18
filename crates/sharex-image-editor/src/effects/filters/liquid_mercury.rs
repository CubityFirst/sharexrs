//! `LiquidMercuryImageEffect` (`Core/ImageEffects/Filters/LiquidMercuryImageEffect.cs`).

use crate::effects::prelude::*;

/// `LiquidMercuryImageEffect`.
#[derive(Clone, Debug)]
pub struct LiquidMercuryImageEffect {
    pub reflection: f32,
    pub ripple: f32,
    pub shine: f32,
    pub fluidity: f32,
    pub depth: f32,
}

impl Default for LiquidMercuryImageEffect {
    fn default() -> Self {
        LiquidMercuryImageEffect {
            reflection: 78.0,
            ripple: 42.0,
            shine: 82.0,
            fluidity: 55.0,
            depth: 65.0,
        }
    }
}

impl EffectImpl for LiquidMercuryImageEffect {
    fn info() -> Info {
        Info::new("liquid_mercury", "Liquid mercury", EffectCategory::Filters, "image-effect/droplet")
            .desc("Transforms the image into a reflective liquid mercury surface.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("reflection", "Reflection", 0.0, 100.0, 78.0, |e: &mut Self, v| e.reflection = v),
            float_slider::<Self>("ripple", "Ripple", 0.0, 100.0, 42.0, |e: &mut Self, v| e.ripple = v),
            float_slider::<Self>("shine", "Shine", 0.0, 100.0, 82.0, |e: &mut Self, v| e.shine = v),
            float_slider::<Self>("fluidity", "Fluidity", 0.0, 100.0, 55.0, |e: &mut Self, v| e.fluidity = v),
            float_slider::<Self>("depth", "Depth", 0.0, 100.0, 65.0, |e: &mut Self, v| e.depth = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
