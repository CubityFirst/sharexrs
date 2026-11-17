//! Clipping edges to the device before scan conversion: `SkLineClipper`
//! and `SkEdgeClipper`. Parts of an edge left (or, unless culled, right)
//! of the clip become vertical lines on the clip's side, so windings stay
//! right.

use crate::geom::{self, Point, Rect};

fn pin_unsorted(value: f64, mut l0: f64, mut l1: f64) -> f64 {
    if l1 < l0 {
        std::mem::swap(&mut l0, &mut l1);
    }
    if value < l0 {
        l0
    } else if value > l1 {
        l1
    } else {
        value
    }
}

fn pin_unsorted_f(value: f32, mut l0: f32, mut l1: f32) -> f32 {
    if l1 < l0 {
        std::mem::swap(&mut l0, &mut l1);
    }
    if value < l0 {
        l0
    } else if value > l1 {
        l1
    } else {
        value
    }
}

fn nearly_zero(x: f32) -> bool {
    x.abs() <= 1.0 / 4096.0
}

fn sect_with_horizontal(src: &[Point; 2], y: f32) -> f32 {
    let dy = src[1].y - src[0].y;
    if nearly_zero(dy) {
        (src[0].x + src[1].x) * 0.5
    } else {
        let (x0, y0, x1, y1) = (
            src[0].x as f64,
            src[0].y as f64,
            src[1].x as f64,
            src[1].y as f64,
        );
        let result = x0 + (y as f64 - y0) * (x1 - x0) / (y1 - y0);
        pin_unsorted(result, x0, x1) as f32
    }
}

fn sect_with_vertical(src: &[Point; 2], x: f32) -> f32 {
    let dx = src[1].x - src[0].x;
    if nearly_zero(dx) {
        (src[0].y + src[1].y) * 0.5
    } else {
        let (x0, y0, x1, y1) = (
            src[0].x as f64,
            src[0].y as f64,
            src[1].x as f64,
            src[1].y as f64,
        );
        (y0 + (x as f64 - x0) * (y1 - y0) / (x1 - x0)) as f32
    }
}

fn sect_clamp_with_vertical(src: &[Point; 2], x: f32) -> f32 {
    let y = sect_with_vertical(src, x);
    pin_unsorted_f(y, src[0].y, src[1].y)
}

/// `SkLineClipper::IntersectLine`: the part of the line inside `clip`
/// (edges included), or `None`.
pub fn intersect_line(src: &[Point; 2], clip: &Rect) -> Option<[Point; 2]> {
    let nested_lt = |a: f32, b: f32, dim: f32| a <= b && (a < b || dim > 0.0);
    let bounds = Rect::from_ltrb(
        src[0].x.min(src[1].x),
        src[0].y.min(src[1].y),
        src[0].x.max(src[1].x),
        src[0].y.max(src[1].y),
    );
    // containsNoEmptyCheck
    if clip.left <= bounds.left
        && clip.top <= bounds.top
        && clip.right >= bounds.right
        && clip.bottom >= bounds.bottom
    {
        return Some(*src);
    }
    if nested_lt(bounds.right, clip.left, bounds.width())
        || nested_lt(clip.right, bounds.left, bounds.width())
        || nested_lt(bounds.bottom, clip.top, bounds.height())
        || nested_lt(clip.bottom, bounds.top, bounds.height())
    {
        return None;
    }
    let (i0, i1) = if src[0].y < src[1].y { (0, 1) } else { (1, 0) };
    let mut tmp = *src;
    if tmp[i0].y < clip.top {
        tmp[i0] = Point::new(sect_with_horizontal(src, clip.top), clip.top);
    }
    if tmp[i1].y > clip.bottom {
        tmp[i1] = Point::new(sect_with_horizontal(src, clip.bottom), clip.bottom);
    }
    let (i0, i1) = if tmp[0].x < tmp[1].x { (0, 1) } else { (1, 0) };
    if tmp[i1].x <= clip.left || tmp[i0].x >= clip.right {
        // usually we will return false, but we don't if the line is vertical and coincident
        // with the clip.
        if tmp[0].x != tmp[1].x || tmp[0].x < clip.left || tmp[0].x > clip.right {
            return None;
        }
    }
    if tmp[i0].x < clip.left {
        tmp[i0] = Point::new(clip.left, sect_with_vertical(&tmp, clip.left));
    }
    if tmp[i1].x > clip.right {
        tmp[i1] = Point::new(clip.right, sect_with_vertical(&tmp, clip.right));
    }
    Some(tmp)
}

