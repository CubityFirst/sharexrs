//! `SkStroke` (`SkStroke.cpp`, `SkStrokerPriv.cpp`): turns a path into the
//! fill path of its stroke, offsetting lines, quads, conics and cubics with
//! quads, with Skia's caps (round caps and joins are conics) and joins.

use crate::geom::{self, Conic, Point, Rect};
use crate::path::{Direction, Matrix, Path, Verb};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cap {
    Butt,
    Round,
    Square,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Join {
    Miter,
    Round,
    Bevel,
}

const NEARLY_ZERO: f32 = 1.0 / 4096.0;
const ROOT2_OVER2: f32 = std::f32::consts::FRAC_1_SQRT_2;

fn nearly_zero_tol(x: f32, tol: f32) -> bool {
    x.abs() <= tol
}

fn can_normalize(dx: f32, dy: f32) -> bool {
    dx.is_finite() && dy.is_finite() && (dx != 0.0 || dy != 0.0)
}

fn degenerate_vector(v: Point) -> bool {
    !can_normalize(v.x, v.y)
}

/// `SkPoint::setLength(x, y, length)`: false (and zero) when it can't.
fn set_length(x: f32, y: f32, length: f32) -> Option<Point> {
    let dmag = ((x as f64) * (x as f64) + (y as f64) * (y as f64)).sqrt();
    let dscale = length as f64 / dmag;
    let nx = (x as f64 * dscale) as f32;
    let ny = (y as f64 * dscale) as f32;
    if !nx.is_finite() || !ny.is_finite() || (nx == 0.0 && ny == 0.0) {
        return None;
    }
    Some(Point::new(nx, ny))
}

fn rotate_ccw(p: Point) -> Point {
    Point::new(p.y, -p.x)
}

fn rotate_cw(p: Point) -> Point {
    Point::new(-p.y, p.x)
}

fn distance_to_sqd(a: Point, b: Point) -> f32 {
    let d = a - b;
    d.dot(d)
}

fn set_normal_unitnormal(
    before: Point,
    after: Point,
    scale: f32,
    radius: f32,
) -> Option<(Point, Point)> {
    let unit = set_length(
        (after.x - before.x) * scale,
        (after.y - before.y) * scale,
        1.0,
    )?;
    let unit = rotate_ccw(unit);
    Some((unit * radius, unit))
}

fn set_normal_unitnormal_vec(vec: Point, radius: f32) -> Option<(Point, Point)> {
    let unit = set_length(vec.x, vec.y, 1.0)?;
    let unit = rotate_ccw(unit);
    Some((unit * radius, unit))
}

// --- curve evaluation (SkGeometry) -----------------------------------------

fn quad_eval(src: &[Point; 3], t: f32) -> Point {
    // SkQuadCoeff: A = P2 - 2P1 + P0, B = 2(P1 - P0), C = P0; (A t + B) t + C
    let c = src[0];
    let b = (src[1] - c) + (src[1] - c);
    let a = src[2] - (src[1] + src[1]) + c;
    Point::new((a.x * t + b.x) * t + c.x, (a.y * t + b.y) * t + c.y)
}

fn quad_tangent(src: &[Point; 3], t: f32) -> Point {
    if (t == 0.0 && src[0] == src[1]) || (t == 1.0 && src[1] == src[2]) {
        return src[2] - src[0];
    }
    let b = src[1] - src[0];
    let a = src[2] - src[1] - b;
    let tt = Point::new(a.x * t + b.x, a.y * t + b.y);
    tt + tt
}

fn conic_eval(c: &Conic, t: f32) -> Point {
    let p0 = c.pts[0];
    let p1 = c.pts[1];
    let p2 = c.pts[2];
    let w = c.w;
    let p1w = p1 * w;
    let nc = p0;
    let na = p2 - (p1w + p1w) + p0;
    let nb = (p1w - p0) + (p1w - p0);
    let dc = 1.0f32;
    let db = (w - dc) + (w - dc);
    let da = 0.0 - db;
    let numer = Point::new((na.x * t + nb.x) * t + nc.x, (na.y * t + nb.y) * t + nc.y);
    let denom = (da * t + db) * t + dc;
    Point::new(numer.x / denom, numer.y / denom)
}

fn conic_tangent(c: &Conic, t: f32) -> Point {
    if (t == 0.0 && c.pts[0] == c.pts[1]) || (t == 1.0 && c.pts[1] == c.pts[2]) {
        return c.pts[2] - c.pts[0];
    }
    let p20 = c.pts[2] - c.pts[0];
    let p10 = c.pts[1] - c.pts[0];
    let cc = p10 * c.w;
    let a = p20 * c.w - p20;
    let b = p20 - cc - cc;
    Point::new((a.x * t + b.x) * t + cc.x, (a.y * t + b.y) * t + cc.y)
}

fn cubic_eval(src: &[Point; 4], t: f32) -> Point {
    let p0 = src[0];
    let p1 = src[1];
    let p2 = src[2];
    let p3 = src[3];
    let a = p3 + (p1 - p2) * 3.0 - p0;
    let b = (p2 - (p1 + p1) + p0) * 3.0;
    let c = (p1 - p0) * 3.0;
    let d = p0;
    Point::new(
        ((a.x * t + b.x) * t + c.x) * t + d.x,
        ((a.y * t + b.y) * t + c.y) * t + d.y,
    )
}

fn eval_cubic_derivative(src: &[Point; 4], t: f32) -> Point {
    let p0 = src[0];
    let p1 = src[1];
    let p2 = src[2];
    let p3 = src[3];
    let a = p3 + (p1 - p2) * 3.0 - p0;
    let bb = p2 - (p1 + p1) + p0;
    let b = bb + bb;
    let c = p1 - p0;
    Point::new((a.x * t + b.x) * t + c.x, (a.y * t + b.y) * t + c.y)
}

fn cubic_tangent(src: &[Point; 4], t: f32) -> Point {
    if (t == 0.0 && src[0] == src[1]) || (t == 1.0 && src[2] == src[3]) {
        let mut tan = if t == 0.0 {
            src[2] - src[0]
        } else {
            src[3] - src[1]
        };
        if tan.x == 0.0 && tan.y == 0.0 {
            tan = src[3] - src[0];
        }
        tan
    } else {
        eval_cubic_derivative(src, t)
    }
}

fn find_quad_max_curvature(src: &[Point; 3]) -> f32 {
    let ax = src[1].x - src[0].x;
    let ay = src[1].y - src[0].y;
    let bx = src[0].x - src[1].x - src[1].x + src[2].x;
    let by = src[0].y - src[1].y - src[1].y + src[2].y;
    let mut numer = -(ax * bx + ay * by);
    let mut denom = bx * bx + by * by;
    if denom < 0.0 {
        numer = -numer;
        denom = -denom;
    }
    if numer <= 0.0 {
        return 0.0;
    }
    if numer >= denom {
        return 1.0;
    }
    numer / denom
}

fn collaps_duplicates(array: &mut [f32], count: usize) -> usize {
    let mut count = count;
    let mut start = 0;
    let mut n = count;
    while n > 1 {
        if array[start] == array[start + 1] {
            for i in 1..n {
                array[start + i - 1] = array[start + i];
            }
            count -= 1;
        } else {
            start += 1;
        }
        n -= 1;
    }
    count
}

fn solve_cubic_poly(coeff: &[f32; 4], t: &mut [f32; 3]) -> usize {
    if coeff[0].abs() <= NEARLY_ZERO {
        let mut r = [0.0f32; 2];
        let n = geom::find_unit_quad_roots(coeff[1], coeff[2], coeff[3], &mut r);
        t[..n].copy_from_slice(&r[..n]);
        return n;
    }
    let inva = 1.0 / coeff[0];
    let a = coeff[1] * inva;
    let b = coeff[2] * inva;
    let c = coeff[3] * inva;
    let q = (a * a - b * 3.0) / 9.0;
    let r = (2.0 * a * a * a - 9.0 * a * b + 27.0 * c) / 54.0;
    let q3 = q * q * q;
    let r2_minus_q3 = r * r - q3;
    let adiv3 = a / 3.0;
    if r2_minus_q3 < 0.0 {
        let theta = (r / q3.sqrt()).clamp(-1.0, 1.0).acos();
        let neg2_root_q = -2.0 * q.sqrt();
        let pi = std::f32::consts::PI;
        t[0] = (neg2_root_q * (theta / 3.0).cos() - adiv3).clamp(0.0, 1.0);
        t[1] = (neg2_root_q * ((theta + 2.0 * pi) / 3.0).cos() - adiv3).clamp(0.0, 1.0);
        t[2] = (neg2_root_q * ((theta - 2.0 * pi) / 3.0).cos() - adiv3).clamp(0.0, 1.0);
        // bubble_sort
        for i in 1..3 {
            for j in (1..=i).rev() {
                if t[j - 1] > t[j] {
                    t.swap(j - 1, j);
                }
            }
        }
        collaps_duplicates(t, 3)
    } else {
        let mut a2 = r.abs() + r2_minus_q3.sqrt();
        a2 = a2.powf(0.3333333);
        if r > 0.0 {
            a2 = -a2;
        }
        if a2 != 0.0 {
            a2 += q / a2;
        }
        t[0] = (a2 - adiv3).clamp(0.0, 1.0);
        1
    }
}

fn formulate_f1_dot_f2(src: [f32; 4]) -> [f32; 4] {
    let a = src[1] - src[0];
    let b = src[2] - 2.0 * src[1] + src[0];
    let c = src[3] + 3.0 * (src[1] - src[2]) - src[0];
    [c * c, 3.0 * b * c, 2.0 * b * b + c * a, a * b]
}

fn find_cubic_max_curvature(src: &[Point; 4], t: &mut [f32; 3]) -> usize {
    let cx = formulate_f1_dot_f2([src[0].x, src[1].x, src[2].x, src[3].x]);
    let cy = formulate_f1_dot_f2([src[0].y, src[1].y, src[2].y, src[3].y]);
    let c = [cx[0] + cy[0], cx[1] + cy[1], cx[2] + cy[2], cx[3] + cy[3]];
    solve_cubic_poly(&c, t)
}

fn find_cubic_inflections(src: &[Point; 4], t: &mut [f32; 2]) -> usize {
    let ax = src[1].x - src[0].x;
    let ay = src[1].y - src[0].y;
    let bx = src[2].x - 2.0 * src[1].x + src[0].x;
    let by = src[2].y - 2.0 * src[1].y + src[0].y;
    let cx = src[3].x + 3.0 * (src[1].x - src[2].x) - src[0].x;
    let cy = src[3].y + 3.0 * (src[1].y - src[2].y) - src[0].y;
    geom::find_unit_quad_roots(bx * cy - by * cx, ax * cy - ay * cx, ax * by - ay * bx, t)
}

fn find_cubic_cusp(src: &[Point; 4]) -> f32 {
    if src[0] == src[1] || src[2] == src[3] {
        return -1.0;
    }
    let on_same_side = |test: usize, line: usize| {
        let origin = src[line];
        let l = src[line + 1] - origin;
        let c0 = l.cross(src[test] - origin);
        let c1 = l.cross(src[test + 1] - origin);
        c0 * c1 >= 0.0
    };
    if on_same_side(0, 2) || on_same_side(2, 0) {
        return -1.0;
    }
    let mut mc = [0.0f32; 3];
    let roots = find_cubic_max_curvature(src, &mut mc);
    for &test_t in &mc[..roots] {
        if 0.0 >= test_t || test_t >= 1.0 {
            continue;
        }
        let d = eval_cubic_derivative(src, test_t);
        let mag = d.dot(d);
        let precision = (distance_to_sqd(src[1], src[0])
            + distance_to_sqd(src[2], src[1])
            + distance_to_sqd(src[3], src[2]))
            * 1e-8;
        if mag < precision {
            return test_t;
        }
    }
    -1.0
}

/// `SkConic::BuildUnitArc` with the joiner's matrix (scale `radius`,
/// translate `pivot`).
fn build_unit_arc(
    u_start: Point,
    u_stop: Point,
    ccw: bool,
    radius: f32,
    pivot: Point,
) -> Vec<Conic> {
    let user = Matrix {
        sx: radius,
        kx: 0.0,
        tx: pivot.x,
        ky: 0.0,
        sy: radius,
        ty: pivot.y,
    };
    build_unit_arc_with(u_start, u_stop, ccw, &user)
}

/// `SkConic::BuildUnitArc` with a user matrix.
pub(crate) fn build_unit_arc_with(u_start: Point, u_stop: Point, ccw: bool, user: &Matrix) -> Vec<Conic> {
    let x = u_start.dot(u_stop);
    let mut y = u_start.cross(u_stop);
    let abs_y = y.abs();
    if abs_y <= NEARLY_ZERO && x > 0.0 && ((y >= 0.0 && !ccw) || (y <= 0.0 && ccw)) {
        return Vec::new();
    }
    if ccw {
        y = -y;
    }
    let mut quadrant = 0usize;
    if y == 0.0 {
        quadrant = 2;
    } else if x == 0.0 {
        quadrant = if y > 0.0 { 1 } else { 3 };
    } else {
        if y < 0.0 {
            quadrant += 2;
        }
        if (x < 0.0) != (y < 0.0) {
            quadrant += 1;
        }
    }
    let qp = [
        Point::new(1.0, 0.0),
        Point::new(1.0, 1.0),
        Point::new(0.0, 1.0),
        Point::new(-1.0, 1.0),
        Point::new(-1.0, 0.0),
        Point::new(-1.0, -1.0),
        Point::new(0.0, -1.0),
        Point::new(1.0, -1.0),
    ];
    let mut dst: Vec<Conic> = Vec::new();
    for i in 0..quadrant {
        dst.push(Conic::new(
            [qp[i * 2], qp[i * 2 + 1], qp[(i * 2 + 2) % 8]],
            ROOT2_OVER2,
        ));
    }
    let final_p = Point::new(x, y);
    let last_q = qp[quadrant * 2];
    let dot = last_q.dot(final_p);
    if dot < 1.0 {
        let off_curve = Point::new(last_q.x + x, last_q.y + y);
        let cos_theta_over2 = ((1.0 + dot) / 2.0).sqrt();
        let off_curve =
            set_length(off_curve.x, off_curve.y, 1.0 / cos_theta_over2).unwrap_or_default();
        let d = last_q - off_curve;
        let equal = !(d.is_finite() && (d.x != 0.0 || d.y != 0.0));
        if !equal {
            dst.push(Conic::new([last_q, off_curve, final_p], cos_theta_over2));
        }
    }
    // matrix.setSinCos(uStart.fY, uStart.fX); preScale(1, -1) if CCW; postConcat(user)
    let mut rot = Matrix {
        sx: u_start.x,
        kx: -u_start.y,
        tx: 0.0,
        ky: u_start.y,
        sy: u_start.x,
        ty: 0.0,
    };
    if ccw {
        rot.kx *= -1.0;
        rot.sy *= -1.0;
    }
    let m = user.pre_concat(&rot);
    for c in &mut dst {
        for p in &mut c.pts {
            *p = m.map(*p);
        }
    }
    dst
}

// --- caps and joins (SkStrokerPriv.cpp) -------------------------------------

fn cap(
    cap: Cap,
    path: &mut Path,
    pivot: Point,
    normal: Point,
    stop: Point,
    other_path_is_line: bool,
) {
    match cap {
        Cap::Butt => {
            path.line_to(stop.x, stop.y);
        }
        Cap::Round => {
            let parallel = rotate_cw(normal);
            let projected_center = pivot + parallel;
            let a = projected_center + normal;
            path.conic_to(
                a.x,
                a.y,
                projected_center.x,
                projected_center.y,
                ROOT2_OVER2,
            );
            let b = projected_center - normal;
            path.conic_to(b.x, b.y, stop.x, stop.y, ROOT2_OVER2);
        }
        Cap::Square => {
            let parallel = rotate_cw(normal);
            if other_path_is_line {
                path.set_last_pt_pub(Point::new(
                    pivot.x + normal.x + parallel.x,
                    pivot.y + normal.y + parallel.y,
                ));
                path.line_to(
                    pivot.x - normal.x + parallel.x,
                    pivot.y - normal.y + parallel.y,
                );
            } else {
                path.line_to(
                    pivot.x + normal.x + parallel.x,
                    pivot.y + normal.y + parallel.y,
                );
                path.line_to(
                    pivot.x - normal.x + parallel.x,
                    pivot.y - normal.y + parallel.y,
                );
                path.line_to(stop.x, stop.y);
            }
        }
    }
}

fn is_clockwise(before: Point, after: Point) -> bool {
    before.x * after.y > before.y * after.x
}

#[derive(PartialEq, Eq)]
enum AngleType {
    Nearly180,
    Sharp,
    Shallow,
    NearlyLine,
}

fn dot2_angle_type(dot: f32) -> AngleType {
    if dot >= 0.0 {
        if (1.0 - dot).abs() <= NEARLY_ZERO {
            AngleType::NearlyLine
        } else {
            AngleType::Shallow
        }
    } else if (1.0 + dot).abs() <= NEARLY_ZERO {
        AngleType::Nearly180
    } else {
        AngleType::Sharp
    }
}

fn handle_inner_join(inner: &mut Path, pivot: Point, after: Point) {
    inner.line_to(pivot.x, pivot.y);
    inner.line_to(pivot.x - after.x, pivot.y - after.y);
}

#[allow(clippy::too_many_arguments)]
fn join<'a>(
    join: Join,
    mut outer: &'a mut Path,
    mut inner: &'a mut Path,
    before_unit_normal: Point,
    pivot: Point,
    after_unit_normal: Point,
    radius: f32,
    inv_miter_limit: f32,
    prev_is_line: bool,
    mut curr_is_line: bool,
) {
    match join {
        Join::Bevel => {
            let mut after = after_unit_normal * radius;
            if !is_clockwise(before_unit_normal, after_unit_normal) {
                std::mem::swap(&mut outer, &mut inner);
                after = -after;
            }
            outer.line_to(pivot.x + after.x, pivot.y + after.y);
            handle_inner_join(inner, pivot, after);
        }
        Join::Round => {
            let dot = before_unit_normal.dot(after_unit_normal);
            if dot2_angle_type(dot) == AngleType::NearlyLine {
                return;
            }
            let mut before = before_unit_normal;
            let mut after = after_unit_normal;
            let mut ccw = false;
            if !is_clockwise(before, after) {
                std::mem::swap(&mut outer, &mut inner);
                before = -before;
                after = -after;
                ccw = true;
            }
            let conics = build_unit_arc(before, after, ccw, radius, pivot);
            if !conics.is_empty() {
                for c in &conics {
                    outer.conic_to(c.pts[1].x, c.pts[1].y, c.pts[2].x, c.pts[2].y, c.w);
                }
                let after = after * radius;
                handle_inner_join(inner, pivot, after);
            }
        }
        Join::Miter => {
            let dot = before_unit_normal.dot(after_unit_normal);
            let angle_type = dot2_angle_type(dot);
            let mut before = before_unit_normal;
            let mut after = after_unit_normal;
            if angle_type == AngleType::NearlyLine {
                return;
            }
            let mut mid = Point::default();
            if angle_type == AngleType::Nearly180 {
                curr_is_line = false;
            } else {
                let mut do_blunt = false;
                let ccw = !is_clockwise(before, after);
                if ccw {
                    std::mem::swap(&mut outer, &mut inner);
                    before = -before;
                    after = -after;
                }
                if dot == 0.0 && inv_miter_limit <= std::f32::consts::FRAC_1_SQRT_2 {
                    mid = (before + after) * radius;
                } else {
                    let sin_half_angle = (0.5 * (1.0 + dot)).sqrt();
                    if sin_half_angle < inv_miter_limit {
                        curr_is_line = false;
                        do_blunt = true;
                    } else {
                        if angle_type == AngleType::Sharp {
                            mid = Point::new(after.y - before.y, before.x - after.x);
                            if ccw {
                                mid = -mid;
                            }
                        } else {
                            mid = Point::new(before.x + after.x, before.y + after.y);
                        }
                        mid = set_length(mid.x, mid.y, radius / sin_half_angle).unwrap_or_default();
                    }
                }
                if !do_blunt {
                    if prev_is_line {
                        outer.set_last_pt_pub(Point::new(pivot.x + mid.x, pivot.y + mid.y));
                    } else {
                        outer.line_to(pivot.x + mid.x, pivot.y + mid.y);
                    }
                }
            }
            let after = after * radius;
            if !curr_is_line {
                outer.line_to(pivot.x + after.x, pivot.y + after.y);
            }
            handle_inner_join(inner, pivot, after);
        }
    }
}

