//! `SkPathOpsCurve.h`, `SkPathOpsCurve.cpp`: `SkDCurve`, `SkOpCurve`,
//! `SkDCurveSweep` and the per-verb function tables (`CurveDPointAtT`,
//! `CurveSlopeAtT`, `CurveIntersectRay`, ...).

use super::conic::DConic;
use super::cubic::DCubic;
use super::intersections::Intersections;
use super::line::DLine;
use super::point::{DPoint, DVector};
use super::quad::DQuad;
use super::rect::{DRect, PathOpsBounds};
use super::types::*;
use crate::geom::Point;
use crate::path::Verb;

/// `SkPathOpsVerbToPoints`.
#[inline]
pub fn verb_to_points(verb: Verb) -> usize {
    match verb {
        Verb::Line => 1,
        Verb::Quad | Verb::Conic => 2,
        Verb::Cubic => 3,
        _ => 0,
    }
}

/// `SkPathOpsPointsToVerb`.
#[inline]
pub fn points_to_verb(points: usize) -> Verb {
    match points {
        0 => Verb::Move,
        1 => Verb::Line,
        2 => Verb::Quad,
        _ => Verb::Cubic,
    }
}

/// `SkOpCurve`: float points and a weight.
#[derive(Clone, Copy, Debug, Default)]
pub struct OpCurve {
    pub pts: [Point; 4],
    pub weight: f32,
}

impl OpCurve {
    pub fn set_quad(&mut self, quad: &DQuad) {
        for index in 0..3 {
            self.pts[index] = quad[index].as_sk_point();
        }
    }

    pub fn set_cubic(&mut self, cubic: &DCubic) {
        for index in 0..4 {
            self.pts[index] = cubic[index].as_sk_point();
        }
    }
}

/// `SkDCurve`: a union of line, quad, conic and cubic sharing their first
/// points; the conic's weight is kept apart.
#[derive(Clone, Copy, Debug, Default)]
pub struct DCurve {
    pub pts: [DPoint; 4],
    pub weight: f32,
}

impl std::ops::Index<usize> for DCurve {
    type Output = DPoint;
    fn index(&self, n: usize) -> &DPoint {
        &self.pts[n]
    }
}

impl std::ops::IndexMut<usize> for DCurve {
    fn index_mut(&mut self, n: usize) -> &mut DPoint {
        &mut self.pts[n]
    }
}

impl DCurve {
    pub fn line(&self) -> DLine {
        DLine { pts: [self.pts[0], self.pts[1]] }
    }
    pub fn quad(&self) -> DQuad {
        DQuad { pts: [self.pts[0], self.pts[1], self.pts[2]] }
    }
    pub fn conic(&self) -> DConic {
        DConic { pts: self.quad(), weight: self.weight }
    }
    pub fn cubic(&self) -> DCubic {
        DCubic { pts: self.pts }
    }
    pub fn set_line(&mut self, l: &DLine) {
        self.pts[0] = l.pts[0];
        self.pts[1] = l.pts[1];
    }
    pub fn set_quad(&mut self, q: &DQuad) {
        self.pts[..3].copy_from_slice(&q.pts);
    }
    pub fn set_conic(&mut self, c: &DConic) {
        self.pts[..3].copy_from_slice(&c.pts.pts);
        self.weight = c.weight;
    }
    pub fn set_cubic(&mut self, c: &DCubic) {
        self.pts = c.pts;
    }

