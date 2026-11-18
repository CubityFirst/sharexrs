//! `SkOpSpan.h`, `SkOpSpan.cpp`: `SkOpPtT`, `SkOpSpanBase`, `SkOpSpan`.

use super::coincidence::Coincidence;
use super::graph::*;
use super::types::*;
use crate::geom::Point;

/// `SkOpSpanBase::Collapsed`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Collapsed {
    No,
    Yes,
    Error,
}

impl PtT {
    #[inline]
    pub fn t(self) -> f64 {
        self.d().t
    }

    #[inline]
    pub fn pt(self) -> Point {
        self.d().pt
    }

    pub fn active(self) -> PtT {
        if !self.d().deleted {
            return self;
        }
        let mut pt_t = self;
        let stop_pt_t = pt_t;
        loop {
            pt_t = pt_t.next();
            if pt_t == stop_pt_t {
                break;
            }
            if pt_t.d().span == self.d().span && !pt_t.d().deleted {
                return pt_t;
            }
        }
        PtT::NULL // should never return deleted; caller must abort
    }

    /// Please keep in sync with debugAddOpp().
    pub fn add_opp(self, opp: PtT, opp_prev: PtT) {
        let old_next = self.d().next;
        self.d().next = opp;
        opp_prev.d().next = old_next;
    }

    pub fn alias(self) -> bool {
        self.span().pt_t() != self
    }

    #[inline]
    pub fn coincident(self) -> bool {
        self.d().coincident
    }

    pub fn contains_pt_t(self, check: PtT) -> bool {
        let mut pt_t = self;
        let stop_pt_t = pt_t;
        loop {
            pt_t = pt_t.next();
            if pt_t == stop_pt_t {
                return false;
            }
            if pt_t == check {
                return true;
            }
        }
    }

    pub fn contains_seg_pt(self, segment: Segment, pt: Point) -> bool {
        let mut pt_t = self;
        let stop_pt_t = pt_t;
        loop {
            pt_t = pt_t.next();
            if pt_t == stop_pt_t {
                return false;
            }
            if pt_t.d().pt == pt && pt_t.segment() == segment {
                return true;
            }
        }
    }

    pub fn contains_seg_t(self, segment: Segment, t: f64) -> bool {
        let mut pt_t = self;
        let stop_pt_t = pt_t;
        loop {
            pt_t = pt_t.next();
            if pt_t == stop_pt_t {
                return false;
            }
            if pt_t.d().t == t && pt_t.segment() == segment {
                return true;
            }
        }
    }

    /// `contains(const SkOpSegment*)`.
    pub fn contains_seg(self, check: Segment) -> PtT {
        let mut pt_t = self;
        let stop_pt_t = pt_t;
        loop {
            pt_t = pt_t.next();
            if pt_t == stop_pt_t {
                return PtT::NULL;
            }
            if pt_t.segment() == check && !pt_t.deleted() {
                return pt_t;
            }
        }
    }

    pub fn contour(self) -> Contour {
        self.segment().contour()
    }

    #[inline]
    pub fn deleted(self) -> bool {
        self.d().deleted
    }

    #[inline]
    pub fn duplicate(self) -> bool {
        self.d().duplicate_pt
    }

    pub fn find(self, segment: Segment) -> PtT {
        let mut pt_t = self;
        let stop_pt_t = pt_t;
        loop {
            if pt_t.segment() == segment && !pt_t.deleted() {
                return pt_t;
            }
            pt_t = pt_t.d().next;
            if stop_pt_t == pt_t {
                break;
            }
        }
        PtT::NULL
    }