// --- SkPathStroker ----------------------------------------------------------

#[derive(Clone, Copy, Default)]
struct QuadConstruct {
    quad: [Point; 3],
    tangent_start: Point,
    tangent_end: Point,
    start_t: f32,
    mid_t: f32,
    end_t: f32,
    start_set: bool,
    end_set: bool,
    opposite_tangents: bool,
}

impl QuadConstruct {
    fn init(&mut self, start: f32, end: f32) -> bool {
        self.start_t = start;
        self.mid_t = (start + end) * 0.5;
        self.end_t = end;
        self.start_set = false;
        self.end_set = false;
        self.start_t < self.mid_t && self.mid_t < self.end_t
    }

    fn init_with_start(&mut self, parent: &QuadConstruct) -> bool {
        if !self.init(parent.start_t, parent.mid_t) {
            return false;
        }
        self.quad[0] = parent.quad[0];
        self.tangent_start = parent.tangent_start;
        self.start_set = true;
        true
    }

    fn init_with_end(&mut self, parent: &QuadConstruct) -> bool {
        if !self.init(parent.mid_t, parent.end_t) {
            return false;
        }
        self.quad[2] = parent.quad[2];
        self.tangent_end = parent.tangent_end;
        self.end_set = true;
        true
    }
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum ResultType {
    Split,
    Degenerate,
    Quad,
}

#[derive(PartialEq, Eq, Clone, Copy, PartialOrd, Ord)]
enum ReductionType {
    Point,
    Line,
    Quad,
    Degenerate,
    Degenerate2,
    Degenerate3,
}

const RECURSIVE_LIMITS: [i32; 4] = [5 * 3, 24, 11 * 3, 11 * 3];
const TANGENT_LIMIT: usize = 0;
const CONIC_LIMIT: usize = 2;
const QUAD_LIMIT: usize = 3;

struct PathStroker {
    radius: f32,
    inv_miter_limit: f32,
    res_scale: f32,
    inv_res_scale: f32,
    inv_res_scale_squared: f32,

