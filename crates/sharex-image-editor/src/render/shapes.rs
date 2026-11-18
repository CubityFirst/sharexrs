//! The shape annotations' visuals: `RectangleAnnotation.Visual.cs`,
//! `EllipseAnnotation.Visual.cs`, `LineAnnotation.Visual.cs`,
//! `ArrowAnnotation.Visual.cs`, `FreehandAnnotation.Visual.cs`, with the
//! geometry Avalonia builds for them (`RectangleGeometry`,
//! `EllipseGeometry`, `StreamGeometry`) and `Shape.Render`'s
//! `DrawGeometry(Fill, pen, geometry)`.

use sharex_skia::{
    Cap, Color, Direction, FillType, Join, Matrix, Path, Pixmap, Rect, Stroke, draw,
};

use super::color::parse_color;
use super::layout::{self, AMatrix};
use crate::model::{ArrowProps, ArrowStyle, BorderStyle, Common, FreehandProps, LineProps, Point};

// --- Avalonia's StreamGeometry on Skia (StreamGeometryImpl) ----------------

/// A `StreamGeometry` as Avalonia's Skia backend builds it: one path for
/// the stroke, and one for the fill holding only the filled figures
/// (the same path while every figure is filled). Both even-odd.
pub(crate) struct StreamGeometry {
    stroke: Path,
    fill: Option<Path>,
    is_filled: bool,
}

impl StreamGeometry {
    pub fn new() -> Self {
        let mut stroke = Path::new();
        stroke.fill_type = FillType::EvenOdd;
        Self {
            stroke,
            fill: None,
            is_filled: false,
        }
    }

    fn ensure_separate_fill(&mut self) {
        if self.fill.is_none() {
            self.fill = Some(self.stroke.clone());
        }
    }

    fn duplicate(&self) -> bool {
        self.is_filled && self.fill.is_some()
    }

    fn both(&mut self, f: impl Fn(&mut Path)) {
        let dup = self.duplicate();
        f(&mut self.stroke);
        if dup && let Some(fill) = &mut self.fill {
            f(fill);
        }
    }

    pub fn begin_figure(&mut self, x: f64, y: f64, is_filled: bool) {
        if !is_filled {
            self.ensure_separate_fill();
        }
        self.is_filled = is_filled;
        let (x, y) = (x as f32, y as f32);
        self.both(|p| {
            p.move_to(x, y);
        });
    }

    pub fn line_to(&mut self, x: f64, y: f64) {
        let (x, y) = (x as f32, y as f32);
        self.both(|p| {
            p.line_to(x, y);
        });
    }

    pub fn quad_to(&mut self, x1: f64, y1: f64, x2: f64, y2: f64) {
        let (x1, y1, x2, y2) = (x1 as f32, y1 as f32, x2 as f32, y2 as f32);
        self.both(|p| {
            p.quad_to(x1, y1, x2, y2);
        });
    }

    /// `ArcTo(point, size, 0, isLargeArc, sweep)`.
    pub fn arc_to(&mut self, x: f64, y: f64, rx: f64, ry: f64, large: bool, clockwise: bool) {
        let dir = if clockwise {
            Direction::Cw
        } else {
            Direction::Ccw
        };
        let (x, y, rx, ry) = (x as f32, y as f32, rx as f32, ry as f32);
        self.both(|p| {
            p.arc_to(rx, ry, 0.0, large, dir, x, y);
        });
    }

    pub fn end_figure(&mut self, closed: bool) {
        if closed {
            self.both(|p| {
                p.close();
            });
        }
    }

    /// (stroke path, fill path).
    pub fn finish(self) -> (Path, Path) {
        let fill = self.fill.unwrap_or_else(|| self.stroke.clone());
        (self.stroke, fill)
    }
}

// --- drawing -----------------------------------------------------------------