    /// This cheats and assumes that the perpendicular to the point is the
    /// closest ray to the curve.
    pub fn near_point(&self, verb: Verb, xy: &DPoint, opp: &DPoint) -> f64 {
        let count = verb_to_points(verb);
        let mut min_x = self.pts[0].x;
        let mut max_x = min_x;
        for index in 1..=count {
            min_x = smin(min_x, self.pts[index].x);
            max_x = smax(max_x, self.pts[index].x);
        }
        if !almost_between_ulps_d(min_x, xy.x, max_x) {
            return -1.0;
        }
        let mut min_y = self.pts[0].y;
        let mut max_y = min_y;
        for index in 1..=count {
            min_y = smin(min_y, self.pts[index].y);
            max_y = smax(max_y, self.pts[index].y);
        }
        if !almost_between_ulps_d(min_y, xy.y, max_y) {
            return -1.0;
        }
        let mut i = Intersections::default();
        let perp = DLine::new(*xy, DPoint::new(xy.x + opp.y - xy.y, xy.y + xy.x - opp.x));
        curve_d_intersect_ray(verb, self, &perp, &mut i);
        let mut min_index: i32 = -1;
        let mut min_dist = f32::MAX as f64;
        for index in 0..i.used() as usize {
            let dist = xy.distance(&i.pt(index));
            if min_dist > dist {
                min_dist = dist;
                min_index = index as i32;
            }
        }
        if min_index < 0 {
            return -1.0;
        }
        let largest = smax(smax(max_x, max_y), -smin(min_x, min_y));
        if !almost_equal_ulps_pin_d(largest, largest + min_dist) {
            return -1.0;
        }
        sk_pin_t(i.t(0, min_index as usize))
    }

    pub fn set_conic_bounds(&self, curve: &[Point], curve_weight: f32, t_start: f64, t_end: f64, bounds: &mut PathOpsBounds) {
        let d_curve = DConic::from_pts(curve, curve_weight);
        let mut d_rect = DRect::default();
        d_rect.set_bounds_conic(&d_curve, &self.conic(), t_start, t_end);
        bounds.set_ltrb(d_rect.left as f32, d_rect.top as f32, d_rect.right as f32, d_rect.bottom as f32);
    }

    pub fn set_cubic_bounds(&self, curve: &[Point], _w: f32, t_start: f64, t_end: f64, bounds: &mut PathOpsBounds) {
        let d_curve = DCubic::from_pts(curve);
        let mut d_rect = DRect::default();
        d_rect.set_bounds_cubic(&d_curve, &self.cubic(), t_start, t_end);
        bounds.set_ltrb(d_rect.left as f32, d_rect.top as f32, d_rect.right as f32, d_rect.bottom as f32);
    }

    pub fn set_quad_bounds(&self, curve: &[Point], _w: f32, t_start: f64, t_end: f64, bounds: &mut PathOpsBounds) {
        let d_curve = DQuad::from_pts(curve);
        let mut d_rect = DRect::default();
        d_rect.set_bounds_quad(&d_curve, &self.quad(), t_start, t_end);
        bounds.set_ltrb(d_rect.left as f32, d_rect.top as f32, d_rect.right as f32, d_rect.bottom as f32);
    }
}

/// `SkDCurveSweep`.
#[derive(Clone, Copy, Debug, Default)]
pub struct DCurveSweep {
    pub curve: DCurve,
    pub sweep: [DVector; 2],
    is_curve: bool,
    ordered: bool,
}

impl DCurveSweep {
    pub fn is_curve(&self) -> bool {
        self.is_curve
    }

    pub fn is_ordered(&self) -> bool {
        self.ordered
    }

