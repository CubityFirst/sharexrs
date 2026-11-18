//! `SkOpAngle.h`, `SkOpAngle.cpp`: the curves leaving a point, sorted
//! counterclockwise. The smallest angle has a positive x and the smallest
//! positive y; the largest has a positive x and a zero y.

use super::cubic::DCubic;
use super::curve::*;
use super::graph::*;
use super::intersections::Intersections;
use super::line::DLine;
use super::line_parameters::LineParameters;
use super::point::{DPoint, DVector};
use super::types::*;
use crate::path::Verb;

/// `SkOpAngle::IncludeType`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum IncludeType {
    UnaryWinding,
    UnaryXor,
    BinarySingle,
    BinaryOpp,
}

static SEDECIMANT: [[[i32; 3]; 3]; 3] = [
    //       y<0           y==0           y>0
    //   x<0 x==0 x>0  x<0 x==0 x>0  x<0 x==0 x>0
    [[4, 3, 2], [7, -1, 15], [10, 11, 12]], // abs(x) <  abs(y)
    [[5, -1, 1], [-1, -1, -1], [9, -1, 13]], // abs(x) == abs(y)
    [[6, 3, 0], [7, -1, 15], [8, 11, 14]],  // abs(x) >  abs(y)
];

impl Angle {
    /// Return true if lh < this < rh.
    pub fn after(self, test: Angle) -> bool {
        let lh = test;
        let rh = lh.d().next;
        let s = self.d();
        s.part.curve = s.original_curve_part;
        // Adjust lh and rh to share the same origin (floating point error in intersections can
        // mean they aren't exactly the same).
        lh.d().part.curve = lh.d().original_curve_part;
        lh.d().part.curve[0] = self.d().part.curve[0];
        rh.d().part.curve = rh.d().original_curve_part;
        rh.d().part.curve[0] = self.d().part.curve[0];

        if lh.d().compute_sector && !lh.compute_sector() {
            return true;
        }
        if self.d().compute_sector && !self.compute_sector() {
            return true;
        }
        if rh.d().compute_sector && !rh.compute_sector() {
            return true;
        }
        let (l, t, r) = (lh.d(), self.d(), rh.d());
        let ltr_overlap = ((l.sector_mask | r.sector_mask) & t.sector_mask) != 0;
        let lr_overlap = (l.sector_mask & r.sector_mask) != 0;
        let lr_order;
        if !lr_overlap {
            // no lh/rh sector overlap
            if !ltr_overlap {
                // no lh/this/rh sector overlap
                return (l.sector_end > r.sector_start) ^ (t.sector_start > l.sector_end) ^ (t.sector_start > r.sector_start);
            }
            let lr_gap = (r.sector_start as i32 - l.sector_start as i32 + 32) & 0x1f;
            // A tiny change can move the start +/- 4. The order can only be determined if
            // lr gap is not 12 to 20 or -12 to -20.
            lr_order = if lr_gap > 20 {
                0
            } else if lr_gap > 11 {
                -1
            } else {
                1
            };
        } else {
            lr_order = lh.orderable(rh);
            if !ltr_overlap && lr_order >= 0 {
                return lr_order == 0;
            }
        }
        let mut lt_order;
        if (lh.d().sector_mask & self.d().sector_mask) != 0 {
            lt_order = lh.orderable(self);
        } else {
            let lt_gap = (self.d().sector_start as i32 - lh.d().sector_start as i32 + 32) & 0x1f;
            lt_order = if lt_gap > 20 {
                0
            } else if lt_gap > 11 {
                -1
            } else {
                1
            };
        }
        let mut tr_order;
        if (rh.d().sector_mask & self.d().sector_mask) != 0 {
            tr_order = self.orderable(rh);
        } else {
            let tr_gap = (rh.d().sector_start as i32 - self.d().sector_start as i32 + 32) & 0x1f;
            tr_order = if tr_gap > 20 {
                0
            } else if tr_gap > 11 {
                -1
            } else {
                1
            };
        }
        self.alignment_same_side(lh, &mut lt_order);
        self.alignment_same_side(rh, &mut tr_order);
        if lr_order >= 0 && lt_order >= 0 && tr_order >= 0 {
            return (if lr_order != 0 { lt_order & tr_order } else { lt_order | tr_order }) != 0;
        }
        // There's not enough information to sort. Get the pairs of angles in opposite planes.
        // If an order is < 0, the pair is already in an opposite plane. Check the remaining pairs.
        if lt_order == 0 && lr_order == 0 {
            let lt_opposite = lh.opposite_planes(self);
            return lt_opposite;
        } else if lt_order == 1 && tr_order == 0 {
            let tr_opposite = self.opposite_planes(rh);
            return tr_opposite;
        } else if lr_order == 1 && tr_order == 1 {
            let lr_opposite = lh.opposite_planes(rh);
            return lr_opposite;
        }
        // If a pair couldn't be ordered, there's not enough information to determine the sort.
        if (self.d().unorderable || lh.d().unorderable || rh.d().unorderable)
            && !self.d().part.is_curve()
            && !lh.d().part.is_curve()
            && !rh.d().part.is_curve()
        {
            // limit to lines; should work with curves, but wait for a failing test to verify
            // see if original raw data is orderable
            // if two share a point, check if third has both points in same half plane
            let lt_share = (lh.d().original_curve_part[0] == self.d().original_curve_part[0]) as i32;
            let lr_share = (lh.d().original_curve_part[0] == rh.d().original_curve_part[0]) as i32;
            let tr_share = (self.d().original_curve_part[0] == rh.d().original_curve_part[0]) as i32;
            // if only one pair are the same, the third point touches neither of the pair
            if lt_share + lr_share + tr_share == 1 {
                if lr_share != 0 {
                    let lt_o_order = lh.lines_on_original_side(self);
                    let rt_o_order = rh.lines_on_original_side(self);
                    if (rt_o_order ^ lt_o_order) == 1 {
                        return lt_o_order != 0;
                    }
                } else if tr_share != 0 {
                    let tl_o_order = self.lines_on_original_side(lh);
                    let rl_o_order = rh.lines_on_original_side(lh);
                    if (tl_o_order ^ rl_o_order) == 1 {
                        return rl_o_order != 0;
                    }
                } else {
                    let tr_o_order = rh.lines_on_original_side(self);
                    let lr_o_order = lh.lines_on_original_side(rh);
                    // result must be 0 and 1 or 1 and 0 to be valid
                    if (lr_o_order ^ tr_o_order) == 1 {
                        return tr_o_order != 0;
                    }
                }
            }
        }
        if lr_order < 0 {
            if lt_order < 0 {
                return tr_order != 0;
            }
            return lt_order != 0;
        }
        lr_order == 0
    }