/// `BorderStyleDashHelper.CreateStrokeDashArray` (in stroke widths).
pub(super) fn dash_array(style: &BorderStyle) -> Vec<f64> {
    match style {
        BorderStyle::Dash => vec![3.0, 1.0],
        BorderStyle::Dot => vec![1.0, 1.0],
        BorderStyle::DashDot => vec![3.0, 1.0, 1.0, 1.0],
        BorderStyle::DashDotDot => vec![3.0, 1.0, 1.0, 1.0, 1.0, 1.0],
        _ => Vec::new(),
    }
}

/// `BorderStyleDashHelper.CreateStrokeLineCap`.
pub(super) fn line_cap(style: &BorderStyle) -> Cap {
    if *style == BorderStyle::Solid {
        Cap::Round
    } else {
        Cap::Butt
    }
}

/// An Avalonia pen, as `DrawingContextImpl.TryCreatePaint` turns it into
/// a Skia stroke (dashes scaled by the thickness).
pub(crate) struct Pen {
    pub color: Color,
    pub thickness: f64,
    pub dashes: Vec<f64>,
    pub cap: Cap,
    pub join: Join,
}

impl Pen {
    pub(crate) fn stroke(&self) -> Option<Stroke> {
        if self.thickness == 0.0 {
            return None;
        }
        let dash = if self.dashes.is_empty() {
            None
        } else {
            let n = if self.dashes.len().is_multiple_of(2) {
                self.dashes.len()
            } else {
                self.dashes.len() * 2
            };
            let arr = (0..n)
                .map(|i| self.dashes[i % self.dashes.len()] as f32 * self.thickness as f32)
                .collect();
            Some((arr, 0.0))
        };
        Some(Stroke {
            width: self.thickness as f32,
            cap: self.cap,
            join: self.join,
            miter_limit: 10.0,
            dash,
        })
    }
}

/// `DrawingContextImpl.DrawGeometry(brush, pen, geometry)` under `ctm`.
pub(super) fn draw_geometry(
    pm: &mut Pixmap,
    ctm: &AMatrix,
    fill: Option<Color>,
    fill_path: &Path,
    pen: Option<&Pen>,
    stroke_path: &Path,
) {
    draw_geometry_clipped(pm, ctm, fill, fill_path, pen, stroke_path, None);
}

/// [`draw_geometry`] inside a device clip rectangle (a visual with
/// `ClipToBounds`).
pub(super) fn draw_geometry_clipped(
    pm: &mut Pixmap,
    ctm: &AMatrix,
    fill: Option<Color>,
    fill_path: &Path,
    pen: Option<&Pen>,
    stroke_path: &Path,
    clip: Option<sharex_skia::IRect>,
) {
    let m: Matrix = ctm.to_sk();
    if let Some(c) = fill
        && c.a != 0
        && !fill_path.is_empty()
    {
        let dev = fill_path.transformed(&m);
        draw::fill_path(pm, &dev, c, clip);
    }
    if let Some(pen) = pen
        && pen.color.a != 0
        && let Some(stroke) = pen.stroke()
    {
        draw::stroke_path(pm, stroke_path, &stroke, &m, pen.color, clip);
    }
}

fn fill_brush(fill_color: &str) -> Option<Color> {
    if fill_color.is_empty() || fill_color == "#00000000" {
        None
    } else {
        Some(parse_color(fill_color))
    }
}

/// `Annotation.GetBounds` for the rectangle-shaped annotations.
pub(super) fn bounds(c: &Common) -> (f32, f32, f32, f32) {
    (
        c.start_point.x.min(c.end_point.x),
        c.start_point.y.min(c.end_point.y),
        c.start_point.x.max(c.end_point.x),
        c.start_point.y.max(c.end_point.y),
    )
}

/// The arranged visual of a rectangle-shaped annotation
/// (`ApplyBoundsControl`, persisted mode) and its render transform.
pub(super) fn arranged(c: &Common) -> (layout::Arranged, AMatrix) {
    let (l, t, r, b) = bounds(c);
    let a = layout::arrange(l as f64, t as f64, (r - l) as f64, (b - t) as f64);
    let m = layout::visual_transform(&a, c.rotation_angle);
    (a, m)
}

