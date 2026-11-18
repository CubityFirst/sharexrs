//! `SkDQuadLineIntersection.cpp` (`LineQuadraticIntersections`),
//! `SkDConicLineIntersection.cpp` (`LineConicIntersections`) and
//! `SkDCubicLineIntersection.cpp` (`LineCubicIntersections`). The three
//! classes are the same code over different curves; they share one generic
//! here, with the per-curve differences (`intersectRay`, `pinTs`, the axis
//! intercepts) kept per curve.

use super::conic::DConic;
use super::cubic::{DCubic, SearchAxis};
use super::curve::DCurve;
use super::intersections::Intersections;
use super::line::DLine;
use super::point::DPoint;
use super::quad::DQuad;
use super::types::*;
use crate::path::Verb;

#[derive(Clone, Copy, PartialEq, Eq)]
enum PinTPoint {
    Uninitialized,
    Initialized,
}

pub trait LineCurve {
    const POINT_LAST: usize;
    const VERB: Verb;
    fn pt_at_t(&self, t: f64) -> DPoint;
    fn p(&self, i: usize) -> DPoint;
    fn as_dcurve(&self) -> DCurve;
}

impl LineCurve for DQuad {
    const POINT_LAST: usize = 2;
    const VERB: Verb = Verb::Quad;
    fn pt_at_t(&self, t: f64) -> DPoint {
        DQuad::pt_at_t(self, t)
    }
    fn p(&self, i: usize) -> DPoint {
        self.pts[i]
    }
    fn as_dcurve(&self) -> DCurve {
        let mut c = DCurve::default();
        c.set_quad(self);
        c
    }
}

impl LineCurve for DConic {
    const POINT_LAST: usize = 2;
    const VERB: Verb = Verb::Conic;
    fn pt_at_t(&self, t: f64) -> DPoint {
        DConic::pt_at_t(self, t)
    }
    fn p(&self, i: usize) -> DPoint {
        self.pts.pts[i]
    }
    fn as_dcurve(&self) -> DCurve {
        let mut c = DCurve::default();
        c.set_conic(self);
        c
    }
}

impl LineCurve for DCubic {
    const POINT_LAST: usize = 3;
    const VERB: Verb = Verb::Cubic;
    fn pt_at_t(&self, t: f64) -> DPoint {
        DCubic::pt_at_t(self, t)
    }
    fn p(&self, i: usize) -> DPoint {
        self.pts[i]
    }
    fn as_dcurve(&self) -> DCurve {
        let mut c = DCurve::default();
        c.set_cubic(self);
        c
    }
}

/// `hasT`.
fn has_t(i: &Intersections, t: f64) -> bool {
    i.used > 0 && (if t == 0.0 { i.ft[0][0] == 0.0 } else { i.ft[0][i.used as usize - 1] == 1.0 })
}

/// `hasOppT`.
fn has_opp_t(i: &Intersections, t: f64) -> bool {
    i.used > 0 && (i.ft[1][0] == t || i.ft[1][i.used as usize - 1] == t)
}

pub struct LineCurveIntersections<'a, C: LineCurve> {
    curve: &'a C,
    line: DLine,
    intersections: &'a mut Intersections,
    allow_near: bool,
}

impl<'a, C: LineCurve> LineCurveIntersections<'a, C> {
    fn new(curve: &'a C, line: DLine, i: &'a mut Intersections, max: i32) -> Self {
        i.set_max(max);
        LineCurveIntersections { curve, line, intersections: i, allow_near: true }
    }

    fn check_coincident(&mut self) {
        let mut last = self.intersections.used() - 1;
        let mut index = 0;
        while index < last {
            let mid_t = (self.intersections.t(0, index as usize) + self.intersections.t(0, index as usize + 1)) / 2.0;
            let mid_pt = self.curve.pt_at_t(mid_t);
            let t = self.line.near_point(&mid_pt, None);
            if t < 0.0 {
                index += 1;
                continue;
            }
            if self.intersections.is_coincident(index as usize) {
                self.intersections.remove_one(index as usize);
                last -= 1;
            } else if self.intersections.is_coincident(index as usize + 1) {
                self.intersections.remove_one(index as usize + 1);
                last -= 1;
            } else {
                self.intersections.set_coincident(index as usize);
                index += 1;
            }
            self.intersections.set_coincident(index as usize);
        }
    }

