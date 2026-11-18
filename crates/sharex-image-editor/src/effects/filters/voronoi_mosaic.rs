//! `VoronoiMosaicImageEffect` (`Core/ImageEffects/Filters/VoronoiMosaicImageEffect.cs`).

use crate::effects::prelude::*;

/// `VoronoiMosaicImageEffect`.
#[derive(Clone, Debug)]
pub struct VoronoiMosaicImageEffect {
    pub cell_count: i32,
    pub draw_edges: bool,
    pub edge_thickness: f32,
    pub edge_color: SkColor,
}

impl Default for VoronoiMosaicImageEffect {
    fn default() -> Self {
        VoronoiMosaicImageEffect {
            cell_count: 300,
            draw_edges: true,
            edge_thickness: 1.0,
            edge_color: SkColor::argb(0xFF000000),
        }
    }
}

impl EffectImpl for VoronoiMosaicImageEffect {
    fn info() -> Info {
        Info::new("voronoi_mosaic", "Voronoi Mosaic", EffectCategory::Filters, "image-effect/hexagon")
            .desc("Divides the image into Voronoi cells colored by average region color.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("cell_count", "Cell Count", 10, 2000, 300, |e: &mut Self, v| e.cell_count = v),
            bool_param::<Self>("draw_edges", "Draw Edges", true, |e: &mut Self, v| e.draw_edges = v),
            float_slider::<Self>("edge_thickness", "Edge Thickness", 0.5, 5.0, 1.0, |e: &mut Self, v| e.edge_thickness = v).tick(0.5).snap(false).format("{}{0:0.0}"),
            color_param::<Self>("edge_color", "Edge Color", SkColor::argb(0xFF000000), |e: &mut Self, v| e.edge_color = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