    pub fn global_state(self) -> &'static mut GlobalState {
        self.contour().global_state()
    }

    pub fn init(self, span: Span, t: f64, pt: Point, duplicate: bool) {
        let d = self.d();
        d.t = t;
        d.pt = pt;
        d.span = span;
        d.next = self;
        d.duplicate_pt = duplicate;
        d.deleted = false;
        d.coincident = false;
    }

    pub fn insert(self, span: PtT) {
        span.d().next = self.d().next;
        self.d().next = span;
    }

    #[inline]
    pub fn next(self) -> PtT {
        self.d().next
    }

    pub fn on_end(self) -> bool {
        let span = self.span();
        if span.pt_t() != self {
            return false;
        }
        let segment = self.segment();
        span == segment.head() || span == segment.tail()
    }

    /// Returns null if this is already in the opp ptT loop.
    pub fn opp_prev(self, opp: PtT) -> PtT {
        // find the fOpp ptr to opp
        let mut opp_prev = opp.d().next;
        if opp_prev == self {
            return PtT::NULL;
        }
        while opp_prev.d().next != opp {
            opp_prev = opp_prev.d().next;
            if opp_prev == self {
                return PtT::NULL;
            }
        }
        opp_prev
    }

    #[allow(non_snake_case)]
    pub fn Overlaps(s1: PtT, e1: PtT, s2: PtT, e2: PtT, s_out: &mut PtT, e_out: &mut PtT) -> bool {
        let start1 = if s1.t() < e1.t() { s1 } else { e1 };
        let start2 = if s2.t() < e2.t() { s2 } else { e2 };
        *s_out = if between(s1.t(), start2.t(), e1.t()) {
            start2
        } else if between(s2.t(), start1.t(), e2.t()) {
            start1
        } else {
            PtT::NULL
        };
        let end1 = if s1.t() < e1.t() { e1 } else { s1 };
        let end2 = if s2.t() < e2.t() { e2 } else { s2 };
        *e_out = if between(s1.t(), end2.t(), e1.t()) {
            end2
        } else if between(s2.t(), end1.t(), e2.t()) {
            end1
        } else {
            PtT::NULL
        };
        if *s_out == *e_out {
            return false;
        }
        s_out.is_some() && e_out.is_some()
    }

    pub fn pt_already_seen(self, mut check: PtT) -> bool {
        while self != check {
            if self.d().pt == check.d().pt {
                return true;
            }
            check = check.d().next;
        }
        false
    }

    pub fn prev(self) -> PtT {
        let mut result = self;
        let mut next = self;
        loop {
            next = next.d().next;
            if next == self {
                break;
            }
            result = next;
        }
        result
    }

    #[inline]
    pub fn segment(self) -> Segment {
        self.span().segment()
    }

    pub fn set_coincident(self) {
        self.d().coincident = true;
    }

    pub fn set_deleted(self) {
        self.d().deleted = true;
    }

    pub fn set_span(self, span: Span) {
        self.d().span = span;
    }

    #[inline]
    pub fn span(self) -> Span {
        self.d().span
    }

    pub fn starter(self, end: PtT) -> PtT {
        if self.t() < end.t() { self } else { end }
    }
}

impl Span {
    // SkOpSpanBase

    pub fn add_opp(self, opp: Span) -> bool {
        let opp_prev = self.pt_t().opp_prev(opp.pt_t());
        if opp_prev.is_null() {
            return true;
        }
        if !self.merge_matches(opp) {
            return false;
        }
        self.pt_t().add_opp(opp.pt_t(), opp_prev);
        self.check_for_collapsed_coincidence();
        true
    }

    pub fn bump_span_adds(self) {
        self.d().span_adds += 1;
    }

    #[inline]
    pub fn chased(self) -> bool {
        self.d().chased
    }

    pub fn coin_end(self) -> Span {
        self.d().coin_end
    }

    pub fn collapsed(self, s: f64, e: f64) -> Collapsed {
        let start = self.pt_t();
        let mut start_next = PtT::NULL;
        let mut walk = start;
        let mut min = walk.t();
        let mut max = min;
        let segment = self.segment();
        let mut safety_net = 100000;
        loop {
            walk = walk.next();
            if walk == start {
                break;
            }
            safety_net -= 1;
            if safety_net == 0 {
                return Collapsed::Error;
            }
            if walk == start_next {
                return Collapsed::Error;
            }
            if walk.segment() != segment {
                continue;
            }
            min = smin(min, walk.t());
            max = smax(max, walk.t());
            if between(min, s, max) && between(min, e, max) {
                return Collapsed::Yes;
            }
            start_next = start.next();
        }
        Collapsed::No
    }

    /// `contains(const SkOpSpanBase*)`.
    pub fn contains_span(self, span: Span) -> bool {
        let start = self.pt_t();
        let check = span.pt_t();
        let mut walk = start;
        loop {
            walk = walk.next();
            if walk == start {
                return false;
            }
            if walk == check {
                return true;
            }
        }
    }