    fn unique_answer(&self, curve_t: f64, pt: &DPoint) -> bool {
        for inner in 0..self.intersections.used() as usize {
            if self.intersections.pt(inner) != *pt {
                continue;
            }
            let existing_t = self.intersections.t(0, inner);
            if curve_t == existing_t {
                return false;
            }
            let mid_t = (existing_t + curve_t) / 2.0;
            let mid_pt = self.curve.pt_at_t(mid_t);
            if mid_pt.approximately_equal(pt) {
                return false;
            }
        }
        true
    }

    fn end_indices() -> [usize; 2] {
        [0, C::POINT_LAST]
    }

    fn add_exact_end_points(&mut self) {
        for c_index in Self::end_indices() {
            let line_t = self.line.exact_point(&self.curve.p(c_index));
            if line_t < 0.0 {
                continue;
            }
            let curve_t = (c_index >> 1) as f64;
            self.intersections.insert(curve_t, line_t, &self.curve.p(c_index));
        }
    }

    fn add_near_end_points(&mut self) {
        for c_index in Self::end_indices() {
            let curve_t = (c_index >> 1) as f64;
            if has_t(self.intersections, curve_t) {
                continue;
            }
            let line_t = self.line.near_point(&self.curve.p(c_index), None);
            if line_t < 0.0 {
                continue;
            }
            self.intersections.insert(curve_t, line_t, &self.curve.p(c_index));
        }
        self.add_line_near_end_points();
    }

    fn add_line_near_end_points(&mut self) {
        for l_index in 0..2 {
            let line_t = l_index as f64;
            if has_opp_t(self.intersections, line_t) {
                continue;
            }
            let curve_t = self.curve.as_dcurve().near_point(C::VERB, &self.line[l_index], &self.line[1 - l_index]);
            if curve_t < 0.0 {
                continue;
            }
            let p = self.line[l_index];
            self.intersections.insert(curve_t, line_t, &p);
        }
    }

    fn add_exact_horizontal_end_points(&mut self, left: f64, right: f64, y: f64) {
        for c_index in Self::end_indices() {
            let line_t = DLine::ExactPointH(&self.curve.p(c_index), left, right, y);
            if line_t < 0.0 {
                continue;
            }
            let curve_t = (c_index >> 1) as f64;
            self.intersections.insert(curve_t, line_t, &self.curve.p(c_index));
        }
    }

    fn add_near_horizontal_end_points(&mut self, left: f64, right: f64, y: f64) {
        for c_index in Self::end_indices() {
            let curve_t = (c_index >> 1) as f64;
            if has_t(self.intersections, curve_t) {
                continue;
            }
            let line_t = DLine::NearPointH(&self.curve.p(c_index), left, right, y);
            if line_t < 0.0 {
                continue;
            }
            self.intersections.insert(curve_t, line_t, &self.curve.p(c_index));
        }
        self.add_line_near_end_points();
    }

    fn add_exact_vertical_end_points(&mut self, top: f64, bottom: f64, x: f64) {
        for c_index in Self::end_indices() {
            let line_t = DLine::ExactPointV(&self.curve.p(c_index), top, bottom, x);
            if line_t < 0.0 {
                continue;
            }
            let curve_t = (c_index >> 1) as f64;
            self.intersections.insert(curve_t, line_t, &self.curve.p(c_index));
        }
    }

    fn add_near_vertical_end_points(&mut self, top: f64, bottom: f64, x: f64) {
        for c_index in Self::end_indices() {
            let curve_t = (c_index >> 1) as f64;
            if has_t(self.intersections, curve_t) {
                continue;
            }
            let line_t = DLine::NearPointV(&self.curve.p(c_index), top, bottom, x);
            if line_t < 0.0 {
                continue;
            }
            self.intersections.insert(curve_t, line_t, &self.curve.p(c_index));
        }
        self.add_line_near_end_points();
    }

