//! `SkOpSegment.h`, `SkOpSegment.cpp`: one curve of a contour, its spans
//! (the curve cut at every intersection) and the winding bookkeeping on them.

use super::angle::IncludeType;
use super::conic::DConic;
use super::cubic::DCubic;
use super::curve::*;
use super::graph::*;
use super::intersections::Intersections;
use super::line::DLine;
use super::point::{DPoint, DVector};
use super::quad::DQuad;
use super::rect::PathOpsBounds;
use super::span::Collapsed;
use super::types::*;
use super::writer::PathWriter;
use super::PathOp;
use crate::geom::Point;
use crate::path::Verb;

const F: bool = false; // discard the edge
const T: bool = true; // keep the edge

static UNARY_ACTIVE_EDGE: [[bool; 2]; 2] = [
    //  from=0  from=1
    //  to=0,1  to=0,1
    [F, T],
    [T, F],
];

#[rustfmt::skip]
static ACTIVE_EDGE: [[[[[bool; 2]; 2]; 2]; 2]; 4] = [
//                 miFrom=0                              miFrom=1
//         miTo=0             miTo=1             miTo=0             miTo=1
//     suFrom=0    1      suFrom=0    1      suFrom=0    1      suFrom=0    1
//   suTo=0,1 suTo=0,1  suTo=0,1 suTo=0,1  suTo=0,1 suTo=0,1  suTo=0,1 suTo=0,1
    [[[[F, F], [F, F]], [[T, F], [T, F]]], [[[T, T], [F, F]], [[F, T], [T, F]]]],  // mi - su
    [[[[F, F], [F, F]], [[F, T], [F, T]]], [[[F, F], [T, T]], [[F, T], [T, F]]]],  // mi & su
    [[[[F, T], [T, F]], [[T, T], [F, F]]], [[[T, F], [T, F]], [[F, F], [F, F]]]],  // mi | su
    [[[[F, T], [T, F]], [[T, F], [F, T]]], [[[T, F], [F, T]], [[F, T], [T, F]]]],  // mi ^ su
];

/// `SkPointPriv::DistanceToSqd`.
fn distance_to_sqd(pt: Point, a: Point) -> f32 {
    let dx = pt.x - a.x;
    let dy = pt.y - a.y;
    dx * dx + dy * dy
}

impl Segment {
    pub fn active_angle(self, start: Span, start_ptr: &mut Span, end_ptr: &mut Span, done: &mut bool) -> Angle {
        let result = self.active_angle_inner(start, start_ptr, end_ptr, done);
        if result.is_some() {
            return result;
        }
        let result = self.active_angle_other(start, start_ptr, end_ptr, done);
        if result.is_some() {
            return result;
        }
        Angle::NULL
    }

    pub fn active_angle_inner(self, start: Span, start_ptr: &mut Span, end_ptr: &mut Span, done: &mut bool) -> Angle {
        let up_span = start.up_castable();
        if up_span.is_some() && (up_span.wind_value() != 0 || up_span.opp_value() != 0) {
            let next = up_span.next();
            if end_ptr.is_null() {
                *start_ptr = start;
                *end_ptr = next;
            }
            if !up_span.done() {
                if up_span.wind_sum() != SK_MIN_S32 {
                    return self.span_to_angle(start, next);
                }
                *done = false;
            }
        }
        let down_span = start.prev();
        // edge leading into junction
        if down_span.is_some() && (down_span.wind_value() != 0 || down_span.opp_value() != 0) {
            if end_ptr.is_null() {
                *start_ptr = start;
                *end_ptr = down_span;
            }
            if !down_span.done() {
                if down_span.wind_sum() != SK_MIN_S32 {
                    return self.span_to_angle(start, down_span);
                }
                *done = false;
            }
        }
        Angle::NULL
    }

    pub fn active_angle_other(self, start: Span, start_ptr: &mut Span, end_ptr: &mut Span, done: &mut bool) -> Angle {
        let o_pt_t = start.pt_t().next();
        let other = o_pt_t.segment();
        let o_span = o_pt_t.span();
        other.active_angle_inner(o_span, start_ptr, end_ptr, done)
    }

