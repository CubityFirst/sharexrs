//! `TVStaticImageEffect` (`Core/ImageEffects/Filters/TVStaticImageEffect.cs`).

use crate::effects::prelude::*;

/// `TVStaticImageEffect`.
#[derive(Clone, Debug)]
pub struct TVStaticImageEffect {
    pub amount: f32,
    pub seed: i32,
}

impl Default for TVStaticImageEffect {
    fn default() -> Self {
        TVStaticImageEffect {
            amount: 40.0,
            seed: 42,
        }
    }
}

impl EffectImpl for TVStaticImageEffect {
    fn info() -> Info {
        Info::new("tv_static", "TV static", EffectCategory::Filters, "image-effect/tv")
            .desc("Blends black-and-white TV static noise over the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("amount", "Amount", 0.0, 100.0, 40.0, |e: &mut Self, v| e.amount = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