    fn find_line_t(&self, t: f64) -> f64 {
        let xy = self.curve.pt_at_t(t);
        let dx = self.line[1].x - self.line[0].x;
        let dy = self.line[1].y - self.line[0].y;
        if dx.abs() > dy.abs() {
            return (xy.x - self.line[0].x) / dx;
        }
        (xy.y - self.line[0].y) / dy
    }

    /// The quad's and conic's `pinTs`.
    fn pin_ts_quadratic(&self, curve_t: &mut f64, line_t: &mut f64, pt: &mut DPoint, pt_set: PinTPoint) -> bool {
        if !approximately_one_or_less_double(*line_t) {
            return false;
        }
        if !approximately_zero_or_more_double(*line_t) {
            return false;
        }
        *curve_t = sk_pin_t(*curve_t);
        let q_t = *curve_t;
        *line_t = sk_pin_t(*line_t);
        let l_t = *line_t;
        if l_t == 0.0 || l_t == 1.0 || (pt_set == PinTPoint::Uninitialized && q_t != 0.0 && q_t != 1.0) {
            *pt = self.line.pt_at_t(l_t);
        } else if pt_set == PinTPoint::Uninitialized {
            *pt = self.curve.pt_at_t(q_t);
        }
        let grid_pt = pt.as_sk_point();
        if DPoint::approximately_equal_sk(grid_pt, self.line[0].as_sk_point()) {
            *pt = self.line[0];
            *line_t = 0.0;
        } else if DPoint::approximately_equal_sk(grid_pt, self.line[1].as_sk_point()) {
            *pt = self.line[1];
            *line_t = 1.0;
        }
        if self.intersections.used() > 0 && approximately_equal(self.intersections.t(1, 0), *line_t) {
            return false;
        }
        if grid_pt == self.curve.p(0).as_sk_point() {
            *pt = self.curve.p(0);
            *curve_t = 0.0;
        } else if grid_pt == self.curve.p(2).as_sk_point() {
            *pt = self.curve.p(2);
            *curve_t = 1.0;
        }
        true
    }

    /// The cubic's `pinTs`.
    fn pin_ts_cubic(&self, cubic_t: &mut f64, line_t: &mut f64, pt: &mut DPoint, pt_set: PinTPoint) -> bool {
        if !approximately_one_or_less(*line_t) {
            return false;
        }
        if !approximately_zero_or_more(*line_t) {
            return false;
        }
        *cubic_t = sk_pin_t(*cubic_t);
        let c_t = *cubic_t;
        *line_t = sk_pin_t(*line_t);
        let l_t = *line_t;
        let l_pt = self.line.pt_at_t(l_t);
        let c_pt = self.curve.pt_at_t(c_t);
        if !l_pt.roughly_equal(&c_pt) {
            return false;
        }
        if l_t == 0.0 || l_t == 1.0 || (pt_set == PinTPoint::Uninitialized && c_t != 0.0 && c_t != 1.0) {
            *pt = l_pt;
        } else if pt_set == PinTPoint::Uninitialized {
            *pt = c_pt;
        }
        let grid_pt = pt.as_sk_point();
        if grid_pt == self.line[0].as_sk_point() {
            *line_t = 0.0;
        } else if grid_pt == self.line[1].as_sk_point() {
            *line_t = 1.0;
        }
        if grid_pt == self.curve.p(0).as_sk_point() && approximately_equal(*cubic_t, 0.0) {
            *cubic_t = 0.0;
        } else if grid_pt == self.curve.p(3).as_sk_point() && approximately_equal(*cubic_t, 1.0) {
            *cubic_t = 1.0;
        }
        true
    }

    fn pin_ts(&self, curve_t: &mut f64, line_t: &mut f64, pt: &mut DPoint, pt_set: PinTPoint) -> bool {
        if C::VERB == Verb::Cubic {
            self.pin_ts_cubic(curve_t, line_t, pt, pt_set)
        } else {
            self.pin_ts_quadratic(curve_t, line_t, pt, pt_set)
        }
    }
}

