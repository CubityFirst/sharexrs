//! Drawing paths as `SkCanvas::drawPath` / `SkDrawBase::drawPath` does on
//! an N32 surface: strokes are turned into a fill path in local
//! coordinates, then transformed by the CTM and filled with anti-aliasing.

use crate::blitter::ColorBlitter;
use crate::geom::{IRect, Rect};
use crate::path::{FillType, Matrix, Path};
use crate::pixmap::Pixmap;
use crate::scan;

/// An unpremultiplied `SkColor`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color {
    pub a: u8,
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const fn argb(a: u8, r: u8, g: u8, b: u8) -> Self {
        Self { a, r, g, b }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Cap {
    #[default]
    Butt,
    Round,
    Square,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Join {
    #[default]
    Miter,
    Round,
    Bevel,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Stroke {
    pub width: f32,
    pub cap: Cap,
    pub join: Join,
    pub miter_limit: f32,
    /// `SkDashPathEffect` intervals and phase.
    pub dash: Option<(Vec<f32>, f32)>,
}

/// Fills `path` (already in device space) with `color`, anti-aliased.
pub fn fill_path(pm: &mut Pixmap, path: &Path, color: Color, clip: Option<IRect>) {
    let full = IRect::from_ltrb(0, 0, pm.width() as i32, pm.height() as i32);
    let mut c = clip.unwrap_or(full);
    if !c.intersect(&full) {
        return;
    }
    let mut blitter = ColorBlitter::new(pm, color.a, color.r, color.g, color.b, Some(c));
    scan::anti_fill_path(path, c, &mut blitter);
}

/// The device a draw goes to, as far as stroking cares: the CTM and the
/// device clip's bounds.
#[derive(Clone, Copy, Debug)]
pub struct StrokeContext {
    pub ctm: Matrix,
    pub device: Option<IRect>,
}

impl StrokeContext {
    /// No transform and no clip (`SkStrokeRec` with res scale 1, no cull
    /// rect).
    pub const NONE: StrokeContext = StrokeContext {
        ctm: Matrix::IDENTITY,
        device: None,
    };

    pub fn new(ctm: Matrix, device: IRect) -> Self {
        Self {
            ctm,
            device: Some(device),
        }
    }

    /// `SkMatrixPriv::ComputeResScaleForStroking`.
    pub fn res_scale(&self) -> f32 {
        crate::dash::res_scale_for_stroking(&self.ctm)
    }

    /// `SkDrawBase::computeConservativeLocalClipBounds`: the device clip
    /// outset by a pixel, mapped back to local space.
    pub fn cull_rect(&self) -> Option<Rect> {
        let d = self.device?;
        if d.is_empty() {
            return None;
        }
        let m = &self.ctm;
        let sk = crate::raster::matrix::Matrix([m.sx, m.kx, m.tx, m.ky, m.sy, m.ty, 0.0, 0.0, 1.0]);
        let inv = sk.invert()?;
        let r = inv.map_rect(crate::raster::matrix::Rect::ltrb(
            (d.left - 1) as f32,
            (d.top - 1) as f32,
            (d.right + 1) as f32,
            (d.bottom + 1) as f32,
        ));
        Some(Rect::from_ltrb(r.left, r.top, r.right, r.bottom))
    }
}

/// `SkStrokeRec` for a stroke (`width` 0 for a hairline).
fn stroke_rec(stroke: &Stroke, width: f32, ctx: &StrokeContext) -> crate::dash::StrokeRec {
    crate::dash::StrokeRec {
        width,
        cap: stroke.cap,
        join: stroke.join,
        miter: stroke.miter_limit,
        res_scale: ctx.res_scale(),
    }
}

/// The paint's path effect (`SkDashPathEffect`) applied to `path` for a
/// stroke of `width`: the path to stroke, or (true) a path to fill
/// instead. Without a dash, or when Skia rejects it, the path itself.
pub fn dash_path(path: &Path, stroke: &Stroke, width: f32, ctx: &StrokeContext) -> (Path, bool) {
    if let Some((intervals, phase)) = &stroke.dash
        && let Some(effect) = crate::dash::DashEffect::new(intervals, *phase)
        && let Some(out) = effect.filter_path(path, &stroke_rec(stroke, width, ctx), ctx.cull_rect())
    {
        return out;
    }
    (path.clone(), false)
}

/// The fill path of a stroke (`skpathutils::FillPathWithPaint`: the dash
/// effect, then `SkStroke::strokePath`), in the path's own coordinates.
pub fn stroke_to_path(path: &Path, stroke: &Stroke, ctx: &StrokeContext) -> Option<Path> {
    let (src, fill) = dash_path(path, stroke, stroke.width, ctx);
    if fill {
        return Some(src);
    }
    let cap = match stroke.cap {
        Cap::Butt => crate::stroke::Cap::Butt,
        Cap::Round => crate::stroke::Cap::Round,
        Cap::Square => crate::stroke::Cap::Square,
    };
    let join = match stroke.join {
        Join::Miter => crate::stroke::Join::Miter,
        Join::Round => crate::stroke::Join::Round,
        Join::Bevel => crate::stroke::Join::Bevel,
    };
    Some(crate::stroke::stroke_path(
        &src,
        stroke.width,
        stroke.miter_limit,
        cap,
        join,
        ctx.res_scale(),
    ))
}

pub fn from_tiny_skia(p: &tiny_skia::Path) -> Path {
    let mut out = Path::new();
    for seg in p.segments() {
        match seg {
            tiny_skia::PathSegment::MoveTo(a) => {
                out.move_to(a.x, a.y);
            }
            tiny_skia::PathSegment::LineTo(a) => {
                out.line_to(a.x, a.y);
            }
            tiny_skia::PathSegment::QuadTo(a, b) => {
                out.quad_to(a.x, a.y, b.x, b.y);
            }
            tiny_skia::PathSegment::CubicTo(a, b, c) => {
                out.cubic_to(a.x, a.y, b.x, b.y, c.x, c.y);
            }
            tiny_skia::PathSegment::Close => {
                out.close();
            }
        }
    }
    out
}

/// Draws `path` (device space) as an anti-aliased hairline
/// (`SkScan::AntiHairPath` and its square and round cap variants).
pub fn hair_path(pm: &mut Pixmap, path: &Path, cap: Cap, color: Color, clip: Option<IRect>) {
    let full = IRect::from_ltrb(0, 0, pm.width() as i32, pm.height() as i32);
    let mut c = clip.unwrap_or(full);
    if !c.intersect(&full) {
        return;
    }
    let mut blitter = ColorBlitter::new(pm, color.a, color.r, color.g, color.b, Some(c));
    crate::hair::anti_hair_path(path, cap, c, &mut blitter);
}

/// `SkCanvas::drawPath` with a stroke paint: strokes in local space, maps
/// to device space with `ctm`, fills. A stroke no wider than a device
/// pixel is drawn as a hairline instead (`SkDrawTreatAsHairline`).
pub fn stroke_path(
    pm: &mut Pixmap,
    path: &Path,
    stroke: &Stroke,
    ctm: &Matrix,
    color: Color,
    clip: Option<IRect>,
) {
    let full = IRect::from_ltrb(0, 0, pm.width() as i32, pm.height() as i32);
    let mut device = clip.unwrap_or(full);
    if !device.intersect(&full) {
        return;
    }
    let ctx = StrokeContext::new(*ctm, device);
    if let Some(coverage) = crate::hair::hairline_coverage(stroke.width, ctm) {
        let color = Color {
            a: crate::hair::hairline_alpha(color.a, coverage),
            ..color
        };
        let (src, fill) = dash_path(path, stroke, 0.0, &ctx);
        if fill {
            fill_path(pm, &src.transformed(ctm), color, clip);
        } else {
            hair_path(pm, &src.transformed(ctm), stroke.cap, color, clip);
        }
        return;
    }
    if let Some(mut fill) = stroke_to_path(path, stroke, &ctx) {
        fill.fill_type = FillType::Winding;
        let dev = fill.transformed(ctm);
        fill_path(pm, &dev, color, clip);
    }
}
