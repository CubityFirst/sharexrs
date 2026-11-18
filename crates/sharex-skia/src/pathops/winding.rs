//! `SkPathOpsWinding.cpp`: a span's initial winding, found by casting a ray
//! from it and summing the windings of the edges it crosses.

use super::curve::*;
use super::graph::*;
use super::point::{DPoint, DVector};
use super::rect::PathOpsBounds;
use super::types::*;
use crate::geom::Point;
use crate::path::Verb;

/// `SkOpRayDir`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RayDir {
    Left,
    Top,
    Right,
    Bottom,
}

impl RayDir {
    fn from_i32(v: i32) -> RayDir {
        match v {
            0 => RayDir::Left,
            1 => RayDir::Top,
            2 => RayDir::Right,
            _ => RayDir::Bottom,
        }
    }
}

fn xy_index(dir: RayDir) -> usize {
    (dir as usize) & 1
}

fn pt_xy(pt: Point, dir: RayDir) -> f32 {
    if xy_index(dir) == 0 { pt.x } else { pt.y }
}

fn pt_yx(pt: Point, dir: RayDir) -> f32 {
    if xy_index(dir) == 0 { pt.y } else { pt.x }
}

fn pt_dxdy(v: &DVector, dir: RayDir) -> f64 {
    if xy_index(dir) == 0 { v.x } else { v.y }
}

fn pt_dydx(v: &DVector, dir: RayDir) -> f64 {
    if xy_index(dir) == 0 { v.y } else { v.x }
}

fn rect_side(r: &PathOpsBounds, dir: RayDir) -> f32 {
    match dir {
        RayDir::Left => r.left,
        RayDir::Top => r.top,
        RayDir::Right => r.right,
        RayDir::Bottom => r.bottom,
    }
}

fn sideways_overlap(rect: &PathOpsBounds, pt: Point, dir: RayDir) -> bool {
    if xy_index(dir) == 0 {
        // i = 1: the y range
        approximately_between(rect.top as f64, pt.y as f64, rect.bottom as f64)
    } else {
        approximately_between(rect.left as f64, pt.x as f64, rect.right as f64)
    }
}

fn less_than(dir: RayDir) -> bool {
    (dir as i32 & 2) == 0
}

fn ccw_dxdy(v: &DVector, dir: RayDir) -> bool {
    let v_part_pos = pt_dydx(v, dir) > 0.0;
    let left_bottom = ((dir as i32 + 1) & 2) != 0;
    v_part_pos == left_bottom
}

/// `SkOpRayHit`.
#[derive(Clone, Copy)]
pub struct RayHit {
    next: *mut RayHit,
    span: Span,
    pt: Point,
    t: f64,
    slope: DVector,
    valid: bool,
}

impl Default for RayHit {
    fn default() -> Self {
        RayHit {
            next: std::ptr::null_mut(),
            span: Span::NULL,
            pt: Point::default(),
            t: 0.0,
            slope: DVector::default(),
            valid: false,
        }
    }
}

impl RayHit {
    fn make_test_base(&mut self, span: Span, t: f64) -> RayDir {
        self.next = std::ptr::null_mut();
        self.span = span;
        self.t = span.t() * (1.0 - t) + span.next().t() * t;
        let segment = span.segment();
        self.slope = segment.d_slope_at_t(self.t);
        self.pt = segment.pt_at_t(self.t);
        self.valid = true;
        if self.slope.x.abs() < self.slope.y.abs() { RayDir::Left } else { RayDir::Top }
    }
}

/// The ray hits, allocated for one `sortableTop` (its `SkSTArenaAlloc`).
type HitArena = Vec<Box<RayHit>>;

impl Contour {
    pub fn ray_check(self, base: &RayHit, dir: RayDir, hits: &mut *mut RayHit, allocator: &mut HitArena) {
        // if the bounds extreme is outside the best, we're done
        let base_xy = pt_xy(base.pt, dir);
        let bounds_xy = rect_side(&self.d().bounds, dir);
        let check_less_than = less_than(dir);
        if !approximately_equal(base_xy as f64, bounds_xy as f64) && (base_xy < bounds_xy) == check_less_than {
            return;
        }
        let mut test_segment = self.first();
        loop {
            test_segment.ray_check(base, dir, hits, allocator);
            test_segment = test_segment.next();
            if test_segment.is_null() {
                break;
            }
        }
    }
}