    pub fn set_curve_hull_sweep(&mut self, verb: Verb) {
        self.ordered = true;
        self.sweep[0] = self.curve[1] - self.curve[0];
        if Verb::Line == verb {
            self.sweep[1] = self.sweep[0];
            self.is_curve = false;
            return;
        }
        self.sweep[1] = self.curve[2] - self.curve[0];
        // OPTIMIZE: I do the following float check a lot -- probably need a
        // central place for this val-is-small-compared-to-curve check
        let mut max_val = 0.0;
        for index in 0..=verb_to_points(verb) {
            max_val = smax(max_val, smax(self.curve[index].x.abs(), self.curve[index].y.abs()));
        }
        'set_is_curve: {
            if Verb::Cubic != verb {
                if roughly_zero_when_compared_to(self.sweep[0].x, max_val)
                    && roughly_zero_when_compared_to(self.sweep[0].y, max_val)
                {
                    self.sweep[0] = self.sweep[1];
                }
                break 'set_is_curve;
            }
            let third_sweep = self.curve[3] - self.curve[0];
            if self.sweep[0].x == 0.0 && self.sweep[0].y == 0.0 {
                self.sweep[0] = self.sweep[1];
                self.sweep[1] = third_sweep;
                if roughly_zero_when_compared_to(self.sweep[0].x, max_val)
                    && roughly_zero_when_compared_to(self.sweep[0].y, max_val)
                {
                    self.sweep[0] = self.sweep[1];
                    self.curve[1] = self.curve[3];
                }
                break 'set_is_curve;
            }
            let s1x3 = self.sweep[0].cross_check(third_sweep);
            let s3x2 = third_sweep.cross_check(self.sweep[1]);
            if s1x3 * s3x2 >= 0.0 {
                // if third vector is on or between first two vectors
                break 'set_is_curve;
            }
            let s2x1 = self.sweep[1].cross_check(self.sweep[0]);
            // FIXME: If the sweep of the cubic is greater than 180 degrees, we're in trouble
            // probably such wide sweeps should be artificially subdivided earlier so that never happens
            if s3x2 * s2x1 < 0.0 {
                self.sweep[0] = self.sweep[1];
                self.ordered = false;
            }
            self.sweep[1] = third_sweep;
        }
        self.is_curve = self.sweep[0].cross_check(self.sweep[1]) != 0.0;
    }
}

/// `CurveDPointAtT[verb]`.
pub fn curve_d_point_at_t(verb: Verb, a: &[Point], weight: f32, t: f64) -> DPoint {
    match verb {
        Verb::Line => DLine::from_pts(a).pt_at_t(t),
        Verb::Quad => DQuad::from_pts(a).pt_at_t(t),
        Verb::Conic => DConic::from_pts(a, weight).pt_at_t(t),
        Verb::Cubic => DCubic::from_pts(a).pt_at_t(t),
        _ => unreachable!(),
    }
}

/// `CurveDDPointAtT[verb]`.
pub fn curve_dd_point_at_t(verb: Verb, c: &DCurve, t: f64) -> DPoint {
    match verb {
        Verb::Line => c.line().pt_at_t(t),
        Verb::Quad => c.quad().pt_at_t(t),
        Verb::Conic => c.conic().pt_at_t(t),
        Verb::Cubic => c.cubic().pt_at_t(t),
        _ => unreachable!(),
    }
}

/// `CurvePointAtT[verb]`.
pub fn curve_point_at_t(verb: Verb, a: &[Point], weight: f32, t: f64) -> Point {
    curve_d_point_at_t(verb, a, weight, t).as_sk_point()
}

/// `CurveDSlopeAtT[verb]`.
pub fn curve_d_slope_at_t(verb: Verb, a: &[Point], weight: f32, t: f64) -> DVector {
    match verb {
        Verb::Line => {
            let line = DLine::from_pts(a);
            line[1] - line[0]
        }
        Verb::Quad => DQuad::from_pts(a).dxdy_at_t(t),
        Verb::Conic => DConic::from_pts(a, weight).dxdy_at_t(t),
        Verb::Cubic => DCubic::from_pts(a).dxdy_at_t(t),
        _ => unreachable!(),
    }
}

/// `CurveDDSlopeAtT[verb]`.
pub fn curve_dd_slope_at_t(verb: Verb, c: &DCurve, t: f64) -> DVector {
    match verb {
        Verb::Line => c.pts[1] - c.pts[0],
        Verb::Quad => c.quad().dxdy_at_t(t),
        Verb::Conic => c.conic().dxdy_at_t(t),
        Verb::Cubic => c.cubic().dxdy_at_t(t),
        _ => unreachable!(),
    }
}

/// `CurveSlopeAtT[verb]`.
pub fn curve_slope_at_t(verb: Verb, a: &[Point], weight: f32, t: f64) -> Point {
    match verb {
        Verb::Line => Point::new(a[1].x - a[0].x, a[1].y - a[0].y),
        _ => curve_d_slope_at_t(verb, a, weight, t).as_sk_vector(),
    }
}

