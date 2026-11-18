//! `SnowflakesImageEffect` (`Core/ImageEffects/Drawings/SnowflakesImageEffect.cs`).

use crate::effects::prelude::*;

/// `SnowflakesImageEffect`.
#[derive(Clone, Debug)]
pub struct SnowflakesImageEffect {
    pub count: i32,
    pub min_size: i32,
    pub max_size: i32,
    pub opacity: f32,
    pub random_angle: bool,
    pub background: bool,
}

impl Default for SnowflakesImageEffect {
    fn default() -> Self {
        SnowflakesImageEffect {
            count: 80,
            min_size: 3,
            max_size: 14,
            opacity: 85.0,
            random_angle: true,
            background: false,
        }
    }
}

impl EffectImpl for SnowflakesImageEffect {
    fn info() -> Info {
        Info::new("snowflakes", "Snowflakes", EffectCategory::Drawings, "image-effect/snowflake")
            .desc("Scatters snowflakes over the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("count", "Count", 10, 500, 80, |e: &mut Self, v| e.count = v),
            int_slider::<Self>("min_size", "Min size", 2, 20, 3, |e: &mut Self, v| e.min_size = v),
            int_slider::<Self>("max_size", "Max size", 4, 60, 14, |e: &mut Self, v| e.max_size = v),
            float_slider::<Self>("opacity", "Opacity", 0.0, 100.0, 85.0, |e: &mut Self, v| e.opacity = v),
            bool_param::<Self>("random_angle", "Random angle", true, |e: &mut Self, v| e.random_angle = v),
            bool_param::<Self>("background", "Background", false, |e: &mut Self, v| e.background = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        if self.count <= 0 {
            return Ok(source.clone());
        }
        let alpha = clampf(self.opacity / 100.0, 0.0, 1.0);
        if alpha <= 0.0 {
            return Ok(source.clone());
        }
        let min_size = 2.max(self.min_size.min(self.max_size));
        let max_size = min_size.max(self.max_size);
        let mut out = source.clone();
        let mut canvas = Canvas::new(&mut out);
        if self.background {
            canvas.clear([255, 255, 255, 0]);
        }
        let (sw, sh) = (source.width() as i32, source.height() as i32);
        for _ in 0..self.count {
            let (x, y, size, fa, angle) = random::shared(|r| {
                let x = r.next_range(0, sw) as f32;
                let y = r.next_range(0, sh) as f32;
                let size = r.next_range(min_size, max_size + 1) as f32;
                let fa = alpha * (0.5 + r.next_single() * 0.5);
                let angle = if self.random_angle { r.next_single() * 360.0 } else { 0.0 };
                (x, y, size, fa, angle)
            });
            snowflake(&mut canvas, x, y, size, fa, angle);
        }
        if self.background {
            canvas.draw_bitmap(source, 0.0, 0.0, None);
        }
        drop(canvas);
        Ok(out)
    }
}

// ---- port ----

fn snowflake(canvas: &mut Canvas, cx: f32, cy: f32, size: f32, alpha: f32, rotation: f32) {
    let a = (255.0 * alpha) as u8;
    let saved = canvas.matrix;
    canvas.translate(cx, cy);
    canvas.rotate(rotation);
    canvas.translate(-cx, -cy);
    let glow = solid(SkColor::rgba(200, 220, 255, (a as f32 * 0.3) as u8), true);
    sk::draw_circle_blur(canvas, cx, cy, size * 0.6, &glow, None, size * 0.4);
    let line = solid(SkColor::rgba(240, 248, 255, a), true);
    let pen = Pen::new(1f32.max(size * 0.1)).cap(Cap::Round);
    let half = size / 2.0;
    for b in 0..6 {
        let angle = (b * 60) as f32 * std::f32::consts::PI / 180.0;
        let (cos, sin) = (angle.cos(), angle.sin());
        sk::draw_line(canvas, cx, cy, cx + cos * half, cy + sin * half, &pen, &line);
        if size >= 6.0 {
            let bl = half * 0.35;
            let mx = cx + cos * half * 0.55;
            let my = cy + sin * half * 0.55;
            for side in [-1.0f32, 1.0] {
                let sa = angle + side * std::f32::consts::PI / 3.0;
                sk::draw_line(canvas, mx, my, mx + sa.cos() * bl, my + sa.sin() * bl, &pen, &line);
            }
        }
    }
    let dot = solid(SkColor::rgba(255, 255, 255, a), true);
    sk::draw_circle(canvas, cx, cy, 1f32.max(size * 0.08), &dot, None);
    canvas.matrix = saved;
}
