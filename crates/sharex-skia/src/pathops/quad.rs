//! `SkPathOpsQuad.h`, `SkPathOpsQuad.cpp`: `SkDQuad`.

use super::conic::DConic;
use super::cubic::DCubic;
use super::intersections::Intersections;
use super::line::DLine;
use super::line_parameters::LineParameters;
use super::point::{DPoint, DVector};
use super::types::*;
use crate::geom::Point;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DQuad {
    pub pts: [DPoint; 3],
}

/// `SkDQuadPair`: two quads sharing pts[2].
#[derive(Clone, Copy, Debug, Default)]
pub struct DQuadPair {
    pub pts: [DPoint; 5],
}

impl DQuadPair {
    pub fn first(&self) -> DQuad {
        DQuad { pts: [self.pts[0], self.pts[1], self.pts[2]] }
    }
    pub fn second(&self) -> DQuad {
        DQuad { pts: [self.pts[2], self.pts[3], self.pts[4]] }
    }
}

impl std::ops::Index<usize> for DQuad {
    type Output = DPoint;
    fn index(&self, n: usize) -> &DPoint {
        &self.pts[n]
    }
}

impl std::ops::IndexMut<usize> for DQuad {
    fn index_mut(&mut self, n: usize) -> &mut DPoint {
        &mut self.pts[n]
    }
}

// from blackpawn.com/texts/pointinpoly
fn point_in_triangle(pts: &[DPoint; 3], test: &DPoint) -> bool {
    let v0 = pts[2] - pts[0];
    let v1 = pts[1] - pts[0];
    let v2 = *test - pts[0];
    let dot00 = v0.dot(v0);
    let dot01 = v0.dot(v1);
    let dot02 = v0.dot(v2);
    let dot11 = v1.dot(v1);
    let dot12 = v1.dot(v2);
    // Compute barycentric coordinates
    let denom = dot00 * dot11 - dot01 * dot01;
    let u = dot11 * dot02 - dot01 * dot12;
    let v = dot00 * dot12 - dot01 * dot02;
    // Check if point is in triangle
    if denom >= 0.0 {
        return u >= 0.0 && v >= 0.0 && u + v < denom;
    }
    u <= 0.0 && v <= 0.0 && u + v > denom
}

fn matches_end(pts: &[DPoint; 3], test: &DPoint) -> bool {
    pts[0] == *test || pts[2] == *test
}

fn handle_zero(b: f64, c: f64, s: &mut [f64]) -> i32 {
    if approximately_zero(b) {
        s[0] = 0.0;
        return (c == 0.0) as i32;
    }
    s[0] = -c / b;
    1
}

/// `interp_quad_coords(const double* src, double t)` over one coordinate.
fn interp_quad_coords(src: [f64; 3], t: f64) -> f64 {
    if 0.0 == t {
        return src[0];
    }
    if 1.0 == t {
        return src[2];
    }
    let ab = sk_d_interp(src[0], src[1], t);
    let bc = sk_d_interp(src[1], src[2], t);
    sk_d_interp(ab, bc, t)
}

/// The classic one t subdivision, over one coordinate.
fn interp_quad_coords_split(src: [f64; 3], t: f64) -> [f64; 5] {
    let ab = sk_d_interp(src[0], src[1], t);
    let bc = sk_d_interp(src[1], src[2], t);
    [src[0], ab, sk_d_interp(ab, bc, t), bc, src[2]]
}

fn valid_unit_divide(mut numer: f64, mut denom: f64, ratio: &mut f64) -> i32 {
    if numer < 0.0 {
        numer = -numer;
        denom = -denom;
    }
    if denom == 0.0 || numer == 0.0 || numer >= denom {
        return 0;
    }
    let r = numer / denom;
    if r == 0.0 {
        // catch underflow if numer <<<< denom
        return 0;
    }
    *ratio = r;
    1
}

pub const QUAD_POINT_COUNT: usize = 3;
pub const QUAD_MAX_INTERSECTIONS: i32 = 4;

