//! `ArtDecoBorderImageEffect` (`Core/ImageEffects/Filters/ArtDecoBorderImageEffect.cs`).

use crate::effects::prelude::*;

/// `ArtDecoBorderImageEffect`.
#[derive(Clone, Debug)]
pub struct ArtDecoBorderImageEffect {
    pub border_size: i32,
    pub bg_color: SkColor,
    pub accent_color: SkColor,
    pub fan_spacing: f32,
    pub fan_rays: i32,
    pub inner_lines: bool,
}

impl Default for ArtDecoBorderImageEffect {
    fn default() -> Self {
        ArtDecoBorderImageEffect {
            border_size: 64,
            bg_color: SkColor::argb(0xFF0F0F19),
            accent_color: SkColor::argb(0xFFD4AF37),
            fan_spacing: 80.0,
            fan_rays: 7,
            inner_lines: true,
        }
    }
}

impl EffectImpl for ArtDecoBorderImageEffect {
    fn info() -> Info {
        Info::new("art_deco_border", "Art deco border", EffectCategory::Filters, "app/trophy")
            .desc("Adds a bold Art Deco border with geometric fan motifs, stepped lines, and metallic gold accents.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_numeric::<Self>("border_size", "Border size", 20, 300, 64, |e: &mut Self, v| e.border_size = v),
            color_param::<Self>("bg_color", "Background color", SkColor::argb(0xFF0F0F19), |e: &mut Self, v| e.bg_color = v),
            color_param::<Self>("accent_color", "Accent color", SkColor::argb(0xFFD4AF37), |e: &mut Self, v| e.accent_color = v),
            float_slider::<Self>("fan_spacing", "Fan spacing", 30.0, 200.0, 80.0, |e: &mut Self, v| e.fan_spacing = v),
            int_slider::<Self>("fan_rays", "Fan rays", 3, 12, 7, |e: &mut Self, v| e.fan_rays = v),
            bool_param::<Self>("inner_lines", "Inner stepped lines", true, |e: &mut Self, v| e.inner_lines = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
