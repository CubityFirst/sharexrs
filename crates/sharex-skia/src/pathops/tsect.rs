//! `SkPathOpsTSect.h`, `SkPathOpsTSect.cpp` and `SkPathOpsTCurve.h`
//! (`SkTQuad`, `SkTConic`, `SkTCubic`): curve/curve intersection by
//! bisecting both curves into spans and discarding the pairs whose hulls
//! miss. The spans are linked as Skia links them (raw pointers into the
//! sect's arena, a free list for deleted spans), so the order in which they
//! are split, trimmed and recycled is Skia's.
#![allow(clippy::missing_safety_doc)]

use super::conic::DConic;
use super::cubic::DCubic;
use super::intersections::Intersections;
use super::line::DLine;
use super::point::{DPoint, DVector};
use super::quad::DQuad;
use super::rect::DRect;
use super::types::*;
use std::ptr::null_mut;

const COINCIDENT_SPAN_COUNT: i32 = 9;

/// `SkTCurve`: the curve a sect bisects.
#[derive(Clone, Copy, Debug)]
pub enum TCurve {
    Quad(DQuad),
    Conic(DConic),
    Cubic(DCubic),
}

impl std::ops::Index<usize> for TCurve {
    type Output = DPoint;
    fn index(&self, n: usize) -> &DPoint {
        match self {
            TCurve::Quad(q) => &q.pts[n],
            TCurve::Conic(c) => &c.pts.pts[n],
            TCurve::Cubic(c) => &c.pts[n],
        }
    }
}

impl TCurve {
    /// `make`: a curve of the same kind (its points are set by `subDivide`).
    fn make(&self) -> TCurve {
        match self {
            TCurve::Quad(_) => TCurve::Quad(DQuad::default()),
            TCurve::Conic(_) => TCurve::Conic(DConic::default()),
            TCurve::Cubic(_) => TCurve::Cubic(DCubic::default()),
        }
    }

    pub fn collapsed(&self) -> bool {
        match self {
            TCurve::Quad(q) => q.collapsed(),
            TCurve::Conic(c) => c.collapsed(),
            TCurve::Cubic(c) => c.collapsed(),
        }
    }

    pub fn controls_inside(&self) -> bool {
        match self {
            TCurve::Quad(q) => q.controls_inside(),
            TCurve::Conic(c) => c.controls_inside(),
            TCurve::Cubic(c) => c.controls_inside(),
        }
    }

    pub fn dxdy_at_t(&self, t: f64) -> DVector {
        match self {
            TCurve::Quad(q) => q.dxdy_at_t(t),
            TCurve::Conic(c) => c.dxdy_at_t(t),
            TCurve::Cubic(c) => c.dxdy_at_t(t),
        }
    }

    /// `hullIntersects(const SkTCurve&)`, through Skia's double dispatch.
    pub fn hull_intersects(&self, opp: &TCurve, is_linear: &mut bool) -> bool {
        match (self, opp) {
            (TCurve::Quad(a), TCurve::Quad(b)) => a.hull_intersects_quad(b, is_linear),
            (TCurve::Conic(a), TCurve::Quad(b)) => a.hull_intersects_quad(b, is_linear),
            (TCurve::Cubic(a), TCurve::Quad(b)) => a.hull_intersects_quad(b, is_linear),
            (TCurve::Quad(a), TCurve::Conic(b)) => a.hull_intersects_conic(b, is_linear),
            (TCurve::Conic(a), TCurve::Conic(b)) => a.hull_intersects_conic(b, is_linear),
            (TCurve::Cubic(a), TCurve::Conic(b)) => a.hull_intersects_conic(b, is_linear),
            (TCurve::Quad(a), TCurve::Cubic(b)) => a.hull_intersects_cubic(b, is_linear),
            (TCurve::Conic(a), TCurve::Cubic(b)) => a.hull_intersects_cubic(b, is_linear),
            (TCurve::Cubic(a), TCurve::Cubic(b)) => a.hull_intersects_cubic(b, is_linear),
        }
    }

    pub fn intersect_ray(&self, i: &mut Intersections, line: &DLine) -> i32 {
        match self {
            TCurve::Quad(q) => i.intersect_ray_quad(q, line),
            TCurve::Conic(c) => i.intersect_ray_conic(c, line),
            TCurve::Cubic(c) => i.intersect_ray_cubic(c, line),
        }
    }

    pub fn is_conic(&self) -> bool {
        matches!(self, TCurve::Conic(_))
    }

    pub fn max_intersections(&self) -> i32 {
        match self {
            TCurve::Quad(_) | TCurve::Conic(_) => 4,
            TCurve::Cubic(_) => 9,
        }
    }

    /// The points other than `odd_man` (2 for quads and conics, 3 for cubics).
    pub fn other_pts(&self, odd_man: usize) -> ([DPoint; 3], usize) {
        match self {
            TCurve::Quad(q) => {
                let o = q.other_pts(odd_man);
                ([o[0], o[1], DPoint::default()], 2)
            }
            TCurve::Conic(c) => {
                let o = c.other_pts(odd_man);
                ([o[0], o[1], DPoint::default()], 2)
            }
            TCurve::Cubic(c) => (c.other_pts(odd_man), 3),
        }
    }

    pub fn point_count(&self) -> usize {
        match self {
            TCurve::Quad(_) | TCurve::Conic(_) => 3,
            TCurve::Cubic(_) => 4,
        }
    }

    pub fn point_last(&self) -> usize {
        self.point_count() - 1
    }

    pub fn pt_at_t(&self, t: f64) -> DPoint {
        match self {
            TCurve::Quad(q) => q.pt_at_t(t),
            TCurve::Conic(c) => c.pt_at_t(t),
            TCurve::Cubic(c) => c.pt_at_t(t),
        }
    }

    pub fn set_bounds(&self, rect: &mut DRect) {
        match self {
            TCurve::Quad(q) => rect.set_bounds_quad(q, q, 0.0, 1.0),
            TCurve::Conic(c) => rect.set_bounds_conic(c, c, 0.0, 1.0),
            TCurve::Cubic(c) => rect.set_bounds_cubic(c, c, 0.0, 1.0),
        }
    }

    pub fn sub_divide(&self, t1: f64, t2: f64) -> TCurve {
        match self {
            TCurve::Quad(q) => TCurve::Quad(q.sub_divide(t1, t2)),
            TCurve::Conic(c) => TCurve::Conic(c.sub_divide(t1, t2)),
            TCurve::Cubic(c) => TCurve::Cubic(c.sub_divide(t1, t2)),
        }
    }
}

/// `SkTCoincident`.
#[derive(Clone, Copy, Debug)]
pub struct TCoincident {
    perp_pt: DPoint,
    perp_t: f64,
    is_match: bool,
}

impl Default for TCoincident {
    fn default() -> Self {
        let mut c = TCoincident { perp_pt: DPoint::default(), perp_t: 0.0, is_match: false };
        c.init();
        c
    }
}

impl TCoincident {
    pub fn is_match(&self) -> bool {
        self.is_match
    }

    pub fn init(&mut self) {
        self.perp_t = -1.0;
        self.is_match = false;
        self.perp_pt = DPoint::new(f32::NAN as f64, f32::NAN as f64);
    }

    pub fn mark_coincident(&mut self) {
        if !self.is_match {
            self.perp_t = -1.0;
        }
        self.is_match = true;
    }

    pub fn perp_pt(&self) -> DPoint {
        self.perp_pt
    }

    pub fn perp_t(&self) -> f64 {
        self.perp_t
    }

    pub fn set_perp(&mut self, c1: &TCurve, t: f64, c_pt: &DPoint, c2: &TCurve) {
        let dxdy = c1.dxdy_at_t(t);
        let perp = DLine::new(*c_pt, DPoint::new(c_pt.x + dxdy.y, c_pt.y - dxdy.x));
        let mut i = Intersections::default();
        let used = c2.intersect_ray(&mut i, &perp);
        // only keep closest
        if used == 0 || used == 3 {
            self.init();
            return;
        }
        self.perp_t = i.t(0, 0);
        self.perp_pt = i.pt(0);
        if used == 2 {
            let dist_sq = (self.perp_pt - *c_pt).length_squared();
            let dist2_sq = (i.pt(1) - *c_pt).length_squared();
            if dist2_sq < dist_sq {
                self.perp_t = i.t(0, 1);
                self.perp_pt = i.pt(1);
            }
        }
        self.is_match = c_pt.approximately_equal(&self.perp_pt);
    }
}

/// `SkTSpanBounded`.
pub struct TSpanBounded {
    bounded: *mut TSpan,
    next: *mut TSpanBounded,
}

/// `SkTSpan`.
pub struct TSpan {
    part: TCurve,
    coin_start: TCoincident,
    coin_end: TCoincident,
    bounded: *mut TSpanBounded,
    prev: *mut TSpan,
    next: *mut TSpan,
    bounds: DRect,
    start_t: f64,
    end_t: f64,
    bounds_max: f64,
    collapsed: bool,
    has_perp: bool,
    is_linear: bool,
    is_line: bool,
    deleted: bool,
}

/// The sect's `SkArenaAlloc`: spans and bounded links live until the sect
/// is dropped.
#[derive(Default)]
struct Heap {
    spans: Vec<*mut TSpan>,
    bounded: Vec<*mut TSpanBounded>,
}

impl Heap {
    fn make_span(&mut self, curve: &TCurve) -> *mut TSpan {
        let p = Box::into_raw(Box::new(TSpan::new(curve)));
        self.spans.push(p);
        p
    }

    fn make_bounded(&mut self) -> *mut TSpanBounded {
        let p = Box::into_raw(Box::new(TSpanBounded { bounded: null_mut(), next: null_mut() }));
        self.bounded.push(p);
        p
    }
}

impl Drop for Heap {
    fn drop(&mut self) {
        for &p in &self.spans {
            drop(unsafe { Box::from_raw(p) });
        }
        for &p in &self.bounded {
            drop(unsafe { Box::from_raw(p) });
        }
    }
}

