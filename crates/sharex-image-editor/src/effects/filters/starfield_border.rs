//! `StarfieldBorderImageEffect` (`Core/ImageEffects/Filters/StarfieldBorderImageEffect.cs`).

use crate::effects::prelude::*;

/// `StarfieldBorderImageEffect`.
#[derive(Clone, Debug)]
pub struct StarfieldBorderImageEffect {
    pub border_size: i32,
    pub bg_color: SkColor,
    pub nebula_color: SkColor,
    pub star_density: f32,
    pub nebula_intensity: f32,
    pub bright_stars: bool,
}

impl Default for StarfieldBorderImageEffect {
    fn default() -> Self {
        StarfieldBorderImageEffect {
            border_size: 60,
            bg_color: SkColor::argb(0xFF050512),
            nebula_color: SkColor::argb(0xFF280A50),
            star_density: 65.0,
            nebula_intensity: 45.0,
            bright_stars: true,
        }
    }
}

impl EffectImpl for StarfieldBorderImageEffect {
    fn info() -> Info {
        Info::new("starfield_border", "Starfield border", EffectCategory::Filters, "image-effect/orbit")
            .desc("Adds a cosmic starfield border with scattered stars, nebula wisps, and subtle color tints.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_numeric::<Self>("border_size", "Border size", 12, 300, 60, |e: &mut Self, v| e.border_size = v),
            color_param::<Self>("bg_color", "Background color", SkColor::argb(0xFF050512), |e: &mut Self, v| e.bg_color = v),
            color_param::<Self>("nebula_color", "Nebula tint", SkColor::argb(0xFF280A50), |e: &mut Self, v| e.nebula_color = v),
            float_slider::<Self>("star_density", "Star density", 10.0, 100.0, 65.0, |e: &mut Self, v| e.star_density = v),
            float_slider::<Self>("nebula_intensity", "Nebula intensity", 0.0, 100.0, 45.0, |e: &mut Self, v| e.nebula_intensity = v),
            bool_param::<Self>("bright_stars", "Bright star glow", true, |e: &mut Self, v| e.bright_stars = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
