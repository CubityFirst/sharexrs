//! `DotMatrixImageEffect` (`Core/ImageEffects/Filters/DotMatrixImageEffect.cs`).

use crate::effects::prelude::*;

/// `DotMatrixImageEffect`.
#[derive(Clone, Debug)]
pub struct DotMatrixImageEffect {
    pub cell_size: i32,
    pub dot_scale: f32,
    pub color_mode: bool,
    pub dot_color: SkColor,
    pub paper_color: SkColor,
}

impl Default for DotMatrixImageEffect {
    fn default() -> Self {
        DotMatrixImageEffect {
            cell_size: 8,
            dot_scale: 0.85,
            color_mode: false,
            dot_color: SkColor::argb(0xFF141414),
            paper_color: SkColor::argb(0xFFFAF5EB),
        }
    }
}

impl EffectImpl for DotMatrixImageEffect {
    fn info() -> Info {
        Info::new("dot_matrix", "Dot Matrix", EffectCategory::Filters, "app/printer")
            .desc("Simulates a vintage dot matrix printer output.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("cell_size", "Cell Size", 3, 30, 8, |e: &mut Self, v| e.cell_size = v),
            float_slider::<Self>("dot_scale", "Dot Scale", 0.1, 1.0, 0.85, |e: &mut Self, v| e.dot_scale = v).tick(0.05).snap(false).format("{}{0:0.00}"),
            bool_param::<Self>("color_mode", "Color", false, |e: &mut Self, v| e.color_mode = v),
            color_param::<Self>("dot_color", "Dot Color", SkColor::argb(0xFF141414), |e: &mut Self, v| e.dot_color = v),
            color_param::<Self>("paper_color", "Paper Color", SkColor::argb(0xFFFAF5EB), |e: &mut Self, v| e.paper_color = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
