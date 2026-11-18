//! `VhsTapeDamageImageEffect` (`Core/ImageEffects/Filters/VhsTapeDamageImageEffect.cs`).

use crate::effects::prelude::*;

/// `VhsTapeDamageImageEffect`.
#[derive(Clone, Debug)]
pub struct VhsTapeDamageImageEffect {
    pub distortion: f32,
    pub noise: f32,
    pub color_bleed: f32,
    pub tracking: f32,
    pub scanlines: f32,
}

impl Default for VhsTapeDamageImageEffect {
    fn default() -> Self {
        VhsTapeDamageImageEffect {
            distortion: 55.0,
            noise: 28.0,
            color_bleed: 30.0,
            tracking: 24.0,
            scanlines: 48.0,
        }
    }
}

impl EffectImpl for VhsTapeDamageImageEffect {
    fn info() -> Info {
        Info::new("vhs_tape_damage", "VHS tape damage", EffectCategory::Filters, "image-effect/videotape")
            .desc("Simulates VHS tape degradation with distortion, noise, and color bleeding.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("distortion", "Distortion", 0.0, 100.0, 55.0, |e: &mut Self, v| e.distortion = v),
            float_slider::<Self>("noise", "Noise", 0.0, 100.0, 28.0, |e: &mut Self, v| e.noise = v),
            float_slider::<Self>("color_bleed", "Color bleed", 0.0, 100.0, 30.0, |e: &mut Self, v| e.color_bleed = v),
            float_slider::<Self>("tracking", "Tracking", 0.0, 100.0, 24.0, |e: &mut Self, v| e.tracking = v),
            float_slider::<Self>("scanlines", "Scanlines", 0.0, 100.0, 48.0, |e: &mut Self, v| e.scanlines = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