impl DQuad {
    pub fn from_pts(pts: &[Point]) -> Self {
        DQuad { pts: [DPoint::from_pt(pts[0]), DPoint::from_pt(pts[1]), DPoint::from_pt(pts[2])] }
    }

    pub fn set(&mut self, pts: &[Point]) -> &Self {
        *self = DQuad::from_pts(pts);
        self
    }

    pub fn xs(&self) -> [f64; 3] {
        [self.pts[0].x, self.pts[1].x, self.pts[2].x]
    }

    pub fn ys(&self) -> [f64; 3] {
        [self.pts[0].y, self.pts[1].y, self.pts[2].y]
    }

    pub fn collapsed(&self) -> bool {
        self.pts[0].approximately_equal(&self.pts[1]) && self.pts[0].approximately_equal(&self.pts[2])
    }

    pub fn controls_inside(&self) -> bool {
        let v01 = self.pts[0] - self.pts[1];
        let v02 = self.pts[0] - self.pts[2];
        let v12 = self.pts[1] - self.pts[2];
        v02.dot(v01) > 0.0 && v02.dot(v12) > 0.0
    }

    pub fn flip(&self) -> DQuad {
        DQuad { pts: [self.pts[2], self.pts[1], self.pts[0]] }
    }

    /// If returning true, `is_linear` is true if the quad's hull collapsed;
    /// if returning false, the quad pair have only the end point in common.
    pub fn hull_intersects_quad(&self, q2: &DQuad, is_linear: &mut bool) -> bool {
        let mut linear = true;
        for odd_man in 0..3 {
            let end_pt = self.other_pts(odd_man);
            let orig_x = end_pt[0].x;
            let orig_y = end_pt[0].y;
            let adj = end_pt[1].x - orig_x;
            let opp = end_pt[1].y - orig_y;
            let sign = (self.pts[odd_man].y - orig_y) * adj - (self.pts[odd_man].x - orig_x) * opp;
            if approximately_zero(sign) {
                continue;
            }
            linear = false;
            let mut found_outlier = false;
            for n in 0..3 {
                let test = (q2[n].y - orig_y) * adj - (q2[n].x - orig_x) * opp;
                if test * sign > 0.0 && !precisely_zero(test) {
                    found_outlier = true;
                    break;
                }
            }
            if !found_outlier {
                return false;
            }
        }
        if linear && !matches_end(&self.pts, &q2.pts[0]) && !matches_end(&self.pts, &q2.pts[2]) {
            // if the end point of the opposite quad is inside the hull that is nearly a line,
            // then representing the quad as a line may cause the intersection to be missed.
            // Check to see if the endpoint is in the triangle.
            if point_in_triangle(&self.pts, &q2.pts[0]) || point_in_triangle(&self.pts, &q2.pts[2]) {
                linear = false;
            }
        }
        *is_linear = linear;
        true
    }

    pub fn hull_intersects_conic(&self, conic: &DConic, is_linear: &mut bool) -> bool {
        conic.hull_intersects_quad(self, is_linear)
    }

    pub fn hull_intersects_cubic(&self, cubic: &DCubic, is_linear: &mut bool) -> bool {
        cubic.hull_intersects_quad(self, is_linear)
    }

    /// The two points other than `odd_man`.
    pub fn other_pts(&self, odd_man: usize) -> [DPoint; 2] {
        let mut end_pt = [DPoint::default(); 2];
        for opp in 1..3i32 {
            let mut end = (odd_man as i32 ^ opp) - odd_man as i32;
            end &= !(end >> 2);
            end_pt[(opp - 1) as usize] = self.pts[end as usize];
        }
        end_pt
    }