    first_normal: Point,
    prev_normal: Point,
    first_unit_normal: Point,
    prev_unit_normal: Point,
    first_pt: Point,
    prev_pt: Point,
    first_outer_pt: Point,
    first_outer_pt_index_in_contour: usize,
    segment_count: i32,
    prev_is_line: bool,
    can_ignore_center: bool,

    cap: Cap,
    join: Join,

    inner: Path,
    outer: Path,
    cusper: Path,

    stroke_type_outer: bool,
    recursion_depth: i32,
    found_tangents: bool,
    join_completed: bool,
}

fn pt_to_line(pt: Point, line_start: Point, line_end: Point) -> f32 {
    let dxy = line_end - line_start;
    let ab0 = pt - line_start;
    let numer = dxy.dot(ab0);
    let denom = dxy.dot(dxy);
    let t = numer / denom;
    if (0.0..=1.0).contains(&t) {
        let hit = Point::new(
            line_start.x * (1.0 - t) + line_end.x * t,
            line_start.y * (1.0 - t) + line_end.y * t,
        );
        distance_to_sqd(hit, pt)
    } else {
        distance_to_sqd(pt, line_start)
    }
}

fn cubic_in_line(cubic: &[Point; 4]) -> bool {
    let mut pt_max = -1.0f32;
    let mut outer1 = 0usize;
    let mut outer2 = 0usize;
    for index in 0..3 {
        for inner in index + 1..4 {
            let d = cubic[inner] - cubic[index];
            let test_max = d.x.abs().max(d.y.abs());
            if pt_max < test_max {
                outer1 = index;
                outer2 = inner;
                pt_max = test_max;
            }
        }
    }
    let mid1 = (1 + (2 >> outer2)) >> outer1;
    let mid2 = outer1 ^ outer2 ^ mid1;
    let line_slop = pt_max * pt_max * 0.00001;
    pt_to_line(cubic[mid1], cubic[outer1], cubic[outer2]) <= line_slop
        && pt_to_line(cubic[mid2], cubic[outer1], cubic[outer2]) <= line_slop
}

fn quad_in_line(quad: &[Point; 3]) -> bool {
    let mut pt_max = -1.0f32;
    let mut outer1 = 0usize;
    let mut outer2 = 0usize;
    for index in 0..2 {
        for inner in index + 1..3 {
            let d = quad[inner] - quad[index];
            let test_max = d.x.abs().max(d.y.abs());
            if pt_max < test_max {
                outer1 = index;
                outer2 = inner;
                pt_max = test_max;
            }
        }
    }
    let mid = outer1 ^ outer2 ^ 3;
    let line_slop = pt_max * pt_max * 0.000005;
    pt_to_line(quad[mid], quad[outer1], quad[outer2]) <= line_slop
}

fn check_quad_linear(quad: &[Point; 3]) -> (ReductionType, Point) {
    let ab = degenerate_vector(quad[1] - quad[0]);
    let bc = degenerate_vector(quad[2] - quad[1]);
    if ab & bc {
        return (ReductionType::Point, Point::default());
    }
    if ab | bc {
        return (ReductionType::Line, Point::default());
    }
    if !quad_in_line(quad) {
        return (ReductionType::Quad, Point::default());
    }
    let t = find_quad_max_curvature(quad);
    if t == 0.0 || t == 1.0 {
        return (ReductionType::Line, Point::default());
    }
    (ReductionType::Degenerate, quad_eval(quad, t))
}

fn check_conic_linear(conic: &Conic) -> (ReductionType, Point) {
    let ab = degenerate_vector(conic.pts[1] - conic.pts[0]);
    let bc = degenerate_vector(conic.pts[2] - conic.pts[1]);
    if ab & bc {
        return (ReductionType::Point, Point::default());
    }
    if ab | bc {
        return (ReductionType::Line, Point::default());
    }
    if !quad_in_line(&conic.pts) {
        return (ReductionType::Quad, Point::default());
    }
    let t = find_quad_max_curvature(&conic.pts);
    if t == 0.0 {
        return (ReductionType::Line, Point::default());
    }
    (ReductionType::Degenerate, conic_eval(conic, t))
}

fn check_cubic_linear(cubic: &[Point; 4]) -> (ReductionType, [Point; 3], usize) {
    let ab = degenerate_vector(cubic[1] - cubic[0]);
    let bc = degenerate_vector(cubic[2] - cubic[1]);
    let cd = degenerate_vector(cubic[3] - cubic[2]);
    let mut reduction = [Point::default(); 3];
    if ab & bc & cd {
        return (ReductionType::Point, reduction, 1);
    }
    if ab as i32 + bc as i32 + cd as i32 == 2 {
        return (ReductionType::Line, reduction, 1);
    }
    if !cubic_in_line(cubic) {
        return (ReductionType::Quad, reduction, if ab { 2 } else { 1 });
    }
    let mut t_values = [0.0f32; 3];
    let count = find_cubic_max_curvature(cubic, &mut t_values);
    let mut r_count = 0;
    for &t in &t_values[..count] {
        if 0.0 >= t || t >= 1.0 {
            continue;
        }
        reduction[r_count] = cubic_eval(cubic, t);
        if reduction[r_count] != cubic[0] && reduction[r_count] != cubic[3] {
            r_count += 1;
        }
    }
    if r_count == 0 {
        return (ReductionType::Line, reduction, 1);
    }
    let rt = match r_count {
        1 => ReductionType::Degenerate,
        2 => ReductionType::Degenerate2,
        _ => ReductionType::Degenerate3,
    };
    (rt, reduction, 1)
}

fn intersect_quad_ray(line: &[Point; 2], quad: &[Point; 3], roots: &mut [f32; 2]) -> usize {
    let vec = line[1] - line[0];
    let mut r = [0.0f32; 3];
    for n in 0..3 {
        r[n] = (quad[n].y - line[0].y) * vec.x - (quad[n].x - line[0].x) * vec.y;
    }
    let mut a = r[2];
    let mut b = r[1];
    let c = r[0];
    a += c - 2.0 * b;
    b -= c;
    geom::find_unit_quad_roots(a, 2.0 * b, c, roots)
}

fn points_within_dist(near: Point, far: Point, limit: f32) -> bool {
    distance_to_sqd(near, far) <= limit * limit
}

fn sharp_angle(quad: &[Point; 3]) -> bool {
    let mut smaller = quad[1] - quad[0];
    let mut larger = quad[1] - quad[2];
    let smaller_len = smaller.dot(smaller);
    let mut larger_len = larger.dot(larger);
    if smaller_len > larger_len {
        std::mem::swap(&mut smaller, &mut larger);
        larger_len = smaller_len;
    }
    let Some(smaller) = set_length(smaller.x, smaller.y, larger_len) else {
        return false;
    };
    smaller.dot(larger) > 0.0
}

impl PathStroker {
    fn new(
        src: &Path,
        radius: f32,
        miter_limit: f32,
        cap: Cap,
        mut join: Join,
        res_scale: f32,
        can_ignore_center: bool,
    ) -> Self {
        let mut inv_miter_limit = 0.0;
        if join == Join::Miter {
            if miter_limit <= 1.0 {
                join = Join::Bevel;
            } else {
                inv_miter_limit = 1.0 / miter_limit;
            }
        }
        let _ = src;
        let inv_res_scale = 1.0 / (res_scale * 4.0);
        Self {
            radius,
            inv_miter_limit,
            res_scale,
            inv_res_scale,
            inv_res_scale_squared: inv_res_scale * inv_res_scale,
            first_normal: Point::default(),
            prev_normal: Point::default(),
            first_unit_normal: Point::default(),
            prev_unit_normal: Point::default(),
            first_pt: Point::default(),
            prev_pt: Point::default(),
            first_outer_pt: Point::default(),
            first_outer_pt_index_in_contour: 0,
            segment_count: -1,
            prev_is_line: false,
            can_ignore_center,
            cap,
            join,
            inner: Path::new(),
            outer: Path::new(),
            cusper: Path::new(),
            stroke_type_outer: true,
            recursion_depth: 0,
            found_tangents: false,
            join_completed: false,
        }
    }

