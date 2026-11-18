//! `SkOpContour.h`, `SkOpContour.cpp`: a closed run of segments, the
//! linked list of contours (`SkOpContourHead`) and `SkOpContourBuilder`.

use super::graph::*;
use super::rect::PathOpsBounds;
use super::types::GlobalState;
use super::writer::PathWriter;
use crate::geom::Point;
use crate::path::Verb;

impl Contour {
    pub fn add_conic(self, pts: &[Point], weight: f32) {
        self.append_segment().add_conic(pts, weight, self);
    }

    pub fn add_cubic(self, pts: &[Point]) {
        self.append_segment().add_cubic(pts, self);
    }

    pub fn add_line(self, pts: &[Point]) -> Segment {
        self.append_segment().add_line(pts, self)
    }

    pub fn add_quad(self, pts: &[Point]) {
        self.append_segment().add_quad(pts, self);
    }

    pub fn append_segment(self) -> Segment {
        let count = self.d().count;
        self.d().count += 1;
        let result = if count != 0 {
            Segment(self.global_state().allocator.make(SegmentData::default()))
        } else {
            self.first()
        };
        result.set_prev(self.d().tail);
        if self.d().tail.is_some() {
            self.d().tail.set_next(result);
        }
        self.d().tail = result;
        result
    }

    #[inline]
    pub fn bounds(self) -> PathOpsBounds {
        self.d().bounds
    }

    pub fn calc_angles(self) {
        let mut segment = self.first();
        loop {
            segment.calc_angles();
            segment = segment.next();
            if segment.is_null() {
                break;
            }
        }
    }

    pub fn complete(self) {
        self.set_bounds();
    }

    #[inline]
    pub fn count(self) -> i32 {
        self.d().count
    }

    #[inline]
    pub fn done(self) -> bool {
        self.d().done
    }

    pub fn end(self) -> Point {
        self.d().tail.last_pt()
    }

    pub fn init(self, global_state: *mut GlobalState, operand: bool, is_xor: bool) {
        let d = self.d();
        d.state = global_state;
        d.operand = operand;
        d.xor = is_xor;
    }

    pub fn is_ccw(self) -> i32 {
        self.d().ccw
    }

    pub fn is_xor(self) -> bool {
        self.d().xor
    }

    pub fn join_segments(self) {
        let mut segment = self.first();
        loop {
            let next = segment.next();
            segment.join_ends(if next.is_some() { next } else { self.first() });
            segment = next;
            if segment.is_null() {
                break;
            }
        }
    }

    pub fn mark_all_done(self) {
        let mut segment = self.first();
        loop {
            segment.mark_all_done();
            segment = segment.next();
            if segment.is_null() {
                break;
            }
        }
    }

    /// Please keep this aligned with debugMissingCoincidence().
    pub fn missing_coincidence(self) -> bool {
        let mut segment = self.first();
        let mut result = false;
        loop {
            if segment.missing_coincidence() {
                result = true;
            }
            segment = segment.next();
            if segment.is_null() {
                break;
            }
        }
        result
    }

    pub fn move_multiples(self) -> bool {
        let mut segment = self.first();
        loop {
            if !segment.move_multiples() {
                return false;
            }
            segment = segment.next();
            if segment.is_null() {
                return true;
            }
        }
    }

    pub fn move_nearby(self) -> bool {
        let mut segment = self.first();
        loop {
            if !segment.move_nearby() {
                return false;
            }
            segment = segment.next();
            if segment.is_null() {
                return true;
            }
        }
    }

    #[inline]
    pub fn next(self) -> Contour {
        self.d().next
    }

    pub fn operand(self) -> bool {
        self.d().operand
    }

    pub fn opp_xor(self) -> bool {
        self.d().opp_xor
    }

    pub fn reset(self) {
        let d = self.d();
        d.tail = Segment::NULL;
        d.next = Contour::NULL;
        d.count = 0;
        d.done = false;
    }

    pub fn reset_reverse(self) {
        let mut next = self;
        loop {
            if next.count() != 0 {
                next.d().ccw = -1;
                next.d().reverse = false;
            }
            next = next.next();
            if next.is_null() {
                break;
            }
        }
    }

    pub fn reversed(self) -> bool {
        self.d().reverse
    }

    pub fn set_bounds(self) {
        let mut segment = self.first();
        self.d().bounds = segment.bounds();
        loop {
            segment = segment.next();
            if segment.is_null() {
                break;
            }
            let b = segment.bounds();
            self.d().bounds.add(&b);
        }
    }

    pub fn set_ccw(self, ccw: i32) {
        self.d().ccw = ccw;
    }

