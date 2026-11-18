//! What the editor asks of each annotation: `GetBounds`, `HitTest`,
//! `MoveBy` (`Core/Annotations/*`), `CurvedSegmentHelper` for lines and
//! arrows, `NumberAnnotation`'s circle and tail, and the selection
//! controller's `GetLogicalRect`/`HitTestShape` around them.

use crate::model::{Annotation, ArrowStyle, Kind, Point};

/// `SKRect`: left, top, right, bottom (floats, as SkiaSharp's).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub l: f32,
    pub t: f32,
    pub r: f32,
    pub b: f32,
}

impl Rect {
    pub const fn new(l: f32, t: f32, r: f32, b: f32) -> Self {
        Self { l, t, r, b }
    }

    pub fn width(&self) -> f32 {
        self.r - self.l
    }

    pub fn height(&self) -> f32 {
        self.b - self.t
    }

    pub fn mid_x(&self) -> f32 {
        self.l + self.width() / 2.0
    }

    pub fn mid_y(&self) -> f32 {
        self.t + self.height() / 2.0
    }

    /// `SKRect.Inflate`.
    pub fn inflate(&self, dx: f32, dy: f32) -> Self {
        Self::new(self.l - dx, self.t - dy, self.r + dx, self.b + dy)
    }

    /// `SKRect.Contains`: left and top inclusive, right and bottom not.
    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.l && p.x < self.r && p.y >= self.t && p.y < self.b
    }

    /// Avalonia's `Rect.Contains` (doubles, every edge inclusive).
    pub fn contains_inclusive(&self, x: f64, y: f64) -> bool {
        x >= self.l as f64 && x <= self.r as f64 && y >= self.t as f64 && y <= self.b as f64
    }

    pub fn union(&self, o: &Rect) -> Rect {
        Rect::new(self.l.min(o.l), self.t.min(o.t), self.r.max(o.r), self.b.max(o.b))
    }
}

fn min_max(s: Point, e: Point) -> Rect {
    Rect::new(s.x.min(e.x), s.y.min(e.y), s.x.max(e.x), s.y.max(e.y))
}

/// `ICurvedSegmentAnnotation`: a line's or an arrow's points.
pub struct Segment {
    pub start: Point,
    pub end: Point,
    pub curve: Point,
    pub activated: bool,
    pub supports_curve: bool,
}

const CURVE_TOLERANCE_EPSILON: f32 = 0.5;

impl Segment {
    pub fn of(a: &Annotation) -> Option<Segment> {
        let (curve, activated, supports_curve) = match &a.kind {
            Kind::Line(p) => (p.curve_point, p.curve_point_activated, true),
            // `SupportsCurve`: not the modern arrow.
            Kind::Arrow(p) => (p.curve_point, p.curve_point_activated, p.style != ArrowStyle::Modern),
            _ => return None,
        };
        Some(Segment { start: a.common.start_point, end: a.common.end_point, curve, activated, supports_curve })
    }

    pub fn midpoint(s: Point, e: Point) -> Point {
        Point::new((s.x + e.x) * 0.5, (s.y + e.y) * 0.5)
    }

    /// `GetEffectiveCurvePoint`.
    pub fn effective_curve_point(&self) -> Point {
        if self.supports_curve && self.activated { self.curve } else { Self::midpoint(self.start, self.end) }
    }

    /// `HasCurve`.
    pub fn has_curve(&self) -> bool {
        if !self.supports_curve || !self.activated {
            return false;
        }
        let m = Self::midpoint(self.start, self.end);
        let (dx, dy) = (self.curve.x - m.x, self.curve.y - m.y);
        dx * dx + dy * dy > CURVE_TOLERANCE_EPSILON * CURVE_TOLERANCE_EPSILON
    }

    fn control_point(&self) -> Point {
        let a = self.effective_curve_point();
        Point::new(2.0 * a.x - 0.5 * (self.start.x + self.end.x), 2.0 * a.y - 0.5 * (self.start.y + self.end.y))
    }

    /// `GetPathPoints` (24 segments).
    pub fn path_points(&self) -> Vec<Point> {
        if !self.has_curve() {
            return vec![self.start, self.end];
        }
        let c = self.control_point();
        let n = 24;
        (0..=n)
            .map(|i| {
                let t = i as f32 / n as f32;
                let u = 1.0 - t;
                Point::new(
                    u * u * self.start.x + 2.0 * u * t * c.x + t * t * self.end.x,
                    u * u * self.start.y + 2.0 * u * t * c.y + t * t * self.end.y,
                )
            })
            .collect()
    }

