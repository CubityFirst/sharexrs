//! `SkPathOpsPoint.h`: `SkDPoint`, `SkDVector`.

use super::types::*;
use crate::geom::Point;

/// `AlmostEqualUlps(const SkPoint&, const SkPoint&)`.
pub fn almost_equal_ulps_pt(pt1: Point, pt2: Point) -> bool {
    almost_equal_ulps(pt1.x, pt2.x) && almost_equal_ulps(pt1.y, pt2.y)
}

/// `SkDPoint` (also `SkDVector`: the same two doubles).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DPoint {
    pub x: f64,
    pub y: f64,
}

pub type DVector = DPoint;

impl std::ops::Sub for DPoint {
    type Output = DVector;
    fn sub(self, b: DPoint) -> DVector {
        DPoint { x: self.x - b.x, y: self.y - b.y }
    }
}

impl std::ops::Add for DPoint {
    type Output = DPoint;
    fn add(self, v: DVector) -> DPoint {
        DPoint { x: self.x + v.x, y: self.y + v.y }
    }
}

impl DPoint {
    pub const fn new(x: f64, y: f64) -> Self {
        DPoint { x, y }
    }

    pub fn from_pt(pt: Point) -> Self {
        DPoint { x: pt.x as f64, y: pt.y as f64 }
    }

    pub fn set(&mut self, pt: Point) {
        self.x = pt.x as f64;
        self.y = pt.y as f64;
    }

    // SkDVector

    pub fn as_sk_vector(self) -> Point {
        Point::new(self.x as f32, self.y as f32)
    }

    pub fn cross(self, a: DVector) -> f64 {
        self.x * a.y - self.y * a.x
    }

    /// Similar to cross, this considers nearly coincident to be zero (ulps epsilon == 16).
    pub fn cross_check(self, a: DVector) -> f64 {
        let xy = self.x * a.y;
        let yx = self.y * a.x;
        if almost_equal_ulps_d(xy, yx) { 0.0 } else { xy - yx }
    }

    /// Allow tinier numbers.
    pub fn cross_no_normal_check(self, a: DVector) -> f64 {
        let xy = self.x * a.y;
        let yx = self.y * a.x;
        if almost_equal_ulps_no_normal_check_d(xy, yx) { 0.0 } else { xy - yx }
    }

    pub fn dot(self, a: DVector) -> f64 {
        self.x * a.x + self.y * a.y
    }

    pub fn length(self) -> f64 {
        self.length_squared().sqrt()
    }

    pub fn length_squared(self) -> f64 {
        self.x * self.x + self.y * self.y
    }

