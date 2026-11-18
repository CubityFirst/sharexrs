//! `TornEdgeImageEffect` (`Core/ImageEffects/Filters/TornEdgeImageEffect.cs`).

use crate::effects::prelude::*;

/// `TornEdgeImageEffect`.
#[derive(Clone, Debug)]
pub struct TornEdgeImageEffect {
    pub depth: i32,
    pub range: i32,
    pub top: bool,
    pub right: bool,
    pub bottom: bool,
    pub left: bool,
    pub curved: bool,
}

impl Default for TornEdgeImageEffect {
    fn default() -> Self {
        TornEdgeImageEffect {
            depth: 20,
            range: 20,
            top: true,
            right: true,
            bottom: true,
            left: true,
            curved: false,
        }
    }
}

impl EffectImpl for TornEdgeImageEffect {
    fn info() -> Info {
        Info::new(
            "torn_edge",
            "Torn edge",
            EffectCategory::Filters,
            "image-effect/torn-edge",
        )
        .desc("Adds a torn edge border effect.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("depth", "Depth", 1, 100, 20, |e: &mut Self, v| e.depth = v)
                .snap(false),
            int_slider::<Self>("range", "Range", 1, 100, 20, |e: &mut Self, v| e.range = v)
                .snap(false),
            bool_param::<Self>("top", "Top", true, |e: &mut Self, v| e.top = v),
            bool_param::<Self>("right", "Right", true, |e: &mut Self, v| e.right = v),
            bool_param::<Self>("bottom", "Bottom", true, |e: &mut Self, v| e.bottom = v),
            bool_param::<Self>("left", "Left", true, |e: &mut Self, v| e.left = v),
            bool_param::<Self>("curved", "Curved edges", false, |e: &mut Self, v| {
                e.curved = v
            }),
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
        let (depth, range) = (self.depth, self.range);
        let h_count = sw / range;
        let v_count = sh / range;
        if h_count < 2 && v_count < 2 {
            return Ok(source.clone());
        }
        let p = |x: i32, y: i32| sharex_skia::Point::new(x as f32, y as f32);
        let mut pts: Vec<sharex_skia::Point> = Vec::new();
        random::shared(|rand| {
            if self.top && h_count > 1 {
                let start = if self.left && v_count > 1 { depth } else { 0 };
                let end = if self.right && v_count > 1 {
                    sw - depth
                } else {
                    sw
                };
                let mut x = start;
                while x < end {
                    pts.push(p(x, rand.next_range(0, depth + 1)));
                    x += range;
                }
            } else {
                pts.push(p(0, 0));
                pts.push(p(sw, 0));
            }
            if self.right && v_count > 1 {
                let start = if self.top && h_count > 1 { depth } else { 0 };
                let end = if self.bottom && h_count > 1 {
                    sh - depth
                } else {
                    sh
                };
                let mut y = start;
                while y < end {
                    pts.push(p(sw - depth + rand.next_range(0, depth + 1), y));
                    y += range;
                }
            } else {
                pts.push(p(sw, 0));
                pts.push(p(sw, sh));
            }
            if self.bottom && h_count > 1 {
                let start = if self.right && v_count > 1 {
                    sw - depth
                } else {
                    sw
                };
                let end = if self.left && v_count > 1 { depth } else { 0 };
                let mut x = start;
                while x >= end {
                    pts.push(p(x, sh - depth + rand.next_range(0, depth + 1)));
                    x -= range;
                }
            } else {
                pts.push(p(sw, sh));
                pts.push(p(0, sh));
            }
            if self.left && v_count > 1 {
                let start = if self.bottom && h_count > 1 {
                    sh - depth
                } else {
                    sh
                };
                let end = if self.top && h_count > 1 { depth } else { 0 };
                let mut y = start;
                while y >= end {
                    pts.push(p(rand.next_range(0, depth + 1), y));
                    y -= range;
                }
            } else {
                pts.push(p(0, sh));
                pts.push(p(0, 0));
            }
        });
        let mut distinct: Vec<sharex_skia::Point> = Vec::new();
        if !pts.is_empty() {
            distinct.push(pts[0]);
            for i in 1..pts.len() {
                if pts[i] != pts[i - 1] {
                    distinct.push(pts[i]);
                }
            }
            if distinct.len() > 1 && distinct[distinct.len() - 1] == distinct[0] {
                distinct.pop();
            }
        }
        let mut out = Bitmap::new(source.width(), source.height());
        if distinct.len() > 2 {
            let mut path = SkPath::new();
            if self.curved {
                let n = distinct.len();
                let (last, first) = (distinct[n - 1], distinct[0]);
                path.move_to((last.x + first.x) / 2.0, (last.y + first.y) / 2.0);
                for i in 0..n {
                    let cur = distinct[i];
                    let next = distinct[(i + 1) % n];
                    path.quad_to(cur.x, cur.y, (cur.x + next.x) / 2.0, (cur.y + next.y) / 2.0);
                }
                path.close();
            } else {
                path.add_poly(&distinct, true);
            }
            let paint = Paint::texture(source, TileMode::Clamp);
            Canvas::new(&mut out).fill_sk_path(&path, &paint);
        }
        Ok(out)
    }
}
