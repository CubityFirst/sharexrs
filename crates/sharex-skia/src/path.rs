//! `SkPath`: verbs, points and conic weights, built as `SkPath` builds them
//! (`moveTo` injection after `close`, `conicTo` with weight 1 becoming a
//! quad, `addRect`/`addOval` point order, the SVG `arcTo`), with the
//! properties the scan converter asks for: bounds, convexity (`SkPath::
//! computeConvexity`) and `isRect`.

use crate::geom::{Point, Rect};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verb {
    Move,
    Line,
    Quad,
    Conic,
    Cubic,
    Close,
}

impl Verb {
    /// Points the verb adds (`SkPathPriv::PtsInVerb`).
    pub fn pts(self) -> usize {
        match self {
            Verb::Move | Verb::Line => 1,
            Verb::Quad | Verb::Conic => 2,
            Verb::Cubic => 3,
            Verb::Close => 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FillType {
    #[default]
    Winding,
    EvenOdd,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Cw,
    Ccw,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FirstDirection {
    Cw,
    Ccw,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Convexity {
    Convex,
    Concave,
}

pub const LINE_SEGMENT_MASK: u8 = 1;
pub const QUAD_SEGMENT_MASK: u8 = 2;
pub const CONIC_SEGMENT_MASK: u8 = 4;
pub const CUBIC_SEGMENT_MASK: u8 = 8;

#[derive(Clone, Debug)]
pub struct Path {
    pub verbs: Vec<Verb>,
    pub points: Vec<Point>,
    pub weights: Vec<f32>,
    pub fill_type: FillType,
    last_move_to_index: i32,
    first_direction: FirstDirection,
    /// A convexity set by whoever built the path (`setConvexity`), else
    /// computed when asked.
    convexity: Option<Convexity>,
    segment_mask: u8,
}

impl Default for Path {
    fn default() -> Self {
        Self::new()
    }
}

impl Path {
    pub fn new() -> Self {
        Self {
            verbs: Vec::new(),
            points: Vec::new(),
            weights: Vec::new(),
            fill_type: FillType::Winding,
            last_move_to_index: !0,
            first_direction: FirstDirection::Unknown,
            convexity: None,
            segment_mask: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.verbs.is_empty()
    }

    pub fn count_points(&self) -> usize {
        self.points.len()
    }

    pub fn segment_mask(&self) -> u8 {
        self.segment_mask
    }

    /// `dirtyAfterEdit`.
    fn dirty(&mut self) {
        self.convexity = None;
        self.first_direction = FirstDirection::Unknown;
    }

    pub fn last_pt(&self) -> Option<Point> {
        self.points.last().copied()
    }

    fn set_last_pt(&mut self, p: Point) {
        if let Some(last) = self.points.last_mut() {
            *last = p;
        } else {
            self.move_to(p.x, p.y);
        }
    }

    fn inject_move_to_if_needed(&mut self) {
        if self.last_move_to_index < 0 {
            let p = if self.verbs.is_empty() {
                Point::new(0.0, 0.0)
            } else {
                self.points[(!self.last_move_to_index) as usize]
            };
            self.move_to(p.x, p.y);
        }
    }

    pub fn move_to(&mut self, x: f32, y: f32) -> &mut Self {
        self.last_move_to_index = self.points.len() as i32;
        self.verbs.push(Verb::Move);
        self.points.push(Point::new(x, y));
        self.dirty();
        self
    }

    pub fn line_to(&mut self, x: f32, y: f32) -> &mut Self {
        self.inject_move_to_if_needed();
        self.verbs.push(Verb::Line);
        self.points.push(Point::new(x, y));
        self.segment_mask |= LINE_SEGMENT_MASK;
        self.dirty();
        self
    }

    pub fn quad_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32) -> &mut Self {
        self.inject_move_to_if_needed();
        self.verbs.push(Verb::Quad);
        self.points.push(Point::new(x1, y1));
        self.points.push(Point::new(x2, y2));
        self.segment_mask |= QUAD_SEGMENT_MASK;
        self.dirty();
        self
    }

    pub fn conic_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, w: f32) -> &mut Self {
        // check for <= 0 or NaN with this test
        if !(w > 0.0) {
            self.line_to(x2, y2);
        } else if !w.is_finite() {
            self.line_to(x1, y1);
            self.line_to(x2, y2);
        } else if w == 1.0 {
            self.quad_to(x1, y1, x2, y2);
        } else {
            self.inject_move_to_if_needed();
            self.verbs.push(Verb::Conic);
            self.points.push(Point::new(x1, y1));
            self.points.push(Point::new(x2, y2));
            self.weights.push(w);
            self.segment_mask |= CONIC_SEGMENT_MASK;
            self.dirty();
        }
        self
    }

    pub fn cubic_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x3: f32, y3: f32) -> &mut Self {
        self.inject_move_to_if_needed();
        self.verbs.push(Verb::Cubic);
        self.points.push(Point::new(x1, y1));
        self.points.push(Point::new(x2, y2));
        self.points.push(Point::new(x3, y3));
        self.segment_mask |= CUBIC_SEGMENT_MASK;
        self.dirty();
        self
    }

    pub fn close(&mut self) -> &mut Self {
        if let Some(&last) = self.verbs.last()
            && last != Verb::Close
        {
            self.verbs.push(Verb::Close);
        }
        // signal that we need a moveTo to follow us (unless we're done)
        self.last_move_to_index ^= !self.last_move_to_index >> 31;
        self
    }

    /// `hasOnlyMoveTos`: no line, quad, conic or cubic (closes don't count).
    fn has_only_move_tos(&self) -> bool {
        !self
            .verbs
            .iter()
            .any(|v| matches!(v, Verb::Line | Verb::Quad | Verb::Conic | Verb::Cubic))
    }

    /// `SkPath::setLastPt`.
    pub fn set_last_pt_pub(&mut self, p: Point) {
        self.set_last_pt(p);
        self.dirty();
    }

    /// `SkPath::isZeroLengthSincePoint`.
    pub fn is_zero_length_since_point(&self, start: usize) -> bool {
        let count = self.points.len() as isize - start as isize;
        if count < 2 {
            return true;
        }
        let first = self.points[start];
        self.points[start + 1..].iter().all(|p| *p == first)
    }

    /// `SkPath::reversePathTo`: appends `path`'s last contour, reversed.
    pub fn reverse_path_to(&mut self, path: &Path) {
        if path.verbs.is_empty() {
            return;
        }
        let mut pi = path.points.len() as isize - 1;
        let mut wi = path.weights.len();
        for &v in path.verbs.iter().rev() {
            pi -= v.pts() as isize;
            let pts = |k: isize| path.points[(pi + k) as usize];
            match v {
                Verb::Move => return,
                Verb::Line => {
                    let p = pts(0);
                    self.line_to(p.x, p.y);
                }
                Verb::Quad => {
                    let (a, b) = (pts(1), pts(0));
                    self.quad_to(a.x, a.y, b.x, b.y);
                }
                Verb::Conic => {
                    wi -= 1;
                    let (a, b) = (pts(1), pts(0));
                    self.conic_to(a.x, a.y, b.x, b.y, path.weights[wi]);
                }
                Verb::Cubic => {
                    let (a, b, c) = (pts(2), pts(1), pts(0));
                    self.cubic_to(a.x, a.y, b.x, b.y, c.x, c.y);
                }
                Verb::Close => {}
            }
        }
    }

    /// `SkPath::addPath` (append mode, identity matrix).
    pub fn add_path(&mut self, src: &Path) {
        if src.verbs.is_empty() {
            return;
        }
        let n = self.points.len() as i32;
        self.last_move_to_index = if src.last_move_to_index >= 0 {
            src.last_move_to_index + n
        } else {
            src.last_move_to_index - n
        };
        self.verbs.extend_from_slice(&src.verbs);
        self.points.extend_from_slice(&src.points);
        self.weights.extend_from_slice(&src.weights);
        self.segment_mask |= src.segment_mask;
        self.dirty();
    }

    /// `SkPath::addCircle`.
    pub fn add_circle(&mut self, x: f32, y: f32, r: f32) -> &mut Self {
        if r > 0.0 {
            self.add_oval(Rect::from_ltrb(x - r, y - r, x + r, y + r), Direction::Cw);
        }
        self
    }

    /// `SkPath::addPoly`.
    pub fn add_poly(&mut self, pts: &[Point], close: bool) -> &mut Self {
        if pts.is_empty() {
            return self;
        }
        self.last_move_to_index = self.points.len() as i32;
        self.verbs.push(Verb::Move);
        self.points.push(pts[0]);
        for p in &pts[1..] {
            self.verbs.push(Verb::Line);
            self.points.push(*p);
            self.segment_mask |= LINE_SEGMENT_MASK;
        }
        if close {
            self.verbs.push(Verb::Close);
            self.last_move_to_index ^= !self.last_move_to_index >> 31;
        }
        self.dirty();
        self
    }

    /// `SkPath::addRoundRect(rect, rx, ry, dir)` (`SkRRect::setRectXY`,
    /// `addRRect` with the legacy start index).
    pub fn add_round_rect(&mut self, rect: Rect, rx: f32, ry: f32, dir: Direction) -> &mut Self {
        if rx < 0.0 || ry < 0.0 {
            return self;
        }
        let (w, h) = (rect.width(), rect.height());
        let (mut rx, mut ry) = (rx, ry);
        if rx <= 0.0 || ry <= 0.0 {
            return self.add_rect(rect, dir, if dir == Direction::Cw { 3 } else { 4 } % 4);
        }
        if rx + rx > w || ry + ry > h {
            let scale = (w / (rx + rx)).min(h / (ry + ry));
            rx *= scale;
            ry *= scale;
        }
        let start: usize = if dir == Direction::Cw { 6 } else { 7 };
        if rx >= w * 0.5 && ry >= h * 0.5 {
            return self.add_oval_start(rect, dir, start / 2);
        }
        let first = if self.has_only_move_tos() {
            dir_to_first(dir)
        } else {
            FirstDirection::Unknown
        };
        let (l, t, r, b) = (rect.left, rect.top, rect.right, rect.bottom);
        let rr = [
            Point::new(l + rx, t),
            Point::new(r - rx, t),
            Point::new(r, t + ry),
            Point::new(r, b - ry),
            Point::new(r - rx, b),
            Point::new(l + rx, b),
            Point::new(l, b - ry),
            Point::new(l, t + ry),
        ];
        let corners = [
            Point::new(l, t),
            Point::new(r, t),
            Point::new(r, b),
            Point::new(l, b),
        ];
        let adv8 = if dir == Direction::Cw { 1 } else { 7 };
        let adv4 = if dir == Direction::Cw { 1 } else { 3 };
        let mut ri = start % 8;
        let mut ci = (start / 2 + if dir == Direction::Cw { 0 } else { 1 }) % 4;
        let starts_with_conic = (start & 1 == 1) == (dir == Direction::Cw);
        let w8 = std::f32::consts::FRAC_1_SQRT_2;
        self.move_to(rr[ri].x, rr[ri].y);
        let next_rr = |ri: &mut usize| {
            *ri = (*ri + adv8) % 8;
            rr[*ri]
        };
        let next_c = |ci: &mut usize| {
            *ci = (*ci + adv4) % 4;
            corners[*ci]
        };
        if starts_with_conic {
            for _ in 0..3 {
                let c = next_c(&mut ci);
                let p = next_rr(&mut ri);
                self.conic_to(c.x, c.y, p.x, p.y, w8);
                let p = next_rr(&mut ri);
                self.line_to(p.x, p.y);
            }
            let c = next_c(&mut ci);
            let p = next_rr(&mut ri);
            self.conic_to(c.x, c.y, p.x, p.y, w8);
        } else {
            for _ in 0..4 {
                let p = next_rr(&mut ri);
                self.line_to(p.x, p.y);
                let c = next_c(&mut ci);
                let p = next_rr(&mut ri);
                self.conic_to(c.x, c.y, p.x, p.y, w8);
            }
        }
        self.close();
        self.first_direction = first;
        self
    }

    /// `SkPath::isRect(rect, isClosed, direction)`.
    pub fn is_rect_full(&self) -> Option<(Rect, bool, Direction)> {
        self.is_rect_impl()
    }

    /// `SkPath::addRect(rect, dir, startIndex)`.
    pub fn add_rect(&mut self, r: Rect, dir: Direction, start: usize) -> &mut Self {
        let first = if self.has_only_move_tos() {
            dir_to_first(dir)
        } else {
            FirstDirection::Unknown
        };
        let pts = [
            Point::new(r.left, r.top),
            Point::new(r.right, r.top),
            Point::new(r.right, r.bottom),
            Point::new(r.left, r.bottom),
        ];
        let adv = if dir == Direction::Cw { 1 } else { 3 };
        let mut i = start % 4;
        self.move_to(pts[i].x, pts[i].y);
        for _ in 0..3 {
            i = (i + adv) % 4;
            self.line_to(pts[i].x, pts[i].y);
        }
        self.close();
        self.first_direction = first;
        self
    }

    /// `SkPath::addOval(oval, dir)` (start index 1, as Skia's legacy default).
    pub fn add_oval(&mut self, oval: Rect, dir: Direction) -> &mut Self {
        self.add_oval_start(oval, dir, 1)
    }

    pub fn add_oval_start(&mut self, oval: Rect, dir: Direction, start: usize) -> &mut Self {
        let is_oval = self.has_only_move_tos();
        let first = if is_oval {
            dir_to_first(dir)
        } else {
            FirstDirection::Unknown
        };
        let cx = midpoint(oval.left, oval.right);
        let cy = midpoint(oval.top, oval.bottom);
        let oval_pts = [
            Point::new(cx, oval.top),
            Point::new(oval.right, cy),
            Point::new(cx, oval.bottom),
            Point::new(oval.left, cy),
        ];
        let rect_pts = [
            Point::new(oval.left, oval.top),
            Point::new(oval.right, oval.top),
            Point::new(oval.right, oval.bottom),
            Point::new(oval.left, oval.bottom),
        ];
        let adv = if dir == Direction::Cw { 1 } else { 3 };
        let mut oi = start % 4;
        let mut ri = (start + if dir == Direction::Cw { 0 } else { 1 }) % 4;
        let w = std::f32::consts::FRAC_1_SQRT_2;
        self.move_to(oval_pts[oi].x, oval_pts[oi].y);
        for _ in 0..4 {
            ri = (ri + adv) % 4;
            oi = (oi + adv) % 4;
            self.conic_to(
                rect_pts[ri].x,
                rect_pts[ri].y,
                oval_pts[oi].x,
                oval_pts[oi].y,
                w,
            );
        }
        self.close();
        self.first_direction = first;
        self
    }

    /// `SkPath::arcTo(rx, ry, angle, arcLarge, arcSweep, x, y)`: the SVG arc.
    #[allow(clippy::too_many_arguments)]
    pub fn arc_to(
        &mut self,
        rx: f32,
        ry: f32,
        angle: f32,
        large: bool,
        sweep: Direction,
        x: f32,
        y: f32,
    ) -> &mut Self {
        self.inject_move_to_if_needed();
        let src0 = self.last_pt().unwrap_or_default();
        if rx == 0.0 || ry == 0.0 {
            return self.line_to(x, y);
        }
        let src1 = Point::new(x, y);
        if src0 == src1 {
            return self.line_to(x, y);
        }
        let mut rx = rx.abs();
        let mut ry = ry.abs();
        let mid = (src0 - src1) * 0.5;

        let rot = Matrix::rotate(-angle);
        let tmid = rot.map(mid);
        let square_rx = rx * rx;
        let square_ry = ry * ry;
        let square_x = tmid.x * tmid.x;
        let square_y = tmid.y * tmid.y;

        let mut radii_scale = square_x / square_rx + square_y / square_ry;
        if radii_scale > 1.0 {
            radii_scale = radii_scale.sqrt();
            rx *= radii_scale;
            ry *= radii_scale;
        }

        let point_transform = Matrix::scale(1.0 / rx, 1.0 / ry).pre_concat(&Matrix::rotate(-angle));
        let mut unit0 = point_transform.map(src0);
        let mut unit1 = point_transform.map(src1);
        let mut delta = unit1 - unit0;

        let d = delta.x * delta.x + delta.y * delta.y;
        let scale_factor_squared = (1.0 / d - 0.25).max(0.0);
        let mut scale_factor = scale_factor_squared.sqrt();
        if (sweep == Direction::Ccw) != large {
            scale_factor = -scale_factor;
        }
        delta = delta * scale_factor;
        let mut center = unit0 + unit1;
        center = center * 0.5;
        center = Point::new(center.x - delta.y, center.y + delta.x);
        unit0 = unit0 - center;
        unit1 = unit1 - center;
        let theta1 = unit0.y.atan2(unit0.x);
        let theta2 = unit1.y.atan2(unit1.x);
        let mut theta_arc = theta2 - theta1;
        let pi = std::f32::consts::PI;
        if theta_arc < 0.0 && sweep == Direction::Cw {
            theta_arc += pi * 2.0;
        } else if theta_arc > 0.0 && sweep != Direction::Cw {
            theta_arc -= pi * 2.0;
        }

        if theta_arc.abs() < pi / (1000.0 * 1000.0) {
            return self.line_to(x, y);
        }

        let point_transform = Matrix::rotate(angle).pre_concat(&Matrix::scale(rx, ry));

        let segments = crate::geom::sat_ceil((theta_arc / (2.0 * pi / 3.0)).abs());
        let theta_width = theta_arc / segments as f32;
        let t = (0.5 * theta_width).tan();
        if !t.is_finite() {
            return self;
        }
        let mut start_theta = theta1;
        let w = (0.5 + theta_width.cos() * 0.5).sqrt();
        let is_int = |v: f32| v == v.floor();
        let expect_integers = nearly_zero(pi / 2.0 - theta_width.abs())
            && is_int(rx)
            && is_int(ry)
            && is_int(x)
            && is_int(y);

        for _ in 0..segments {
            let end_theta = start_theta + theta_width;
            let sin_end = sin_snap_to_zero(end_theta);
            let cos_end = cos_snap_to_zero(end_theta);
            let u1 = Point::new(cos_end, sin_end) + center;
            let u0 = Point::new(u1.x + t * sin_end, u1.y - t * cos_end);
            let mut m0 = point_transform.map(u0);
            let mut m1 = point_transform.map(u1);
            if expect_integers {
                m0 = Point::new(round_to_scalar(m0.x), round_to_scalar(m0.y));
                m1 = Point::new(round_to_scalar(m1.x), round_to_scalar(m1.y));
            }
            self.conic_to(m0.x, m0.y, m1.x, m1.y, w);
            start_theta = end_theta;
        }
        // The final point should match the input point (by definition); replace it to
        // ensure that rounding errors in the above math don't cause any problems.
        self.set_last_pt(Point::new(x, y));
        self
    }

    /// `SkPath::getBounds`: the bounds of every point, control points too.
    pub fn bounds(&self) -> Rect {
        Rect::bounds_of(&self.points).0
    }

    pub fn is_finite(&self) -> bool {
        Rect::bounds_of(&self.points).1
    }

    /// `SkPath::transform` for a translation.
    pub fn translated(&self, dx: f32, dy: f32) -> Path {
        let mut p = self.clone();
        if dx != 0.0 || dy != 0.0 {
            for pt in &mut p.points {
                pt.x += dx;
                pt.y += dy;
            }
        }
        p
    }

    /// `SkPath::transform` for an affine matrix (no perspective). Convexity
    /// and direction are recomputed, as Skia does for anything but a
    /// scale/translate.
    pub fn transformed(&self, m: &Matrix) -> Path {
        if m.is_translate() {
            return self.translated(m.tx, m.ty);
        }
        let mut p = self.clone();
        for pt in &mut p.points {
            *pt = m.map(*pt);
        }
        p.convexity = None;
        p.first_direction = FirstDirection::Unknown;
        p
    }

    pub fn set_convexity(&mut self, c: Option<Convexity>) {
        self.convexity = c;
    }

    pub fn first_direction(&self) -> FirstDirection {
        self.first_direction
    }

    pub fn is_convex(&self) -> bool {
        self.convexity() == Convexity::Convex
    }

    pub fn convexity(&self) -> Convexity {
        if let Some(c) = self.convexity {
            return c;
        }
        self.compute_convexity()
    }

    /// `SkPath::computeConvexity`.
    fn compute_convexity(&self) -> Convexity {
        if !self.is_finite() {
            return Convexity::Concave;
        }
        let mut point_count = self.points.len() as i32;
        let leading_moves = self.verbs.iter().take_while(|v| **v == Verb::Move).count() as i32;
        let skip_count = leading_moves - 1;

        if self.last_move_to_index >= 0 {
            if self.last_move_to_index == point_count - 1 {
                let mut vi = self.verbs.len() - 1;
                while vi > 0 && self.verbs[vi] == Verb::Move {
                    vi -= 1;
                    point_count -= 1;
                }
            } else if self.last_move_to_index != skip_count {
                return Convexity::Concave;
            }
        }
        let mut points: &[Point] = &self.points;
        if skip_count > 0 {
            points = &points[skip_count as usize..];
            point_count -= skip_count;
        }
        let points = &points[..point_count.max(0) as usize];

        if !convexicator_by_sign(points) {
            return Convexity::Concave;
        }

        let mut contour_count = 0;
        let mut needs_close = false;
        let mut state = Convexicator::default();

        for (verb, pts, _) in self.iter_verbs() {
            if contour_count == 0 {
                if verb == Verb::Move {
                    state.set_move_pt(pts[0]);
                } else {
                    contour_count += 1;
                    needs_close = true;
                }
            }
            if contour_count == 1 {
                if verb == Verb::Close || verb == Verb::Move {
                    if !state.close() {
                        return Convexity::Concave;
                    }
                    needs_close = false;
                    contour_count += 1;
                } else {
                    for p in &pts[1..=verb.pts()] {
                        if !state.add_pt(*p) {
                            return Convexity::Concave;
                        }
                    }
                }
            } else if contour_count > 1 && verb != Verb::Move {
                return Convexity::Concave;
            }
        }

        if needs_close && !state.close() {
            return Convexity::Concave;
        }

        if self.first_direction == FirstDirection::Unknown
            && state.first_direction == FirstDirection::Unknown
            && !self.bounds().is_empty()
        {
            return if state.reversals < 3 {
                Convexity::Convex
            } else {
                Convexity::Concave
            };
        }
        Convexity::Convex
    }

    /// `SkPathPriv::Iterate`: each verb with its points, the previous
    /// point first (`pts[0]`), as `SkPath::RawIter`'s points.
    pub fn iter_verbs(&self) -> Vec<(Verb, Vec<Point>, f32)> {
        let mut out = Vec::with_capacity(self.verbs.len());
        let mut pi = 0usize;
        let mut wi = 0usize;
        let mut last_move = Point::default();
        for &v in &self.verbs {
            match v {
                Verb::Move => {
                    last_move = self.points[pi];
                    out.push((v, vec![self.points[pi]], 1.0));
                    pi += 1;
                }
                Verb::Close => {
                    out.push((v, vec![last_move], 1.0));
                }
                _ => {
                    let n = v.pts();
                    let mut pts = Vec::with_capacity(n + 1);
                    pts.push(self.points[pi - 1]);
                    pts.extend_from_slice(&self.points[pi..pi + n]);
                    let w = if v == Verb::Conic {
                        wi += 1;
                        self.weights[wi - 1]
                    } else {
                        1.0
                    };
                    out.push((v, pts, w));
                    pi += n;
                }
            }
        }
        out
    }

    /// `SkPathEdgeIter`: every segment, with the implicit closing line of
    /// each contour. Conics carry their weight.
    pub fn edges(&self) -> Vec<Edge> {
        let mut out = Vec::new();
        let mut pi = 0usize;
        let mut wi = 0usize;
        let mut move_pt = 0usize;
        let mut needs_close_line = false;
        let close_line = |out: &mut Vec<Edge>, last: Point, mv: Point| {
            out.push(Edge::Line([last, mv]));
        };
        for &v in &self.verbs {
            match v {
                Verb::Move => {
                    if needs_close_line {
                        close_line(&mut out, self.points[pi - 1], self.points[move_pt]);
                        needs_close_line = false;
                    }
                    move_pt = pi;
                    pi += 1;
                }
                Verb::Close => {
                    if needs_close_line {
                        close_line(&mut out, self.points[pi - 1], self.points[move_pt]);
                        needs_close_line = false;
                    }
                }
                Verb::Line => {
                    needs_close_line = true;
                    out.push(Edge::Line([self.points[pi - 1], self.points[pi]]));
                    pi += 1;
                }
                Verb::Quad => {
                    needs_close_line = true;
                    out.push(Edge::Quad([
                        self.points[pi - 1],
                        self.points[pi],
                        self.points[pi + 1],
                    ]));
                    pi += 2;
                }
                Verb::Conic => {
                    needs_close_line = true;
                    out.push(Edge::Conic(
                        [self.points[pi - 1], self.points[pi], self.points[pi + 1]],
                        self.weights[wi],
                    ));
                    wi += 1;
                    pi += 2;
                }
                Verb::Cubic => {
                    needs_close_line = true;
                    out.push(Edge::Cubic([
                        self.points[pi - 1],
                        self.points[pi],
                        self.points[pi + 1],
                        self.points[pi + 2],
                    ]));
                    pi += 3;
                }
            }
        }
        if needs_close_line {
            close_line(&mut out, self.points[pi - 1], self.points[move_pt]);
        }
        out
    }

    /// `SkPath::isRect` (`SkPathPriv::IsRectContour` with allowPartial false).
    pub fn is_rect(&self) -> Option<Rect> {
        self.is_rect_impl().map(|r| r.0)
    }

    fn is_rect_impl(&self) -> Option<(Rect, bool, Direction)> {
        let mut corners = 0;
        let mut line_start = Point::new(0.0, 0.0);
        let mut first_pt: Option<Point> = None;
        let mut last_pt: Option<Point> = None;
        let mut first_corner = Point::default();
        let mut third_corner = Point::default();
        let mut directions = [-1i32; 5];
        let mut closed_or_moved = false;
        let mut auto_close = false;
        let mut pi = 0usize;
        for &verb in &self.verbs {
            match verb {
                Verb::Close | Verb::Line => {
                    if verb == Verb::Close {
                        auto_close = true;
                    } else {
                        last_pt = Some(self.points[pi]);
                    }
                    let line_end = if verb == Verb::Close {
                        first_pt?
                    } else {
                        let p = self.points[pi];
                        pi += 1;
                        p
                    };
                    let d = line_end - line_start;
                    if d.x != 0.0 && d.y != 0.0 {
                        return None;
                    }
                    if !d.is_finite() {
                        return None;
                    }
                    if line_start == line_end {
                        continue;
                    }
                    let next_direction =
                        ((d.x != 0.0) as i32) | (((d.x > 0.0 || d.y > 0.0) as i32) << 1);
                    if corners == 0 {
                        directions[0] = next_direction;
                        corners = 1;
                        closed_or_moved = false;
                        line_start = line_end;
                        continue;
                    }
                    if closed_or_moved {
                        return None;
                    }
                    if auto_close && next_direction == directions[0] {
                        continue;
                    }
                    closed_or_moved = auto_close;
                    if directions[corners - 1] == next_direction {
                        if corners == 3 && verb == Verb::Line {
                            third_corner = line_end;
                        }
                        line_start = line_end;
                        continue;
                    }
                    directions[corners] = next_direction;
                    corners += 1;
                    match corners {
                        2 => first_corner = line_start,
                        3 => {
                            if (directions[0] ^ directions[2]) != 2 {
                                return None;
                            }
                            third_corner = line_end;
                        }
                        4 => {
                            if (directions[1] ^ directions[3]) != 2 {
                                return None;
                            }
                        }
                        _ => return None,
                    }
                    line_start = line_end;
                }
                Verb::Quad | Verb::Conic | Verb::Cubic => return None,
                Verb::Move => {
                    if corners == 0 {
                        first_pt = Some(self.points[pi]);
                    } else {
                        let c = first_pt? - last_pt?;
                        if c.x != 0.0 && c.y != 0.0 {
                            return None;
                        }
                    }
                    line_start = self.points[pi];
                    pi += 1;
                    closed_or_moved = true;
                }
            }
        }
        if !(3..=4).contains(&corners) {
            return None;
        }
        let c = first_pt? - last_pt?;
        if c.x != 0.0 && c.y != 0.0 {
            return None;
        }
        let dir = if directions[0] == ((directions[1] + 1) & 3) {
            Direction::Cw
        } else {
            Direction::Ccw
        };
        Some((
            Rect::from_ltrb(
                first_corner.x.min(third_corner.x),
                first_corner.y.min(third_corner.y),
                first_corner.x.max(third_corner.x),
                first_corner.y.max(third_corner.y),
            ),
            auto_close,
            dir,
        ))
    }

    /// The path for tiny-skia (conics as quads, as Skia's edge builder
    /// approximates them), for the code that still goes through tiny-skia.
    pub fn to_tiny_skia(&self) -> Option<tiny_skia::Path> {
        let mut pb = tiny_skia::PathBuilder::new();
        for (verb, pts, w) in self.iter_verbs() {
            match verb {
                Verb::Move => pb.move_to(pts[0].x, pts[0].y),
                Verb::Line => pb.line_to(pts[1].x, pts[1].y),
                Verb::Quad => pb.quad_to(pts[1].x, pts[1].y, pts[2].x, pts[2].y),
                Verb::Cubic => {
                    pb.cubic_to(pts[1].x, pts[1].y, pts[2].x, pts[2].y, pts[3].x, pts[3].y)
                }
                Verb::Conic => {
                    let q = crate::geom::Conic::new([pts[0], pts[1], pts[2]], w).to_quads(0.25);
                    for i in 0..(q.len() - 1) / 2 {
                        pb.quad_to(
                            q[1 + 2 * i].x,
                            q[1 + 2 * i].y,
                            q[2 + 2 * i].x,
                            q[2 + 2 * i].y,
                        );
                    }
                }
                Verb::Close => pb.close(),
            }
        }
        pb.finish()
    }
}

/// An edge of [`Path::edges`].
#[derive(Clone, Copy, Debug)]
pub enum Edge {
    Line([Point; 2]),
    Quad([Point; 3]),
    Conic([Point; 3], f32),
    Cubic([Point; 4]),
}

fn dir_to_first(d: Direction) -> FirstDirection {
    match d {
        Direction::Cw => FirstDirection::Cw,
        Direction::Ccw => FirstDirection::Ccw,
    }
}

/// `sk_float_midpoint`.
pub fn midpoint(a: f32, b: f32) -> f32 {
    (0.5 * (a as f64 + b as f64)) as f32
}

fn nearly_zero(x: f32) -> bool {
    x.abs() <= 1.0 / 4096.0
}

fn sin_snap_to_zero(r: f32) -> f32 {
    let v = r.sin();
    if nearly_zero(v) { 0.0 } else { v }
}

fn cos_snap_to_zero(r: f32) -> f32 {
    let v = r.cos();
    if nearly_zero(v) { 0.0 } else { v }
}

/// `SkScalarRoundToScalar`: `sk_float_floor(x + 0.5f)`.
fn round_to_scalar(x: f32) -> f32 {
    (x + 0.5).floor()
}

/// `SkMatrix` without perspective, with Skia's type classification so
/// points map with the same float operations.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Matrix {
    pub sx: f32,
    pub kx: f32,
    pub tx: f32,
    pub ky: f32,
    pub sy: f32,
    pub ty: f32,
}

impl Matrix {
    pub const IDENTITY: Matrix = Matrix {
        sx: 1.0,
        kx: 0.0,
        tx: 0.0,
        ky: 0.0,
        sy: 1.0,
        ty: 0.0,
    };

    pub fn translate(tx: f32, ty: f32) -> Self {
        Matrix {
            tx,
            ty,
            ..Self::IDENTITY
        }
    }

    pub fn scale(sx: f32, sy: f32) -> Self {
        Matrix {
            sx,
            sy,
            ..Self::IDENTITY
        }
    }

    /// `SkMatrix::setRotate(degrees)`.
    pub fn rotate(degrees: f32) -> Self {
        let rad = degrees * (std::f32::consts::PI / 180.0);
        let s = sin_snap_to_zero(rad);
        let c = cos_snap_to_zero(rad);
        Matrix {
            sx: c,
            kx: -s,
            tx: 0.0,
            ky: s,
            sy: c,
            ty: 0.0,
        }
    }

    pub fn is_translate(&self) -> bool {
        self.sx == 1.0 && self.sy == 1.0 && self.kx == 0.0 && self.ky == 0.0
    }

    fn is_scale_translate(&self) -> bool {
        self.kx == 0.0 && self.ky == 0.0
    }

    /// `this * o` (`SkMatrix::preConcat`).
    pub fn pre_concat(&self, o: &Matrix) -> Matrix {
        let a = self;
        let b = o;
        if a == &Self::IDENTITY {
            return *b;
        }
        if b == &Self::IDENTITY {
            return *a;
        }
        if a.is_scale_translate() && b.is_scale_translate() {
            return Matrix {
                sx: a.sx * b.sx,
                kx: 0.0,
                tx: a.sx * b.tx + a.tx,
                ky: 0.0,
                sy: a.sy * b.sy,
                ty: a.sy * b.ty + a.ty,
            };
        }
        // muladdmul(a, b, c, d) = a * b + c * d in double precision.
        let mad =
            |p: f32, q: f32, r: f32, s: f32| (p as f64 * q as f64 + r as f64 * s as f64) as f32;
        Matrix {
            sx: mad(a.sx, b.sx, a.kx, b.ky),
            kx: mad(a.sx, b.kx, a.kx, b.sy),
            tx: mad(a.sx, b.tx, a.kx, b.ty) + a.tx,
            ky: mad(a.ky, b.sx, a.sy, b.ky),
            sy: mad(a.ky, b.kx, a.sy, b.sy),
            ty: mad(a.ky, b.tx, a.sy, b.ty) + a.ty,
        }
    }

    /// `SkMatrix::mapPoints`, with the proc Skia picks for the matrix type.
    pub fn map(&self, p: Point) -> Point {
        if self.is_translate() {
            Point::new(p.x + self.tx, p.y + self.ty)
        } else if self.is_scale_translate() {
            Point::new(p.x * self.sx + self.tx, p.y * self.sy + self.ty)
        } else {
            Point::new(
                p.x * self.sx + p.y * self.kx + self.tx,
                p.y * self.sy + p.x * self.ky + self.ty,
            )
        }
    }
}

// --- Convexicator (SkPath.cpp) ---------------------------------------------

fn sign(x: f32) -> i32 {
    (x < 0.0) as i32
}

fn convexicator_by_sign(points: &[Point]) -> bool {
    if points.len() <= 3 {
        return true;
    }
    let mut idx = 0usize;
    let mut curr = points[idx];
    idx += 1;
    let first = curr;
    let mut dxes = 0;
    let mut dyes = 0;
    let mut last_sx = 2;
    let mut last_sy = 2;
    for outer in 0..2 {
        loop {
            let p = if outer == 0 {
                if idx >= points.len() {
                    break;
                }
                points[idx]
            } else {
                first
            };
            let vec = p - curr;
            if !vec.is_zero() {
                if !vec.is_finite() {
                    return false;
                }
                let sx = sign(vec.x);
                let sy = sign(vec.y);
                dxes += (sx != last_sx) as i32;
                dyes += (sy != last_sy) as i32;
                if dxes > 3 || dyes > 3 {
                    return false;
                }
                last_sx = sx;
                last_sy = sy;
            }
            curr = p;
            idx += 1;
            if outer == 1 {
                break;
            }
        }
    }
    true
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DirChange {
    Unknown,
    Left,
    Right,
    Straight,
    Backwards,
    Invalid,
}

struct Convexicator {
    first_pt: Point,
    first_vec: Point,
    last_pt: Point,
    last_vec: Point,
    expected_dir: DirChange,
    first_direction: FirstDirection,
    reversals: i32,
}

impl Default for Convexicator {
    fn default() -> Self {
        Self {
            first_pt: Point::default(),
            first_vec: Point::default(),
            last_pt: Point::default(),
            last_vec: Point::default(),
            expected_dir: DirChange::Invalid,
            first_direction: FirstDirection::Unknown,
            reversals: 0,
        }
    }
}

impl Convexicator {
    fn set_move_pt(&mut self, p: Point) {
        self.first_pt = p;
        self.last_pt = p;
        self.expected_dir = DirChange::Invalid;
    }

    fn add_pt(&mut self, p: Point) -> bool {
        if self.last_pt == p {
            return true;
        }
        if self.first_pt == self.last_pt && self.expected_dir == DirChange::Invalid {
            self.last_vec = p - self.last_pt;
            self.first_vec = self.last_vec;
        } else if !self.add_vec(p - self.last_pt) {
            return false;
        }
        self.last_pt = p;
        true
    }

    fn close(&mut self) -> bool {
        let first = self.first_pt;
        let first_vec = self.first_vec;
        self.add_pt(first) && self.add_vec(first_vec)
    }

    fn direction_change(&self, cur: Point) -> DirChange {
        let cross = self.last_vec.cross(cur);
        if !cross.is_finite() {
            return DirChange::Unknown;
        }
        if cross == 0.0 {
            return if self.last_vec.dot(cur) < 0.0 {
                DirChange::Backwards
            } else {
                DirChange::Straight
            };
        }
        if cross > 0.0 {
            DirChange::Right
        } else {
            DirChange::Left
        }
    }

    fn add_vec(&mut self, cur: Point) -> bool {
        let dir = self.direction_change(cur);
        match dir {
            DirChange::Left | DirChange::Right => {
                if self.expected_dir == DirChange::Invalid {
                    self.expected_dir = dir;
                    self.first_direction = if dir == DirChange::Right {
                        FirstDirection::Cw
                    } else {
                        FirstDirection::Ccw
                    };
                } else if dir != self.expected_dir {
                    self.first_direction = FirstDirection::Unknown;
                    return false;
                }
                self.last_vec = cur;
            }
            DirChange::Straight => {}
            DirChange::Backwards => {
                self.last_vec = cur;
                self.reversals += 1;
                return self.reversals < 3;
            }
            DirChange::Unknown => return false,
            DirChange::Invalid => unreachable!(),
        }
        true
    }
}