    pub fn set_next(self, contour: Contour) {
        self.d().next = contour;
    }

    pub fn set_opp_xor(self, is_opp_xor: bool) {
        self.d().opp_xor = is_opp_xor;
    }

    pub fn sort_angles(self) -> bool {
        let mut segment = self.first();
        loop {
            if !segment.sort_angles() {
                return false;
            }
            segment = segment.next();
            if segment.is_null() {
                return true;
            }
        }
    }

    pub fn start(self) -> Point {
        self.first().pts()[0]
    }

    pub fn to_path(self, path: &mut PathWriter) {
        if self.count() == 0 {
            return;
        }
        let mut segment = self.first();
        loop {
            segment.add_curve_to(segment.head(), segment.tail(), path);
            segment = segment.next();
            if segment.is_null() {
                break;
            }
        }
        path.finish_contour();
        path.assemble();
    }

    pub fn to_reverse_path(self, path: &mut PathWriter) {
        let mut segment = self.d().tail;
        loop {
            segment.add_curve_to(segment.tail(), segment.head(), path);
            segment = segment.prev();
            if segment.is_null() {
                break;
            }
        }
        path.finish_contour();
        path.assemble();
    }

    pub fn undone_span(self) -> Span {
        let mut test_segment = self.first();
        loop {
            if !test_segment.done() {
                return test_segment.undone_span();
            }
            test_segment = test_segment.next();
            if test_segment.is_null() {
                break;
            }
        }
        self.d().done = true;
        Span::NULL
    }

    /// `operator<`: by top, then left.
    pub fn less_than(self, rh: Contour) -> bool {
        let (a, b) = (self.d().bounds, rh.d().bounds);
        if a.top == b.top { a.left < b.left } else { a.top < b.top }
    }

    // SkOpContourHead

    pub fn append_contour(self) -> Contour {
        let contour = Contour(self.global_state().allocator.make(ContourData::default()));
        contour.set_next(Contour::NULL);
        let mut prev = self;
        loop {
            let next = prev.next();
            if next.is_null() {
                break;
            }
            prev = next;
        }
        prev.set_next(contour);
        contour
    }

    pub fn join_all_segments(self) {
        let mut next = self;
        loop {
            if next.count() != 0 {
                next.join_segments();
            }
            next = next.next();
            if next.is_null() {
                break;
            }
        }
    }

    pub fn remove(self, contour: Contour) {
        if contour == self {
            return;
        }
        let mut prev = self;
        loop {
            let next = prev.next();
            if next == contour {
                break;
            }
            prev = next;
        }
        prev.set_next(Contour::NULL);
    }
}

/// `SkOpContourBuilder`: collapses a line followed by its exact reverse.
pub struct ContourBuilder {
    contour: Contour,
    last_line: [Point; 2],
    last_is_line: bool,
}

impl ContourBuilder {
    pub fn new(contour: Contour) -> Self {
        ContourBuilder { contour, last_line: [Point::default(); 2], last_is_line: false }
    }

    pub fn add_conic(&mut self, pts: &[Point], weight: f32) {
        self.flush();
        self.contour.add_conic(pts, weight);
    }

    pub fn add_cubic(&mut self, pts: &[Point]) {
        self.flush();
        self.contour.add_cubic(pts);
    }

    pub fn add_curve(&mut self, verb: Verb, pts: &[Point], weight: f32) {
        match verb {
            Verb::Line => self.add_line(pts),
            Verb::Quad => self.add_quad(pts),
            Verb::Conic => self.add_conic(pts, weight),
            Verb::Cubic => self.add_cubic(pts),
            _ => {}
        }
    }

    pub fn add_line(&mut self, pts: &[Point]) {
        // if the previous line added is the exact opposite, eliminate both
        if self.last_is_line {
            if self.last_line[0] == pts[1] && self.last_line[1] == pts[0] {
                self.last_is_line = false;
                return;
            } else {
                self.flush();
            }
        }
        self.last_line = [pts[0], pts[1]];
        self.last_is_line = true;
    }

    pub fn add_quad(&mut self, pts: &[Point]) {
        self.flush();
        self.contour.add_quad(pts);
    }

    pub fn flush(&mut self) {
        if !self.last_is_line {
            return;
        }
        let pts = self.last_line;
        let _ = self.contour.add_line(&pts);
        self.last_is_line = false;
    }

    pub fn contour(&self) -> Contour {
        self.contour
    }

    pub fn set_contour(&mut self, contour: Contour) {
        self.flush();
        self.contour = contour;
    }
}