/// `SkLineClipper::ClipLine`: the polyline of the clipped line (0 to 3
/// segments).
pub fn clip_line(pts: &[Point; 2], clip: &Rect, can_cull_to_the_right: bool) -> Vec<Point> {
    let (i0, i1) = if pts[0].y < pts[1].y { (0, 1) } else { (1, 0) };
    if pts[i1].y <= clip.top || pts[i0].y >= clip.bottom {
        return Vec::new();
    }
    let mut tmp = *pts;
    if pts[i0].y < clip.top {
        tmp[i0] = Point::new(sect_with_horizontal(pts, clip.top), clip.top);
    }
    if tmp[i1].y > clip.bottom {
        tmp[i1] = Point::new(sect_with_horizontal(pts, clip.bottom), clip.bottom);
    }

    let (i0, i1, mut reverse) = if pts[0].x < pts[1].x {
        (0, 1, false)
    } else {
        (1, 0, true)
    };
    let result: Vec<Point>;
    if tmp[i1].x <= clip.left {
        tmp[0].x = clip.left;
        tmp[1].x = clip.left;
        result = tmp.to_vec();
        reverse = false;
    } else if tmp[i0].x >= clip.right {
        if can_cull_to_the_right {
            return Vec::new();
        }
        tmp[0].x = clip.right;
        tmp[1].x = clip.right;
        result = tmp.to_vec();
        reverse = false;
    } else {
        let mut r = Vec::with_capacity(4);
        if tmp[i0].x < clip.left {
            r.push(Point::new(clip.left, tmp[i0].y));
            r.push(Point::new(
                clip.left,
                sect_clamp_with_vertical(&tmp, clip.left),
            ));
        } else {
            r.push(tmp[i0]);
        }
        if tmp[i1].x > clip.right {
            r.push(Point::new(
                clip.right,
                sect_clamp_with_vertical(&tmp, clip.right),
            ));
            r.push(Point::new(clip.right, tmp[i1].y));
        } else {
            r.push(tmp[i1]);
        }
        result = r;
    }
    if reverse {
        result.into_iter().rev().collect()
    } else {
        result
    }
}

/// A clipped segment.
#[derive(Clone, Copy, Debug)]
pub enum Seg {
    Line([Point; 2]),
    Quad([Point; 3]),
    Cubic([Point; 4]),
}

fn sort_increasing_y<const N: usize>(src: &[Point; N]) -> ([Point; N], bool) {
    if src[0].y > src[N - 1].y {
        let mut d = *src;
        d.reverse();
        (d, true)
    } else {
        (*src, false)
    }
}

fn chop_mono_quad_at(c0: f32, c1: f32, c2: f32, target: f32) -> Option<f32> {
    let a = c0 - c1 - c1 + c2;
    let b = 2.0 * (c1 - c0);
    let c = c0 - target;
    let mut roots = [0.0f32; 2];
    if geom::find_unit_quad_roots(a, b, c, &mut roots) > 0 {
        Some(roots[0])
    } else {
        None
    }
}

pub struct EdgeClipper {
    pub can_cull_to_the_right: bool,
    pub out: Vec<Seg>,
}

impl EdgeClipper {
    fn append_vline(&mut self, x: f32, mut y0: f32, mut y1: f32, reverse: bool) {
        if reverse {
            std::mem::swap(&mut y0, &mut y1);
        }
        self.out
            .push(Seg::Line([Point::new(x, y0), Point::new(x, y1)]));
    }

    fn append_quad(&mut self, pts: &[Point], reverse: bool) {
        if reverse {
            self.out.push(Seg::Quad([pts[2], pts[1], pts[0]]));
        } else {
            self.out.push(Seg::Quad([pts[0], pts[1], pts[2]]));
        }
    }

    pub fn clip_line(&mut self, p0: Point, p1: Point, clip: &Rect) {
        let lines = clip_line(&[p0, p1], clip, self.can_cull_to_the_right);
        for w in lines.windows(2) {
            self.out.push(Seg::Line([w[0], w[1]]));
        }
    }

