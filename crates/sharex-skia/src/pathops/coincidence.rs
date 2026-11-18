//! `SkOpCoincidence.h`, `SkOpCoincidence.cpp`: runs of two segments that
//! lie on each other (`SkCoincidentSpans`) and their resolution.

use super::curve::*;
use super::graph::*;
use super::intersections::Intersections;
use super::line::DLine;
use super::point::DPoint;
use super::span::Collapsed;
use super::types::*;
use crate::path::Verb;

/// `SkPath::Verb` as Skia numbers it, for `Ordered`.
fn verb_int(v: Verb) -> i32 {
    match v {
        Verb::Move => 0,
        Verb::Line => 1,
        Verb::Quad => 2,
        Verb::Conic => 3,
        Verb::Cubic => 4,
        Verb::Close => 5,
    }
}

impl CoinSpans {
    #[inline]
    pub fn coin_pt_t_start(self) -> PtT {
        self.d().coin_pt_t_start
    }
    #[inline]
    pub fn coin_pt_t_end(self) -> PtT {
        self.d().coin_pt_t_end
    }
    #[inline]
    pub fn opp_pt_t_start(self) -> PtT {
        self.d().opp_pt_t_start
    }
    #[inline]
    pub fn opp_pt_t_end(self) -> PtT {
        self.d().opp_pt_t_end
    }
    #[inline]
    pub fn next(self) -> CoinSpans {
        self.d().next
    }

    pub fn set_next(self, next: CoinSpans) {
        self.d().next = next;
    }

    pub fn flipped(self) -> bool {
        self.opp_pt_t_start().t() > self.opp_pt_t_end().t()
    }

    pub fn set_coin_pt_t_end(self, pt_t: PtT) {
        self.d().coin_pt_t_end = pt_t;
        pt_t.set_coincident();
    }

    pub fn set_coin_pt_t_start(self, pt_t: PtT) {
        self.d().coin_pt_t_start = pt_t;
        pt_t.set_coincident();
    }

    pub fn set_opp_pt_t_end(self, pt_t: PtT) {
        self.d().opp_pt_t_end = pt_t;
        pt_t.set_coincident();
    }

    pub fn set_opp_pt_t_start(self, pt_t: PtT) {
        self.d().opp_pt_t_start = pt_t;
        pt_t.set_coincident();
    }

    pub fn set_ends(self, coin_pt_t_end: PtT, opp_pt_t_end: PtT) {
        self.set_coin_pt_t_end(coin_pt_t_end);
        self.set_opp_pt_t_end(opp_pt_t_end);
    }

    pub fn set_starts(self, coin_pt_t_start: PtT, opp_pt_t_start: PtT) {
        self.set_coin_pt_t_start(coin_pt_t_start);
        self.set_opp_pt_t_start(opp_pt_t_start);
    }

    /// Returns true if the coincident span's start and end are the same.
    pub fn collapsed(self, test: PtT) -> bool {
        (self.coin_pt_t_start() == test && self.coin_pt_t_end().contains_pt_t(test))
            || (self.coin_pt_t_end() == test && self.coin_pt_t_start().contains_pt_t(test))
            || (self.opp_pt_t_start() == test && self.opp_pt_t_end().contains_pt_t(test))
            || (self.opp_pt_t_end() == test && self.opp_pt_t_start().contains_pt_t(test))
    }

    /// Sets the span's end to the ptT referenced by the previous-next.
    fn correct_one_end(self, get_end: fn(CoinSpans) -> PtT, set_end: fn(CoinSpans, PtT)) {
        let orig_pt_t = get_end(self);
        let orig_span = orig_pt_t.span();
        let prev = orig_span.prev();
        let test_pt_t = if prev.is_some() {
            prev.next().pt_t()
        } else {
            orig_span.up_cast().next().prev().pt_t()
        };
        if orig_pt_t != test_pt_t {
            set_end(self, test_pt_t);
        }
    }

    /// Makes all span ends agree with the segment's spans that define them.
    pub fn correct_ends(self) {
        self.correct_one_end(CoinSpans::coin_pt_t_start, CoinSpans::set_coin_pt_t_start);
        self.correct_one_end(CoinSpans::coin_pt_t_end, CoinSpans::set_coin_pt_t_end);
        self.correct_one_end(CoinSpans::opp_pt_t_start, CoinSpans::set_opp_pt_t_start);
        self.correct_one_end(CoinSpans::opp_pt_t_end, CoinSpans::set_opp_pt_t_end);
    }

    /// Expand the range by checking adjacent spans for coincidence.
    pub fn expand(self) -> bool {
        let mut expanded = false;
        let segment = self.coin_pt_t_start().segment();
        let opp_segment = self.opp_pt_t_start().segment();
        loop {
            let start = self.coin_pt_t_start().span().up_cast();
            let prev = start.prev();
            if prev.is_null() {
                break;
            }
            let opp_pt_t = prev.contains_seg(opp_segment);
            if opp_pt_t.is_null() {
                break;
            }
            let mid_t = (prev.t() + start.t()) / 2.0;
            if !segment.is_close(mid_t, opp_segment) {
                break;
            }
            self.set_starts(prev.pt_t(), opp_pt_t);
            expanded = true;
        }
        loop {
            let end = self.coin_pt_t_end().span();
            let next = if end.is_final() { Span::NULL } else { end.up_cast().next() };
            if next.is_some() && next.deleted() {
                break;
            }
            if next.is_null() {
                break;
            }
            let opp_pt_t = next.contains_seg(opp_segment);
            if opp_pt_t.is_null() {
                break;
            }
            let mid_t = (end.t() + next.t()) / 2.0;
            if !segment.is_close(mid_t, opp_segment) {
                break;
            }
            self.set_ends(next.pt_t(), opp_pt_t);
            expanded = true;
        }
        expanded
    }

    /// Increase the range of this span.
    pub fn extend(self, coin_pt_t_start: PtT, coin_pt_t_end: PtT, opp_pt_t_start: PtT, opp_pt_t_end: PtT) -> bool {
        let mut result = false;
        if self.coin_pt_t_start().t() > coin_pt_t_start.t()
            || (if self.flipped() {
                self.opp_pt_t_start().t() < opp_pt_t_start.t()
            } else {
                self.opp_pt_t_start().t() > opp_pt_t_start.t()
            })
        {
            self.set_starts(coin_pt_t_start, opp_pt_t_start);
            result = true;
        }
        if self.coin_pt_t_end().t() < coin_pt_t_end.t()
            || (if self.flipped() {
                self.opp_pt_t_end().t() > opp_pt_t_end.t()
            } else {
                self.opp_pt_t_end().t() < opp_pt_t_end.t()
            })
        {
            self.set_ends(coin_pt_t_end, opp_pt_t_end);
            result = true;
        }
        result
    }

    /// Set the range of this span.
    pub fn set(self, next: CoinSpans, coin_pt_t_start: PtT, coin_pt_t_end: PtT, opp_pt_t_start: PtT, opp_pt_t_end: PtT) {
        self.d().next = next;
        self.set_starts(coin_pt_t_start, opp_pt_t_start);
        self.set_ends(coin_pt_t_end, opp_pt_t_end);
    }