impl Segment {
    pub fn ray_check(self, base: &RayHit, dir: RayDir, hits: &mut *mut RayHit, allocator: &mut HitArena) {
        let bounds = self.bounds();
        if !sideways_overlap(&bounds, base.pt, dir) {
            return;
        }
        let base_xy = pt_xy(base.pt, dir);
        let bounds_xy = rect_side(&bounds, dir);
        let check_less_than = less_than(dir);
        if !approximately_equal(base_xy as f64, bounds_xy as f64) && (base_xy < bounds_xy) == check_less_than {
            return;
        }
        let mut t_vals = [0.0f64; 3];
        let base_yx = pt_yx(base.pt, dir);
        let verb = self.verb();
        let pts = *self.pts();
        let roots = curve_intercept(verb, xy_index(dir) == 1, &pts, self.weight(), base_yx, &mut t_vals);
        for &t in t_vals.iter().take(roots.max(0) as usize) {
            if base.span.segment() == self && approximately_equal(base.t, t) {
                continue;
            }
            let mut slope = DVector::default();
            let pt;
            let mut valid = false;
            if approximately_zero(t) {
                pt = pts[0];
            } else if approximately_equal(t, 1.0) {
                pt = pts[verb_to_points(verb)];
            } else {
                pt = self.pt_at_t(t);
                if DPoint::approximately_equal_sk(pt, base.pt) {
                    if base.span.segment() == self {
                        continue;
                    }
                } else {
                    let pt_xy_v = pt_xy(pt, dir);
                    if !approximately_equal(base_xy as f64, pt_xy_v as f64) && (base_xy < pt_xy_v) == check_less_than {
                        continue;
                    }
                    slope = self.d_slope_at_t(t);
                    if verb == Verb::Cubic
                        && base.span.segment() == self
                        && roughly_equal(base.t, t)
                        && DPoint::roughly_equal_sk(pt, base.pt)
                    {
                        continue;
                    }
                    if (pt_dydx(&slope, dir) * 10000.0).abs() > pt_dxdy(&slope, dir).abs() {
                        valid = true;
                    }
                }
            }
            let span = self.winding_span_at_t(t);
            if span.is_null() {
                valid = false;
            } else if span.wind_value() == 0 && span.opp_value() == 0 {
                continue;
            }
            let mut new_hit = Box::new(RayHit { next: *hits, pt, slope, span, t, valid });
            let p: *mut RayHit = &mut *new_hit;
            allocator.push(new_hit);
            *hits = p;
        }
    }

    pub fn winding_span_at_t(self, t_hit: f64) -> Span {
        let mut span = self.head();
        loop {
            let next = span.next();
            if approximately_equal(t_hit, next.t()) {
                return Span::NULL;
            }
            if t_hit < next.t() {
                return span;
            }
            if next.is_final() {
                break;
            }
            span = next.up_cast();
            if span.is_null() {
                break;
            }
        }
        Span::NULL
    }

    pub fn find_sortable_top(self, contour_head: Contour) -> Span {
        let mut span = self.head();
        loop {
            let next = span.next();
            if !span.done() {
                if span.wind_sum() != SK_MIN_S32 {
                    return span;
                }
                if span.sortable_top(contour_head) {
                    return span;
                }
            }
            if next.is_final() {
                break;
            }
            span = next.up_cast();
            if span.is_null() {
                break;
            }
        }
        Span::NULL
    }
}

fn get_t_guess(mut t_try: i32, dir_offset: &mut i32) -> f64 {
    let mut t = 0.5;
    *dir_offset = t_try & 1;
    let t_base = t_try >> 1;
    let mut t_bits = 0;
    loop {
        t_try >>= 1;
        if t_try == 0 {
            break;
        }
        t /= 2.0;
        t_bits += 1;
    }
    if t_bits != 0 {
        let t_index = (t_base - 1) & ((1 << t_bits) - 1);
        t += t * 2.0 * t_index as f64;
    }
    t
}

