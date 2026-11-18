//! `SKCanvas` shape calls as Skia's raster device runs them (`SkCanvas` →
//! `SkBitmapDevice` → `SkDraw`), on top of `sharex_skia`'s canvas.

use sharex_skia::raster::{Canvas, Matrix, Paint, Rect, Shader};
use sharex_skia::{Cap, Direction, Join, Path};

use super::color::SkColor;

/// The stroke half of an `SKPaint` (`Style = Stroke`). SkiaSharp's defaults:
/// width 0 (a hairline), butt caps, miter joins, miter limit 4.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Pen {
    pub width: f32,
    pub cap: Cap,
    pub join: Join,
    pub miter: f32,
}

impl Pen {
    pub fn new(width: f32) -> Self {
        Pen {
            width,
            cap: Cap::Butt,
            join: Join::Miter,
            miter: 4.0,
        }
    }

    pub fn cap(mut self, cap: Cap) -> Self {
        self.cap = cap;
        self
    }

    pub fn join(mut self, join: Join) -> Self {
        self.join = join;
        self
    }
}

/// An `SKPaint { Color = color, IsAntialias = aa }`.
pub(crate) fn solid(color: SkColor, aa: bool) -> Paint<'static> {
    let mut p = Paint::shader(Shader::Color(color.rgba_array()));
    p.anti_alias = aa;
    p
}

/// `SkDraw::drawPath` with a stroke paint: a hairline when the width is 0 or
/// (anti-aliased) no wider than a device pixel (`SkDrawTreatAsHairline`),
/// else `SkStroke` and a winding fill of the outline.
pub(crate) fn stroke_path(canvas: &mut Canvas, path: &Path, pen: &Pen, paint: &Paint) {
    let s = sharex_skia::Stroke {
        width: pen.width,
        cap: pen.cap,
        join: pen.join,
        miter_limit: pen.miter,
        dash: None,
    };
    if pen.width == 0.0 || (paint.anti_alias && canvas.is_hairline(pen.width)) {
        // A non-anti-aliased hairline (SkScan::HairPath) isn't ported; none of
        // ShareX's effects draw one.
        assert!(paint.anti_alias, "non-anti-aliased hairlines aren't ported");
        canvas.stroke_hairline(path, pen.width, pen.cap, paint);
        return;
    }
    let ctx = canvas.stroke_context();
    if let Some(mut outline) = sharex_skia::draw::stroke_to_path(path, &s, &ctx) {
        outline.fill_type = sharex_skia::FillType::Winding;
        canvas.fill_sk_path(&outline, paint);
    }
}

/// `SKCanvas.DrawPath(path, paint)`: a fill, or a stroke with `pen`.
pub(crate) fn draw_path(canvas: &mut Canvas, path: &Path, paint: &Paint, pen: Option<&Pen>) {
    match pen {
        Some(pen) => stroke_path(canvas, path, pen, paint),
        None => canvas.fill_sk_path(path, paint),
    }
}

/// `SKCanvas.DrawLine`: `drawPoints(kLines)`, which strokes `moveTo/lineTo`
/// (whatever the paint's style).
pub(crate) fn draw_line(canvas: &mut Canvas, x0: f32, y0: f32, x1: f32, y1: f32, pen: &Pen, paint: &Paint) {
    let mut path = Path::new();
    path.move_to(x0, y0);
    path.line_to(x1, y1);
    stroke_path(canvas, &path, pen, paint);
}

/// `SKCanvas.DrawOval`: `SkPath::Oval(sorted rect)` (clockwise, start 1).
pub(crate) fn draw_oval(canvas: &mut Canvas, oval: Rect, paint: &Paint, pen: Option<&Pen>) {
    let r = Rect::ltrb(
        oval.left.min(oval.right),
        oval.top.min(oval.bottom),
        oval.left.max(oval.right),
        oval.top.max(oval.bottom),
    );
    let mut path = Path::new();
    path.add_oval(sharex_skia::Rect::from_ltrb(r.left, r.top, r.right, r.bottom), Direction::Cw);
    draw_path(canvas, &path, paint, pen);
}

