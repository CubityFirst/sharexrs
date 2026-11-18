//! `CrystalPrismImageEffect` (`Core/ImageEffects/Filters/CrystalPrismImageEffect.cs`).

use crate::effects::prelude::*;

/// `CrystalPrismImageEffect`.
#[derive(Clone, Debug)]
pub struct CrystalPrismImageEffect {
    pub facet_size: i32,
    pub refraction: f32,
    pub dispersion: f32,
    pub sparkle: f32,
    pub seed: i32,
}

impl Default for CrystalPrismImageEffect {
    fn default() -> Self {
        CrystalPrismImageEffect {
            facet_size: 24,
            refraction: 8.0,
            dispersion: 4.0,
            sparkle: 25.0,
            seed: 4771,
        }
    }
}

impl EffectImpl for CrystalPrismImageEffect {
    fn info() -> Info {
        Info::new("crystal_prism", "Crystal prism", EffectCategory::Filters, "image-effect/gem")
            .desc("Refracts and disperses light through crystal-like facets.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("facetSize", "Facet size", 6, 96, 24, |e: &mut Self, v| e.facet_size = v),
            float_slider::<Self>("refraction", "Refraction", 0.0, 30.0, 8.0, |e: &mut Self, v| e.refraction = v),
            float_slider::<Self>("dispersion", "Dispersion", 0.0, 20.0, 4.0, |e: &mut Self, v| e.dispersion = v),
            float_slider::<Self>("sparkle", "Sparkle", 0.0, 100.0, 25.0, |e: &mut Self, v| e.sparkle = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
