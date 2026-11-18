//! `SkAddIntersections.cpp` (`AddIntersectTs`) and `SkIntersectionHelper.h`:
//! intersect every pair of segments of two contours and record the
//! intersections as spans.

use super::coincidence::Coincidence;
use super::conic::DConic;
use super::cubic::DCubic;
use super::graph::*;
use super::intersections::Intersections;
use super::line::DLine;
use super::quad::DQuad;
use super::rect::PathOpsBounds;
use super::types::*;
use crate::path::Verb;

/// `SkIntersectionHelper::SegmentType`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum SegmentType {
    HorizontalLine,
    VerticalLine,
    Line,
    Quad,
    Conic,
    Cubic,
}

/// `SkIntersectionHelper`.
struct IntersectionHelper {
    segment: Segment,
}

impl IntersectionHelper {
    fn advance(&mut self) -> bool {
        self.segment = self.segment.next();
        self.segment.is_some()
    }

    fn bounds(&self) -> PathOpsBounds {
        self.segment.bounds()
    }

    fn bottom(&self) -> f32 {
        self.bounds().bottom
    }

    fn left(&self) -> f32 {
        self.bounds().left
    }

    fn right(&self) -> f32 {
        self.bounds().right
    }

    fn top(&self) -> f32 {
        self.bounds().top
    }

    fn x(&self) -> f32 {
        self.bounds().left
    }

    fn y(&self) -> f32 {
        self.bounds().top
    }

    fn pts(&self) -> &'static [crate::geom::Point; 4] {
        self.segment.pts()
    }

    fn weight(&self) -> f32 {
        self.segment.weight()
    }

    fn segment_type(&self) -> SegmentType {
        match self.segment.verb() {
            Verb::Quad => SegmentType::Quad,
            Verb::Conic => SegmentType::Conic,
            Verb::Cubic => SegmentType::Cubic,
            _ => {
                if self.segment.is_horizontal() {
                    SegmentType::HorizontalLine
                } else if self.segment.is_vertical() {
                    SegmentType::VerticalLine
                } else {
                    SegmentType::Line
                }
            }
        }
    }

    fn start_after(&mut self, after: &IntersectionHelper) -> bool {
        self.segment = after.segment.next();
        self.segment.is_some()
    }

    fn x_flipped(&self) -> bool {
        self.x() != self.pts()[0].x
    }

    fn y_flipped(&self) -> bool {
        self.y() != self.pts()[0].y
    }
}

fn line_of(h: &IntersectionHelper) -> DLine {
    DLine::from_pts(h.pts())
}

fn quad_of(h: &IntersectionHelper) -> DQuad {
    DQuad::from_pts(h.pts())
}

fn conic_of(h: &IntersectionHelper) -> DConic {
    DConic::from_pts(h.pts(), h.weight())
}

fn cubic_of(h: &IntersectionHelper) -> DCubic {
    DCubic::from_pts(h.pts())
}

/// `AddIntersectTs`.
pub fn add_intersect_ts(test: Contour, next: Contour, coincidence: &mut Coincidence) -> bool {
    if test != next {
        if almost_less_ulps(test.bounds().bottom, next.bounds().top) {
            return false;
        }
        // OPTIMIZATION: outset contour bounds a smidgen instead?
        if !PathOpsBounds::Intersects(&test.bounds(), &next.bounds()) {
            return true;
        }
    }
    let mut wt = IntersectionHelper { segment: test.first() };
    loop {
        let mut wn = IntersectionHelper { segment: next.first() };
        if !(test == next && !wn.start_after(&wt)) {
            loop {
                if PathOpsBounds::Intersects(&wt.bounds(), &wn.bounds()) {
                    intersect_pair(&wt, &wn, coincidence);
                }
                if !wn.advance() {
                    break;
                }
            }
        }
        if !wt.advance() {
            break;
        }
    }
    true
}