    /// Returns true if both points are inside this.
    pub fn contains(self, mut s: PtT, mut e: PtT) -> bool {
        if s.t() > e.t() {
            std::mem::swap(&mut s, &mut e);
        }
        if s.segment() == self.coin_pt_t_start().segment() {
            self.coin_pt_t_start().t() <= s.t() && e.t() <= self.coin_pt_t_end().t()
        } else {
            let mut opp_ts = self.opp_pt_t_start().t();
            let mut opp_te = self.opp_pt_t_end().t();
            if opp_ts > opp_te {
                std::mem::swap(&mut opp_ts, &mut opp_te);
            }
            opp_ts <= s.t() && e.t() <= opp_te
        }
    }

    /// A coincident span is unordered if the pairs of points in the main
    /// and opposite curves' t values do not ascend or descend.
    pub fn ordered(self, result: &mut bool) -> bool {
        let start = self.coin_pt_t_start().span();
        let end = self.coin_pt_t_end().span();
        let mut next = start.up_cast().next();
        if next == end {
            *result = true;
            return true;
        }
        let flipped = self.flipped();
        let opp_seg = self.opp_pt_t_start().segment();
        let mut opp_last_t = self.opp_pt_t_start().t();
        loop {
            let opp = next.contains_seg(opp_seg);
            if opp.is_null() {
                return false;
            }
            if (opp_last_t > opp.t()) != flipped {
                *result = false;
                return true;
            }
            opp_last_t = opp.t();
            if next == end {
                break;
            }
            if next.up_castable().is_null() {
                *result = false;
                return true;
            }
            next = next.up_cast().next();
        }
        *result = true;
        true
    }
}

/// `SkOpCoincidence`.
pub struct Coincidence {
    head: CoinSpans,
    top: CoinSpans,
    global_state: *mut GlobalState,
}

impl Coincidence {
    pub fn new(global_state: *mut GlobalState) -> Self {
        Coincidence { head: CoinSpans::NULL, top: CoinSpans::NULL, global_state }
    }

