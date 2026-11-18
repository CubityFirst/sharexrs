//! `BloomImageEffect` (`Core/ImageEffects/Filters/BloomImageEffect.cs`).

use crate::effects::prelude::*;

/// `BloomImageEffect`.
#[derive(Clone, Debug)]
pub struct BloomImageEffect {
    pub threshold: f32,
    pub soft_knee: f32,
    pub radius: f32,
    pub intensity: f32,
}

impl Default for BloomImageEffect {
    fn default() -> Self {
        BloomImageEffect {
            threshold: 65.0,
            soft_knee: 35.0,
            radius: 24.0,
            intensity: 85.0,
        }
    }
}

impl EffectImpl for BloomImageEffect {
    fn info() -> Info {
        Info::new("bloom", "Bloom", EffectCategory::Filters, "image-effect/sparkle")
            .desc("Adds a soft glow bloom effect to bright areas.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("threshold", "Threshold", 0.0, 100.0, 65.0, |e: &mut Self, v| e.threshold = v),
            float_slider::<Self>("soft_knee", "Soft knee", 0.0, 100.0, 35.0, |e: &mut Self, v| e.soft_knee = v),
            float_slider::<Self>("radius", "Radius", 1.0, 100.0, 24.0, |e: &mut Self, v| e.radius = v),
            float_slider::<Self>("intensity", "Intensity", 0.0, 200.0, 85.0, |e: &mut Self, v| e.intensity = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