    fn line_on_one_side_impl(self, origin: &DPoint, line: &DVector, test: Angle, use_original: bool) -> i32 {
        let mut crosses = [0.0f64; 3];
        let test_verb = test.segment().verb();
        let i_max = verb_to_points(test_verb);
        let test_curve = if use_original { test.d().original_curve_part } else { test.d().part.curve };
        for index in 1..=i_max {
            let xy1 = line.x * (test_curve[index].y - origin.y);
            let xy2 = line.y * (test_curve[index].x - origin.x);
            crosses[index - 1] = if almost_bequal_ulps_d(xy1, xy2) { 0.0 } else { xy1 - xy2 };
        }
        if crosses[0] * crosses[1] < 0.0 {
            return -1;
        }
        if Verb::Cubic == test_verb && (crosses[0] * crosses[2] < 0.0 || crosses[1] * crosses[2] < 0.0) {
            return -1;
        }
        if crosses[0] != 0.0 {
            return (crosses[0] < 0.0) as i32;
        }
        if crosses[1] != 0.0 {
            return (crosses[1] < 0.0) as i32;
        }
        if Verb::Cubic == test_verb && crosses[2] != 0.0 {
            return (crosses[2] < 0.0) as i32;
        }
        -2
    }

    /// Given a line, see if the opposite curve's convex hull is all on one
    /// side: -1 = not on one side, 0 = this CW of test, 1 = this CCW of test.
    fn line_on_one_side(self, test: Angle, use_original: bool) -> i32 {
        let origin = self.d().part.curve[0];
        let line = self.d().part.curve[1] - origin;
        let mut result = self.line_on_one_side_impl(&origin, &line, test, use_original);
        if -2 == result {
            self.d().unorderable = true;
            result = -1;
        }
        result
    }

    /// Experiment works only with lines for now.
    fn lines_on_original_side(self, test: Angle) -> i32 {
        let origin = self.d().original_curve_part[0];
        let line = self.d().original_curve_part[1] - origin;
        let mut dots = [0.0f64; 2];
        let mut crosses = [0.0f64; 2];
        let test_curve = test.d().original_curve_part;
        for index in 0..2 {
            let test_line = test_curve[index] - origin;
            let xy1 = line.x * test_line.y;
            let xy2 = line.y * test_line.x;
            dots[index] = line.x * test_line.x + line.y * test_line.y;
            crosses[index] = if almost_bequal_ulps_d(xy1, xy2) { 0.0 } else { xy1 - xy2 };
        }
        if crosses[0] * crosses[1] < 0.0 {
            return -1;
        }
        if crosses[0] != 0.0 {
            return (crosses[0] < 0.0) as i32;
        }
        if crosses[1] != 0.0 {
            return (crosses[1] < 0.0) as i32;
        }
        if (dots[0] == 0.0 && dots[1] < 0.0) || (dots[0] < 0.0 && dots[1] == 0.0) {
            return 2; // 180 degrees apart
        }
        self.d().unorderable = true;
        -1
    }

    /// To sort the angles, all curves are translated to have the same
    /// starting point. If the curve's control point in its original position
    /// is on one side of a compared line, and translated is on the opposite
    /// side, reverse the previously computed order.
    fn alignment_same_side(self, test: Angle, order: &mut i32) {
        if *order < 0 {
            return;
        }
        if self.d().part.is_curve() {
            // This should support all curve types, but only bug that requires this has lines
            // Turning on for curves causes existing tests to fail
            return;
        }
        if test.d().part.is_curve() {
            return;
        }
        let x_origin = test.d().part.curve[0];
        let o_origin = test.d().original_curve_part[0];
        if x_origin == o_origin {
            return;
        }
        let i_max = verb_to_points(self.segment().verb());
        let x_line = test.d().part.curve[1] - x_origin;
        let o_line = test.d().original_curve_part[1] - o_origin;
        for index in 1..=i_max {
            let test_pt = self.d().part.curve[index];
            let x_cross = o_line.cross_check(test_pt - x_origin);
            let o_cross = x_line.cross_check(test_pt - o_origin);
            if o_cross * x_cross < 0.0 {
                *order ^= 1;
                break;
            }
        }
    }

