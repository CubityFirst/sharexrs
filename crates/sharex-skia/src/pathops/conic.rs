//! `SkPathOpsConic.h`, `SkPathOpsConic.cpp`: `SkDConic`.

use super::cubic::DCubic;
use super::point::{DPoint, DVector};
use super::quad::DQuad;
use super::types::*;
use crate::geom::Point;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DConic {
    pub pts: DQuad,
    pub weight: f32,
}

impl std::ops::Index<usize> for DConic {
    type Output = DPoint;
    fn index(&self, n: usize) -> &DPoint {
        &self.pts.pts[n]
    }
}

impl std::ops::IndexMut<usize> for DConic {
    fn index_mut(&mut self, n: usize) -> &mut DPoint {
        &mut self.pts.pts[n]
    }
}

// cribbed from the float version in SkGeometry.cpp
fn conic_deriv_coeff(src: [f64; 3], w: f32) -> [f64; 3] {
    let p20 = src[2] - src[0];
    let p10 = src[1] - src[0];
    let w_p10 = w as f64 * p10;
    [w as f64 * p20 - p20, p20 - 2.0 * w_p10, w_p10]
}

fn conic_eval_tan(coord: [f64; 3], w: f32, t: f64) -> f64 {
    let coeff = conic_deriv_coeff(coord, w);
    t * (t * coeff[0] + coeff[1]) + coeff[2]
}

fn conic_eval_numerator(src: [f64; 3], w: f32, t: f64) -> f64 {
    let src2w = src[1] * w as f64;
    let c = src[0];
    let a = src[2] - 2.0 * src2w + c;
    let b = 2.0 * (src2w - c);
    (a * t + b) * t + c
}

fn conic_eval_denominator(w: f32, t: f64) -> f64 {
    let b = 2.0 * (w as f64 - 1.0);
    let c = 1.0;
    let a = -b;
    (a * t + b) * t + c
}

pub const CONIC_POINT_COUNT: usize = 3;
pub const CONIC_MAX_INTERSECTIONS: i32 = 4;

impl DConic {
    pub fn from_pts(pts: &[Point], weight: f32) -> Self {
        DConic { pts: DQuad::from_pts(pts), weight }
    }

    pub fn set(&mut self, pts: &[Point], weight: f32) -> &Self {
        *self = DConic::from_pts(pts, weight);
        self
    }

    pub fn collapsed(&self) -> bool {
        self.pts.collapsed()
    }

    pub fn controls_inside(&self) -> bool {
        self.pts.controls_inside()
    }

    pub fn flip(&self) -> DConic {
        DConic { pts: self.pts.flip(), weight: self.weight }
    }

    pub fn align(&self, end_index: usize, dst_pt: &mut DPoint) {
        self.pts.align(end_index, dst_pt);
    }

    #[allow(non_snake_case)]
    pub fn FindExtrema(src: [f64; 3], w: f32, t: &mut [f64]) -> i32 {
        let coeff = conic_deriv_coeff(src, w);
        let mut t_values = [0.0; 2];
        let roots = DQuad::RootsValidT(coeff[0], coeff[1], coeff[2], &mut t_values);
        // In extreme cases, the number of roots returned can be 2. Pathops
        // will fail later on, so there's no advantage to plumbing in an error
        // return here.
        if 1 == roots {
            t[0] = t_values[0];
            return 1;
        }
        0
    }

    pub fn dxdy_at_t(&self, t: f64) -> DVector {
        let mut result = DVector {
            x: conic_eval_tan(self.pts.xs(), self.weight, t),
            y: conic_eval_tan(self.pts.ys(), self.weight, t),
        };
        if result.x == 0.0 && result.y == 0.0 && zero_or_one(t) {
            result = self[2] - self[0];
        }
        result
    }

    pub fn hull_intersects_quad(&self, quad: &DQuad, is_linear: &mut bool) -> bool {
        self.pts.hull_intersects_quad(quad, is_linear)
    }

    pub fn hull_intersects_conic(&self, conic: &DConic, is_linear: &mut bool) -> bool {
        self.pts.hull_intersects_quad(&conic.pts, is_linear)
    }