    /// `DistanceToPath`.
    pub fn distance_to(&self, p: Point) -> f32 {
        let pts = self.path_points();
        pts.windows(2).map(|w| distance_to_segment(p, w[0], w[1])).fold(f32::MAX, f32::min)
    }

    /// `CurvedSegmentHelper.GetBounds`.
    pub fn bounds(&self) -> Rect {
        let pts = self.path_points();
        let mut r = Rect::new(pts[0].x, pts[0].y, pts[0].x, pts[0].y);
        for p in &pts {
            r = Rect::new(r.l.min(p.x), r.t.min(p.y), r.r.max(p.x), r.b.max(p.y));
        }
        r
    }
}

fn distance(a: Point, b: Point) -> f32 {
    let (dx, dy) = (a.x - b.x, a.y - b.y);
    (dx * dx + dy * dy).sqrt()
}

/// `DistanceToSegment` (both helpers' versions agree).
pub fn distance_to_segment(p: Point, s: Point, e: Point) -> f32 {
    let (dx, dy) = (e.x - s.x, e.y - s.y);
    let len2 = dx * dx + dy * dy;
    if len2 <= 0.001 {
        return distance(p, s);
    }
    let t = (((p.x - s.x) * dx + (p.y - s.y) * dy) / len2).clamp(0.0, 1.0);
    distance(p, Point::new(s.x + t * dx, s.y + t * dy))
}

/// Writes a segment's points back (`SetEndpoints`, `SetCurvePoint`,
/// `ResetCurvePoint`, `EnsureCurveActivated`, `OffsetCurvePoint`).
pub fn set_segment(a: &mut Annotation, start: Point, end: Point, curve: Option<Point>) {
    let Some(seg) = Segment::of(a) else { return };
    let keep_neutral = seg.supports_curve && seg.activated && !seg.has_curve();
    a.common.start_point = start;
    a.common.end_point = end;
    let (cp, act) = match &mut a.kind {
        Kind::Line(p) => (&mut p.curve_point, &mut p.curve_point_activated),
        Kind::Arrow(p) => (&mut p.curve_point, &mut p.curve_point_activated),
        _ => return,
    };
    if let Some(c) = curve {
        if seg.supports_curve {
            *cp = c;
            *act = true;
        } else {
            *cp = Segment::midpoint(start, end);
            *act = false;
        }
    } else if keep_neutral {
        *cp = Segment::midpoint(start, end);
    } else if !seg.supports_curve {
        *cp = Segment::midpoint(start, end);
        *act = false;
    }
}

/// `EnsureCurveActivated`.
pub fn ensure_curve_activated(a: &mut Annotation) {
    let Some(seg) = Segment::of(a) else { return };
    let m = Segment::midpoint(seg.start, seg.end);
    let (cp, act) = match &mut a.kind {
        Kind::Line(p) => (&mut p.curve_point, &mut p.curve_point_activated),
        Kind::Arrow(p) => (&mut p.curve_point, &mut p.curve_point_activated),
        _ => return,
    };
    if !seg.supports_curve {
        *cp = m;
        *act = false;
    } else if !*act {
        *cp = m;
        *act = true;
    }
}

/// `NumberAnnotation.Radius`.
pub fn step_radius(a: &Annotation) -> f32 {
    match &a.kind {
        Kind::Number(p) => crate::render::step_radius(p),
        _ => 0.0,
    }
}

/// `NumberAnnotation.TryGetTailPolygon`: base start, tip, base end.
pub fn step_tail(a: &Annotation) -> Option<(Point, Point, Point)> {
    let Kind::Number(p) = &a.kind else { return None };
    step_tail_points(a.common.start_point, step_radius(a), p)
}

/// `NumberAnnotation.TryGetTailPolygon` for a circle of `radius` at
/// `center`: the tail's base meets the circle at its tangent points.
pub fn step_tail_points(center: Point, radius: f32, p: &crate::model::NumberProps) -> Option<(Point, Point, Point)> {
    if !p.tail_enabled || !p.tail_point_initialized {
        return None;
    }
    let c = center;
    let r = radius;
    if r <= GEOMETRY_EPSILON {
        return None;
    }
    let tip = p.tail_point;
    let (dx, dy) = (tip.x - c.x, tip.y - c.y);
    let len = (dx * dx + dy * dy).sqrt();
    if len <= r + GEOMETRY_EPSILON {
        return None;
    }
    let (nx, ny) = (dx / len, dy / len);
    let perp = (-ny, nx);
    let proj = (r * r) / len;
    let off = r * ((len * len) - (r * r)).sqrt() / len;
    let tc = (c.x + nx * proj, c.y + ny * proj);
    Some((
        Point::new(tc.0 + perp.0 * off, tc.1 + perp.1 * off),
        tip,
        Point::new(tc.0 - perp.0 * off, tc.1 - perp.1 * off),
    ))
}