impl TSpan {
    fn new(curve: &TCurve) -> Self {
        TSpan {
            part: curve.make(),
            coin_start: TCoincident::default(),
            coin_end: TCoincident::default(),
            bounded: null_mut(),
            prev: null_mut(),
            next: null_mut(),
            bounds: DRect::default(),
            start_t: 0.0,
            end_t: 0.0,
            bounds_max: 0.0,
            collapsed: false,
            has_perp: false,
            is_linear: false,
            is_line: false,
            deleted: false,
        }
    }

    pub fn start_t(&self) -> f64 {
        self.start_t
    }

    pub fn end_t(&self) -> f64 {
        self.end_t
    }

    pub fn part(&self) -> &TCurve {
        &self.part
    }

    fn point_count(&self) -> usize {
        self.part.point_count()
    }

    fn point_first(&self) -> DPoint {
        self.part[0]
    }

    fn point_last(&self) -> DPoint {
        self.part[self.part.point_last()]
    }

    fn is_bounded(&self) -> bool {
        !self.bounded.is_null()
    }

    fn mark_coincident(&mut self) {
        self.coin_start.mark_coincident();
        self.coin_end.mark_coincident();
    }

    fn reset(&mut self) {
        self.bounded = null_mut();
    }

    fn reset_bounds(&mut self, curve: &TCurve) {
        self.is_linear = false;
        self.is_line = false;
        self.init_bounds(curve);
    }

    unsafe fn add_bounded(this: *mut TSpan, span: *mut TSpan, heap: &mut Heap) {
        let bounded = heap.make_bounded();
        unsafe {
            (*bounded).bounded = span;
            (*bounded).next = (*this).bounded;
            (*this).bounded = bounded;
        }
    }

    fn closest_bounded_t(&self, pt: &DPoint) -> f64 {
        let mut result = -1.0;
        let mut closest = f64::MAX;
        let mut test_bounded = self.bounded;
        while !test_bounded.is_null() {
            unsafe {
                let test = &*(*test_bounded).bounded;
                let start_dist = test.point_first().distance_squared(pt);
                if closest > start_dist {
                    closest = start_dist;
                    result = test.start_t;
                }
                let end_dist = test.point_last().distance_squared(pt);
                if closest > end_dist {
                    closest = end_dist;
                    result = test.end_t;
                }
                test_bounded = (*test_bounded).next;
            }
        }
        result
    }

    unsafe fn contains(this: *const TSpan, t: f64) -> bool {
        let mut work = this;
        loop {
            unsafe {
                if between((*work).start_t, t, (*work).end_t) {
                    return true;
                }
                work = (*work).next;
            }
            if work.is_null() {
                return false;
            }
        }
    }

    unsafe fn find_opp_span(this: *const TSpan, opp: *const TSpan) -> *mut TSpan {
        unsafe {
            let mut bounded = (*this).bounded;
            while !bounded.is_null() {
                let test = (*bounded).bounded;
                if opp == test as *const TSpan {
                    return test;
                }
                bounded = (*bounded).next;
            }
        }
        null_mut()
    }

    /// 0 if no hull intersection, 1 if hulls intersect, 2 if hulls only
    /// share a common endpoint, -1 if linear and further checking is required.
    unsafe fn hull_check(this: *mut TSpan, opp: *const TSpan, start: &mut bool, opp_start: &mut bool) -> i32 {
        unsafe {
            if (*this).is_linear {
                return -1;
            }
            let mut pts_in_common = false;
            if Self::only_end_points_in_common(this, opp, start, opp_start, &mut pts_in_common) {
                return 2;
            }
            let mut linear = false;
            if (*this).part.hull_intersects(&(*opp).part, &mut linear) {
                if !linear {
                    // check set true if linear
                    return 1;
                }
                (*this).is_linear = true;
                (*this).is_line = (*this).part.controls_inside();
                return if pts_in_common { 1 } else { -1 };
            }
            // hull is not linear; check set true if intersected at the end points
            (pts_in_common as i32) << 1 // 0 or 2
        }
    }

    unsafe fn hulls_intersect(this: *mut TSpan, opp: *mut TSpan, start: &mut bool, opp_start: &mut bool) -> i32 {
        unsafe {
            if !(*this).bounds.intersects(&(*opp).bounds) {
                return 0;
            }
            let mut hull_sect = Self::hull_check(this, opp, start, opp_start);
            if hull_sect >= 0 {
                return hull_sect;
            }
            hull_sect = Self::hull_check(opp, this, opp_start, start);
            if hull_sect >= 0 {
                return hull_sect;
            }
        }
        -1
    }

    fn init(&mut self, c: &TCurve) {
        self.prev = null_mut();
        self.next = null_mut();
        self.start_t = 0.0;
        self.end_t = 1.0;
        self.bounded = null_mut();
        self.reset_bounds(c);
    }

    fn init_bounds(&mut self, c: &TCurve) -> bool {
        if self.start_t.is_nan() || self.end_t.is_nan() {
            return false;
        }
        self.part = c.sub_divide(self.start_t, self.end_t);
        self.part.set_bounds(&mut self.bounds);
        self.coin_start.init();
        self.coin_end.init();
        self.bounds_max = smax(self.bounds.width(), self.bounds.height());
        self.collapsed = self.part.collapsed();
        self.has_perp = false;
        self.deleted = false;
        self.bounds.valid()
    }

    unsafe fn linears_intersect(this: *mut TSpan, span: *mut TSpan) -> bool {
        unsafe {
            let result = (*this).linear_intersects(&(*span).part);
            if result <= 1 {
                return result != 0;
            }
            let result = (*span).linear_intersects(&(*this).part);
            result != 0
        }
    }

    fn linear_intersects(&self, q2: &TCurve) -> i32 {
        // looks like q1 is near-linear
        let mut start = 0usize;
        let mut end = self.part.point_last(); // the outside points are usually the extremes
        if !self.part.controls_inside() {
            let mut dist = 0.0; // if there's any question, compute distance to find best outsiders
            for outer in 0..self.point_count() - 1 {
                for inner in outer + 1..self.point_count() {
                    let test = (self.part[outer] - self.part[inner]).length_squared();
                    if dist > test {
                        continue;
                    }
                    dist = test;
                    start = outer;
                    end = inner;
                }
            }
        }
        // see if q2 is on one side of the line formed by the extreme points
        let orig_x = self.part[start].x;
        let orig_y = self.part[start].y;
        let adj = self.part[end].x - orig_x;
        let opp = self.part[end].y - orig_y;
        let max_part = smax(adj.abs(), opp.abs());
        let mut sign = 0.0;
        for n in 0..q2.point_count() {
            let dx = q2[n].y - orig_y;
            let dy = q2[n].x - orig_x;
            let max_val = smax(max_part, smax(dx.abs(), dy.abs()));
            let test = (q2[n].y - orig_y) * adj - (q2[n].x - orig_x) * opp;
            if precisely_zero_when_compared_to(test, max_val) {
                return 1;
            }
            if approximately_zero_when_compared_to(test, max_val) {
                return 3;
            }
            if n == 0 {
                sign = test;
                continue;
            }
            if test * sign < 0.0 {
                return 1;
            }
        }
        0
    }

    unsafe fn only_end_points_in_common(
        this: *mut TSpan,
        opp: *const TSpan,
        start: &mut bool,
        opp_start: &mut bool,
        pts_in_common: &mut bool,
    ) -> bool {
        let (s, o) = unsafe { (&*this, &*opp) };
        if o.point_first() == s.point_first() {
            *start = true;
            *opp_start = true;
        } else if o.point_first() == s.point_last() {
            *start = false;
            *opp_start = true;
        } else if o.point_last() == s.point_first() {
            *start = true;
            *opp_start = false;
        } else if o.point_last() == s.point_last() {
            *start = false;
            *opp_start = false;
        } else {
            *pts_in_common = false;
            return false;
        }
        *pts_in_common = true;
        let base_index = if *start { 0 } else { s.part.point_last() };
        let (other_pts, _) = s.part.other_pts(base_index);
        let (opp_other_pts, _) = o.part.other_pts(if *opp_start { 0 } else { o.part.point_last() });
        let base = s.part[base_index];
        for o1 in 0..s.point_count() - 1 {
            let v1 = other_pts[o1] - base;
            for o2 in 0..o.point_count() - 1 {
                let v2 = opp_other_pts[o2] - base;
                if v2.dot(v1) >= 0.0 {
                    return false;
                }
            }
        }
        true
    }

    unsafe fn opp_t(this: *const TSpan, t: f64) -> *mut TSpan {
        unsafe {
            let mut bounded = (*this).bounded;
            while !bounded.is_null() {
                let test = (*bounded).bounded;
                if between((*test).start_t, t, (*test).end_t) {
                    return test;
                }
                bounded = (*bounded).next;
            }
        }
        null_mut()
    }

    unsafe fn find_opp_t(this: *const TSpan, t: f64) -> *mut TSpan {
        unsafe { Self::opp_t(this, t) }
    }

    unsafe fn has_opp_t(this: *const TSpan, t: f64) -> bool {
        unsafe { !Self::opp_t(this, t).is_null() }
    }

    unsafe fn remove_all_bounded(this: *mut TSpan) -> bool {
        let mut delete_span = false;
        unsafe {
            let mut bounded = (*this).bounded;
            while !bounded.is_null() {
                let opp = (*bounded).bounded;
                delete_span |= Self::remove_bounded(opp, this);
                bounded = (*bounded).next;
            }
        }
        delete_span
    }