/// The per-curve `intersectRay(double roots[])` and axis intercepts.
pub trait LineCurveRoots: LineCurve + Sized {
    fn intersect_ray_roots(lci: &LineCurveIntersections<Self>, roots: &mut [f64; 3]) -> i32;
    fn horizontal_roots(&self, axis_intercept: f64, roots: &mut [f64]) -> i32;
    fn vertical_roots(&self, axis_intercept: f64, roots: &mut [f64]) -> i32;
    /// The point on the curve for a horizontal (vertical) intercept root.
    fn axis_pt(&self, t: f64, axis_intercept: f64, vertical: bool) -> DPoint;
}

impl LineCurveRoots for DQuad {
    fn intersect_ray_roots(lci: &LineCurveIntersections<Self>, roots: &mut [f64; 3]) -> i32 {
        let line = &lci.line;
        let adj = line[1].x - line[0].x;
        let opp = line[1].y - line[0].y;
        let mut r = [0.0f64; 3];
        for n in 0..3 {
            r[n] = (lci.curve[n].y - line[0].y) * adj - (lci.curve[n].x - line[0].x) * opp;
        }
        let mut a = r[2];
        let mut b = r[1];
        let c = r[0];
        a += c - 2.0 * b; // A = a - 2*b + c
        b -= c; // B = -(b - c)
        DQuad::RootsValidT(a, 2.0 * b, c, roots)
    }

    fn horizontal_roots(&self, axis_intercept: f64, roots: &mut [f64]) -> i32 {
        let mut d = self[2].y; // f
        let mut e = self[1].y; // e
        let mut f = self[0].y; // d
        d += f - 2.0 * e; // D = d - 2*e + f
        e -= f; // E = -(d - e)
        f -= axis_intercept;
        DQuad::RootsValidT(d, 2.0 * e, f, roots)
    }

    fn vertical_roots(&self, axis_intercept: f64, roots: &mut [f64]) -> i32 {
        let mut d = self[2].x;
        let mut e = self[1].x;
        let mut f = self[0].x;
        d += f - 2.0 * e;
        e -= f;
        f -= axis_intercept;
        DQuad::RootsValidT(d, 2.0 * e, f, roots)
    }

    fn axis_pt(&self, t: f64, _axis_intercept: f64, _vertical: bool) -> DPoint {
        self.pt_at_t(t)
    }
}

fn conic_valid_t(conic: &DConic, r: [f64; 3], axis_intercept: f64, roots: &mut [f64]) -> i32 {
    let w = conic.weight as f64;
    let mut a = r[2];
    let mut b = r[1] * w - axis_intercept * w + axis_intercept;
    let mut c = r[0];
    a += c - 2.0 * b; // A = a + c - 2*(b*w - xCept*w + xCept)
    b -= c; // B = b*w - w * xCept + xCept - a
    c -= axis_intercept;
    DQuad::RootsValidT(a, 2.0 * b, c, roots)
}

impl LineCurveRoots for DConic {
    fn intersect_ray_roots(lci: &LineCurveIntersections<Self>, roots: &mut [f64; 3]) -> i32 {
        let line = &lci.line;
        let adj = line[1].x - line[0].x;
        let opp = line[1].y - line[0].y;
        let mut r = [0.0f64; 3];
        for n in 0..3 {
            r[n] = (lci.curve[n].y - line[0].y) * adj - (lci.curve[n].x - line[0].x) * opp;
        }
        conic_valid_t(lci.curve, r, 0.0, roots)
    }

    fn horizontal_roots(&self, axis_intercept: f64, roots: &mut [f64]) -> i32 {
        conic_valid_t(self, [self[0].y, self[1].y, self[2].y], axis_intercept, roots)
    }

    fn vertical_roots(&self, axis_intercept: f64, roots: &mut [f64]) -> i32 {
        conic_valid_t(self, [self[0].x, self[1].x, self[2].x], axis_intercept, roots)
    }

    fn axis_pt(&self, t: f64, _axis_intercept: f64, _vertical: bool) -> DPoint {
        self.pt_at_t(t)
    }
}

