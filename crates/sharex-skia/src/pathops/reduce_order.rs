//! `SkReduceOrder.h`, `SkReduceOrder.cpp`: degenerate quads and cubics to
//! lines (or quads). `SkReduceOrder` is a union of a line, a quad and a
//! cubic over the same points, kept here as one array.

use super::cubic::DCubic;
use super::curve::points_to_verb;
use super::point::DPoint;
use super::quad::DQuad;
use super::types::*;
use crate::geom::{Conic, Point};
use crate::path::Verb;

#[derive(Default)]
pub struct ReduceOrder {
    pub pts: [DPoint; 4],
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Quadratics {
    No,
    Allow,
}

fn reduction_line_count(reduction: &[DPoint; 4]) -> i32 {
    1 + !reduction[0].approximately_equal(&reduction[1]) as i32
}

impl ReduceOrder {
    pub fn reduce_quad(&mut self, quad: &DQuad) -> i32 {
        let (mut min_x, mut max_x, mut min_y, mut max_y) = (0usize, 0usize, 0usize, 0usize);
        let mut min_x_set = 0;
        let mut min_y_set = 0;
        for index in 1..3 {
            if quad[min_x].x > quad[index].x {
                min_x = index;
            }
            if quad[min_y].y > quad[index].y {
                min_y = index;
            }
            if quad[max_x].x < quad[index].x {
                max_x = index;
            }
            if quad[max_y].y < quad[index].y {
                max_y = index;
            }
        }
        let _ = (max_x, max_y);
        for index in 0..3 {
            if almost_equal_ulps_d(quad[index].x, quad[min_x].x) {
                min_x_set |= 1 << index;
            }
            if almost_equal_ulps_d(quad[index].y, quad[min_y].y) {
                min_y_set |= 1 << index;
            }
        }
        if (min_x_set & 0x05) == 0x5 && (min_y_set & 0x05) == 0x5 {
            // test for degenerate: this quad starts and ends at the same place,
            // so never contributes to the fill
            self.pts[0] = quad[0];
            self.pts[1] = quad[0];
            return 1;
        }
        if min_x_set == 0x7 || min_y_set == 0x7 {
            // vertical or horizontal line
            self.pts[0] = quad[0];
            self.pts[1] = quad[2];
            return reduction_line_count(&self.pts);
        }
        // check_linear
        if quad.is_linear(0, 2) {
            // four are colinear: return line formed by outside
            self.pts[0] = quad[0];
            self.pts[1] = quad[2];
            let result = reduction_line_count(&self.pts);
            if result != 0 {
                return result;
            }
        }
        self.pts[..3].copy_from_slice(&quad.pts);
        3
    }

    pub fn reduce_cubic(&mut self, cubic: &DCubic, allow_quadratics: Quadratics) -> i32 {
        let (mut min_x, mut max_x, mut min_y, mut max_y) = (0usize, 0usize, 0usize, 0usize);
        let mut min_x_set = 0;
        let mut min_y_set = 0;
        for index in 1..4 {
            if cubic[min_x].x > cubic[index].x {
                min_x = index;
            }
            if cubic[min_y].y > cubic[index].y {
                min_y = index;
            }
            if cubic[max_x].x < cubic[index].x {
                max_x = index;
            }
            if cubic[max_y].y < cubic[index].y {
                max_y = index;
            }
        }
        let _ = (max_x, max_y);
        for index in 0..4 {
            let cx = cubic[index].x;
            let cy = cubic[index].y;
            let denom = smax(cx.abs(), smax(cy.abs(), smax(cubic[min_x].x.abs(), cubic[min_y].y.abs())));
            if denom == 0.0 {
                min_x_set |= 1 << index;
                min_y_set |= 1 << index;
                continue;
            }
            let inv = 1.0 / denom;
            if approximately_equal_half(cx * inv, cubic[min_x].x * inv) {
                min_x_set |= 1 << index;
            }
            if approximately_equal_half(cy * inv, cubic[min_y].y * inv) {
                min_y_set |= 1 << index;
            }
        }
        if min_x_set == 0xF {
            // test for vertical line
            if min_y_set == 0xF {
                // return 1 if all four are coincident
                self.pts[0] = cubic[0];
                self.pts[1] = cubic[0];
                return 1;
            }
            self.pts[0] = cubic[0];
            self.pts[1] = cubic[3];
            return reduction_line_count(&self.pts);
        }
        if min_y_set == 0xF {
            // test for horizontal line
            self.pts[0] = cubic[0];
            self.pts[1] = cubic[3];
            return reduction_line_count(&self.pts);
        }
        // check_linear
        if cubic.is_linear(0, 3) {
            self.pts[0] = cubic[0];
            self.pts[1] = cubic[3];
            let result = reduction_line_count(&self.pts);
            if result != 0 {
                return result;
            }
        }
        if allow_quadratics == Quadratics::Allow {
            let result = self.check_quadratic(cubic);
            if result != 0 {
                return result;
            }
        }
        self.pts = cubic.pts;
        4
    }

