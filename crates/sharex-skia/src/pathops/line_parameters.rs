//! `SkLineParameters.h`: a line segment as `ax + by + c = 0`.

use super::cubic::DCubic;
use super::line::DLine;
use super::point::DPoint;
use super::quad::DQuad;
use super::types::*;

#[derive(Clone, Copy, Debug, Default)]
pub struct LineParameters {
    a: f64,
    b: f64,
    c: f64,
}

impl LineParameters {
    pub fn cubic_end_points(&mut self, pts: &DCubic) -> bool {
        let mut end_index = 1;
        self.cubic_end_points_se(pts, 0, end_index);
        if self.dy() != 0.0 {
            return true;
        }
        if self.dx() == 0.0 {
            end_index += 1;
            self.cubic_end_points_se(pts, 0, end_index);
            if self.dy() != 0.0 {
                return true;
            }
            if self.dx() == 0.0 {
                end_index += 1;
                self.cubic_end_points_se(pts, 0, end_index); // line
                return false;
            }
        }
        // FIXME: after switching to round sort, remove bumping fA
        if self.dx() < 0.0 {
            return true;
        }
        // if cubic tangent is on x axis, look at next control point to break tie
        end_index += 1;
        if not_almost_equal_ulps_d(pts[0].y, pts[end_index].y) {
            if pts[0].y > pts[end_index].y {
                self.a = DBL_EPSILON; // push it from 0 to slightly negative (y() returns -a)
            }
            return true;
        }
        if end_index == 3 {
            return true;
        }
        if pts[0].y > pts[3].y {
            self.a = DBL_EPSILON;
        }
        true
    }

    pub fn cubic_end_points_se(&mut self, pts: &DCubic, s: usize, e: usize) {
        self.a = pts[s].y - pts[e].y;
        self.b = pts[e].x - pts[s].x;
        self.c = pts[s].x * pts[e].y - pts[e].x * pts[s].y;
    }

    pub fn cubic_part(&mut self, part: &DCubic) -> f64 {
        self.cubic_end_points(part);
        if part[0] == part[1] || DLine::new(part[0], part[1]).near_ray(&part[2]) {
            return self.point_distance(&part[3]);
        }
        self.point_distance(&part[2])
    }

    pub fn line_end_points(&mut self, pts: &DLine) {
        self.a = pts[0].y - pts[1].y;
        self.b = pts[1].x - pts[0].x;
        self.c = pts[0].x * pts[1].y - pts[1].x * pts[0].y;
    }

    pub fn quad_end_points(&mut self, pts: &DQuad) -> bool {
        self.quad_end_points_se(pts, 0, 1);
        if self.dy() != 0.0 {
            return true;
        }
        if self.dx() == 0.0 {
            self.quad_end_points_se(pts, 0, 2);
            return false;
        }
        if self.dx() < 0.0 {
            return true;
        }
        if pts[0].y > pts[2].y {
            self.a = DBL_EPSILON;
        }
        true
    }

    pub fn quad_end_points_se(&mut self, pts: &DQuad, s: usize, e: usize) {
        self.a = pts[s].y - pts[e].y;
        self.b = pts[e].x - pts[s].x;
        self.c = pts[s].x * pts[e].y - pts[e].x * pts[s].y;
    }

    pub fn quad_part(&mut self, part: &DQuad) -> f64 {
        self.quad_end_points(part);
        self.point_distance(&part[2])
    }

    pub fn normal_squared(&self) -> f64 {
        self.a * self.a + self.b * self.b
    }

    pub fn normalize(&mut self) -> bool {
        let normal = self.normal_squared().sqrt();
        if approximately_zero(normal) {
            self.a = 0.0;
            self.b = 0.0;
            self.c = 0.0;
            return false;
        }
        let reciprocal = 1.0 / normal;
        self.a *= reciprocal;
        self.b *= reciprocal;
        self.c *= reciprocal;
        true
    }

    pub fn cubic_distance_y(&self, pts: &DCubic, distance: &mut DCubic) {
        let one_third = 1.0 / 3.0;
        for index in 0..4 {
            distance[index].x = index as f64 * one_third;
            distance[index].y = self.a * pts[index].x + self.b * pts[index].y + self.c;
        }
    }

    pub fn quad_distance_y(&self, pts: &DQuad, distance: &mut DQuad) {
        let one_half = 1.0 / 2.0;
        for index in 0..3 {
            distance[index].x = index as f64 * one_half;
            distance[index].y = self.a * pts[index].x + self.b * pts[index].y + self.c;
        }
    }

    pub fn control_pt_distance_cubic(&self, pts: &DCubic, index: usize) -> f64 {
        self.a * pts[index].x + self.b * pts[index].y + self.c
    }

    pub fn control_pt_distance_quad(&self, pts: &DQuad) -> f64 {
        self.a * pts[1].x + self.b * pts[1].y + self.c
    }

    pub fn point_distance(&self, pt: &DPoint) -> f64 {
        self.a * pt.x + self.b * pt.y + self.c
    }

    pub fn dx(&self) -> f64 {
        self.b
    }

    pub fn dy(&self) -> f64 {
        -self.a
    }
}
