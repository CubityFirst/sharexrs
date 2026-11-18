//! `CyanotypeImageEffect` (`Core/ImageEffects/Filters/CyanotypeImageEffect.cs`).

use crate::effects::prelude::*;

/// `CyanotypeImageEffect`.
#[derive(Clone, Debug)]
pub struct CyanotypeImageEffect {
    pub contrast: f32,
    pub paper_texture: f32,
    pub stain: f32,
    pub grain: f32,
    pub vignette: f32,
    pub seed: i32,
}

impl Default for CyanotypeImageEffect {
    fn default() -> Self {
        CyanotypeImageEffect {
            contrast: 118.0,
            paper_texture: 34.0,
            stain: 28.0,
            grain: 12.0,
            vignette: 20.0,
            seed: 1842,
        }
    }
}

impl EffectImpl for CyanotypeImageEffect {
    fn info() -> Info {
        Info::new("cyanotype", "Cyanotype", EffectCategory::Filters, "app/swatches")
            .desc("Simulates a cyanotype photographic printing process with blue-toned output.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("contrast", "Contrast", 50.0, 200.0, 118.0, |e: &mut Self, v| e.contrast = v),
            float_slider::<Self>("paper_texture", "Paper texture", 0.0, 100.0, 34.0, |e: &mut Self, v| e.paper_texture = v),
            float_slider::<Self>("stain", "Stain", 0.0, 100.0, 28.0, |e: &mut Self, v| e.stain = v),
            float_slider::<Self>("grain", "Grain", 0.0, 100.0, 12.0, |e: &mut Self, v| e.grain = v),
            float_slider::<Self>("vignette", "Vignette", 0.0, 100.0, 20.0, |e: &mut Self, v| e.vignette = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
