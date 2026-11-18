//! `TextWatermarkEffect` (`Core/ImageEffects/Drawings/TextWatermarkEffect.cs`).

use crate::effects::prelude::*;
use crate::effects::drawings::enums::*;

/// `TextWatermarkEffect`.
#[derive(Clone, Debug)]
pub struct TextWatermarkEffect {
    /// `Math.Max(0, value)` when set.
    pub corner_radius: i32,
    /// `Math.Max(0, value)` when set.
    pub border_size: i32,
    pub text: String,
    pub placement: DrawingPlacement,
    pub offset: PointI,
    pub auto_hide: bool,
    pub font_family: String,
    pub font_size: f32,
    pub bold: bool,
    pub italic: bool,
    pub text_color: SkColor,
    pub draw_text_shadow: bool,
    pub text_shadow_color: SkColor,
    pub text_shadow_offset: PointI,
    pub padding_left: i32,
    pub padding_top: i32,
    pub padding_right: i32,
    pub padding_bottom: i32,
    pub draw_border: bool,
    pub border_color: SkColor,
    pub draw_background: bool,
    pub background_color: SkColor,
}

impl Default for TextWatermarkEffect {
    fn default() -> Self {
        TextWatermarkEffect {
            corner_radius: 4,
            border_size: 1,
            text: "Text watermark".to_string(),
            placement: DrawingPlacement::BottomRight,
            offset: PointI::new(5, 5),
            auto_hide: false,
            font_family: "Arial".to_string(),
            font_size: 15.0,
            bold: false,
            italic: false,
            text_color: SkColor::argb(0xFFEBEBEB),
            draw_text_shadow: false,
            text_shadow_color: SkColor::argb(0xFF000000),
            text_shadow_offset: PointI::new(-1, -1),
            padding_left: 5,
            padding_top: 5,
            padding_right: 5,
            padding_bottom: 5,
            draw_border: true,
            border_color: SkColor::argb(0xFF000000),
            draw_background: true,
            background_color: SkColor::argb(0xFF2A2F38),
        }
    }
}

impl EffectImpl for TextWatermarkEffect {
    fn info() -> Info {
        Info::new("text_watermark", "Text watermark", EffectCategory::Drawings, "image-effect/text-cursor")
            .desc("Draws a text watermark with background on the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            text_param::<Self>("text", "Text", "Text watermark", |e: &mut Self, v| e.text = v),
            enum_param::<Self, DrawingPlacement>("placement", "Placement", DrawingPlacement::BottomRight, |e: &mut Self, v| e.placement = v, &[("Top left", DrawingPlacement::TopLeft), ("Top center", DrawingPlacement::TopCenter), ("Top right", DrawingPlacement::TopRight), ("Middle left", DrawingPlacement::MiddleLeft), ("Middle center", DrawingPlacement::MiddleCenter), ("Middle right", DrawingPlacement::MiddleRight), ("Bottom left", DrawingPlacement::BottomLeft), ("Bottom center", DrawingPlacement::BottomCenter), ("Bottom right", DrawingPlacement::BottomRight)]),
            int_numeric::<Self>("offset_x", "Offset X", -10000, 10000, 5, |e: &mut Self, v| e.offset.x = v),
            int_numeric::<Self>("offset_y", "Offset Y", -10000, 10000, 5, |e: &mut Self, v| e.offset.y = v),
            bool_param::<Self>("auto_hide", "Auto hide", false, |e: &mut Self, v| e.auto_hide = v),
            text_param::<Self>("font_family", "Font family", "Arial", |e: &mut Self, v| e.font_family = v),
            float_slider::<Self>("font_size", "Font size", 1.0, 500.0, 15.0, |e: &mut Self, v| e.font_size = v),
            bool_param::<Self>("bold", "Bold", false, |e: &mut Self, v| e.bold = v),
            bool_param::<Self>("italic", "Italic", false, |e: &mut Self, v| e.italic = v),
            color_param::<Self>("text_color", "Text color", SkColor::argb(0xFFEBEBEB), |e: &mut Self, v| e.text_color = v),
            bool_param::<Self>("draw_text_shadow", "Draw text shadow", false, |e: &mut Self, v| e.draw_text_shadow = v),
            color_param::<Self>("text_shadow_color", "Text shadow color", SkColor::argb(0xFF000000), |e: &mut Self, v| e.text_shadow_color = v),
            int_numeric::<Self>("text_shadow_offset_x", "Text shadow offset X", -1000, 1000, -1, |e: &mut Self, v| e.text_shadow_offset.x = v),
            int_numeric::<Self>("text_shadow_offset_y", "Text shadow offset Y", -1000, 1000, -1, |e: &mut Self, v| e.text_shadow_offset.y = v),
            int_numeric::<Self>("corner_radius", "Corner radius", 0, 100, 4, |e: &mut Self, v| e.corner_radius = v.max(0)),
            int_numeric::<Self>("padding_left", "Padding left", 0, 100, 5, |e: &mut Self, v| e.padding_left = v),
            int_numeric::<Self>("padding_top", "Padding top", 0, 100, 5, |e: &mut Self, v| e.padding_top = v),
            int_numeric::<Self>("padding_right", "Padding right", 0, 100, 5, |e: &mut Self, v| e.padding_right = v),
            int_numeric::<Self>("padding_bottom", "Padding bottom", 0, 100, 5, |e: &mut Self, v| e.padding_bottom = v),
            bool_param::<Self>("draw_border", "Draw border", true, |e: &mut Self, v| e.draw_border = v),
            color_param::<Self>("border_color", "Border color", SkColor::argb(0xFF000000), |e: &mut Self, v| e.border_color = v),
            int_numeric::<Self>("border_size", "Border size", 0, 50, 1, |e: &mut Self, v| e.border_size = v.max(0)),
            bool_param::<Self>("draw_background", "Draw background", true, |e: &mut Self, v| e.draw_background = v),
            color_param::<Self>("background_color", "Background color", SkColor::argb(0xFF2A2F38), |e: &mut Self, v| e.background_color = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