/// `TailGeometryHelper.GeometryEpsilon`.
const GEOMETRY_EPSILON: f32 = 0.001;

/// `SpeechBalloonAnnotation.TailWidthMultiplier`.
const TAIL_WIDTH_MULTIPLIER: f32 = 0.3;

/// `SpeechBalloonAnnotation.DefaultTailOffset`.
const DEFAULT_TAIL_OFFSET: f32 = 30.0;

/// `SpeechBalloonAnnotation.GetEffectiveTailPoint`: the tail point, or
/// below the middle of the body when it was never set.
pub fn balloon_effective_tail_point(body: &Rect, p: &crate::model::SpeechBalloonProps) -> Point {
    let has_tail_point = p.tail_point_initialized || p.tail_point != Point::default();
    if has_tail_point {
        p.tail_point
    } else {
        Point::new(body.mid_x(), body.b + DEFAULT_TAIL_OFFSET)
    }
}

/// `SpeechBalloonAnnotation.TryGetTailPolygon`: a tail from the body's
/// centre towards the tail point, its base where it leaves the body's
/// rectangle.
pub fn balloon_tail_points(body: &Rect, p: &crate::model::SpeechBalloonProps) -> Option<(Point, Point, Point)> {
    if !p.tail_enabled {
        return None;
    }
    let bounds = *body;
    if bounds.width() <= 0.0 || bounds.height() <= 0.0 {
        return None;
    }
    let tail_tip = balloon_effective_tail_point(body, p);
    if bounds.contains(tail_tip) {
        return None;
    }
    let center = Point::new(bounds.mid_x(), bounds.mid_y());
    let direction_x = tail_tip.x - center.x;
    let direction_y = tail_tip.y - center.y;
    let direction_length = (direction_x * direction_x + direction_y * direction_y).sqrt();
    if direction_length <= GEOMETRY_EPSILON {
        return None;
    }
    let normalized_direction_x = direction_x / direction_length;
    let normalized_direction_y = direction_y / direction_length;
    let rect_average_size = (bounds.width() + bounds.height()) / 2.0;
    let mut tail_width = TAIL_WIDTH_MULTIPLIER * rect_average_size;
    tail_width = tail_width.min(bounds.width().min(bounds.height()));
    if tail_width <= GEOMETRY_EPSILON {
        return None;
    }
    let half_tail_width = tail_width / 2.0;
    let perpendicular = (-normalized_direction_y, normalized_direction_x);
    let base_start = Point::new(center.x + perpendicular.0 * half_tail_width, center.y + perpendicular.1 * half_tail_width);
    let base_end = Point::new(center.x - perpendicular.0 * half_tail_width, center.y - perpendicular.1 * half_tail_width);
    let s = segment_exit_point(&bounds, base_start, tail_tip)?;
    let e = segment_exit_point(&bounds, base_end, tail_tip)?;
    Some((s, tail_tip, e))
}

/// `SpeechBalloonAnnotation.TryGetSegmentExitPoint` (NaN t passes the
/// range test, as in C#).
#[allow(clippy::manual_range_contains)]
fn segment_exit_point(bounds: &Rect, start: Point, end: Point) -> Option<Point> {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let mut best_t = f32::INFINITY;
    let mut best = Point::default();
    let mut consider = |t: f32| {
        if t < -GEOMETRY_EPSILON || t > 1.0 + GEOMETRY_EPSILON || t >= best_t {
            return;
        }
        let x = start.x + dx * t;
        let y = start.y + dy * t;
        if x < bounds.l - GEOMETRY_EPSILON
            || x > bounds.r + GEOMETRY_EPSILON
            || y < bounds.t - GEOMETRY_EPSILON
            || y > bounds.b + GEOMETRY_EPSILON
        {
            return;
        }
        best_t = t.clamp(0.0, 1.0);
        best = Point::new(x.clamp(bounds.l, bounds.r), y.clamp(bounds.t, bounds.b));
    };
    if dx.abs() > GEOMETRY_EPSILON {
        consider((bounds.l - start.x) / dx);
        consider((bounds.r - start.x) / dx);
    }
    if dy.abs() > GEOMETRY_EPSILON {
        consider((bounds.t - start.y) / dy);
        consider((bounds.b - start.y) / dy);
    }
    if best_t.is_infinite() { None } else { Some(best) }
}