/// `Rect(Bounds.Size).Deflate(StrokeThickness / 2)` as an `SKRect`.
fn deflated(a: &layout::Arranged, thickness: f64) -> Rect {
    let h = thickness / 2.0;
    let x = h;
    let y = h;
    let w = (a.width - h - h).max(0.0);
    let hh = (a.height - h - h).max(0.0);
    Rect::from_ltrb(x as f32, y as f32, (x + w) as f32, (y + hh) as f32)
}

pub fn draw_rectangle(pm: &mut Pixmap, c: &Common, border_style: BorderStyle, corner_radius: i32) {
    let (a, ctm) = arranged(c);
    let thickness = c.stroke_width as f64;
    let rect = deflated(&a, thickness);
    let radius = corner_radius.max(0) as f64;
    let path = if radius == 0.0 {
        // RectangleGeometryImpl: SKPath.AddRect
        let mut p = Path::new();
        p.add_rect(rect, Direction::Cw, 0);
        p
    } else {
        // GeometryBuilder.DrawRoundedCornersRectangle
        let (l, t, r, b) = (
            rect.left as f64,
            rect.top as f64,
            rect.right as f64,
            rect.bottom as f64,
        );
        let (rl, rt, rw, rh) = (l, t, r - l, b - t);
        let (right, bottom) = (rl + rw, rt + rh);
        let mut g = StreamGeometry::new();
        g.begin_figure(rl + radius, rt, true);
        g.line_to(right - radius, rt);
        g.arc_to(right, rt + radius, radius, radius, false, true);
        g.line_to(right, bottom - radius);
        g.arc_to(right - radius, bottom, radius, radius, false, true);
        g.line_to(rl + radius, bottom);
        g.arc_to(rl, bottom - radius, radius, radius, false, true);
        g.line_to(rl, rt + radius);
        g.arc_to(rl + radius, rt, radius, radius, false, true);
        g.end_figure(true);
        g.finish().0
    };
    let pen = Pen {
        color: parse_color(&c.stroke_color),
        thickness,
        dashes: dash_array(&border_style),
        cap: line_cap(&border_style),
        join: Join::Miter,
    };
    draw_geometry(
        pm,
        &ctm,
        fill_brush(&c.fill_color),
        &path,
        Some(&pen),
        &path,
    );
}

pub fn draw_ellipse(pm: &mut Pixmap, c: &Common, border_style: BorderStyle) {
    let (a, ctm) = arranged(c);
    let thickness = c.stroke_width as f64;
    let rect = deflated(&a, thickness);
    let mut path = Path::new();
    path.add_oval(rect, Direction::Cw);
    let pen = Pen {
        color: parse_color(&c.stroke_color),
        thickness,
        dashes: dash_array(&border_style),
        cap: line_cap(&border_style),
        join: Join::Miter,
    };
    draw_geometry(
        pm,
        &ctm,
        fill_brush(&c.fill_color),
        &path,
        Some(&pen),
        &path,
    );
}

// --- CurvedSegmentHelper --------------------------------------------------

const CURVE_TOLERANCE_EPSILON: f32 = 0.5;

fn midpoint(s: Point, e: Point) -> Point {
    Point::new((s.x + e.x) * 0.5, (s.y + e.y) * 0.5)
}

struct Curve {
    start: Point,
    end: Point,
    curve_point: Point,
    activated: bool,
    supports_curve: bool,
}

impl Curve {
    fn has_curve(&self) -> bool {
        if !self.supports_curve || !self.activated {
            return false;
        }
        let m = midpoint(self.start, self.end);
        let dx = self.curve_point.x - m.x;
        let dy = self.curve_point.y - m.y;
        dx * dx + dy * dy > CURVE_TOLERANCE_EPSILON * CURVE_TOLERANCE_EPSILON
    }

    fn effective_curve_point(&self) -> Point {
        if self.supports_curve && self.activated {
            self.curve_point
        } else {
            midpoint(self.start, self.end)
        }
    }

    fn control_point(&self) -> Point {
        let a = self.effective_curve_point();
        Point::new(
            2.0 * a.x - 0.5 * (self.start.x + self.end.x),
            2.0 * a.y - 0.5 * (self.start.y + self.end.y),
        )
    }

