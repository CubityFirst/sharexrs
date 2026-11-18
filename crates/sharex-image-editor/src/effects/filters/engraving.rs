//! `EngravingImageEffect` (`Core/ImageEffects/Filters/EngravingImageEffect.cs`).

use crate::effects::prelude::*;

/// `EngravingImageEffect`.
#[derive(Clone, Debug)]
pub struct EngravingImageEffect {
    pub line_spacing: f32,
    pub angle: f32,
    pub line_width_max: f32,
    pub line_color: SkColor,
    pub paper_color: SkColor,
}

impl Default for EngravingImageEffect {
    fn default() -> Self {
        EngravingImageEffect {
            line_spacing: 5.0,
            angle: 45.0,
            line_width_max: 2.5,
            line_color: SkColor::argb(0xFF141428),
            paper_color: SkColor::argb(0xFFF5F0E6),
        }
    }
}

impl EffectImpl for EngravingImageEffect {
    fn info() -> Info {
        Info::new(
            "engraving",
            "Engraving",
            EffectCategory::Filters,
            "app/annotate",
        )
        .desc("Simulates a line engraving effect like banknote illustrations.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>(
                "line_spacing",
                "Line Spacing",
                2.0,
                20.0,
                5.0,
                |e: &mut Self, v| e.line_spacing = v,
            )
            .tick(0.5)
            .snap(false)
            .format("{}{0:0.0}"),
            float_slider::<Self>("angle", "Angle", 0.0, 180.0, 45.0, |e: &mut Self, v| {
                e.angle = v
            }),
            float_slider::<Self>(
                "line_width_max",
                "Max Line Width",
                0.5,
                8.0,
                2.5,
                |e: &mut Self, v| e.line_width_max = v,
            )
            .tick(0.1)
            .snap(false)
            .format("{}{0:0.0}"),
            color_param::<Self>(
                "line_color",
                "Line Color",
                SkColor::argb(0xFF141428),
                |e: &mut Self, v| e.line_color = v,
            ),
            color_param::<Self>(
                "paper_color",
                "Paper Color",
                SkColor::argb(0xFFF5F0E6),
                |e: &mut Self, v| e.paper_color = v,
            ),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let (width, height) = (source.width() as i32, source.height() as i32);
        let spacing = clampf(self.line_spacing, 1.0, 50.0);
        let max_w = clampf(self.line_width_max, 0.1, 20.0);
        let rad = self.angle * std::f32::consts::PI / 180.0;
        let lum: Vec<f32> = pixels(source)
            .iter()
            .map(|c| (0.299 * c.r as f32 + 0.587 * c.g as f32 + 0.114 * c.b as f32) / 255.0)
            .collect();
        let mut out = Bitmap::new(source.width(), source.height());
        let mut canvas = Canvas::new(&mut out);
        canvas.clear(self.paper_color.rgba_array());
        let (cos_a, sin_a) = (rad.cos(), rad.sin());
        let diagonal = ((width
            .wrapping_mul(width)
            .wrapping_add(height.wrapping_mul(height))) as f32)
            .sqrt();
        let line_count = (diagonal / spacing).ceil() as i32 + 1;
        let paint = solid(self.line_color, true);
        let (cx, cy) = (width as f32 * 0.5, height as f32 * 0.5);
        for i in -line_count..=line_count {
            let offset = i as f32 * spacing;
            let bx = cx + cos_a * offset;
            let by = cy + sin_a * offset;
            let x0 = bx - sin_a * diagonal;
            let y0 = by + cos_a * diagonal;
            let x1 = bx + sin_a * diagonal;
            let y1 = by - cos_a * diagonal;
            let steps = (diagonal * 2.0 / spacing) as i32 + 1;
            let (mut prev_x, mut prev_y) = (x0, y0);
            for s in 1..=steps {
                let t = s as f32 / steps as f32;
                let px = x0 + (x1 - x0) * t;
                let py = y0 + (y1 - y0) * t;
                let sx = px.round_ties_even() as i32;
                let sy = py.round_ties_even() as i32;
                if sx >= 0 && sx < width && sy >= 0 && sy < height {
                    let sw = (1.0 - lum[(sy * width + sx) as usize]) * max_w;
                    if sw > 0.05 {
                        let pen = Pen::new(sw).cap(Cap::Round);
                        sk::draw_line(&mut canvas, prev_x, prev_y, px, py, &pen, &paint);
                    }
                }
                prev_x = px;
                prev_y = py;
            }
        }
        drop(canvas);
        Ok(out)
    }
}