    fn check_crosses_zero(self) -> bool {
        let d = self.d();
        let start = (d.sector_start as i32).min(d.sector_end as i32);
        let end = (d.sector_start as i32).max(d.sector_end as i32);
        end - start > 16
    }

    fn check_parallel(self, rh: Angle) -> bool {
        let sweep = if self.d().part.is_ordered() {
            self.d().part.sweep[0]
        } else {
            self.d().part.curve[1] - self.d().part.curve[0]
        };
        let tweep = if rh.d().part.is_ordered() {
            rh.d().part.sweep[0]
        } else {
            rh.d().part.curve[1] - rh.d().part.curve[0]
        };
        let s0xt0 = sweep.cross_check(tweep);
        if self.tangents_diverge(rh, s0xt0) {
            return s0xt0 < 0.0;
        }
        // compute the perpendicular to the endpoints and see where it intersects the opposite curve
        // if the intersections within the t range, do a cross check on those
        let mut inside = false;
        if !self.d().end.contains_span(rh.d().end) {
            if self.end_to_side(rh, &mut inside) {
                return inside;
            }
            if rh.end_to_side(self, &mut inside) {
                return !inside;
            }
        }
        if self.mid_to_side(rh, &mut inside) {
            return inside;
        }
        if rh.mid_to_side(self, &mut inside) {
            return !inside;
        }
        // compute the cross check from the mid T values (last resort)
        let m0 = self.segment().d_pt_at_t(self.mid_t()) - self.d().part.curve[0];
        let m1 = rh.segment().d_pt_at_t(rh.mid_t()) - rh.d().part.curve[0];
        let m0xm1 = m0.cross_check(m1);
        if m0xm1 == 0.0 {
            self.d().unorderable = true;
            rh.d().unorderable = true;
            return true;
        }
        m0xm1 < 0.0
    }