    fn has_only_move_to(&self) -> bool {
        self.segment_count == 0
    }

    fn is_current_contour_empty(&self) -> bool {
        self.inner.is_zero_length_since_point(0)
            && self
                .outer
                .is_zero_length_since_point(self.first_outer_pt_index_in_contour)
    }

    fn do_join(
        &mut self,
        before: Point,
        pivot: Point,
        after: Point,
        prev_is_line: bool,
        curr_is_line: bool,
    ) {
        join(
            self.join,
            &mut self.outer,
            &mut self.inner,
            before,
            pivot,
            after,
            self.radius,
            self.inv_miter_limit,
            prev_is_line,
            curr_is_line,
        );
    }

    fn pre_join_to(&mut self, curr_pt: Point, curr_is_line: bool) -> Option<(Point, Point)> {
        let prev_x = self.prev_pt.x;
        let prev_y = self.prev_pt.y;
        let (normal, unit_normal) =
            match set_normal_unitnormal(self.prev_pt, curr_pt, self.res_scale, self.radius) {
                Some(v) => v,
                None => {
                    if self.cap == Cap::Butt {
                        return None;
                    }
                    (Point::new(self.radius, 0.0), Point::new(1.0, 0.0))
                }
            };
        if self.segment_count == 0 {
            self.first_normal = normal;
            self.first_unit_normal = unit_normal;
            self.first_outer_pt = Point::new(prev_x + normal.x, prev_y + normal.y);
            self.outer
                .move_to(self.first_outer_pt.x, self.first_outer_pt.y);
            self.inner.move_to(prev_x - normal.x, prev_y - normal.y);
        } else {
            let (pu, pp, pil) = (self.prev_unit_normal, self.prev_pt, self.prev_is_line);
            self.do_join(pu, pp, unit_normal, pil, curr_is_line);
        }
        self.prev_is_line = curr_is_line;
        Some((normal, unit_normal))
    }

