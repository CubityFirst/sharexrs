//! `DrawTextEffect` (`Core/ImageEffects/Drawings/DrawTextEffect.cs`).

use crate::effects::drawings::enums::*;
use crate::effects::prelude::*;

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
        Info::new(
            "draw_text",
            "Text",
            EffectCategory::Drawings,
            "image-editor/text",
        )
        .desc("Draws text on the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            text_param::<Self>("text", "Text", "Text", |e: &mut Self, v| e.text = v),
            enum_param::<Self, DrawingPlacement>(
                "placement",
                "Placement",
                DrawingPlacement::TopLeft,
                |e: &mut Self, v| e.placement = v,
                &[
                    ("Top left", DrawingPlacement::TopLeft),
                    ("Top center", DrawingPlacement::TopCenter),
                    ("Top right", DrawingPlacement::TopRight),
                    ("Middle left", DrawingPlacement::MiddleLeft),
                    ("Middle center", DrawingPlacement::MiddleCenter),
                    ("Middle right", DrawingPlacement::MiddleRight),
                    ("Bottom left", DrawingPlacement::BottomLeft),
                    ("Bottom center", DrawingPlacement::BottomCenter),
                    ("Bottom right", DrawingPlacement::BottomRight),
                ],
            ),
            int_numeric::<Self>(
                "offset_x",
                "Offset X",
                -10000,
                10000,
                0,
                |e: &mut Self, v| e.offset.x = v,
            ),
            int_numeric::<Self>(
                "offset_y",
                "Offset Y",
                -10000,
                10000,
                0,
                |e: &mut Self, v| e.offset.y = v,
            ),
            int_numeric::<Self>("angle", "Angle", -360, 360, 0, |e: &mut Self, v| {
                e.angle = v
            }),
            bool_param::<Self>("auto_hide", "Auto hide", false, |e: &mut Self, v| {
                e.auto_hide = v
            }),
            text_param::<Self>("font_family", "Font family", "Arial", |e: &mut Self, v| {
                e.font_family = v
            }),
            float_slider::<Self>(
                "font_size",
                "Font size",
                1.0,
                500.0,
                36.0,
                |e: &mut Self, v| e.font_size = v,
            ),
            bool_param::<Self>("bold", "Bold", false, |e: &mut Self, v| e.bold = v),
            bool_param::<Self>("italic", "Italic", false, |e: &mut Self, v| e.italic = v),
            color_param::<Self>(
                "color",
                "Color",
                SkColor::argb(0xFFEBEBEB),
                |e: &mut Self, v| e.color = v,
            ),
            bool_param::<Self>("outline", "Outline", false, |e: &mut Self, v| e.outline = v),
            int_numeric::<Self>(
                "outline_size",
                "Outline size",
                1,
                100,
                5,
                |e: &mut Self, v| e.outline_size = v,
            ),
            color_param::<Self>(
                "outline_color",
                "Outline color",
                SkColor::argb(0xFFEB0000),
                |e: &mut Self, v| e.outline_color = v,
            ),
            bool_param::<Self>("shadow", "Shadow", false, |e: &mut Self, v| e.shadow = v),
            int_numeric::<Self>(
                "shadow_offset_x",
                "Shadow offset X",
                -1000,
                1000,
                0,
                |e: &mut Self, v| e.shadow_offset.x = v,
            ),
            int_numeric::<Self>(
                "shadow_offset_y",
                "Shadow offset Y",
                -1000,
                1000,
                5,
                |e: &mut Self, v| e.shadow_offset.y = v,
            ),
            color_param::<Self>(
                "shadow_color",
                "Shadow color",
                SkColor::argb(0x7D000000),
                |e: &mut Self, v| e.shadow_color = v,
            ),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        if is_blank(&self.text) || self.font_size < 1.0 {
            return Ok(source.clone());
        }
        let weight = if self.bold { 700 } else { 400 };
        let font = Font::new(
            Typeface::from_family(&self.font_family, weight, self.italic),
            self.font_size,
        );
        let m = font.metrics();
        let line_h = (m.descent - m.ascent + m.leading).max(font.size);
        let base = -m.ascent;
        let mut path = SkPath::new();
        path.fill_type = sharex_skia::FillType::Winding;
        for (i, line) in self.text.replace("\r\n", "\n").split('\n').enumerate() {
            if line.is_empty() {
                continue;
            }
            path.add_path(&font.text_path(line, 0.0, base + i as f32 * line_h));
        }
        if path.is_empty() {
            return Ok(source.clone());
        }
        if self.angle != 0 {
            path = path.transformed(&sharex_skia::Matrix::rotate(self.angle as f32));
        }
        let b = path.bounds();
        if b.is_empty() {
            return Ok(source.clone());
        }
        let size = SizeI::new(
            (b.width() as f64).ceil() as i32 + 1,
            (b.height() as f64).ceil() as i32 + 1,
        );
        let (sw, sh) = (source.width() as i32, source.height() as i32);
        let pos =
            super::helpers::get_position(self.placement, self.offset, SizeI::new(sw, sh), size);
        let (l, t, r, bt) = (pos.x, pos.y, pos.x + size.width, pos.y + size.height);
        if self.auto_hide && !(l >= 0 && t >= 0 && r <= sw && bt <= sh) {
            return Ok(source.clone());
        }
        let path = path.translated(l as f32 - b.left, t as f32 - b.top);
        let mut out = source.clone();
        let mut canvas = Canvas::new(&mut out);
        let outline = self.outline && self.outline_size > 0;
        let stroke = |canvas: &mut Canvas, p: &SkPath, size: i32, c: SkColor| {
            if c.a != 0 {
                sk::stroke_path(
                    canvas,
                    p,
                    &Pen::new(1.max(size) as f32).join(Join::Round),
                    &solid(c, true),
                );
            }
        };
        let fill = |canvas: &mut Canvas, p: &SkPath, c: SkColor| {
            if c.a != 0 {
                canvas.fill_sk_path(p, &solid(c, true));
            }
        };
        if self.shadow && self.shadow_color.a > 0 {
            let sp = path.translated(self.shadow_offset.x as f32, self.shadow_offset.y as f32);
            if outline {
                stroke(&mut canvas, &sp, self.outline_size, self.shadow_color);
            } else {
                fill(&mut canvas, &sp, self.shadow_color);
            }
        }
        if outline {
            stroke(&mut canvas, &path, self.outline_size, self.outline_color);
        }
        fill(&mut canvas, &path, self.color);
        drop(canvas);
        Ok(out)
    }
}