/// `SKCanvas.DrawCircle`: the oval of the circle's bounds.
pub(crate) fn draw_circle(canvas: &mut Canvas, cx: f32, cy: f32, radius: f32, paint: &Paint, pen: Option<&Pen>) {
    let r = radius.max(0.0);
    draw_oval(canvas, Rect::ltrb(cx - r, cy - r, cx + r, cy + r), paint, pen);
}

/// `SKCanvas.DrawRect` (fill or stroke): `SkDraw::drawRect`'s rect types.
pub(crate) fn draw_rect(canvas: &mut Canvas, rect: Rect, paint: &Paint, pen: Option<&Pen>) {
    match pen {
        None => canvas.draw_rect(rect, paint),
        Some(pen) if pen.width > 0.0 && pen.join == Join::Miter && pen.miter >= std::f32::consts::SQRT_2 => {
            canvas.stroke_rect(rect, pen.width, paint)
        }
        Some(pen) => {
            let r = Rect::ltrb(
                rect.left.min(rect.right),
                rect.top.min(rect.bottom),
                rect.left.max(rect.right),
                rect.top.max(rect.bottom),
            );
            let mut path = Path::new();
            path.add_rect(sharex_skia::Rect::from_ltrb(r.left, r.top, r.right, r.bottom), Direction::Cw, 0);
            stroke_path(canvas, &path, pen, paint);
        }
    }
}

/// `SKCanvas.DrawRect(x, y, w, h, paint)`.
pub(crate) fn draw_rect_xywh(canvas: &mut Canvas, x: f32, y: f32, w: f32, h: f32, paint: &Paint, pen: Option<&Pen>) {
    draw_rect(canvas, Rect::ltrb(x, y, x + w, y + h), paint, pen);
}

/// `SKCanvas.DrawRoundRect(rect, rx, ry, paint)`: a rect when either radius is
/// 0, an oval when `setRectXY` makes one, else `SkPath::RRect`.
pub(crate) fn draw_round_rect(canvas: &mut Canvas, rect: Rect, rx: f32, ry: f32, paint: &Paint, pen: Option<&Pen>) {
    let r = Rect::ltrb(
        rect.left.min(rect.right),
        rect.top.min(rect.bottom),
        rect.left.max(rect.right),
        rect.top.max(rect.bottom),
    );
    if !(rx > 0.0 && ry > 0.0) || r.width() <= 0.0 || r.height() <= 0.0 {
        draw_rect(canvas, r, paint, pen);
        return;
    }
    // SkRRect::setRectXY: radii scaled to fit; one that's all corners is an oval
    // (SkCanvas::onDrawRRect draws it with drawOval).
    let (w, h) = (r.width(), r.height());
    let (mut rx, mut ry) = (rx, ry);
    if rx + rx > w || ry + ry > h {
        let scale = (w / (rx + rx)).min(h / (ry + ry));
        rx *= scale;
        ry *= scale;
    }
    if rx >= w * 0.5 && ry >= h * 0.5 {
        draw_oval(canvas, r, paint, pen);
        return;
    }
    let mut path = Path::new();
    path.add_round_rect(sharex_skia::Rect::from_ltrb(r.left, r.top, r.right, r.bottom), rx, ry, Direction::Cw);
    draw_path(canvas, &path, paint, pen);
}

/// `SKCanvas.RotateDegrees(degrees, px, py)`: `SkMatrix::setRotate(degrees, px, py)`
/// (`setSinCos` with snapped sine and cosine) concatenated.
pub(crate) fn rotate_about(canvas: &mut Canvas, degrees: f32, px: f32, py: f32) {
    let rad = degrees * (std::f32::consts::PI / 180.0);
    let snap = |v: f32| if v.abs() <= 1.0 / 4096.0 { 0.0 } else { v };
    let (s, c) = (snap(rad.sin()), snap(rad.cos()));
    let omc = 1.0 - c;
    let m = Matrix([c, -s, s * py + omc * px, s, c, -s * px + omc * py, 0.0, 0.0, 1.0]);
    if !m.is_identity() {
        canvas.matrix = canvas.matrix.m44_pre_concat(&m);
    }
}
