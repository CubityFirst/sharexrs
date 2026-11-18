//! `MosaicPolygonImageEffect` (`Core/ImageEffects/Filters/MosaicPolygonImageEffect.cs`).

use crate::effects::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MosaicPolygonShape {
    Hexagon,
    Triangle,
}

/// `MosaicPolygonImageEffect`.
#[derive(Clone, Debug)]
pub struct MosaicPolygonImageEffect {
    pub cell_size: i32,
    pub shape: MosaicPolygonShape,
    pub border_width: f32,
    pub border_opacity: f32,
    pub randomness: f32,
    pub seed: i32,
}

impl Default for MosaicPolygonImageEffect {
    fn default() -> Self {
        MosaicPolygonImageEffect {
            cell_size: 24,
            shape: MosaicPolygonShape::Hexagon,
            border_width: 1.0,
            border_opacity: 45.0,
            randomness: 18.0,
            seed: 9031,
        }
    }
}

impl EffectImpl for MosaicPolygonImageEffect {
    fn info() -> Info {
        Info::new(
            "mosaic_polygon",
            "Mosaic polygon",
            EffectCategory::Filters,
            "image-effect/hexagon",
        )
        .desc("Creates a mosaic using hexagonal or triangular polygon tiles.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("cell_size", "Cell size", 8, 80, 24, |e: &mut Self, v| {
                e.cell_size = v
            }),
            enum_param::<Self, MosaicPolygonShape>(
                "shape",
                "Shape",
                MosaicPolygonShape::Hexagon,
                |e: &mut Self, v| e.shape = v,
                &[
                    ("Hexagon", MosaicPolygonShape::Hexagon),
                    ("Triangle", MosaicPolygonShape::Triangle),
                ],
            ),
            float_slider::<Self>(
                "border_width",
                "Border width",
                0.0,
                6.0,
                1.0,
                |e: &mut Self, v| e.border_width = v,
            ),
            float_slider::<Self>(
                "border_opacity",
                "Border opacity",
                0.0,
                100.0,
                45.0,
                |e: &mut Self, v| e.border_opacity = v,
            ),
            float_slider::<Self>(
                "randomness",
                "Randomness",
                0.0,
                100.0,
                18.0,
                |e: &mut Self, v| e.randomness = v,
            ),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let (width, height) = (source.width() as i32, source.height() as i32);
        if width <= 0 || height <= 0 {
            return Ok(source.clone());
        }
        let cell = self.cell_size.clamp(8, 80);
        let bw = clampf(self.border_width, 0.0, 6.0);
        let bo = clampf(self.border_opacity, 0.0, 100.0) / 100.0;
        let randomness = clampf(self.randomness, 0.0, 100.0) / 100.0;
        let src = pixels(source);
        let mut out = Bitmap::new(source.width(), source.height());
        let mut canvas = Canvas::new(&mut out);
        let stroke_color = SkColor::rgba(0, 0, 0, clamp_to_byte(255.0 * bo));
        let stroke = solid(stroke_color, true);
        let pen = Pen::new(bw).join(Join::Round);
        let draw_stroke = bw > 0.0 && stroke_color.a > 0;
        let seed = self.seed;
        let (wf, hf) = (width as f32, height as f32);
        match self.shape {
            MosaicPolygonShape::Hexagon => {
                let radius = cell as f32 * 0.5;
                let vertical = 1.732_050_8 * radius;
                let horizontal = 1.5 * radius;
                let jitter = randomness * radius * 0.45;
                let mut col = -1i32;
                loop {
                    let center_x = radius + col as f32 * horizontal;
                    if center_x > wf + radius {
                        break;
                    }
                    let start_y = radius + if col & 1 != 0 { vertical * 0.5 } else { 0.0 };
                    let mut center_y = start_y - vertical;
                    while center_y < hf + radius {
                        let gx = col + 4096;
                        let gy = ((center_y + vertical) / vertical.max(0.0001)).floor() as i32;
                        let ox = (hash01(gx, gy, seed ^ 17) * 2.0 - 1.0) * jitter;
                        let oy = (hash01(gx, gy, seed ^ 73) * 2.0 - 1.0) * jitter;
                        let (cx, cy) = (center_x + ox, center_y + oy);
                        let color = bilinear_sample(&src, width, height, cx, cy);
                        let mut path = SkPath::new();
                        for i in 0..6 {
                            let a = (std::f32::consts::PI / 3.0) * i as f32
                                - std::f32::consts::PI / 6.0;
                            let (x, y) = (cx + a.cos() * radius, cy + a.sin() * radius);
                            if i == 0 {
                                path.move_to(x, y);
                            } else {
                                path.line_to(x, y);
                            }
                        }
                        path.close();
                        canvas.fill_sk_path(&path, &solid(color, true));
                        if draw_stroke {
                            sk::stroke_path(&mut canvas, &path, &pen, &stroke);
                        }
                        center_y += vertical;
                    }
                    col += 1;
                }
            }
            MosaicPolygonShape::Triangle => {
                let side = cell as f32;
                let tri_h = side * 0.866_025_4;
                let half = side * 0.5;
                let jitter = randomness * side * 0.30;
                let rows = ((hf / tri_h) as f64).ceil() as i32 + 2;
                let cols = ((wf / half) as f64).ceil() as i32 + 2;
                for row in -1..rows {
                    let by = row as f32 * tri_h;
                    for col in -1..cols {
                        let bx = col as f32 * half;
                        let (p1, p2, p3) = if (row + col) & 1 == 0 {
                            ((bx, by + tri_h), (bx + half, by), (bx + side, by + tri_h))
                        } else {
                            ((bx, by), (bx + side, by), (bx + half, by + tri_h))
                        };
                        let mut cx = (p1.0 + p2.0 + p3.0) / 3.0;
                        let mut cy = (p1.1 + p2.1 + p3.1) / 3.0;
                        cx += (hash01(col, row, seed ^ 173) * 2.0 - 1.0) * jitter;
                        cy += (hash01(col, row, seed ^ 719) * 2.0 - 1.0) * jitter;
                        let color = bilinear_sample(&src, width, height, cx, cy);
                        let mut path = SkPath::new();
                        path.move_to(p1.0, p1.1);
                        path.line_to(p2.0, p2.1);
                        path.line_to(p3.0, p3.1);
                        path.close();
                        canvas.fill_sk_path(&path, &solid(color, true));
                        if draw_stroke {
                            sk::stroke_path(&mut canvas, &path, &pen, &stroke);
                        }
                    }
                }
            }
        }
        drop(canvas);
        Ok(out)
    }
}