    fn tangent_at_start(&self) -> Point {
        let c = self.control_point();
        let mut t = Point::new(c.x - self.start.x, c.y - self.start.y);
        if t.x.abs() < 0.001 && t.y.abs() < 0.001 {
            t = Point::new(self.end.x - self.start.x, self.end.y - self.start.y);
        }
        t
    }

    fn tangent_at_end(&self) -> Point {
        let c = self.control_point();
        let mut t = Point::new(self.end.x - c.x, self.end.y - c.y);
        if t.x.abs() < 0.001 && t.y.abs() < 0.001 {
            t = Point::new(self.end.x - self.start.x, self.end.y - self.start.y);
        }
        t
    }
}

fn line_geometry(c: &Common, p: &LineProps) -> StreamGeometry {
    let curve = Curve {
        start: c.start_point,
        end: c.end_point,
        curve_point: p.curve_point,
        activated: p.curve_point_activated,
        supports_curve: true,
    };
    let mut g = StreamGeometry::new();
    let (s, e) = (c.start_point, c.end_point);
    g.begin_figure(s.x as f64, s.y as f64, false);
    if curve.has_curve() {
        let cp = curve.control_point();
        g.quad_to(cp.x as f64, cp.y as f64, e.x as f64, e.y as f64);
    } else {
        g.line_to(e.x as f64, e.y as f64);
    }
    g.end_figure(false);
    g
}

pub fn draw_line(pm: &mut Pixmap, c: &Common, p: &LineProps) {
    let (stroke, fill) = line_geometry(c, p).finish();
    let pen = Pen {
        color: parse_color(&c.stroke_color),
        thickness: c.stroke_width as f64,
        dashes: dash_array(&p.border_style),
        cap: line_cap(&p.border_style),
        join: Join::Round,
    };
    // Fill = Brushes.Transparent
    draw_geometry(
        pm,
        &AMatrix::translation(0.0, 0.0),
        None,
        &fill,
        Some(&pen),
        &stroke,
    );
}

// --- ArrowAnnotation -------------------------------------------------------

const ARROW_HEAD_LENGTH_RATIO: f64 = 3.0;
const ARROW_HEAD_BACK_CURVE_CONTROL_RATIO: f64 = 2.0;

fn head_width_multiplier(style: &ArrowStyle) -> f64 {
    match style {
        ArrowStyle::Line => 1.75,
        ArrowStyle::Basic => 1.75,
        ArrowStyle::Modern => 3.0,
        _ => 2.0,
    }
}

struct CapPoints {
    left_base: Point,
    right_base: Point,
    back_curve_control: Point,
}

/// `ComputeArrowCapPointsFromTangent`.
fn cap_points(tip_x: f32, tip_y: f32, tx: f32, ty: f32, half: f64) -> Option<CapPoints> {
    let len = ((tx * tx + ty * ty) as f64).sqrt();
    if len <= 0.0 {
        return None;
    }
    let ux = tx as f64 / len;
    let uy = ty as f64 / len;
    let (px, py) = (-uy, ux);
    let head_length = half * ARROW_HEAD_LENGTH_RATIO;
    let back = half * ARROW_HEAD_BACK_CURVE_CONTROL_RATIO;
    let bx = tip_x as f64 - head_length * ux;
    let by = tip_y as f64 - head_length * uy;
    Some(CapPoints {
        left_base: Point::new((bx + px * half) as f32, (by + py * half) as f32),
        right_base: Point::new((bx - px * half) as f32, (by - py * half) as f32),
        back_curve_control: Point::new(
            (tip_x as f64 - ux * back) as f32,
            (tip_y as f64 - uy * back) as f32,
        ),
    })
}