    /// `contains(const SkOpSegment*)`.
    pub fn contains_seg(self, segment: Segment) -> PtT {
        let start = self.pt_t();
        let mut walk = start;
        loop {
            walk = walk.next();
            if walk == start {
                return PtT::NULL;
            }
            if walk.deleted() {
                continue;
            }
            if walk.segment() == segment && walk.span().pt_t() == walk {
                return walk;
            }
        }
    }

    pub fn contains_coin_end(self, coin: Span) -> bool {
        let mut next = self;
        loop {
            next = next.d().coin_end;
            if next == self {
                return false;
            }
            if next == coin {
                return true;
            }
        }
    }

    pub fn contains_coin_end_seg(self, segment: Segment) -> bool {
        let mut next = self;
        loop {
            next = next.d().coin_end;
            if next == self {
                return false;
            }
            if next.segment() == segment {
                return true;
            }
        }
    }

    pub fn contour(self) -> Contour {
        self.segment().contour()
    }

    #[inline]
    pub fn deleted(self) -> bool {
        self.pt_t().deleted()
    }

    /// `final()`.
    #[inline]
    pub fn is_final(self) -> bool {
        self.d().pt_t.t == 1.0
    }

    #[inline]
    pub fn from_angle(self) -> Angle {
        self.d().from_angle
    }

