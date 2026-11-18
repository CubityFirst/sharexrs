//! `InkSplatterDripsImageEffect` (`Core/ImageEffects/Filters/InkSplatterDripsImageEffect.cs`).

use crate::effects::prelude::*;

/// `InkSplatterDripsImageEffect`.
#[derive(Clone, Debug)]
pub struct InkSplatterDripsImageEffect {
    pub ink_amount: f32,
    pub drip_length: f32,
    pub spread: f32,
    pub paper_fade: f32,
}

impl Default for InkSplatterDripsImageEffect {
    fn default() -> Self {
        InkSplatterDripsImageEffect {
            ink_amount: 55.0,
            drip_length: 45.0,
            spread: 35.0,
            paper_fade: 20.0,
        }
    }
}

impl EffectImpl for InkSplatterDripsImageEffect {
    fn info() -> Info {
        Info::new("ink_splatter_drips", "Ink splatter + drips", EffectCategory::Filters, "image-effect/paint-brush")
            .desc("Adds ink splatter blobs and vertical drip marks.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("ink_amount", "Ink amount", 0.0, 100.0, 55.0, |e: &mut Self, v| e.ink_amount = v),
            float_slider::<Self>("drip_length", "Drip length", 0.0, 100.0, 45.0, |e: &mut Self, v| e.drip_length = v),
            float_slider::<Self>("spread", "Spread", 0.0, 100.0, 35.0, |e: &mut Self, v| e.spread = v),
            float_slider::<Self>("paper_fade", "Paper fade", 0.0, 100.0, 20.0, |e: &mut Self, v| e.paper_fade = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