    #[allow(non_snake_case)]
    pub fn AddValidTs(s: &[f64], real_roots: i32, t: &mut [f64]) -> i32 {
        let mut found_roots = 0;
        'next_root: for index in 0..real_roots as usize {
            let mut t_value = s[index];
            if approximately_zero_or_more(t_value) && approximately_one_or_less(t_value) {
                if approximately_less_than_zero(t_value) {
                    t_value = 0.0;
                } else if approximately_greater_than_one(t_value) {
                    t_value = 1.0;
                }
                for idx2 in 0..found_roots {
                    if approximately_equal(t[idx2], t_value) {
                        continue 'next_root;
                    }
                }
                t[found_roots] = t_value;
                found_roots += 1;
            }
        }
        found_roots as i32
    }

    /// Caller expects multiple results to be sorted smaller first.
    #[allow(non_snake_case)]
    pub fn RootsValidT(a: f64, b: f64, c: f64, t: &mut [f64]) -> i32 {
        let mut s = [0.0; 2];
        let real_roots = DQuad::RootsReal(a, b, c, &mut s);
        DQuad::AddValidTs(&s, real_roots, t)
    }

    /// Does not discard real roots <= 0 or >= 1.
    #[allow(non_snake_case)]
    pub fn RootsReal(a: f64, b: f64, c: f64, s: &mut [f64]) -> i32 {
        if a == 0.0 {
            return handle_zero(b, c, s);
        }
        let p = b / (2.0 * a);
        let q = c / a;
        if approximately_zero(a) && (approximately_zero_inverse(p) || approximately_zero_inverse(q)) {
            return handle_zero(b, c, s);
        }
        // normal form: x^2 + px + q = 0
        let p2 = p * p;
        if !almost_dequal_ulps_d(p2, q) && p2 < q {
            return 0;
        }
        let mut sqrt_d = 0.0;
        if p2 > q {
            sqrt_d = (p2 - q).sqrt();
        }
        s[0] = sqrt_d - p;
        s[1] = -sqrt_d - p;
        1 + !almost_dequal_ulps_d(s[0], s[1]) as i32
    }

    pub fn is_linear(&self, start_index: usize, end_index: usize) -> bool {
        let mut line_parameters = LineParameters::default();
        line_parameters.quad_end_points_se(self, start_index, end_index);
        // FIXME: maybe it's possible to avoid this and compare non-normalized
        line_parameters.normalize();
        let distance = line_parameters.control_pt_distance_quad(self);
        let p = &self.pts;
        let tiniest = smin(smin(smin(smin(smin(p[0].x, p[0].y), p[1].x), p[1].y), p[2].x), p[2].y);
        let mut largest = smax(smax(smax(smax(smax(p[0].x, p[0].y), p[1].x), p[1].y), p[2].x), p[2].y);
        largest = smax(largest, -tiniest);
        approximately_zero_when_compared_to(distance, largest)
    }

    pub fn dxdy_at_t(&self, t: f64) -> DVector {
        let a = t - 1.0;
        let b = 1.0 - 2.0 * t;
        let c = t;
        let mut result = DVector {
            x: a * self.pts[0].x + b * self.pts[1].x + c * self.pts[2].x,
            y: a * self.pts[0].y + b * self.pts[1].y + c * self.pts[2].y,
        };
        if result.x == 0.0 && result.y == 0.0 && zero_or_one(t) {
            result = self.pts[2] - self.pts[0];
        }
        result
    }

    pub fn pt_at_t(&self, t: f64) -> DPoint {
        if 0.0 == t {
            return self.pts[0];
        }
        if 1.0 == t {
            return self.pts[2];
        }
        let one_t = 1.0 - t;
        let a = one_t * one_t;
        let b = 2.0 * one_t * t;
        let c = t * t;
        DPoint {
            x: a * self.pts[0].x + b * self.pts[1].x + c * self.pts[2].x,
            y: a * self.pts[0].y + b * self.pts[1].y + c * self.pts[2].y,
        }
    }

    pub fn monotonic_in_x(&self) -> bool {
        between(self.pts[0].x, self.pts[1].x, self.pts[2].x)
    }

    pub fn monotonic_in_y(&self) -> bool {
        between(self.pts[0].y, self.pts[1].y, self.pts[2].y)
    }

    pub fn sub_divide(&self, t1: f64, t2: f64) -> DQuad {
        if 0.0 == t1 && 1.0 == t2 {
            return *self;
        }
        let mut dst = DQuad::default();
        let ax = interp_quad_coords(self.xs(), t1);
        let ay = interp_quad_coords(self.ys(), t1);
        dst[0] = DPoint::new(ax, ay);
        let dx = interp_quad_coords(self.xs(), (t1 + t2) / 2.0);
        let dy = interp_quad_coords(self.ys(), (t1 + t2) / 2.0);
        let cx = interp_quad_coords(self.xs(), t2);
        let cy = interp_quad_coords(self.ys(), t2);
        dst[2] = DPoint::new(cx, cy);
        dst[1].x = 2.0 * dx - (ax + cx) / 2.0;
        dst[1].y = 2.0 * dy - (ay + cy) / 2.0;
        dst
    }

    #[allow(non_snake_case)]
    pub fn SubDivide(a: &[Point], t1: f64, t2: f64) -> DQuad {
        DQuad::from_pts(a).sub_divide(t1, t2)
    }

    pub fn align(&self, end_index: usize, dst_pt: &mut DPoint) {
        if self.pts[end_index].x == self.pts[1].x {
            dst_pt.x = self.pts[end_index].x;
        }
        if self.pts[end_index].y == self.pts[1].y {
            dst_pt.y = self.pts[end_index].y;
        }
    }

    pub fn sub_divide_pt(&self, a: &DPoint, c: &DPoint, t1: f64, t2: f64) -> DPoint {
        let mut b;
        let sub = self.sub_divide(t1, t2);
        let b0 = DLine::new(*a, sub[1] + (*a - sub[0]));
        let b1 = DLine::new(*c, sub[1] + (*c - sub[2]));
        let mut i = Intersections::default();
        i.intersect_ray_line(&b0, &b1);
        if i.used() == 1 && i.t(0, 0) >= 0.0 && i.t(1, 0) >= 0.0 {
            b = i.pt(0);
        } else {
            return DPoint::Mid(&b0[1], &b1[1]);
        }
        if t1 == 0.0 || t2 == 0.0 {
            self.align(0, &mut b);
        }
        if t1 == 1.0 || t2 == 1.0 {
            self.align(2, &mut b);
        }
        if almost_bequal_ulps_d(b.x, a.x) {
            b.x = a.x;
        } else if almost_bequal_ulps_d(b.x, c.x) {
            b.x = c.x;
        }
        if almost_bequal_ulps_d(b.y, a.y) {
            b.y = a.y;
        } else if almost_bequal_ulps_d(b.y, c.y) {
            b.y = c.y;
        }
        b
    }

    #[allow(non_snake_case)]
    pub fn SubDividePt(pts: &[Point], a: &DPoint, c: &DPoint, t1: f64, t2: f64) -> DPoint {
        DQuad::from_pts(pts).sub_divide_pt(a, c, t1, t2)
    }

    pub fn chop_at(&self, t: f64) -> DQuadPair {
        let xs = interp_quad_coords_split(self.xs(), t);
        let ys = interp_quad_coords_split(self.ys(), t);
        let mut dst = DQuadPair::default();
        for i in 0..5 {
            dst.pts[i] = DPoint::new(xs[i], ys[i]);
        }
        dst
    }

    /// Quad'(t) = At + B; solve for t only if it fits between 0 < t < 1.
    #[allow(non_snake_case)]
    pub fn FindExtrema(src: [f64; 3], t_value: &mut [f64]) -> i32 {
        let a = src[0];
        let b = src[1];
        let c = src[2];
        valid_unit_divide(a - b, a - b - b + c, &mut t_value[0])
    }

    /// Parameterization form, given A*t*t + 2*B*t*(1-t) + C*(1-t)*(1-t).
    #[allow(non_snake_case)]
    pub fn SetABC(quad: [f64; 3]) -> (f64, f64, f64) {
        let mut a = quad[0]; // a = A
        let mut b = 2.0 * quad[1]; // b =     2*B
        let c = quad[2]; // c =             C
        b -= c; // b =     2*B -   C
        a -= b; // a = A - 2*B +   C
        b -= c; // b =     2*B - 2*C
        (a, b, c)
    }
}
