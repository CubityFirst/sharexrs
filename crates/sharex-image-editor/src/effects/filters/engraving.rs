//! `EngravingImageEffect` (`Core/ImageEffects/Filters/EngravingImageEffect.cs`).

use crate::effects::prelude::*;

/// `EngravingImageEffect`.
#[derive(Clone, Debug)]
pub struct EngravingImageEffect {
    pub line_spacing: f32,
    pub angle: f32,
    pub line_width_max: f32,
    pub line_color: SkColor,
    pub paper_color: SkColor,
}

impl Default for EngravingImageEffect {
    fn default() -> Self {
        EngravingImageEffect {
            line_spacing: 5.0,
            angle: 45.0,
            line_width_max: 2.5,
            line_color: SkColor::argb(0xFF141428),
            paper_color: SkColor::argb(0xFFF5F0E6),
        }
    }
}

impl EffectImpl for EngravingImageEffect {
    fn info() -> Info {
        Info::new("engraving", "Engraving", EffectCategory::Filters, "app/annotate")
            .desc("Simulates a line engraving effect like banknote illustrations.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("line_spacing", "Line Spacing", 2.0, 20.0, 5.0, |e: &mut Self, v| e.line_spacing = v).tick(0.5).snap(false).format("{}{0:0.0}"),
            float_slider::<Self>("angle", "Angle", 0.0, 180.0, 45.0, |e: &mut Self, v| e.angle = v),
            float_slider::<Self>("line_width_max", "Max Line Width", 0.5, 8.0, 2.5, |e: &mut Self, v| e.line_width_max = v).tick(0.1).snap(false).format("{}{0:0.0}"),
            color_param::<Self>("line_color", "Line Color", SkColor::argb(0xFF141428), |e: &mut Self, v| e.line_color = v),
            color_param::<Self>("paper_color", "Paper Color", SkColor::argb(0xFFF5F0E6), |e: &mut Self, v| e.paper_color = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