impl LineCurveRoots for DCubic {
    fn intersect_ray_roots(lci: &LineCurveIntersections<Self>, roots: &mut [f64; 3]) -> i32 {
        let line = &lci.line;
        let cubic = lci.curve;
        let adj = line[1].x - line[0].x;
        let opp = line[1].y - line[0].y;
        let mut c = DCubic::default();
        for n in 0..4 {
            c[n].x = (cubic[n].y - line[0].y) * adj - (cubic[n].x - line[0].x) * opp;
        }
        let (a, b, cc, d) = DCubic::Coefficients(c.xs());
        let mut count = DCubic::RootsValidT(a, b, cc, d, roots);
        for index in 0..count as usize {
            let calc_pt = c.pt_at_t(roots[index]);
            if !approximately_zero(calc_pt.x) {
                for n in 0..4 {
                    c[n].y = (cubic[n].y - line[0].y) * opp + (cubic[n].x - line[0].x) * adj;
                }
                let mut extreme_ts = [0.0f64; 6];
                let extrema = DCubic::FindExtrema(c.xs(), &mut extreme_ts);
                count = c.search_roots(&mut extreme_ts, extrema as usize, 0.0, SearchAxis::XAxis, roots);
                break;
            }
        }
        count
    }

    fn horizontal_roots(&self, axis_intercept: f64, roots: &mut [f64]) -> i32 {
        self.horizontal_intersect(axis_intercept, roots)
    }

    fn vertical_roots(&self, axis_intercept: f64, roots: &mut [f64]) -> i32 {
        self.vertical_intersect(axis_intercept, roots)
    }

    fn axis_pt(&self, t: f64, axis_intercept: f64, vertical: bool) -> DPoint {
        if vertical {
            DPoint::new(axis_intercept, self.pt_at_t(t).y)
        } else {
            DPoint::new(self.pt_at_t(t).x, axis_intercept)
        }
    }
}

impl DCubic {
    /// `LineCubicIntersections::HorizontalIntersect`.
    pub fn horizontal_intersect(&self, axis_intercept: f64, roots: &mut [f64]) -> i32 {
        let (a, b, c, mut d) = DCubic::Coefficients(self.ys());
        d -= axis_intercept;
        let mut count = DCubic::RootsValidT(a, b, c, d, roots);
        for index in 0..count as usize {
            let calc_pt = self.pt_at_t(roots[index]);
            if !approximately_equal(calc_pt.y, axis_intercept) {
                let mut extreme_ts = [0.0f64; 6];
                let extrema = DCubic::FindExtrema(self.ys(), &mut extreme_ts);
                count = self.search_roots(&mut extreme_ts, extrema as usize, axis_intercept, SearchAxis::YAxis, roots);
                break;
            }
        }
        count
    }

    /// `LineCubicIntersections::VerticalIntersect`.
    pub fn vertical_intersect(&self, axis_intercept: f64, roots: &mut [f64]) -> i32 {
        let (a, b, c, mut d) = DCubic::Coefficients(self.xs());
        d -= axis_intercept;
        let mut count = DCubic::RootsValidT(a, b, c, d, roots);
        for index in 0..count as usize {
            let calc_pt = self.pt_at_t(roots[index]);
            if !approximately_equal(calc_pt.x, axis_intercept) {
                let mut extreme_ts = [0.0f64; 6];
                let extrema = DCubic::FindExtrema(self.xs(), &mut extreme_ts);
                count = self.search_roots(&mut extreme_ts, extrema as usize, axis_intercept, SearchAxis::XAxis, roots);
                break;
            }
        }
        count
    }
}

impl<'a, C: LineCurveRoots> LineCurveIntersections<'a, C> {
    fn intersect(&mut self) -> i32 {
        self.add_exact_end_points();
        if self.allow_near {
            self.add_near_end_points();
        }
        let mut root_vals = [0.0f64; 3];
        let roots = C::intersect_ray_roots(self, &mut root_vals);
        for index in 0..roots as usize {
            let mut curve_t = root_vals[index];
            let mut line_t = self.find_line_t(curve_t);
            let mut pt = DPoint::default();
            if self.pin_ts(&mut curve_t, &mut line_t, &mut pt, PinTPoint::Uninitialized)
                && self.unique_answer(curve_t, &pt)
            {
                self.intersections.insert(curve_t, line_t, &pt);
            }
        }
        self.check_coincident();
        self.intersections.used()
    }

