//! `CelticKnotBorderImageEffect` (`Core/ImageEffects/Filters/CelticKnotBorderImageEffect.cs`).

use crate::effects::prelude::*;

/// `CelticKnotBorderImageEffect`.
#[derive(Clone, Debug)]
pub struct CelticKnotBorderImageEffect {
    pub border_size: i32,
    pub knot_color: SkColor,
    pub bg_color: SkColor,
    pub strand_width: f32,
    pub outline_strands: bool,
    pub inner_line: bool,
}

impl Default for CelticKnotBorderImageEffect {
    fn default() -> Self {
        CelticKnotBorderImageEffect {
            border_size: 52,
            knot_color: SkColor::argb(0xFFB48C3C),
            bg_color: SkColor::argb(0xFF233719),
            strand_width: 35.0,
            outline_strands: true,
            inner_line: true,
        }
    }
}

impl EffectImpl for CelticKnotBorderImageEffect {
    fn info() -> Info {
        Info::new("celtic_knot_border", "Celtic knot border", EffectCategory::Filters, "image-effect/clover")
            .desc("Adds an interweaving Celtic knot pattern border inspired by Insular art manuscripts.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_numeric::<Self>("border_size", "Border size", 16, 200, 52, |e: &mut Self, v| e.border_size = v),
            color_param::<Self>("knot_color", "Knot color", SkColor::argb(0xFFB48C3C), |e: &mut Self, v| e.knot_color = v),
            color_param::<Self>("bg_color", "Background color", SkColor::argb(0xFF233719), |e: &mut Self, v| e.bg_color = v),
            float_slider::<Self>("strand_width", "Strand width", 10.0, 80.0, 35.0, |e: &mut Self, v| e.strand_width = v),
            bool_param::<Self>("outline_strands", "Outline strands", true, |e: &mut Self, v| e.outline_strands = v),
            bool_param::<Self>("inner_line", "Inner line", true, |e: &mut Self, v| e.inner_line = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
