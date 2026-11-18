//! `EmbroideryImageEffect` (`Core/ImageEffects/Filters/EmbroideryImageEffect.cs`).

use crate::effects::prelude::*;

/// `EmbroideryImageEffect`.
#[derive(Clone, Debug)]
pub struct EmbroideryImageEffect {
    pub stitch_size: i32,
    pub color_count: i32,
    pub show_grid: bool,
    pub fabric_color: SkColor,
}

impl Default for EmbroideryImageEffect {
    fn default() -> Self {
        EmbroideryImageEffect {
            stitch_size: 10,
            color_count: 16,
            show_grid: true,
            fabric_color: SkColor::argb(0xFFF0EBDC),
        }
    }
}

impl EffectImpl for EmbroideryImageEffect {
    fn info() -> Info {
        Info::new("embroidery", "Embroidery", EffectCategory::Filters, "app/stamp")
            .desc("Simulates a cross-stitch embroidery pattern.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("stitch_size", "Stitch Size", 4, 40, 10, |e: &mut Self, v| e.stitch_size = v),
            int_slider::<Self>("color_count", "Color Count", 2, 64, 16, |e: &mut Self, v| e.color_count = v),
            bool_param::<Self>("show_grid", "Show Grid", true, |e: &mut Self, v| e.show_grid = v),
            color_param::<Self>("fabric_color", "Fabric Color", SkColor::argb(0xFFF0EBDC), |e: &mut Self, v| e.fabric_color = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let (width, height) = (source.width() as i32, source.height() as i32);
        let size = self.stitch_size.clamp(2, 100);
        let color_count = self.color_count.clamp(2, 256);
        let src = pixels(source);
        let levels = 2.max((color_count as f32).powf(1.0 / 3.0).ceil() as i32);
        let mut out = Bitmap::new(source.width(), source.height());
        let mut canvas = Canvas::new(&mut out);
        canvas.clear(self.fabric_color.rgba_array());
        let cols = (width + size - 1) / size;
        let rows = (height + size - 1) / size;
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
                let color = SkColor::rgba(
                    quantize((sr / n) as u8, levels),
                    quantize((sg / n) as u8, levels),
                    quantize((sb / n) as u8, levels),
                    255,
                );
                let margin = size as f32 * 0.1;
                let (sx, sy) = (x0 as f32 + margin, y0 as f32 + margin);
                let (ex, ey) = (x1 as f32 - margin, y1 as f32 - margin);
                let pen = Pen::new(1f32.max(size as f32 * 0.15)).cap(Cap::Round).join(Join::Round);
                let paint = solid(color, true);
                sk::draw_line(&mut canvas, sx, sy, ex, ey, &pen, &paint);
                sk::draw_line(&mut canvas, sx, ey, ex, sy, &pen, &paint);
            }
        }
        if self.show_grid {
            let f = self.fabric_color;
            let cl = |v: u8| (v as i32 - 30).clamp(0, 255) as u8;
            let paint = solid(SkColor::rgba(cl(f.r), cl(f.g), cl(f.b), 80), false);
            let pen = Pen::new(1.0);
            for gx in 0..=cols {
                let x = (gx * size).min(width - 1) as f32;
                sk::draw_line(&mut canvas, x, 0.0, x, height as f32, &pen, &paint);
            }
            for gy in 0..=rows {
                let y = (gy * size).min(height - 1) as f32;
                sk::draw_line(&mut canvas, 0.0, y, width as f32, y, &pen, &paint);
            }
        }
        drop(canvas);
        Ok(out)
    }
}

// ---- port ----

fn quantize(value: u8, levels: i32) -> u8 {
    let step = 255.0 / (levels - 1) as f32;
    let q = (value as f32 / step).round_ties_even() as i32;
    ((q as f32 * step).round_ties_even() as i32).clamp(0, 255) as u8
}
