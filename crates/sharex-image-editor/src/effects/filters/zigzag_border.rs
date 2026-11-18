//! `ZigZagBorderImageEffect` (`Core/ImageEffects/Filters/ZigZagBorderImageEffect.cs`).

use crate::effects::prelude::*;

/// `ZigZagBorderImageEffect`.
#[derive(Clone, Debug)]
pub struct ZigZagBorderImageEffect {
    pub border_size: i32,
    pub teeth_height: f32,
    pub teeth_width: f32,
    pub frame_color: SkColor,
    pub accent_color: SkColor,
    pub accent_stripe: bool,
}

impl Default for ZigZagBorderImageEffect {
    fn default() -> Self {
        ZigZagBorderImageEffect {
            border_size: 36,
            teeth_height: 40.0,
            teeth_width: 24.0,
            frame_color: SkColor::argb(0xFF282832),
            accent_color: SkColor::argb(0xFFDCB43C),
            accent_stripe: true,
        }
    }
}

impl EffectImpl for ZigZagBorderImageEffect {
    fn info() -> Info {
        Info::new("zigzag_border", "Zigzag border", EffectCategory::Filters, "image-effect/waves")
            .desc("Adds a border with decorative jagged zigzag inner teeth that bite into the image edges.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_numeric::<Self>("border_size", "Border size", 8, 200, 36, |e: &mut Self, v| e.border_size = v),
            float_slider::<Self>("teeth_height", "Teeth height", 5.0, 80.0, 40.0, |e: &mut Self, v| e.teeth_height = v),
            float_slider::<Self>("teeth_width", "Teeth width", 4.0, 80.0, 24.0, |e: &mut Self, v| e.teeth_width = v),
            color_param::<Self>("frame_color", "Frame color", SkColor::argb(0xFF282832), |e: &mut Self, v| e.frame_color = v),
            color_param::<Self>("accent_color", "Accent color", SkColor::argb(0xFFDCB43C), |e: &mut Self, v| e.accent_color = v),
            bool_param::<Self>("accent_stripe", "Accent stripe", true, |e: &mut Self, v| e.accent_stripe = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
