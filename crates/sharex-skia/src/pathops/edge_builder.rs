//! `SkOpEdgeBuilder.h`, `SkOpEdgeBuilder.cpp`: the operands' paths, cleaned
//! (tiny values to zero, degenerate curves reduced, contours closed) and cut
//! into segments.

use super::contour::ContourBuilder;
use super::cubic::DCubic;
use super::curve::verb_to_points;
use super::geometry::{chop_quad_at_max_curvature, conic_chop_at, find_quad_max_curvature};
use super::graph::*;
use super::point::DPoint;
use super::reduce_order::ReduceOrder;
use super::types::*;
use crate::geom::{Conic, Point};
use crate::path::{FillType, Path, Verb};

/// Very tiny points cause numerical instability: don't allow them.
fn force_small_to_zero(pt: Point) -> Point {
    let mut ret = pt;
    if (ret.x.abs() as f64) < FLT_EPSILON_ORDERABLE_ERR {
        ret.x = 0.0;
    }
    if (ret.y.abs() as f64) < FLT_EPSILON_ORDERABLE_ERR {
        ret.y = 0.0;
    }
    ret
}

fn can_add_curve(verb: Verb, curve: &mut [Point]) -> bool {
    if Verb::Move == verb {
        return false;
    }
    for p in curve.iter_mut().take(verb_to_points(verb) + 1) {
        *p = force_small_to_zero(*p);
    }
    Verb::Line != verb || !DPoint::approximately_equal_sk(curve[0], curve[1])
}

pub struct EdgeBuilder {
    global_state: *mut GlobalState,
    path_pts: Vec<Point>,
    weights: Vec<f32>,
    /// The verbs; `None` is Skia's `kDone_Verb`.
    path_verbs: Vec<Option<Verb>>,
    contour_builder: ContourBuilder,
    contours_head: Contour,
    xor_mask: [i32; 2],
    second_half: usize,
    operand: bool,
    allow_open_contours: bool,
    unparseable: bool,
}

impl EdgeBuilder {
    pub fn new(path: &Path, contours2: Contour, global_state: *mut GlobalState) -> Self {
        let mut b = EdgeBuilder {
            global_state,
            path_pts: Vec::new(),
            weights: Vec::new(),
            path_verbs: Vec::new(),
            contour_builder: ContourBuilder::new(contours2),
            contours_head: contours2,
            xor_mask: [0; 2],
            second_half: 0,
            operand: false,
            allow_open_contours: false,
            unparseable: false,
        };
        b.init(path);
        b
    }
}

impl EdgeBuilder {
    fn init(&mut self, path: &Path) {
        self.operand = false;
        let mask = if path.fill_type == FillType::EvenOdd { EVEN_ODD_PATH_OPS_MASK } else { WINDING_PATH_OPS_MASK };
        self.xor_mask = [mask, mask];
        self.unparseable = false;
        self.second_half = self.pre_fetch(path);
    }

    pub fn add_operand(&mut self, path: &Path) {
        // fPathVerbs.pop_back(): drop the kDone_Verb
        self.path_verbs.pop();
        self.xor_mask[1] = if path.fill_type == FillType::EvenOdd { EVEN_ODD_PATH_OPS_MASK } else { WINDING_PATH_OPS_MASK };
        self.pre_fetch(path);
    }

    pub fn finish(&mut self) -> bool {
        self.operand = false;
        if self.unparseable || !self.walk() {
            return false;
        }
        self.complete();
        let contour = self.contour_builder.contour();
        if contour.is_some() && contour.count() == 0 {
            self.contours_head.remove(contour);
        }
        true
    }

    fn close_contour(&mut self, curve_end: Point, curve_start: Point) {
        if !DPoint::approximately_equal_sk(curve_end, curve_start) {
            self.path_verbs.push(Some(Verb::Line));
            self.path_pts.push(curve_start);
        } else {
            let verb_count = self.path_verbs.len();
            let pts_count = self.path_pts.len();
            if Some(Verb::Line) == self.path_verbs[verb_count - 1]
                && self.path_pts[pts_count - 2] == curve_start
            {
                self.path_verbs.pop();
                self.path_pts.pop();
            } else {
                self.path_pts[pts_count - 1] = curve_start;
            }
        }
        self.path_verbs.push(Some(Verb::Close));
    }