/// `ComputeBasicArrowHeadPointsFromTangent` / `ComputeLineArrowHeadPointsFromTangent`.
fn base_points(tip_x: f32, tip_y: f32, tx: f32, ty: f32, half: f64) -> Option<(Point, Point)> {
    let len = ((tx * tx + ty * ty) as f64).sqrt();
    if len <= 0.0 {
        return None;
    }
    let ux = tx as f64 / len;
    let uy = ty as f64 / len;
    let (px, py) = (-uy, ux);
    let head_length = half * 3.0;
    let bx = tip_x as f64 - head_length * ux;
    let by = tip_y as f64 - head_length * uy;
    Some((
        Point::new((bx + px * half) as f32, (by + py * half) as f32),
        Point::new((bx - px * half) as f32, (by - py * half) as f32),
    ))
}

/// `ComputeModernArrowHeadPointsFromTangent`.
fn modern_head(tip_x: f32, tip_y: f32, tx: f32, ty: f32, head_size: f64) -> Option<(Point, Point)> {
    let len = ((tx * tx + ty * ty) as f64).sqrt();
    if len <= 0.0 {
        return None;
    }
    let ux = tx as f64 / len;
    let uy = ty as f64 / len;
    let (px, py) = (-uy, ux);
    let head_height = head_size * 2.5;
    let head_width_base = head_size * 1.5;
    let wing = head_width_base * (std::f64::consts::PI / 5.14).tan();
    let bx = tip_x as f64 - head_height * ux;
    let by = tip_y as f64 - head_height * uy;
    Some((
        Point::new((bx + px * wing) as f32, (by + py * wing) as f32),
        Point::new((bx - px * wing) as f32, (by - py * wing) as f32),
    ))
}

/// `ComputeArrowPoints` (the modern straight arrow).
fn modern_points(sx: f32, sy: f32, ex: f32, ey: f32, head_size: f64) -> Option<[Point; 4]> {
    let dx = ex - sx;
    let dy = ey - sy;
    let len = ((dx * dx + dy * dy) as f64).sqrt();
    if len <= 0.0 {
        return None;
    }
    let ux = dx as f64 / len;
    let uy = dy as f64 / len;
    let (px, py) = (-uy, ux);
    let head_height = head_size * 2.5;
    let head_width_base = head_size * 1.5;
    let arrow_angle = std::f64::consts::PI / 5.14;
    let bx = ex as f64 - head_height * ux;
    let by = ey as f64 - head_height * uy;
    let wing = head_width_base * arrow_angle.tan();
    let shaft = head_width_base * 0.25;
    let p = |w: f64| Point::new((bx + px * w) as f32, (by + py * w) as f32);
    Some([p(shaft), p(-shaft), p(wing), p(-wing)])
}

fn append_cap_figure(g: &mut StreamGeometry, tip: Point, cap: &CapPoints) {
    g.begin_figure(tip.x as f64, tip.y as f64, true);
    g.line_to(cap.left_base.x as f64, cap.left_base.y as f64);
    g.quad_to(
        cap.back_curve_control.x as f64,
        cap.back_curve_control.y as f64,
        cap.right_base.x as f64,
        cap.right_base.y as f64,
    );
    g.end_figure(true);
}

fn degenerate_circle(g: &mut StreamGeometry, s: Point) {
    let r = 2.0;
    let (x, y) = (s.x as f64, s.y as f64);
    g.begin_figure(x - r, y, true);
    g.arc_to(x + r, y, r, r, false, true);
    g.arc_to(x - r, y, r, r, false, true);
    g.end_figure(true);
}