    pub fn hull_intersects_cubic(&self, cubic: &DCubic, is_linear: &mut bool) -> bool {
        cubic.hull_intersects_conic(self, is_linear)
    }

    pub fn is_linear(&self, start_index: usize, end_index: usize) -> bool {
        self.pts.is_linear(start_index, end_index)
    }

    pub fn monotonic_in_x(&self) -> bool {
        self.pts.monotonic_in_x()
    }

    pub fn monotonic_in_y(&self) -> bool {
        self.pts.monotonic_in_y()
    }

    pub fn other_pts(&self, odd_man: usize) -> [DPoint; 2] {
        self.pts.other_pts(odd_man)
    }

    pub fn pt_at_t(&self, t: f64) -> DPoint {
        if t == 0.0 {
            return self[0];
        }
        if t == 1.0 {
            return self[2];
        }
        let denominator = conic_eval_denominator(self.weight, t);
        DPoint {
            x: conic_eval_numerator(self.pts.xs(), self.weight, t) / denominator,
            y: conic_eval_numerator(self.pts.ys(), self.weight, t) / denominator,
        }
    }

    /// See the quad subdivide for the point rationale; the weight is the
    /// ratio of the distance from the mid of the end points to the on-curve
    /// point and the distance of the on-curve point to the control point.
    pub fn sub_divide(&self, t1: f64, t2: f64) -> DConic {
        let (ax, ay, az);
        let xs = self.pts.xs();
        let ys = self.pts.ys();
        if t1 == 0.0 {
            ax = self[0].x;
            ay = self[0].y;
            az = 1.0;
        } else if t1 != 1.0 {
            ax = conic_eval_numerator(xs, self.weight, t1);
            ay = conic_eval_numerator(ys, self.weight, t1);
            az = conic_eval_denominator(self.weight, t1);
        } else {
            ax = self[2].x;
            ay = self[2].y;
            az = 1.0;
        }
        let mid_t = (t1 + t2) / 2.0;
        let dx = conic_eval_numerator(xs, self.weight, mid_t);
        let dy = conic_eval_numerator(ys, self.weight, mid_t);
        let dz = conic_eval_denominator(self.weight, mid_t);
        let (cx, cy, cz);
        if t2 == 1.0 {
            cx = self[2].x;
            cy = self[2].y;
            cz = 1.0;
        } else if t2 != 0.0 {
            cx = conic_eval_numerator(xs, self.weight, t2);
            cy = conic_eval_numerator(ys, self.weight, t2);
            cz = conic_eval_denominator(self.weight, t2);
        } else {
            cx = self[0].x;
            cy = self[0].y;
            cz = 1.0;
        }
        let bx = 2.0 * dx - (ax + cx) / 2.0;
        let by = 2.0 * dy - (ay + cy) / 2.0;
        let mut bz = 2.0 * dz - (az + cz) / 2.0;
        if bz == 0.0 {
            bz = 1.0; // if bz is 0, weight is 0, control point has no effect: any value will do
        }
        DConic {
            pts: DQuad {
                pts: [
                    DPoint::new(ax / az, ay / az),
                    DPoint::new(bx / bz, by / bz),
                    DPoint::new(cx / cz, cy / cz),
                ],
            },
            weight: (bz / (az * cz).sqrt()) as f32,
        }
    }

    #[allow(non_snake_case)]
    pub fn SubDivide(a: &[Point], weight: f32, t1: f64, t2: f64) -> DConic {
        DConic::from_pts(a, weight).sub_divide(t1, t2)
    }

    pub fn sub_divide_pt(&self, _a: &DPoint, _c: &DPoint, t1: f64, t2: f64, weight: &mut f32) -> DPoint {
        let chopped = self.sub_divide(t1, t2);
        *weight = chopped.weight;
        chopped[1]
    }

    #[allow(non_snake_case)]
    pub fn SubDividePt(
        pts: &[Point],
        weight: f32,
        a: &DPoint,
        c: &DPoint,
        t1: f64,
        t2: f64,
        new_weight: &mut f32,
    ) -> DPoint {
        DConic::from_pts(pts, weight).sub_divide_pt(a, c, t1, t2, new_weight)
    }
}