    pub fn active_op(self, start: Span, end: Span, xor_mi_mask: i32, xor_su_mask: i32, op: PathOp) -> bool {
        let mut sum_mi_winding = self.update_winding(end, start);
        let mut sum_su_winding = self.update_opp_winding(end, start);
        if self.operand() {
            std::mem::swap(&mut sum_mi_winding, &mut sum_su_winding);
        }
        self.active_op_sums(xor_mi_mask, xor_su_mask, start, end, op, &mut sum_mi_winding, &mut sum_su_winding)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn active_op_sums(
        self,
        xor_mi_mask: i32,
        xor_su_mask: i32,
        start: Span,
        end: Span,
        op: PathOp,
        sum_mi_winding: &mut i32,
        sum_su_winding: &mut i32,
    ) -> bool {
        let (mut max_winding, mut sum_winding, mut opp_max_winding, mut opp_sum_winding) = (0, 0, 0, 0);
        self.set_up_windings_op(
            start,
            end,
            sum_mi_winding,
            sum_su_winding,
            &mut max_winding,
            &mut sum_winding,
            &mut opp_max_winding,
            &mut opp_sum_winding,
        );
        let (mi_from, mi_to, su_from, su_to);
        if self.operand() {
            mi_from = (opp_max_winding & xor_mi_mask) != 0;
            mi_to = (opp_sum_winding & xor_mi_mask) != 0;
            su_from = (max_winding & xor_su_mask) != 0;
            su_to = (sum_winding & xor_su_mask) != 0;
        } else {
            mi_from = (max_winding & xor_mi_mask) != 0;
            mi_to = (sum_winding & xor_mi_mask) != 0;
            su_from = (opp_max_winding & xor_su_mask) != 0;
            su_to = (opp_sum_winding & xor_su_mask) != 0;
        }
        ACTIVE_EDGE[op as usize][mi_from as usize][mi_to as usize][su_from as usize][su_to as usize]
    }

    pub fn active_winding(self, start: Span, end: Span) -> bool {
        let mut sum_winding = self.update_winding(end, start);
        self.active_winding_sum(start, end, &mut sum_winding)
    }

    pub fn active_winding_sum(self, start: Span, end: Span, sum_winding: &mut i32) -> bool {
        let mut max_winding = 0;
        self.set_up_winding(start, end, &mut max_winding, sum_winding);
        let from = max_winding != 0;
        let to = *sum_winding != 0;
        UNARY_ACTIVE_EDGE[from as usize][to as usize]
    }

    pub fn add_conic(self, pts: &[Point], weight: f32, parent: Contour) -> Segment {
        self.init(pts, weight, parent, Verb::Conic);
        let mut curve = DCurve::default();
        curve.set_conic(&DConic::from_pts(pts, weight));
        let mut bounds = PathOpsBounds::default();
        curve.set_conic_bounds(pts, weight, 0.0, 1.0, &mut bounds);
        self.d().bounds = bounds;
        self
    }

    pub fn add_cubic(self, pts: &[Point], parent: Contour) -> Segment {
        self.init(pts, 1.0, parent, Verb::Cubic);
        let mut curve = DCurve::default();
        curve.set_cubic(&DCubic::from_pts(pts));
        let mut bounds = PathOpsBounds::default();
        curve.set_cubic_bounds(pts, 1.0, 0.0, 1.0, &mut bounds);
        self.d().bounds = bounds;
        self
    }

    pub fn add_curve_to(self, start: Span, end: Span, path: &mut PathWriter) -> bool {
        let span_start = start.starter(end);
        if span_start.already_added() {
            return false;
        }
        span_start.mark_added();
        let mut curve_part = DCurveSweep::default();
        start.segment().sub_divide(start, end, &mut curve_part.curve);
        let verb_self = self.verb();
        curve_part.set_curve_hull_sweep(verb_self);
        let verb = if curve_part.is_curve() { verb_self } else { Verb::Line };
        path.deferred_move(start.pt_t());
        match verb {
            Verb::Line => {
                if !path.deferred_line(end.pt_t()) {
                    return false;
                }
            }
            Verb::Quad => path.quad_to(curve_part.curve[1].as_sk_point(), end.pt_t()),
            Verb::Conic => path.conic_to(curve_part.curve[1].as_sk_point(), end.pt_t(), curve_part.curve.weight),
            Verb::Cubic => {
                path.cubic_to(curve_part.curve[1].as_sk_point(), curve_part.curve[2].as_sk_point(), end.pt_t())
            }
            _ => {}
        }
        true
    }

    pub fn add_end_span(self) -> Angle {
        let angle = Angle(self.global_state().allocator.make(AngleData::default()));
        let tail = self.tail();
        angle.set(tail, tail.prev());
        tail.set_from_angle(angle);
        angle
    }

    /// Break the span so that the coincident part does not change the angle
    /// of the remainder.
    pub fn add_expanded(self, new_t: f64, test: Span, start_over: &mut bool) -> bool {
        if self.contains(new_t) {
            return true;
        }
        self.global_state().allocated_op_span = false;
        if !between(0.0, new_t, 1.0) {
            return false;
        }
        let new_pt_t = self.add_t(new_t);
        *start_over |= self.global_state().allocated_op_span;
        if new_pt_t.is_null() {
            return false;
        }
        new_pt_t.d().pt = self.pt_at_t(new_t);
        let opp_prev = test.pt_t().opp_prev(new_pt_t);
        if opp_prev.is_some() {
            // const cast away to change linked list; pt/t values stays unchanged
            test.merge_matches(new_pt_t.span());
            test.pt_t().add_opp(new_pt_t, opp_prev);
            test.check_for_collapsed_coincidence();
        }
        true
    }

    pub fn add_line(self, pts: &[Point], parent: Contour) -> Segment {
        self.init(pts, 1.0, parent, Verb::Line);
        self.d().bounds.set_bounds(&pts[..2]);
        self
    }

    pub fn add_start_span(self) -> Angle {
        let angle = Angle(self.global_state().allocator.make(AngleData::default()));
        let head = self.head();
        angle.set(head, head.next());
        head.set_to_angle(angle);
        angle
    }

    pub fn add_quad(self, pts: &[Point], parent: Contour) -> Segment {
        self.init(pts, 1.0, parent, Verb::Quad);
        let mut curve = DCurve::default();
        curve.set_quad(&DQuad::from_pts(pts));
        let mut bounds = PathOpsBounds::default();
        curve.set_quad_bounds(pts, 1.0, 0.0, 1.0, &mut bounds);
        self.d().bounds = bounds;
        self
    }

    /// Please keep this in sync with debugAddT().
    pub fn add_t_pt(self, t: f64, pt: Point) -> PtT {
        let mut span_base = self.head();
        loop {
            let result = span_base.pt_t();
            if t == result.t() || (!zero_or_one(t) && self.match_(result, self, t, pt)) {
                span_base.bump_span_adds();
                return result;
            }
            if t < result.t() {
                let prev = result.span().prev();
                if prev.is_null() {
                    return PtT::NULL;
                }
                // marks in global state that new op span has been allocated
                let span = self.insert(prev);
                span.init(self, prev, t, pt);
                span.bump_span_adds();
                return span.pt_t();
            }
            if span_base == self.tail() {
                return PtT::NULL;
            }
            span_base = span_base.up_cast().next();
            if span_base.is_null() {
                break;
            }
        }
        PtT::NULL
    }

    pub fn add_t(self, t: f64) -> PtT {
        self.add_t_pt(t, self.pt_at_t(t))
    }

    #[inline]
    pub fn bounds(self) -> PathOpsBounds {
        self.d().bounds
    }

    pub fn bump_count(self) {
        self.d().count += 1;
    }

    pub fn calc_angles(self) {
        let head = self.head();
        let tail = self.tail();
        let mut active_prior = !head.is_canceled();
        if active_prior && !head.simple() {
            self.add_start_span();
        }
        let mut prior = head;
        let mut span_base = head.next();
        while span_base != tail {
            if active_prior {
                let prior_angle = Angle(self.global_state().allocator.make(AngleData::default()));
                prior_angle.set(span_base, prior);
                span_base.set_from_angle(prior_angle);
            }
            let span = span_base.up_cast();
            let active = !span.is_canceled();
            let next = span.next();
            if active {
                let angle = Angle(self.global_state().allocator.make(AngleData::default()));
                angle.set(span, next);
                span.set_to_angle(angle);
            }
            active_prior = active;
            prior = span;
            span_base = next;
        }
        if active_prior && !tail.simple() {
            self.add_end_span();
        }
    }

    /// Please keep this in sync with debugClearAll().
    pub fn clear_all(self) {
        let mut span = self.head();
        loop {
            self.clear_one(span);
            span = span.next().up_castable();
            if span.is_null() {
                break;
            }
        }
        unsafe { (*self.global_state().coincidence).release_seg(self) };
    }

    /// Please keep this in sync with debugClearOne().
    pub fn clear_one(self, span: Span) {
        span.set_wind_value(0);
        span.set_opp_value(0);
        self.mark_done(span);
    }

    pub fn collapsed(self, s: f64, e: f64) -> Collapsed {
        let mut span = self.head();
        loop {
            let result = span.collapsed(s, e);
            if Collapsed::No != result {
                return result;
            }
            if span.up_castable().is_null() {
                break;
            }
            span = span.up_cast().next();
            if span.is_null() {
                break;
            }
        }
        Collapsed::No
    }

    #[allow(non_snake_case)]
    pub fn ComputeOneSum(base_angle: Angle, next_angle: Angle, include_type: IncludeType) -> bool {
        let base_segment = base_angle.segment();
        let mut sum_mi_winding = base_segment.update_winding_reverse(base_angle);
        let mut sum_su_winding = 0;
        let binary = include_type >= IncludeType::BinarySingle;
        if binary {
            sum_su_winding = base_segment.update_opp_winding_reverse(base_angle);
            if base_segment.operand() {
                std::mem::swap(&mut sum_mi_winding, &mut sum_su_winding);
            }
        }
        let next_segment = next_angle.segment();
        let (mut max_winding, mut sum_winding) = (0, 0);
        let mut last = Span::NULL;
        if binary {
            let (mut opp_max_winding, mut opp_sum_winding) = (0, 0);
            next_segment.set_up_windings_op(
                next_angle.start(),
                next_angle.end(),
                &mut sum_mi_winding,
                &mut sum_su_winding,
                &mut max_winding,
                &mut sum_winding,
                &mut opp_max_winding,
                &mut opp_sum_winding,
            );
            if !next_segment.mark_angle_op(
                max_winding,
                sum_winding,
                opp_max_winding,
                opp_sum_winding,
                next_angle,
                Some(&mut last),
            ) {
                return false;
            }
        } else {
            next_segment.set_up_windings(
                next_angle.start(),
                next_angle.end(),
                &mut sum_mi_winding,
                &mut max_winding,
                &mut sum_winding,
            );
            if !next_segment.mark_angle(max_winding, sum_winding, next_angle, Some(&mut last)) {
                return false;
            }
        }
        next_angle.set_last_marked(last);
        true
    }

    #[allow(non_snake_case)]
    pub fn ComputeOneSumReverse(base_angle: Angle, next_angle: Angle, include_type: IncludeType) -> bool {
        let base_segment = base_angle.segment();
        let mut sum_mi_winding = base_segment.update_winding_angle(base_angle);
        let mut sum_su_winding = 0;
        let binary = include_type >= IncludeType::BinarySingle;
        if binary {
            sum_su_winding = base_segment.update_opp_winding_angle(base_angle);
            if base_segment.operand() {
                std::mem::swap(&mut sum_mi_winding, &mut sum_su_winding);
            }
        }
        let next_segment = next_angle.segment();
        let (mut max_winding, mut sum_winding) = (0, 0);
        let mut last = Span::NULL;
        if binary {
            let (mut opp_max_winding, mut opp_sum_winding) = (0, 0);
            next_segment.set_up_windings_op(
                next_angle.end(),
                next_angle.start(),
                &mut sum_mi_winding,
                &mut sum_su_winding,
                &mut max_winding,
                &mut sum_winding,
                &mut opp_max_winding,
                &mut opp_sum_winding,
            );
            if !next_segment.mark_angle_op(
                max_winding,
                sum_winding,
                opp_max_winding,
                opp_sum_winding,
                next_angle,
                Some(&mut last),
            ) {
                return false;
            }
        } else {
            next_segment.set_up_windings(
                next_angle.end(),
                next_angle.start(),
                &mut sum_mi_winding,
                &mut max_winding,
                &mut sum_winding,
            );
            if !next_segment.mark_angle(max_winding, sum_winding, next_angle, Some(&mut last)) {
                return false;
            }
        }
        next_angle.set_last_marked(last);
        true
    }

    /// At this point, the span is already ordered, or unorderable.
    pub fn compute_sum(self, start: Span, end: Span, include_type: IncludeType) -> i32 {
        let mut first_angle = self.span_to_angle(end, start);
        if first_angle.is_null() || first_angle.next().is_null() {
            return SK_NAN32;
        }
        // if all angles have a computed winding,
        //  or if no adjacent angles are orderable,
        //  or if adjacent orderable angles have no computed winding,
        //  there's nothing to do
        // if two orderable angles are adjacent, and both are next to orderable angles,
        //  and one has winding computed, transfer to the other
        let mut base_angle = Angle::NULL;
        let mut try_reverse = false;
        // look for counterclockwise transfers
        let mut angle = first_angle.previous();
        let mut next = angle.next();
        first_angle = next;
        loop {
            let prior = angle;
            angle = next;
            next = angle.next();
            'body: {
                if prior.unorderable() || angle.unorderable() || next.unorderable() {
                    base_angle = Angle::NULL;
                    break 'body;
                }
                let test_winding = angle.starter().wind_sum();
                if SK_MIN_S32 != test_winding {
                    base_angle = angle;
                    try_reverse = true;
                    break 'body;
                }
                if base_angle.is_some() {
                    Segment::ComputeOneSum(base_angle, angle, include_type);
                    base_angle = if SK_MIN_S32 != angle.starter().wind_sum() { angle } else { Angle::NULL };
                }
            }
            if next == first_angle {
                break;
            }
        }
        if base_angle.is_some() && SK_MIN_S32 == first_angle.starter().wind_sum() {
            first_angle = base_angle;
            try_reverse = true;
        }
        if try_reverse {
            base_angle = Angle::NULL;
            let mut prior = first_angle;
            loop {
                angle = prior;
                prior = angle.previous();
                next = angle.next();
                'body: {
                    if prior.unorderable() || angle.unorderable() || next.unorderable() {
                        base_angle = Angle::NULL;
                        break 'body;
                    }
                    let test_winding = angle.starter().wind_sum();
                    if SK_MIN_S32 != test_winding {
                        base_angle = angle;
                        break 'body;
                    }
                    if base_angle.is_some() {
                        Segment::ComputeOneSumReverse(base_angle, angle, include_type);
                        base_angle = if SK_MIN_S32 != angle.starter().wind_sum() { angle } else { Angle::NULL };
                    }
                }
                if prior == first_angle {
                    break;
                }
            }
        }
        start.starter(end).wind_sum()
    }

    pub fn contains(self, new_t: f64) -> bool {
        let mut span_base = self.head();
        loop {
            if span_base.pt_t().contains_seg_t(self, new_t) {
                return true;
            }
            if span_base == self.tail() {
                break;
            }
            span_base = span_base.up_cast().next();
        }
        false
    }

    #[inline]
    pub fn contour(self) -> Contour {
        self.d().contour
    }

    #[inline]
    pub fn count(self) -> i32 {
        self.d().count
    }

    #[inline]
    pub fn done(self) -> bool {
        self.d().done_count == self.d().count
    }

    pub fn done_angle(self, angle: Angle) -> bool {
        angle.start().starter(angle.end()).done()
    }

    pub fn d_pt_at_t(self, mid: f64) -> DPoint {
        let d = self.d();
        curve_d_point_at_t(d.verb, &d.pts, d.weight, mid)
    }

    pub fn d_slope_at_t(self, mid: f64) -> DVector {
        let d = self.d();
        curve_d_slope_at_t(d.verb, &d.pts, d.weight, mid)
    }

    pub fn existing(self, t: f64, opp: Segment) -> PtT {
        let mut test = self.head();
        let mut test_pt_t;
        let pt = self.pt_at_t(t);
        'found_match: {
            loop {
                test_pt_t = test.pt_t();
                if test_pt_t.t() == t {
                    break 'found_match;
                }
                if !self.match_(test_pt_t, self, t, pt) {
                    if t < test_pt_t.t() {
                        return PtT::NULL;
                    }
                } else {
                    if opp.is_null() {
                        return test_pt_t;
                    }
                    let mut loop_ = test_pt_t.next();
                    while loop_ != test_pt_t {
                        if loop_.segment() == self && loop_.t() == t && loop_.pt() == pt {
                            break 'found_match;
                        }
                        loop_ = loop_.next();
                    }
                    return PtT::NULL;
                }
                test = test.up_cast().next();
                if test.is_null() {
                    break;
                }
            }
        }
        // foundMatch:
        if opp.is_some() && test.contains_seg(opp).is_null() {
            PtT::NULL
        } else {
            test_pt_t
        }
    }

    /// The M and S variable name parts stand for the operators: Mi for
    /// Minuend, Su for Subtrahend; Opp values result from combining
    /// coincident spans.
    #[allow(clippy::too_many_arguments)]
    pub fn find_next_op(
        self,
        chase: &mut Vec<Span>,
        next_start: &mut Span,
        next_end: &mut Span,
        unsortable: &mut bool,
        simple: &mut bool,
        op: PathOp,
        xor_mi_mask: i32,
        xor_su_mask: i32,
    ) -> Segment {
        let start = *next_start;
        let end = *next_end;
        let mut step = start.step(end);
        let other = self.is_simple(next_start, &mut step); // advances nextStart
        *simple = other.is_some();
        if *simple {
            // mark the smaller of startIndex, endIndex done, and all adjacent
            // spans with the same T value (but not 'other' spans)
            let start_span = start.starter(end);
            if start_span.done() {
                return Segment::NULL;
            }
            self.mark_done(start_span);
            *next_end = if step > 0 { next_start.up_cast().next() } else { next_start.prev() };
            return other;
        }
        let end_near = if step > 0 { next_start.up_cast().next() } else { next_start.prev() };
        // more than one viable candidate -- measure angles to find best
        let calc_winding = self.compute_sum(start, end_near, IncludeType::BinaryOpp);
        let sortable = calc_winding != SK_NAN32;
        if !sortable {
            *unsortable = true;
            self.mark_done(start.starter(end));
            return Segment::NULL;
        }
        let angle = self.span_to_angle(end, start);
        if angle.unorderable() {
            *unsortable = true;
            self.mark_done(start.starter(end));
            return Segment::NULL;
        }
        let mut sum_mi_winding = self.update_winding(end, start);
        if sum_mi_winding == SK_MIN_S32 {
            *unsortable = true;
            self.mark_done(start.starter(end));
            return Segment::NULL;
        }
        let mut sum_su_winding = self.update_opp_winding(end, start);
        if self.operand() {
            std::mem::swap(&mut sum_mi_winding, &mut sum_su_winding);
        }
        let mut next_angle = angle.next();
        let mut found_angle = Angle::NULL;
        let mut found_done = false;
        // iterate through the angle, and compute everyone's winding
        let mut next_segment;
        let mut active_count = 0;
        loop {
            next_segment = next_angle.segment();
            let active_angle = next_segment.active_op_sums(
                xor_mi_mask,
                xor_su_mask,
                next_angle.start(),
                next_angle.end(),
                op,
                &mut sum_mi_winding,
                &mut sum_su_winding,
            );
            if active_angle {
                active_count += 1;
                if found_angle.is_null() || (found_done && (active_count & 1) != 0) {
                    found_angle = next_angle;
                    found_done = next_segment.done_angle(next_angle);
                }
            }
            if !next_segment.done() {
                if !active_angle {
                    let _ = next_segment.mark_and_chase_done(next_angle.start(), next_angle.end(), None);
                }
                let last = next_angle.last_marked();
                if last.is_some() {
                    chase.push(last);
                }
            }
            next_angle = next_angle.next();
            if next_angle == angle {
                break;
            }
        }
        start.segment().mark_done(start.starter(end));
        if found_angle.is_null() {
            return Segment::NULL;
        }
        *next_start = found_angle.start();
        *next_end = found_angle.end();
        found_angle.segment()
    }

    pub fn find_next_winding(
        self,
        chase: &mut Vec<Span>,
        next_start: &mut Span,
        next_end: &mut Span,
        unsortable: &mut bool,
    ) -> Segment {
        let start = *next_start;
        let end = *next_end;
        let mut step = start.step(end);
        let other = self.is_simple(next_start, &mut step); // advances nextStart
        if other.is_some() {
            let start_span = start.starter(end);
            if start_span.done() {
                return Segment::NULL;
            }
            self.mark_done(start_span);
            *next_end = if step > 0 { next_start.up_cast().next() } else { next_start.prev() };
            return other;
        }
        let end_near = if step > 0 { next_start.up_cast().next() } else { next_start.prev() };
        let calc_winding = self.compute_sum(start, end_near, IncludeType::UnaryWinding);
        let sortable = calc_winding != SK_NAN32;
        if !sortable {
            *unsortable = true;
            self.mark_done(start.starter(end));
            return Segment::NULL;
        }
        let angle = self.span_to_angle(end, start);
        if angle.unorderable() {
            *unsortable = true;
            self.mark_done(start.starter(end));
            return Segment::NULL;
        }
        let mut sum_winding = self.update_winding(end, start);
        let mut next_angle = angle.next();
        let mut found_angle = Angle::NULL;
        let mut found_done = false;
        let mut next_segment;
        let mut active_count = 0;
        loop {
            next_segment = next_angle.segment();
            let active_angle = next_segment.active_winding_sum(next_angle.start(), next_angle.end(), &mut sum_winding);
            if active_angle {
                active_count += 1;
                if found_angle.is_null() || (found_done && (active_count & 1) != 0) {
                    found_angle = next_angle;
                    found_done = next_segment.done_angle(next_angle);
                }
            }
            if !next_segment.done() {
                if !active_angle {
                    let _ = next_segment.mark_and_chase_done(next_angle.start(), next_angle.end(), None);
                }
                let last = next_angle.last_marked();
                if last.is_some() {
                    chase.push(last);
                }
            }
            next_angle = next_angle.next();
            if next_angle == angle {
                break;
            }
        }
        start.segment().mark_done(start.starter(end));
        if found_angle.is_null() {
            return Segment::NULL;
        }
        *next_start = found_angle.start();
        *next_end = found_angle.end();
        found_angle.segment()
    }

    pub fn find_next_xor(self, next_start: &mut Span, next_end: &mut Span, unsortable: &mut bool) -> Segment {
        let start = *next_start;
        let end = *next_end;
        let mut step = start.step(end);
        let other = self.is_simple(next_start, &mut step); // advances nextStart
        if other.is_some() {
            let start_span = start.starter(end);
            if start_span.done() {
                return Segment::NULL;
            }
            self.mark_done(start_span);
            *next_end = if step > 0 { next_start.up_cast().next() } else { next_start.prev() };
            return other;
        }
        let angle = self.span_to_angle(end, start);
        if angle.is_null() || angle.unorderable() {
            *unsortable = true;
            self.mark_done(start.starter(end));
            return Segment::NULL;
        }
        let mut next_angle = angle.next();
        let mut found_angle = Angle::NULL;
        let mut found_done = false;
        let mut active_count = 0;
        loop {
            if next_angle.is_null() {
                return Segment::NULL;
            }
            let next_segment = next_angle.segment();
            active_count += 1;
            if found_angle.is_null() || (found_done && (active_count & 1) != 0) {
                found_angle = next_angle;
                found_done = next_segment.done_angle(next_angle);
                if !found_done {
                    break;
                }
            }
            next_angle = next_angle.next();
            if next_angle == angle {
                break;
            }
        }
        start.segment().mark_done(start.starter(end));
        if found_angle.is_null() {
            return Segment::NULL;
        }
        *next_start = found_angle.start();
        *next_end = found_angle.end();
        found_angle.segment()
    }

    pub fn global_state(self) -> &'static mut GlobalState {
        self.contour().global_state()
    }

    pub fn init(self, pts: &[Point], weight: f32, contour: Contour, verb: Verb) {
        {
            let d = self.d();
            d.contour = contour;
            d.next = Segment::NULL;
            let n = verb_to_points(verb) + 1;
            d.pts[..n].copy_from_slice(&pts[..n]);
            d.weight = weight;
            d.verb = verb;
            d.count = 0;
            d.done_count = 0;
            d.visited = false;
        }
        let zero_span = self.head();
        zero_span.init(self, Span::NULL, 0.0, self.d().pts[0]);
        let one_span = self.tail();
        zero_span.set_next(one_span);
        let last = self.d().pts[verb_to_points(verb)];
        one_span.init_base(self, zero_span, 1.0, last);
    }

    pub fn insert(self, prev: Span) -> Span {
        let global_state = self.global_state();
        global_state.allocated_op_span = true;
        let result = Span(global_state.allocator.make(SpanData::default()));
        let next = prev.next();
        result.set_prev(prev);
        prev.set_next(result);
        result.set_next(next);
        if next.is_some() {
            next.set_prev(result);
        }
        result
    }

    pub fn is_close(self, t: f64, opp: Segment) -> bool {
        let c_pt = self.d_pt_at_t(t);
        let d = self.d();
        let dxdy = curve_d_slope_at_t(d.verb, &d.pts, d.weight, t);
        let perp = DLine::new(c_pt, DPoint::new(c_pt.x + dxdy.y, c_pt.y - dxdy.x));
        let mut i = Intersections::default();
        let od = opp.d();
        curve_intersect_ray(od.verb, &od.pts, od.weight, &perp, &mut i);
        let used = i.used();
        for index in 0..used as usize {
            if c_pt.roughly_equal(&i.pt(index)) {
                return true;
            }
        }
        false
    }

    pub fn is_horizontal(self) -> bool {
        self.d().bounds.top == self.d().bounds.bottom
    }

    pub fn is_simple(self, end: &mut Span, step: &mut i32) -> Segment {
        self.next_chase(end, step, None, None)
    }

    pub fn is_vertical(self) -> bool {
        self.d().bounds.left == self.d().bounds.right
    }

    pub fn is_vertical_span(self, start: Span, end: Span) -> bool {
        let d = self.d();
        curve_is_vertical(d.verb, &d.pts, d.weight, start.t(), end.t())
    }

    pub fn is_xor(self) -> bool {
        self.contour().d().xor
    }

    pub fn join_ends(self, start: Segment) {
        self.tail().pt_t().add_opp(start.head().pt_t(), start.head().pt_t());
    }

    pub fn last_pt(self) -> Point {
        let d = self.d();
        d.pts[verb_to_points(d.verb)]
    }

    pub fn mark_all_done(self) {
        let mut span = self.head();
        loop {
            self.mark_done(span);
            span = span.next().up_castable();
            if span.is_null() {
                break;
            }
        }
    }

    pub fn mark_and_chase_done(self, mut start: Span, end: Span, found: Option<&mut Span>) -> bool {
        let mut step = start.step(end);
        let mut min_span = start.starter(end);
        self.mark_done(min_span);
        let mut last = Span::NULL;
        let mut other = self;
        let mut prior_done = Span::NULL;
        let mut last_done = Span::NULL;
        let mut safety_net = 100000;
        loop {
            other = other.next_chase(&mut start, &mut step, Some(&mut min_span), Some(&mut last));
            if other.is_null() {
                break;
            }
            safety_net -= 1;
            if safety_net == 0 {
                return false;
            }
            if other.done() {
                break;
            }
            if last_done == min_span || prior_done == min_span {
                if let Some(found) = found {
                    *found = Span::NULL;
                }
                return true;
            }
            other.mark_done(min_span);
            prior_done = last_done;
            last_done = min_span;
        }
        if let Some(found) = found {
            *found = last;
        }
        true
    }

    pub fn mark_and_chase_winding(self, mut start: Span, end: Span, winding: i32, last_ptr: Option<&mut Span>) -> bool {
        let mut span_start = start.starter(end);
        let mut step = start.step(end);
        let success = self.mark_winding(span_start, winding);
        let mut last = Span::NULL;
        let mut other = self;
        let mut safety_net = 100000;
        loop {
            other = other.next_chase(&mut start, &mut step, Some(&mut span_start), Some(&mut last));
            if other.is_null() {
                break;
            }
            safety_net -= 1;
            if safety_net == 0 {
                return false;
            }
            if span_start.wind_sum() != SK_MIN_S32 {
                break;
            }
            let _ = other.mark_winding(span_start, winding);
        }
        if let Some(last_ptr) = last_ptr {
            *last_ptr = last;
        }
        success
    }

    pub fn mark_and_chase_winding_op(
        self,
        mut start: Span,
        end: Span,
        winding: i32,
        opp_winding: i32,
        last_ptr: Option<&mut Span>,
    ) -> bool {
        let mut span_start = start.starter(end);
        let mut step = start.step(end);
        let success = self.mark_winding_op(span_start, winding, opp_winding);
        let mut last = Span::NULL;
        let mut other = self;
        let mut safety_net = 100000;
        loop {
            other = other.next_chase(&mut start, &mut step, Some(&mut span_start), Some(&mut last));
            if other.is_null() {
                break;
            }
            safety_net -= 1;
            if safety_net == 0 {
                return false;
            }
            if span_start.wind_sum() != SK_MIN_S32 {
                if self.operand() == other.operand() {
                    if span_start.wind_sum() != winding || span_start.opp_sum() != opp_winding {
                        self.global_state().winding_failed = true;
                        return true; // ... but let it succeed anyway
                    }
                } else {
                    if span_start.wind_sum() != opp_winding {
                        return false;
                    }
                    if span_start.opp_sum() != winding {
                        return false;
                    }
                }
                break;
            }
            if self.operand() == other.operand() {
                let _ = other.mark_winding_op(span_start, winding, opp_winding);
            } else {
                let _ = other.mark_winding_op(span_start, opp_winding, winding);
            }
        }
        if let Some(last_ptr) = last_ptr {
            *last_ptr = last;
        }
        success
    }

    pub fn mark_angle(self, mut max_winding: i32, sum_winding: i32, angle: Angle, result: Option<&mut Span>) -> bool {
        if Segment::UseInnerWinding(max_winding, sum_winding) {
            max_winding = sum_winding;
        }
        self.mark_and_chase_winding(angle.start(), angle.end(), max_winding, result)
    }

    pub fn mark_angle_op(
        self,
        mut max_winding: i32,
        sum_winding: i32,
        mut opp_max_winding: i32,
        opp_sum_winding: i32,
        angle: Angle,
        result: Option<&mut Span>,
    ) -> bool {
        if Segment::UseInnerWinding(max_winding, sum_winding) {
            max_winding = sum_winding;
        }
        if opp_max_winding != opp_sum_winding && Segment::UseInnerWinding(opp_max_winding, opp_sum_winding) {
            opp_max_winding = opp_sum_winding;
        }
        // caller doesn't require that this marks anything
        self.mark_and_chase_winding_op(angle.start(), angle.end(), max_winding, opp_max_winding, result)
    }

    pub fn mark_done(self, span: Span) {
        if span.done() {
            return;
        }
        span.set_done(true);
        self.d().done_count += 1;
    }

    pub fn mark_winding(self, span: Span, winding: i32) -> bool {
        if span.done() {
            return false;
        }
        span.set_wind_sum(winding);
        true
    }

    pub fn mark_winding_op(self, span: Span, winding: i32, opp_winding: i32) -> bool {
        if span.done() {
            return false;
        }
        span.set_wind_sum(winding);
        span.set_opp_sum(opp_winding);
        true
    }

    /// `match`.
    pub fn match_(self, base: PtT, test_parent: Segment, test_t: f64, test_pt: Point) -> bool {
        if self == test_parent && precisely_equal(base.t(), test_t) {
            return true;
        }
        if !DPoint::approximately_equal_sk(test_pt, base.pt()) {
            return false;
        }
        self != test_parent || !self.pts_disjoint(base.t(), base.pt(), test_t, test_pt)
    }

    /// Please keep this in sync with debugMissingCoincidence(). Look for
    /// pairs of undetected coincident curves.
    pub fn missing_coincidence(self) -> bool {
        if self.done() {
            return false;
        }
        let mut prior = Span::NULL;
        let mut span_base = self.head();
        let mut result = false;
        let mut safety_net = 100000;
        loop {
            let mut pt_t = span_base.pt_t();
            let span_stop_pt_t = pt_t;
            loop {
                pt_t = pt_t.next();
                if pt_t == span_stop_pt_t {
                    break;
                }
                safety_net -= 1;
                if safety_net == 0 {
                    return false;
                }
                if pt_t.deleted() {
                    continue;
                }
                let opp = pt_t.span().segment();
                if opp.done() {
                    continue;
                }
                // when opp is encounted the 1st time, continue; on 2nd encounter, look for coincidence
                if !opp.visited() {
                    continue;
                }
                if span_base == self.head() {
                    continue;
                }
                if pt_t.segment() == self {
                    continue;
                }
                let span = span_base.up_castable();
                // FIXME?: this assumes that if the opposite segment is coincident then no more
                // coincidence needs to be detected. This may not be true.
                if span.is_some() && span.contains_coincidence_seg(opp) {
                    continue;
                }
                if span_base.contains_coin_end_seg(opp) {
                    continue;
                }
                let mut prior_pt_t = PtT::NULL;
                // find prior span containing opp segment
                let mut prior_opp = Segment::NULL;
                let mut prior_test = span_base.prev();
                while prior_opp.is_null() && prior_test.is_some() {
                    prior_pt_t = prior_test.pt_t();
                    let prior_stop_pt_t = prior_pt_t;
                    loop {
                        prior_pt_t = prior_pt_t.next();
                        if prior_pt_t == prior_stop_pt_t {
                            break;
                        }
                        if prior_pt_t.deleted() {
                            continue;
                        }
                        let segment = prior_pt_t.span().segment();
                        if segment == opp {
                            prior = prior_test;
                            prior_opp = opp;
                            break;
                        }
                    }
                    prior_test = prior_test.prev();
                }
                if prior_opp.is_null() {
                    continue;
                }
                if prior_pt_t == pt_t {
                    continue;
                }
                let mut opp_start = prior.pt_t();
                let mut opp_end = span_base.pt_t();
                let swapped = prior_pt_t.t() > pt_t.t();
                if swapped {
                    std::mem::swap(&mut prior_pt_t, &mut pt_t);
                    std::mem::swap(&mut opp_start, &mut opp_end);
                }
                let coincidences = unsafe { &mut *self.global_state().coincidence };
                let root_prior_pt_t = prior_pt_t.span().pt_t();
                let root_pt_t = pt_t.span().pt_t();
                let root_opp_start = opp_start.span().pt_t();
                let root_opp_end = opp_end.span().pt_t();
                if !coincidences.contains(root_prior_pt_t, root_pt_t, root_opp_start, root_opp_end)
                    && self.test_for_coincidence(root_prior_pt_t, root_pt_t, prior, span_base, opp)
                {
                    // mark coincidence
                    if !coincidences.extend(root_prior_pt_t, root_pt_t, root_opp_start, root_opp_end) {
                        coincidences.add(root_prior_pt_t, root_pt_t, root_opp_start, root_opp_end);
                    }
                    result = true;
                }
                // swapBack:
                if swapped {
                    std::mem::swap(&mut prior_pt_t, &mut pt_t);
                }
            }
            span_base = if span_base.is_final() { Span::NULL } else { span_base.up_cast().next() };
            if span_base.is_null() {
                break;
            }
        }
        Segment::ClearVisited(self.head());
        result
    }

    /// Please keep this in sync with DebugClearVisited().
    #[allow(non_snake_case)]
    pub fn ClearVisited(mut span: Span) {
        // reset visited flag back to false
        loop {
            let mut pt_t = span.pt_t();
            let stop_pt_t = pt_t;
            loop {
                pt_t = pt_t.next();
                if pt_t == stop_pt_t {
                    break;
                }
                let opp = pt_t.segment();
                opp.reset_visited();
            }
            if span.is_final() {
                break;
            }
            span = span.up_cast().next();
            if span.is_null() {
                break;
            }
        }
    }

    /// Please keep this in sync with debugMoveMultiples(). If a span has more
    /// than one intersection, merge the other segments' span as needed.
    // The innermost loop only ever runs once (every path leaves it), as in Skia.
    #[allow(clippy::never_loop)]
    pub fn move_multiples(self) -> bool {
        let mut test = self.head();
        loop {
            'check_next_span: {
                let add_count = test.span_adds_count();
                if add_count <= 1 {
                    break 'check_next_span;
                }
                let start_pt_t = test.pt_t();
                let mut test_pt_t = start_pt_t;
                let mut safety_hatch = 1000000;
                loop {
                    // iterate through all spans associated with start
                    safety_hatch -= 1;
                    if safety_hatch == 0 {
                        return false;
                    }
                    'next_pt_t: {
                        let opp_span = test_pt_t.span();
                        if opp_span.span_adds_count() == add_count {
                            break 'next_pt_t;
                        }
                        if opp_span.deleted() {
                            break 'next_pt_t;
                        }
                        let opp_segment = opp_span.segment();
                        if opp_segment == self {
                            break 'next_pt_t;
                        }
                        // find range of spans to consider merging
                        let mut opp_prev = opp_span;
                        let mut opp_first = opp_span;
                        loop {
                            opp_prev = opp_prev.prev();
                            if opp_prev.is_null() {
                                break;
                            }
                            if !roughly_equal(opp_prev.t(), opp_span.t()) {
                                break;
                            }
                            if opp_prev.span_adds_count() == add_count {
                                continue;
                            }
                            if opp_prev.deleted() {
                                continue;
                            }
                            opp_first = opp_prev;
                        }
                        let mut opp_next = opp_span;
                        let mut opp_last = opp_span;
                        loop {
                            opp_next = if opp_next.is_final() { Span::NULL } else { opp_next.up_cast().next() };
                            if opp_next.is_null() {
                                break;
                            }
                            if !roughly_equal(opp_next.t(), opp_span.t()) {
                                break;
                            }
                            if opp_next.span_adds_count() == add_count {
                                continue;
                            }
                            if opp_next.deleted() {
                                continue;
                            }
                            opp_last = opp_next;
                        }
                        if opp_first == opp_last {
                            break 'next_pt_t;
                        }
                        let mut opp_test = opp_first;
                        loop {
                            'try_next_span: {
                                if opp_test == opp_span {
                                    break 'try_next_span;
                                }
                                // check to see if the candidate meets specific criteria:
                                // it contains spans of segments in test's loop but not including 'this'
                                let opp_start_pt_t = opp_test.pt_t();
                                let mut opp_pt_t = opp_start_pt_t;
                                loop {
                                    opp_pt_t = opp_pt_t.next();
                                    if opp_pt_t == opp_start_pt_t {
                                        break;
                                    }
                                    let opp_pt_t_segment = opp_pt_t.segment();
                                    if opp_pt_t_segment == self {
                                        break 'try_next_span;
                                    }
                                    let mut match_pt_t = start_pt_t;
                                    let mut found = false;
                                    loop {
                                        if match_pt_t.segment() == opp_pt_t_segment {
                                            found = true;
                                            break;
                                        }
                                        match_pt_t = match_pt_t.next();
                                        if match_pt_t == start_pt_t {
                                            break;
                                        }
                                    }
                                    if !found {
                                        break 'try_next_span;
                                    }
                                    // foundMatch: merge oppTest and oppSpan
                                    opp_test.merge_matches(opp_span);
                                    opp_test.add_opp(opp_span);
                                    break 'check_next_span;
                                }
                            }
                            if !(opp_test != opp_last) {
                                break;
                            }
                            opp_test = opp_test.up_cast().next();
                            if opp_test.is_null() {
                                break;
                            }
                        }
                    }
                    test_pt_t = test_pt_t.next();
                    if test_pt_t == start_pt_t {
                        break;
                    }
                }
            }
            // checkNextSpan:
            test = if test.is_final() { Span::NULL } else { test.up_cast().next() };
            if test.is_null() {
                break;
            }
        }
        true
    }

    /// Adjacent spans may have points close by.
    pub fn spans_nearby(self, ref_span: Span, check_span: Span, found: &mut bool) -> bool {
        let ref_head = ref_span.pt_t();
        let check_head = check_span.pt_t();
        // if the first pt pair from adjacent spans are far apart, assume that all are far enough apart
        if !DPoint::way_roughly_equal(ref_head.pt(), check_head.pt()) {
            *found = false;
            return true;
        }
        // check only unique points
        let mut dist_sq_best = f32::MAX;
        let mut ref_best = PtT::NULL;
        let mut check_best = PtT::NULL;
        let mut ref_ = ref_head;
        'done_checking_distance: loop {
            'next_ref: {
                if ref_.deleted() {
                    break 'next_ref;
                }
                while ref_.pt_already_seen(ref_head) {
                    ref_ = ref_.next();
                    if ref_ == ref_head {
                        break 'done_checking_distance;
                    }
                }
                let mut check = check_head;
                let ref_seg = ref_.segment();
                let mut escape_hatch = 100000; // defend against infinite loops
                loop {
                    'next_check: {
                        if check.deleted() {
                            break 'next_check;
                        }
                        while check.pt_already_seen(check_head) {
                            check = check.next();
                            if check == check_head {
                                break 'next_ref;
                            }
                        }
                        let dist_sq = distance_to_sqd(ref_.pt(), check.pt());
                        if dist_sq_best > dist_sq
                            && (ref_seg != check.segment() || !ref_seg.pts_disjoint(ref_.t(), ref_.pt(), check.t(), check.pt()))
                        {
                            dist_sq_best = dist_sq;
                            ref_best = ref_;
                            check_best = check;
                        }
                        escape_hatch -= 1;
                        if escape_hatch <= 0 {
                            return false;
                        }
                    }
                    check = check.next();
                    if check == check_head {
                        break;
                    }
                }
            }
            ref_ = ref_.next();
            if ref_ == ref_head {
                break;
            }
        }
        // doneCheckingDistance:
        *found = check_best.is_some()
            && ref_best.segment().match_(ref_best, check_best.segment(), check_best.t(), check_best.pt());
        true
    }

    /// Please keep this function in sync with debugMoveNearby(). Move nearby
    /// t values and pts so they all hang off the same span.
    pub fn move_nearby(self) -> bool {
        // release undeleted spans pointing to this seg that are linked to the primary span
        let mut span_base = self.head();
        let mut escape_hatch = 9999; // the largest count for a regular test is 50; for a fuzzer, 500
        loop {
            let mut pt_t = span_base.pt_t();
            let head_pt_t = pt_t;
            loop {
                pt_t = pt_t.next();
                if pt_t == head_pt_t {
                    break;
                }
                escape_hatch -= 1;
                if escape_hatch == 0 {
                    return false;
                }
                let test = pt_t.span();
                if pt_t.segment() == self && !pt_t.deleted() && test != span_base && test.pt_t() == pt_t {
                    if test.is_final() {
                        if span_base == self.head() {
                            self.clear_all();
                            return true;
                        }
                        span_base.up_cast().release(pt_t);
                    } else if test.prev().is_some() {
                        test.up_cast().release(head_pt_t);
                    }
                    break;
                }
            }
            span_base = span_base.up_cast().next();
            if span_base.is_final() {
                break;
            }
        }
        // This loop looks for adjacent spans which are near by
        span_base = self.head();
        loop {
            // iterate through all spans associated with start
            let test = span_base.up_cast().next();
            let mut found = false;
            if !self.spans_nearby(span_base, test, &mut found) {
                return false;
            }
            if found {
                if test.is_final() {
                    if span_base.prev().is_some() {
                        test.merge(span_base.up_cast());
                    } else {
                        self.clear_all();
                        return true;
                    }
                } else {
                    span_base.merge(test.up_cast());
                }
            }
            span_base = test;
            if span_base.is_final() {
                break;
            }
        }
        true
    }

    #[inline]
    pub fn next(self) -> Segment {
        self.d().next
    }

    pub fn next_chase(self, start_ptr: &mut Span, step_ptr: &mut i32, min_ptr: Option<&mut Span>, last: Option<&mut Span>) -> Segment {
        let orig_start = *start_ptr;
        let step = *step_ptr;
        let mut end_span = if step > 0 { orig_start.up_cast().next() } else { orig_start.prev() };
        let angle = if step > 0 { end_span.from_angle() } else { end_span.up_cast().to_angle() };
        let found_span;
        let other_end;
        let other;
        if angle.is_null() {
            if end_span.t() != 0.0 && end_span.t() != 1.0 {
                return Segment::NULL;
            }
            let other_pt_t = end_span.pt_t().next();
            other = other_pt_t.segment();
            found_span = other_pt_t.span();
            other_end = if step > 0 {
                if found_span.up_castable().is_some() { found_span.up_cast().next() } else { Span::NULL }
            } else {
                found_span.prev()
            };
        } else {
            let loop_count = angle.loop_count();
            if loop_count > 2 {
                if let Some(last) = last {
                    *last = end_span;
                }
                return Segment::NULL;
            }
            let next = angle.next();
            if next.is_null() {
                return Segment::NULL;
            }
            other = next.segment();
            end_span = next.start();
            found_span = end_span;
            other_end = next.end();
        }
        if other_end.is_null() {
            return Segment::NULL;
        }
        let found_step = found_span.step(other_end);
        if *step_ptr != found_step {
            if let Some(last) = last {
                *last = end_span;
            }
            return Segment::NULL;
        }
        let orig_min = if step < 0 { orig_start.prev() } else { orig_start.up_cast() };
        let found_min = found_span.starter(other_end);
        if found_min.wind_value() != orig_min.wind_value() || found_min.opp_value() != orig_min.opp_value() {
            if let Some(last) = last {
                *last = end_span;
            }
            return Segment::NULL;
        }
        *start_ptr = found_span;
        *step_ptr = found_step;
        if let Some(min_ptr) = min_ptr {
            *min_ptr = found_min;
        }
        other
    }

    pub fn operand(self) -> bool {
        self.contour().d().operand
    }

    #[allow(non_snake_case)]
    pub fn OppSign(start: Span, end: Span) -> i32 {
        if start.t() < end.t() { -start.up_cast().opp_value() } else { end.up_cast().opp_value() }
    }

    pub fn opp_xor(self) -> bool {
        self.contour().d().opp_xor
    }

    #[inline]
    pub fn prev(self) -> Segment {
        self.d().prev
    }

    pub fn pt_at_t(self, mid: f64) -> Point {
        let d = self.d();
        curve_point_at_t(d.verb, &d.pts, d.weight, mid)
    }

    #[inline]
    pub fn pts(self) -> &'static [Point; 4] {
        &self.d().pts
    }

    pub fn pts_disjoint(self, t1: f64, pt1: Point, t2: f64, pt2: Point) -> bool {
        if self.d().verb == Verb::Line {
            return false;
        }
        // quads (and cubics) can loop back to nearly a line so that an opposite curve
        // hits in two places with very different t values.
        let mid_t = (t1 + t2) / 2.0;
        let mid_pt = self.pt_at_t(mid_t);
        let se_dist_sq = sminf_max(distance_to_sqd(pt1, pt2) * 2.0, f32::EPSILON * 2.0);
        distance_to_sqd(mid_pt, pt1) > se_dist_sq || distance_to_sqd(mid_pt, pt2) > se_dist_sq
    }

    pub fn release(self, span: Span) {
        if span.done() {
            self.d().done_count -= 1;
        }
        self.d().count -= 1;
    }

    pub fn reset_visited(self) {
        self.d().visited = false;
    }

    pub fn set_contour(self, contour: Contour) {
        self.d().contour = contour;
    }

    pub fn set_next(self, next: Segment) {
        self.d().next = next;
    }

    pub fn set_prev(self, prev: Segment) {
        self.d().prev = prev;
    }

    pub fn set_up_winding(self, start: Span, end: Span, max_winding: &mut i32, sum_winding: &mut i32) {
        let delta_sum = Segment::SpanSign(start, end);
        *max_winding = *sum_winding;
        if *sum_winding == SK_MIN_S32 {
            return;
        }
        *sum_winding -= delta_sum;
    }

    pub fn set_up_windings(
        self,
        start: Span,
        end: Span,
        sum_mi_winding: &mut i32,
        max_winding: &mut i32,
        sum_winding: &mut i32,
    ) {
        let delta_sum = Segment::SpanSign(start, end);
        *max_winding = *sum_mi_winding;
        *sum_mi_winding = sum_mi_winding.wrapping_sub(delta_sum);
        *sum_winding = *sum_mi_winding;
    }

    #[allow(clippy::too_many_arguments)]
    pub fn set_up_windings_op(
        self,
        start: Span,
        end: Span,
        sum_mi_winding: &mut i32,
        sum_su_winding: &mut i32,
        max_winding: &mut i32,
        sum_winding: &mut i32,
        opp_max_winding: &mut i32,
        opp_sum_winding: &mut i32,
    ) {
        let delta_sum = Segment::SpanSign(start, end);
        let opp_delta_sum = Segment::OppSign(start, end);
        if self.operand() {
            *max_winding = *sum_su_winding;
            *sum_su_winding = sum_su_winding.wrapping_sub(delta_sum);
            *sum_winding = *sum_su_winding;
            *opp_max_winding = *sum_mi_winding;
            *sum_mi_winding = sum_mi_winding.wrapping_sub(opp_delta_sum);
            *opp_sum_winding = *sum_mi_winding;
        } else {
            *max_winding = *sum_mi_winding;
            *sum_mi_winding = sum_mi_winding.wrapping_sub(delta_sum);
            *sum_winding = *sum_mi_winding;
            *opp_max_winding = *sum_su_winding;
            *sum_su_winding = sum_su_winding.wrapping_sub(opp_delta_sum);
            *opp_sum_winding = *sum_su_winding;
        }
    }

    pub fn sort_angles(self) -> bool {
        let mut span = self.head();
        loop {
            'body: {
                let from_angle = span.from_angle();
                let to_angle = if span.is_final() { Angle::NULL } else { span.up_cast().to_angle() };
                if from_angle.is_null() && to_angle.is_null() {
                    break 'body;
                }
                let mut base_angle = from_angle;
                if from_angle.is_some() && to_angle.is_some() {
                    if !from_angle.insert(to_angle) {
                        return false;
                    }
                } else if from_angle.is_null() {
                    base_angle = to_angle;
                }
                let mut pt_t = span.pt_t();
                let stop_pt_t = pt_t;
                let mut safety_net = 1000000;
                loop {
                    safety_net -= 1;
                    if safety_net == 0 {
                        return false;
                    }
                    let o_span = pt_t.span();
                    if o_span != span {
                        let mut o_angle = o_span.from_angle();
                        if o_angle.is_some() && !o_angle.loop_contains(base_angle) {
                            base_angle.insert(o_angle);
                        }
                        if !o_span.is_final() {
                            o_angle = o_span.up_cast().to_angle();
                            if o_angle.is_some() && !o_angle.loop_contains(base_angle) {
                                base_angle.insert(o_angle);
                            }
                        }
                    }
                    pt_t = pt_t.next();
                    if pt_t == stop_pt_t {
                        break;
                    }
                }
                if base_angle.loop_count() == 1 {
                    span.set_from_angle(Angle::NULL);
                    if to_angle.is_some() {
                        span.up_cast().set_to_angle(Angle::NULL);
                    }
                }
            }
            if span.is_final() {
                break;
            }
            span = span.up_cast().next();
            if span.is_null() {
                break;
            }
        }
        true
    }

    #[allow(non_snake_case)]
    pub fn SpanSign(start: Span, end: Span) -> i32 {
        if start.t() < end.t() { -start.up_cast().wind_value() } else { end.up_cast().wind_value() }
    }

    pub fn span_to_angle(self, start: Span, end: Span) -> Angle {
        if start.t() < end.t() { start.up_cast().to_angle() } else { start.from_angle() }
    }

    pub fn sub_divide(self, start: Span, end: Span, edge: &mut DCurve) -> bool {
        let start_pt_t = start.pt_t();
        let end_pt_t = end.pt_t();
        let d = self.d();
        edge[0].set(start_pt_t.pt());
        let points = verb_to_points(d.verb);
        edge[points].set(end_pt_t.pt());
        if d.verb == Verb::Line {
            return false;
        }
        let start_t = start_pt_t.t();
        let end_t = end_pt_t.t();
        if (start_t == 0.0 || end_t == 0.0) && (start_t == 1.0 || end_t == 1.0) {
            // don't compute midpoints if we already have them
            if d.verb == Verb::Quad {
                edge[1].set(d.pts[1]);
                return false;
            }
            if d.verb == Verb::Conic {
                edge[1].set(d.pts[1]);
                edge.weight = d.weight;
                return false;
            }
            if start_t == 0.0 {
                edge[1].set(d.pts[1]);
                edge[2].set(d.pts[2]);
                return false;
            }
            edge[1].set(d.pts[2]);
            edge[2].set(d.pts[1]);
            return false;
        }
        if d.verb == Verb::Quad {
            edge[1] = DQuad::SubDividePt(&d.pts, &edge[0], &edge[2], start_t, end_t);
        } else if d.verb == Verb::Conic {
            let mut w = edge.weight;
            edge[1] = DConic::SubDividePt(&d.pts, d.weight, &edge[0], &edge[2], start_t, end_t, &mut w);
            edge.weight = w;
        } else {
            let p = DCubic::SubDividePts(&d.pts, &edge[0], &edge[3], start_t, end_t);
            edge[1] = p[0];
            edge[2] = p[1];
        }
        true
    }

    pub fn test_for_coincidence(self, prior_pt_t: PtT, pt_t: PtT, prior: Span, span_base: Span, opp: Segment) -> bool {
        // average t, find mid pt
        let mid_t = (prior.t() + span_base.t()) / 2.0;
        let mid_pt = self.pt_at_t(mid_t);
        let mut coincident = true;
        // if the mid pt is not near either end pt, project perpendicular through opp seg
        if !DPoint::approximately_equal_sk(prior_pt_t.pt(), mid_pt) && !DPoint::approximately_equal_sk(pt_t.pt(), mid_pt) {
            if prior_pt_t.span() == pt_t.span() {
                return false;
            }
            coincident = false;
            let mut i = Intersections::default();
            let mut curve_part = DCurve::default();
            self.sub_divide(prior, span_base, &mut curve_part);
            let verb = self.verb();
            let dxdy = curve_dd_slope_at_t(verb, &curve_part, 0.5f32 as f64);
            let part_mid_pt = curve_dd_point_at_t(verb, &curve_part, 0.5f32 as f64);
            let ray = DLine::new(
                DPoint::new(mid_pt.x as f64, mid_pt.y as f64),
                DPoint::new(part_mid_pt.x + dxdy.y, part_mid_pt.y - dxdy.x),
            );
            let mut opp_part = DCurve::default();
            opp.sub_divide(prior_pt_t.span(), pt_t.span(), &mut opp_part);
            curve_d_intersect_ray(opp.verb(), &opp_part, &ray, &mut i);
            // measure distance and see if it's small enough to denote coincidence
            for index in 0..i.used() as usize {
                if !between(0.0, i.t(0, index), 1.0) {
                    continue;
                }
                let opp_pt = i.pt(index);
                if opp_pt.approximately_d_equal_pt(mid_pt) {
                    // the coincidence can occur at almost any angle
                    coincident = true;
                }
            }
        }
        coincident
    }

    pub fn undone_span(self) -> Span {
        let mut span = self.head();
        loop {
            let next = span.next();
            if !span.done() {
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

    pub fn update_opp_winding(self, start: Span, end: Span) -> i32 {
        let lesser = start.starter(end);
        let mut opp_winding = lesser.opp_sum();
        let opp_span_winding = Segment::OppSign(start, end);
        if opp_span_winding != 0
            && Segment::UseInnerWinding(opp_winding.wrapping_sub(opp_span_winding), opp_winding)
            && opp_winding != SK_MAX_S32
        {
            opp_winding = opp_winding.wrapping_sub(opp_span_winding);
        }
        opp_winding
    }

    pub fn update_opp_winding_angle(self, angle: Angle) -> i32 {
        let start_span = angle.start();
        let end_span = angle.end();
        self.update_opp_winding(end_span, start_span)
    }

    pub fn update_opp_winding_reverse(self, angle: Angle) -> i32 {
        let start_span = angle.start();
        let end_span = angle.end();
        self.update_opp_winding(start_span, end_span)
    }

    pub fn update_winding(self, start: Span, end: Span) -> i32 {
        let lesser = start.starter(end);
        let mut winding = lesser.wind_sum();
        if winding == SK_MIN_S32 {
            winding = lesser.compute_wind_sum();
        }
        if winding == SK_MIN_S32 {
            return winding;
        }
        let span_winding = Segment::SpanSign(start, end);
        if winding != 0
            && Segment::UseInnerWinding(winding.wrapping_sub(span_winding), winding)
            && winding != SK_MAX_S32
        {
            winding = winding.wrapping_sub(span_winding);
        }
        winding
    }

    pub fn update_winding_angle(self, angle: Angle) -> i32 {
        let start_span = angle.start();
        let end_span = angle.end();
        self.update_winding(end_span, start_span)
    }

    pub fn update_winding_reverse(self, angle: Angle) -> i32 {
        let start_span = angle.start();
        let end_span = angle.end();
        self.update_winding(start_span, end_span)
    }

    #[allow(non_snake_case)]
    pub fn UseInnerWinding(outer_winding: i32, inner_winding: i32) -> bool {
        let abs_out = outer_winding.wrapping_abs();
        let abs_in = inner_winding.wrapping_abs();
        if abs_out == abs_in { outer_winding < 0 } else { abs_out < abs_in }
    }

    #[inline]
    pub fn verb(self) -> Verb {
        self.d().verb
    }

    /// Look for two different spans that point to the same opposite segment.
    pub fn visited(self) -> bool {
        if !self.d().visited {
            self.d().visited = true;
            return false;
        }
        true
    }

    #[inline]
    pub fn weight(self) -> f32 {
        self.d().weight
    }

    pub fn wind_sum_angle(self, angle: Angle) -> i32 {
        let min_span = angle.start().starter(angle.end());
        min_span.wind_sum()
    }
}

/// `std::max(float, float)`.
fn sminf_max(a: f32, b: f32) -> f32 {
    if a < b { b } else { a }
}