    pub fn normalize(&mut self) -> &mut Self {
        let inverse_length = 1.0 / self.length();
        self.x *= inverse_length;
        self.y *= inverse_length;
        self
    }

    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }

    // SkDPoint

    /// Note: this can not be implemented with approximately_equal on each
    /// coordinate, because that would not take the magnitude into account.
    pub fn approximately_d_equal(&self, a: &DPoint) -> bool {
        if approximately_equal(self.x, a.x) && approximately_equal(self.y, a.y) {
            return true;
        }
        if !roughly_equal_ulps_d(self.x, a.x) || !roughly_equal_ulps_d(self.y, a.y) {
            return false;
        }
        let dist = self.distance(a);
        let tiniest = smin(smin(smin(self.x, a.x), self.y), a.y);
        let mut largest = smax(smax(smax(self.x, a.x), self.y), a.y);
        largest = smax(largest, -tiniest);
        almost_dequal_ulps_d(largest, largest + dist)
    }

    pub fn approximately_d_equal_pt(&self, a: Point) -> bool {
        self.approximately_d_equal(&DPoint::from_pt(a))
    }

    pub fn approximately_equal(&self, a: &DPoint) -> bool {
        if approximately_equal(self.x, a.x) && approximately_equal(self.y, a.y) {
            return true;
        }
        if !roughly_equal_ulps_d(self.x, a.x) || !roughly_equal_ulps_d(self.y, a.y) {
            return false;
        }
        let dist = self.distance(a);
        let tiniest = smin(smin(smin(self.x, a.x), self.y), a.y);
        let mut largest = smax(smax(smax(self.x, a.x), self.y), a.y);
        largest = smax(largest, -tiniest);
        almost_pequal_ulps_d(largest, largest + dist)
    }

    pub fn approximately_equal_pt(&self, a: Point) -> bool {
        self.approximately_equal(&DPoint::from_pt(a))
    }

    /// `SkDPoint::ApproximatelyEqual(const SkPoint&, const SkPoint&)`.
    pub fn approximately_equal_sk(a: Point, b: Point) -> bool {
        if approximately_equal(a.x as f64, b.x as f64) && approximately_equal(a.y as f64, b.y as f64) {
            return true;
        }
        if !roughly_equal_ulps(a.x, b.x) || !roughly_equal_ulps(a.y, b.y) {
            return false;
        }
        let da = DPoint::from_pt(a);
        let db = DPoint::from_pt(b);
        let dist = da.distance(&db);
        let tiniest = sminf(sminf(sminf(a.x, b.x), a.y), b.y);
        let mut largest = smaxf(smaxf(smaxf(a.x, b.x), a.y), b.y);
        largest = smaxf(largest, -tiniest);
        almost_dequal_ulps_d(largest as f64, largest as f64 + dist)
    }

    pub fn approximately_zero(&self) -> bool {
        approximately_zero(self.x) && approximately_zero(self.y)
    }

    pub fn as_sk_point(&self) -> Point {
        Point::new(self.x as f32, self.y as f32)
    }

    pub fn distance(&self, a: &DPoint) -> f64 {
        (*self - *a).length()
    }

    pub fn distance_squared(&self, a: &DPoint) -> f64 {
        (*self - *a).length_squared()
    }

    #[allow(non_snake_case)]
    pub fn Mid(a: &DPoint, b: &DPoint) -> DPoint {
        DPoint { x: (a.x + b.x) / 2.0, y: (a.y + b.y) / 2.0 }
    }

    pub fn roughly_equal(&self, a: &DPoint) -> bool {
        if roughly_equal(self.x, a.x) && roughly_equal(self.y, a.y) {
            return true;
        }
        let dist = self.distance(a);
        let tiniest = smin(smin(smin(self.x, a.x), self.y), a.y);
        let mut largest = smax(smax(smax(self.x, a.x), self.y), a.y);
        largest = smax(largest, -tiniest);
        roughly_equal_ulps_d(largest, largest + dist)
    }

    /// `SkDPoint::RoughlyEqual(const SkPoint&, const SkPoint&)`.
    pub fn roughly_equal_sk(a: Point, b: Point) -> bool {
        if !roughly_equal_ulps(a.x, b.x) && !roughly_equal_ulps(a.y, b.y) {
            return false;
        }
        let da = DPoint::from_pt(a);
        let db = DPoint::from_pt(b);
        let dist = da.distance(&db);
        let tiniest = sminf(sminf(sminf(a.x, b.x), a.y), b.y);
        let mut largest = smaxf(smaxf(smaxf(a.x, b.x), a.y), b.y);
        largest = smaxf(largest, -tiniest);
        roughly_equal_ulps_d(largest as f64, largest as f64 + dist)
    }

    /// Very light weight check, should only be used for inequality check.
    pub fn way_roughly_equal(a: Point, b: Point) -> bool {
        let largest_number = smaxf(a.x.abs(), smaxf(a.y.abs(), smaxf(b.x.abs(), b.y.abs())));
        let diffs = Point::new(a.x - b.x, a.y - b.y);
        let largest_diff = smaxf(diffs.x, diffs.y);
        roughly_zero_when_compared_to(largest_diff as f64, largest_number as f64)
    }
}