impl Span {
    pub fn sortable_top(self, contour_head: Contour) -> bool {
        let mut allocator: HitArena = Vec::new();
        let mut dir_offset = 0;
        let t_try = self.d().top_t_try;
        self.d().top_t_try += 1;
        let t = get_t_guess(t_try, &mut dir_offset);
        let mut hit_base = Box::new(RayHit::default());
        let mut dir = hit_base.make_test_base(self, t);
        if hit_base.slope.x == 0.0 && hit_base.slope.y == 0.0 {
            return false;
        }
        let mut hit_head: *mut RayHit = &mut *hit_base;
        dir = RayDir::from_i32(dir as i32 + dir_offset);
        if hit_base.span.is_some()
            && hit_base.span.segment().verb() != Verb::Line
            && pt_dydx(&hit_base.slope, dir) == 0.0
        {
            return false;
        }
        let base_copy = *hit_base;
        let mut contour = contour_head;
        loop {
            if contour.count() != 0 {
                contour.ray_check(&base_copy, dir, &mut hit_head, &mut allocator);
            }
            contour = contour.next();
            if contour.is_null() {
                break;
            }
        }
        // sort hits
        let mut sorted: Vec<*mut RayHit> = Vec::new();
        let mut hit = hit_head;
        while !hit.is_null() {
            sorted.push(hit);
            hit = unsafe { (*hit).next };
        }
        let count = sorted.len();
        unsafe {
            if xy_index(dir) == 1 {
                if less_than(dir) {
                    super::sort::tq_sort(&mut sorted, |&a, &b| (*a).pt.y < (*b).pt.y);
                } else {
                    super::sort::tq_sort(&mut sorted, |&a, &b| (*b).pt.y < (*a).pt.y);
                }
            } else if less_than(dir) {
                super::sort::tq_sort(&mut sorted, |&a, &b| (*a).pt.x < (*b).pt.x);
            } else {
                super::sort::tq_sort(&mut sorted, |&a, &b| (*b).pt.x < (*a).pt.x);
            }
        }
        // verify windings
        let mut last: Option<Point> = None;
        let mut wind = 0i32;
        let mut opp_wind = 0i32;
        for index in 0..count {
            let hit = unsafe { *sorted[index] };
            if !hit.valid {
                return false;
            }
            let ccw = ccw_dxdy(&hit.slope, dir);
            let span = hit.span;
            if span.is_null() {
                return false;
            }
            let hit_segment = span.segment();
            if span.wind_value() == 0 && span.opp_value() == 0 {
                continue;
            }
            if let Some(last) = last
                && DPoint::approximately_equal_sk(last, hit.pt)
            {
                return false;
            }
            if index < count - 1 {
                let next = unsafe { (*sorted[index + 1]).pt };
                if DPoint::approximately_equal_sk(next, hit.pt) {
                    return false;
                }
            }
            let operand = hit_segment.operand();
            if operand {
                std::mem::swap(&mut wind, &mut opp_wind);
            }
            let last_wind = wind;
            let last_opp = opp_wind;
            let wind_value = if ccw { -span.wind_value() } else { span.wind_value() };
            let opp_value = if ccw { -span.opp_value() } else { span.opp_value() };
            wind = wind.wrapping_add(wind_value);
            opp_wind = opp_wind.wrapping_add(opp_value);
            let mut sum_set = false;
            let span_sum = span.wind_sum();
            let wind_sum = if Segment::UseInnerWinding(last_wind, wind) { wind } else { last_wind };
            if span_sum == SK_MIN_S32 {
                span.set_wind_sum(wind_sum);
                sum_set = true;
            }
            let o_span_sum = span.opp_sum();
            let opp_sum = if Segment::UseInnerWinding(last_opp, opp_wind) { opp_wind } else { last_opp };
            if o_span_sum == SK_MIN_S32 {
                span.set_opp_sum(opp_sum);
            }
            if sum_set {
                if self.global_state().phase == OpPhase::FixWinding {
                    hit_segment.contour().d().ccw = ccw as i32;
                } else {
                    let _ = hit_segment.mark_and_chase_winding_op(span, span.next(), wind_sum, opp_sum, None);
                    let _ = hit_segment.mark_and_chase_winding_op(span.next(), span, wind_sum, opp_sum, None);
                }
            }
            if operand {
                std::mem::swap(&mut wind, &mut opp_wind);
            }
            last = Some(hit.pt);
            self.global_state().nested += 1;
        }
        true
    }
}

impl Contour {
    pub fn find_sortable_top(self, contour_head: Contour) -> Span {
        let mut all_done = true;
        if self.count() != 0 {
            let mut test_segment = self.first();
            loop {
                if !test_segment.done() {
                    all_done = false;
                    let result = test_segment.find_sortable_top(contour_head);
                    if result.is_some() {
                        return result;
                    }
                }
                test_segment = test_segment.next();
                if test_segment.is_null() {
                    break;
                }
            }
        }
        if all_done {
            self.d().done = true;
        }
        Span::NULL
    }
}

/// `FindSortableTop`.
pub fn find_sortable_top(contour_head: Contour) -> Span {
    for _ in 0..MAX_WINDING_TRIES {
        let mut contour = contour_head;
        loop {
            if !contour.done() {
                let result = contour.find_sortable_top(contour_head);
                if result.is_some() {
                    return result;
                }
            }
            contour = contour.next();
            if contour.is_null() {
                break;
            }
        }
    }
    Span::NULL
}
