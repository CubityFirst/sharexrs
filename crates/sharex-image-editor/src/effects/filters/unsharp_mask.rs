//! `UnsharpMaskImageEffect` (`Core/ImageEffects/Filters/UnsharpMaskImageEffect.cs`).

use crate::effects::prelude::*;

/// `UnsharpMaskImageEffect`.
#[derive(Clone, Debug)]
pub struct UnsharpMaskImageEffect {
    pub radius: f32,
    pub amount: f32,
    pub threshold: i32,
}

impl Default for UnsharpMaskImageEffect {
    fn default() -> Self {
        UnsharpMaskImageEffect {
            radius: 5.0,
            amount: 150.0,
            threshold: 0,
        }
    }
}

impl EffectImpl for UnsharpMaskImageEffect {
    fn info() -> Info {
        Info::new("unsharp_mask", "Unsharp mask", EffectCategory::Filters, "app/magic-wand")
            .desc("Sharpens the image using an unsharp mask technique.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("radius", "Radius", 1.0, 100.0, 5.0, |e: &mut Self, v| e.radius = v),
            float_slider::<Self>("amount", "Amount", 0.0, 500.0, 150.0, |e: &mut Self, v| e.amount = v),
            int_slider::<Self>("threshold", "Threshold", 0, 255, 0, |e: &mut Self, v| e.threshold = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