    fn pre_fetch(&mut self, path: &Path) -> usize {
        if !path.is_finite() {
            self.unparseable = true;
            return 0;
        }
        let mut curve_start = Point::default();
        let mut curve = [Point::default(); 4];
        let mut last_curve = false;
        for (path_verb, pts, w) in path.iter_verbs() {
            let mut verb = path_verb;
            match path_verb {
                Verb::Move => {
                    if !self.allow_open_contours && last_curve {
                        self.close_contour(curve[0], curve_start);
                    }
                    self.path_verbs.push(Some(verb));
                    curve[0] = force_small_to_zero(pts[0]);
                    self.path_pts.push(curve[0]);
                    curve_start = curve[0];
                    last_curve = false;
                    continue;
                }
                Verb::Line => {
                    curve[1] = force_small_to_zero(pts[1]);
                    if DPoint::approximately_equal_sk(curve[0], curve[1]) {
                        let last_verb = *self.path_verbs.last().unwrap();
                        if last_verb != Some(Verb::Line) && last_verb != Some(Verb::Move) {
                            curve[0] = curve[1];
                            *self.path_pts.last_mut().unwrap() = curve[0];
                        }
                        continue; // skip degenerate points
                    }
                }
                Verb::Quad => {
                    curve[1] = force_small_to_zero(pts[1]);
                    curve[2] = force_small_to_zero(pts[2]);
                    let src = curve;
                    verb = ReduceOrder::Quad(&src, &mut curve);
                    if verb == Verb::Move {
                        continue; // skip degenerate points
                    }
                }
                Verb::Conic => {
                    curve[1] = force_small_to_zero(pts[1]);
                    curve[2] = force_small_to_zero(pts[2]);
                    let src = curve;
                    verb = ReduceOrder::Quad(&src, &mut curve);
                    if Verb::Quad == verb && 1.0 != w {
                        verb = Verb::Conic;
                    } else if verb == Verb::Move {
                        continue; // skip degenerate points
                    }
                }
                Verb::Cubic => {
                    curve[1] = force_small_to_zero(pts[1]);
                    curve[2] = force_small_to_zero(pts[2]);
                    curve[3] = force_small_to_zero(pts[3]);
                    let src = curve;
                    verb = ReduceOrder::Cubic(&src, &mut curve);
                    if verb == Verb::Move {
                        continue; // skip degenerate points
                    }
                }
                Verb::Close => {
                    self.close_contour(curve[0], curve_start);
                    last_curve = false;
                    continue;
                }
            }
            self.path_verbs.push(Some(verb));
            let pt_count = verb_to_points(verb);
            self.path_pts.extend_from_slice(&curve[1..=pt_count]);
            if verb == Verb::Conic {
                self.weights.push(w);
            }
            curve[0] = curve[pt_count];
            last_curve = true;
        }
        if !self.allow_open_contours && last_curve {
            self.close_contour(curve[0], curve_start);
        }
        self.path_verbs.push(None);
        self.path_verbs.len() - 1
    }

    fn close(&mut self) -> bool {
        self.complete();
        true
    }

    pub fn complete(&mut self) {
        self.contour_builder.flush();
        let contour = self.contour_builder.contour();
        if contour.is_some() && contour.count() != 0 {
            contour.complete();
            self.contour_builder.set_contour(Contour::NULL);
        }
    }

    pub fn xor_mask(&self) -> i32 {
        self.xor_mask[self.operand as usize]
    }

    pub fn unparseable(&self) -> bool {
        self.unparseable
    }