    fn horizontal_intersect(&mut self, axis_intercept: f64, left: f64, right: f64, flipped: bool) -> i32 {
        self.add_exact_horizontal_end_points(left, right, axis_intercept);
        if self.allow_near {
            self.add_near_horizontal_end_points(left, right, axis_intercept);
        }
        let mut roots = [0.0f64; 3];
        let count = self.curve.horizontal_roots(axis_intercept, &mut roots);
        for index in 0..count as usize {
            let mut curve_t = roots[index];
            let mut pt = self.curve.axis_pt(curve_t, axis_intercept, false);
            let mut line_t = (pt.x - left) / (right - left);
            if self.pin_ts(&mut curve_t, &mut line_t, &mut pt, PinTPoint::Initialized)
                && self.unique_answer(curve_t, &pt)
            {
                self.intersections.insert(curve_t, line_t, &pt);
            }
        }
        if flipped {
            self.intersections.flip();
        }
        self.check_coincident();
        self.intersections.used()
    }

    fn vertical_intersect(&mut self, axis_intercept: f64, top: f64, bottom: f64, flipped: bool) -> i32 {
        self.add_exact_vertical_end_points(top, bottom, axis_intercept);
        if self.allow_near {
            self.add_near_vertical_end_points(top, bottom, axis_intercept);
        }
        let mut roots = [0.0f64; 3];
        let count = self.curve.vertical_roots(axis_intercept, &mut roots);
        for index in 0..count as usize {
            let mut curve_t = roots[index];
            let mut pt = self.curve.axis_pt(curve_t, axis_intercept, true);
            let mut line_t = (pt.y - top) / (bottom - top);
            if self.pin_ts(&mut curve_t, &mut line_t, &mut pt, PinTPoint::Initialized)
                && self.unique_answer(curve_t, &pt)
            {
                self.intersections.insert(curve_t, line_t, &pt);
            }
        }
        if flipped {
            self.intersections.flip();
        }
        self.check_coincident();
        self.intersections.used()
    }
}

// The quad and conic set fMax to 5 and 4; the cubic to 4.
fn max_for(verb: Verb) -> i32 {
    match verb {
        Verb::Quad => 5,
        _ => 4,
    }
}

impl Intersections {
    pub fn horizontal_quad(&mut self, quad: &DQuad, left: f64, right: f64, y: f64, flipped: bool) -> i32 {
        let line = DLine::new(DPoint::new(left, y), DPoint::new(right, y));
        let mut q = LineCurveIntersections::new(quad, line, self, max_for(Verb::Quad));
        q.horizontal_intersect(y, left, right, flipped)
    }

    pub fn vertical_quad(&mut self, quad: &DQuad, top: f64, bottom: f64, x: f64, flipped: bool) -> i32 {
        let line = DLine::new(DPoint::new(x, top), DPoint::new(x, bottom));
        let mut q = LineCurveIntersections::new(quad, line, self, max_for(Verb::Quad));
        q.vertical_intersect(x, top, bottom, flipped)
    }

    pub fn intersect_quad_line(&mut self, quad: &DQuad, line: &DLine) -> i32 {
        let allow = self.allow_near;
        let mut q = LineCurveIntersections::new(quad, *line, self, max_for(Verb::Quad));
        q.allow_near = allow;
        q.intersect()
    }

    pub fn intersect_ray_quad(&mut self, quad: &DQuad, line: &DLine) -> i32 {
        let mut roots = [self.ft[0][0], self.ft[0][1], self.ft[0][2]];
        let used = {
            let q = LineCurveIntersections::new(quad, *line, self, max_for(Verb::Quad));
            DQuad::intersect_ray_roots(&q, &mut roots)
        };
        // the roots are written into fT[0] in place
        self.ft[0][..3].copy_from_slice(&roots);
        self.used = used as u8;
        for index in 0..used as usize {
            self.pt[index] = quad.pt_at_t(roots[index]);
        }
        self.used as i32
    }

    #[allow(non_snake_case)]
    pub fn HorizontalInterceptQuad(quad: &DQuad, y: f64, roots: &mut [f64]) -> i32 {
        quad.horizontal_roots(y as f32 as f64, roots)
    }