    /// Check to see if it is a quadratic or a line.
    fn check_quadratic(&mut self, cubic: &DCubic) -> i32 {
        let dx10 = cubic[1].x - cubic[0].x;
        let dx23 = cubic[2].x - cubic[3].x;
        let mid_x = cubic[0].x + dx10 * 3.0 / 2.0;
        let side_ax = mid_x - cubic[3].x;
        let side_bx = dx23 * 3.0 / 2.0;
        if if approximately_zero(side_ax) {
            !approximately_equal(side_ax, side_bx)
        } else {
            !almost_equal_ulps_pin_d(side_ax, side_bx)
        } {
            return 0;
        }
        let dy10 = cubic[1].y - cubic[0].y;
        let dy23 = cubic[2].y - cubic[3].y;
        let mid_y = cubic[0].y + dy10 * 3.0 / 2.0;
        let side_ay = mid_y - cubic[3].y;
        let side_by = dy23 * 3.0 / 2.0;
        if if approximately_zero(side_ay) {
            !approximately_equal(side_ay, side_by)
        } else {
            !almost_equal_ulps_pin_d(side_ay, side_by)
        } {
            return 0;
        }
        self.pts[0] = cubic[0];
        self.pts[1].x = mid_x;
        self.pts[1].y = mid_y;
        self.pts[2] = cubic[3];
        3
    }

    /// `SkReduceOrder::Quad`: the points of the line, if the quad became one.
    #[allow(non_snake_case)]
    pub fn Quad(a: &[Point], reduce_pts: &mut [Point]) -> Verb {
        let quad = DQuad::from_pts(a);
        let mut reducer = ReduceOrder::default();
        let order = reducer.reduce_quad(&quad);
        if order == 2 {
            // quad became line
            for index in 0..order as usize {
                reduce_pts[index] = reducer.pts[index].as_sk_point();
            }
        }
        points_to_verb((order - 1) as usize)
    }

    #[allow(non_snake_case)]
    pub fn Conic(c: &Conic, reduce_pts: &mut [Point]) -> Verb {
        let verb = ReduceOrder::Quad(&c.pts, reduce_pts);
        if verb_gt_line(verb) && c.w == 1.0 {
            return Verb::Quad;
        }
        if verb == Verb::Quad { Verb::Conic } else { verb }
    }

    #[allow(non_snake_case)]
    pub fn Cubic(a: &[Point], reduce_pts: &mut [Point]) -> Verb {
        if DPoint::approximately_equal_sk(a[0], a[1])
            && DPoint::approximately_equal_sk(a[0], a[2])
            && DPoint::approximately_equal_sk(a[0], a[3])
        {
            reduce_pts[0] = a[0];
            return Verb::Move;
        }
        let cubic = DCubic::from_pts(a);
        let mut reducer = ReduceOrder::default();
        let order = reducer.reduce_cubic(&cubic, Quadratics::Allow);
        if order == 2 || order == 3 {
            // cubic became line or quad
            for index in 0..order as usize {
                reduce_pts[index] = reducer.pts[index].as_sk_point();
            }
        }
        points_to_verb((order - 1) as usize)
    }
}

/// `verb > SkPath::kLine_Verb` in Skia's verb numbering.
fn verb_gt_line(v: Verb) -> bool {
    matches!(v, Verb::Quad | Verb::Conic | Verb::Cubic | Verb::Close)
}
