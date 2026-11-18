//! `WetPlateCollodionImageEffect` (`Core/ImageEffects/Filters/WetPlateCollodionImageEffect.cs`).

use crate::effects::prelude::*;

/// `WetPlateCollodionImageEffect`.
#[derive(Clone, Debug)]
pub struct WetPlateCollodionImageEffect {
    pub contrast: f32,
    pub plate_stains: f32,
    pub scratches: f32,
    pub silvering: f32,
    pub vignette: f32,
    pub seed: i32,
}

impl Default for WetPlateCollodionImageEffect {
    fn default() -> Self {
        WetPlateCollodionImageEffect {
            contrast: 132.0,
            plate_stains: 34.0,
            scratches: 24.0,
            silvering: 36.0,
            vignette: 58.0,
            seed: 1855,
        }
    }
}

impl EffectImpl for WetPlateCollodionImageEffect {
    fn info() -> Info {
        Info::new("wet_plate_collodion", "Wet plate collodion", EffectCategory::Filters, "app/film")
            .desc("Simulates the wet plate collodion photographic process with stains and silvering.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("contrast", "Contrast", 50.0, 200.0, 132.0, |e: &mut Self, v| e.contrast = v),
            float_slider::<Self>("plate_stains", "Plate stains", 0.0, 100.0, 34.0, |e: &mut Self, v| e.plate_stains = v),
            float_slider::<Self>("scratches", "Scratches", 0.0, 100.0, 24.0, |e: &mut Self, v| e.scratches = v),
            float_slider::<Self>("silvering", "Silvering", 0.0, 100.0, 36.0, |e: &mut Self, v| e.silvering = v),
            float_slider::<Self>("vignette", "Vignette", 0.0, 100.0, 58.0, |e: &mut Self, v| e.vignette = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
