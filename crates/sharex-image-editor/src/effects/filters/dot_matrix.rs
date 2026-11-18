//! `DotMatrixImageEffect` (`Core/ImageEffects/Filters/DotMatrixImageEffect.cs`).

use crate::effects::prelude::*;

/// `DotMatrixImageEffect`.
#[derive(Clone, Debug)]
pub struct DotMatrixImageEffect {
    pub cell_size: i32,
    pub dot_scale: f32,
    pub color_mode: bool,
    pub dot_color: SkColor,
    pub paper_color: SkColor,
}

impl Default for DotMatrixImageEffect {
    fn default() -> Self {
        DotMatrixImageEffect {
            cell_size: 8,
            dot_scale: 0.85,
            color_mode: false,
            dot_color: SkColor::argb(0xFF141414),
            paper_color: SkColor::argb(0xFFFAF5EB),
        }
    }
}

impl EffectImpl for DotMatrixImageEffect {
    fn info() -> Info {
        Info::new(
            "dot_matrix",
            "Dot Matrix",
            EffectCategory::Filters,
            "app/printer",
        )
        .desc("Simulates a vintage dot matrix printer output.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("cell_size", "Cell Size", 3, 30, 8, |e: &mut Self, v| {
                e.cell_size = v
            }),
            float_slider::<Self>(
                "dot_scale",
                "Dot Scale",
                0.1,
                1.0,
                0.85,
                |e: &mut Self, v| e.dot_scale = v,
            )
            .tick(0.05)
            .snap(false)
            .format("{}{0:0.00}"),
            bool_param::<Self>("color_mode", "Color", false, |e: &mut Self, v| {
                e.color_mode = v
            }),
            color_param::<Self>(
                "dot_color",
                "Dot Color",
                SkColor::argb(0xFF141414),
                |e: &mut Self, v| e.dot_color = v,
            ),
            color_param::<Self>(
                "paper_color",
                "Paper Color",
                SkColor::argb(0xFFFAF5EB),
                |e: &mut Self, v| e.paper_color = v,
            ),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let (width, height) = (source.width() as i32, source.height() as i32);
        let size = self.cell_size.clamp(2, 100);
        let dot_scale = clampf(self.dot_scale, 0.05, 1.0);
        let max_r = size as f32 * 0.5 * dot_scale;
        let src = pixels(source);
        let cols = (width + size - 1) / size;
        let rows = (height + size - 1) / size;
        let mut out = Bitmap::new(source.width(), source.height());
        let mut canvas = Canvas::new(&mut out);
        canvas.clear(self.paper_color.rgba_array());
        for gy in 0..rows {
            for gx in 0..cols {
                let (x0, y0) = (gx * size, gy * size);
                let (x1, y1) = ((x0 + size).min(width), (y0 + size).min(height));
                let (mut sr, mut sg, mut sb, mut n) = (0i64, 0i64, 0i64, 0i64);
                for py in y0..y1 {
                    for px in x0..x1 {
                        let c = src[(py * width + px) as usize];
                        sr += c.r as i64;
                        sg += c.g as i64;
                        sb += c.b as i64;
                        n += 1;
                    }
                }
                if n == 0 {
                    continue;
                }
                let (ar, ag, ab) = ((sr / n) as u8, (sg / n) as u8, (sb / n) as u8);
                let lum = (0.299 * ar as f32 + 0.587 * ag as f32 + 0.114 * ab as f32) / 255.0;
                let radius = (1.0 - lum) * max_r;
                if radius < 0.3 {
                    continue;
                }
                let color = if self.color_mode {
                    SkColor::rgba(ar, ag, ab, 255)
                } else {
                    self.dot_color
                };
                let cx = x0 as f32 + size as f32 * 0.5;
                let cy = y0 as f32 + size as f32 * 0.5;
                sk::draw_circle(&mut canvas, cx, cy, radius, &solid(color, true), None);
            }
        }
        drop(canvas);
        Ok(out)
    }
}
