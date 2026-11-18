//! `DrawTextEffect` (`Core/ImageEffects/Drawings/DrawTextEffect.cs`).

use crate::effects::prelude::*;
use crate::effects::drawings::enums::*;

/// `DrawTextEffect`.
#[derive(Clone, Debug)]
pub struct DrawTextEffect {
    pub text: String,
    pub placement: DrawingPlacement,
    pub offset: PointI,
    pub angle: i32,
    pub auto_hide: bool,
    pub font_family: String,
    pub font_size: f32,
    pub bold: bool,
    pub italic: bool,
    pub color: SkColor,
    pub outline: bool,
    pub outline_size: i32,
    pub outline_color: SkColor,
    pub shadow: bool,
    pub shadow_offset: PointI,
    pub shadow_color: SkColor,
}

impl Default for DrawTextEffect {
    fn default() -> Self {
        DrawTextEffect {
            text: "Text".to_string(),
            placement: DrawingPlacement::TopLeft,
            offset: PointI::new(0, 0),
            angle: 0,
            auto_hide: false,
            font_family: "Arial".to_string(),
            font_size: 36.0,
            bold: false,
            italic: false,
            color: SkColor::argb(0xFFEBEBEB),
            outline: false,
            outline_size: 5,
            outline_color: SkColor::argb(0xFFEB0000),
            shadow: false,
            shadow_offset: PointI::new(0, 5),
            shadow_color: SkColor::argb(0x7D000000),
        }
    }
}

impl EffectImpl for DrawTextEffect {
    fn info() -> Info {
        Info::new("draw_text", "Text", EffectCategory::Drawings, "image-editor/text")
            .desc("Draws text on the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            text_param::<Self>("text", "Text", "Text", |e: &mut Self, v| e.text = v),
            enum_param::<Self, DrawingPlacement>("placement", "Placement", DrawingPlacement::TopLeft, |e: &mut Self, v| e.placement = v, &[("Top left", DrawingPlacement::TopLeft), ("Top center", DrawingPlacement::TopCenter), ("Top right", DrawingPlacement::TopRight), ("Middle left", DrawingPlacement::MiddleLeft), ("Middle center", DrawingPlacement::MiddleCenter), ("Middle right", DrawingPlacement::MiddleRight), ("Bottom left", DrawingPlacement::BottomLeft), ("Bottom center", DrawingPlacement::BottomCenter), ("Bottom right", DrawingPlacement::BottomRight)]),
            int_numeric::<Self>("offset_x", "Offset X", -10000, 10000, 0, |e: &mut Self, v| e.offset.x = v),
            int_numeric::<Self>("offset_y", "Offset Y", -10000, 10000, 0, |e: &mut Self, v| e.offset.y = v),
            int_numeric::<Self>("angle", "Angle", -360, 360, 0, |e: &mut Self, v| e.angle = v),
            bool_param::<Self>("auto_hide", "Auto hide", false, |e: &mut Self, v| e.auto_hide = v),
            text_param::<Self>("font_family", "Font family", "Arial", |e: &mut Self, v| e.font_family = v),
            float_slider::<Self>("font_size", "Font size", 1.0, 500.0, 36.0, |e: &mut Self, v| e.font_size = v),
            bool_param::<Self>("bold", "Bold", false, |e: &mut Self, v| e.bold = v),
            bool_param::<Self>("italic", "Italic", false, |e: &mut Self, v| e.italic = v),
            color_param::<Self>("color", "Color", SkColor::argb(0xFFEBEBEB), |e: &mut Self, v| e.color = v),
            bool_param::<Self>("outline", "Outline", false, |e: &mut Self, v| e.outline = v),
            int_numeric::<Self>("outline_size", "Outline size", 1, 100, 5, |e: &mut Self, v| e.outline_size = v),
            color_param::<Self>("outline_color", "Outline color", SkColor::argb(0xFFEB0000), |e: &mut Self, v| e.outline_color = v),
            bool_param::<Self>("shadow", "Shadow", false, |e: &mut Self, v| e.shadow = v),
            int_numeric::<Self>("shadow_offset_x", "Shadow offset X", -1000, 1000, 0, |e: &mut Self, v| e.shadow_offset.x = v),
            int_numeric::<Self>("shadow_offset_y", "Shadow offset Y", -1000, 1000, 5, |e: &mut Self, v| e.shadow_offset.y = v),
            color_param::<Self>("shadow_color", "Shadow color", SkColor::argb(0x7D000000), |e: &mut Self, v| e.shadow_color = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