    fn post_join_to(&mut self, curr_pt: Point, normal: Point, unit_normal: Point) {
        self.join_completed = true;
        self.prev_pt = curr_pt;
        self.prev_unit_normal = unit_normal;
        self.prev_normal = normal;
        self.segment_count += 1;
    }

    fn finish_contour(&mut self, close: bool, curr_is_line: bool) {
        if self.segment_count > 0 {
            if close {
                let (pu, pp, fu, pil) = (
                    self.prev_unit_normal,
                    self.prev_pt,
                    self.first_unit_normal,
                    self.prev_is_line,
                );
                self.do_join(pu, pp, fu, pil, curr_is_line);
                self.outer.close();
                if self.can_ignore_center {
                    if self.inner.bounds().contains(&self.outer.bounds()) {
                        std::mem::swap(&mut self.inner, &mut self.outer);
                    }
                } else {
                    let pt = self.inner.last_pt().unwrap_or_default();
                    self.outer.move_to(pt.x, pt.y);
                    let inner = std::mem::take(&mut self.inner);
                    self.outer.reverse_path_to(&inner);
                    self.inner = inner;
                    self.outer.close();
                }
            } else {
                let pt = self.inner.last_pt().unwrap_or_default();
                let (pp, pn) = (self.prev_pt, self.prev_normal);
                cap(self.cap, &mut self.outer, pp, pn, pt, curr_is_line);
                let inner = std::mem::take(&mut self.inner);
                self.outer.reverse_path_to(&inner);
                self.inner = inner;
                let (fp, fnorm, fop, pil) = (
                    self.first_pt,
                    self.first_normal,
                    self.first_outer_pt,
                    self.prev_is_line,
                );
                cap(self.cap, &mut self.outer, fp, -fnorm, fop, pil);
                self.outer.close();
            }
            if !self.cusper.is_empty() {
                let cusper = std::mem::take(&mut self.cusper);
                self.outer.add_path(&cusper);
            }
        }
        self.inner = Path::new();
        self.segment_count = -1;
        self.first_outer_pt_index_in_contour = self.outer.count_points();
    }

    fn move_to(&mut self, pt: Point) {
        if self.segment_count > 0 {
            self.finish_contour(false, false);
        }
        self.segment_count = 0;
        self.first_pt = pt;
        self.prev_pt = pt;
        self.join_completed = false;
    }

    fn line_to_pts(&mut self, curr_pt: Point, normal: Point) {
        self.outer
            .line_to(curr_pt.x + normal.x, curr_pt.y + normal.y);
        self.inner
            .line_to(curr_pt.x - normal.x, curr_pt.y - normal.y);
    }

    fn line_to(&mut self, curr_pt: Point, has_valid_tangent: Option<&dyn Fn() -> bool>) {
        let tol = NEARLY_ZERO * self.inv_res_scale;
        let teeny_line = nearly_zero_tol(self.prev_pt.x - curr_pt.x, tol)
            && nearly_zero_tol(self.prev_pt.y - curr_pt.y, tol);
        if self.cap == Cap::Butt && teeny_line {
            return;
        }
        if teeny_line && (self.join_completed || has_valid_tangent.is_some_and(|f| f())) {
            return;
        }
        let Some((normal, unit_normal)) = self.pre_join_to(curr_pt, true) else {
            return;
        };
        self.line_to_pts(curr_pt, normal);
        self.post_join_to(curr_pt, normal, unit_normal);
    }

    fn set_quad_end_normal(
        &self,
        quad: &[Point; 3],
        normal_ab: Point,
        unit_ab: Point,
    ) -> (Point, Point) {
        set_normal_unitnormal(quad[1], quad[2], self.res_scale, self.radius)
            .unwrap_or((normal_ab, unit_ab))
    }

    fn set_cubic_end_normal(
        &self,
        cubic: &[Point; 4],
        normal_ab: Point,
        unit_ab: Point,
    ) -> (Point, Point) {
        let mut ab = cubic[1] - cubic[0];
        let mut cd = cubic[3] - cubic[2];
        let mut dab = degenerate_vector(ab);
        let mut dcd = degenerate_vector(cd);
        if dab && dcd {
            return (normal_ab, unit_ab);
        }
        if dab {
            ab = cubic[2] - cubic[0];
            dab = degenerate_vector(ab);
        }
        if dcd {
            cd = cubic[3] - cubic[1];
            dcd = degenerate_vector(cd);
        }
        if dab || dcd {
            return (normal_ab, unit_ab);
        }
        set_normal_unitnormal_vec(cd, self.radius).unwrap_or((normal_ab, unit_ab))
    }

    fn init(&mut self, outer: bool, q: &mut QuadConstruct, t_start: f32, t_end: f32) {
        self.stroke_type_outer = outer;
        self.found_tangents = false;
        q.init(t_start, t_end);
    }

    fn set_ray_pts(&self, t_pt: Point, dxy: Point) -> (Point, Point) {
        let dxy = set_length(dxy.x, dxy.y, self.radius).unwrap_or(Point::new(self.radius, 0.0));
        let axis_flip = if self.stroke_type_outer { 1.0 } else { -1.0 };
        let on_pt = Point::new(t_pt.x + axis_flip * dxy.y, t_pt.y - axis_flip * dxy.x);
        let tangent = Point::new(on_pt.x + dxy.x, on_pt.y + dxy.y);
        (on_pt, tangent)
    }

    fn conic_perp_ray(&self, conic: &Conic, t: f32) -> (Point, Point, Point) {
        let t_pt = conic_eval(conic, t);
        let mut dxy = conic_tangent(conic, t);
        if dxy.x == 0.0 && dxy.y == 0.0 {
            dxy = conic.pts[2] - conic.pts[0];
        }
        let (on, tan) = self.set_ray_pts(t_pt, dxy);
        (t_pt, on, tan)
    }

    fn quad_perp_ray(&self, quad: &[Point; 3], t: f32) -> (Point, Point, Point) {
        let t_pt = quad_eval(quad, t);
        let mut dxy = quad_tangent(quad, t);
        if dxy.x == 0.0 && dxy.y == 0.0 {
            dxy = quad[2] - quad[0];
        }
        let (on, tan) = self.set_ray_pts(t_pt, dxy);
        (t_pt, on, tan)
    }