    unsafe fn remove_bounded(this: *mut TSpan, opp: *const TSpan) -> bool {
        unsafe {
            if (*this).has_perp {
                let mut found_start = false;
                let mut found_end = false;
                let mut bounded = (*this).bounded;
                while !bounded.is_null() {
                    let test = (*bounded).bounded;
                    if opp != test as *const TSpan {
                        found_start |= between((*test).start_t, (*this).coin_start.perp_t(), (*test).end_t);
                        found_end |= between((*test).start_t, (*this).coin_end.perp_t(), (*test).end_t);
                    }
                    bounded = (*bounded).next;
                }
                if !found_start || !found_end {
                    (*this).has_perp = false;
                    (*this).coin_start.init();
                    (*this).coin_end.init();
                }
            }
            let mut bounded = (*this).bounded;
            let mut prev: *mut TSpanBounded = null_mut();
            while !bounded.is_null() {
                let bounded_next = (*bounded).next;
                if opp == (*bounded).bounded as *const TSpan {
                    if !prev.is_null() {
                        (*prev).next = bounded_next;
                        return false;
                    } else {
                        (*this).bounded = bounded_next;
                        return (*this).bounded.is_null();
                    }
                }
                prev = bounded;
                bounded = bounded_next;
            }
        }
        false
    }

    unsafe fn split_at(this: *mut TSpan, work: *mut TSpan, t: f64, heap: &mut Heap) -> bool {
        unsafe {
            (*this).start_t = t;
            (*this).end_t = (*work).end_t;
            if (*this).start_t == (*this).end_t {
                (*this).collapsed = true;
                return false;
            }
            (*work).end_t = t;
            if (*work).start_t == (*work).end_t {
                (*work).collapsed = true;
                return false;
            }
            (*this).prev = work;
            (*this).next = (*work).next;
            (*this).is_linear = (*work).is_linear;
            (*this).is_line = (*work).is_line;

            (*work).next = this;
            if !(*this).next.is_null() {
                (*(*this).next).prev = this;
            }
            let mut bounded = (*work).bounded;
            (*this).bounded = null_mut();
            while !bounded.is_null() {
                Self::add_bounded(this, (*bounded).bounded, heap);
                bounded = (*bounded).next;
            }
            bounded = (*this).bounded;
            while !bounded.is_null() {
                Self::add_bounded((*bounded).bounded, this, heap);
                bounded = (*bounded).next;
            }
        }
        true
    }

    unsafe fn split(this: *mut TSpan, work: *mut TSpan, heap: &mut Heap) -> bool {
        unsafe {
            let t = ((*work).start_t + (*work).end_t) * 0.5;
            Self::split_at(this, work, t, heap)
        }
    }
}

/// `SkTSect`.
pub struct TSect {
    curve: TCurve,
    heap: Heap,
    head: *mut TSpan,
    coincident: *mut TSpan,
    deleted: *mut TSpan,
    active_count: i32,
    removed_start_t: bool,
    removed_end_t: bool,
    hung: bool,
}

const ZERO_S1_SET: i32 = 1;
const ONE_S1_SET: i32 = 2;
const ZERO_S2_SET: i32 = 4;
const ONE_S2_SET: i32 = 8;

impl TSect {
    /// Boxed so that its address (which spans don't hold, but the opposite
    /// sect's code does) stays put.
    pub fn new(c: TCurve) -> Box<TSect> {
        let mut sect = Box::new(TSect {
            curve: c,
            heap: Heap::default(),
            head: null_mut(),
            coincident: null_mut(),
            deleted: null_mut(),
            active_count: 0,
            removed_start_t: false,
            removed_end_t: false,
            hung: false,
        });
        sect.reset_removed_ends();
        sect.head = sect.add_one();
        let curve = sect.curve;
        unsafe { (*sect.head).init(&curve) };
        sect
    }

    fn reset_removed_ends(&mut self) {
        self.removed_start_t = false;
        self.removed_end_t = false;
    }

    fn point_last(&self) -> DPoint {
        self.curve[self.curve.point_last()]
    }

    unsafe fn add_following(&mut self, prior: *mut TSpan) -> *mut TSpan {
        let result = self.add_one();
        unsafe {
            (*result).start_t = if !prior.is_null() { (*prior).end_t } else { 0.0 };
            let next = if !prior.is_null() { (*prior).next } else { self.head };
            (*result).end_t = if !next.is_null() { (*next).start_t } else { 1.0 };
            (*result).prev = prior;
            (*result).next = next;
            if !prior.is_null() {
                (*prior).next = result;
            } else {
                self.head = result;
            }
            if !next.is_null() {
                (*next).prev = result;
            }
            let curve = self.curve;
            (*result).reset_bounds(&curve);
        }
        result
    }

    unsafe fn add_for_perp(&mut self, span: *mut TSpan, t: f64) {
        unsafe {
            if !TSpan::has_opp_t(span, t) {
                let mut prior_span = null_mut();
                let mut opp = self.span_at_t(t, &mut prior_span);
                if opp.is_null() {
                    opp = self.add_following(prior_span);
                }
                TSpan::add_bounded(opp, span, &mut self.heap);
                TSpan::add_bounded(span, opp, &mut self.heap);
            }
        }
    }

    fn add_one(&mut self) -> *mut TSpan {
        let result;
        if !self.deleted.is_null() {
            result = self.deleted;
            self.deleted = unsafe { (*result).next };
        } else {
            result = self.heap.make_span(&self.curve);
        }
        unsafe {
            (*result).reset();
            (*result).has_perp = false;
            (*result).deleted = false;
        }
        self.active_count += 1;
        result
    }

    unsafe fn add_split_at(&mut self, span: *mut TSpan, t: f64) -> *mut TSpan {
        let result = self.add_one();
        unsafe {
            TSpan::split_at(result, span, t, &mut self.heap);
            let curve = self.curve;
            (*result).init_bounds(&curve);
            (*span).init_bounds(&curve);
        }
        result
    }

    unsafe fn binary_search_coin(
        &mut self,
        sect2: *mut TSect,
        t_start: f64,
        mut t_step: f64,
        result_t: &mut f64,
        opp_t: &mut f64,
        opp_first: &mut *mut TSpan,
    ) -> bool {
        let mut work = TSpan::new(&self.curve);
        work.start_t = t_start;
        work.end_t = t_start;
        let mut result = t_start;
        let mut last = self.curve.pt_at_t(t_start);
        let mut opp_pt = DPoint::default();
        let mut flip = false;
        let mut contained = false;
        let down = t_step < 0.0;
        let opp = unsafe { (*sect2).curve };
        loop {
            t_step *= 0.5;
            work.start_t += t_step;
            if flip {
                t_step = -t_step;
                flip = false;
            }
            work.init_bounds(&self.curve);
            if work.collapsed {
                return false;
            }
            if last.approximately_equal(&work.point_first()) {
                break;
            }
            last = work.point_first();
            work.coin_start.set_perp(&self.curve, work.start_t, &last, &opp);
            if work.coin_start.is_match() {
                let opp_t_test = work.coin_start.perp_t();
                if unsafe { TSpan::contains((*sect2).head, opp_t_test) } {
                    *opp_t = opp_t_test;
                    opp_pt = work.coin_start.perp_pt();
                    contained = true;
                    if if down { result <= work.start_t } else { result >= work.start_t } {
                        *opp_first = null_mut(); // signal caller to fail
                        return false;
                    }
                    result = work.start_t;
                    continue;
                }
            }
            t_step = -t_step;
            flip = true;
        }
        if !contained {
            return false;
        }
        if last.approximately_equal(&self.curve[0]) {
            result = 0.0;
        } else if last.approximately_equal(&self.point_last()) {
            result = 1.0;
        }
        if opp_pt.approximately_equal(&opp[0]) {
            *opp_t = 0.0;
        } else if opp_pt.approximately_equal(&unsafe { (*sect2).point_last() }) {
            *opp_t = 1.0;
        }
        *result_t = result;
        true
    }

    fn bounds_max(&mut self) -> *mut TSpan {
        let mut test = self.head;
        let mut largest = self.head;
        unsafe {
            let mut l_collapsed = (*largest).collapsed;
            let mut safety_net = 10000;
            loop {
                test = (*test).next;
                if test.is_null() {
                    break;
                }
                safety_net -= 1;
                if safety_net == 0 {
                    self.hung = true;
                    return null_mut();
                }
                let t_collapsed = (*test).collapsed;
                if (l_collapsed && !t_collapsed)
                    || (l_collapsed == t_collapsed && (*largest).bounds_max < (*test).bounds_max)
                {
                    largest = test;
                    l_collapsed = (*test).collapsed;
                }
            }
        }
        largest
    }

    unsafe fn coincident_check(&mut self, sect2: *mut TSect) -> bool {
        let mut first = self.head;
        if first.is_null() {
            return false;
        }
        unsafe {
            loop {
                let mut last = null_mut();
                let consecutive = self.count_consecutive_spans(first, &mut last);
                let next = (*last).next;
                if consecutive >= COINCIDENT_SPAN_COUNT {
                    self.compute_perpendiculars(sect2, first, last);
                    // check to see if a range of points are on the curve
                    let mut coin_start = first;
                    loop {
                        let start = coin_start;
                        let success = self.extract_coincident(sect2, start, last, &mut coin_start);
                        if !success {
                            return false;
                        }
                        if !(!coin_start.is_null() && !(*last).deleted) {
                            break;
                        }
                    }
                    if self.head.is_null() || (*sect2).head.is_null() {
                        break;
                    }
                    if next.is_null() || (*next).deleted {
                        break;
                    }
                }
                first = next;
                if first.is_null() {
                    break;
                }
            }
        }
        true
    }