    /// The original angle is too short to get meaningful sector information;
    /// lengthen it until it is long enough to be meaningful or leave it unset
    /// if lengthening it would cause it to intersect one of the adjacent angles.
    fn compute_sector(self) -> bool {
        if self.d().computed_sector {
            return !self.d().unorderable;
        }
        self.d().computed_sector = true;
        let f_start = self.d().start;
        let f_end = self.d().end;
        let step_up = f_start.t() < f_end.t();
        let mut check_end = f_end;
        if check_end.is_final() && step_up {
            self.d().unorderable = true;
            return false;
        }
        let segment = self.segment();
        'recompute_sector: loop {
            // advance end
            let other = check_end.segment();
            let mut o_span = other.head();
            loop {
                if o_span.segment() == segment
                    && o_span != check_end
                    && approximately_equal(o_span.t(), check_end.t())
                {
                    break 'recompute_sector;
                }
                if o_span.is_final() {
                    break;
                }
                o_span = o_span.up_cast().next();
                if o_span.is_null() {
                    break;
                }
            }
            check_end = if step_up {
                if !check_end.is_final() { check_end.up_cast().next() } else { Span::NULL }
            } else {
                check_end.prev()
            };
            if check_end.is_null() {
                break;
            }
        }
        // recomputeSector:
        let computed_end = if step_up {
            if check_end.is_some() { check_end.prev() } else { f_end.segment().head() }
        } else if check_end.is_some() {
            check_end.up_cast().next()
        } else {
            f_end.segment().tail()
        };
        if check_end == f_end || computed_end == f_end || computed_end == f_start {
            self.d().unorderable = true;
            return false;
        }
        if step_up != (f_start.t() < computed_end.t()) {
            self.d().unorderable = true;
            return false;
        }
        let save_end = f_end;
        self.d().computed_end = computed_end;
        self.d().end = computed_end;
        self.set_spans();
        self.set_sector();
        self.d().end = save_end;
        !self.d().unorderable
    }

    fn convex_hull_overlaps(self, rh: Angle) -> i32 {
        let sweep = self.d().part.sweep;
        let tweep = rh.d().part.sweep;
        let s0xs1 = sweep[0].cross_check(sweep[1]);
        let s0xt0 = sweep[0].cross_check(tweep[0]);
        let s1xt0 = sweep[1].cross_check(tweep[0]);
        let mut t_between_s = if s0xs1 > 0.0 { s0xt0 > 0.0 && s1xt0 < 0.0 } else { s0xt0 < 0.0 && s1xt0 > 0.0 };
        let s0xt1 = sweep[0].cross_check(tweep[1]);
        let s1xt1 = sweep[1].cross_check(tweep[1]);
        t_between_s |= if s0xs1 > 0.0 { s0xt1 > 0.0 && s1xt1 < 0.0 } else { s0xt1 < 0.0 && s1xt1 > 0.0 };
        let t0xt1 = tweep[0].cross_check(tweep[1]);
        if t_between_s {
            return -1;
        }
        if (s0xt0 == 0.0 && s1xt1 == 0.0) || (s1xt0 == 0.0 && s0xt1 == 0.0) {
            // s0 to s1 equals t0 to t1
            return -1;
        }
        let mut s_between_t = if t0xt1 > 0.0 { s0xt0 < 0.0 && s0xt1 > 0.0 } else { s0xt0 > 0.0 && s0xt1 < 0.0 };
        s_between_t |= if t0xt1 > 0.0 { s1xt0 < 0.0 && s1xt1 > 0.0 } else { s1xt0 > 0.0 && s1xt1 < 0.0 };
        if s_between_t {
            return -1;
        }
        // if all of the sweeps are in the same half plane, then the order of any pair is enough
        if s0xt0 >= 0.0 && s0xt1 >= 0.0 && s1xt0 >= 0.0 && s1xt1 >= 0.0 {
            return 0;
        }
        if s0xt0 <= 0.0 && s0xt1 <= 0.0 && s1xt0 <= 0.0 && s1xt1 <= 0.0 {
            return 1;
        }
        // if the outside sweeps are greater than 180 degress:
        // first assume the inital tangents are the ordering
        // if the midpoint direction matches the inital order, that is enough
        let m0 = self.segment().d_pt_at_t(self.mid_t()) - self.d().part.curve[0];
        let m1 = rh.segment().d_pt_at_t(rh.mid_t()) - rh.d().part.curve[0];
        let m0xm1 = m0.cross_check(m1);
        if s0xt0 > 0.0 && m0xm1 > 0.0 {
            return 0;
        }
        if s0xt0 < 0.0 && m0xm1 < 0.0 {
            return 1;
        }
        if self.tangents_diverge(rh, s0xt0) {
            return (s0xt0 < 0.0) as i32;
        }
        (m0xm1 < 0.0) as i32
    }

    /// OPTIMIZATION: longest can all be either lazily computed here or
    /// precomputed in setup.
    pub fn dist_end_ratio(self, dist: f64) -> f64 {
        let mut longest = 0.0;
        let segment = self.segment();
        let pt_count = verb_to_points(segment.verb());
        let pts = segment.pts();
        for idx1 in 0..pt_count {
            for idx2 in idx1 + 1..=pt_count {
                // SkDVector::set(pts[idx2] - pts[idx1]): the difference in float
                let v = DVector::new((pts[idx2].x - pts[idx1].x) as f64, (pts[idx2].y - pts[idx1].y) as f64);
                let len_sq = v.length_squared();
                longest = smax(longest, len_sq);
            }
        }
        longest.sqrt() / dist
    }

    fn ends_intersect(self, rh: Angle) -> bool {
        let l_verb = self.segment().verb();
        let r_verb = rh.segment().verb();
        let l_pts = verb_to_points(l_verb);
        let r_pts = verb_to_points(r_verb);
        let rays = [
            DLine::new(self.d().part.curve[0], rh.d().part.curve[r_pts]),
            DLine::new(self.d().part.curve[0], self.d().part.curve[l_pts]),
        ];
        if self.d().end.contains_span(rh.d().end) {
            return self.check_parallel(rh);
        }
        let mut small_ts = [-1.0f64, -1.0];
        let mut limited = [false, false];
        for index in 0..2 {
            let c_verb = if index != 0 { r_verb } else { l_verb };
            // if the curve is a line, then the line and the ray intersect only at their crossing
            if c_verb == Verb::Line {
                continue;
            }
            let segment = if index != 0 { rh.segment() } else { self.segment() };
            let mut i = Intersections::default();
            curve_intersect_ray(c_verb, segment.pts(), segment.weight(), &rays[index], &mut i);
            let t_start = if index != 0 { rh.d().start.t() } else { self.d().start.t() };
            let t_end = if index != 0 { rh.d().computed_end.t() } else { self.d().computed_end.t() };
            let test_ascends = t_start < (if index != 0 { rh.d().computed_end.t() } else { self.d().computed_end.t() });
            let mut t = if test_ascends { 0.0 } else { 1.0 };
            for idx2 in 0..i.used() as usize {
                let test_t = i.t(0, idx2);
                if !approximately_between_orderable(t_start, test_t, t_end) {
                    continue;
                }
                if approximately_equal_orderable(t_start, test_t) {
                    continue;
                }
                t = if test_ascends { smax(t, test_t) } else { smin(t, test_t) };
                small_ts[index] = t;
                limited[index] = approximately_equal_orderable(t, t_end);
            }
        }
        let mut s_ray_longer = false;
        let mut s_cept = DVector::new(0.0, 0.0);
        let mut s_cept_t = -1.0;
        let mut s_index: i32 = -1;
        let mut use_intersect = false;
        for index in 0..2usize {
            if small_ts[index] < 0.0 {
                continue;
            }
            let segment = if index != 0 { rh.segment() } else { self.segment() };
            let d_pt = segment.d_pt_at_t(small_ts[index]);
            let cept = d_pt - rays[index][0];
            // If this point is on the curve, it should have been detected earlier by ordinary
            // curve intersection. This may be hard to determine in general, but for lines,
            // the point could be close to or equal to its end, but shouldn't be near the start.
            if (if index != 0 { l_pts } else { r_pts }) == 1 {
                let total = rays[index][1] - rays[index][0];
                if cept.length_squared() * 2.0 < total.length_squared() {
                    continue;
                }
            }
            let end = rays[index][1] - rays[index][0];
            if cept.x * end.x < 0.0 || cept.y * end.y < 0.0 {
                continue;
            }
            let ray_dist = cept.length();
            let end_dist = end.length();
            let ray_longer = ray_dist > end_dist;
            if limited[0] && limited[1] && ray_longer {
                use_intersect = true;
                s_ray_longer = ray_longer;
                s_cept = cept;
                s_cept_t = small_ts[index];
                s_index = index as i32;
                break;
            }
            let mut delta = (ray_dist - end_dist).abs();
            let mut min_x = f32::INFINITY as f64;
            let mut min_y = min_x;
            let mut max_x = -(f32::INFINITY as f64);
            let mut max_y = max_x;
            let curve = if index != 0 { rh.d().part.curve } else { self.d().part.curve };
            let pt_count = if index != 0 { r_pts } else { l_pts };
            for idx2 in 0..=pt_count {
                min_x = smin(min_x, curve[idx2].x);
                min_y = smin(min_y, curve[idx2].y);
                max_x = smax(max_x, curve[idx2].x);
                max_y = smax(max_y, curve[idx2].y);
            }
            let max_width = smax(max_x - min_x, max_y - min_y);
            delta /= max_width;
            // FIXME: move these magic numbers
            // This fixes skbug.com/8380
            if delta < 4e-3
                && delta > 1e-3
                && !use_intersect
                && self.d().part.is_curve()
                && rh.d().part.is_curve()
                && self.d().original_curve_part[0] != self.d().part.curve[0]
            {
                // see if original curve is on one side of hull; translated is on the other
                let origin = rh.d().original_curve_part[0];
                let count = verb_to_points(rh.segment().verb());
                let line = rh.d().original_curve_part[count] - origin;
                let original_side = rh.line_on_one_side_impl(&origin, &line, self, true);
                if original_side >= 0 {
                    let translated_side = rh.line_on_one_side_impl(&origin, &line, self, false);
                    if original_side != translated_side {
                        continue;
                    }
                }
            }
            if delta > 1e-3 && {
                use_intersect ^= true;
                use_intersect
            } {
                s_ray_longer = ray_longer;
                s_cept = cept;
                s_cept_t = small_ts[index];
                s_index = index as i32;
            }
        }
        if use_intersect {
            let curve = if s_index != 0 { rh.d().part.curve } else { self.d().part.curve };
            let segment = if s_index != 0 { rh.segment() } else { self.segment() };
            let t_start = if s_index != 0 { rh.d().start.t() } else { self.d().start.t() };
            let mid = segment.d_pt_at_t(t_start + (s_cept_t - t_start) / 2.0) - curve[0];
            let sept_dir = mid.cross_check(s_cept);
            if sept_dir == 0.0 {
                return self.check_parallel(rh);
            }
            s_ray_longer ^ (s_index == 0) ^ (sept_dir < 0.0)
        } else {
            self.check_parallel(rh)
        }
    }

    fn end_to_side(self, rh: Angle, inside: &mut bool) -> bool {
        let segment = self.segment();
        let verb = segment.verb();
        let mut ray_end = DLine::default();
        ray_end[0].set(self.d().end.pt());
        ray_end[1] = ray_end[0];
        let slope_at_end = curve_d_slope_at_t(verb, segment.pts(), segment.weight(), self.d().end.t());
        ray_end[1].x += slope_at_end.y;
        ray_end[1].y -= slope_at_end.x;
        let mut i_end = Intersections::default();
        let opp_segment = rh.segment();
        let opp_verb = opp_segment.verb();
        curve_intersect_ray(opp_verb, opp_segment.pts(), opp_segment.weight(), &ray_end, &mut i_end);
        let mut end_dist = 0.0;
        let closest_end = i_end.closest_to(rh.d().start.t(), rh.d().end.t(), &ray_end[0], &mut end_dist);
        if closest_end < 0 {
            return false;
        }
        if end_dist == 0.0 {
            return false;
        }
        let mut start = DPoint::default();
        start.set(self.d().start.pt());
        // OPTIMIZATION: multiple times in the code we find the max scalar
        let mut min_x = f32::INFINITY as f64;
        let mut min_y = min_x;
        let mut max_x = -(f32::INFINITY as f64);
        let mut max_y = max_x;
        let curve = rh.d().part.curve;
        let opp_pts = verb_to_points(opp_verb);
        for idx2 in 0..=opp_pts {
            min_x = smin(min_x, curve[idx2].x);
            min_y = smin(min_y, curve[idx2].y);
            max_x = smax(max_x, curve[idx2].x);
            max_y = smax(max_y, curve[idx2].y);
        }
        let max_width = smax(max_x - min_x, max_y - min_y);
        end_dist /= max_width;
        if !(end_dist >= 5e-12) {
            // empirically found
            return false; // ! above catches NaN
        }
        let end_pt = ray_end[0];
        let opp_pt = i_end.pt(closest_end as usize);
        let v_left = end_pt - start;
        let v_right = opp_pt - start;
        let dir = v_left.cross_no_normal_check(v_right);
        if dir == 0.0 {
            return false;
        }
        *inside = dir < 0.0;
        true
    }

    fn find_sector(self, verb: Verb, x: f64, y: f64) -> i32 {
        let abs_x = x.abs();
        let abs_y = y.abs();
        let xy = if Verb::Line == verb || !almost_equal_ulps_d(abs_x, abs_y) { abs_x - abs_y } else { 0.0 };
        let i = (xy >= 0.0) as usize + (xy > 0.0) as usize;
        let j = (y >= 0.0) as usize + (y > 0.0) as usize;
        let k = (x >= 0.0) as usize + (x > 0.0) as usize;
        SEDECIMANT[i][j][k] * 2 + 1
    }

    pub fn global_state(self) -> &'static mut GlobalState {
        self.segment().global_state()
    }

    pub fn insert(self, angle: Angle) -> bool {
        if angle.d().next.is_some() {
            if self.loop_count() >= angle.loop_count() {
                if !self.merge(angle) {
                    return true;
                }
            } else if self.d().next.is_some() {
                if !angle.merge(self) {
                    return true;
                }
            } else {
                angle.insert(self);
            }
            return true;
        }
        let singleton = self.d().next.is_null();
        if singleton {
            self.d().next = self;
        }
        let mut next = self.d().next;
        if next.d().next == self {
            if singleton || angle.after(self) {
                self.d().next = angle;
                angle.d().next = next;
            } else {
                next.d().next = angle;
                angle.d().next = self;
            }
            return true;
        }
        let mut last = self;
        let mut flip_ambiguity = false;
        loop {
            if angle.after(last) ^ (angle.tangents_ambiguous() & flip_ambiguity) {
                last.d().next = angle;
                angle.d().next = next;
                break;
            }
            last = next;
            if last == self {
                if flip_ambiguity {
                    return false;
                }
                // We're in a loop. If a sort was ambiguous, flip it to end the loop.
                flip_ambiguity = true;
            }
            next = next.d().next;
        }
        true
    }

    pub fn last_marked(self) -> Span {
        let last_marked = self.d().last_marked;
        if last_marked.is_some() {
            if last_marked.chased() {
                return Span::NULL;
            }
            last_marked.set_chased(true);
        }
        last_marked
    }

    pub fn loop_contains(self, angle: Angle) -> bool {
        if self.d().next.is_null() {
            return false;
        }
        let first = self;
        let mut loop_ = self;
        let t_segment = angle.d().start.segment();
        let t_start = angle.d().start.t();
        let t_end = angle.d().end.t();
        loop {
            let l_segment = loop_.d().start.segment();
            if l_segment == t_segment {
                let l_start = loop_.d().start.t();
                if l_start == t_end {
                    let l_end = loop_.d().end.t();
                    if l_end == t_start {
                        return true;
                    }
                }
            }
            loop_ = loop_.d().next;
            if loop_ == first {
                break;
            }
        }
        false
    }

    pub fn loop_count(self) -> i32 {
        let mut count = 0;
        let first = self;
        let mut next = self;
        loop {
            next = next.d().next;
            count += 1;
            if !(next.is_some() && next != first) {
                break;
            }
        }
        count
    }

    fn merge(self, angle: Angle) -> bool {
        let mut working = angle;
        loop {
            if self == working {
                return false;
            }
            working = working.d().next;
            if working == angle {
                break;
            }
        }
        loop {
            let next = working.d().next;
            working.d().next = Angle::NULL;
            self.insert(working);
            working = next;
            if working == angle {
                break;
            }
        }
        // it's likely that a pair of the angles are unorderable
        true
    }

    fn mid_t(self) -> f64 {
        (self.d().start.t() + self.d().end.t()) / 2.0
    }

    fn mid_to_side(self, rh: Angle, inside: &mut bool) -> bool {
        let segment = self.segment();
        let verb = segment.verb();
        let start_pt = self.d().start.pt();
        let end_pt = self.d().end.pt();
        let mut d_start_pt = DPoint::default();
        d_start_pt.set(start_pt);
        let mut ray_mid = DLine::default();
        // computed in float: (startPt.fX + endPt.fX) / 2
        ray_mid[0].x = ((start_pt.x + end_pt.x) / 2.0) as f64;
        ray_mid[0].y = ((start_pt.y + end_pt.y) / 2.0) as f64;
        ray_mid[1].x = ray_mid[0].x + (end_pt.y - start_pt.y) as f64;
        ray_mid[1].y = ray_mid[0].y - (end_pt.x - start_pt.x) as f64;
        let mut i_mid = Intersections::default();
        curve_intersect_ray(verb, segment.pts(), segment.weight(), &ray_mid, &mut i_mid);
        let i_outside = i_mid.most_outside(self.d().start.t(), self.d().end.t(), &d_start_pt);
        if i_outside < 0 {
            return false;
        }
        let opp_segment = rh.segment();
        let opp_verb = opp_segment.verb();
        let mut opp_mid = Intersections::default();
        curve_intersect_ray(opp_verb, opp_segment.pts(), opp_segment.weight(), &ray_mid, &mut opp_mid);
        let opp_outside = opp_mid.most_outside(rh.d().start.t(), rh.d().end.t(), &d_start_pt);
        if opp_outside < 0 {
            return false;
        }
        let i_side = i_mid.pt(i_outside as usize) - d_start_pt;
        let opp_side = opp_mid.pt(opp_outside as usize) - d_start_pt;
        let dir = i_side.cross_check(opp_side);
        if dir == 0.0 {
            return false;
        }
        *inside = dir < 0.0;
        true
    }

    fn opposite_planes(self, rh: Angle) -> bool {
        let start_span = (rh.d().sector_start as i32 - self.d().sector_start as i32).abs();
        start_span >= 8
    }

    /// false == this < rh ; true == this > rh; -1 == unorderable.
    fn orderable(self, rh: Angle) -> i32 {
        let result;
        'unorderable: {
            if !self.d().part.is_curve() {
                if !rh.d().part.is_curve() {
                    let left_x = self.d().tangent_half.dx();
                    let left_y = self.d().tangent_half.dy();
                    let right_x = rh.d().tangent_half.dx();
                    let right_y = rh.d().tangent_half.dy();
                    let x_ry = left_x * right_y;
                    let rx_y = right_x * left_y;
                    if x_ry == rx_y {
                        if left_x * right_x < 0.0 || left_y * right_y < 0.0 {
                            return 1; // exactly 180 degrees apart
                        }
                        break 'unorderable;
                    }
                    return if x_ry < rx_y { 1 } else { 0 };
                }
                result = self.line_on_one_side(rh, false);
                if result >= 0 {
                    return result;
                }
                if self.d().unorderable || approximately_zero(rh.d().side) {
                    break 'unorderable;
                }
            } else if !rh.d().part.is_curve() {
                result = rh.line_on_one_side(self, false);
                if result >= 0 {
                    return if result != 0 { 0 } else { 1 };
                }
                if rh.d().unorderable || approximately_zero(self.d().side) {
                    break 'unorderable;
                }
            } else {
                result = self.convex_hull_overlaps(rh);
                if result >= 0 {
                    return result;
                }
            }
            return if self.ends_intersect(rh) { 1 } else { 0 };
        }
        // unorderable:
        self.d().unorderable = true;
        rh.d().unorderable = true;
        -1
    }

    /// OPTIMIZE: if this shows up in a profile, add a previous pointer; as
    /// is, this should be rarely called.
    pub fn previous(self) -> Angle {
        let mut last = self.d().next;
        loop {
            let next = last.d().next;
            if next == self {
                return last;
            }
            last = next;
        }
    }

    #[inline]
    pub fn segment(self) -> Segment {
        self.d().start.segment()
    }

    pub fn set(self, start: Span, end: Span) {
        {
            let d = self.d();
            d.start = start;
            d.computed_end = end;
            d.end = end;
            d.next = Angle::NULL;
            d.compute_sector = false;
            d.computed_sector = false;
            d.check_coincidence = false;
            d.tangents_ambiguous = false;
        }
        self.set_spans();
        self.set_sector();
    }

    fn set_spans(self) {
        self.d().unorderable = false;
        self.d().last_marked = Span::NULL;
        let f_start = self.d().start;
        if f_start.is_null() {
            self.d().unorderable = true;
            return;
        }
        let f_end = self.d().end;
        let segment = f_start.segment();
        let pts = *segment.pts();
        {
            let d = self.d();
            segment.sub_divide(f_start, f_end, &mut d.part.curve); // set at least the line part if not more
        }
        self.d().original_curve_part = self.d().part.curve;
        let verb = segment.verb();
        self.d().part.set_curve_hull_sweep(verb);
        if Verb::Line != verb && !self.d().part.is_curve() {
            let d = self.d();
            d.part.curve[1] = d.part.curve[verb_to_points(verb)];
            d.original_curve_part[1] = d.part.curve[1];
            let mut line_half = DLine::default();
            line_half[0].set(d.part.curve[0].as_sk_point());
            line_half[1].set(d.part.curve[1].as_sk_point());
            d.tangent_half.line_end_points(&line_half);
            d.side = 0.0;
        }
        match verb {
            Verb::Line => {
                let c_p1 = pts[(f_start.t() < f_end.t()) as usize];
                let mut line_half = DLine::default();
                line_half[0].set(f_start.pt());
                line_half[1].set(c_p1);
                let d = self.d();
                d.tangent_half.line_end_points(&line_half);
                d.side = 0.0;
            }
            Verb::Quad | Verb::Conic => {
                let d = self.d();
                let mut tangent_part = LineParameters::default();
                let _ = tangent_part.quad_end_points(&d.part.curve.quad());
                d.side = -tangent_part.point_distance(&d.part.curve[2]); // not normalized -- compare sign only
            }
            Verb::Cubic => {
                let d = self.d();
                let mut tangent_part = LineParameters::default();
                let _ = tangent_part.cubic_part(&d.part.curve.cubic());
                d.side = -tangent_part.point_distance(&d.part.curve[3]);
                let mut test_ts = [0.0f64; 4];
                // OPTIMIZATION: keep inflections precomputed with cubic segment?
                let mut test_count = DCubic::from_pts(&pts).find_inflections(&mut test_ts) as usize;
                let start_t = f_start.t();
                let end_t = f_end.t();
                let limit_t = end_t;
                for t in test_ts.iter_mut().take(test_count) {
                    if !between(start_t, *t, limit_t) {
                        *t = -1.0;
                    }
                }
                test_ts[test_count] = start_t;
                test_count += 1;
                test_ts[test_count] = end_t;
                test_count += 1;
                super::sort::tq_sort_f64(&mut test_ts[..test_count]);
                let mut best_side = 0.0f64;
                let test_cases = (test_count << 1) - 1;
                let mut index = 0usize;
                while test_ts[index] < 0.0 {
                    index += 1;
                }
                index <<= 1;
                while index < test_cases {
                    let test_index = index >> 1;
                    let mut test_t = test_ts[test_index];
                    if index & 1 != 0 {
                        test_t = (test_t + test_ts[test_index + 1]) / 2.0;
                    }
                    // OPTIMIZE: could avoid call for t == startT, endT
                    let pt = DCubic::from_pts(&pts).pt_at_t(test_t);
                    let mut test_part = LineParameters::default();
                    test_part.cubic_end_points(&d.part.curve.cubic());
                    let test_side = test_part.point_distance(&pt);
                    if best_side.abs() < test_side.abs() {
                        best_side = test_side;
                    }
                    index += 1;
                }
                d.side = -best_side; // compare sign only
            }
            _ => {}
        }
    }

    fn set_sector(self) {
        let f_start = self.d().start;
        if f_start.is_null() {
            self.d().unorderable = true;
            return;
        }
        let segment = f_start.segment();
        let verb = segment.verb();
        let sweep = self.d().part.sweep;
        let sector_start = self.find_sector(verb, sweep[0].x, sweep[0].y);
        self.d().sector_start = sector_start as i8;
        'defer_til_later: {
            if sector_start < 0 {
                break 'defer_til_later;
            }
            if !self.d().part.is_curve() {
                // if it's a line or line-like, note that both sectors are the same
                let d = self.d();
                d.sector_end = d.sector_start;
                d.sector_mask = 1 << d.sector_start;
                return;
            }
            let sector_end = self.find_sector(verb, sweep[1].x, sweep[1].y);
            self.d().sector_end = sector_end as i8;
            if sector_end < 0 {
                break 'defer_til_later;
            }
            {
                let d = self.d();
                if d.sector_end == d.sector_start && (d.sector_start & 3) != 3 {
                    // if the sector has no span, it can't be an exact angle
                    d.sector_mask = 1 << d.sector_start;
                    return;
                }
            }
            let mut crosses_zero = self.check_crosses_zero();
            let d = self.d();
            let mut start = (d.sector_start as i32).min(d.sector_end as i32);
            let curve_bends_ccw = (d.sector_start as i32 == start) ^ crosses_zero;
            // bump the start and end of the sector span if they are on exact compass points
            if (d.sector_start & 3) == 3 {
                d.sector_start = ((d.sector_start as i32 + if curve_bends_ccw { 1 } else { 31 }) & 0x1f) as i8;
            }
            if (d.sector_end & 3) == 3 {
                d.sector_end = ((d.sector_end as i32 + if curve_bends_ccw { 31 } else { 1 }) & 0x1f) as i8;
            }
            crosses_zero = self.check_crosses_zero();
            let d = self.d();
            start = (d.sector_start as i32).min(d.sector_end as i32);
            let end = (d.sector_start as i32).max(d.sector_end as i32);
            if !crosses_zero {
                d.sector_mask = ((u32::MAX >> (31 - end + start)) << start) as i32;
            } else {
                d.sector_mask = ((u32::MAX >> (31 - start)) | (u32::MAX << end)) as i32;
            }
            return;
        }
        // deferTilLater:
        let d = self.d();
        d.sector_start = -1;
        d.sector_end = -1;
        d.sector_mask = 0;
        d.compute_sector = true; // can't determine sector until segment length can be found
    }

    pub fn starter(self) -> Span {
        self.d().start.starter(self.d().end)
    }

    #[inline]
    pub fn next(self) -> Angle {
        self.d().next
    }

    #[inline]
    pub fn start(self) -> Span {
        self.d().start
    }

    #[inline]
    pub fn end(self) -> Span {
        self.d().end
    }

    pub fn set_last_marked(self, marked: Span) {
        self.d().last_marked = marked;
    }

    pub fn tangents_ambiguous(self) -> bool {
        self.d().tangents_ambiguous
    }

    #[inline]
    pub fn unorderable(self) -> bool {
        self.d().unorderable
    }

    fn tangents_diverge(self, rh: Angle, s0xt0: f64) -> bool {
        if s0xt0 == 0.0 {
            return false;
        }
        // if the ctrl tangents are not nearly parallel, use them
        // m = v1.cross(v2) / v1.dot(v2)
        let sweep = self.d().part.sweep;
        let tweep = rh.d().part.sweep;
        let s0dt0 = sweep[0].dot(tweep[0]);
        if s0dt0 == 0.0 {
            return true;
        }
        let m = s0xt0 / s0dt0;
        let s_dist = sweep[0].length() * m;
        let t_dist = tweep[0].length() * m;
        let use_s = s_dist.abs() < t_dist.abs();
        let m_factor = (if use_s { self.dist_end_ratio(s_dist) } else { rh.dist_end_ratio(t_dist) }).abs();
        self.d().tangents_ambiguous = (50.0..200.0).contains(&m_factor);
        m_factor < 50.0 // empirically found limit
    }
}