    pub fn global_state(self) -> &'static mut GlobalState {
        self.contour().global_state()
    }

    pub fn init_base(self, segment: Segment, prev: Span, t: f64, pt: Point) {
        self.d().segment = segment;
        self.pt_t().init(self, t, pt, false);
        let d = self.d();
        d.coin_end = self;
        d.from_angle = Angle::NULL;
        d.prev = prev;
        d.span_adds = 0;
        d.aligned = true;
        d.chased = false;
    }

    /// Please keep this in sync with debugInsertCoinEnd().
    pub fn insert_coin_end(self, coin: Span) {
        if self.contains_coin_end(coin) {
            return;
        }
        let coin_next = coin.d().coin_end;
        coin.d().coin_end = self.d().coin_end;
        self.d().coin_end = coin_next;
    }

    /// This pair of spans share a common t value or point; merge them and
    /// eliminate duplicates. This does not compute the best t or pt value;
    /// this merely moves all data into a single list.
    pub fn merge(self, span: Span) {
        let span_pt_t = span.pt_t();
        span.release(self.pt_t());
        if self.contains_span(span) {
            return; // merge is already in the ptT loop
        }
        let mut remainder = span_pt_t.next();
        self.pt_t().insert(span_pt_t);
        while remainder != span_pt_t {
            let next = remainder.next();
            let mut compare = span_pt_t.next();
            let mut found = false;
            while compare != span_pt_t {
                let next_c = compare.next();
                if next_c.span() == remainder.span() && next_c.t() == remainder.t() {
                    found = true;
                    break;
                }
                compare = next_c;
            }
            if !found {
                span_pt_t.insert(remainder);
            }
            remainder = next;
        }
        self.d().span_adds += span.d().span_adds;
    }

    /// Please keep in sync with debugCheckForCollapsedCoincidence().
    pub fn check_for_collapsed_coincidence(self) {
        let coins: &mut Coincidence = unsafe { &mut *self.global_state().coincidence };
        if coins.is_empty() {
            return;
        }
        // the insert above may have put both ends of a coincident run in the same span
        // for each coincident ptT in loop; see if its opposite in is also in the loop
        // this implementation is the motivation for marking that a ptT is referenced by a coincident span
        let head = self.pt_t();
        let mut test = head;
        loop {
            if test.coincident() {
                coins.mark_collapsed(test);
            }
            test = test.next();
            if test == head {
                break;
            }
        }
        coins.release_deleted();
    }

    /// Please keep in sync with debugMergeMatches(). Look to see if the pt-t
    /// linked list contains the same segment more than once; if so, and if
    /// each pt-t is directly pointed to by spans in that segment, merge them.
    pub fn merge_matches(self, opp: Span) -> bool {
        let mut test = self.pt_t();
        let stop = test;
        let mut safety_hatch = 1000000;
        loop {
            safety_hatch -= 1;
            if safety_hatch == 0 {
                return false;
            }
            let test_next = test.next();
            'body: {
                if test.deleted() {
                    break 'body;
                }
                let test_base = test.span();
                let segment = test.segment();
                if segment.done() {
                    break 'body;
                }
                let mut inner = opp.pt_t();
                let inner_stop = inner;
                'inner_loop: loop {
                    'inner_body: {
                        if inner.segment() != segment {
                            break 'inner_body;
                        }
                        if inner.deleted() {
                            break 'inner_body;
                        }
                        let inner_base = inner.span();
                        // when the intersection is first detected, the span base is marked if there are
                        // more than one point in the intersection.
                        if !zero_or_one(inner.t()) {
                            inner_base.release(test);
                        } else if !zero_or_one(test.t()) {
                            test_base.release(inner);
                        } else {
                            segment.mark_all_done(); // mark segment as collapsed
                            test.set_deleted();
                            inner.set_deleted();
                        }
                        break 'inner_loop;
                    }
                    inner = inner.next();
                    if inner == inner_stop {
                        break;
                    }
                }
            }
            test = test_next;
            if test == stop {
                break;
            }
        }
        self.check_for_collapsed_coincidence();
        true
    }

    #[inline]
    pub fn prev(self) -> Span {
        self.d().prev
    }

    #[inline]
    pub fn pt(self) -> Point {
        self.d().pt_t.pt
    }

    #[inline]
    pub fn segment(self) -> Segment {
        self.d().segment
    }

    pub fn set_aligned(self) {
        self.d().aligned = true;
    }

    pub fn set_chased(self, chased: bool) {
        self.d().chased = chased;
    }

    pub fn set_from_angle(self, angle: Angle) {
        self.d().from_angle = angle;
    }

    pub fn set_prev(self, prev: Span) {
        self.d().prev = prev;
    }

    pub fn simple(self) -> bool {
        self.pt_t().next().next() == self.pt_t()
    }

    pub fn span_adds_count(self) -> i32 {
        self.d().span_adds
    }

    /// `starter(const SkOpSpanBase* end)`.
    #[inline]
    pub fn starter(self, end: Span) -> Span {
        if self.t() < end.t() { self } else { end }
    }

    /// `starter(SkOpSpanBase** endPtr)`.
    pub fn starter_ptr(self, end_ptr: &mut Span) -> Span {
        let end = *end_ptr;
        if self.t() < end.t() {
            self
        } else {
            *end_ptr = self;
            end
        }
    }

    pub fn step(self, end: Span) -> i32 {
        if self.t() < end.t() { 1 } else { -1 }
    }

    #[inline]
    pub fn t(self) -> f64 {
        self.d().pt_t.t
    }

    pub fn unaligned(self) {
        self.d().aligned = false;
    }

    /// `upCast()`: the identity (one struct for both).
    #[inline]
    pub fn up_cast(self) -> Span {
        self
    }

    /// `upCastable()`: null for the final span.
    #[inline]
    pub fn up_castable(self) -> Span {
        if self.is_final() { Span::NULL } else { self }
    }

    // SkOpSpan

    pub fn already_added(self) -> bool {
        self.d().already_added
    }

    pub fn clear_coincident(self) -> bool {
        if self.d().coincident == self {
            return false;
        }
        self.d().coincident = self;
        true
    }

    pub fn compute_wind_sum(self) -> i32 {
        let contour_head = self.global_state().contour_head;
        let mut wind_try = 0;
        loop {
            if self.sortable_top(contour_head) {
                break;
            }
            wind_try += 1;
            if wind_try >= MAX_WINDING_TRIES {
                break;
            }
        }
        self.wind_sum()
    }

    pub fn contains_coincidence_seg(self, segment: Segment) -> bool {
        let mut next = self.d().coincident;
        loop {
            if next.segment() == segment {
                return true;
            }
            next = next.d().coincident;
            if next == self {
                return false;
            }
        }
    }

    pub fn contains_coincidence(self, coin: Span) -> bool {
        let mut next = self;
        loop {
            next = next.d().coincident;
            if next == self {
                return false;
            }
            if next == coin {
                return true;
            }
        }
    }

    #[inline]
    pub fn done(self) -> bool {
        self.d().done
    }

    pub fn init(self, segment: Segment, prev: Span, t: f64, pt: Point) {
        self.init_base(segment, prev, t, pt);
        let d = self.d();
        d.coincident = self;
        d.to_angle = Angle::NULL;
        d.wind_sum = SK_MIN_S32;
        d.opp_sum = SK_MIN_S32;
        d.wind_value = 1;
        d.opp_value = 0;
        d.top_t_try = 0;
        d.chased = false;
        d.done = false;
        segment.bump_count();
        self.d().already_added = false;
    }

    /// Please keep this in sync with debugInsertCoincidence().
    pub fn insert_coincidence_seg(self, segment: Segment, flipped: bool, ordered: bool) -> bool {
        if self.contains_coincidence_seg(segment) {
            return true;
        }
        let mut next = self.pt_t();
        loop {
            next = next.next();
            if next == self.pt_t() {
                break;
            }
            if next.segment() == segment {
                let span;
                let base = next.span();
                if !ordered {
                    let span_end_pt_t = self.d().next.contains_seg(segment);
                    if span_end_pt_t.is_null() {
                        return false;
                    }
                    let span_end = span_end_pt_t.span();
                    let start = base.pt_t().starter(span_end.pt_t());
                    if start.span().up_castable().is_null() {
                        return false;
                    }
                    span = start.span();
                } else if flipped {
                    span = base.prev();
                    if span.is_null() {
                        return false;
                    }
                } else {
                    if base.up_castable().is_null() {
                        return false;
                    }
                    span = base;
                }
                self.insert_coincidence(span);
                return true;
            }
        }
        true
    }

    /// Please keep this in sync with debugInsertCoincidence().
    pub fn insert_coincidence(self, coin: Span) {
        if self.contains_coincidence(coin) {
            return;
        }
        let coin_next = coin.d().coincident;
        coin.d().coincident = self.d().coincident;
        self.d().coincident = coin_next;
    }

    pub fn is_canceled(self) -> bool {
        self.d().wind_value == 0 && self.d().opp_value == 0
    }

    pub fn is_coincident(self) -> bool {
        self.d().coincident != self
    }

    pub fn mark_added(self) {
        self.d().already_added = true;
    }

    #[inline]
    pub fn next(self) -> Span {
        self.d().next
    }

    #[inline]
    pub fn opp_sum(self) -> i32 {
        self.d().opp_sum
    }

    #[inline]
    pub fn opp_value(self) -> i32 {
        self.d().opp_value
    }

    pub fn release(self, kept: PtT) {
        let prev = self.prev();
        let next = self.next();
        prev.set_next(next);
        next.set_prev(prev);
        self.segment().release(self);
        let coincidence = self.global_state().coincidence;
        if !coincidence.is_null() {
            unsafe { (*coincidence).fix_up(self.pt_t(), kept) };
        }
        self.pt_t().set_deleted();
        let stop_pt_t = self.pt_t();
        let mut test_pt_t = stop_pt_t;
        let kept_span = kept.span();
        loop {
            if self == test_pt_t.span() {
                test_pt_t.set_span(kept_span);
            }
            test_pt_t = test_pt_t.next();
            if test_pt_t == stop_pt_t {
                break;
            }
        }
    }

    pub fn set_done(self, done: bool) {
        self.d().done = done;
    }

    pub fn set_next(self, next_t: Span) {
        self.d().next = next_t;
    }

    pub fn set_opp_sum(self, opp_sum: i32) {
        if self.d().opp_sum != SK_MIN_S32 && self.d().opp_sum != opp_sum {
            self.global_state().winding_failed = true;
            return;
        }
        self.d().opp_sum = opp_sum;
    }

    pub fn set_opp_value(self, opp_value: i32) {
        self.d().opp_value = opp_value;
    }

    pub fn set_to_angle(self, angle: Angle) {
        self.d().to_angle = angle;
    }

    pub fn set_wind_sum(self, wind_sum: i32) {
        if self.d().wind_sum != SK_MIN_S32 && self.d().wind_sum != wind_sum {
            self.global_state().winding_failed = true;
            return;
        }
        self.d().wind_sum = wind_sum;
    }

    pub fn set_wind_value(self, wind_value: i32) {
        self.d().wind_value = wind_value;
    }

    #[inline]
    pub fn to_angle(self) -> Angle {
        self.d().to_angle
    }

    #[inline]
    pub fn wind_sum(self) -> i32 {
        self.d().wind_sum
    }

    #[inline]
    pub fn wind_value(self) -> i32 {
        self.d().wind_value
    }
}
