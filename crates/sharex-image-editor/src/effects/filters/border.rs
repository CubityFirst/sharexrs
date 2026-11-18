//! `BorderImageEffect` (`Core/ImageEffects/Filters/BorderImageEffect.cs`).

use crate::effects::prelude::*;

/// `ImageHelpers.BorderType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BorderType {
    Outside,
    Inside,
}

/// `ImageHelpers.DashStyle`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DashStyle {
    Solid,
    Dash,
    Dot,
    DashDot,
}

/// `BorderImageEffect`.
#[derive(Clone, Debug)]
pub struct BorderImageEffect {
    pub r#type: BorderType,
    pub size: i32,
    pub dash_style: DashStyle,
    pub color: SkColor,
}

impl Default for BorderImageEffect {
    fn default() -> Self {
        BorderImageEffect {
            r#type: BorderType::Outside,
            size: 5,
            dash_style: DashStyle::Solid,
            color: SkColor::argb(0xFF000000),
        }
    }
}

impl EffectImpl for BorderImageEffect {
    fn info() -> Info {
        Info::new(
            "border",
            "Border",
            EffectCategory::Drawings,
            "image-effect/frame",
        )
        .desc("Adds a border to the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            enum_param::<Self, BorderType>(
                "type",
                "Type",
                BorderType::Outside,
                |e: &mut Self, v| e.r#type = v,
                &[
                    ("Outside", BorderType::Outside),
                    ("Inside", BorderType::Inside),
                ],
            ),
            int_slider::<Self>("size", "Size", 1, 100, 5, |e: &mut Self, v| e.size = v),
            enum_param::<Self, DashStyle>(
                "dash_style",
                "Dash style",
                DashStyle::Solid,
                |e: &mut Self, v| e.dash_style = v,
                &[
                    ("Solid", DashStyle::Solid),
                    ("Dash", DashStyle::Dash),
                    ("Dot", DashStyle::Dot),
                    ("Dash Dot", DashStyle::DashDot),
                ],
            ),
            color_param::<Self>(
                "color",
                "Color",
                SkColor::argb(0xFF000000),
                |e: &mut Self, v| e.color = v,
            ),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        if self.size <= 0 {
            return Ok(source.clone());
        }
        let size = self.size;
        let outside = self.r#type == BorderType::Outside;
        let (iw, ih) = (source.width() as i32, source.height() as i32);
        let nw = if outside { iw + size * 2 } else { iw };
        let nh = if outside { ih + size * 2 } else { ih };
        let mut out = Bitmap::new(nw.max(0) as u32, nh.max(0) as u32);
        let mut canvas = Canvas::new(&mut out);
        let off = if outside { size } else { 0 };
        canvas.draw_bitmap(source, off as f32, off as f32, None);
        let paint = solid(self.color, true);
        let sf = size as f32;
        let mut pen = Pen::new(sf);
        let intervals = match self.dash_style {
            DashStyle::Dash => Some(vec![sf * 3.0, sf]),
            DashStyle::Dot => Some(vec![sf, sf]),
            DashStyle::DashDot => Some(vec![sf * 3.0, sf, sf, sf]),
            DashStyle::Solid => None,
        };
        if let Some(i) = intervals {
            pen = pen.dash(i, 0.0);
        }
        let half = sf / 2.0;
        let rect = if outside {
            RRect::ltrb(half, half, nw as f32 - half, nh as f32 - half)
        } else {
            RRect::ltrb(half, half, iw as f32 - half, ih as f32 - half)
        };
        sk::draw_rect(&mut canvas, rect, &paint, Some(&pen));
        drop(canvas);
        Ok(out)
    }
}
