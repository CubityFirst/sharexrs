//! `MosaicPolygonImageEffect` (`Core/ImageEffects/Filters/MosaicPolygonImageEffect.cs`).

use crate::effects::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MosaicPolygonShape {
    Hexagon,
    Triangle,
}

/// `MosaicPolygonImageEffect`.
#[derive(Clone, Debug)]
pub struct MosaicPolygonImageEffect {
    pub cell_size: i32,
    pub shape: MosaicPolygonShape,
    pub border_width: f32,
    pub border_opacity: f32,
    pub randomness: f32,
    pub seed: i32,
}

impl Default for MosaicPolygonImageEffect {
    fn default() -> Self {
        MosaicPolygonImageEffect {
            cell_size: 24,
            shape: MosaicPolygonShape::Hexagon,
            border_width: 1.0,
            border_opacity: 45.0,
            randomness: 18.0,
            seed: 9031,
        }
    }
}

impl EffectImpl for MosaicPolygonImageEffect {
    fn info() -> Info {
        Info::new("mosaic_polygon", "Mosaic polygon", EffectCategory::Filters, "image-effect/hexagon")
            .desc("Creates a mosaic using hexagonal or triangular polygon tiles.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("cell_size", "Cell size", 8, 80, 24, |e: &mut Self, v| e.cell_size = v),
            enum_param::<Self, MosaicPolygonShape>("shape", "Shape", MosaicPolygonShape::Hexagon, |e: &mut Self, v| e.shape = v, &[("Hexagon", MosaicPolygonShape::Hexagon), ("Triangle", MosaicPolygonShape::Triangle)]),
            float_slider::<Self>("border_width", "Border width", 0.0, 6.0, 1.0, |e: &mut Self, v| e.border_width = v),
            float_slider::<Self>("border_opacity", "Border opacity", 0.0, 100.0, 45.0, |e: &mut Self, v| e.border_opacity = v),
            float_slider::<Self>("randomness", "Randomness", 0.0, 100.0, 18.0, |e: &mut Self, v| e.randomness = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