fn arrow_geometry(c: &Common, p: &ArrowProps) -> StreamGeometry {
    let head = c.stroke_width as f64 * head_width_multiplier(&p.style);
    let (s, e) = (c.start_point, c.end_point);
    let curve = Curve {
        start: s,
        end: e,
        curve_point: p.curve_point,
        activated: p.curve_point_activated,
        supports_curve: p.style != ArrowStyle::Modern,
    };
    let has_curve = curve.has_curve();
    let mut g = StreamGeometry::new();
    let line = |g: &mut StreamGeometry| {
        g.begin_figure(s.x as f64, s.y as f64, false);
        g.line_to(e.x as f64, e.y as f64);
        g.end_figure(false);
    };
    let quad = |g: &mut StreamGeometry| {
        let cp = curve.control_point();
        g.begin_figure(s.x as f64, s.y as f64, false);
        g.quad_to(cp.x as f64, cp.y as f64, e.x as f64, e.y as f64);
        g.end_figure(false);
    };
    match (&p.style, has_curve) {
        (ArrowStyle::Classic | ArrowStyle::Double, false) | (ArrowStyle::Unknown(_), false) => {
            let double = p.style == ArrowStyle::Double;
            match cap_points(e.x, e.y, e.x - s.x, e.y - s.y, head) {
                Some(end_cap) => {
                    line(&mut g);
                    if double && let Some(sc) = cap_points(s.x, s.y, s.x - e.x, s.y - e.y, head) {
                        append_cap_figure(&mut g, s, &sc);
                    }
                    append_cap_figure(&mut g, e, &end_cap);
                }
                None => degenerate_circle(&mut g, s),
            }
        }
        (ArrowStyle::Classic | ArrowStyle::Double, true) | (ArrowStyle::Unknown(_), true) => {
            quad(&mut g);
            let t = curve.tangent_at_end();
            let end_cap = cap_points(e.x, e.y, t.x, t.y, head);
            if p.style == ArrowStyle::Double {
                let st = curve.tangent_at_start();
                if let Some(sc) = cap_points(s.x, s.y, -st.x, -st.y, head) {
                    append_cap_figure(&mut g, s, &sc);
                }
            }
            if let Some(ec) = end_cap {
                append_cap_figure(&mut g, e, &ec);
            }
        }
        (ArrowStyle::Modern, _) => match modern_points(s.x, s.y, e.x, e.y, head) {
            Some([sl, sr, wl, wr]) => {
                g.begin_figure(s.x as f64, s.y as f64, true);
                g.line_to(sl.x as f64, sl.y as f64);
                g.line_to(wl.x as f64, wl.y as f64);
                g.line_to(e.x as f64, e.y as f64);
                g.line_to(wr.x as f64, wr.y as f64);
                g.line_to(sr.x as f64, sr.y as f64);
                g.end_figure(true);
            }
            None => degenerate_circle(&mut g, s),
        },
        (ArrowStyle::Basic | ArrowStyle::Line, curved) => {
            let tangent = if curved {
                curve.tangent_at_end()
            } else {
                Point::new(e.x - s.x, e.y - s.y)
            };
            let headpts = base_points(e.x, e.y, tangent.x, tangent.y, head);
            if !curved && headpts.is_none() {
                degenerate_circle(&mut g, s);
            } else {
                if curved {
                    quad(&mut g)
                } else {
                    line(&mut g)
                }
                if let Some((lb, rb)) = headpts {
                    if p.style == ArrowStyle::Basic {
                        g.begin_figure(e.x as f64, e.y as f64, true);
                        g.line_to(lb.x as f64, lb.y as f64);
                        g.line_to(rb.x as f64, rb.y as f64);
                        g.end_figure(true);
                    } else {
                        g.begin_figure(lb.x as f64, lb.y as f64, false);
                        g.line_to(e.x as f64, e.y as f64);
                        g.line_to(rb.x as f64, rb.y as f64);
                        g.end_figure(false);
                    }
                }
            }
        }
    }
    // The curved modern arrow (unreachable: modern arrows don't curve).
    let _ = modern_head;
    g
}

pub fn draw_arrow(pm: &mut Pixmap, c: &Common, p: &ArrowProps) {
    let (stroke, fill) = arrow_geometry(c, p).finish();
    let color = parse_color(&c.stroke_color);
    let pen = Pen {
        color,
        thickness: c.stroke_width as f64,
        dashes: Vec::new(),
        cap: if p.rounded_corners {
            Cap::Round
        } else {
            Cap::Butt
        },
        join: if p.rounded_corners {
            Join::Round
        } else {
            Join::Miter
        },
    };
    draw_geometry(
        pm,
        &AMatrix::translation(0.0, 0.0),
        Some(color),
        &fill,
        Some(&pen),
        &stroke,
    );
}

// --- FreehandAnnotation ----------------------------------------------------