/// `SpeechBalloonAnnotation.GetInteractionBounds(0)`: the body and its tail.
pub fn balloon_interaction_bounds(body: &Rect, p: &crate::model::SpeechBalloonProps) -> Rect {
    match balloon_tail_points(body, p) {
        Some((s, t, e)) => body.union(&triangle_bounds(s, t, e)),
        None => *body,
    }
}

/// `TailGeometryHelper.GetBounds`.
pub fn triangle_bounds(a: Point, b: Point, c: Point) -> Rect {
    Rect::new(a.x.min(b.x).min(c.x), a.y.min(b.y).min(c.y), a.x.max(b.x).max(c.x), a.y.max(b.y).max(c.y))
}

/// `TailGeometryHelper.HitTest`: inside the triangle, or within
/// `tolerance` of its edges.
fn triangle_hit(p: Point, a: Point, b: Point, c: Point, tolerance: f32) -> bool {
    let sign = |p1: Point, p2: Point, p3: Point| (p1.x - p3.x) * (p2.y - p3.y) - (p2.x - p3.x) * (p1.y - p3.y);
    let (d1, d2, d3) = (sign(p, a, b), sign(p, b, c), sign(p, c, a));
    let neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    if !(neg && pos) {
        return true;
    }
    distance_to_segment(p, a, b) <= tolerance
        || distance_to_segment(p, b, c) <= tolerance
        || distance_to_segment(p, c, a) <= tolerance
}

/// `Annotation.GetBounds` of each type.
pub fn bounds(a: &Annotation) -> Rect {
    let c = &a.common;
    match &a.kind {
        Kind::Line(_) | Kind::Arrow(_) => Segment::of(a).map(|s| s.bounds()).unwrap_or_default(),
        Kind::Freehand(p) => {
            if p.points.is_empty() {
                return Rect::default();
            }
            let mut r = Rect::new(f32::MAX, f32::MAX, f32::MIN, f32::MIN);
            for q in &p.points {
                r = Rect::new(r.l.min(q.x), r.t.min(q.y), r.r.max(q.x), r.b.max(q.y));
            }
            r
        }
        Kind::Text(_) => {
            let mut r = min_max(c.start_point, c.end_point);
            if r.r - r.l < 10.0 {
                r.r = r.l + 10.0;
            }
            if r.b - r.t < 10.0 {
                r.b = r.t + 10.0;
            }
            r
        }
        Kind::Number(_) => {
            let r = step_radius(a);
            let s = c.start_point;
            Rect::new(s.x - r, s.y - r, s.x + r, s.y + r)
        }
        _ => min_max(c.start_point, c.end_point),
    }
}

/// `NumberAnnotation.GetInteractionBounds`.
pub fn step_interaction_bounds(a: &Annotation, tolerance: f32) -> Rect {
    let mut r = bounds(a);
    if let Some((s, t, e)) = step_tail(a) {
        r = r.union(&triangle_bounds(s, t, e));
    }
    if tolerance > 0.0 { r.inflate(tolerance, tolerance) } else { r }
}

/// `Annotation.GetUnrotatedPoint`.
fn unrotated(a: &Annotation, p: Point, r: &Rect) -> Point {
    let angle = a.common.rotation_angle;
    if angle == 0.0 {
        return p;
    }
    let (cx, cy) = (r.mid_x(), r.mid_y());
    let rad = -angle * std::f32::consts::PI / 180.0;
    let (cos, sin) = (rad.cos(), rad.sin());
    let (dx, dy) = (p.x - cx, p.y - cy);
    Point::new(cx + dx * cos - dy * sin, cy + dx * sin + dy * cos)
}

fn in_ellipse(p: Point, r: &Rect) -> bool {
    let (rx, ry) = (r.width() / 2.0, r.height() / 2.0);
    if rx <= 0.0 || ry <= 0.0 {
        return false;
    }
    let (nx, ny) = ((p.x - r.mid_x()) / rx, (p.y - r.mid_y()) / ry);
    nx * nx + ny * ny <= 1.0
}