    fn cubic_perp_ray(&self, cubic: &[Point; 4], t: f32) -> (Point, Point, Point) {
        let t_pt = cubic_eval(cubic, t);
        let mut dxy = cubic_tangent(cubic, t);
        if dxy.x == 0.0 && dxy.y == 0.0 {
            let mut c_pts: [Point; 4] = *cubic;
            if t.abs() <= NEARLY_ZERO {
                dxy = cubic[2] - cubic[0];
            } else if (1.0 - t).abs() <= NEARLY_ZERO {
                dxy = cubic[3] - cubic[1];
            } else {
                let mut chopped = [Point::default(); 7];
                geom::chop_cubic_at(cubic, &mut chopped, t);
                dxy = chopped[3] - chopped[2];
                if dxy.x == 0.0 && dxy.y == 0.0 {
                    dxy = chopped[3] - chopped[1];
                    c_pts = [chopped[0], chopped[1], chopped[2], chopped[3]];
                }
            }
            if dxy.x == 0.0 && dxy.y == 0.0 {
                dxy = c_pts[3] - c_pts[0];
            }
        }
        let (on, tan) = self.set_ray_pts(t_pt, dxy);
        (t_pt, on, tan)
    }

    fn conic_quad_ends(&self, conic: &Conic, q: &mut QuadConstruct) {
        if !q.start_set {
            let (_, on, tan) = self.conic_perp_ray(conic, q.start_t);
            q.quad[0] = on;
            q.tangent_start = tan;
            q.start_set = true;
        }
        if !q.end_set {
            let (_, on, tan) = self.conic_perp_ray(conic, q.end_t);
            q.quad[2] = on;
            q.tangent_end = tan;
            q.end_set = true;
        }
    }

    fn cubic_quad_ends(&self, cubic: &[Point; 4], q: &mut QuadConstruct) {
        if !q.start_set {
            let (_, on, tan) = self.cubic_perp_ray(cubic, q.start_t);
            q.quad[0] = on;
            q.tangent_start = tan;
            q.start_set = true;
        }
        if !q.end_set {
            let (_, on, tan) = self.cubic_perp_ray(cubic, q.end_t);
            q.quad[2] = on;
            q.tangent_end = tan;
            q.end_set = true;
        }
    }

    fn intersect_ray(&self, q: &mut QuadConstruct, ctrl_pt: bool) -> ResultType {
        let start = q.quad[0];
        let end = q.quad[2];
        let a_len = q.tangent_start - start;
        let b_len = q.tangent_end - end;
        let denom = a_len.cross(b_len);
        if denom == 0.0 || !denom.is_finite() {
            q.opposite_tangents = a_len.dot(b_len) < 0.0;
            return ResultType::Degenerate;
        }
        q.opposite_tangents = false;
        let ab0 = start - end;
        let mut numer_a = b_len.cross(ab0);
        let numer_b = a_len.cross(ab0);
        if (numer_a >= 0.0) == (numer_b >= 0.0) {
            let dist1 = pt_to_line(start, end, q.tangent_end);
            let dist2 = pt_to_line(end, start, q.tangent_start);
            if dist1.max(dist2) <= self.inv_res_scale_squared {
                return ResultType::Degenerate;
            }
            return ResultType::Split;
        }
        numer_a /= denom;
        let valid_divide = numer_a > numer_a - 1.0;
        if valid_divide {
            if ctrl_pt {
                q.quad[1] = Point::new(
                    start.x * (1.0 - numer_a) + q.tangent_start.x * numer_a,
                    start.y * (1.0 - numer_a) + q.tangent_start.y * numer_a,
                );
            }
            return ResultType::Quad;
        }
        q.opposite_tangents = a_len.dot(b_len) < 0.0;
        ResultType::Degenerate
    }

    fn pt_in_quad_bounds(&self, quad: &[Point; 3], pt: Point) -> bool {
        let x_min = quad[0].x.min(quad[1].x).min(quad[2].x);
        if pt.x + self.inv_res_scale < x_min {
            return false;
        }
        let x_max = quad[0].x.max(quad[1].x).max(quad[2].x);
        if pt.x - self.inv_res_scale > x_max {
            return false;
        }
        let y_min = quad[0].y.min(quad[1].y).min(quad[2].y);
        if pt.y + self.inv_res_scale < y_min {
            return false;
        }
        let y_max = quad[0].y.max(quad[1].y).max(quad[2].y);
        if pt.y - self.inv_res_scale > y_max {
            return false;
        }
        true
    }

    fn stroke_close_enough(
        &self,
        stroke: &[Point; 3],
        ray: &[Point; 2],
        q: &QuadConstruct,
    ) -> ResultType {
        let stroke_mid = quad_eval(stroke, 0.5);
        if points_within_dist(ray[0], stroke_mid, self.inv_res_scale) {
            if sharp_angle(&q.quad) {
                return ResultType::Split;
            }
            return ResultType::Quad;
        }
        if !self.pt_in_quad_bounds(stroke, ray[0]) {
            return ResultType::Split;
        }
        let mut roots = [0.0f32; 2];
        let root_count = intersect_quad_ray(ray, stroke, &mut roots);
        if root_count != 1 {
            return ResultType::Split;
        }
        let quad_pt = quad_eval(stroke, roots[0]);
        let error = self.inv_res_scale * (1.0 - (roots[0] - 0.5).abs() * 2.0);
        if points_within_dist(ray[0], quad_pt, error) {
            if sharp_angle(&q.quad) {
                return ResultType::Split;
            }
            return ResultType::Quad;
        }
        ResultType::Split
    }

    fn compare_quad_conic(&self, conic: &Conic, q: &mut QuadConstruct) -> ResultType {
        self.conic_quad_ends(conic, q);
        let r = self.intersect_ray(q, true);
        if r != ResultType::Quad {
            return r;
        }
        let (on_curve, on_stroke, _) = self.conic_perp_ray(conic, q.mid_t);
        let quad = q.quad;
        self.stroke_close_enough(&quad, &[on_stroke, on_curve], q)
    }

    fn compare_quad_cubic(&self, cubic: &[Point; 4], q: &mut QuadConstruct) -> ResultType {
        self.cubic_quad_ends(cubic, q);
        let r = self.intersect_ray(q, true);
        if r != ResultType::Quad {
            return r;
        }
        let (on_curve, on_stroke, _) = self.cubic_perp_ray(cubic, q.mid_t);
        let quad = q.quad;
        self.stroke_close_enough(&quad, &[on_stroke, on_curve], q)
    }

    fn compare_quad_quad(&self, quad: &[Point; 3], q: &mut QuadConstruct) -> ResultType {
        if !q.start_set {
            let (_, on, tan) = self.quad_perp_ray(quad, q.start_t);
            q.quad[0] = on;
            q.tangent_start = tan;
            q.start_set = true;
        }
        if !q.end_set {
            let (_, on, tan) = self.quad_perp_ray(quad, q.end_t);
            q.quad[2] = on;
            q.tangent_end = tan;
            q.end_set = true;
        }
        let r = self.intersect_ray(q, true);
        if r != ResultType::Quad {
            return r;
        }
        let (on_curve, on_stroke, _) = self.quad_perp_ray(quad, q.mid_t);
        let qq = q.quad;
        self.stroke_close_enough(&qq, &[on_stroke, on_curve], q)
    }

    fn path(&mut self) -> &mut Path {
        if self.stroke_type_outer {
            &mut self.outer
        } else {
            &mut self.inner
        }
    }

    fn add_degenerate_line(&mut self, q: &QuadConstruct) {
        let p = q.quad[2];
        self.path().line_to(p.x, p.y);
    }

    fn cubic_mid_on_line(&self, cubic: &[Point; 4], q: &QuadConstruct) -> bool {
        let (_, stroke_mid, _) = self.cubic_perp_ray(cubic, q.mid_t);
        pt_to_line(stroke_mid, q.quad[0], q.quad[2]) < self.inv_res_scale_squared
    }