fn freehand_geometry(c: &Common, p: &FreehandProps) -> StreamGeometry {
    let pts = &p.points;
    let mut g = StreamGeometry::new();
    if pts.len() < 2 {
        return g;
    }
    g.begin_figure(pts[0].x as f64, pts[0].y as f64, false);
    if pts.len() == 2 {
        g.line_to(pts[1].x as f64, pts[1].y as f64);
    } else {
        let mid = Point::new((pts[0].x + pts[1].x) / 2.0, (pts[0].y + pts[1].y) / 2.0);
        g.line_to(mid.x as f64, mid.y as f64);
        for i in 1..pts.len() - 1 {
            let c0 = pts[i];
            let n = pts[i + 1];
            let next_mid = Point::new((c0.x + n.x) / 2.0, (c0.y + n.y) / 2.0);
            g.quad_to(
                c0.x as f64,
                c0.y as f64,
                next_mid.x as f64,
                next_mid.y as f64,
            );
        }
        let last = pts[pts.len() - 1];
        g.line_to(last.x as f64, last.y as f64);
    }
    g.end_figure(false);

    if p.show_arrow_head {
        let half = c.stroke_width as f64 * 2.0;
        let tip = pts[pts.len() - 1];
        let head_length = half * 3.0;
        let mut from = pts[0];
        let mut travelled = 0.0f64;
        for i in (0..pts.len() - 1).rev() {
            let next = pts[i + 1];
            travelled += ((next.x - pts[i].x) as f64)
                .powi(2)
                .add_sqrt(((next.y - pts[i].y) as f64).powi(2));
            if travelled >= head_length {
                from = pts[i];
                break;
            }
        }
        if let Some(cap) = cap_points(tip.x, tip.y, tip.x - from.x, tip.y - from.y, half) {
            append_cap_figure(&mut g, tip, &cap);
        }
    }
    g
}

pub fn draw_freehand(pm: &mut Pixmap, c: &Common, p: &FreehandProps) {
    if p.points.len() < 2 {
        return;
    }
    let (stroke, fill) = freehand_geometry(c, p).finish();
    let color = parse_color(&c.stroke_color);
    let pen = Pen {
        color,
        thickness: c.stroke_width as f64,
        dashes: dash_array(&p.border_style),
        cap: line_cap(&p.border_style),
        join: Join::Round,
    };
    draw_geometry(
        pm,
        &AMatrix::translation(0.0, 0.0),
        Some(color),
        &fill,
        Some(&pen),
        &stroke,
    );
}

trait AddSqrt {
    fn add_sqrt(self, other: f64) -> f64;
}

impl AddSqrt for f64 {
    /// `Math.Sqrt(a + b)`.
    fn add_sqrt(self, other: f64) -> f64 {
        (self + other).sqrt()
    }
}

/// The arranged size and transform of an annotation's visual, for the
/// visuals that can carry a drop shadow.
pub(super) fn visual_frame(a: &crate::model::Annotation) -> Option<((f64, f64), AMatrix)> {
    use crate::model::Kind;
    match &a.kind {
        Kind::Rectangle(_) | Kind::Ellipse(_) | Kind::SmartEraser(_) => {
            let (ar, ctm) = arranged(&a.common);
            Some(((ar.width, ar.height), ctm))
        }
        Kind::Line(_) | Kind::Arrow(_) | Kind::Freehand(_) => {
            // A Path at (0, 0): its size is its geometry's bottom-right corner.
            let path = match &a.kind {
                Kind::Line(p) => line_geometry(&a.common, p).finish().0,
                Kind::Arrow(p) => arrow_geometry(&a.common, p).finish().0,
                Kind::Freehand(p) => freehand_geometry(&a.common, p).finish().0,
                _ => unreachable!(),
            };
            let b = path.bounds();
            Some((
                (
                    (b.right.max(0.0) as f64).ceil(),
                    (b.bottom.max(0.0) as f64).ceil(),
                ),
                AMatrix::translation(0.0, 0.0),
            ))
        }
        _ => None,
    }
}
