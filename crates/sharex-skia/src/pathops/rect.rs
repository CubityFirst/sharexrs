//! `SkPathOpsRect.h/.cpp` (`SkDRect`) and `SkPathOpsBounds.h`
//! (`SkPathOpsBounds`).

use super::conic::DConic;
use super::cubic::DCubic;
use super::point::DPoint;
use super::quad::DQuad;
use super::types::*;
use crate::geom::Point;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DRect {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

impl DRect {
    pub fn add(&mut self, pt: &DPoint) {
        self.left = smin(self.left, pt.x);
        self.top = smin(self.top, pt.y);
        self.right = smax(self.right, pt.x);
        self.bottom = smax(self.bottom, pt.y);
    }

    pub fn contains(&self, pt: &DPoint) -> bool {
        approximately_between(self.left, pt.x, self.right) && approximately_between(self.top, pt.y, self.bottom)
    }

    pub fn intersects(&self, r: &DRect) -> bool {
        r.left <= self.right && self.left <= r.right && r.top <= self.bottom && self.top <= r.bottom
    }

    pub fn set(&mut self, pt: &DPoint) {
        self.left = pt.x;
        self.right = pt.x;
        self.top = pt.y;
        self.bottom = pt.y;
    }

    pub fn width(&self) -> f64 {
        self.right - self.left
    }

    pub fn height(&self) -> f64 {
        self.bottom - self.top
    }

    pub fn valid(&self) -> bool {
        self.left <= self.right && self.top <= self.bottom
    }

    pub fn set_bounds_quad(&mut self, curve: &DQuad, sub: &DQuad, start_t: f64, end_t: f64) {
        self.set(&sub[0]);
        self.add(&sub[2]);
        let mut t_values = [0.0; 2];
        let mut roots = 0usize;
        if !sub.monotonic_in_x() {
            roots = DQuad::FindExtrema(sub.xs(), &mut t_values) as usize;
        }
        if !sub.monotonic_in_y() {
            roots += DQuad::FindExtrema(sub.ys(), &mut t_values[roots..]) as usize;
        }
        for index in 0..roots {
            let t = start_t + (end_t - start_t) * t_values[index];
            self.add(&curve.pt_at_t(t));
        }
    }

    pub fn set_bounds_conic(&mut self, curve: &DConic, sub: &DConic, start_t: f64, end_t: f64) {
        self.set(&sub[0]);
        self.add(&sub[2]);
        let mut t_values = [0.0; 2];
        let mut roots = 0usize;
        if !sub.monotonic_in_x() {
            roots = DConic::FindExtrema(sub.pts.xs(), sub.weight, &mut t_values) as usize;
        }
        if !sub.monotonic_in_y() {
            roots += DConic::FindExtrema(sub.pts.ys(), sub.weight, &mut t_values[roots..]) as usize;
        }
        for index in 0..roots {
            let t = start_t + (end_t - start_t) * t_values[index];
            self.add(&curve.pt_at_t(t));
        }
    }

    pub fn set_bounds_cubic(&mut self, curve: &DCubic, sub: &DCubic, start_t: f64, end_t: f64) {
        self.set(&sub[0]);
        self.add(&sub[3]);
        let mut t_values = [0.0; 4];
        let mut roots = 0usize;
        if !sub.monotonic_in_x() {
            roots = DCubic::FindExtrema(sub.xs(), &mut t_values) as usize;
        }
        if !sub.monotonic_in_y() {
            roots += DCubic::FindExtrema(sub.ys(), &mut t_values[roots..]) as usize;
        }
        for index in 0..roots {
            let t = start_t + (end_t - start_t) * t_values[index];
            self.add(&curve.pt_at_t(t));
        }
    }
}

/// `SkPathOpsBounds`: unlike `SkRect`, a line isn't empty.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PathOpsBounds {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl PathOpsBounds {
    #[allow(non_snake_case)]
    pub fn Intersects(a: &PathOpsBounds, b: &PathOpsBounds) -> bool {
        almost_less_or_equal_ulps(a.left, b.right)
            && almost_less_or_equal_ulps(b.left, a.right)
            && almost_less_or_equal_ulps(a.top, b.bottom)
            && almost_less_or_equal_ulps(b.top, a.bottom)
    }

    pub fn set_ltrb(&mut self, l: f32, t: f32, r: f32, b: f32) {
        self.left = l;
        self.top = t;
        self.right = r;
        self.bottom = b;
    }

    pub fn add_ltrb(&mut self, left: f32, top: f32, right: f32, bottom: f32) {
        if left < self.left {
            self.left = left;
        }
        if top < self.top {
            self.top = top;
        }
        if right > self.right {
            self.right = right;
        }
        if bottom > self.bottom {
            self.bottom = bottom;
        }
    }

    pub fn add(&mut self, to_add: &PathOpsBounds) {
        self.add_ltrb(to_add.left, to_add.top, to_add.right, to_add.bottom);
    }

    pub fn add_pt(&mut self, pt: Point) {
        if pt.x < self.left {
            self.left = pt.x;
        }
        if pt.y < self.top {
            self.top = pt.y;
        }
        if pt.x > self.right {
            self.right = pt.x;
        }
        if pt.y > self.bottom {
            self.bottom = pt.y;
        }
    }

    pub fn add_dpt(&mut self, pt: &DPoint) {
        if pt.x < self.left as f64 {
            self.left = pt.x as f32;
        }
        if pt.y < self.top as f64 {
            self.top = pt.y as f32;
        }
        if pt.x > self.right as f64 {
            self.right = pt.x as f32;
        }
        if pt.y > self.bottom as f64 {
            self.bottom = pt.y as f32;
        }
    }

    pub fn almost_contains(&self, pt: Point) -> bool {
        almost_less_or_equal_ulps(self.left, pt.x)
            && almost_less_or_equal_ulps(pt.x, self.right)
            && almost_less_or_equal_ulps(self.top, pt.y)
            && almost_less_or_equal_ulps(pt.y, self.bottom)
    }

    pub fn contains(&self, pt: Point) -> bool {
        self.left <= pt.x && self.top <= pt.y && self.right >= pt.x && self.bottom >= pt.y
    }

    /// `SkRect::setBounds(pts, count)` for finite points.
    pub fn set_bounds(&mut self, pts: &[Point]) {
        let mut l = pts[0].x;
        let mut t = pts[0].y;
        let mut r = l;
        let mut b = t;
        for p in &pts[1..] {
            // skvx::min/max: (b < a) ? b : a, (a < b) ? b : a
            l = sminf(l, p.x);
            t = sminf(t, p.y);
            r = smaxf(r, p.x);
            b = smaxf(b, p.y);
        }
        self.set_ltrb(l, t, r, b);
    }

    pub fn width(&self) -> f32 {
        self.right - self.left
    }

    pub fn height(&self) -> f32 {
        self.bottom - self.top
    }
}