/// `CurveIsVertical[verb]`.
pub fn curve_is_vertical(verb: Verb, a: &[Point], weight: f32, start_t: f64, end_t: f64) -> bool {
    match verb {
        Verb::Line => {
            let line = DLine::from_pts(a);
            let dst = [line.pt_at_t(start_t), line.pt_at_t(end_t)];
            almost_equal_ulps_d(dst[0].x, dst[1].x)
        }
        Verb::Quad => {
            let dst = DQuad::from_pts(a).sub_divide(start_t, end_t);
            almost_equal_ulps_d(dst[0].x, dst[1].x) && almost_equal_ulps_d(dst[1].x, dst[2].x)
        }
        Verb::Conic => {
            let dst = DConic::from_pts(a, weight).sub_divide(start_t, end_t);
            almost_equal_ulps_d(dst[0].x, dst[1].x) && almost_equal_ulps_d(dst[1].x, dst[2].x)
        }
        Verb::Cubic => {
            let dst = DCubic::from_pts(a).sub_divide(start_t, end_t);
            almost_equal_ulps_d(dst[0].x, dst[1].x)
                && almost_equal_ulps_d(dst[1].x, dst[2].x)
                && almost_equal_ulps_d(dst[2].x, dst[3].x)
        }
        _ => unreachable!(),
    }
}

/// `CurveIntersectRay[verb]`.
pub fn curve_intersect_ray(verb: Verb, a: &[Point], weight: f32, ray: &DLine, i: &mut Intersections) {
    match verb {
        Verb::Line => {
            i.intersect_ray_line(&DLine::from_pts(a), ray);
        }
        Verb::Quad => {
            i.intersect_ray_quad(&DQuad::from_pts(a), ray);
        }
        Verb::Conic => {
            i.intersect_ray_conic(&DConic::from_pts(a, weight), ray);
        }
        Verb::Cubic => {
            i.intersect_ray_cubic(&DCubic::from_pts(a), ray);
        }
        _ => unreachable!(),
    }
}

/// `CurveDIntersectRay[verb]`.
pub fn curve_d_intersect_ray(verb: Verb, c: &DCurve, ray: &DLine, i: &mut Intersections) {
    match verb {
        Verb::Line => {
            i.intersect_ray_line(&c.line(), ray);
        }
        Verb::Quad => {
            i.intersect_ray_quad(&c.quad(), ray);
        }
        Verb::Conic => {
            i.intersect_ray_conic(&c.conic(), ray);
        }
        Verb::Cubic => {
            i.intersect_ray_cubic(&c.cubic(), ray);
        }
        _ => unreachable!(),
    }
}

/// `CurveIntercept[verb * 2 + vertical]`: the curve's crossings of the
/// horizontal (or vertical) line through `xy`.
pub fn curve_intercept(verb: Verb, vertical: bool, a: &[Point], w: f32, xy: f32, roots: &mut [f64]) -> i32 {
    match (verb, vertical) {
        (Verb::Line, false) => {
            if a[0].y == a[1].y {
                return 0;
            }
            roots[0] = Intersections::HorizontalInterceptLine(&DLine::from_pts(a), xy as f64);
            between(0.0, roots[0], 1.0) as i32
        }
        (Verb::Line, true) => {
            if a[0].x == a[1].x {
                return 0;
            }
            roots[0] = Intersections::VerticalInterceptLine(&DLine::from_pts(a), xy as f64);
            between(0.0, roots[0], 1.0) as i32
        }
        (Verb::Quad, false) => Intersections::HorizontalInterceptQuad(&DQuad::from_pts(a), xy as f64, roots),
        (Verb::Quad, true) => Intersections::VerticalInterceptQuad(&DQuad::from_pts(a), xy as f64, roots),
        (Verb::Conic, false) => Intersections::HorizontalInterceptConic(&DConic::from_pts(a, w), xy as f64, roots),
        (Verb::Conic, true) => Intersections::VerticalInterceptConic(&DConic::from_pts(a, w), xy as f64, roots),
        (Verb::Cubic, false) => DCubic::from_pts(a).horizontal_intersect(xy as f64, roots),
        (Verb::Cubic, true) => DCubic::from_pts(a).vertical_intersect(xy as f64, roots),
        _ => unreachable!(),
    }
}