    unsafe fn coincident_force(&mut self, sect2: *mut TSect, start1s: f64, start1e: f64) {
        unsafe {
            let first = self.head;
            let last = self.tail();
            let opp_first = (*sect2).head;
            let opp_last = (*sect2).tail();
            if last.is_null() || opp_last.is_null() {
                return;
            }
            let mut delete_empty_spans = self.update_bounded(first, last, opp_first);
            delete_empty_spans |= (*sect2).update_bounded(opp_first, opp_last, first);
            self.remove_span_range(first, last);
            (*sect2).remove_span_range(opp_first, opp_last);
            (*first).start_t = start1s;
            (*first).end_t = start1e;
            let curve = self.curve;
            let curve2 = (*sect2).curve;
            (*first).reset_bounds(&curve);
            (*first).coin_start.set_perp(&curve, start1s, &curve[0], &curve2);
            let pl = self.point_last();
            (*first).coin_end.set_perp(&curve, start1e, &pl, &curve2);
            let opp_matched = (*first).coin_start.perp_t() < (*first).coin_end.perp_t();
            let mut opp_start_t =
                if (*first).coin_start.perp_t() == -1.0 { 0.0 } else { smax(0.0, (*first).coin_start.perp_t()) };
            let mut opp_end_t =
                if (*first).coin_end.perp_t() == -1.0 { 1.0 } else { smin(1.0, (*first).coin_end.perp_t()) };
            if !opp_matched {
                std::mem::swap(&mut opp_start_t, &mut opp_end_t);
            }
            (*opp_first).start_t = opp_start_t;
            (*opp_first).end_t = opp_end_t;
            (*opp_first).reset_bounds(&curve2);
            self.remove_coincident(first, false);
            (*sect2).remove_coincident(opp_first, true);
            if delete_empty_spans {
                self.delete_empty_spans();
                (*sect2).delete_empty_spans();
            }
        }
    }

    fn coincident_has_t(&self, t: f64) -> bool {
        let mut test = self.coincident;
        while !test.is_null() {
            unsafe {
                if between((*test).start_t, t, (*test).end_t) {
                    return true;
                }
                test = (*test).next;
            }
        }
        false
    }

    fn collapsed(&self) -> i32 {
        let mut result = 0;
        let mut test = self.head;
        while !test.is_null() {
            unsafe {
                if (*test).collapsed {
                    result += 1;
                }
                test = (*test).next;
            }
        }
        result
    }

    unsafe fn compute_perpendiculars(&mut self, sect2: *mut TSect, first: *mut TSpan, last: *mut TSpan) {
        if last.is_null() {
            return;
        }
        unsafe {
            let opp = (*sect2).curve;
            let curve = self.curve;
            let mut work = first;
            let mut prior: *mut TSpan = null_mut();
            loop {
                if !(*work).has_perp && !(*work).collapsed {
                    if !prior.is_null() {
                        (*work).coin_start = (*prior).coin_end;
                    } else {
                        let (st, pf) = ((*work).start_t, (*work).point_first());
                        (*work).coin_start.set_perp(&curve, st, &pf, &opp);
                    }
                    if (*work).coin_start.is_match() {
                        let perp_t = (*work).coin_start.perp_t();
                        if (*sect2).coincident_has_t(perp_t) {
                            (*work).coin_start.init();
                        } else {
                            (*sect2).add_for_perp(work, perp_t);
                        }
                    }
                    let (et, pl) = ((*work).end_t, (*work).point_last());
                    (*work).coin_end.set_perp(&curve, et, &pl, &opp);
                    if (*work).coin_end.is_match() {
                        let perp_t = (*work).coin_end.perp_t();
                        if (*sect2).coincident_has_t(perp_t) {
                            (*work).coin_end.init();
                        } else {
                            (*sect2).add_for_perp(work, perp_t);
                        }
                    }
                    (*work).has_perp = true;
                }
                if work == last {
                    break;
                }
                prior = work;
                work = (*work).next;
            }
        }
    }

    unsafe fn count_consecutive_spans(&self, first: *mut TSpan, last_ptr: &mut *mut TSpan) -> i32 {
        let mut consecutive = 1;
        let mut last = first;
        unsafe {
            loop {
                let next = (*last).next;
                if next.is_null() {
                    break;
                }
                if (*next).start_t > (*last).end_t {
                    break;
                }
                consecutive += 1;
                last = next;
            }
        }
        *last_ptr = last;
        consecutive
    }

    unsafe fn has_bounded(&self, span: *const TSpan) -> bool {
        let mut test = self.head;
        if test.is_null() {
            return false;
        }
        unsafe {
            loop {
                if !TSpan::find_opp_span(test, span).is_null() {
                    return true;
                }
                test = (*test).next;
                if test.is_null() {
                    break;
                }
            }
        }
        false
    }

    unsafe fn delete_empty_spans(&mut self) -> bool {
        let mut next = self.head;
        let mut safety_hatch = 1000;
        unsafe {
            while !next.is_null() {
                let test = next;
                next = (*test).next;
                if (*test).bounded.is_null() && !self.remove_span(test) {
                    return false;
                }
                safety_hatch -= 1;
                if safety_hatch < 0 {
                    return false;
                }
            }
        }
        true
    }

    unsafe fn extract_coincident(
        &mut self,
        sect2: *mut TSect,
        first_in: *mut TSpan,
        last_in: *mut TSpan,
        result: &mut *mut TSpan,
    ) -> bool {
        unsafe {
            let mut last = last_in;
            let mut first = self.find_coincident_run(first_in, &mut last);
            if first.is_null() || last.is_null() {
                *result = null_mut();
                return true;
            }
            // march outwards to find limit of coincidence from here to previous and next spans
            let start_t = (*first).start_t;
            let mut opp_start_t = 0.0;
            let mut opp_end_t = 0.0;
            let prev = (*first).prev;
            let mut opp_first = TSpan::find_opp_t(first, (*first).coin_start.perp_t());
            let opp_matched = (*first).coin_start.perp_t() < (*first).coin_end.perp_t();
            let mut coin_start = 0.0;
            let cut_first: *mut TSpan;
            if !prev.is_null()
                && (*prev).end_t == start_t
                && self.binary_search_coin(
                    sect2,
                    start_t,
                    (*prev).start_t - start_t,
                    &mut coin_start,
                    &mut opp_start_t,
                    &mut opp_first,
                )
                && (*prev).start_t < coin_start
                && coin_start < start_t
                && {
                    cut_first = TSpan::opp_t(prev, opp_start_t);
                    !cut_first.is_null()
                }
            {
                opp_first = cut_first;
                first = self.add_split_at(prev, coin_start);
                (*first).mark_coincident();
                (*prev).coin_end.mark_coincident();
                if (*opp_first).start_t < opp_start_t && opp_start_t < (*opp_first).end_t {
                    let opp_half = (*sect2).add_split_at(opp_first, opp_start_t);
                    if opp_matched {
                        (*opp_first).coin_end.mark_coincident();
                        (*opp_half).mark_coincident();
                        opp_first = opp_half;
                    } else {
                        (*opp_first).mark_coincident();
                        (*opp_half).coin_start.mark_coincident();
                    }
                }
            } else if opp_first.is_null() {
                return false;
            }
            // FIXME: incomplete : if we're not at the end, find end of coin
            let mut opp_last = TSpan::find_opp_t(last, (*last).coin_end.perp_t());
            if !opp_matched {
                std::mem::swap(&mut opp_first, &mut opp_last);
                std::mem::swap(&mut opp_start_t, &mut opp_end_t);
            }
            if opp_first.is_null() {
                *result = null_mut();
                return true;
            }
            if opp_last.is_null() {
                *result = null_mut();
                return true;
            }
            // reduce coincident runs to single entries
            let mut delete_empty_spans = self.update_bounded(first, last, opp_first);
            delete_empty_spans |= (*sect2).update_bounded(opp_first, opp_last, first);
            self.remove_span_range(first, last);
            (*sect2).remove_span_range(opp_first, opp_last);
            (*first).end_t = (*last).end_t;
            let curve = self.curve;
            let curve2 = (*sect2).curve;
            (*first).reset_bounds(&curve);
            let (fs, fpf) = ((*first).start_t, (*first).point_first());
            (*first).coin_start.set_perp(&curve, fs, &fpf, &curve2);
            let (fe, fpl) = ((*first).end_t, (*first).point_last());
            (*first).coin_end.set_perp(&curve, fe, &fpl, &curve2);
            opp_start_t = (*first).coin_start.perp_t();
            opp_end_t = (*first).coin_end.perp_t();
            if between(0.0, opp_start_t, 1.0) && between(0.0, opp_end_t, 1.0) {
                if !opp_matched {
                    std::mem::swap(&mut opp_start_t, &mut opp_end_t);
                }
                (*opp_first).start_t = opp_start_t;
                (*opp_first).end_t = opp_end_t;
                (*opp_first).reset_bounds(&curve2);
            }
            last = (*first).next;
            if !self.remove_coincident(first, false) {
                return false;
            }
            if !(*sect2).remove_coincident(opp_first, true) {
                return false;
            }
            if delete_empty_spans && (!self.delete_empty_spans() || !(*sect2).delete_empty_spans()) {
                *result = null_mut();
                return false;
            }
            *result = if !last.is_null() && !(*last).deleted && !self.head.is_null() && !(*sect2).head.is_null() {
                last
            } else {
                null_mut()
            };
        }
        true
    }

    unsafe fn find_coincident_run(&mut self, first_in: *mut TSpan, last_ptr: &mut *mut TSpan) -> *mut TSpan {
        let mut work = first_in;
        let mut last_candidate: *mut TSpan = null_mut();
        let mut first: *mut TSpan = null_mut();
        unsafe {
            // find the first fully coincident span
            loop {
                if (*work).coin_start.is_match() {
                    if !(*work).coin_end.is_match() {
                        break;
                    }
                    last_candidate = work;
                    if first.is_null() {
                        first = work;
                    }
                } else if !first.is_null() && (*work).collapsed {
                    *last_ptr = last_candidate;
                    return first;
                } else {
                    last_candidate = null_mut();
                }
                if work == *last_ptr {
                    return first;
                }
                work = (*work).next;
                if work.is_null() {
                    return null_mut();
                }
            }
        }
        if !last_candidate.is_null() {
            *last_ptr = last_candidate;
        }
        first
    }