    fn walk(&mut self) -> bool {
        let mut verb_index = 0usize;
        let end_of_first_half = self.second_half;
        let mut points_index = 0usize;
        let mut weight_index = 0usize;
        let mut contour = self.contour_builder.contour();
        let mut move_to_ptr_bump = 0usize;
        while let Some(verb) = self.path_verbs[verb_index] {
            if verb_index == end_of_first_half {
                self.operand = true;
            }
            verb_index += 1;
            match verb {
                Verb::Move => {
                    if contour.is_some() && contour.count() != 0 {
                        if self.allow_open_contours {
                            self.complete();
                        } else if !self.close() {
                            return false;
                        }
                    }
                    if contour.is_null() {
                        contour = self.contours_head.append_contour();
                        self.contour_builder.set_contour(contour);
                    }
                    let is_xor = self.xor_mask[self.operand as usize] == EVEN_ODD_PATH_OPS_MASK;
                    contour.init(self.global_state, self.operand, is_xor);
                    points_index += move_to_ptr_bump;
                    move_to_ptr_bump = 1;
                    continue;
                }
                Verb::Line => {
                    let pts = [self.path_pts[points_index], self.path_pts[points_index + 1]];
                    self.contour_builder.add_line(&pts);
                }
                Verb::Quad => {
                    let pts = [self.path_pts[points_index], self.path_pts[points_index + 1], self.path_pts[points_index + 2]];
                    let vec1 = Point::new(pts[1].x - pts[0].x, pts[1].y - pts[0].y);
                    let vec2 = Point::new(pts[2].x - pts[1].x, pts[2].y - pts[1].y);
                    let mut added = false;
                    if vec1.dot(vec2) < 0.0 {
                        let mut pair = [Point::default(); 5];
                        if chop_quad_at_max_curvature(&pts, &mut pair) != 1 {
                            if !pair.iter().all(|p| p.x.is_finite() && p.y.is_finite()) {
                                return false;
                            }
                            for p in pair.iter_mut() {
                                *p = force_small_to_zero(*p);
                            }
                            let mut c_storage = [[Point::default(); 2]; 2];
                            let v1 = ReduceOrder::Quad(&pair[0..3], &mut c_storage[0]);
                            let v2 = ReduceOrder::Quad(&pair[2..5], &mut c_storage[1]);
                            let mut curve1: Vec<Point> =
                                if v1 != Verb::Line { pair[0..3].to_vec() } else { c_storage[0].to_vec() };
                            let mut curve2: Vec<Point> =
                                if v2 != Verb::Line { pair[2..5].to_vec() } else { c_storage[1].to_vec() };
                            if can_add_curve(v1, &mut curve1) && can_add_curve(v2, &mut curve2) {
                                self.contour_builder.add_curve(v1, &curve1, 1.0);
                                self.contour_builder.add_curve(v2, &curve2, 1.0);
                                added = true;
                            }
                        }
                    }
                    if !added {
                        // addOneQuad:
                        self.contour_builder.add_quad(&pts);
                    }
                }
                Verb::Conic => {
                    let pts = [self.path_pts[points_index], self.path_pts[points_index + 1], self.path_pts[points_index + 2]];
                    let vec1 = Point::new(pts[1].x - pts[0].x, pts[1].y - pts[0].y);
                    let vec2 = Point::new(pts[2].x - pts[1].x, pts[2].y - pts[1].y);
                    let weight = self.weights[weight_index];
                    weight_index += 1;
                    let mut added = false;
                    if vec1.dot(vec2) < 0.0 {
                        // FIXME: max curvature for conics hasn't been implemented; use placeholder
                        let max_curvature = find_quad_max_curvature(&pts);
                        if 0.0 < max_curvature && max_curvature < 1.0 {
                            let conic = Conic { pts, w: weight };
                            let mut pair = [Conic { pts: [Point::default(); 3], w: 0.0 }; 2];
                            if !conic_chop_at(&conic, max_curvature, &mut pair) {
                                // if result can't be computed, use original
                                self.contour_builder.add_conic(&pts, weight);
                                added = true;
                            } else {
                                let mut c_storage = [[Point::default(); 3]; 2];
                                let v1 = ReduceOrder::Conic(&pair[0], &mut c_storage[0]);
                                let v2 = ReduceOrder::Conic(&pair[1], &mut c_storage[1]);
                                let mut curve1: Vec<Point> =
                                    if v1 != Verb::Line { pair[0].pts.to_vec() } else { c_storage[0].to_vec() };
                                let mut curve2: Vec<Point> =
                                    if v2 != Verb::Line { pair[1].pts.to_vec() } else { c_storage[1].to_vec() };
                                if can_add_curve(v1, &mut curve1) && can_add_curve(v2, &mut curve2) {
                                    self.contour_builder.add_curve(v1, &curve1, pair[0].w);
                                    self.contour_builder.add_curve(v2, &curve2, pair[1].w);
                                    added = true;
                                }
                            }
                        }
                    }
                    if !added {
                        self.contour_builder.add_conic(&pts, weight);
                    }
                }
                Verb::Cubic => {
                    let pts = [
                        self.path_pts[points_index],
                        self.path_pts[points_index + 1],
                        self.path_pts[points_index + 2],
                        self.path_pts[points_index + 3],
                    ];
                    // Split complex cubics (such as self-intersecting curves or
                    // ones with difficult curvature) in two before proceeding.
                    // This can be required for intersection to succeed.
                    let mut split_t = [0.0f32; 3];
                    let breaks = DCubic::ComplexBreak(&pts, &mut split_t) as usize;
                    if breaks == 0 {
                        self.contour_builder.add_cubic(&pts);
                    } else {
                        #[derive(Clone, Copy, Default)]
                        struct Splitsville {
                            t: [f64; 2],
                            pts: [Point; 4],
                            reduced: [Point; 4],
                            verb: Option<Verb>,
                            can_add: bool,
                        }
                        let mut splits = [Splitsville::default(); 4];
                        super::sort::tq_sort_f32(&mut split_t[..breaks]);
                        for index in 0..=breaks {
                            let split = &mut splits[index];
                            split.t[0] = if index != 0 { split_t[index - 1] as f64 } else { 0.0 };
                            split.t[1] = if index < breaks { split_t[index] as f64 } else { 1.0 };
                            let part = DCubic::SubDivide(&pts, split.t[0], split.t[1]);
                            if !part.to_float_points(&mut split.pts) {
                                return false;
                            }
                            let v = ReduceOrder::Cubic(&split.pts, &mut split.reduced);
                            split.verb = Some(v);
                            let mut curve: Vec<Point> =
                                if Verb::Cubic == v { split.pts.to_vec() } else { split.reduced.to_vec() };
                            split.can_add = can_add_curve(v, &mut curve);
                            // can_add_curve zeroes tiny values in place
                            if Verb::Cubic == v {
                                split.pts.copy_from_slice(&curve);
                            } else {
                                split.reduced.copy_from_slice(&curve);
                            }
                        }
                        for index in 0..=breaks {
                            if !splits[index].can_add {
                                continue;
                            }
                            let mut prior = index;
                            while prior > 0 && !splits[prior - 1].can_add {
                                prior -= 1;
                            }
                            if prior < index {
                                splits[index].t[0] = splits[prior].t[0];
                                splits[index].pts[0] = splits[prior].pts[0];
                            }
                            let mut next = index;
                            let break_limit = breaks.min(splits.len() - 1);
                            while next < break_limit && !splits[next + 1].can_add {
                                next += 1;
                            }
                            if next > index {
                                splits[index].t[1] = splits[next].t[1];
                                splits[index].pts[3] = splits[next].pts[3];
                            }
                            if prior < index || next > index {
                                let p = splits[index].pts;
                                let v = ReduceOrder::Cubic(&p, &mut splits[index].reduced);
                                splits[index].verb = Some(v);
                            }
                            let v = splits[index].verb.unwrap();
                            let mut curve: Vec<Point> =
                                if Verb::Cubic == v { splits[index].pts.to_vec() } else { splits[index].reduced.to_vec() };
                            if !can_add_curve(v, &mut curve) {
                                return false;
                            }
                            self.contour_builder.add_curve(v, &curve, 1.0);
                        }
                    }
                }
                Verb::Close => {
                    if !self.close() {
                        return false;
                    }
                    contour = Contour::NULL;
                    continue;
                }
            }
            points_index += verb_to_points(verb);
        }
        self.contour_builder.flush();
        if contour.is_some() && contour.count() != 0 && !self.allow_open_contours && !self.close() {
            return false;
        }
        true
    }
}
