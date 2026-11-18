//! `WaveEdgeImageEffect` (`Core/ImageEffects/Filters/WaveEdgeImageEffect.cs`).

use crate::effects::prelude::*;

/// `WaveEdgeImageEffect`.
#[derive(Clone, Debug)]
pub struct WaveEdgeImageEffect {
    pub depth: i32,
    pub range: i32,
    pub top: bool,
    pub right: bool,
    pub bottom: bool,
    pub left: bool,
}

impl Default for WaveEdgeImageEffect {
    fn default() -> Self {
        WaveEdgeImageEffect {
            depth: 15,
            range: 20,
            top: true,
            right: true,
            bottom: true,
            left: true,
        }
    }
}

impl EffectImpl for WaveEdgeImageEffect {
    fn info() -> Info {
        Info::new("wave_edge", "Wave edge", EffectCategory::Filters, "image-effect/waves")
            .desc("Adds a wavy edge to the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("depth", "Depth", 1, 100, 15, |e: &mut Self, v| e.depth = v).snap(false),
            int_slider::<Self>("range", "Range", 1, 100, 20, |e: &mut Self, v| e.range = v).snap(false),
            bool_param::<Self>("top", "Top", true, |e: &mut Self, v| e.top = v),
            bool_param::<Self>("right", "Right", true, |e: &mut Self, v| e.right = v),
            bool_param::<Self>("bottom", "Bottom", true, |e: &mut Self, v| e.bottom = v),
            bool_param::<Self>("left", "Left", true, |e: &mut Self, v| e.left = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        if self.depth < 1 || self.range < 1 {
            return Ok(source.clone());
        }
        if !self.top && !self.right && !self.bottom && !self.left {
            return Ok(source.clone());
        }
        let (sw, sh) = (source.width() as i32, source.height() as i32);
        let depth = self.depth;
        let mut pts: Vec<sharex_skia::Point> = Vec::new();
        let p = |x: f32, y: f32| sharex_skia::Point::new(x, y);
        let h_count = 2.max((sw / self.range + 1) / 2 * 2) - 1;
        let v_count = 2.max((sh / self.range + 1) / 2 * 2) - 1;
        let h_range = sw / h_count;
        let v_range = sh / v_count;
        let step = 1.max(self.range / depth).min(10);
        let wave = |t: i32, max: i32| ((1.0 - (t as f64 * std::f64::consts::PI / max as f64).cos()) * depth as f64 / 2.0) as f32;
        if self.top {
            let start = if self.left { depth } else { 0 };
            let end = if self.right { sw - depth } else { sw };
            let mut x = start;
            while x < end {
                pts.push(p(x as f32, wave(x, h_range)));
                x += step;
            }
            pts.push(p(end as f32, wave(end, h_range)));
        } else {
            pts.push(p(0.0, 0.0));
        }
        if self.right {
            let start = if self.top { depth } else { 0 };
            let end = if self.bottom { sh - depth } else { sh };
            let mut y = start;
            while y < end {
                pts.push(p((sw - depth) as f32 + wave(y, v_range), y as f32));
                y += step;
            }
            pts.push(p((sw - depth) as f32 + wave(end, v_range), end as f32));
        } else {
            let last = pts[pts.len() - 1];
            pts.push(p(sw as f32, last.y));
        }
        if self.bottom {
            let start = if self.right { sw - depth } else { sw };
            let end = if self.left { depth } else { 0 };
            let mut x = start;
            while x >= end {
                pts.push(p(x as f32, (sh - depth) as f32 + wave(x, h_range)));
                x -= step;
            }
            pts.push(p(end as f32, (sh - depth) as f32 + wave(end, h_range)));
        } else {
            let last = pts[pts.len() - 1];
            pts.push(p(last.x, sh as f32));
        }
        if self.left {
            let start = if self.bottom { sh - depth } else { sh };
            let end = if self.top { depth } else { 0 };
            let mut y = start;
            while y >= end {
                pts.push(p(wave(y, v_range), y as f32));
                y -= step;
            }
            pts.push(p(wave(end, v_range), end as f32));
        } else {
            let last = pts[pts.len() - 1];
            pts.push(p(0.0, last.y));
        }
        if !self.top {
            let last = pts[pts.len() - 1];
            pts[0] = p(last.x, 0.0);
        }
        let mut out = Bitmap::new(source.width(), source.height());
        if pts.len() <= 2 {
            return Ok(source.clone());
        }
        let mut path = SkPath::new();
        path.add_poly(&pts, true);
        let paint = Paint::texture(source, TileMode::Clamp);
        Canvas::new(&mut out).fill_sk_path(&path, &paint);
        Ok(out)
    }
}