    unsafe fn intersects(&mut self, span: *mut TSpan, opp: *mut TSect, opp_span: *mut TSpan, opp_result: &mut i32) -> i32 {
        unsafe {
            let mut span_start = false;
            let mut opp_start = false;
            let mut hull_result = TSpan::hulls_intersect(span, opp_span, &mut span_start, &mut opp_start);
            if hull_result >= 0 {
                if hull_result == 2 {
                    // hulls have one point in common
                    if (*span).bounded.is_null() || (*(*span).bounded).next.is_null() {
                        if span_start {
                            (*span).end_t = (*span).start_t;
                        } else {
                            (*span).start_t = (*span).end_t;
                        }
                    } else {
                        hull_result = 1;
                    }
                    if (*opp_span).bounded.is_null() || (*(*opp_span).bounded).next.is_null() {
                        if !(*opp_span).bounded.is_null() && (*(*opp_span).bounded).bounded != span {
                            return 0;
                        }
                        if opp_start {
                            (*opp_span).end_t = (*opp_span).start_t;
                        } else {
                            (*opp_span).start_t = (*opp_span).end_t;
                        }
                        *opp_result = 2;
                    } else {
                        *opp_result = 1;
                    }
                } else {
                    *opp_result = 1;
                }
                return hull_result;
            }
            if (*span).is_line && (*opp_span).is_line {
                let mut i = Intersections::default();
                let sects = self.lines_intersect(span, opp, opp_span, &mut i);
                if sects == 2 {
                    *opp_result = 1;
                    return 1;
                }
                if sects == 0 {
                    return -1;
                }
                self.removed_end_check(span);
                (*span).start_t = i.t(0, 0);
                (*span).end_t = i.t(0, 0);
                (*opp).removed_end_check(opp_span);
                (*opp_span).start_t = i.t(1, 0);
                (*opp_span).end_t = i.t(1, 0);
                *opp_result = 2;
                return 2;
            }
            if (*span).is_linear || (*opp_span).is_linear {
                *opp_result = TSpan::linears_intersect(span, opp_span) as i32;
                return *opp_result;
            }
        }
        *opp_result = 1;
        1
    }

    /// While the intersection points are sufficiently far apart, construct
    /// the tangent lines from the intersections and find the point where the
    /// tangent line intersects the opposite curve.
    unsafe fn lines_intersect(&mut self, span: *mut TSpan, opp: *mut TSect, opp_span: *mut TSpan, i: &mut Intersections) -> i32 {
        unsafe {
            let mut this_ray_i = Intersections::default();
            let mut opp_ray_i = Intersections::default();
            let mut this_line = DLine::new((*span).point_first(), (*span).point_last());
            let mut opp_line = DLine::new((*opp_span).point_first(), (*opp_span).point_last());
            let mut loop_count = 0;
            let mut best_dist_sq = f64::MAX;
            let opp_curve = (*opp).curve;
            let curve = self.curve;
            if opp_curve.intersect_ray(&mut this_ray_i, &this_line) == 0 {
                return 0;
            }
            if curve.intersect_ray(&mut opp_ray_i, &opp_line) == 0 {
                return 0;
            }
            // if the ends of each line intersect the opposite curve, the lines are coincident
            if this_ray_i.used() > 1 {
                let mut pt_matches = 0;
                for t_index in 0..this_ray_i.used() as usize {
                    for l_index in 0..2 {
                        pt_matches += this_ray_i.pt(t_index).approximately_equal(&this_line.pts[l_index]) as i32;
                    }
                }
                if pt_matches == 2 || is_parallel(&this_line, &opp_curve) {
                    return 2;
                }
            }
            if opp_ray_i.used() > 1 {
                let mut pt_matches = 0;
                for o_index in 0..opp_ray_i.used() as usize {
                    for l_index in 0..2 {
                        pt_matches += opp_ray_i.pt(o_index).approximately_equal(&opp_line.pts[l_index]) as i32;
                    }
                }
                if pt_matches == 2 || is_parallel(&opp_line, &curve) {
                    return 2;
                }
            }
            loop {
                // pick the closest pair of points
                let mut closest = f64::MAX;
                let mut close_index = 0usize;
                let mut opp_close_index = 0usize;
                for index in 0..opp_ray_i.used() as usize {
                    if !roughly_between((*span).start_t, opp_ray_i.t(0, index), (*span).end_t) {
                        continue;
                    }
                    for o_index in 0..this_ray_i.used() as usize {
                        if !roughly_between((*opp_span).start_t, this_ray_i.t(0, o_index), (*opp_span).end_t) {
                            continue;
                        }
                        let dist_sq = this_ray_i.pt(index).distance_squared(&opp_ray_i.pt(o_index));
                        if closest > dist_sq {
                            closest = dist_sq;
                            close_index = index;
                            opp_close_index = o_index;
                        }
                    }
                }
                if closest == f64::MAX {
                    break;
                }
                let opp_i_pt = this_ray_i.pt(opp_close_index);
                let i_pt = opp_ray_i.pt(close_index);
                if between((*span).start_t, opp_ray_i.t(0, close_index), (*span).end_t)
                    && between((*opp_span).start_t, this_ray_i.t(0, opp_close_index), (*opp_span).end_t)
                    && opp_i_pt.approximately_equal(&i_pt)
                {
                    i.merge(&opp_ray_i, close_index, &this_ray_i, opp_close_index);
                    return i.used();
                }
                let dist_sq = opp_i_pt.distance_squared(&i_pt);
                loop_count += 1;
                if best_dist_sq < dist_sq || loop_count > 5 {
                    return 0;
                }
                best_dist_sq = dist_sq;
                let opp_start = opp_ray_i.t(0, close_index);
                this_line[0] = curve.pt_at_t(opp_start);
                this_line[1] = this_line[0] + curve.dxdy_at_t(opp_start);
                if opp_curve.intersect_ray(&mut this_ray_i, &this_line) == 0 {
                    break;
                }
                let start = this_ray_i.t(0, opp_close_index);
                opp_line[0] = opp_curve.pt_at_t(start);
                opp_line[1] = opp_line[0] + opp_curve.dxdy_at_t(start);
                if curve.intersect_ray(&mut opp_ray_i, &opp_line) == 0 {
                    break;
                }
            }
            // convergence may fail if the curves are nearly coincident
            let mut o_coin_s = TCoincident::default();
            let mut o_coin_e = TCoincident::default();
            o_coin_s.set_perp(&opp_curve, (*opp_span).start_t, &(*opp_span).point_first(), &curve);
            o_coin_e.set_perp(&opp_curve, (*opp_span).end_t, &(*opp_span).point_last(), &curve);
            let mut t_start = o_coin_s.perp_t();
            let mut t_end = o_coin_e.perp_t();
            let swap = t_start > t_end;
            if swap {
                std::mem::swap(&mut t_start, &mut t_end);
            }
            t_start = smax(t_start, (*span).start_t);
            t_end = smin(t_end, (*span).end_t);
            if t_start > t_end {
                return 0;
            }
            let perp_s;
            let perp_e;
            if t_start == (*span).start_t {
                let mut coin_s = TCoincident::default();
                coin_s.set_perp(&curve, (*span).start_t, &(*span).point_first(), &opp_curve);
                perp_s = (*span).point_first() - coin_s.perp_pt();
            } else if swap {
                perp_s = o_coin_e.perp_pt() - (*opp_span).point_last();
            } else {
                perp_s = o_coin_s.perp_pt() - (*opp_span).point_first();
            }
            if t_end == (*span).end_t {
                let mut coin_e = TCoincident::default();
                coin_e.set_perp(&curve, (*span).end_t, &(*span).point_last(), &opp_curve);
                perp_e = (*span).point_last() - coin_e.perp_pt();
            } else if swap {
                perp_e = o_coin_s.perp_pt() - (*opp_span).point_first();
            } else {
                perp_e = o_coin_e.perp_pt() - (*opp_span).point_last();
            }
            if perp_s.dot(perp_e) >= 0.0 {
                return 0;
            }
            let mut coin_w = TCoincident::default();
            let mut work_t = t_start;
            let mut t_step = t_end - t_start;
            let mut work_pt;
            loop {
                t_step *= 0.5;
                if precisely_zero(t_step) {
                    return 0;
                }
                work_t += t_step;
                work_pt = curve.pt_at_t(work_t);
                coin_w.set_perp(&curve, work_t, &work_pt, &opp_curve);
                let perp_t = coin_w.perp_t();
                if if coin_w.is_match() {
                    !between((*opp_span).start_t, perp_t, (*opp_span).end_t)
                } else {
                    perp_t < 0.0
                } {
                    continue;
                }
                let perp_w = work_pt - coin_w.perp_pt();
                if (perp_s.dot(perp_w) >= 0.0) == (t_step < 0.0) {
                    t_step = -t_step;
                }
                if work_pt.approximately_equal(&coin_w.perp_pt()) {
                    break;
                }
            }
            let opp_t_test = coin_w.perp_t();
            if !TSpan::contains((*opp).head, opp_t_test) {
                return 0;
            }
            i.set_max(1);
            i.insert(work_t, opp_t_test, &work_pt);
        }
        1
    }

    unsafe fn mark_span_gone(&mut self, span: *mut TSpan) -> bool {
        self.active_count -= 1;
        if self.active_count < 0 {
            return false;
        }
        unsafe {
            (*span).next = self.deleted;
            self.deleted = span;
            (*span).deleted = true;
        }
        true
    }