    fn global_state(&self) -> &'static mut GlobalState {
        unsafe { &mut *self.global_state }
    }

    pub fn clear(&mut self) {
        self.head = CoinSpans::NULL;
        self.top = CoinSpans::NULL;
    }

    pub fn is_empty(&self) -> bool {
        self.head.is_null() && self.top.is_null()
    }

    /// If there is an existing pair that overlaps the addition, extend it.
    pub fn extend(&mut self, mut coin_pt_t_start: PtT, mut coin_pt_t_end: PtT, mut opp_pt_t_start: PtT, mut opp_pt_t_end: PtT) -> bool {
        let mut test = self.head;
        if test.is_null() {
            return false;
        }
        let mut coin_seg = coin_pt_t_start.segment();
        let mut opp_seg = opp_pt_t_start.segment();
        if !Coincidence::ordered_pt_t(coin_pt_t_start, opp_pt_t_start) {
            std::mem::swap(&mut coin_seg, &mut opp_seg);
            std::mem::swap(&mut coin_pt_t_start, &mut opp_pt_t_start);
            std::mem::swap(&mut coin_pt_t_end, &mut opp_pt_t_end);
            if coin_pt_t_start.t() > coin_pt_t_end.t() {
                std::mem::swap(&mut coin_pt_t_start, &mut coin_pt_t_end);
                std::mem::swap(&mut opp_pt_t_start, &mut opp_pt_t_end);
            }
        }
        let opp_min_t = smin(opp_pt_t_start.t(), opp_pt_t_end.t());
        loop {
            if coin_seg == test.coin_pt_t_start().segment() && opp_seg == test.opp_pt_t_start().segment() {
                let o_test_min_t = smin(test.opp_pt_t_start().t(), test.opp_pt_t_end().t());
                let o_test_max_t = smax(test.opp_pt_t_start().t(), test.opp_pt_t_end().t());
                if (test.coin_pt_t_start().t() <= coin_pt_t_end.t() && coin_pt_t_start.t() <= test.coin_pt_t_end().t())
                    || (o_test_min_t <= o_test_max_t && opp_min_t <= o_test_max_t)
                {
                    test.extend(coin_pt_t_start, coin_pt_t_end, opp_pt_t_start, opp_pt_t_end);
                    return true;
                }
            }
            test = test.next();
            if test.is_null() {
                break;
            }
        }
        false
    }

    /// Adds a new coincident pair.
    pub fn add(&mut self, mut coin_pt_t_start: PtT, mut coin_pt_t_end: PtT, mut opp_pt_t_start: PtT, mut opp_pt_t_end: PtT) {
        // OPTIMIZE: caller should have already sorted
        if !Coincidence::ordered_pt_t(coin_pt_t_start, opp_pt_t_start) {
            if opp_pt_t_start.t() < opp_pt_t_end.t() {
                self.add(opp_pt_t_start, opp_pt_t_end, coin_pt_t_start, coin_pt_t_end);
            } else {
                self.add(opp_pt_t_end, opp_pt_t_start, coin_pt_t_end, coin_pt_t_start);
            }
            return;
        }
        // choose the ptT at the front of the list to track
        coin_pt_t_start = coin_pt_t_start.span().pt_t();
        coin_pt_t_end = coin_pt_t_end.span().pt_t();
        opp_pt_t_start = opp_pt_t_start.span().pt_t();
        opp_pt_t_end = opp_pt_t_end.span().pt_t();
        let coin_rec = CoinSpans(self.global_state().allocator.make(CoinSpansData::default()));
        coin_rec.set(self.head, coin_pt_t_start, coin_pt_t_end, opp_pt_t_start, opp_pt_t_end);
        self.head = coin_rec;
    }

    fn add_end_moved_spans_base(&mut self, base: Span, test_span: Span) -> bool {
        let mut test_pt_t = test_span.pt_t();
        let stop_pt_t = test_pt_t;
        let base_seg = base.segment();
        let mut escape_hatch = 100000; // this is 100 times larger than the debugLoopLimit test
        loop {
            test_pt_t = test_pt_t.next();
            if test_pt_t == stop_pt_t {
                break;
            }
            escape_hatch -= 1;
            if escape_hatch <= 0 {
                return false; // if triggered (likely by a fuzz-generated test) too complex to succeed
            }
            let test_seg = test_pt_t.segment();
            if test_pt_t.deleted() {
                continue;
            }
            if test_seg == base_seg {
                continue;
            }
            if test_pt_t.span().pt_t() != test_pt_t {
                continue;
            }
            if self.contains_seg(base_seg, test_seg, test_pt_t.t()) {
                continue;
            }
            // intersect perp with base->ptT() with testPtT->segment()
            let dxdy = base_seg.d_slope_at_t(base.t());
            let pt = base.pt();
            let ray = DLine::new(
                DPoint::new(pt.x as f64, pt.y as f64),
                DPoint::new(pt.x as f64 + dxdy.y, pt.y as f64 - dxdy.x),
            );
            let mut i = Intersections::default();
            curve_intersect_ray(test_seg.verb(), test_seg.pts(), test_seg.weight(), &ray, &mut i);
            for index in 0..i.used() as usize {
                let t = i.t(0, index);
                if !between(0.0, t, 1.0) {
                    continue;
                }
                let opp_pt = i.pt(index);
                if !opp_pt.approximately_equal_pt(pt) {
                    continue;
                }
                let opp_start = test_seg.add_t(t);
                if opp_start == test_pt_t {
                    continue;
                }
                opp_start.span().add_opp(base);
                if opp_start.deleted() {
                    continue;
                }
                let mut coin_seg = base.segment();
                let mut opp_seg = opp_start.segment();
                let (mut coin_ts, mut coin_te, mut opp_ts, mut opp_te);
                if Coincidence::Ordered(coin_seg, opp_seg) {
                    coin_ts = base.t();
                    coin_te = test_span.t();
                    opp_ts = opp_start.t();
                    opp_te = test_pt_t.t();
                } else {
                    std::mem::swap(&mut coin_seg, &mut opp_seg);
                    coin_ts = opp_start.t();
                    coin_te = test_pt_t.t();
                    opp_ts = base.t();
                    opp_te = test_span.t();
                }
                if coin_ts > coin_te {
                    std::mem::swap(&mut coin_ts, &mut coin_te);
                    std::mem::swap(&mut opp_ts, &mut opp_te);
                }
                let mut added = false;
                if !self.add_or_overlap(coin_seg, opp_seg, coin_ts, coin_te, opp_ts, opp_te, &mut added) {
                    return false;
                }
            }
        }
        true
    }

    fn add_end_moved_spans_pt_t(&mut self, pt_t: PtT) -> bool {
        if pt_t.span().up_castable().is_null() {
            return false;
        }
        let base = pt_t.span().up_cast();
        let prev = base.prev();
        if prev.is_null() {
            return false;
        }
        if !prev.is_canceled() && !self.add_end_moved_spans_base(base, base.prev()) {
            return false;
        }
        if !base.is_canceled() && !self.add_end_moved_spans_base(base, base.next()) {
            return false;
        }
        true
    }

    /// If A is coincident with B and B includes an endpoint, and A's
    /// matching point is not the endpoint, assume that the same implied line
    /// may intersect another curve close to B.
    pub fn add_end_moved_spans(&mut self) -> bool {
        let mut span = self.head;
        if span.is_null() {
            return true;
        }
        self.top = span;
        self.head = CoinSpans::NULL;
        loop {
            if span.coin_pt_t_start().pt() != span.opp_pt_t_start().pt() {
                if 1.0 == span.coin_pt_t_start().t() {
                    return false;
                }
                let on_end = span.coin_pt_t_start().t() == 0.0;
                let o_on_end = zero_or_one(span.opp_pt_t_start().t());
                if on_end {
                    if !o_on_end && !self.add_end_moved_spans_pt_t(span.opp_pt_t_start()) {
                        return false;
                    }
                } else if o_on_end && !self.add_end_moved_spans_pt_t(span.coin_pt_t_start()) {
                    return false;
                }
            }
            if span.coin_pt_t_end().pt() != span.opp_pt_t_end().pt() {
                let on_end = span.coin_pt_t_end().t() == 1.0;
                let o_on_end = zero_or_one(span.opp_pt_t_end().t());
                if on_end {
                    if !o_on_end && !self.add_end_moved_spans_pt_t(span.opp_pt_t_end()) {
                        return false;
                    }
                } else if o_on_end && !self.add_end_moved_spans_pt_t(span.coin_pt_t_end()) {
                    return false;
                }
            }
            span = span.next();
            if span.is_null() {
                break;
            }
        }
        self.restore_head();
        true
    }

    /// For each coincident pair, match the spans; if the spans don't match,
    /// add the missing pt to the segment and loop it in the opposite span.
    pub fn add_expanded(&mut self) -> bool {
        let mut coin = self.head;
        if coin.is_null() {
            return true;
        }
        loop {
            let start_pt_t = coin.coin_pt_t_start();
            let o_start_pt_t = coin.opp_pt_t_start();
            let mut prior_t = start_pt_t.t();
            let mut o_prior_t = o_start_pt_t.t();
            if !start_pt_t.contains_pt_t(o_start_pt_t) {
                return false;
            }
            let start = start_pt_t.span();
            let o_start = o_start_pt_t.span();
            let mut end = coin.coin_pt_t_end().span();
            let mut o_end = coin.opp_pt_t_end().span();
            if o_end.deleted() {
                return false;
            }
            if start.up_castable().is_null() {
                return false;
            }
            let mut test = start.up_cast().next();
            if !coin.flipped() && o_start.up_castable().is_null() {
                return false;
            }
            let mut o_test = if coin.flipped() { o_start.prev() } else { o_start.up_cast().next() };
            if o_test.is_null() {
                return false;
            }
            let seg = start.segment();
            let o_seg = o_start.segment();
            while test != end || o_test != o_end {
                let contained_opp = test.pt_t().contains_seg(o_seg);
                let contained_this = o_test.pt_t().contains_seg(seg);
                if contained_opp.is_null() || contained_this.is_null() {
                    // choose the ends, or the first common pt-t list shared by both
                    let (next_t, o_next_t);
                    if contained_opp.is_some() {
                        next_t = test.t();
                        o_next_t = contained_opp.t();
                    } else if contained_this.is_some() {
                        next_t = contained_this.t();
                        o_next_t = o_test.t();
                    } else {
                        // iterate through until a pt-t list found that contains the other
                        let mut walk = test;
                        let mut walk_opp;
                        loop {
                            if walk.up_castable().is_null() {
                                return false;
                            }
                            walk = walk.up_cast().next();
                            walk_opp = walk.pt_t().contains_seg(o_seg);
                            if !(walk_opp.is_null() && walk != coin.coin_pt_t_end().span()) {
                                break;
                            }
                        }
                        if walk_opp.is_null() {
                            return false;
                        }
                        next_t = walk.t();
                        o_next_t = walk_opp.t();
                    }
                    // use t ranges to guess which one is missing
                    let start_range = next_t - prior_t;
                    if start_range == 0.0 {
                        return false;
                    }
                    let start_part = (test.t() - prior_t) / start_range;
                    let o_start_range = o_next_t - o_prior_t;
                    if o_start_range == 0.0 {
                        return false;
                    }
                    let o_start_part = (o_test.t() - o_prior_t) / o_start_range;
                    if start_part == o_start_part {
                        return false;
                    }
                    let add_to_opp = if contained_opp.is_null() && contained_this.is_null() {
                        start_part < o_start_part
                    } else {
                        contained_this.is_some()
                    };
                    let mut start_over = false;
                    let success = if add_to_opp {
                        o_seg.add_expanded(o_prior_t + o_start_range * start_part, test, &mut start_over)
                    } else {
                        seg.add_expanded(prior_t + start_range * o_start_part, o_test, &mut start_over)
                    };
                    if !success {
                        return false;
                    }
                    if start_over {
                        test = start;
                        o_test = o_start;
                    }
                    end = coin.coin_pt_t_end().span();
                    o_end = coin.opp_pt_t_end().span();
                }
                if test != end {
                    if test.up_castable().is_null() {
                        return false;
                    }
                    prior_t = test.t();
                    test = test.up_cast().next();
                }
                if o_test != o_end {
                    o_prior_t = o_test.t();
                    if coin.flipped() {
                        o_test = o_test.prev();
                    } else {
                        if o_test.up_castable().is_null() {
                            return false;
                        }
                        o_test = o_test.up_cast().next();
                    }
                    if o_test.is_null() {
                        return false;
                    }
                }
            }
            coin = coin.next();
            if coin.is_null() {
                break;
            }
        }
        true
    }

    /// Given a t span, map the same range on the coincident span.
    #[allow(non_snake_case)]
    fn TRange(over_s: PtT, t: f64, coin_seg: Segment) -> f64 {
        let mut work = over_s.span();
        let mut found_start = PtT::NULL;
        let mut found_end = PtT::NULL;
        let mut coin_start = PtT::NULL;
        let mut coin_end = PtT::NULL;
        loop {
            let contained = work.contains_seg(coin_seg);
            if contained.is_null() {
                if work.is_final() {
                    break;
                }
            } else {
                if work.t() <= t {
                    coin_start = contained;
                    found_start = work.pt_t();
                }
                if work.t() >= t {
                    coin_end = contained;
                    found_end = work.pt_t();
                    break;
                }
            }
            work = work.up_cast().next();
            if work.is_null() {
                break;
            }
        }
        if coin_start.is_null() || coin_end.is_null() {
            return 1.0;
        }
        // while overS->fT <=t and overS contains coinSeg
        let denom = found_end.t() - found_start.t();
        let s_ratio = if denom != 0.0 { (t - found_start.t()) / denom } else { 1.0 };
        coin_start.t() + (coin_end.t() - coin_start.t()) * s_ratio
    }

    /// Return true if span overlaps existing and needs to adjust the
    /// coincident list.
    #[allow(clippy::too_many_arguments)]
    fn check_overlap(
        &self,
        mut check: CoinSpans,
        coin_seg: Segment,
        opp_seg: Segment,
        coin_ts: f64,
        coin_te: f64,
        mut opp_ts: f64,
        mut opp_te: f64,
        overlaps: &mut Vec<CoinSpans>,
    ) -> bool {
        if !Coincidence::Ordered(coin_seg, opp_seg) {
            if opp_ts < opp_te {
                return self.check_overlap(check, opp_seg, coin_seg, opp_ts, opp_te, coin_ts, coin_te, overlaps);
            }
            return self.check_overlap(check, opp_seg, coin_seg, opp_te, opp_ts, coin_te, coin_ts, overlaps);
        }
        let swap_opp = opp_ts > opp_te;
        if swap_opp {
            std::mem::swap(&mut opp_ts, &mut opp_te);
        }
        loop {
            'next: {
                if check.coin_pt_t_start().segment() != coin_seg {
                    break 'next;
                }
                if check.opp_pt_t_start().segment() != opp_seg {
                    break 'next;
                }
                let check_ts = check.coin_pt_t_start().t();
                let check_te = check.coin_pt_t_end().t();
                let coin_outside = coin_te < check_ts || coin_ts > check_te;
                let mut o_check_ts = check.opp_pt_t_start().t();
                let mut o_check_te = check.opp_pt_t_end().t();
                if swap_opp {
                    if o_check_ts <= o_check_te {
                        return false;
                    }
                    std::mem::swap(&mut o_check_ts, &mut o_check_te);
                }
                let opp_outside = opp_te < o_check_ts || opp_ts > o_check_te;
                if coin_outside && opp_outside {
                    break 'next;
                }
                let coin_inside = coin_te <= check_te && coin_ts >= check_ts;
                let opp_inside = opp_te <= o_check_te && opp_ts >= o_check_ts;
                if coin_inside && opp_inside {
                    // already included, do nothing
                    return false;
                }
                overlaps.push(check); // partial overlap, extend existing entry
            }
            check = check.next();
            if check.is_null() {
                break;
            }
        }
        true
    }

    /// Please keep this in sync with debugAddIfMissing(). Note that over1s,
    /// over1e, over2s, over2e are ordered.
    #[allow(clippy::too_many_arguments)]
    fn add_if_missing(
        &mut self,
        over1s: PtT,
        over2s: PtT,
        t_start: f64,
        t_end: f64,
        coin_seg: Segment,
        opp_seg: Segment,
        added: &mut bool,
    ) -> bool {
        let mut coin_ts = Coincidence::TRange(over1s, t_start, coin_seg);
        let mut coin_te = Coincidence::TRange(over1s, t_end, coin_seg);
        let mut result = coin_seg.collapsed(coin_ts, coin_te);
        if Collapsed::No != result {
            return Collapsed::Yes == result;
        }
        let mut opp_ts = Coincidence::TRange(over2s, t_start, opp_seg);
        let mut opp_te = Coincidence::TRange(over2s, t_end, opp_seg);
        result = opp_seg.collapsed(opp_ts, opp_te);
        if Collapsed::No != result {
            return Collapsed::Yes == result;
        }
        if coin_ts > coin_te {
            std::mem::swap(&mut coin_ts, &mut coin_te);
            std::mem::swap(&mut opp_ts, &mut opp_te);
        }
        let _ = self.add_or_overlap(coin_seg, opp_seg, coin_ts, coin_te, opp_ts, opp_te, added);
        true
    }

    /// Please keep this in sync with debugAddOrOverlap(). If this is called
    /// by addEndMovedSpans(), a returned false propagates out to an abort;
    /// if by AddIfMissing(), it indicates there was nothing to add.
    #[allow(clippy::too_many_arguments)]
    fn add_or_overlap(
        &mut self,
        coin_seg: Segment,
        opp_seg: Segment,
        coin_ts: f64,
        coin_te: f64,
        opp_ts: f64,
        opp_te: f64,
        added: &mut bool,
    ) -> bool {
        let mut overlaps: Vec<CoinSpans> = Vec::new();
        if self.top.is_null() {
            return false;
        }
        if !self.check_overlap(self.top, coin_seg, opp_seg, coin_ts, coin_te, opp_ts, opp_te, &mut overlaps) {
            return true;
        }
        if self.head.is_some()
            && !self.check_overlap(self.head, coin_seg, opp_seg, coin_ts, coin_te, opp_ts, opp_te, &mut overlaps)
        {
            return true;
        }
        let overlap = if !overlaps.is_empty() { overlaps[0] } else { CoinSpans::NULL };
        for &test in overlaps.iter().skip(1) {
            // combine overlaps before continuing
            if overlap.coin_pt_t_start().t() > test.coin_pt_t_start().t() {
                overlap.set_coin_pt_t_start(test.coin_pt_t_start());
            }
            if overlap.coin_pt_t_end().t() < test.coin_pt_t_end().t() {
                overlap.set_coin_pt_t_end(test.coin_pt_t_end());
            }
            if if overlap.flipped() {
                overlap.opp_pt_t_start().t() < test.opp_pt_t_start().t()
            } else {
                overlap.opp_pt_t_start().t() > test.opp_pt_t_start().t()
            } {
                overlap.set_opp_pt_t_start(test.opp_pt_t_start());
            }
            if if overlap.flipped() {
                overlap.opp_pt_t_end().t() > test.opp_pt_t_end().t()
            } else {
                overlap.opp_pt_t_end().t() < test.opp_pt_t_end().t()
            } {
                overlap.set_opp_pt_t_end(test.opp_pt_t_end());
            }
            if self.head.is_null() || !self.release(self.head, test) {
                let top = self.top;
                self.release(top, test);
            }
        }
        let mut cs = coin_seg.existing(coin_ts, opp_seg);
        let mut ce = coin_seg.existing(coin_te, opp_seg);
        if overlap.is_some() && cs.is_some() && ce.is_some() && overlap.contains(cs, ce) {
            return true;
        }
        if cs == ce && cs.is_some() {
            return false;
        }
        let mut os = opp_seg.existing(opp_ts, coin_seg);
        let mut oe = opp_seg.existing(opp_te, coin_seg);
        if overlap.is_some() && os.is_some() && oe.is_some() && overlap.contains(os, oe) {
            return true;
        }
        if cs.is_some() && cs.deleted() {
            return false;
        }
        if os.is_some() && os.deleted() {
            return false;
        }
        if ce.is_some() && ce.deleted() {
            return false;
        }
        if oe.is_some() && oe.deleted() {
            return false;
        }
        let cs_existing = if cs.is_null() { coin_seg.existing(coin_ts, Segment::NULL) } else { PtT::NULL };
        let ce_existing = if ce.is_null() { coin_seg.existing(coin_te, Segment::NULL) } else { PtT::NULL };
        if cs_existing.is_some() && cs_existing == ce_existing {
            return false;
        }
        if ce_existing.is_some()
            && (ce_existing == cs || ce_existing.contains_pt_t(if cs_existing.is_some() { cs_existing } else { cs }))
        {
            return false;
        }
        let os_existing = if os.is_null() { opp_seg.existing(opp_ts, Segment::NULL) } else { PtT::NULL };
        let oe_existing = if oe.is_null() { opp_seg.existing(opp_te, Segment::NULL) } else { PtT::NULL };
        if os_existing.is_some() && os_existing == oe_existing {
            return false;
        }
        if os_existing.is_some()
            && (os_existing == oe || os_existing.contains_pt_t(if oe_existing.is_some() { oe_existing } else { oe }))
        {
            return false;
        }
        if oe_existing.is_some()
            && (oe_existing == os || oe_existing.contains_pt_t(if os_existing.is_some() { os_existing } else { os }))
        {
            return false;
        }
        if cs.is_null() || os.is_null() {
            let cs_writable = if cs.is_some() { cs } else { coin_seg.add_t(coin_ts) };
            if cs_writable == ce {
                return true;
            }
            let os_writable = if os.is_some() { os } else { opp_seg.add_t(opp_ts) };
            if cs_writable.is_null() || os_writable.is_null() {
                return false;
            }
            cs_writable.span().add_opp(os_writable.span());
            cs = cs_writable;
            os = os_writable.active();
            if os.is_null() {
                return false;
            }
            if (ce.is_some() && ce.deleted()) || (oe.is_some() && oe.deleted()) {
                return false;
            }
        }
        if ce.is_null() || oe.is_null() {
            let ce_writable = if ce.is_some() { ce } else { coin_seg.add_t(coin_te) };
            let oe_writable = if oe.is_some() { oe } else { opp_seg.add_t(opp_te) };
            if !ce_writable.span().add_opp(oe_writable.span()) {
                return false;
            }
            ce = ce_writable;
            oe = oe_writable;
        }
        if cs.deleted() || os.deleted() || ce.deleted() || oe.deleted() {
            return false;
        }
        if cs.contains_pt_t(ce) || os.contains_pt_t(oe) {
            return false;
        }
        let mut result = true;
        if overlap.is_some() {
            if overlap.coin_pt_t_start().segment() == coin_seg {
                result = overlap.extend(cs, ce, os, oe);
            } else {
                if os.t() > oe.t() {
                    std::mem::swap(&mut cs, &mut ce);
                    std::mem::swap(&mut os, &mut oe);
                }
                result = overlap.extend(os, oe, cs, ce);
            }
        } else {
            self.add(cs, ce, os, oe);
        }
        if result {
            *added = true;
        }
        true
    }

    /// Please keep this in sync with debugAddMissing(). Detects overlaps of
    /// different coincident runs on the same segment; returns true if the
    /// caller should loop again.
    pub fn add_missing(&mut self, added: &mut bool) -> bool {
        let mut outer = self.head;
        *added = false;
        if outer.is_null() {
            return true;
        }
        self.top = outer;
        self.head = CoinSpans::NULL;
        loop {
            // addifmissing can modify the list that this is walking
            // save head so that walker can iterate over old data unperturbed
            // addifmissing adds to head freely then add saved head in the end
            let ocs = outer.coin_pt_t_start();
            if ocs.deleted() {
                return false;
            }
            let outer_coin = ocs.segment();
            if outer_coin.done() {
                return false;
            }
            let oos = outer.opp_pt_t_start();
            if oos.deleted() {
                return true;
            }
            let outer_opp = oos.segment();
            let mut inner = outer;
            loop {
                inner = inner.next();
                if inner.is_null() {
                    break;
                }
                let mut over_s = 0.0;
                let mut over_e = 0.0;
                let ics = inner.coin_pt_t_start();
                if ics.deleted() {
                    return false;
                }
                let inner_coin = ics.segment();
                if inner_coin.done() {
                    return false;
                }
                let ios = inner.opp_pt_t_start();
                if ios.deleted() {
                    return false;
                }
                let inner_opp = ios.segment();
                if outer_coin == inner_coin {
                    let oce = outer.coin_pt_t_end();
                    if oce.deleted() {
                        return true;
                    }
                    let ice = inner.coin_pt_t_end();
                    if ice.deleted() {
                        return false;
                    }
                    if outer_opp != inner_opp
                        && self.overlap(ocs, oce, ics, ice, &mut over_s, &mut over_e)
                        && !self.add_if_missing(ocs.starter(oce), ics.starter(ice), over_s, over_e, outer_opp, inner_opp, added)
                    {
                        return false;
                    }
                } else if outer_coin == inner_opp {
                    let oce = outer.coin_pt_t_end();
                    if oce.deleted() {
                        return false;
                    }
                    let ioe = inner.opp_pt_t_end();
                    if ioe.deleted() {
                        return false;
                    }
                    if outer_opp != inner_coin
                        && self.overlap(ocs, oce, ios, ioe, &mut over_s, &mut over_e)
                        && !self.add_if_missing(ocs.starter(oce), ios.starter(ioe), over_s, over_e, outer_opp, inner_coin, added)
                    {
                        return false;
                    }
                } else if outer_opp == inner_coin {
                    let ooe = outer.opp_pt_t_end();
                    if ooe.deleted() {
                        return false;
                    }
                    let ice = inner.coin_pt_t_end();
                    if ice.deleted() {
                        return false;
                    }
                    if self.overlap(oos, ooe, ics, ice, &mut over_s, &mut over_e)
                        && !self.add_if_missing(oos.starter(ooe), ics.starter(ice), over_s, over_e, outer_coin, inner_opp, added)
                    {
                        return false;
                    }
                } else if outer_opp == inner_opp {
                    let ooe = outer.opp_pt_t_end();
                    if ooe.deleted() {
                        return false;
                    }
                    let ioe = inner.opp_pt_t_end();
                    if ioe.deleted() {
                        return true;
                    }
                    if self.overlap(oos, ooe, ios, ioe, &mut over_s, &mut over_e)
                        && !self.add_if_missing(oos.starter(ooe), ios.starter(ioe), over_s, over_e, outer_coin, inner_coin, added)
                    {
                        return false;
                    }
                }
            }
            outer = outer.next();
            if outer.is_null() {
                break;
            }
        }
        self.restore_head();
        true
    }

    fn add_overlap(&mut self, seg1: Segment, seg1o: Segment, seg2: Segment, seg2o: Segment, over_s: PtT, over_e: PtT) -> bool {
        let mut s1 = over_s.find(seg1);
        let mut e1 = over_e.find(seg1);
        if s1.is_null() || e1.is_null() {
            return false;
        }
        if s1.starter(e1).span().up_cast().wind_value() == 0 {
            s1 = over_s.find(seg1o);
            e1 = over_e.find(seg1o);
            if s1.is_null() || e1.is_null() {
                return false;
            }
            if s1.starter(e1).span().up_cast().wind_value() == 0 {
                return true;
            }
        }
        let mut s2 = over_s.find(seg2);
        let mut e2 = over_e.find(seg2);
        if s2.is_null() || e2.is_null() {
            return false;
        }
        if s2.starter(e2).span().up_cast().wind_value() == 0 {
            s2 = over_s.find(seg2o);
            e2 = over_e.find(seg2o);
            if s2.is_null() || e2.is_null() {
                return false;
            }
            if s2.starter(e2).span().up_cast().wind_value() == 0 {
                return true;
            }
        }
        if s1.segment() == s2.segment() {
            return true;
        }
        if s1.t() > e1.t() {
            std::mem::swap(&mut s1, &mut e1);
            std::mem::swap(&mut s2, &mut e2);
        }
        self.add(s1, e1, s2, e2);
        true
    }

    fn contains_seg(&self, seg: Segment, opp: Segment, opp_t: f64) -> bool {
        Coincidence::contains_in(self.head, seg, opp, opp_t) || Coincidence::contains_in(self.top, seg, opp, opp_t)
    }

    fn contains_in(mut coin: CoinSpans, seg: Segment, opp: Segment, opp_t: f64) -> bool {
        if coin.is_null() {
            return false;
        }
        loop {
            if coin.coin_pt_t_start().segment() == seg
                && coin.opp_pt_t_start().segment() == opp
                && between(coin.opp_pt_t_start().t(), opp_t, coin.opp_pt_t_end().t())
            {
                return true;
            }
            if coin.opp_pt_t_start().segment() == seg
                && coin.coin_pt_t_start().segment() == opp
                && between(coin.coin_pt_t_start().t(), opp_t, coin.coin_pt_t_end().t())
            {
                return true;
            }
            coin = coin.next();
            if coin.is_null() {
                return false;
            }
        }
    }

    pub fn contains(&self, mut coin_pt_t_start: PtT, mut coin_pt_t_end: PtT, mut opp_pt_t_start: PtT, mut opp_pt_t_end: PtT) -> bool {
        let mut test = self.head;
        if test.is_null() {
            return false;
        }
        let mut coin_seg = coin_pt_t_start.segment();
        let mut opp_seg = opp_pt_t_start.segment();
        if !Coincidence::ordered_pt_t(coin_pt_t_start, opp_pt_t_start) {
            std::mem::swap(&mut coin_seg, &mut opp_seg);
            std::mem::swap(&mut coin_pt_t_start, &mut opp_pt_t_start);
            std::mem::swap(&mut coin_pt_t_end, &mut opp_pt_t_end);
            if coin_pt_t_start.t() > coin_pt_t_end.t() {
                std::mem::swap(&mut coin_pt_t_start, &mut coin_pt_t_end);
                std::mem::swap(&mut opp_pt_t_start, &mut opp_pt_t_end);
            }
        }
        let opp_min_t = smin(opp_pt_t_start.t(), opp_pt_t_end.t());
        let opp_max_t = smax(opp_pt_t_start.t(), opp_pt_t_end.t());
        loop {
            if coin_seg == test.coin_pt_t_start().segment()
                && !(coin_pt_t_start.t() < test.coin_pt_t_start().t())
                && !(coin_pt_t_end.t() > test.coin_pt_t_end().t())
                && opp_seg == test.opp_pt_t_start().segment()
                && !(opp_min_t < smin(test.opp_pt_t_start().t(), test.opp_pt_t_end().t()))
                && !(opp_max_t > smax(test.opp_pt_t_start().t(), test.opp_pt_t_end().t()))
            {
                return true;
            }
            test = test.next();
            if test.is_null() {
                return false;
            }
        }
    }

    pub fn correct_ends(&mut self) {
        let mut coin = self.head;
        while coin.is_some() {
            coin.correct_ends();
            coin = coin.next();
        }
    }

    /// Walk span sets in parallel, moving winding from one to the other.
    pub fn apply(&mut self) -> bool {
        let mut coin = self.head;
        if coin.is_null() {
            return true;
        }
        loop {
            'next_coin: {
                let start_span = coin.coin_pt_t_start().span();
                if start_span.up_castable().is_null() {
                    return false;
                }
                let mut start = start_span.up_cast();
                if start.deleted() {
                    break 'next_coin;
                }
                let end = coin.coin_pt_t_end().span();
                if start != start.starter(end) {
                    return false;
                }
                let flipped = coin.flipped();
                let o_start_base = (if flipped { coin.opp_pt_t_end() } else { coin.opp_pt_t_start() }).span();
                if o_start_base.up_castable().is_null() {
                    return false;
                }
                let mut o_start = o_start_base.up_cast();
                if o_start.deleted() {
                    break 'next_coin;
                }
                let o_end = (if flipped { coin.opp_pt_t_start() } else { coin.opp_pt_t_end() }).span();
                let segment = start.segment();
                let o_segment = o_start.segment();
                let operand_swap = segment.operand() != o_segment.operand();
                if flipped {
                    if o_end.deleted() {
                        break 'next_coin;
                    }
                    loop {
                        let o_next = o_start.next();
                        if o_next == o_end {
                            break;
                        }
                        if o_next.up_castable().is_null() {
                            return false;
                        }
                        o_start = o_next.up_cast();
                    }
                }
                loop {
                    let mut wind_value = start.wind_value();
                    let mut opp_value = start.opp_value();
                    let mut o_wind_value = o_start.wind_value();
                    let mut o_opp_value = o_start.opp_value();
                    // winding values are added or subtracted depending on direction and wind type
                    // same or opposite values are summed depending on the operand value
                    let mut wind_diff = if operand_swap { o_opp_value } else { o_wind_value };
                    let mut o_wind_diff = if operand_swap { opp_value } else { wind_value };
                    if !flipped {
                        wind_diff = -wind_diff;
                        o_wind_diff = -o_wind_diff;
                    }
                    let mut add_to_start = wind_value != 0
                        && (wind_value > wind_diff || (wind_value == wind_diff && o_wind_value <= o_wind_diff));
                    if if add_to_start { start.done() } else { o_start.done() } {
                        add_to_start ^= true;
                    }
                    if add_to_start {
                        if operand_swap {
                            std::mem::swap(&mut o_wind_value, &mut o_opp_value);
                        }
                        if flipped {
                            wind_value -= o_wind_value;
                            opp_value -= o_opp_value;
                        } else {
                            wind_value += o_wind_value;
                            opp_value += o_opp_value;
                        }
                        if segment.is_xor() {
                            wind_value &= 1;
                        }
                        if segment.opp_xor() {
                            opp_value &= 1;
                        }
                        o_wind_value = 0;
                        o_opp_value = 0;
                    } else {
                        if operand_swap {
                            std::mem::swap(&mut wind_value, &mut opp_value);
                        }
                        if flipped {
                            o_wind_value -= wind_value;
                            o_opp_value -= opp_value;
                        } else {
                            o_wind_value += wind_value;
                            o_opp_value += opp_value;
                        }
                        if o_segment.is_xor() {
                            o_wind_value &= 1;
                        }
                        if o_segment.opp_xor() {
                            o_opp_value &= 1;
                        }
                        wind_value = 0;
                        opp_value = 0;
                    }
                    if wind_value <= -1 {
                        return false;
                    }
                    start.set_wind_value(wind_value);
                    start.set_opp_value(opp_value);
                    if o_wind_value <= -1 {
                        return false;
                    }
                    o_start.set_wind_value(o_wind_value);
                    o_start.set_opp_value(o_opp_value);
                    if wind_value == 0 && opp_value == 0 {
                        segment.mark_done(start);
                    }
                    if o_wind_value == 0 && o_opp_value == 0 {
                        o_segment.mark_done(o_start);
                    }
                    let next = start.next();
                    let mut o_next = if flipped { o_start.prev() } else { o_start.next() };
                    if next == end {
                        break;
                    }
                    if next.up_castable().is_null() {
                        return false;
                    }
                    start = next.up_cast();
                    // if the opposite ran out too soon, just reuse the last span
                    if o_next.is_null() || o_next.up_castable().is_null() {
                        o_next = o_start;
                    }
                    o_start = o_next.up_cast();
                }
            }
            coin = coin.next();
            if coin.is_null() {
                break;
            }
        }
        true
    }

    /// Please keep this in sync with debugRelease().
    fn release(&mut self, mut coin: CoinSpans, remove: CoinSpans) -> bool {
        let head = coin;
        let mut prev = CoinSpans::NULL;
        loop {
            let next = coin.next();
            if coin == remove {
                if prev.is_some() {
                    prev.set_next(next);
                } else if head == self.head {
                    self.head = next;
                } else {
                    self.top = next;
                }
                break;
            }
            prev = coin;
            coin = next;
            if coin.is_null() {
                break;
            }
        }
        coin.is_some()
    }

    fn release_deleted_in(&mut self, mut coin: CoinSpans) {
        if coin.is_null() {
            return;
        }
        let head = coin;
        let mut prev = CoinSpans::NULL;
        loop {
            let next = coin.next();
            if coin.coin_pt_t_start().deleted() {
                if prev.is_some() {
                    prev.set_next(next);
                } else if head == self.head {
                    self.head = next;
                } else {
                    self.top = next;
                }
            } else {
                prev = coin;
            }
            coin = next;
            if coin.is_null() {
                break;
            }
        }
    }

    pub fn release_deleted(&mut self) {
        let head = self.head;
        self.release_deleted_in(head);
        let top = self.top;
        self.release_deleted_in(top);
    }

    fn restore_head(&mut self) {
        // append fTop to the end of fHead
        if self.head.is_null() {
            self.head = self.top;
        } else {
            let mut last = self.head;
            while last.next().is_some() {
                last = last.next();
            }
            last.set_next(self.top);
        }
        self.top = CoinSpans::NULL;
        // segments may have collapsed in the meantime; remove empty referenced segments
        let mut prev = CoinSpans::NULL;
        let mut test = self.head;
        while test.is_some() {
            if test.coin_pt_t_start().segment().done() || test.opp_pt_t_start().segment().done() {
                let next = test.next();
                if prev.is_some() {
                    prev.set_next(next);
                } else {
                    self.head = next;
                }
                test = next;
                continue;
            }
            prev = test;
            test = test.next();
        }
    }

    /// Please keep this in sync with debugExpand(). Expand the range by
    /// checking adjacent spans for coincidence.
    pub fn expand(&mut self) -> bool {
        let mut coin = self.head;
        if coin.is_null() {
            return false;
        }
        let mut expanded = false;
        loop {
            if coin.expand() {
                // check to see if multiple spans expanded so they are now identical
                let mut test = self.head;
                loop {
                    if coin != test
                        && coin.coin_pt_t_start() == test.coin_pt_t_start()
                        && coin.opp_pt_t_start() == test.opp_pt_t_start()
                    {
                        let head = self.head;
                        self.release(head, test);
                        break;
                    }
                    test = test.next();
                    if test.is_null() {
                        break;
                    }
                }
                expanded = true;
            }
            coin = coin.next();
            if coin.is_null() {
                break;
            }
        }
        expanded
    }

    pub fn find_overlaps(&self, overlaps: &mut Coincidence) -> bool {
        overlaps.head = CoinSpans::NULL;
        overlaps.top = CoinSpans::NULL;
        let mut outer = self.head;
        while outer.is_some() {
            let outer_coin = outer.coin_pt_t_start().segment();
            let outer_opp = outer.opp_pt_t_start().segment();
            let mut inner = outer;
            loop {
                inner = inner.next();
                if inner.is_null() {
                    break;
                }
                let inner_coin = inner.coin_pt_t_start().segment();
                if outer_coin == inner_coin {
                    continue; // both winners are the same segment, so there's no additional overlap
                }
                let inner_opp = inner.opp_pt_t_start().segment();
                let mut overlap_s = PtT::NULL;
                let mut overlap_e = PtT::NULL;
                if ((outer_opp == inner_coin
                    && PtT::Overlaps(
                        outer.opp_pt_t_start(),
                        outer.opp_pt_t_end(),
                        inner.coin_pt_t_start(),
                        inner.coin_pt_t_end(),
                        &mut overlap_s,
                        &mut overlap_e,
                    ))
                    || (outer_coin == inner_opp
                        && PtT::Overlaps(
                            outer.coin_pt_t_start(),
                            outer.coin_pt_t_end(),
                            inner.opp_pt_t_start(),
                            inner.opp_pt_t_end(),
                            &mut overlap_s,
                            &mut overlap_e,
                        ))
                    || (outer_opp == inner_opp
                        && PtT::Overlaps(
                            outer.opp_pt_t_start(),
                            outer.opp_pt_t_end(),
                            inner.opp_pt_t_start(),
                            inner.opp_pt_t_end(),
                            &mut overlap_s,
                            &mut overlap_e,
                        )))
                    && !overlaps.add_overlap(outer_coin, outer_opp, inner_coin, inner_opp, overlap_s, overlap_e)
                {
                    return false;
                }
            }
            outer = outer.next();
        }
        true
    }

    pub fn fix_up(&mut self, deleted: PtT, kept: PtT) {
        if self.head.is_some() {
            let head = self.head;
            self.fix_up_in(head, deleted, kept);
        }
        if self.top.is_some() {
            let top = self.top;
            self.fix_up_in(top, deleted, kept);
        }
    }

    fn fix_up_in(&mut self, mut coin: CoinSpans, deleted: PtT, kept: PtT) {
        let head = coin;
        loop {
            'next: {
                if coin.coin_pt_t_start() == deleted {
                    if coin.coin_pt_t_end().span() == kept.span() {
                        self.release(head, coin);
                        break 'next;
                    }
                    coin.set_coin_pt_t_start(kept);
                }
                if coin.coin_pt_t_end() == deleted {
                    if coin.coin_pt_t_start().span() == kept.span() {
                        self.release(head, coin);
                        break 'next;
                    }
                    coin.set_coin_pt_t_end(kept);
                }
                if coin.opp_pt_t_start() == deleted {
                    if coin.opp_pt_t_end().span() == kept.span() {
                        self.release(head, coin);
                        break 'next;
                    }
                    coin.set_opp_pt_t_start(kept);
                }
                if coin.opp_pt_t_end() == deleted {
                    if coin.opp_pt_t_start().span() == kept.span() {
                        self.release(head, coin);
                        break 'next;
                    }
                    coin.set_opp_pt_t_end(kept);
                }
            }
            coin = coin.next();
            if coin.is_null() {
                break;
            }
        }
    }

    /// Please keep this in sync with debugMark(). This sets up the
    /// coincidence links in the segments when the coincidence crosses
    /// multiple spans.
    pub fn mark(&mut self) -> bool {
        let mut coin = self.head;
        if coin.is_null() {
            return true;
        }
        loop {
            let start_base = coin.coin_pt_t_start().span();
            if start_base.up_castable().is_null() {
                return false;
            }
            let start = start_base.up_cast();
            if start.deleted() {
                return false;
            }
            let end = coin.coin_pt_t_end().span();
            let mut o_start = coin.opp_pt_t_start().span();
            let mut o_end = coin.opp_pt_t_end().span();
            if o_end.deleted() {
                return false;
            }
            let flipped = coin.flipped();
            if flipped {
                std::mem::swap(&mut o_start, &mut o_end);
            }
            // coin and opp spans may not match up. Mark the ends, and then let the interior
            // get marked as many times as the spans allow
            if o_start.up_castable().is_null() {
                return false;
            }
            start.insert_coincidence(o_start.up_cast());
            end.insert_coin_end(o_end);
            let segment = start.segment();
            let o_segment = o_start.segment();
            let mut next = start;
            let mut o_next = o_start;
            let mut ordered = false;
            if !coin.ordered(&mut ordered) {
                return false;
            }
            loop {
                next = next.up_cast().next();
                if next == end {
                    break;
                }
                if next.up_castable().is_null() {
                    return false;
                }
                if !next.up_cast().insert_coincidence_seg(o_segment, flipped, ordered) {
                    return false;
                }
            }
            loop {
                o_next = o_next.up_cast().next();
                if o_next == o_end {
                    break;
                }
                if o_next.up_castable().is_null() {
                    return false;
                }
                if !o_next.up_cast().insert_coincidence_seg(segment, flipped, ordered) {
                    return false;
                }
            }
            coin = coin.next();
            if coin.is_null() {
                break;
            }
        }
        true
    }

    /// Please keep in sync with debugMarkCollapsed().
    fn mark_collapsed_in(&mut self, mut coin: CoinSpans, test: PtT) {
        let head = coin;
        while coin.is_some() {
            if coin.collapsed(test) {
                if zero_or_one(coin.coin_pt_t_start().t()) && zero_or_one(coin.coin_pt_t_end().t()) {
                    coin.coin_pt_t_start().segment().mark_all_done();
                }
                if zero_or_one(coin.opp_pt_t_start().t()) && zero_or_one(coin.opp_pt_t_end().t()) {
                    coin.opp_pt_t_start().segment().mark_all_done();
                }
                self.release(head, coin);
            }
            coin = coin.next();
        }
    }

    pub fn mark_collapsed(&mut self, test: PtT) {
        let head = self.head;
        self.mark_collapsed_in(head, test);
        let top = self.top;
        self.mark_collapsed_in(top, test);
    }

    pub fn ordered_pt_t(coin_pt_t_start: PtT, opp_pt_t_start: PtT) -> bool {
        Coincidence::Ordered(coin_pt_t_start.segment(), opp_pt_t_start.segment())
    }

    #[allow(non_snake_case)]
    pub fn Ordered(coin_seg: Segment, opp_seg: Segment) -> bool {
        let (cv, ov) = (verb_int(coin_seg.verb()), verb_int(opp_seg.verb()));
        if cv < ov {
            return true;
        }
        if cv > ov {
            return false;
        }
        let count = verb_to_points(coin_seg.verb()) + 1;
        let c_pts = coin_seg.pts();
        let o_pts = opp_seg.pts();
        for index in 0..count {
            for (c, o) in [(c_pts[index].x, o_pts[index].x), (c_pts[index].y, o_pts[index].y)] {
                if c < o {
                    return true;
                }
                if c > o {
                    return false;
                }
            }
        }
        true
    }

    fn overlap(&self, coin1s: PtT, coin1e: PtT, coin2s: PtT, coin2e: PtT, over_s: &mut f64, over_e: &mut f64) -> bool {
        *over_s = smax(smin(coin1s.t(), coin1e.t()), smin(coin2s.t(), coin2e.t()));
        *over_e = smin(smax(coin1s.t(), coin1e.t()), smax(coin2s.t(), coin2e.t()));
        *over_s < *over_e
    }

    /// Commented-out lines keep this in sync with debugRelease().
    pub fn release_seg(&mut self, deleted: Segment) {
        let mut coin = self.head;
        if coin.is_null() {
            return;
        }
        loop {
            if coin.coin_pt_t_start().segment() == deleted
                || coin.coin_pt_t_end().segment() == deleted
                || coin.opp_pt_t_start().segment() == deleted
                || coin.opp_pt_t_end().segment() == deleted
            {
                let head = self.head;
                self.release(head, coin);
            }
            coin = coin.next();
            if coin.is_null() {
                break;
            }
        }
    }
}