    fn cubic_stroke(&mut self, cubic: &[Point; 4], q: &mut QuadConstruct) -> bool {
        if !self.found_tangents {
            self.cubic_quad_ends(cubic, q);
            let r = self.intersect_ray(q, false);
            if r != ResultType::Quad {
                if (r == ResultType::Degenerate
                    || points_within_dist(q.quad[0], q.quad[2], self.inv_res_scale))
                    && self.cubic_mid_on_line(cubic, q)
                {
                    self.add_degenerate_line(q);
                    return true;
                }
            } else {
                self.found_tangents = true;
            }
        }
        if self.found_tangents {
            let r = self.compare_quad_cubic(cubic, q);
            if r == ResultType::Quad {
                let s = q.quad;
                self.path().quad_to(s[1].x, s[1].y, s[2].x, s[2].y);
                return true;
            }
            if r == ResultType::Degenerate && !q.opposite_tangents {
                self.add_degenerate_line(q);
                return true;
            }
        }
        if !q.quad[2].x.is_finite() || !q.quad[2].y.is_finite() {
            return false;
        }
        self.recursion_depth += 1;
        if self.recursion_depth > RECURSIVE_LIMITS[self.found_tangents as usize] {
            return false;
        }
        let mut half = QuadConstruct::default();
        if !half.init_with_start(q) {
            self.add_degenerate_line(q);
            self.recursion_depth -= 1;
            return true;
        }
        if !self.cubic_stroke(cubic, &mut half) {
            return false;
        }
        if !half.init_with_end(q) {
            self.add_degenerate_line(q);
            self.recursion_depth -= 1;
            return true;
        }
        if !self.cubic_stroke(cubic, &mut half) {
            return false;
        }
        self.recursion_depth -= 1;
        true
    }

    fn conic_stroke(&mut self, conic: &Conic, q: &mut QuadConstruct) -> bool {
        let r = self.compare_quad_conic(conic, q);
        if r == ResultType::Quad {
            let s = q.quad;
            self.path().quad_to(s[1].x, s[1].y, s[2].x, s[2].y);
            return true;
        }
        if r == ResultType::Degenerate {
            self.add_degenerate_line(q);
            return true;
        }
        self.recursion_depth += 1;
        if self.recursion_depth > RECURSIVE_LIMITS[CONIC_LIMIT] {
            return false;
        }
        let mut half = QuadConstruct::default();
        half.init_with_start(q);
        if !self.conic_stroke(conic, &mut half) {
            return false;
        }
        half.init_with_end(q);
        if !self.conic_stroke(conic, &mut half) {
            return false;
        }
        self.recursion_depth -= 1;
        true
    }

    fn quad_stroke(&mut self, quad: &[Point; 3], q: &mut QuadConstruct) -> bool {
        let r = self.compare_quad_quad(quad, q);
        if r == ResultType::Quad {
            let s = q.quad;
            self.path().quad_to(s[1].x, s[1].y, s[2].x, s[2].y);
            return true;
        }
        if r == ResultType::Degenerate {
            self.add_degenerate_line(q);
            return true;
        }
        self.recursion_depth += 1;
        if self.recursion_depth > RECURSIVE_LIMITS[QUAD_LIMIT] {
            return false;
        }
        let mut half = QuadConstruct::default();
        half.init_with_start(q);
        if !self.quad_stroke(quad, &mut half) {
            return false;
        }
        half.init_with_end(q);
        if !self.quad_stroke(quad, &mut half) {
            return false;
        }
        self.recursion_depth -= 1;
        true
    }

    fn with_round_join(&mut self, f: impl FnOnce(&mut Self)) {
        let save = self.join;
        self.join = Join::Round;
        f(self);
        self.join = save;
    }

    fn conic_to(&mut self, pt1: Point, pt2: Point, weight: f32) {
        let conic = Conic::new([self.prev_pt, pt1, pt2], weight);
        let (rt, reduction) = check_conic_linear(&conic);
        if rt == ReductionType::Point || rt == ReductionType::Line {
            self.line_to(pt2, None);
            return;
        }
        if rt == ReductionType::Degenerate {
            self.line_to(reduction, None);
            self.with_round_join(|s| s.line_to(pt2, None));
            return;
        }
        let Some((normal_ab, unit_ab)) = self.pre_join_to(pt1, false) else {
            self.line_to(pt2, None);
            return;
        };
        let mut q = QuadConstruct::default();
        self.init(true, &mut q, 0.0, 1.0);
        let _ = self.conic_stroke(&conic, &mut q);
        self.init(false, &mut q, 0.0, 1.0);
        let _ = self.conic_stroke(&conic, &mut q);
        let (normal_bc, unit_bc) = self.set_quad_end_normal(&conic.pts, normal_ab, unit_ab);
        self.post_join_to(pt2, normal_bc, unit_bc);
    }

    fn quad_to(&mut self, pt1: Point, pt2: Point) {
        let quad = [self.prev_pt, pt1, pt2];
        let (rt, reduction) = check_quad_linear(&quad);
        if rt == ReductionType::Point || rt == ReductionType::Line {
            self.line_to(pt2, None);
            return;
        }
        if rt == ReductionType::Degenerate {
            self.line_to(reduction, None);
            self.with_round_join(|s| s.line_to(pt2, None));
            return;
        }
        let Some((normal_ab, unit_ab)) = self.pre_join_to(pt1, false) else {
            self.line_to(pt2, None);
            return;
        };
        let mut q = QuadConstruct::default();
        self.init(true, &mut q, 0.0, 1.0);
        let _ = self.quad_stroke(&quad, &mut q);
        self.init(false, &mut q, 0.0, 1.0);
        let _ = self.quad_stroke(&quad, &mut q);
        let (normal_bc, unit_bc) = self.set_quad_end_normal(&quad, normal_ab, unit_ab);
        self.post_join_to(pt2, normal_bc, unit_bc);
    }

    fn cubic_to(&mut self, pt1: Point, pt2: Point, pt3: Point) {
        let cubic = [self.prev_pt, pt1, pt2, pt3];
        let (rt, reduction, tangent_idx) = check_cubic_linear(&cubic);
        if rt == ReductionType::Point || rt == ReductionType::Line {
            self.line_to(pt3, None);
            return;
        }
        if rt >= ReductionType::Degenerate {
            self.line_to(reduction[0], None);
            self.with_round_join(|s| {
                if rt >= ReductionType::Degenerate2 {
                    s.line_to(reduction[1], None);
                }
                if rt == ReductionType::Degenerate3 {
                    s.line_to(reduction[2], None);
                }
                s.line_to(pt3, None);
            });
            return;
        }
        let Some((normal_ab, unit_ab)) = self.pre_join_to(cubic[tangent_idx], false) else {
            self.line_to(pt3, None);
            return;
        };
        let mut t_values = [0.0f32; 2];
        let count = find_cubic_inflections(&cubic, &mut t_values);
        let mut last_t = 0.0;
        for index in 0..=count {
            let next_t = if index < count { t_values[index] } else { 1.0 };
            let mut q = QuadConstruct::default();
            self.init(true, &mut q, last_t, next_t);
            let _ = self.cubic_stroke(&cubic, &mut q);
            self.init(false, &mut q, last_t, next_t);
            let _ = self.cubic_stroke(&cubic, &mut q);
            last_t = next_t;
        }
        let cusp = find_cubic_cusp(&cubic);
        if cusp > 0.0 {
            let loc = cubic_eval(&cubic, cusp);
            self.cusper.add_circle(loc.x, loc.y, self.radius);
        }
        let (normal_cd, unit_cd) = self.set_cubic_end_normal(&cubic, normal_ab, unit_ab);
        self.post_join_to(pt3, normal_cd, unit_cd);
    }