    unsafe fn merge_coincidence(&mut self, sect2: *mut TSect) {
        let mut small_limit = 0.0;
        unsafe {
            loop {
                // find the smallest unprocessed span
                let mut smaller: *mut TSpan = null_mut();
                let mut test = self.coincident;
                loop {
                    if test.is_null() {
                        return;
                    }
                    if !((*test).start_t < small_limit) && !(!smaller.is_null() && (*smaller).end_t < (*test).start_t) {
                        smaller = test;
                    }
                    test = (*test).next;
                    if test.is_null() {
                        break;
                    }
                }
                if smaller.is_null() {
                    return;
                }
                small_limit = (*smaller).end_t;
                // find next larger span
                let mut prior: *mut TSpan = null_mut();
                let mut larger: *mut TSpan = null_mut();
                let mut larger_prior: *mut TSpan = null_mut();
                test = self.coincident;
                loop {
                    if !((*test).start_t < (*smaller).end_t)
                        && !(!larger.is_null() && (*larger).start_t < (*test).start_t)
                    {
                        larger_prior = prior;
                        larger = test;
                    }
                    prior = test;
                    test = (*test).next;
                    if test.is_null() {
                        break;
                    }
                }
                if larger.is_null() {
                    continue;
                }
                // check middle t value to see if it is coincident as well
                let mid_t = ((*smaller).end_t + (*larger).start_t) / 2.0;
                let mid_pt = self.curve.pt_at_t(mid_t);
                let mut coin = TCoincident::default();
                coin.set_perp(&self.curve, mid_t, &mid_pt, &(*sect2).curve);
                if coin.is_match() {
                    (*smaller).end_t = (*larger).end_t;
                    (*smaller).coin_end = (*larger).coin_end;
                    if !larger_prior.is_null() {
                        (*larger_prior).next = (*larger).next;
                    } else {
                        self.coincident = (*larger).next;
                    }
                }
            }
        }
    }

    unsafe fn recover_collapsed(&mut self) {
        let mut deleted = self.deleted;
        unsafe {
            while !deleted.is_null() {
                let del_next = (*deleted).next;
                if (*deleted).collapsed {
                    let mut span_ptr: *mut *mut TSpan = &mut self.head;
                    while !(*span_ptr).is_null() && (**span_ptr).end_t <= (*deleted).start_t {
                        span_ptr = &mut (**span_ptr).next;
                    }
                    (*deleted).next = *span_ptr;
                    *span_ptr = deleted;
                }
                deleted = del_next;
            }
        }
    }

    unsafe fn remove_all_but(&mut self, keep: *const TSpan, span: *mut TSpan, opp: *mut TSect) {
        unsafe {
            let mut test_bounded = (*span).bounded;
            while !test_bounded.is_null() {
                let bounded = (*test_bounded).bounded;
                let next = (*test_bounded).next;
                // may have been deleted when opp did 'remove all but'
                if bounded as *const TSpan != keep && !(*bounded).deleted {
                    TSpan::remove_bounded(span, bounded);
                    if TSpan::remove_bounded(bounded, span) {
                        (*opp).remove_span(bounded);
                    }
                }
                test_bounded = next;
            }
        }
    }

    unsafe fn remove_by_perpendicular(&mut self, opp: *mut TSect) -> bool {
        let mut test = self.head;
        unsafe {
            loop {
                let next = (*test).next;
                let skip = (*test).coin_start.perp_t() < 0.0 || (*test).coin_end.perp_t() < 0.0 || {
                    let start_v = (*test).coin_start.perp_pt() - (*test).point_first();
                    let end_v = (*test).coin_end.perp_pt() - (*test).point_last();
                    start_v.dot(end_v) <= 0.0
                };
                if !skip && !self.remove_spans(test, opp) {
                    return false;
                }
                test = next;
                if test.is_null() {
                    break;
                }
            }
        }
        true
    }

    unsafe fn remove_coincident(&mut self, span: *mut TSpan, is_between: bool) -> bool {
        unsafe {
            if !self.unlink_span(span) {
                return false;
            }
            if is_between || between(0.0, (*span).coin_start.perp_t(), 1.0) {
                self.active_count -= 1;
                (*span).next = self.coincident;
                self.coincident = span;
            } else {
                self.mark_span_gone(span);
            }
        }
        true
    }

    unsafe fn removed_end_check(&mut self, span: *mut TSpan) {
        unsafe {
            if (*span).start_t == 0.0 {
                self.removed_start_t = true;
            }
            if 1.0 == (*span).end_t {
                self.removed_end_t = true;
            }
        }
    }

    unsafe fn remove_span(&mut self, span: *mut TSpan) -> bool {
        unsafe {
            self.removed_end_check(span);
            if !self.unlink_span(span) {
                return false;
            }
            self.mark_span_gone(span)
        }
    }

    unsafe fn remove_span_range(&mut self, first: *mut TSpan, last: *mut TSpan) {
        if first == last {
            return;
        }
        unsafe {
            let final_ = (*last).next;
            let mut next = (*first).next;
            while !next.is_null() && next != final_ {
                let span = next;
                next = (*span).next;
                self.mark_span_gone(span);
            }
            if !final_.is_null() {
                (*final_).prev = first;
            }
            (*first).next = final_;
        }
    }

    unsafe fn remove_spans(&mut self, span: *mut TSpan, opp: *mut TSect) -> bool {
        unsafe {
            let mut bounded = (*span).bounded;
            while !bounded.is_null() {
                let span_bounded = (*bounded).bounded;
                let next = (*bounded).next;
                if TSpan::remove_bounded(span, span_bounded) {
                    // shuffles last into position 0
                    self.remove_span(span);
                }
                if TSpan::remove_bounded(span_bounded, span) {
                    (*opp).remove_span(span_bounded);
                }
                if (*span).deleted && (*opp).has_bounded(span) {
                    return false;
                }
                bounded = next;
            }
        }
        true
    }

    fn span_at_t(&self, t: f64, prior_span: &mut *mut TSpan) -> *mut TSpan {
        let mut test = self.head;
        let mut prev: *mut TSpan = null_mut();
        unsafe {
            while !test.is_null() && (*test).end_t < t {
                prev = test;
                test = (*test).next;
            }
            *prior_span = prev;
            if !test.is_null() && (*test).start_t <= t { test } else { null_mut() }
        }
    }

    fn tail(&self) -> *mut TSpan {
        let mut result = self.head;
        let mut next = self.head;
        let mut safety_net = 100000;
        unsafe {
            loop {
                next = (*next).next;
                if next.is_null() {
                    break;
                }
                safety_net -= 1;
                if safety_net == 0 {
                    return null_mut();
                }
                if (*next).end_t > (*result).end_t {
                    result = next;
                }
            }
        }
        result
    }

    /// Each span has a range of opposite spans it intersects. After the span
    /// is split in two, adjust the range to its new size.
    unsafe fn trim(&mut self, span: *mut TSpan, opp: *mut TSect) -> bool {
        unsafe {
            let curve = self.curve;
            if !(*span).init_bounds(&curve) {
                return false;
            }
            let mut test_bounded = (*span).bounded;
            while !test_bounded.is_null() {
                let test = (*test_bounded).bounded;
                let next = (*test_bounded).next;
                let mut opp_sects = 0;
                let sects = self.intersects(span, opp, test, &mut opp_sects);
                if sects >= 1 {
                    if opp_sects == 2 {
                        let opp_curve = (*opp).curve;
                        (*test).init_bounds(&opp_curve);
                        (*opp).remove_all_but(span, test, self);
                    }
                    if sects == 2 {
                        (*span).init_bounds(&curve);
                        self.remove_all_but(test, span, opp);
                        return true;
                    }
                } else {
                    if TSpan::remove_bounded(span, test) {
                        self.remove_span(span);
                    }
                    if TSpan::remove_bounded(test, span) {
                        (*opp).remove_span(test);
                    }
                }
                test_bounded = next;
            }
        }
        true
    }

    unsafe fn unlink_span(&mut self, span: *mut TSpan) -> bool {
        unsafe {
            let prev = (*span).prev;
            let next = (*span).next;
            if !prev.is_null() {
                (*prev).next = next;
                if !next.is_null() {
                    (*next).prev = prev;
                    if (*next).start_t > (*next).end_t {
                        return false;
                    }
                }
            } else {
                self.head = next;
                if !next.is_null() {
                    (*next).prev = null_mut();
                }
            }
        }
        true
    }

    unsafe fn update_bounded(&mut self, first: *mut TSpan, last: *mut TSpan, opp_first: *mut TSpan) -> bool {
        let mut test = first;
        let mut delete_span = false;
        unsafe {
            let final_ = (*last).next;
            loop {
                delete_span |= TSpan::remove_all_bounded(test);
                test = (*test).next;
                if !(test != final_ && !test.is_null()) {
                    break;
                }
            }
            (*first).bounded = null_mut();
            TSpan::add_bounded(first, opp_first, &mut self.heap);
        }
        delete_span
    }

    fn ends_equal(sect1: &TSect, sect2: &TSect, intersections: &mut Intersections) -> i32 {
        let mut zero_one_set = 0;
        if sect1.curve[0] == sect2.curve[0] {
            zero_one_set |= ZERO_S1_SET | ZERO_S2_SET;
            intersections.insert(0.0, 0.0, &sect1.curve[0]);
        }
        if sect1.curve[0] == sect2.point_last() {
            zero_one_set |= ZERO_S1_SET | ONE_S2_SET;
            intersections.insert(0.0, 1.0, &sect1.curve[0]);
        }
        if sect1.point_last() == sect2.curve[0] {
            zero_one_set |= ONE_S1_SET | ZERO_S2_SET;
            intersections.insert(1.0, 0.0, &sect1.point_last());
        }
        if sect1.point_last() == sect2.point_last() {
            zero_one_set |= ONE_S1_SET | ONE_S2_SET;
            intersections.insert(1.0, 1.0, &sect1.point_last());
        }
        // check for zero
        if (zero_one_set & (ZERO_S1_SET | ZERO_S2_SET)) == 0 && sect1.curve[0].approximately_equal(&sect2.curve[0]) {
            zero_one_set |= ZERO_S1_SET | ZERO_S2_SET;
            intersections.insert_near(0.0, 0.0, &sect1.curve[0], &sect2.curve[0]);
        }
        if (zero_one_set & (ZERO_S1_SET | ONE_S2_SET)) == 0 && sect1.curve[0].approximately_equal(&sect2.point_last()) {
            zero_one_set |= ZERO_S1_SET | ONE_S2_SET;
            intersections.insert_near(0.0, 1.0, &sect1.curve[0], &sect2.point_last());
        }
        // check for one
        if (zero_one_set & (ONE_S1_SET | ZERO_S2_SET)) == 0 && sect1.point_last().approximately_equal(&sect2.curve[0]) {
            zero_one_set |= ONE_S1_SET | ZERO_S2_SET;
            intersections.insert_near(1.0, 0.0, &sect1.point_last(), &sect2.curve[0]);
        }
        if (zero_one_set & (ONE_S1_SET | ONE_S2_SET)) == 0 && sect1.point_last().approximately_equal(&sect2.point_last()) {
            zero_one_set |= ONE_S1_SET | ONE_S2_SET;
            intersections.insert_near(1.0, 1.0, &sect1.point_last(), &sect2.point_last());
        }
        zero_one_set
    }

