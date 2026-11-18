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
        let resolve = |v: i32, full: u32| if v == -1 { full as i32 } else { v };
        let width = resolve(self.size.width, source.width());
        let height = resolve(self.size.height, source.height());
        if width <= 0 || height <= 0 || self.color.a == 0 {
            return Ok(source.clone());
        }
        let size = SizeI::new(width, height);
        let pos = super::helpers::get_position(
            self.placement,
            self.offset,
            SizeI::new(source.width() as i32, source.height() as i32),
            size,
        );
        let rect = RRect::ltrb(pos.x as f32, pos.y as f32, (pos.x + width) as f32, (pos.y + height) as f32);
        let mut out = source.clone();
        let mut canvas = Canvas::new(&mut out);
        let paint = solid(self.color, true);
        let mid_x = rect.left + rect.width() / 2.0;
        let mid_y = rect.top + rect.height() / 2.0;
        match self.shape {
            DrawingShapeType::RoundedRectangle => {
                let radius = 1f32.max(rect.width().min(rect.height()) * 0.2);
                sk::draw_round_rect(&mut canvas, rect, radius, radius, &paint, None);
            }
            DrawingShapeType::Ellipse => sk::draw_oval(&mut canvas, rect, &paint, None),
            DrawingShapeType::Triangle => {
                let mut path = SkPath::new();
                path.move_to(mid_x, rect.top);
                path.line_to(rect.right, rect.bottom);
                path.line_to(rect.left, rect.bottom);
                path.close();
                canvas.fill_sk_path(&path, &paint);
            }
            DrawingShapeType::Diamond => {
                let mut path = SkPath::new();
                path.move_to(mid_x, rect.top);
                path.line_to(rect.right, mid_y);
                path.line_to(mid_x, rect.bottom);
                path.line_to(rect.left, mid_y);
                path.close();
                canvas.fill_sk_path(&path, &paint);
            }
            DrawingShapeType::Rectangle => sk::draw_rect(&mut canvas, rect, &paint, None),
        }
        drop(canvas);
        Ok(out)
    }
}
