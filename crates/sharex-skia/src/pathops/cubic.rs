//! `SkPathOpsCubic.h`, `SkPathOpsCubic.cpp` and `SkOpCubicHull.cpp`:
//! `SkDCubic`.

use super::conic::DConic;
use super::geometry::{CubicType, classify_cubic};
use super::line_parameters::LineParameters;
use super::point::{DPoint, DVector};
use super::quad::DQuad;
use super::types::*;
use crate::geom::Point;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DCubic {
    pub pts: [DPoint; 4],
}

impl std::ops::Index<usize> for DCubic {
    type Output = DPoint;
    fn index(&self, n: usize) -> &DPoint {
        &self.pts[n]
    }
}

impl std::ops::IndexMut<usize> for DCubic {
    fn index_mut(&mut self, n: usize) -> &mut DPoint {
        &mut self.pts[n]
    }
}

/// `SkDCubicPair`: two cubics sharing pts[3].
#[derive(Clone, Copy, Debug, Default)]
pub struct DCubicPair {
    pub pts: [DPoint; 7],
}

impl DCubicPair {
    pub fn first(&self) -> DCubic {
        DCubic { pts: [self.pts[0], self.pts[1], self.pts[2], self.pts[3]] }
    }
    pub fn second(&self) -> DCubic {
        DCubic { pts: [self.pts[3], self.pts[4], self.pts[5], self.pts[6]] }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SearchAxis {
    XAxis,
    YAxis,
}

/// Given the set [0, 1, 2, 3] and two of its members, an XOR mask that
/// computes the other two.
#[inline]
pub fn other_two(one: i32, two: i32) -> i32 {
    (1 >> (3 - (one ^ two))) ^ 3
}

pub const CUBIC_POINT_COUNT: usize = 4;
pub const CUBIC_MAX_INTERSECTIONS: i32 = 9;
const PRECISION_UNIT: f64 = 256.0;
const PI: f64 = std::f64::consts::PI; // Skia's 3.141592653589793

/// The classic one t subdivision, over one coordinate.
fn interp_cubic_coords_split(src: [f64; 4], t: f64) -> [f64; 7] {
    let ab = sk_d_interp(src[0], src[1], t);
    let bc = sk_d_interp(src[1], src[2], t);
    let cd = sk_d_interp(src[2], src[3], t);
    let abc = sk_d_interp(ab, bc, t);
    let bcd = sk_d_interp(bc, cd, t);
    let abcd = sk_d_interp(abc, bcd, t);
    [src[0], ab, abc, abcd, bcd, cd, src[3]]
}

fn interp_cubic_coords(src: [f64; 4], t: f64) -> f64 {
    let ab = sk_d_interp(src[0], src[1], t);
    let bc = sk_d_interp(src[1], src[2], t);
    let cd = sk_d_interp(src[2], src[3], t);
    let abc = sk_d_interp(ab, bc, t);
    let bcd = sk_d_interp(bc, cd, t);
    sk_d_interp(abc, bcd, t)
}

// c'(t) = 3(b-a)(1-t)^2 + 6(c-b)t(1-t) + 3(d-c)t^2
fn derivative_at_t(src: [f64; 4], t: f64) -> f64 {
    let one_t = 1.0 - t;
    let a = src[0];
    let b = src[1];
    let c = src[2];
    let d = src[3];
    3.0 * ((b - a) * one_t * one_t + 2.0 * (c - b) * t * one_t + (d - c) * t * t)
}

fn formulate_f1_dot_f2(src: [f64; 4]) -> [f64; 4] {
    let a = src[1] - src[0];
    let b = src[2] - 2.0 * src[1] + src[0];
    let c = src[3] + 3.0 * (src[1] - src[2]) - src[0];
    [c * c, 3.0 * b * c, 2.0 * b * b + c * a, a * b]
}

// SkOpCubicHull.cpp
fn rotate(cubic: &DCubic, zero: usize, index: usize, rot_path: &mut DCubic) -> bool {
    let dy = cubic[index].y - cubic[zero].y;
    let dx = cubic[index].x - cubic[zero].x;
    if approximately_zero(dy) {
        if approximately_zero(dx) {
            return false;
        }
        *rot_path = *cubic;
        if dy != 0.0 {
            rot_path[index].y = cubic[zero].y;
            let mask = other_two(index as i32, zero as i32);
            let side1 = (index as i32 ^ mask) as usize;
            let side2 = (zero as i32 ^ mask) as usize;
            if approximately_equal(cubic[side1].y, cubic[zero].y) {
                rot_path[side1].y = cubic[zero].y;
            }
            if approximately_equal(cubic[side2].y, cubic[zero].y) {
                rot_path[side2].y = cubic[zero].y;
            }
        }
        return true;
    }
    for i in 0..4 {
        rot_path[i].x = cubic[i].x * dx + cubic[i].y * dy;
        rot_path[i].y = cubic[i].y * dx - cubic[i].x * dy;
    }
    true
}

// Returns 0 if negative, 1 if zero, 2 if positive
fn side(x: f64) -> i32 {
    (x > 0.0) as i32 + (x >= 0.0) as i32
}

impl DCubic {
    pub fn from_pts(pts: &[Point]) -> Self {
        DCubic {
            pts: [
                DPoint::from_pt(pts[0]),
                DPoint::from_pt(pts[1]),
                DPoint::from_pt(pts[2]),
                DPoint::from_pt(pts[3]),
            ],
        }
    }

    pub fn set(&mut self, pts: &[Point]) -> &Self {
        *self = DCubic::from_pts(pts);
        self
    }

    pub fn xs(&self) -> [f64; 4] {
        [self.pts[0].x, self.pts[1].x, self.pts[2].x, self.pts[3].x]
    }

    pub fn ys(&self) -> [f64; 4] {
        [self.pts[0].y, self.pts[1].y, self.pts[2].y, self.pts[3].y]
    }

    fn axis(p: &DPoint, axis: SearchAxis) -> f64 {
        match axis {
            SearchAxis::XAxis => p.x,
            SearchAxis::YAxis => p.y,
        }
    }

    /// The first three points as a quad (`(const SkDQuad*) this`).
    pub fn as_quad(&self) -> DQuad {
        DQuad { pts: [self.pts[0], self.pts[1], self.pts[2]] }
    }

    pub fn collapsed(&self) -> bool {
        self.pts[0].approximately_equal(&self.pts[1])
            && self.pts[0].approximately_equal(&self.pts[2])
            && self.pts[0].approximately_equal(&self.pts[3])
    }

    pub fn controls_inside(&self) -> bool {
        let v01 = self.pts[0] - self.pts[1];
        let v02 = self.pts[0] - self.pts[2];
        let v03 = self.pts[0] - self.pts[3];
        let v13 = self.pts[1] - self.pts[3];
        let v23 = self.pts[2] - self.pts[3];
        v03.dot(v01) > 0.0 && v03.dot(v02) > 0.0 && v03.dot(v13) > 0.0 && v03.dot(v23) > 0.0
    }

    pub fn align(&self, end_index: usize, ctrl_index: usize, dst_pt: &mut DPoint) {
        if self.pts[end_index].x == self.pts[ctrl_index].x {
            dst_pt.x = self.pts[end_index].x;
        }
        if self.pts[end_index].y == self.pts[ctrl_index].y {
            dst_pt.y = self.pts[end_index].y;
        }
    }

    /// Give up when changing t no longer moves the point.
    pub fn binary_search(&self, min: f64, max: f64, axis_intercept: f64, x_axis: SearchAxis) -> f64 {
        let mut t = (min + max) / 2.0;
        let mut step = (t - min) / 2.0;
        let mut cubic_at_t = self.pt_at_t(t);
        let mut calc_pos = Self::axis(&cubic_at_t, x_axis);
        let mut calc_dist = calc_pos - axis_intercept;
        loop {
            let prior_t = smax(min, t - step);
            let less_pt = self.pt_at_t(prior_t);
            if approximately_equal_half(less_pt.x, cubic_at_t.x)
                && approximately_equal_half(less_pt.y, cubic_at_t.y)
            {
                return -1.0; // binary search found no point at this axis intercept
            }
            let less_dist = Self::axis(&less_pt, x_axis) - axis_intercept;
            let last_step = step;
            step /= 2.0;
            let go_less = if calc_dist > 0.0 { calc_dist > less_dist } else { calc_dist < less_dist };
            if go_less {
                t = prior_t;
            } else {
                let next_t = t + last_step;
                if next_t > max {
                    return -1.0;
                }
                let more_pt = self.pt_at_t(next_t);
                if approximately_equal_half(more_pt.x, cubic_at_t.x)
                    && approximately_equal_half(more_pt.y, cubic_at_t.y)
                {
                    return -1.0;
                }
                let more_dist = Self::axis(&more_pt, x_axis) - axis_intercept;
                let skip = if calc_dist > 0.0 { calc_dist <= more_dist } else { calc_dist >= more_dist };
                if skip {
                    if approximately_equal(calc_pos, axis_intercept) {
                        break;
                    }
                    continue;
                }
                t = next_t;
            }
            let test_at_t = self.pt_at_t(t);
            cubic_at_t = test_at_t;
            calc_pos = Self::axis(&cubic_at_t, x_axis);
            calc_dist = calc_pos - axis_intercept;
            if approximately_equal(calc_pos, axis_intercept) {
                break;
            }
        }
        t
    }

    /// The rough scale of the cubic; used to determine if curvature is extreme.
    pub fn calc_precision(&self) -> f64 {
        ((self.pts[1] - self.pts[0]).length()
            + (self.pts[2] - self.pts[1]).length()
            + (self.pts[3] - self.pts[2]).length())
            / PRECISION_UNIT
    }

    pub fn chop_at(&self, t: f64) -> DCubicPair {
        let mut dst = DCubicPair::default();
        let p = &self.pts;
        if t == 0.5 {
            dst.pts[0] = p[0];
            dst.pts[1].x = (p[0].x + p[1].x) / 2.0;
            dst.pts[1].y = (p[0].y + p[1].y) / 2.0;
            dst.pts[2].x = (p[0].x + 2.0 * p[1].x + p[2].x) / 4.0;
            dst.pts[2].y = (p[0].y + 2.0 * p[1].y + p[2].y) / 4.0;
            dst.pts[3].x = (p[0].x + 3.0 * (p[1].x + p[2].x) + p[3].x) / 8.0;
            dst.pts[3].y = (p[0].y + 3.0 * (p[1].y + p[2].y) + p[3].y) / 8.0;
            dst.pts[4].x = (p[1].x + 2.0 * p[2].x + p[3].x) / 4.0;
            dst.pts[4].y = (p[1].y + 2.0 * p[2].y + p[3].y) / 4.0;
            dst.pts[5].x = (p[2].x + p[3].x) / 2.0;
            dst.pts[5].y = (p[2].y + p[3].y) / 2.0;
            dst.pts[6] = p[3];
            return dst;
        }
        let xs = interp_cubic_coords_split(self.xs(), t);
        let ys = interp_cubic_coords_split(self.ys(), t);
        for i in 0..7 {
            dst.pts[i] = DPoint::new(xs[i], ys[i]);
        }
        dst
    }

    #[allow(non_snake_case)]
    pub fn Coefficients(src: [f64; 4]) -> (f64, f64, f64, f64) {
        let mut a = src[3]; // d
        let mut b = src[2] * 3.0; // 3*c
        let mut c = src[1] * 3.0; // 3*b
        let d = src[0]; // a
        a -= d - c + b; // A =   -a + 3*b - 3*c + d
        b += 3.0 * d - 2.0 * c; // B =  3*a - 6*b + 3*c
        c -= 3.0 * d; // C = -3*a + 3*b
        (a, b, c, d)
    }

    pub fn ends_are_extrema_in_x_or_y(&self) -> bool {
        let p = &self.pts;
        (between(p[0].x, p[1].x, p[3].x) && between(p[0].x, p[2].x, p[3].x))
            || (between(p[0].y, p[1].y, p[3].y) && between(p[0].y, p[2].y, p[3].y))
    }

    /// Quick reject by rotating all points relative to a line formed by a
    /// pair of this cubic's points.
    pub fn hull_intersects_pts(&self, pts: &[DPoint], is_linear: &mut bool) -> bool {
        let mut linear = true;
        let mut hull_order = [0i32; 4];
        let hull_count = self.convex_hull(&mut hull_order);
        let mut end1 = hull_order[0];
        let mut hull_index = 0;
        let mut end_pt0 = self.pts[end1 as usize];
        loop {
            hull_index = (hull_index + 1) % hull_count;
            let end2 = hull_order[hull_index as usize];
            let end_pt1 = self.pts[end2 as usize];
            let orig_x = end_pt0.x;
            let orig_y = end_pt0.y;
            let adj = end_pt1.x - orig_x;
            let opp = end_pt1.y - orig_y;
            let odd_man_mask = other_two(end1, end2);
            let odd_man = (end1 ^ odd_man_mask) as usize;
            let mut sign = (self.pts[odd_man].y - orig_y) * adj - (self.pts[odd_man].x - orig_x) * opp;
            let odd_man2 = (end2 ^ odd_man_mask) as usize;
            let sign2 = (self.pts[odd_man2].y - orig_y) * adj - (self.pts[odd_man2].x - orig_x) * opp;
            let mut skip = false;
            if sign * sign2 < 0.0 {
                skip = true;
            } else if approximately_zero(sign) {
                sign = sign2;
                if approximately_zero(sign) {
                    skip = true;
                }
            }
            if !skip {
                linear = false;
                let mut found_outlier = false;
                for p in pts {
                    let test = (p.y - orig_y) * adj - (p.x - orig_x) * opp;
                    if test * sign > 0.0 && !precisely_zero(test) {
                        found_outlier = true;
                        break;
                    }
                }
                if !found_outlier {
                    return false;
                }
                end_pt0 = end_pt1;
                end1 = end2;
            }
            if hull_index == 0 {
                break;
            }
        }
        *is_linear = linear;
        true
    }

    pub fn hull_intersects_cubic(&self, c2: &DCubic, is_linear: &mut bool) -> bool {
        self.hull_intersects_pts(&c2.pts, is_linear)
    }

    pub fn hull_intersects_quad(&self, quad: &DQuad, is_linear: &mut bool) -> bool {
        self.hull_intersects_pts(&quad.pts, is_linear)
    }

    pub fn hull_intersects_conic(&self, conic: &DConic, is_linear: &mut bool) -> bool {
        self.hull_intersects_quad(&conic.pts, is_linear)
    }

    pub fn is_linear(&self, start_index: usize, end_index: usize) -> bool {
        if self.pts[0].approximately_d_equal(&self.pts[3]) {
            return self.as_quad().is_linear(0, 2);
        }
        let mut line_parameters = LineParameters::default();
        line_parameters.cubic_end_points_se(self, start_index, end_index);
        // FIXME: maybe it's possible to avoid this and compare non-normalized
        line_parameters.normalize();
        let p = &self.pts;
        let tiniest = smin(
            smin(smin(smin(smin(smin(smin(p[0].x, p[0].y), p[1].x), p[1].y), p[2].x), p[2].y), p[3].x),
            p[3].y,
        );
        let mut largest = smax(
            smax(smax(smax(smax(smax(smax(p[0].x, p[0].y), p[1].x), p[1].y), p[2].x), p[2].y), p[3].x),
            p[3].y,
        );
        largest = smax(largest, -tiniest);
        let mut distance = line_parameters.control_pt_distance_cubic(self, 1);
        if !approximately_zero_when_compared_to(distance, largest) {
            return false;
        }
        distance = line_parameters.control_pt_distance_cubic(self, 2);
        approximately_zero_when_compared_to(distance, largest)
    }

    /// `SkDCubic::ComplexBreak`: where to split complex cubics (such as
    /// self-intersecting curves or ones with difficult curvature).
    #[allow(non_snake_case)]
    pub fn ComplexBreak(points: &[Point], t: &mut [f32; 3]) -> i32 {
        let cubic = DCubic::from_pts(points);
        if cubic.monotonic_in_x() && cubic.monotonic_in_y() {
            return 0;
        }
        let mut tt = [0.0f64; 2];
        let mut ss = [0.0f64; 2];
        let cubic_type = classify_cubic(points, &mut tt, &mut ss);
        let mut fall_through = false;
        if cubic_type == CubicType::Loop {
            let (td, te, sd, se) = (tt[0], tt[1], ss[0], ss[1]);
            if roughly_between(0.0, td, sd) && roughly_between(0.0, te, se) {
                t[0] = ((td * se + te * sd) / (2.0 * sd * se)) as f32;
                return (t[0] > 0.0 && t[0] < 1.0) as i32;
            }
            fall_through = true;
        }
        if fall_through
            || matches!(
                cubic_type,
                CubicType::Serpentine | CubicType::LocalCusp | CubicType::CuspAtInfinity
            )
        {
            let mut inflection_ts = [0.0f64; 2];
            let inf_t_count = cubic.find_inflections(&mut inflection_ts);
            let mut max_curvature = [0.0f64; 3];
            let roots = cubic.find_max_curvature(&mut max_curvature);
            if inf_t_count == 2 {
                for index in 0..roots as usize {
                    if between(inflection_ts[0], max_curvature[index], inflection_ts[1]) {
                        t[0] = max_curvature[index] as f32;
                        return (t[0] > 0.0 && t[0] < 1.0) as i32;
                    }
                }
            } else {
                let mut result_count = 0usize;
                // FIXME: constant found through experimentation -- maybe there's a better way....
                let precision = cubic.calc_precision() * 2.0;
                for index in 0..roots as usize {
                    let test_t = max_curvature[index];
                    if 0.0 >= test_t || test_t >= 1.0 {
                        continue;
                    }
                    // don't call dxdyAtT since we want (0,0) results
                    let d_pt = DVector {
                        x: derivative_at_t(cubic.xs(), test_t),
                        y: derivative_at_t(cubic.ys(), test_t),
                    };
                    let d_pt_len = d_pt.length();
                    if d_pt_len < precision {
                        t[result_count] = test_t as f32;
                        result_count += 1;
                    }
                }
                if result_count == 0 && inf_t_count == 1 {
                    t[0] = inflection_ts[0] as f32;
                    result_count = (t[0] > 0.0 && t[0] < 1.0) as usize;
                }
                return result_count as i32;
            }
        }
        0
    }

    pub fn monotonic_in_x(&self) -> bool {
        let p = &self.pts;
        precisely_between(p[0].x, p[1].x, p[3].x) && precisely_between(p[0].x, p[2].x, p[3].x)
    }

    pub fn monotonic_in_y(&self) -> bool {
        let p = &self.pts;
        precisely_between(p[0].y, p[1].y, p[3].y) && precisely_between(p[0].y, p[2].y, p[3].y)
    }

    pub fn other_pts(&self, index: usize) -> [DPoint; 3] {
        let mut offset = (index == 0) as usize;
        let a = self.pts[offset];
        offset += 1;
        let b = self.pts[offset];
        offset += 1;
        let c = self.pts[offset];
        [a, b, c]
    }

    pub fn search_roots(
        &self,
        extreme_ts: &mut [f64; 6],
        mut extrema: usize,
        axis_intercept: f64,
        x_axis: SearchAxis,
        valid_roots: &mut [f64],
    ) -> i32 {
        let mut infl = [0.0; 2];
        let n = self.find_inflections(&mut infl) as usize;
        extreme_ts[extrema..extrema + n].copy_from_slice(&infl[..n]);
        extrema += n;
        extreme_ts[extrema] = 0.0;
        extrema += 1;
        extreme_ts[extrema] = 1.0;
        super::sort::tq_sort_f64(&mut extreme_ts[..extrema + 1]);
        let mut valid_count = 0;
        let mut index = 0;
        while index < extrema {
            let min = extreme_ts[index];
            index += 1;
            let max = extreme_ts[index];
            if min == max {
                continue;
            }
            let new_t = self.binary_search(min, max, axis_intercept, x_axis);
            if new_t >= 0.0 {
                if valid_count >= 3 {
                    return 0;
                }
                valid_roots[valid_count] = new_t;
                valid_count += 1;
            }
        }
        valid_count as i32
    }

    #[allow(non_snake_case)]
    pub fn RootsValidT(a: f64, b: f64, c: f64, d: f64, t: &mut [f64]) -> i32 {
        let mut s = [0.0; 3];
        let real_roots = DCubic::RootsReal(a, b, c, d, &mut s);
        let mut found_roots = DQuad::AddValidTs(&s, real_roots, t) as usize;
        'next_root: for index in 0..real_roots as usize {
            let t_value = s[index];
            if !approximately_one_or_less(t_value) && between(1.0, t_value, 1.00005) {
                for idx2 in 0..found_roots {
                    if approximately_equal(t[idx2], 1.0) {
                        continue 'next_root;
                    }
                }
                t[found_roots] = 1.0;
                found_roots += 1;
            } else if !approximately_zero_or_more(t_value) && between(-0.00005, t_value, 0.0) {
                for idx2 in 0..found_roots {
                    if approximately_equal(t[idx2], 0.0) {
                        continue 'next_root;
                    }
                }
                t[found_roots] = 0.0;
                found_roots += 1;
            }
        }
        found_roots as i32
    }

    #[allow(non_snake_case)]
    pub fn RootsReal(mut A: f64, B: f64, C: f64, D: f64, s: &mut [f64]) -> i32 {
        if approximately_zero(A)
            && approximately_zero_when_compared_to(A, B)
            && approximately_zero_when_compared_to(A, C)
            && approximately_zero_when_compared_to(A, D)
        {
            // we're just a quadratic
            return DQuad::RootsReal(B, C, D, s);
        }
        if approximately_zero_when_compared_to(D, A)
            && approximately_zero_when_compared_to(D, B)
            && approximately_zero_when_compared_to(D, C)
        {
            // 0 is one root
            let mut num = DQuad::RootsReal(A, B, C, s) as usize;
            for i in 0..num {
                if approximately_zero(s[i]) {
                    return num as i32;
                }
            }
            s[num] = 0.0;
            num += 1;
            return num as i32;
        }
        if approximately_zero(A + B + C + D) {
            // 1 is one root
            let mut num = DQuad::RootsReal(A, A + B, -D, s) as usize;
            for i in 0..num {
                if almost_dequal_ulps_d(s[i], 1.0) {
                    return num as i32;
                }
            }
            s[num] = 1.0;
            num += 1;
            return num as i32;
        }
        let (a, b, c);
        {
            let inv_a = 1.0 / A;
            a = B * inv_a;
            b = C * inv_a;
            c = D * inv_a;
        }
        let a2 = a * a;
        let q = (a2 - b * 3.0) / 9.0;
        let r = (2.0 * a2 * a - 9.0 * a * b + 27.0 * c) / 54.0;
        let r2 = r * r;
        let q3 = q * q * q;
        let r2_minus_q3 = r2 - q3;
        let adiv3 = a / 3.0;
        let mut n = 0usize;
        if r2_minus_q3 < 0.0 {
            // we have 3 real roots
            // the divide/root can, due to finite precisions, be slightly outside of -1...1
            let pinned = {
                let v = r / q3.sqrt();
                // SkTPin(x, lo, hi) = max(lo, min(x, hi))
                smax(-1.0, smin(v, 1.0))
            };
            let theta = pinned.acos();
            let neg2_root_q = -2.0 * q.sqrt();

            let mut rr = neg2_root_q * (theta / 3.0).cos() - adiv3;
            s[n] = rr;
            n += 1;

            rr = neg2_root_q * ((theta + 2.0 * PI) / 3.0).cos() - adiv3;
            if !almost_dequal_ulps_d(s[0], rr) {
                s[n] = rr;
                n += 1;
            }
            rr = neg2_root_q * ((theta - 2.0 * PI) / 3.0).cos() - adiv3;
            if !almost_dequal_ulps_d(s[0], rr) && (n == 1 || !almost_dequal_ulps_d(s[1], rr)) {
                s[n] = rr;
                n += 1;
            }
        } else {
            // we have 1 real root
            let sqrt_r2_minus_q3 = r2_minus_q3.sqrt();
            A = r.abs() + sqrt_r2_minus_q3;
            A = A.cbrt(); // cube root
            if r > 0.0 {
                A = -A;
            }
            if A != 0.0 {
                A += q / A;
            }
            let mut rr = A - adiv3;
            s[n] = rr;
            n += 1;
            if almost_dequal_ulps_d(r2, q3) {
                rr = -A / 2.0 - adiv3;
                if !almost_dequal_ulps_d(s[0], rr) {
                    s[n] = rr;
                    n += 1;
                }
            }
        }
        n as i32
    }

    pub fn dxdy_at_t(&self, t: f64) -> DVector {
        let mut result = DVector { x: derivative_at_t(self.xs(), t), y: derivative_at_t(self.ys(), t) };
        if result.x == 0.0 && result.y == 0.0 {
            if t == 0.0 {
                result = self.pts[2] - self.pts[0];
            } else if t == 1.0 {
                result = self.pts[3] - self.pts[1];
            }
            if result.x == 0.0 && result.y == 0.0 && zero_or_one(t) {
                result = self.pts[3] - self.pts[0];
            }
        }
        result
    }

    pub fn find_inflections(&self, t_values: &mut [f64]) -> i32 {
        let p = &self.pts;
        let ax = p[1].x - p[0].x;
        let ay = p[1].y - p[0].y;
        let bx = p[2].x - 2.0 * p[1].x + p[0].x;
        let by = p[2].y - 2.0 * p[1].y + p[0].y;
        let cx = p[3].x + 3.0 * (p[1].x - p[2].x) - p[0].x;
        let cy = p[3].y + 3.0 * (p[1].y - p[2].y) - p[0].y;
        DQuad::RootsValidT(bx * cy - by * cx, ax * cy - ay * cx, ax * by - ay * bx, t_values)
    }

    /// SkDCubic'(t) = At^2 + Bt + C; solve for t, keeping 0 < t < 1.
    #[allow(non_snake_case)]
    pub fn FindExtrema(src: [f64; 4], t_values: &mut [f64]) -> i32 {
        // we divide A,B,C by 3 to simplify
        let a = src[0];
        let b = src[1];
        let c = src[2];
        let d = src[3];
        let A = d - a + 3.0 * (b - c);
        let B = 2.0 * (a - b - b + c);
        let C = b - a;
        DQuad::RootsValidT(A, B, C, t_values)
    }

    /// Looking for F' dot F'' == 0.
    pub fn find_max_curvature(&self, t_values: &mut [f64]) -> i32 {
        let mut coeff_x = formulate_f1_dot_f2(self.xs());
        let coeff_y = formulate_f1_dot_f2(self.ys());
        for i in 0..4 {
            coeff_x[i] += coeff_y[i];
        }
        DCubic::RootsValidT(coeff_x[0], coeff_x[1], coeff_x[2], coeff_x[3], t_values)
    }

    pub fn pt_at_t(&self, t: f64) -> DPoint {
        if 0.0 == t {
            return self.pts[0];
        }
        if 1.0 == t {
            return self.pts[3];
        }
        let one_t = 1.0 - t;
        let one_t2 = one_t * one_t;
        let a = one_t2 * one_t;
        let b = 3.0 * one_t2 * t;
        let t2 = t * t;
        let c = 3.0 * one_t * t2;
        let d = t2 * t;
        let p = &self.pts;
        DPoint {
            x: a * p[0].x + b * p[1].x + c * p[2].x + d * p[3].x,
            y: a * p[0].y + b * p[1].y + c * p[2].y + d * p[3].y,
        }
    }

    pub fn sub_divide(&self, t1: f64, t2: f64) -> DCubic {
        if t1 == 0.0 || t2 == 1.0 {
            if t1 == 0.0 && t2 == 1.0 {
                return *self;
            }
            let pair = self.chop_at(if t1 == 0.0 { t2 } else { t1 });
            return if t1 == 0.0 { pair.first() } else { pair.second() };
        }
        let mut dst = DCubic::default();
        let xs = self.xs();
        let ys = self.ys();
        let ax = interp_cubic_coords(xs, t1);
        let ay = interp_cubic_coords(ys, t1);
        dst[0] = DPoint::new(ax, ay);
        let ex = interp_cubic_coords(xs, (t1 * 2.0 + t2) / 3.0);
        let ey = interp_cubic_coords(ys, (t1 * 2.0 + t2) / 3.0);
        let fx = interp_cubic_coords(xs, (t1 + t2 * 2.0) / 3.0);
        let fy = interp_cubic_coords(ys, (t1 + t2 * 2.0) / 3.0);
        let dx = interp_cubic_coords(xs, t2);
        let dy = interp_cubic_coords(ys, t2);
        dst[3] = DPoint::new(dx, dy);
        let mx = ex * 27.0 - ax * 8.0 - dx;
        let my = ey * 27.0 - ay * 8.0 - dy;
        let nx = fx * 27.0 - ax - dx * 8.0;
        let ny = fy * 27.0 - ay - dy * 8.0;
        dst[1].x = (mx * 2.0 - nx) / 18.0;
        dst[1].y = (my * 2.0 - ny) / 18.0;
        dst[2].x = (nx * 2.0 - mx) / 18.0;
        dst[2].y = (ny * 2.0 - my) / 18.0;
        dst
    }

    #[allow(non_snake_case)]
    pub fn SubDivide(a: &[Point], t1: f64, t2: f64) -> DCubic {
        DCubic::from_pts(a).sub_divide(t1, t2)
    }

    pub fn sub_divide_pts(&self, a: &DPoint, d: &DPoint, t1: f64, t2: f64) -> [DPoint; 2] {
        // this approach assumes that the control points computed directly are accurate enough
        let sub = self.sub_divide(t1, t2);
        let mut dst = [sub[1] + (*a - sub[0]), sub[2] + (*d - sub[3])];
        if t1 == 0.0 || t2 == 0.0 {
            let idx = if t1 == 0.0 { 0 } else { 1 };
            self.align(0, 1, &mut dst[idx]);
        }
        if t1 == 1.0 || t2 == 1.0 {
            let idx = if t1 == 1.0 { 0 } else { 1 };
            self.align(3, 2, &mut dst[idx]);
        }
        if almost_bequal_ulps_d(dst[0].x, a.x) {
            dst[0].x = a.x;
        }
        if almost_bequal_ulps_d(dst[0].y, a.y) {
            dst[0].y = a.y;
        }
        if almost_bequal_ulps_d(dst[1].x, d.x) {
            dst[1].x = d.x;
        }
        if almost_bequal_ulps_d(dst[1].y, d.y) {
            dst[1].y = d.y;
        }
        dst
    }

    #[allow(non_snake_case)]
    pub fn SubDividePts(pts: &[Point], a: &DPoint, d: &DPoint, t1: f64, t2: f64) -> [DPoint; 2] {
        DCubic::from_pts(pts).sub_divide_pts(a, d, t1, t2)
    }

    pub fn to_float_points(&self, pts: &mut [Point]) -> bool {
        let mut all_finite = true;
        for i in 0..4 {
            let mut x = self.pts[i].x as f32;
            if (x.abs() as f64) < FLT_EPSILON_ORDERABLE_ERR {
                x = 0.0;
            }
            let mut y = self.pts[i].y as f32;
            if (y.abs() as f64) < FLT_EPSILON_ORDERABLE_ERR {
                y = 0.0;
            }
            pts[i] = Point::new(x, y);
            all_finite &= x.is_finite() && y.is_finite();
        }
        all_finite
    }

    pub fn top(&self, d_curve: &DCubic, start_t: f64, end_t: f64, top_pt: &mut DPoint) -> f64 {
        let mut extreme_ts = [0.0; 2];
        let mut top_t = -1.0;
        let roots = DCubic::FindExtrema(self.ys(), &mut extreme_ts);
        for index in 0..roots as usize {
            let t = start_t + (end_t - start_t) * extreme_ts[index];
            let mid = d_curve.pt_at_t(t);
            if top_pt.y > mid.y || (top_pt.y == mid.y && top_pt.x > mid.x) {
                top_t = t;
                *top_pt = mid;
            }
        }
        top_t
    }

    /// `SkDCubic::convexHull`: the indices of the end and control points in
    /// hull order; 3 or 4.
    pub fn convex_hull(&self, order: &mut [i32; 4]) -> i32 {
        // find top point
        let mut y_min: usize = 0;
        for index in 1..4usize {
            if self.pts[y_min].y > self.pts[index].y
                || (self.pts[y_min].y == self.pts[index].y && self.pts[y_min].x > self.pts[index].x)
            {
                y_min = index;
            }
        }
        order[0] = y_min as i32;
        let mut mid_x: i32 = -1;
        let mut backup_y_min: i32 = -1;
        for _pass in 0..2 {
            for index in 0..4usize {
                if index == y_min {
                    continue;
                }
                // rotate line from (yMin, index) to axis
                // see if remaining two points are both above or below
                // use this to find mid
                let mask = other_two(y_min as i32, index as i32);
                let side1 = (y_min as i32 ^ mask) as usize;
                let side2 = (index as i32 ^ mask) as usize;
                let mut rot_path = DCubic::default();
                if !rotate(self, y_min, index, &mut rot_path) {
                    // ! if cbc[yMin]==cbc[idx]
                    order[1] = side1 as i32;
                    order[2] = side2 as i32;
                    return 3;
                }
                let mut sides = side(rot_path[side1].y - rot_path[y_min].y);
                sides ^= side(rot_path[side2].y - rot_path[y_min].y);
                if sides == 2 {
                    // '2' means one remaining point <0, one >0
                    if mid_x >= 0 {
                        // one of the control points is equal to an end point
                        order[0] = 0;
                        order[1] = 3;
                        let p = &self.pts;
                        if p[1] == p[0] || p[1] == p[3] {
                            order[2] = 2;
                            return 3;
                        }
                        if p[2] == p[0] || p[2] == p[3] {
                            order[2] = 1;
                            return 3;
                        }
                        // one of the control points may be very nearly but not exactly equal --
                        let dist1_0 = p[1].distance_squared(&p[0]);
                        let dist1_3 = p[1].distance_squared(&p[3]);
                        let dist2_0 = p[2].distance_squared(&p[0]);
                        let dist2_3 = p[2].distance_squared(&p[3]);
                        let smallest1_dist_sq = smin(dist1_0, dist1_3);
                        let smallest2_dist_sq = smin(dist2_0, dist2_3);
                        if approximately_zero(smin(smallest1_dist_sq, smallest2_dist_sq)) {
                            order[2] = if smallest1_dist_sq < smallest2_dist_sq { 2 } else { 1 };
                            return 3;
                        }
                    }
                    mid_x = index as i32;
                } else if sides == 0 {
                    // '0' means both to one side or the other
                    backup_y_min = index as i32;
                }
            }
            if mid_x >= 0 {
                break;
            }
            if backup_y_min < 0 {
                break;
            }
            y_min = backup_y_min as usize;
            backup_y_min = -1;
        }
        if mid_x < 0 {
            mid_x = y_min as i32 ^ 3; // choose any other point
        }
        let mask = other_two(y_min as i32, mid_x);
        let least = (y_min as i32 ^ mask) as usize;
        let most = (mid_x ^ mask) as usize;
        order[0] = y_min as i32;
        order[1] = least as i32;

        // see if mid value is on same side of line (least, most) as yMin
        let mut mid_path = DCubic::default();
        if !rotate(self, least, most, &mut mid_path) {
            // ! if cbc[least]==cbc[most]
            order[2] = mid_x;
            return 3;
        }
        let mut mid_sides = side(mid_path[y_min].y - mid_path[least].y);
        mid_sides ^= side(mid_path[mid_x as usize].y - mid_path[least].y);
        if mid_sides != 2 {
            // if mid point is not between
            order[2] = most as i32;
            return 3; // result is a triangle
        }
        order[2] = mid_x;
        order[3] = most as i32;
        4 // result is a quadralateral
    }
}