    /// `SkTSect::BinarySearch`.
    pub fn binary_search(sect1: &mut TSect, sect2: &mut TSect, intersections: &mut Intersections) {
        let s1: *mut TSect = sect1;
        let s2: *mut TSect = sect2;
        unsafe { Self::binary_search_impl(s1, s2, intersections) }
    }

    unsafe fn binary_search_impl(sect1: *mut TSect, sect2: *mut TSect, intersections: &mut Intersections) {
        unsafe {
            intersections.reset();
            intersections.set_max((*sect1).curve.max_intersections() + 4); // give extra for slop
            let span1 = (*sect1).head;
            let span2 = (*sect2).head;
            let mut opp_sect = 0;
            let sect = (*sect1).intersects(span1, sect2, span2, &mut opp_sect);
            if sect == 0 {
                return;
            }
            if sect == 2 && opp_sect == 2 {
                let _ = Self::ends_equal(&*sect1, &*sect2, intersections);
                return;
            }
            TSpan::add_bounded(span1, span2, &mut (*sect1).heap);
            TSpan::add_bounded(span2, span1, &mut (*sect2).heap);
            const MAX_COIN_LOOP_COUNT: i32 = 8;
            let mut coin_loop_count = MAX_COIN_LOOP_COUNT;
            let mut start1s = 0.0;
            let mut start1e = 0.0;
            loop {
                // find the largest bounds
                let largest1 = (*sect1).bounds_max();
                if largest1.is_null() {
                    if (*sect1).hung {
                        return;
                    }
                    break;
                }
                let largest2 = (*sect2).bounds_max();
                // split it
                if largest2.is_null()
                    || ((*largest1).bounds_max > (*largest2).bounds_max
                        || (!(*largest1).collapsed && (*largest2).collapsed))
                {
                    if (*sect2).hung {
                        return;
                    }
                    if (*largest1).collapsed {
                        break;
                    }
                    (*sect1).reset_removed_ends();
                    (*sect2).reset_removed_ends();
                    // trim parts that don't intersect the opposite
                    let half1 = (*sect1).add_one();
                    if !TSpan::split(half1, largest1, &mut (*sect1).heap) {
                        break;
                    }
                    if !(*sect1).trim(largest1, sect2) {
                        return;
                    }
                    if !(*sect1).trim(half1, sect2) {
                        return;
                    }
                } else {
                    if (*largest2).collapsed {
                        break;
                    }
                    (*sect1).reset_removed_ends();
                    (*sect2).reset_removed_ends();
                    // trim parts that don't intersect the opposite
                    let half2 = (*sect2).add_one();
                    if !TSpan::split(half2, largest2, &mut (*sect2).heap) {
                        break;
                    }
                    if !(*sect2).trim(largest2, sect1) {
                        return;
                    }
                    if !(*sect2).trim(half2, sect1) {
                        return;
                    }
                }
                // if there are 9 or more continuous spans on both sects, suspect coincidence
                if (*sect1).active_count >= COINCIDENT_SPAN_COUNT && (*sect2).active_count >= COINCIDENT_SPAN_COUNT {
                    if coin_loop_count == MAX_COIN_LOOP_COUNT {
                        start1s = (*(*sect1).head).start_t;
                        start1e = (*(*sect1).tail()).end_t;
                    }
                    if !(*sect1).coincident_check(sect2) {
                        return;
                    }
                    coin_loop_count -= 1;
                    if coin_loop_count == 0 && !(*sect1).head.is_null() && !(*sect2).head.is_null() {
                        (*sect1).coincident_force(sect2, start1s, start1e);
                    }
                }
                if (*sect1).active_count >= COINCIDENT_SPAN_COUNT && (*sect2).active_count >= COINCIDENT_SPAN_COUNT {
                    if (*sect1).head.is_null() {
                        return;
                    }
                    let (h1, t1) = ((*sect1).head, (*sect1).tail());
                    (*sect1).compute_perpendiculars(sect2, h1, t1);
                    if (*sect2).head.is_null() {
                        return;
                    }
                    let (h2, t2) = ((*sect2).head, (*sect2).tail());
                    (*sect2).compute_perpendiculars(sect1, h2, t2);
                    if !(*sect1).remove_by_perpendicular(sect2) {
                        return;
                    }
                    if (*sect1).collapsed() > (*sect1).curve.max_intersections() {
                        break;
                    }
                }
                if (*sect1).head.is_null() || (*sect2).head.is_null() {
                    break;
                }
            }
            let mut coincident = (*sect1).coincident;
            if !coincident.is_null() {
                // if there is more than one coincident span, check loosely to see if they should be joined
                if !(*coincident).next.is_null() {
                    (*sect1).merge_coincidence(sect2);
                    coincident = (*sect1).coincident;
                }
                loop {
                    if coincident.is_null() {
                        return;
                    }
                    if (*coincident).coin_start.is_match() && (*coincident).coin_end.is_match() {
                        let perp_t = (*coincident).coin_start.perp_t();
                        if perp_t < 0.0 {
                            return;
                        }
                        let index = intersections.insert_coincident((*coincident).start_t, perp_t, &(*coincident).point_first());
                        if intersections.insert_coincident(
                            (*coincident).end_t,
                            (*coincident).coin_end.perp_t(),
                            &(*coincident).point_last(),
                        ) < 0
                            && index >= 0
                        {
                            intersections.clear_coincidence(index as usize);
                        }
                    }
                    coincident = (*coincident).next;
                    if coincident.is_null() {
                        break;
                    }
                }
            }
            let zero_one_set = Self::ends_equal(&*sect1, &*sect2, intersections);
            let c1 = (*sect1).curve;
            let c2 = (*sect2).curve;
            // if the final iteration contains an end (0 or 1),
            if (*sect1).removed_start_t && (zero_one_set & ZERO_S1_SET) == 0 {
                let mut perp = TCoincident::default(); // intersect perpendicular with opposite curve
                perp.set_perp(&c1, 0.0, &c1[0], &c2);
                if perp.is_match() {
                    intersections.insert(0.0, perp.perp_t(), &perp.perp_pt());
                }
            }
            if (*sect1).removed_end_t && (zero_one_set & ONE_S1_SET) == 0 {
                let mut perp = TCoincident::default();
                perp.set_perp(&c1, 1.0, &(*sect1).point_last(), &c2);
                if perp.is_match() {
                    intersections.insert(1.0, perp.perp_t(), &perp.perp_pt());
                }
            }
            if (*sect2).removed_start_t && (zero_one_set & ZERO_S2_SET) == 0 {
                let mut perp = TCoincident::default();
                perp.set_perp(&c2, 0.0, &c2[0], &c1);
                if perp.is_match() {
                    intersections.insert(perp.perp_t(), 0.0, &perp.perp_pt());
                }
            }
            if (*sect2).removed_end_t && (zero_one_set & ONE_S2_SET) == 0 {
                let mut perp = TCoincident::default();
                perp.set_perp(&c2, 1.0, &(*sect2).point_last(), &c1);
                if perp.is_match() {
                    intersections.insert(perp.perp_t(), 1.0, &perp.perp_pt());
                }
            }
            if (*sect1).head.is_null() || (*sect2).head.is_null() {
                return;
            }
            (*sect1).recover_collapsed();
            (*sect2).recover_collapsed();
            let mut result1 = (*sect1).head;
            // check heads and tails for zero and ones and insert them if we haven't already done so
            let head1 = result1;
            if (zero_one_set & ZERO_S1_SET) == 0 && approximately_less_than_zero((*head1).start_t) {
                let start1 = c1[0];
                if (*head1).is_bounded() {
                    let t = (*head1).closest_bounded_t(&start1);
                    if c2.pt_at_t(t).approximately_equal(&start1) {
                        intersections.insert(0.0, t, &start1);
                    }
                }
            }
            let head2 = (*sect2).head;
            if (zero_one_set & ZERO_S2_SET) == 0 && approximately_less_than_zero((*head2).start_t) {
                let start2 = c2[0];
                if (*head2).is_bounded() {
                    let t = (*head2).closest_bounded_t(&start2);
                    if c1.pt_at_t(t).approximately_equal(&start2) {
                        intersections.insert(t, 0.0, &start2);
                    }
                }
            }
            if (zero_one_set & ONE_S1_SET) == 0 {
                let tail1 = (*sect1).tail();
                if tail1.is_null() {
                    return;
                }
                if approximately_greater_than_one((*tail1).end_t) {
                    let end1 = (*sect1).point_last();
                    if (*tail1).is_bounded() {
                        let t = (*tail1).closest_bounded_t(&end1);
                        if c2.pt_at_t(t).approximately_equal(&end1) {
                            intersections.insert(1.0, t, &end1);
                        }
                    }
                }
            }
            if (zero_one_set & ONE_S2_SET) == 0 {
                let tail2 = (*sect2).tail();
                if tail2.is_null() {
                    return;
                }
                if approximately_greater_than_one((*tail2).end_t) {
                    let end2 = (*sect2).point_last();
                    if (*tail2).is_bounded() {
                        let t = (*tail2).closest_bounded_t(&end2);
                        if c1.pt_at_t(t).approximately_equal(&end2) {
                            intersections.insert(t, 1.0, &end2);
                        }
                    }
                }
            }
            let mut closest = ClosestSect::new();
            loop {
                while !result1.is_null() && (*result1).coin_start.is_match() && (*result1).coin_end.is_match() {
                    result1 = (*result1).next;
                }
                if result1.is_null() {
                    break;
                }
                let mut result2 = (*sect2).head;
                while !result2.is_null() {
                    closest.find(result1, result2);
                    result2 = (*result2).next;
                }
                result1 = (*result1).next;
                if result1.is_null() {
                    break;
                }
            }
            closest.finish(intersections);
            // if there is more than one intersection and it isn't already coincident, check
            let mut last = intersections.used() - 1;
            let mut index = 0;
            while index < last {
                if intersections.is_coincident(index as usize) && intersections.is_coincident(index as usize + 1) {
                    index += 1;
                    continue;
                }
                let mid_t = (intersections.t(0, index as usize) + intersections.t(0, index as usize + 1)) / 2.0;
                let mid_pt = c1.pt_at_t(mid_t);
                // intersect perpendicular with opposite curve
                let mut perp = TCoincident::default();
                perp.set_perp(&c1, mid_t, &mid_pt, &c2);
                if !perp.is_match() {
                    index += 1;
                    continue;
                }
                if intersections.is_coincident(index as usize) {
                    intersections.remove_one(index as usize);
                    last -= 1;
                } else if intersections.is_coincident(index as usize + 1) {
                    intersections.remove_one(index as usize + 1);
                    last -= 1;
                } else {
                    intersections.set_coincident(index as usize);
                    index += 1;
                }
                intersections.set_coincident(index as usize);
            }
        }
    }
}

