//! `OrnateScrollBorderImageEffect` (`Core/ImageEffects/Filters/OrnateScrollBorderImageEffect.cs`).

use crate::effects::prelude::*;

/// `OrnateScrollBorderImageEffect`.
#[derive(Clone, Debug)]
pub struct OrnateScrollBorderImageEffect {
    pub border_size: i32,
    pub frame_color: SkColor,
    pub ornament_color: SkColor,
    pub line_thickness: f32,
    pub fill_background: bool,
    pub shadow: bool,
}

impl Default for OrnateScrollBorderImageEffect {
    fn default() -> Self {
        OrnateScrollBorderImageEffect {
            border_size: 48,
            frame_color: SkColor::argb(0xFFF5F0DC),
            ornament_color: SkColor::argb(0xFFA0823C),
            line_thickness: 2.0,
            fill_background: true,
            shadow: true,
        }
    }
}

impl EffectImpl for OrnateScrollBorderImageEffect {
    fn info() -> Info {
        Info::new("ornate_scroll_border", "Ornate scroll border", EffectCategory::Filters, "image-effect/scroll")
            .desc("Adds an elegant Victorian-style ornate border with decorative corner scrollwork and double-line detailing.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_numeric::<Self>("border_size", "Border size", 10, 200, 48, |e: &mut Self, v| e.border_size = v),
            color_param::<Self>("frame_color", "Frame color", SkColor::argb(0xFFF5F0DC), |e: &mut Self, v| e.frame_color = v),
            color_param::<Self>("ornament_color", "Ornament color", SkColor::argb(0xFFA0823C), |e: &mut Self, v| e.ornament_color = v),
            float_slider::<Self>("line_thickness", "Line thickness", 0.5, 6.0, 2.0, |e: &mut Self, v| e.line_thickness = v),
            bool_param::<Self>("fill_background", "Fill background", true, |e: &mut Self, v| e.fill_background = v),
            bool_param::<Self>("shadow", "Shadow", true, |e: &mut Self, v| e.shadow = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