    fn done(mut self, is_line: bool) -> Path {
        self.finish_contour(false, is_line);
        self.outer
    }
}

/// One step of `SkPath::Iter` (with forceClose false).
#[derive(Clone, Copy)]
struct IterVerb {
    verb: Verb,
    pts: [Point; 4],
    w: f32,
}

fn iter_path(path: &Path) -> Vec<IterVerb> {
    let mut out = Vec::new();
    let mut pi = 0usize;
    let mut wi = 0usize;
    let mut move_to = Point::default();
    let mut last_pt = Point::default();
    let n = path.verbs.len();
    let mut vi = 0;
    while vi < n {
        let v = path.verbs[vi];
        vi += 1;
        let mut pts = [Point::default(); 4];
        match v {
            Verb::Move => {
                if vi == n {
                    break; // a trailing moveTo
                }
                move_to = path.points[pi];
                pts[0] = move_to;
                pi += 1;
                last_pt = move_to;
                out.push(IterVerb {
                    verb: Verb::Move,
                    pts,
                    w: 1.0,
                });
            }
            Verb::Line => {
                pts[0] = last_pt;
                pts[1] = path.points[pi];
                last_pt = pts[1];
                pi += 1;
                out.push(IterVerb {
                    verb: Verb::Line,
                    pts,
                    w: 1.0,
                });
            }
            Verb::Quad | Verb::Conic => {
                pts[0] = last_pt;
                pts[1] = path.points[pi];
                pts[2] = path.points[pi + 1];
                last_pt = pts[2];
                pi += 2;
                let w = if v == Verb::Conic {
                    wi += 1;
                    path.weights[wi - 1]
                } else {
                    1.0
                };
                out.push(IterVerb { verb: v, pts, w });
            }
            Verb::Cubic => {
                pts[0] = last_pt;
                pts[1] = path.points[pi];
                pts[2] = path.points[pi + 1];
                pts[3] = path.points[pi + 2];
                last_pt = pts[3];
                pi += 3;
                out.push(IterVerb {
                    verb: Verb::Cubic,
                    pts,
                    w: 1.0,
                });
            }
            Verb::Close => {
                if last_pt != move_to
                    && !(last_pt.x.is_nan()
                        || last_pt.y.is_nan()
                        || move_to.x.is_nan()
                        || move_to.y.is_nan())
                {
                    pts[0] = last_pt;
                    pts[1] = move_to;
                    out.push(IterVerb {
                        verb: Verb::Line,
                        pts,
                        w: 1.0,
                    });
                }
                pts[0] = move_to;
                last_pt = move_to;
                out.push(IterVerb {
                    verb: Verb::Close,
                    pts,
                    w: 1.0,
                });
            }
        }
    }
    out
}

fn has_valid_tangent(rest: &[IterVerb]) -> bool {
    for v in rest {
        let p = &v.pts;
        match v.verb {
            Verb::Move => return false,
            Verb::Line => {
                if p[0] == p[1] {
                    continue;
                }
                return true;
            }
            Verb::Quad | Verb::Conic => {
                if p[0] == p[1] && p[0] == p[2] {
                    continue;
                }
                return true;
            }
            Verb::Cubic => {
                if p[0] == p[1] && p[0] == p[2] && p[0] == p[3] {
                    continue;
                }
                return true;
            }
            Verb::Close => return false,
        }
    }
    false
}

/// `SkStroke::strokePath` for a stroke style (not stroke-and-fill), with
/// `res_scale` from the CTM (1 without scaling).
pub fn stroke_path(
    src: &Path,
    width: f32,
    miter_limit: f32,
    cap_: Cap,
    join_: Join,
    res_scale: f32,
) -> Path {
    let radius = width * 0.5;
    if radius <= 0.0 {
        return Path::new();
    }

    if let Some((rect, closed, dir)) = src.is_rect_full()
        && closed
    {
        return stroke_rect(rect, dir, width, miter_limit, join_);
    }

    let mut stroker = PathStroker::new(src, radius, miter_limit, cap_, join_, res_scale, false);
    let verbs = iter_path(src);
    let mut last_segment = Verb::Move;
    let mut i = 0;
    while i < verbs.len() {
        let v = verbs[i];
        i += 1;
        match v.verb {
            Verb::Move => stroker.move_to(v.pts[0]),
            Verb::Line => {
                let rest = &verbs[i..];
                stroker.line_to(v.pts[1], Some(&|| has_valid_tangent(rest)));
                last_segment = Verb::Line;
            }
            Verb::Quad => {
                stroker.quad_to(v.pts[1], v.pts[2]);
                last_segment = Verb::Quad;
            }
            Verb::Conic => {
                stroker.conic_to(v.pts[1], v.pts[2], v.w);
                last_segment = Verb::Conic;
            }
            Verb::Cubic => {
                stroker.cubic_to(v.pts[1], v.pts[2], v.pts[3]);
                last_segment = Verb::Cubic;
            }
            Verb::Close => {
                if cap_ != Cap::Butt {
                    if stroker.has_only_move_to() {
                        let p = stroker.first_pt;
                        stroker.line_to(p, None);
                        last_segment = Verb::Line;
                        continue;
                    }
                    if stroker.is_current_contour_empty() {
                        last_segment = Verb::Line;
                        continue;
                    }
                }
                stroker.finish_contour(true, last_segment == Verb::Line);
            }
        }
    }
    stroker.done(last_segment == Verb::Line)
}

/// `SkStroke::strokeRect`.
fn stroke_rect(orig: Rect, mut dir: Direction, width: f32, miter_limit: f32, join_: Join) -> Path {
    let mut dst = Path::new();
    let radius = width * 0.5;
    let mut rw = orig.width();
    let mut rh = orig.height();
    if (rw < 0.0) ^ (rh < 0.0) {
        dir = reverse_dir(dir);
    }
    let rect = Rect::from_ltrb(
        orig.left.min(orig.right),
        orig.top.min(orig.bottom),
        orig.left.max(orig.right),
        orig.top.max(orig.bottom),
    );
    rw = rect.width();
    rh = rect.height();
    let r = Rect::from_ltrb(
        rect.left - radius,
        rect.top - radius,
        rect.right + radius,
        rect.bottom + radius,
    );
    let mut join_ = join_;
    if join_ == Join::Miter && miter_limit < std::f32::consts::SQRT_2 {
        join_ = Join::Bevel;
    }
    match join_ {
        Join::Miter => {
            dst.add_rect(r, dir, 0);
        }
        Join::Bevel => {
            let o = r;
            let pts: [Point; 8] = if dir == Direction::Cw {
                [
                    Point::new(rect.left, o.top),
                    Point::new(rect.right, o.top),
                    Point::new(o.right, rect.top),
                    Point::new(o.right, rect.bottom),
                    Point::new(rect.right, o.bottom),
                    Point::new(rect.left, o.bottom),
                    Point::new(o.left, rect.bottom),
                    Point::new(o.left, rect.top),
                ]
            } else {
                [
                    Point::new(o.left, rect.top),
                    Point::new(o.left, rect.bottom),
                    Point::new(rect.left, o.bottom),
                    Point::new(rect.right, o.bottom),
                    Point::new(o.right, rect.bottom),
                    Point::new(o.right, rect.top),
                    Point::new(rect.right, o.top),
                    Point::new(rect.left, o.top),
                ]
            };
            dst.add_poly(&pts, true);
        }
        Join::Round => {
            dst.add_round_rect(r, radius, radius, dir);
        }
    }
    if width < rw.min(rh) {
        let inner = Rect::from_ltrb(
            rect.left + radius,
            rect.top + radius,
            rect.right - radius,
            rect.bottom - radius,
        );
        dst.add_rect(inner, reverse_dir(dir), 0);
    }
    dst
}

fn reverse_dir(d: Direction) -> Direction {
    match d {
        Direction::Cw => Direction::Ccw,
        Direction::Ccw => Direction::Cw,
    }
}

#[allow(dead_code)]
const _: usize = TANGENT_LIMIT;