fn is_parallel(this_line: &DLine, opp: &TCurve) -> bool {
    if !opp.is_conic() {
        return false; // FIXME : breaks a lot of stuff now
    }
    let mut finds = 0;
    let mut this_perp = DLine::default();
    this_perp.pts[0].x = this_line.pts[1].x + (this_line.pts[1].y - this_line.pts[0].y);
    this_perp.pts[0].y = this_line.pts[1].y + (this_line.pts[0].x - this_line.pts[1].x);
    this_perp.pts[1] = this_line.pts[1];
    let mut perp_ray_i = Intersections::default();
    opp.intersect_ray(&mut perp_ray_i, &this_perp);
    for p_index in 0..perp_ray_i.used() as usize {
        finds += perp_ray_i.pt(p_index).approximately_equal(&this_perp.pts[1]) as i32;
    }
    this_perp.pts[1].x = this_line.pts[0].x + (this_line.pts[1].y - this_line.pts[0].y);
    this_perp.pts[1].y = this_line.pts[0].y + (this_line.pts[0].x - this_line.pts[1].x);
    this_perp.pts[0] = this_line.pts[0];
    opp.intersect_ray(&mut perp_ray_i, &this_perp);
    for p_index in 0..perp_ray_i.used() as usize {
        finds += perp_ray_i.pt(p_index).approximately_equal(&this_perp.pts[0]) as i32;
    }
    finds >= 2
}

/// `SkClosestRecord`.
#[derive(Clone, Copy)]
struct ClosestRecord {
    c1_span: *const TSpan,
    c2_span: *const TSpan,
    c1_start_t: f64,
    c1_end_t: f64,
    c2_start_t: f64,
    c2_end_t: f64,
    closest: f64,
    c1_index: usize,
    c2_index: usize,
}

impl ClosestRecord {
    fn new() -> Self {
        ClosestRecord {
            c1_span: std::ptr::null(),
            c2_span: std::ptr::null(),
            c1_start_t: 0.0,
            c1_end_t: 0.0,
            c2_start_t: 0.0,
            c2_end_t: 0.0,
            closest: f32::MAX as f64,
            c1_index: 0,
            c2_index: 0,
        }
    }

    unsafe fn add_intersection(&self, intersections: &mut Intersections) {
        unsafe {
            let r1t = if self.c1_index != 0 { (*self.c1_span).end_t } else { (*self.c1_span).start_t };
            let r2t = if self.c2_index != 0 { (*self.c2_span).end_t } else { (*self.c2_span).start_t };
            let part: &TCurve = &(*self.c1_span).part;
            intersections.insert(r1t, r2t, &part[self.c1_index]);
        }
    }

    unsafe fn find_end(&mut self, span1: *const TSpan, span2: *const TSpan, c1_index: usize, c2_index: usize) {
        unsafe {
            let c1 = &(*span1).part;
            let c2 = &(*span2).part;
            if !c1[c1_index].approximately_equal(&c2[c2_index]) {
                return;
            }
            let dist = c1[c1_index].distance_squared(&c2[c2_index]);
            if self.closest < dist {
                return;
            }
            self.c1_span = span1;
            self.c2_span = span2;
            self.c1_start_t = (*span1).start_t;
            self.c1_end_t = (*span1).end_t;
            self.c2_start_t = (*span2).start_t;
            self.c2_end_t = (*span2).end_t;
            self.c1_index = c1_index;
            self.c2_index = c2_index;
            self.closest = dist;
        }
    }

    unsafe fn mates_with(&self, mate: &ClosestRecord) -> bool {
        unsafe {
            self.c1_span == mate.c1_span
                || (*self.c1_span).end_t == (*mate.c1_span).start_t
                || (*self.c1_span).start_t == (*mate.c1_span).end_t
                || self.c2_span == mate.c2_span
                || (*self.c2_span).end_t == (*mate.c2_span).start_t
                || (*self.c2_span).start_t == (*mate.c2_span).end_t
        }
    }

    fn merge(&mut self, mate: &ClosestRecord) {
        self.c1_span = mate.c1_span;
        self.c2_span = mate.c2_span;
        self.closest = mate.closest;
        self.c1_index = mate.c1_index;
        self.c2_index = mate.c2_index;
    }

    fn reset(&mut self) {
        self.closest = f32::MAX as f64;
    }

    fn update(&mut self, mate: &ClosestRecord) {
        self.c1_start_t = smin(self.c1_start_t, mate.c1_start_t);
        self.c1_end_t = smax(self.c1_end_t, mate.c1_end_t);
        self.c2_start_t = smin(self.c2_start_t, mate.c2_start_t);
        self.c2_end_t = smax(self.c2_end_t, mate.c2_end_t);
    }
}

/// `SkClosestSect`.
struct ClosestSect {
    closest: Vec<ClosestRecord>,
    used: usize,
}

impl ClosestSect {
    fn new() -> Self {
        ClosestSect { closest: vec![ClosestRecord::new()], used: 0 }
    }

    unsafe fn find(&mut self, span1: *const TSpan, span2: *const TSpan) -> bool {
        unsafe {
            let used = self.used;
            let pl1 = (*span1).part.point_last();
            let pl2 = (*span2).part.point_last();
            {
                let record = &mut self.closest[used];
                record.find_end(span1, span2, 0, 0);
                record.find_end(span1, span2, 0, pl2);
                record.find_end(span1, span2, pl1, 0);
                record.find_end(span1, span2, pl1, pl2);
                if record.closest == f32::MAX as f64 {
                    return false;
                }
            }
            let record = self.closest[used];
            for index in 0..used {
                if self.closest[index].mates_with(&record) {
                    let test = &mut self.closest[index];
                    if test.closest > record.closest {
                        test.merge(&record);
                    }
                    test.update(&record);
                    self.closest[used].reset();
                    return false;
                }
            }
        }
        self.used += 1;
        let mut r = ClosestRecord::new();
        r.reset();
        self.closest.push(r);
        true
    }

    unsafe fn finish(&self, intersections: &mut Intersections) {
        let mut closest_ptrs: Vec<usize> = (0..self.used).collect();
        super::sort::tq_sort(&mut closest_ptrs, |&a, &b| self.closest[a].closest < self.closest[b].closest);
        for index in 0..self.used {
            unsafe { self.closest[closest_ptrs[index]].add_intersection(intersections) };
        }
    }
}

impl Intersections {
    fn intersect_tcurves(&mut self, a: TCurve, b: TCurve) -> i32 {
        let mut sect1 = TSect::new(a);
        let mut sect2 = TSect::new(b);
        TSect::binary_search(&mut sect1, &mut sect2, self);
        self.used()
    }

    pub fn intersect_quad_quad(&mut self, q1: &DQuad, q2: &DQuad) -> i32 {
        self.intersect_tcurves(TCurve::Quad(*q1), TCurve::Quad(*q2))
    }

    pub fn intersect_conic_quad(&mut self, c: &DConic, q: &DQuad) -> i32 {
        self.intersect_tcurves(TCurve::Conic(*c), TCurve::Quad(*q))
    }

    pub fn intersect_conic_conic(&mut self, c1: &DConic, c2: &DConic) -> i32 {
        self.intersect_tcurves(TCurve::Conic(*c1), TCurve::Conic(*c2))
    }

    pub fn intersect_cubic_quad(&mut self, c: &DCubic, q: &DQuad) -> i32 {
        self.intersect_tcurves(TCurve::Cubic(*c), TCurve::Quad(*q))
    }

    pub fn intersect_cubic_conic(&mut self, cu: &DCubic, co: &DConic) -> i32 {
        self.intersect_tcurves(TCurve::Cubic(*cu), TCurve::Conic(*co))
    }

    pub fn intersect_cubic_cubic(&mut self, c1: &DCubic, c2: &DCubic) -> i32 {
        self.intersect_tcurves(TCurve::Cubic(*c1), TCurve::Cubic(*c2))
    }
}
