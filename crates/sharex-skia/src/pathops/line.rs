//! `SkPathOpsLine.h`, `SkPathOpsLine.cpp`: `SkDLine`.

use super::point::{DPoint, DVector};
use super::types::*;
use crate::geom::Point;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DLine {
    pub pts: [DPoint; 2],
}

impl std::ops::Index<usize> for DLine {
    type Output = DPoint;
    fn index(&self, n: usize) -> &DPoint {
        &self.pts[n]
    }
}

impl std::ops::IndexMut<usize> for DLine {
    fn index_mut(&mut self, n: usize) -> &mut DPoint {
        &mut self.pts[n]
    }
}

impl DLine {
    pub fn new(a: DPoint, b: DPoint) -> Self {
        DLine { pts: [a, b] }
    }

    pub fn from_pts(pts: &[Point]) -> Self {
        DLine { pts: [DPoint::from_pt(pts[0]), DPoint::from_pt(pts[1])] }
    }

    pub fn set(&mut self, pts: &[Point]) -> &Self {
        self.pts[0].set(pts[0]);
        self.pts[1].set(pts[1]);
        self
    }

    pub fn pt_at_t(&self, t: f64) -> DPoint {
        if 0.0 == t {
            return self.pts[0];
        }
        if 1.0 == t {
            return self.pts[1];
        }
        let one_t = 1.0 - t;
        DPoint {
            x: one_t * self.pts[0].x + t * self.pts[1].x,
            y: one_t * self.pts[0].y + t * self.pts[1].y,
        }
    }

    pub fn exact_point(&self, xy: &DPoint) -> f64 {
        if *xy == self.pts[0] {
            return 0.0;
        }
        if *xy == self.pts[1] {
            return 1.0;
        }
        -1.0
    }

    pub fn near_point(&self, xy: &DPoint, unequal: Option<&mut bool>) -> f64 {
        if !almost_between_ulps_d(self.pts[0].x, xy.x, self.pts[1].x)
            || !almost_between_ulps_d(self.pts[0].y, xy.y, self.pts[1].y)
        {
            return -1.0;
        }
        // project a perpendicular ray from the point to the line; find the T on the line
        let len: DVector = self.pts[1] - self.pts[0];
        let denom = len.x * len.x + len.y * len.y;
        let ab0 = *xy - self.pts[0];
        let numer = len.x * ab0.x + ab0.y * len.y;
        if !between(0.0, numer, denom) {
            return -1.0;
        }
        if denom == 0.0 {
            return 0.0;
        }
        let mut t = numer / denom;
        let real_pt = self.pt_at_t(t);
        let dist = real_pt.distance(xy);
        // find the ordinal in the original line with the largest unsigned exponent
        let tiniest = smin(smin(smin(self.pts[0].x, self.pts[0].y), self.pts[1].x), self.pts[1].y);
        let mut largest = smax(smax(smax(self.pts[0].x, self.pts[0].y), self.pts[1].x), self.pts[1].y);
        largest = smax(largest, -tiniest);
        if !almost_equal_ulps_pin_d(largest, largest + dist) {
            return -1.0;
        }
        if let Some(unequal) = unequal {
            *unequal = largest as f32 != (largest + dist) as f32;
        }
        t = sk_pin_t(t);
        t
    }

    pub fn near_ray(&self, xy: &DPoint) -> bool {
        let len = self.pts[1] - self.pts[0];
        let denom = len.x * len.x + len.y * len.y;
        let ab0 = *xy - self.pts[0];
        let numer = len.x * ab0.x + ab0.y * len.y;
        let t = numer / denom;
        let real_pt = self.pt_at_t(t);
        let dist = real_pt.distance(xy);
        let tiniest = smin(smin(smin(self.pts[0].x, self.pts[0].y), self.pts[1].x), self.pts[1].y);
        let mut largest = smax(smax(smax(self.pts[0].x, self.pts[0].y), self.pts[1].x), self.pts[1].y);
        largest = smax(largest, -tiniest);
        roughly_equal_ulps_d(largest, largest + dist)
    }

    #[allow(non_snake_case)]
    pub fn ExactPointH(xy: &DPoint, left: f64, right: f64, y: f64) -> f64 {
        if xy.y == y {
            if xy.x == left {
                return 0.0;
            }
            if xy.x == right {
                return 1.0;
            }
        }
        -1.0
    }

    #[allow(non_snake_case)]
    pub fn NearPointH(xy: &DPoint, left: f64, right: f64, y: f64) -> f64 {
        if !almost_bequal_ulps_d(xy.y, y) {
            return -1.0;
        }
        if !almost_between_ulps_d(left, xy.x, right) {
            return -1.0;
        }
        let mut t = (xy.x - left) / (right - left);
        t = sk_pin_t(t);
        let real_pt_x = (1.0 - t) * left + t * right;
        let dist_u = DVector { x: xy.y - y, y: xy.x - real_pt_x };
        let dist_sq = dist_u.x * dist_u.x + dist_u.y * dist_u.y;
        let dist = dist_sq.sqrt();
        let tiniest = smin(smin(y, left), right);
        let mut largest = smax(smax(y, left), right);
        largest = smax(largest, -tiniest);
        if !almost_equal_ulps_d(largest, largest + dist) {
            return -1.0;
        }
        t
    }

    #[allow(non_snake_case)]
    pub fn ExactPointV(xy: &DPoint, top: f64, bottom: f64, x: f64) -> f64 {
        if xy.x == x {
            if xy.y == top {
                return 0.0;
            }
            if xy.y == bottom {
                return 1.0;
            }
        }
        -1.0
    }

    #[allow(non_snake_case)]
    pub fn NearPointV(xy: &DPoint, top: f64, bottom: f64, x: f64) -> f64 {
        if !almost_bequal_ulps_d(xy.x, x) {
            return -1.0;
        }
        if !almost_between_ulps_d(top, xy.y, bottom) {
            return -1.0;
        }
        let mut t = (xy.y - top) / (bottom - top);
        t = sk_pin_t(t);
        let real_pt_y = (1.0 - t) * top + t * bottom;
        let dist_u = DVector { x: xy.x - x, y: xy.y - real_pt_y };
        let dist_sq = dist_u.x * dist_u.x + dist_u.y * dist_u.y;
        let dist = dist_sq.sqrt();
        let tiniest = smin(smin(x, top), bottom);
        let mut largest = smax(smax(x, top), bottom);
        largest = smax(largest, -tiniest);
        if !almost_equal_ulps_d(largest, largest + dist) {
            return -1.0;
        }
        t
    }
}
