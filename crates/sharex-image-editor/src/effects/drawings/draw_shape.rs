//! `DrawShapeEffect` (`Core/ImageEffects/Drawings/DrawShapeEffect.cs`).

use crate::effects::prelude::*;
use crate::effects::drawings::enums::*;

/// `DrawShapeEffect`.
#[derive(Clone, Debug)]
pub struct DrawShapeEffect {
    pub shape: DrawingShapeType,
    pub placement: DrawingPlacement,
    pub offset: PointI,
    pub size: SizeI,
    pub color: SkColor,
}

impl Default for DrawShapeEffect {
    fn default() -> Self {
        DrawShapeEffect {
            shape: DrawingShapeType::Rectangle,
            placement: DrawingPlacement::TopLeft,
            offset: PointI::new(0, 0),
            size: SizeI::new(100, 100),
            color: SkColor::argb(0xFFFFFFFF),
        }
    }
}

impl EffectImpl for DrawShapeEffect {
    fn info() -> Info {
        Info::new("draw_shape", "Shape", EffectCategory::Drawings, "image-effect/shapes")
            .desc("Draws a shape on the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            enum_param::<Self, DrawingShapeType>("shape", "Shape", DrawingShapeType::Rectangle, |e: &mut Self, v| e.shape = v, &[("Rectangle", DrawingShapeType::Rectangle), ("Rounded rectangle", DrawingShapeType::RoundedRectangle), ("Ellipse", DrawingShapeType::Ellipse), ("Triangle", DrawingShapeType::Triangle), ("Diamond", DrawingShapeType::Diamond)]),
            enum_param::<Self, DrawingPlacement>("placement", "Placement", DrawingPlacement::TopLeft, |e: &mut Self, v| e.placement = v, &[("Top left", DrawingPlacement::TopLeft), ("Top center", DrawingPlacement::TopCenter), ("Top right", DrawingPlacement::TopRight), ("Middle left", DrawingPlacement::MiddleLeft), ("Middle center", DrawingPlacement::MiddleCenter), ("Middle right", DrawingPlacement::MiddleRight), ("Bottom left", DrawingPlacement::BottomLeft), ("Bottom center", DrawingPlacement::BottomCenter), ("Bottom right", DrawingPlacement::BottomRight)]),
            int_numeric::<Self>("offset_x", "Offset X", -10000, 10000, 0, |e: &mut Self, v| e.offset.x = v),
            int_numeric::<Self>("offset_y", "Offset Y", -10000, 10000, 0, |e: &mut Self, v| e.offset.y = v),
            int_numeric::<Self>("size_width", "Size width", -1, 10000, 100, |e: &mut Self, v| e.size.width = v),
            int_numeric::<Self>("size_height", "Size height", -1, 10000, 100, |e: &mut Self, v| e.size.height = v),
            color_param::<Self>("color", "Color", SkColor::argb(0xFFFFFFFF), |e: &mut Self, v| e.color = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
