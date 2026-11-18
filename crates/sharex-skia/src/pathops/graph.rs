//! The path ops' object graph: `SkOpPtT`, `SkOpSpanBase`/`SkOpSpan`,
//! `SkOpSegment`, `SkOpContour`, `SkOpAngle` and `SkCoincidentSpans`. Skia
//! links them with raw pointers (pt-t rings, span lists, angle rings,
//! coincidence lists) into objects that live in the operation's arena and
//! embeds some in others (a span's pt-t, a segment's head and tail spans, a
//! contour's first segment). The port keeps that layout: each type is a
//! `*Data` struct, reached through a `Copy` handle wrapping the raw pointer,
//! with the methods on the handle, so the code reads as Skia's.
//!
//! `SkOpSpanBase` and `SkOpSpan` are one struct here: a segment's tail is a
//! span base whose span fields are never read (`upCast` is the identity).

use super::curve::{DCurve, DCurveSweep};
use super::line_parameters::LineParameters;
use super::rect::PathOpsBounds;
use super::types::GlobalState;
use crate::geom::Point;
use crate::path::Verb;

macro_rules! handle {
    ($name:ident, $data:ty) => {
        #[derive(Clone, Copy, PartialEq, Eq, Debug)]
        pub struct $name(pub *mut $data);

        impl $name {
            pub const NULL: $name = $name(std::ptr::null_mut());

            #[inline]
            pub fn is_null(self) -> bool {
                self.0.is_null()
            }

            #[inline]
            pub fn is_some(self) -> bool {
                !self.0.is_null()
            }

            /// The object behind the handle. Only ever used for one field
            /// access at a time: the graph aliases freely.
            #[inline]
            #[allow(clippy::mut_from_ref)]
            pub(crate) fn d(self) -> &'static mut $data {
                debug_assert!(!self.0.is_null());
                unsafe { &mut *self.0 }
            }
        }
    };
}

handle!(PtT, PtTData);
handle!(Span, SpanData);
handle!(Segment, SegmentData);
handle!(Contour, ContourData);
handle!(Angle, AngleData);
handle!(CoinSpans, CoinSpansData);

/// `SkOpPtT`.
pub struct PtTData {
    pub t: f64,
    pub pt: Point,
    pub span: Span,
    pub next: PtT,
    pub deleted: bool,
    pub duplicate_pt: bool,
    pub coincident: bool,
}

impl Default for PtTData {
    fn default() -> Self {
        PtTData {
            t: 0.0,
            pt: Point::default(),
            span: Span::NULL,
            next: PtT::NULL,
            deleted: false,
            duplicate_pt: false,
            coincident: false,
        }
    }
}

/// `SkOpSpanBase` + `SkOpSpan`.
pub struct SpanData {
    // SkOpSpanBase
    pub pt_t: PtTData,
    pub segment: Segment,
    pub coin_end: Span,
    pub from_angle: Angle,
    pub prev: Span,
    pub span_adds: i32,
    pub aligned: bool,
    pub chased: bool,
    // SkOpSpan
    pub coincident: Span,
    pub to_angle: Angle,
    pub next: Span,
    pub wind_sum: i32,
    pub opp_sum: i32,
    pub wind_value: i32,
    pub opp_value: i32,
    pub top_t_try: i32,
    pub done: bool,
    pub already_added: bool,
}

impl Default for SpanData {
    fn default() -> Self {
        SpanData {
            pt_t: PtTData::default(),
            segment: Segment::NULL,
            coin_end: Span::NULL,
            from_angle: Angle::NULL,
            prev: Span::NULL,
            span_adds: 0,
            aligned: false,
            chased: false,
            coincident: Span::NULL,
            to_angle: Angle::NULL,
            next: Span::NULL,
            wind_sum: 0,
            opp_sum: 0,
            wind_value: 0,
            opp_value: 0,
            top_t_try: 0,
            done: false,
            already_added: false,
        }
    }
}

/// `SkOpSegment`.
pub struct SegmentData {
    pub head: SpanData,
    pub tail: SpanData,
    pub contour: Contour,
    pub next: Segment,
    pub prev: Segment,
    pub pts: [Point; 4],
    pub bounds: PathOpsBounds,
    pub weight: f32,
    pub count: i32,
    pub done_count: i32,
    pub verb: Verb,
    pub visited: bool,
}