    fn clip_mono_quad(&mut self, src: &[Point; 3], clip: &Rect) {
        let (mut pts, mut reverse) = sort_increasing_y(src);
        if pts[2].y <= clip.top || pts[0].y >= clip.bottom {
            return;
        }
        // chop_quad_in_Y
        if pts[0].y < clip.top {
            if let Some(t) = chop_mono_quad_at(pts[0].y, pts[1].y, pts[2].y, clip.top) {
                let mut tmp = geom::chop_quad_at(&pts, t);
                tmp[2].y = clip.top;
                if tmp[3].y < clip.top {
                    tmp[3].y = clip.top;
                }
                pts[0] = tmp[2];
                pts[1] = tmp[3];
            } else {
                for p in &mut pts {
                    if p.y < clip.top {
                        p.y = clip.top;
                    }
                }
            }
        }
        if pts[2].y > clip.bottom {
            if let Some(t) = chop_mono_quad_at(pts[0].y, pts[1].y, pts[2].y, clip.bottom) {
                let mut tmp = geom::chop_quad_at(&pts, t);
                if tmp[1].y > clip.bottom {
                    tmp[1].y = clip.bottom;
                }
                tmp[2].y = clip.bottom;
                pts[1] = tmp[1];
                pts[2] = tmp[2];
            } else {
                for p in &mut pts {
                    if p.y > clip.bottom {
                        p.y = clip.bottom;
                    }
                }
            }
        }

        if pts[0].x > pts[2].x {
            pts.swap(0, 2);
            reverse = !reverse;
        }
        if pts[2].x <= clip.left {
            self.append_vline(clip.left, pts[0].y, pts[2].y, reverse);
            return;
        }
        if pts[0].x >= clip.right {
            if !self.can_cull_to_the_right {
                self.append_vline(clip.right, pts[0].y, pts[2].y, reverse);
            }
            return;
        }
        if pts[0].x < clip.left {
            if let Some(t) = chop_mono_quad_at(pts[0].x, pts[1].x, pts[2].x, clip.left) {
                let mut tmp = geom::chop_quad_at(&pts, t);
                self.append_vline(clip.left, tmp[0].y, tmp[2].y, reverse);
                tmp[2].x = clip.left;
                if tmp[3].x < clip.left {
                    tmp[3].x = clip.left;
                }
                pts[0] = tmp[2];
                pts[1] = tmp[3];
            } else {
                self.append_vline(clip.left, pts[0].y, pts[2].y, reverse);
                return;
            }
        }
        if pts[2].x > clip.right {
            if let Some(t) = chop_mono_quad_at(pts[0].x, pts[1].x, pts[2].x, clip.right) {
                let mut tmp = geom::chop_quad_at(&pts, t);
                if tmp[1].x > clip.right {
                    tmp[1].x = clip.right;
                }
                tmp[2].x = clip.right;
                self.append_quad(&tmp[0..3], reverse);
                self.append_vline(clip.right, tmp[2].y, tmp[4].y, reverse);
            } else {
                pts[1].x = pts[1].x.min(clip.right);
                pts[2].x = pts[2].x.min(clip.right);
                self.append_quad(&pts, reverse);
            }
        } else {
            self.append_quad(&pts, reverse);
        }
    }

    pub fn clip_quad(&mut self, src: &[Point; 3], clip: &Rect) {
        let (b, _) = Rect::bounds_of(src);
        if b.top >= clip.bottom || b.bottom <= clip.top {
            return;
        }
        let mut mono_y = [Point::default(); 5];
        let count_y = geom::chop_quad_at_y_extrema(src, &mut mono_y);
        for y in 0..=count_y {
            let q = [mono_y[y * 2], mono_y[y * 2 + 1], mono_y[y * 2 + 2]];
            let mut mono_x = [Point::default(); 5];
            let count_x = geom::chop_quad_at_x_extrema(&q, &mut mono_x);
            for x in 0..=count_x {
                self.clip_mono_quad(&[mono_x[x * 2], mono_x[x * 2 + 1], mono_x[x * 2 + 2]], clip);
            }
        }
    }

    fn append_cubic(&mut self, pts: &[Point], reverse: bool) {
        if reverse {
            self.out.push(Seg::Cubic([pts[3], pts[2], pts[1], pts[0]]));
        } else {
            self.out.push(Seg::Cubic([pts[0], pts[1], pts[2], pts[3]]));
        }
    }

    /// `SkEdgeClipper::clipMonoCubic` (`src` monotonic in X and Y).
    fn clip_mono_cubic(&mut self, src: &[Point; 4], clip: &Rect) {
        let (mut pts, mut reverse) = sort_increasing_y(src);
        if pts[3].y <= clip.top || pts[0].y >= clip.bottom {
            return;
        }
        chop_cubic_in_y(&mut pts, clip);
        if pts[0].x > pts[3].x {
            pts.swap(0, 3);
            pts.swap(1, 2);
            reverse = !reverse;
        }
        if pts[3].x <= clip.left {
            self.append_vline(clip.left, pts[0].y, pts[3].y, reverse);
            return;
        }
        if pts[0].x >= clip.right {
            if !self.can_cull_to_the_right {
                self.append_vline(clip.right, pts[0].y, pts[3].y, reverse);
            }
            return;
        }
        if pts[0].x < clip.left {
            let mut tmp = [Point::default(); 7];
            chop_mono_cubic_at_axis(&pts, false, clip.left, &mut tmp);
            self.append_vline(clip.left, tmp[0].y, tmp[3].y, reverse);
            tmp[3].x = clip.left;
            if tmp[4].x < clip.left {
                tmp[4].x = clip.left;
            }
            pts[0] = tmp[3];
            pts[1] = tmp[4];
            pts[2] = tmp[5];
        }
        if pts[3].x > clip.right {
            let mut tmp = [Point::default(); 7];
            chop_mono_cubic_at_axis(&pts, false, clip.right, &mut tmp);
            tmp[3].x = clip.right;
            if tmp[2].x > clip.right {
                tmp[2].x = clip.right;
            }
            self.append_cubic(&tmp[0..4], reverse);
            self.append_vline(clip.right, tmp[3].y, tmp[6].y, reverse);
        } else {
            self.append_cubic(&pts, reverse);
        }
    }