fn intersect_pair(wt: &IntersectionHelper, wn: &IntersectionHelper, coincidence: &mut Coincidence) {
    use SegmentType::*;
    let mut ts = Intersections::default();
    let mut swap = false;
    let pts: i32 = match wt.segment_type() {
        HorizontalLine => {
            swap = true;
            let (left, right, y, flipped) = (wt.left() as f64, wt.right() as f64, wt.y() as f64, wt.x_flipped());
            match wn.segment_type() {
                HorizontalLine | VerticalLine | Line => {
                    ts.max = 2;
                    ts.horizontal_line(&line_of(wn), left, right, y, flipped)
                }
                Quad => {
                    ts.max = 2;
                    ts.horizontal_quad(&quad_of(wn), left, right, y, flipped)
                }
                Conic => {
                    ts.max = 2;
                    ts.horizontal_conic(&conic_of(wn), left, right, y, flipped)
                }
                Cubic => {
                    ts.max = 3;
                    ts.horizontal_cubic(&cubic_of(wn), left, right, y, flipped)
                }
            }
        }
        VerticalLine => {
            swap = true;
            let (top, bottom, x, flipped) = (wt.top() as f64, wt.bottom() as f64, wt.x() as f64, wt.y_flipped());
            match wn.segment_type() {
                HorizontalLine | VerticalLine | Line => {
                    ts.max = 2;
                    ts.vertical_line(&line_of(wn), top, bottom, x, flipped)
                }
                Quad => {
                    ts.max = 2;
                    ts.vertical_quad(&quad_of(wn), top, bottom, x, flipped)
                }
                Conic => {
                    ts.max = 2;
                    ts.vertical_conic(&conic_of(wn), top, bottom, x, flipped)
                }
                Cubic => {
                    ts.max = 3;
                    ts.vertical_cubic(&cubic_of(wn), top, bottom, x, flipped)
                }
            }
        }
        Line => match wn.segment_type() {
            HorizontalLine => {
                ts.max = 2;
                ts.horizontal_line(&line_of(wt), wn.left() as f64, wn.right() as f64, wn.y() as f64, wn.x_flipped())
            }
            VerticalLine => {
                ts.max = 2;
                ts.vertical_line(&line_of(wt), wn.top() as f64, wn.bottom() as f64, wn.x() as f64, wn.y_flipped())
            }
            Line => {
                ts.max = 2;
                ts.intersect_line_line(&line_of(wt), &line_of(wn))
            }
            Quad => {
                swap = true;
                ts.intersect_quad_line(&quad_of(wn), &line_of(wt))
            }
            Conic => {
                swap = true;
                ts.max = 3;
                ts.intersect_conic_line(&conic_of(wn), &line_of(wt))
            }
            Cubic => {
                swap = true;
                ts.max = 3;
                ts.intersect_cubic_line(&cubic_of(wn), &line_of(wt))
            }
        },
        Quad => match wn.segment_type() {
            HorizontalLine => {
                ts.max = 2;
                ts.horizontal_quad(&quad_of(wt), wn.left() as f64, wn.right() as f64, wn.y() as f64, wn.x_flipped())
            }
            VerticalLine => {
                ts.max = 2;
                ts.vertical_quad(&quad_of(wt), wn.top() as f64, wn.bottom() as f64, wn.x() as f64, wn.y_flipped())
            }
            Line => ts.intersect_quad_line(&quad_of(wt), &line_of(wn)),
            Quad => ts.intersect_quad_quad(&quad_of(wt), &quad_of(wn)),
            Conic => {
                swap = true;
                ts.intersect_conic_quad(&conic_of(wn), &quad_of(wt))
            }
            Cubic => {
                swap = true;
                ts.intersect_cubic_quad(&cubic_of(wn), &quad_of(wt))
            }
        },
        Conic => match wn.segment_type() {
            HorizontalLine => {
                ts.max = 2;
                ts.horizontal_conic(&conic_of(wt), wn.left() as f64, wn.right() as f64, wn.y() as f64, wn.x_flipped())
            }
            VerticalLine => {
                ts.max = 2;
                ts.vertical_conic(&conic_of(wt), wn.top() as f64, wn.bottom() as f64, wn.x() as f64, wn.y_flipped())
            }
            Line => {
                ts.max = 3;
                ts.intersect_conic_line(&conic_of(wt), &line_of(wn))
            }
            Quad => ts.intersect_conic_quad(&conic_of(wt), &quad_of(wn)),
            Conic => ts.intersect_conic_conic(&conic_of(wt), &conic_of(wn)),
            Cubic => {
                swap = true;
                ts.intersect_cubic_conic(&cubic_of(wn), &conic_of(wt))
            }
        },
        Cubic => match wn.segment_type() {
            HorizontalLine => {
                ts.max = 3;
                ts.horizontal_cubic(&cubic_of(wt), wn.left() as f64, wn.right() as f64, wn.y() as f64, wn.x_flipped())
            }
            VerticalLine => {
                ts.max = 3;
                ts.vertical_cubic(&cubic_of(wt), wn.top() as f64, wn.bottom() as f64, wn.x() as f64, wn.y_flipped())
            }
            Line => {
                ts.max = 3;
                ts.intersect_cubic_line(&cubic_of(wt), &line_of(wn))
            }
            Quad => ts.intersect_cubic_quad(&cubic_of(wt), &quad_of(wn)),
            Conic => ts.intersect_cubic_conic(&cubic_of(wt), &conic_of(wn)),
            Cubic => ts.intersect_cubic_cubic(&cubic_of(wt), &cubic_of(wn)),
        },
    };
    let mut coin_index: i32 = -1;
    let mut coin_pt_t = [PtT::NULL; 2];
    for pt in 0..pts.max(0) as usize {
        // if t value is used to compute pt in addT, error may creep in and
        // rect intersections may result in non-rects. if pt value from intersection
        // is passed in, current tests break. As a workaround, pass in pt
        // value from intersection only if pt.x and pt.y is integral
        let i_pt = ts.pt(pt).as_sk_point();
        let i_pt_is_integral = i_pt.x == i_pt.x.floor() && i_pt.y == i_pt.y.floor();
        let mut test_t_at = if i_pt_is_integral {
            wt.segment.add_t_pt(ts.t(swap as usize, pt), i_pt)
        } else {
            wt.segment.add_t(ts.t(swap as usize, pt))
        };
        let mut next_t_at = if i_pt_is_integral {
            wn.segment.add_t_pt(ts.t((!swap) as usize, pt), i_pt)
        } else {
            wn.segment.add_t(ts.t((!swap) as usize, pt))
        };
        if !test_t_at.contains_pt_t(next_t_at) {
            let opp_prev = test_t_at.opp_prev(next_t_at); //  Returns nullptr if pair
            if opp_prev.is_some() {
                //  already share a pt-t loop.
                test_t_at.span().merge_matches(next_t_at.span());
                test_t_at.add_opp(next_t_at, opp_prev);
            }
            if test_t_at.pt() != next_t_at.pt() {
                test_t_at.span().unaligned();
                next_t_at.span().unaligned();
            }
        }
        if !ts.is_coincident(pt) {
            continue;
        }
        if coin_index < 0 {
            coin_pt_t[0] = test_t_at;
            coin_pt_t[1] = next_t_at;
            coin_index = pt as i32;
            continue;
        }
        if coin_pt_t[0].span() == test_t_at.span() {
            coin_index = -1;
            continue;
        }
        if coin_pt_t[1].span() == next_t_at.span() {
            coin_index = -1; // coincidence span collapsed
            continue;
        }
        if swap {
            coin_pt_t.swap(0, 1);
            std::mem::swap(&mut test_t_at, &mut next_t_at);
        }
        if coin_pt_t[0].span().deleted() {
            coin_index = -1;
            continue;
        }
        if test_t_at.span().deleted() {
            coin_index = -1;
            continue;
        }
        coincidence.add(coin_pt_t[0], test_t_at, coin_pt_t[1], next_t_at);
        coin_index = -1;
    }
}
