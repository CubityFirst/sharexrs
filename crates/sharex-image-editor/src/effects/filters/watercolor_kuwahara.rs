//! `WatercolorKuwaharaImageEffect` (`Core/ImageEffects/Filters/WatercolorKuwaharaImageEffect.cs`).

use crate::effects::prelude::*;

/// `WatercolorKuwaharaImageEffect`.
#[derive(Clone, Debug)]
pub struct WatercolorKuwaharaImageEffect {
    pub radius: i32,
    pub saturation_boost: f32,
    pub detail_blend: f32,
}

impl Default for WatercolorKuwaharaImageEffect {
    fn default() -> Self {
        WatercolorKuwaharaImageEffect {
            radius: 4,
            saturation_boost: 20.0,
            detail_blend: 18.0,
        }
    }
}

impl EffectImpl for WatercolorKuwaharaImageEffect {
    fn info() -> Info {
        Info::new("watercolor_kuwahara", "Watercolor / Kuwahara", EffectCategory::Filters, "image-effect/paint-brush")
            .desc("Applies a Kuwahara filter for a watercolor painting effect.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("radius", "Radius", 2, 10, 4, |e: &mut Self, v| e.radius = v),
            float_slider::<Self>("saturation_boost", "Saturation boost", 0.0, 100.0, 20.0, |e: &mut Self, v| e.saturation_boost = v),
            float_slider::<Self>("detail_blend", "Detail blend", 0.0, 100.0, 18.0, |e: &mut Self, v| e.detail_blend = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
