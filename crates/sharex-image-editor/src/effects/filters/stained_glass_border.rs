//! `StainedGlassBorderImageEffect` (`Core/ImageEffects/Filters/StainedGlassBorderImageEffect.cs`).

use crate::effects::prelude::*;

/// `StainedGlassBorderImageEffect`.
#[derive(Clone, Debug)]
pub struct StainedGlassBorderImageEffect {
    pub border_size: i32,
    pub panel_width: f32,
    pub lead_thickness: f32,
    pub glass_brightness: f32,
    pub glow: bool,
    pub inner_line: bool,
}

impl Default for StainedGlassBorderImageEffect {
    fn default() -> Self {
        StainedGlassBorderImageEffect {
            border_size: 60,
            panel_width: 40.0,
            lead_thickness: 3.5,
            glass_brightness: 75.0,
            glow: true,
            inner_line: true,
        }
    }
}

impl EffectImpl for StainedGlassBorderImageEffect {
    fn info() -> Info {
        Info::new("stained_glass_border", "Stained glass border", EffectCategory::Filters, "app/palette")
            .desc("Adds a stained glass border with jewel-toned glass panels divided by dark lead came lines.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_numeric::<Self>("border_size", "Border size", 20, 300, 60, |e: &mut Self, v| e.border_size = v),
            float_slider::<Self>("panel_width", "Panel width", 10.0, 120.0, 40.0, |e: &mut Self, v| e.panel_width = v),
            float_slider::<Self>("lead_thickness", "Lead thickness", 1.0, 12.0, 3.5, |e: &mut Self, v| e.lead_thickness = v),
            float_slider::<Self>("glass_brightness", "Glass brightness", 30.0, 100.0, 75.0, |e: &mut Self, v| e.glass_brightness = v),
            bool_param::<Self>("glow", "Glass glow", true, |e: &mut Self, v| e.glow = v),
            bool_param::<Self>("inner_line", "Inner line", true, |e: &mut Self, v| e.inner_line = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