/// `Annotation.HitTest(point, tolerance)` of each type.
pub fn hit_test(a: &Annotation, p: Point, tolerance: f32) -> bool {
    let r = bounds(a);
    match &a.kind {
        Kind::Rectangle(_) | Kind::SmartEraser(_) | Kind::Text(_) | Kind::Image(_) | Kind::Emoji(_) | Kind::Cursor(_) => {
            let q = unrotated(a, p, &r);
            r.inflate(tolerance, tolerance).contains(q)
        }
        Kind::Ellipse(_) => {
            let q = unrotated(a, p, &r);
            if !r.inflate(tolerance, tolerance).contains(q) {
                return false;
            }
            let (rx, ry) = (r.width() / 2.0 + tolerance, r.height() / 2.0 + tolerance);
            if rx <= 0.0 || ry <= 0.0 {
                return false;
            }
            let (dx, dy) = (q.x - r.mid_x(), q.y - r.mid_y());
            (dx * dx) / (rx * rx) + (dy * dy) / (ry * ry) <= 1.0
        }
        Kind::Line(_) | Kind::Arrow(_) => Segment::of(a).is_some_and(|s| s.distance_to(p) <= tolerance),
        Kind::Freehand(f) => {
            if !r.inflate(tolerance, tolerance).contains(p) {
                return false;
            }
            f.points.windows(2).any(|w| distance_to_segment(p, w[0], w[1]) <= tolerance)
        }
        Kind::Highlight(e) | Kind::Blur(e) | Kind::Pixelate(e) => {
            effect_hit(a, p, &r, tolerance, e.is_ellipse)
        }
        Kind::Magnify(m) => effect_hit(a, p, &r, tolerance, m.is_ellipse),
        Kind::Spotlight(s) => {
            let inflated = r.inflate(tolerance, tolerance);
            if s.is_ellipse { in_ellipse(p, &inflated) } else { inflated.contains(p) }
        }
        Kind::Number(_) => {
            let c = a.common.start_point;
            if distance(p, c) <= step_radius(a) + tolerance {
                return true;
            }
            step_tail(a).is_some_and(|(s, t, e)| triangle_hit(p, s, t, e, tolerance))
        }
        Kind::SpeechBalloon(_) => {
            let q = unrotated(a, p, &r);
            r.inflate(tolerance, tolerance).contains(q)
        }
        Kind::Crop => false,
    }
}

fn effect_hit(a: &Annotation, p: Point, r: &Rect, tolerance: f32, ellipse: bool) -> bool {
    let q = unrotated(a, p, r);
    let inflated = r.inflate(tolerance, tolerance);
    if ellipse { in_ellipse(q, &inflated) } else { inflated.contains(q) }
}

/// `Annotation.MoveBy` of each type.
pub fn move_by(a: &mut Annotation, dx: f32, dy: f32) {
    let shift = |p: &mut Point| *p = Point::new(p.x + dx, p.y + dy);
    match &mut a.kind {
        Kind::Freehand(f) => {
            // Positioned by its points, not its drawing end points.
            f.points.iter_mut().for_each(shift);
            return;
        }
        Kind::Line(p) if p.curve_point_activated => shift(&mut p.curve_point),
        Kind::Arrow(p) if p.curve_point_activated && p.style != ArrowStyle::Modern => shift(&mut p.curve_point),
        Kind::Number(p) if p.tail_point_initialized => shift(&mut p.tail_point),
        Kind::SpeechBalloon(p) if p.tail_point_initialized => shift(&mut p.tail_point),
        _ => {}
    }
    shift(&mut a.common.start_point);
    shift(&mut a.common.end_point);
}

/// The tolerance `HitTestShape` calls `HitTest` with: the base
/// declaration's default (10), as C# binds defaults to the static type.
pub const HIT_TOLERANCE: f32 = 10.0;

