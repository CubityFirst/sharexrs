//! `ChromeMetallicBorderImageEffect` (`Core/ImageEffects/Filters/ChromeMetallicBorderImageEffect.cs`).

use crate::effects::prelude::*;

/// `ChromeMetallicBorderImageEffect`.
#[derive(Clone, Debug)]
pub struct ChromeMetallicBorderImageEffect {
    pub border_size: i32,
    pub tint_color: SkColor,
    pub reflectivity: f32,
    pub bevel_strength: f32,
    pub inner_line: bool,
    pub outer_line: bool,
}

impl Default for ChromeMetallicBorderImageEffect {
    fn default() -> Self {
        ChromeMetallicBorderImageEffect {
            border_size: 36,
            tint_color: SkColor::argb(0xFFC8CDD2),
            reflectivity: 80.0,
            bevel_strength: 70.0,
            inner_line: true,
            outer_line: true,
        }
    }
}

impl EffectImpl for ChromeMetallicBorderImageEffect {
    fn info() -> Info {
        Info::new("chrome_metallic_border", "Chrome metallic border", EffectCategory::Filters, "app/sparkles")
            .desc("Adds a shiny chrome metallic border with realistic specular highlights and reflections.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_numeric::<Self>("border_size", "Border size", 4, 300, 36, |e: &mut Self, v| e.border_size = v),
            color_param::<Self>("tint_color", "Tint color", SkColor::argb(0xFFC8CDD2), |e: &mut Self, v| e.tint_color = v),
            float_slider::<Self>("reflectivity", "Reflectivity", 0.0, 100.0, 80.0, |e: &mut Self, v| e.reflectivity = v),
            float_slider::<Self>("bevel_strength", "Bevel strength", 0.0, 100.0, 70.0, |e: &mut Self, v| e.bevel_strength = v),
            bool_param::<Self>("inner_line", "Inner line", true, |e: &mut Self, v| e.inner_line = v),
            bool_param::<Self>("outer_line", "Outer line", true, |e: &mut Self, v| e.outer_line = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
