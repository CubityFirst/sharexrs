//! `DrawLineEffect` (`Core/ImageEffects/Drawings/DrawLineEffect.cs`).

use crate::effects::prelude::*;

/// `DrawLineEffect`.
#[derive(Clone, Debug)]
pub struct DrawLineEffect {
    pub start_point: PointI,
    pub end_point: PointI,
    pub color: SkColor,
    pub thickness: f32,
}

impl Default for DrawLineEffect {
    fn default() -> Self {
        DrawLineEffect {
            start_point: PointI::new(0, 0),
            end_point: PointI::new(200, 0),
            color: SkColor::argb(0xFFFFFFFF),
            thickness: 4.0,
        }
    }
}

impl EffectImpl for DrawLineEffect {
    fn info() -> Info {
        Info::new(
            "draw_line",
            "Line",
            EffectCategory::Drawings,
            "image-editor/line",
        )
        .desc("Draws a line on the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_numeric::<Self>("start_x", "Start X", -10000, 10000, 0, |e: &mut Self, v| {
                e.start_point.x = v
            }),
            int_numeric::<Self>("start_y", "Start Y", -10000, 10000, 0, |e: &mut Self, v| {
                e.start_point.y = v
            }),
            int_numeric::<Self>("end_x", "End X", -10000, 10000, 200, |e: &mut Self, v| {
                e.end_point.x = v
            }),
            int_numeric::<Self>("end_y", "End Y", -10000, 10000, 0, |e: &mut Self, v| {
                e.end_point.y = v
            }),
            color_param::<Self>(
                "color",
                "Color",
                SkColor::argb(0xFFFFFFFF),
                |e: &mut Self, v| e.color = v,
            ),
            float_slider::<Self>(
                "thickness",
                "Thickness",
                0.5,
                100.0,
                4.0,
                |e: &mut Self, v| e.thickness = v,
            ),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        if self.thickness <= 0.0 || self.color.a == 0 {
            return Ok(source.clone());
        }
        let mut out = source.clone();
        let mut canvas = Canvas::new(&mut out);
        let pen = Pen::new(self.thickness).cap(Cap::Round).join(Join::Round);
        let (s, e) = (self.start_point, self.end_point);
        sk::draw_line(
            &mut canvas,
            s.x as f32,
            s.y as f32,
            e.x as f32,
            e.y as f32,
            &pen,
            &solid(self.color, true),
        );
        drop(canvas);
        Ok(out)
    }
}