    /// `SkEdgeClipper::clipCubic`.
    pub fn clip_cubic(&mut self, src: &[Point; 4], clip: &Rect) {
        let (b, _) = Rect::bounds_of(src);
        if b.bottom > clip.top && b.top < clip.bottom {
            // too_big_for_reliable_float_math: draw a line, which clips safely.
            let limit = (1 << 22) as f32;
            if b.left < -limit || b.top < -limit || b.right > limit || b.bottom > limit {
                self.clip_line(src[0], src[3], clip);
                return;
            }
            let mut mono_y = [Point::default(); 10];
            let count_y = geom::chop_cubic_at_y_extrema(src, &mut mono_y);
            for y in 0..=count_y {
                let c = [
                    mono_y[y * 3],
                    mono_y[y * 3 + 1],
                    mono_y[y * 3 + 2],
                    mono_y[y * 3 + 3],
                ];
                let mut mono_x = [Point::default(); 10];
                let count_x = geom::chop_cubic_at_x_extrema(&c, &mut mono_x);
                for x in 0..=count_x {
                    self.clip_mono_cubic(
                        &[
                            mono_x[x * 3],
                            mono_x[x * 3 + 1],
                            mono_x[x * 3 + 2],
                            mono_x[x * 3 + 3],
                        ],
                        clip,
                    );
                }
            }
        }
    }
}

/// `mono_cubic_closestT`.
fn mono_cubic_closest_t(src: [f32; 4], mut x: f32) -> f32 {
    let mut t = 0.5f32;
    let mut best_t = 0.0f32;
    let mut step = 0.25f32;
    let d = src[0];
    let a = src[3] + 3.0 * (src[1] - src[2]) - d;
    let b = 3.0 * (src[2] - src[1] - src[1] + d);
    let c = 3.0 * (src[1] - d);
    x -= d;
    let mut closest = f32::MAX;
    loop {
        let loc = ((a * t + b) * t + c) * t;
        let dist = (loc - x).abs();
        if closest > dist {
            closest = dist;
            best_t = t;
        }
        let last_t = t;
        t += if loc < x { step } else { -step };
        step *= 0.5;
        if !(closest > 0.25 && last_t != t) {
            break;
        }
    }
    best_t
}

/// `chop_mono_cubic_at_y` / `chop_mono_cubic_at_x`.
fn chop_mono_cubic_at_axis(src: &[Point; 4], y: bool, v: f32, dst: &mut [Point; 7]) {
    if crate::cubics::chop_mono_cubic_at(src, y, v, dst) {
        return;
    }
    let coords = if y {
        [src[0].y, src[1].y, src[2].y, src[3].y]
    } else {
        [src[0].x, src[1].x, src[2].x, src[3].x]
    };
    geom::chop_cubic_at(src, dst, mono_cubic_closest_t(coords, v));
}

/// `chop_cubic_in_Y`: clips `pts` to the clip's top and bottom.
fn chop_cubic_in_y(pts: &mut [Point; 4], clip: &Rect) {
    if pts[0].y < clip.top {
        let mut tmp = [Point::default(); 7];
        chop_mono_cubic_at_axis(pts, true, clip.top, &mut tmp);
        // A poor chop can leave the lower cubic partly above: chop it again.
        if tmp[3].y < clip.top && tmp[4].y < clip.top && tmp[5].y < clip.top {
            let tmp2 = [tmp[3], tmp[4], tmp[5], tmp[6]];
            chop_mono_cubic_at_axis(&tmp2, true, clip.top, &mut tmp);
        }
        tmp[3].y = clip.top;
        if tmp[4].y < clip.top {
            tmp[4].y = clip.top;
        }
        pts[0] = tmp[3];
        pts[1] = tmp[4];
        pts[2] = tmp[5];
    }
    if pts[3].y > clip.bottom {
        let mut tmp = [Point::default(); 7];
        chop_mono_cubic_at_axis(pts, true, clip.bottom, &mut tmp);
        tmp[3].y = clip.bottom;
        if tmp[2].y > clip.bottom {
            tmp[2].y = clip.bottom;
        }
        pts[1] = tmp[1];
        pts[2] = tmp[2];
        pts[3] = tmp[3];
    }
}
