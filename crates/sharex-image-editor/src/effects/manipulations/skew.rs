//! `SkewImageEffect` (`Core/ImageEffects/Manipulations/SkewImageEffect.cs`).

use crate::effects::prelude::*;

/// `SkewImageEffect`.
#[derive(Clone, Debug)]
pub struct SkewImageEffect {
    pub horizontally: f32,
    pub vertically: f32,
    pub auto_resize: bool,
}

impl Default for SkewImageEffect {
    fn default() -> Self {
        SkewImageEffect {
            horizontally: 0.0,
            vertically: 0.0,
            auto_resize: true,
        }
    }
}

impl EffectImpl for SkewImageEffect {
    fn info() -> Info {
        Info::new("skew", "Skew", EffectCategory::Manipulations, "image-effect/move-diagonal")
            .desc("Skews the image horizontally and vertically.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("horizontally", "Horizontally", -89.0, 89.0, 0.0, |e: &mut Self, v| e.horizontally = v),
            float_slider::<Self>("vertically", "Vertically", -89.0, 89.0, 0.0, |e: &mut Self, v| e.vertically = v),
            bool_param::<Self>("auto_resize", "Auto resize", true, |e: &mut Self, v| e.auto_resize = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        if self.horizontally == 0.0 && self.vertically == 0.0 {
            return Ok(source.clone());
        }
        let (w, h) = (source.width() as f32, source.height() as f32);
        let kx = (self.horizontally as f64 * std::f64::consts::PI / 180.0).tan() as f32;
        let ky = (self.vertically as f64 * std::f64::consts::PI / 180.0).tan() as f32;
        let skew = Matrix([1.0, kx, 0.0, ky, 1.0, 0.0, 0.0, 0.0, 1.0]);
        let corners = [(0.0, 0.0), (w, 0.0), (w, h), (0.0, h)];
        let (mut min_x, mut min_y, mut max_x, mut max_y) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for (x, y) in corners {
            let (px, py) = skew.map(x, y);
            min_x = min_x.min(px);
            min_y = min_y.min(py);
            max_x = max_x.max(px);
            max_y = max_y.max(py);
        }
        let (nw, nh, ox, oy) = if self.auto_resize {
            (
                1.max(((max_x - min_x) as f64).ceil() as i32),
                1.max(((max_y - min_y) as f64).ceil() as i32),
                -min_x,
                -min_y,
            )
        } else {
            (w as i32, h as i32, 0.0, 0.0)
        };
        let mut out = Bitmap::new(nw as u32, nh as u32);
        {
            let mut canvas = Canvas::new(&mut out);
            canvas.translate(ox, oy);
            canvas.matrix = Matrix::concat(&canvas.matrix, &skew);
            canvas.draw_bitmap(source, 0.0, 0.0, None);
        }
        Ok(out)
    }
}