impl Default for SegmentData {
    fn default() -> Self {
        SegmentData {
            head: SpanData::default(),
            tail: SpanData::default(),
            contour: Contour::NULL,
            next: Segment::NULL,
            prev: Segment::NULL,
            pts: [Point::default(); 4],
            bounds: PathOpsBounds::default(),
            weight: 0.0,
            count: 0,
            done_count: 0,
            verb: Verb::Move,
            visited: false,
        }
    }
}

/// `SkOpContour` (and `SkOpContourHead`).
pub struct ContourData {
    pub state: *mut GlobalState,
    pub head: SegmentData,
    pub tail: Segment,
    pub next: Contour,
    pub bounds: PathOpsBounds,
    pub ccw: i32,
    pub count: i32,
    pub first_sorted: i32,
    pub done: bool,
    pub operand: bool,
    pub reverse: bool,
    pub xor: bool,
    pub opp_xor: bool,
}

impl Default for ContourData {
    fn default() -> Self {
        // SkOpContour() { reset(); }
        ContourData {
            state: std::ptr::null_mut(),
            head: SegmentData::default(),
            tail: Segment::NULL,
            next: Contour::NULL,
            bounds: PathOpsBounds::default(),
            ccw: 0,
            count: 0,
            first_sorted: 0,
            done: false,
            operand: false,
            reverse: false,
            xor: false,
            opp_xor: false,
        }
    }
}

/// `SkOpAngle`.
pub struct AngleData {
    pub original_curve_part: DCurve,
    pub part: DCurveSweep,
    pub side: f64,
    pub tangent_half: LineParameters,
    pub next: Angle,
    pub last_marked: Span,
    pub start: Span,
    pub end: Span,
    pub computed_end: Span,
    pub sector_mask: i32,
    pub sector_start: i8,
    pub sector_end: i8,
    pub unorderable: bool,
    pub compute_sector: bool,
    pub computed_sector: bool,
    pub check_coincidence: bool,
    pub tangents_ambiguous: bool,
}

impl Default for AngleData {
    fn default() -> Self {
        AngleData {
            original_curve_part: DCurve::default(),
            part: DCurveSweep::default(),
            side: 0.0,
            tangent_half: LineParameters::default(),
            next: Angle::NULL,
            last_marked: Span::NULL,
            start: Span::NULL,
            end: Span::NULL,
            computed_end: Span::NULL,
            sector_mask: 0,
            sector_start: 0,
            sector_end: 0,
            unorderable: false,
            compute_sector: false,
            computed_sector: false,
            check_coincidence: false,
            tangents_ambiguous: false,
        }
    }
}

/// `SkCoincidentSpans`.
pub struct CoinSpansData {
    pub next: CoinSpans,
    pub coin_pt_t_start: PtT,
    pub coin_pt_t_end: PtT,
    pub opp_pt_t_start: PtT,
    pub opp_pt_t_end: PtT,
}

impl Default for CoinSpansData {
    fn default() -> Self {
        CoinSpansData {
            next: CoinSpans::NULL,
            coin_pt_t_start: PtT::NULL,
            coin_pt_t_end: PtT::NULL,
            opp_pt_t_start: PtT::NULL,
            opp_pt_t_end: PtT::NULL,
        }
    }
}

impl Span {
    /// `ptT()`: the span's embedded pt-t.
    #[inline]
    pub fn pt_t(self) -> PtT {
        PtT(unsafe { std::ptr::addr_of_mut!((*self.0).pt_t) })
    }
}

impl Segment {
    /// `head()`.
    #[inline]
    pub fn head(self) -> Span {
        Span(unsafe { std::ptr::addr_of_mut!((*self.0).head) })
    }

    /// `tail()`.
    #[inline]
    pub fn tail(self) -> Span {
        Span(unsafe { std::ptr::addr_of_mut!((*self.0).tail) })
    }
}

impl Contour {
    /// `first()` / `&fHead`.
    #[inline]
    pub fn first(self) -> Segment {
        Segment(unsafe { std::ptr::addr_of_mut!((*self.0).head) })
    }

    #[inline]
    pub fn global_state(self) -> &'static mut GlobalState {
        unsafe { &mut *self.d().state }
    }
}
