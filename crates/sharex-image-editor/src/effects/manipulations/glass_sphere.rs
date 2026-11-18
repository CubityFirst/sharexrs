//! `GlassSphereImageEffect` (`Core/ImageEffects/Manipulations/GlassSphereImageEffect.cs`).

use crate::effects::prelude::*;

/// `GlassSphereImageEffect`.
#[derive(Clone, Debug)]
pub struct GlassSphereImageEffect {
    pub radius_percent: f32,
    pub refraction: f32,
}

impl Default for GlassSphereImageEffect {
    fn default() -> Self {
        GlassSphereImageEffect {
            radius_percent: 50.0,
            refraction: 1.5,
        }
    }
}

impl EffectImpl for GlassSphereImageEffect {
    fn info() -> Info {
        Info::new("glass_sphere", "Glass sphere", EffectCategory::Manipulations, "image-effect/sphere")
            .desc("Applies a spherical glass lens distortion at the center.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("radius", "Radius %", 10.0, 100.0, 50.0, |e: &mut Self, v| e.radius_percent = v),
            float_slider::<Self>("refraction", "Refraction", 0.1, 2.0, 1.5, |e: &mut Self, v| e.refraction = v).tick(0.05).snap(false).format("{}{0:0.00}"),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