    #[allow(non_snake_case)]
    pub fn VerticalInterceptQuad(quad: &DQuad, x: f64, roots: &mut [f64]) -> i32 {
        quad.vertical_roots(x as f32 as f64, roots)
    }

    pub fn horizontal_conic(&mut self, conic: &DConic, left: f64, right: f64, y: f64, flipped: bool) -> i32 {
        let line = DLine::new(DPoint::new(left, y), DPoint::new(right, y));
        let mut c = LineCurveIntersections::new(conic, line, self, max_for(Verb::Conic));
        c.horizontal_intersect(y, left, right, flipped)
    }

    pub fn vertical_conic(&mut self, conic: &DConic, top: f64, bottom: f64, x: f64, flipped: bool) -> i32 {
        let line = DLine::new(DPoint::new(x, top), DPoint::new(x, bottom));
        let mut c = LineCurveIntersections::new(conic, line, self, max_for(Verb::Conic));
        c.vertical_intersect(x, top, bottom, flipped)
    }

    pub fn intersect_conic_line(&mut self, conic: &DConic, line: &DLine) -> i32 {
        let allow = self.allow_near;
        let mut c = LineCurveIntersections::new(conic, *line, self, max_for(Verb::Conic));
        c.allow_near = allow;
        c.intersect()
    }

    pub fn intersect_ray_conic(&mut self, conic: &DConic, line: &DLine) -> i32 {
        let mut roots = [self.ft[0][0], self.ft[0][1], self.ft[0][2]];
        let used = {
            let c = LineCurveIntersections::new(conic, *line, self, max_for(Verb::Conic));
            DConic::intersect_ray_roots(&c, &mut roots)
        };
        // the roots are written into fT[0] in place
        self.ft[0][..3].copy_from_slice(&roots);
        self.used = used as u8;
        for index in 0..used as usize {
            self.pt[index] = conic.pt_at_t(roots[index]);
        }
        self.used as i32
    }

    #[allow(non_snake_case)]
    pub fn HorizontalInterceptConic(conic: &DConic, y: f64, roots: &mut [f64]) -> i32 {
        conic.horizontal_roots(y as f32 as f64, roots)
    }

    #[allow(non_snake_case)]
    pub fn VerticalInterceptConic(conic: &DConic, x: f64, roots: &mut [f64]) -> i32 {
        conic.vertical_roots(x as f32 as f64, roots)
    }

    pub fn horizontal_cubic(&mut self, cubic: &DCubic, left: f64, right: f64, y: f64, flipped: bool) -> i32 {
        let line = DLine::new(DPoint::new(left, y), DPoint::new(right, y));
        let mut c = LineCurveIntersections::new(cubic, line, self, max_for(Verb::Cubic));
        c.horizontal_intersect(y, left, right, flipped)
    }

    pub fn vertical_cubic(&mut self, cubic: &DCubic, top: f64, bottom: f64, x: f64, flipped: bool) -> i32 {
        let line = DLine::new(DPoint::new(x, top), DPoint::new(x, bottom));
        let mut c = LineCurveIntersections::new(cubic, line, self, max_for(Verb::Cubic));
        c.vertical_intersect(x, top, bottom, flipped)
    }

    pub fn intersect_cubic_line(&mut self, cubic: &DCubic, line: &DLine) -> i32 {
        let allow = self.allow_near;
        let mut c = LineCurveIntersections::new(cubic, *line, self, max_for(Verb::Cubic));
        c.allow_near = allow;
        c.intersect()
    }

    pub fn intersect_ray_cubic(&mut self, cubic: &DCubic, line: &DLine) -> i32 {
        let mut roots = [self.ft[0][0], self.ft[0][1], self.ft[0][2]];
        let used = {
            let c = LineCurveIntersections::new(cubic, *line, self, max_for(Verb::Cubic));
            DCubic::intersect_ray_roots(&c, &mut roots)
        };
        // the roots are written into fT[0] in place
        self.ft[0][..3].copy_from_slice(&roots);
        self.used = used as u8;
        for index in 0..used as usize {
            self.pt[index] = cubic.pt_at_t(roots[index]);
        }
        self.used as i32
    }
}
