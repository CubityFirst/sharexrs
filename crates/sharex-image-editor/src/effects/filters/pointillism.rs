//! `PointillismImageEffect` (`Core/ImageEffects/Filters/PointillismImageEffect.cs`).

use crate::effects::prelude::*;

/// `PointillismImageEffect`.
#[derive(Clone, Debug)]
pub struct PointillismImageEffect {
    pub dot_size: i32,
    pub density: f32,
    pub jitter: f32,
    pub color_boost: f32,
    pub background_mix: f32,
    pub seed: i32,
}

impl Default for PointillismImageEffect {
    fn default() -> Self {
        PointillismImageEffect {
            dot_size: 7,
            density: 72.0,
            jitter: 65.0,
            color_boost: 20.0,
            background_mix: 20.0,
            seed: 2026,
        }
    }
}

impl EffectImpl for PointillismImageEffect {
    fn info() -> Info {
        Info::new(
            "pointillism",
            "Pointillism",
            EffectCategory::Filters,
            "image-effect/circle-dot",
        )
        .desc("Renders the image as a pointillist painting with colored dots.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("dot_size", "Dot size", 2, 24, 7, |e: &mut Self, v| {
                e.dot_size = v
            }),
            float_slider::<Self>(
                "density",
                "Density",
                10.0,
                100.0,
                72.0,
                |e: &mut Self, v| e.density = v,
            ),
            float_slider::<Self>("jitter", "Jitter", 0.0, 100.0, 65.0, |e: &mut Self, v| {
                e.jitter = v
            }),
            float_slider::<Self>(
                "color_boost",
                "Color boost",
                0.0,
                100.0,
                20.0,
                |e: &mut Self, v| e.color_boost = v,
            ),
            float_slider::<Self>(
                "background_mix",
                "Background mix",
                0.0,
                100.0,
                20.0,
                |e: &mut Self, v| e.background_mix = v,
            ),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let dot_size = self.dot_size.clamp(2, 24);
        let density = clampf(self.density, 10.0, 100.0) / 100.0;
        let jitter = clampf(self.jitter, 0.0, 100.0) / 100.0;
        let boost = clampf(self.color_boost, 0.0, 100.0) / 100.0;
        let bg_mix = clampf(self.background_mix, 0.0, 100.0) / 100.0;
        let (width, height) = (source.width() as i32, source.height() as i32);
        if width <= 0 || height <= 0 {
            return Ok(source.clone());
        }
        let src = pixels(source);
        let mut out = Bitmap::new(source.width(), source.height());
        let mut canvas = Canvas::new(&mut out);
        if bg_mix > 0.0 {
            let a = clamp_to_byte(bg_mix * 255.0);
            let mut p = solid(SkColor::rgba(255, 255, 255, a), true);
            p.alpha = a;
            canvas.draw_bitmap(source, 0.0, 0.0, Some(&p));
        }
        let step = 2.max(dot_size);
        let jitter_amount = jitter * step as f32 * 0.6;
        let seed = self.seed;
        let mut gy = 0;
        while gy < height {
            let mut gx = 0;
            while gx < width {
                let (cell_x, cell_y) = (gx / step, gy / step);
                if hash01(cell_x, cell_y, seed) > density {
                    gx += step;
                    continue;
                }
                let jx = (hash01(cell_x, cell_y, seed ^ 111) * 2.0 - 1.0) * jitter_amount;
                let jy = (hash01(cell_x, cell_y, seed ^ 257) * 2.0 - 1.0) * jitter_amount;
                let cx = clampf(gx as f32 + step as f32 * 0.5 + jx, 0.0, width as f32 - 1.0);
                let cy = clampf(gy as f32 + step as f32 * 0.5 + jy, 0.0, height as f32 - 1.0);
                let mut sample = bilinear_sample(&src, width, height, cx, cy);
                if boost > 0.0 {
                    sample = boost_saturation(sample, boost);
                }
                let rj = 0.6 + hash01(cell_x, cell_y, seed ^ 509) * 0.9;
                let radius = 1f32.max(dot_size as f32 * 0.36 * rj);
                sk::draw_circle(&mut canvas, cx, cy, radius, &solid(sample, true), None);
                gx += step;
            }
            gy += step;
        }
        drop(canvas);
        Ok(out)
    }
}

// ---- port ----

fn boost_saturation(c: SkColor, amount: f32) -> SkColor {
    let (r, g, b) = (c.r as f32, c.g as f32, c.b as f32);
    let gray = (r + g + b) / 3.0;
    let f = 1.0 + amount;
    SkColor::rgba(
        clamp_to_byte(gray + (r - gray) * f),
        clamp_to_byte(gray + (g - gray) * f),
        clamp_to_byte(gray + (b - gray) * f),
        c.a,
    )
}
