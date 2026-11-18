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
        Info::new("draw_line", "Line", EffectCategory::Drawings, "image-editor/line")
            .desc("Draws a line on the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_numeric::<Self>("start_x", "Start X", -10000, 10000, 0, |e: &mut Self, v| e.start_point.x = v),
            int_numeric::<Self>("start_y", "Start Y", -10000, 10000, 0, |e: &mut Self, v| e.start_point.y = v),
            int_numeric::<Self>("end_x", "End X", -10000, 10000, 200, |e: &mut Self, v| e.end_point.x = v),
            int_numeric::<Self>("end_y", "End Y", -10000, 10000, 0, |e: &mut Self, v| e.end_point.y = v),
            color_param::<Self>("color", "Color", SkColor::argb(0xFFFFFFFF), |e: &mut Self, v| e.color = v),
            float_slider::<Self>("thickness", "Thickness", 0.5, 100.0, 4.0, |e: &mut Self, v| e.thickness = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
