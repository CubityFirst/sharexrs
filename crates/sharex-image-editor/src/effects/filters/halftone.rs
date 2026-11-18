//! `HalftoneImageEffect` (`Core/ImageEffects/Filters/HalftoneImageEffect.cs`).

use crate::effects::prelude::*;

/// `HalftoneImageEffect`.
#[derive(Clone, Debug)]
pub struct HalftoneImageEffect {
    pub cell_size: i32,
    pub dot_softness: f32,
    pub ink_strength: f32,
    pub source_blend: f32,
    pub angle_offset: f32,
}

impl Default for HalftoneImageEffect {
    fn default() -> Self {
        HalftoneImageEffect {
            cell_size: 8,
            dot_softness: 18.0,
            ink_strength: 90.0,
            source_blend: 20.0,
            angle_offset: 0.0,
        }
    }
}

impl EffectImpl for HalftoneImageEffect {
    fn info() -> Info {
        Info::new("halftone", "Halftone", EffectCategory::Filters, "image-effect/circle-dot")
            .desc("Converts the image into a CMYK halftone dot pattern.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("cell_size", "Cell size", 3, 24, 8, |e: &mut Self, v| e.cell_size = v),
            float_slider::<Self>("dot_softness", "Dot softness", 0.0, 100.0, 18.0, |e: &mut Self, v| e.dot_softness = v),
            float_slider::<Self>("ink_strength", "Ink strength", 0.0, 100.0, 90.0, |e: &mut Self, v| e.ink_strength = v),
            float_slider::<Self>("source_blend", "Source blend", 0.0, 100.0, 20.0, |e: &mut Self, v| e.source_blend = v),
            float_slider::<Self>("angle_offset", "Angle offset", -45.0, 45.0, 0.0, |e: &mut Self, v| e.angle_offset = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