/// `EditorSelectionController.HitTestShape` for one annotation: the
/// rough bounds check (`GetLogicalRect`, widened for lines, freehand,
/// steps and balloons; skipped when rotated), then `HitTest`.
pub fn shape_hit(a: &Annotation, x: f64, y: f64) -> bool {
    let p = Point::new(x as f32, y as f32);
    if let Kind::Spotlight(_) = a.kind {
        // `sa.HitTest(point)` on a `SpotlightAnnotation`: its own default.
        return hit_test(a, p, 5.0);
    }
    let rough = match &a.kind {
        Kind::Line(_) | Kind::Arrow(_) => bounds(a).inflate(10.0, 10.0),
        Kind::Freehand(_) => bounds(a).inflate(5.0, 5.0),
        Kind::Number(_) => step_interaction_bounds(a, 5.0),
        Kind::SpeechBalloon(_) => bounds(a).inflate(5.0, 5.0),
        _ => bounds(a),
    };
    let rotated = a.common.rotation_angle != 0.0;
    (rotated || rough.contains_inclusive(x, y)) && hit_test(a, p, HIT_TOLERANCE)
}

/// Rotates `p` about `c` by `degrees` (`RotatePoint`).
pub fn rotate_point(p: (f64, f64), c: (f64, f64), degrees: f64) -> (f64, f64) {
    if degrees == 0.0 {
        return p;
    }
    let rad = degrees * std::f64::consts::PI / 180.0;
    let (cos, sin) = (rad.cos(), rad.sin());
    let (dx, dy) = (p.0 - c.0, p.1 - c.1);
    (c.0 + dx * cos - dy * sin, c.1 + dx * sin + dy * cos)
}

/// `Annotation.TransformAdditionalPoints` of each type: a freehand's
/// points, a step's tail (once it has one), a balloon's tail (its
/// effective point, kept placed or set when it has one). A line's or an
/// arrow's curve point isn't transformed, as in ShareX.
pub fn transform_additional_points(a: &mut Annotation, f: &dyn Fn(Point) -> Point) {
    match &mut a.kind {
        Kind::Freehand(p) => p.points.iter_mut().for_each(|q| *q = f(*q)),
        Kind::Number(p) => {
            if p.tail_point_initialized {
                p.tail_point = f(p.tail_point);
            }
        }
        Kind::SpeechBalloon(_) => {
            let body = bounds(a);
            let Kind::SpeechBalloon(p) = &mut a.kind else {
                unreachable!()
            };
            let has_tail = p.tail_point_initialized || p.tail_point != Point::default();
            let transformed = f(balloon_effective_tail_point(&body, p));
            // SetPlacedTailPoint keeps it placed; SetTailPoint sets it.
            if p.tail_point_placed || has_tail {
                p.tail_point = transformed;
                p.tail_point_initialized = true;
            }
        }
        _ => {}
    }
}

/// `Annotation.TransformPoints`: the end points, then the type's own.
pub fn transform_points(a: &mut Annotation, f: &dyn Fn(Point) -> Point) {
    a.common.start_point = f(a.common.start_point);
    a.common.end_point = f(a.common.end_point);
    transform_additional_points(a, f);
}

/// `Annotation.Scale` (and `TextAnnotation.Scale`, which scales its font).
pub fn scale(a: &mut Annotation, sx: f32, sy: f32) {
    transform_points(a, &|p| Point::new(p.x * sx, p.y * sy));
    a.common.stroke_width *= sx.min(sy);
    if let Kind::Text(t) = &mut a.kind {
        t.font_size *= sy;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(l: f32, t: f32, r: f32, b: f32) -> Annotation {
        let mut a = Annotation::new("Rectangle").unwrap();
        a.common.start_point = Point::new(l, t);
        a.common.end_point = Point::new(r, b);
        a
    }

    #[test]
    fn rectangles_hit_inside_their_bounds_only() {
        let a = rect(10.0, 10.0, 50.0, 40.0);
        assert!(shape_hit(&a, 10.0, 10.0));
        assert!(shape_hit(&a, 50.0, 40.0));
        assert!(!shape_hit(&a, 9.0, 20.0));
    }

    #[test]
    fn lines_hit_within_ten_pixels() {
        let mut a = Annotation::new("Line").unwrap();
        a.common.start_point = Point::new(0.0, 0.0);
        a.common.end_point = Point::new(100.0, 0.0);
        assert!(shape_hit(&a, 50.0, 9.0));
        assert!(!shape_hit(&a, 50.0, 11.0));
    }

    #[test]
    fn curve_points_follow_moves() {
        let mut a = Annotation::new("Line").unwrap();
        a.common.end_point = Point::new(100.0, 0.0);
        ensure_curve_activated(&mut a);
        move_by(&mut a, 5.0, 5.0);
        let Kind::Line(p) = &a.kind else { unreachable!() };
        assert_eq!(p.curve_point, Point::new(55.0, 5.0));
    }
}
