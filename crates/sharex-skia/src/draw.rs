//! Drawing paths as `SkCanvas::drawPath` / `SkDrawBase::drawPath` does on
//! an N32 surface: strokes are turned into a fill path in local
//! coordinates, then transformed by the CTM and filled with anti-aliasing.

use crate::blitter::ColorBlitter;
use crate::geom::IRect;
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

/// The fill path of a stroke (`SkStroke::strokePath`), in the path's own
/// coordinates. Dashes still go through tiny-skia's dasher.
pub fn stroke_to_path(path: &Path, stroke: &Stroke) -> Option<Path> {
    let dashed;
    let src = if let Some((intervals, phase)) = &stroke.dash {
        let dash = tiny_skia::StrokeDash::new(intervals.clone(), *phase)?;
        dashed = from_tiny_skia(&path.to_tiny_skia()?.dash(&dash, 1.0)?);
        &dashed
    } else {
        path
    };
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
        src,
        stroke.width,
        stroke.miter_limit,
        cap,
        join,
        1.0,
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

/// `SkCanvas::drawPath` with a stroke paint: strokes in local space, maps
/// to device space with `ctm`, fills.
pub fn stroke_path(
    pm: &mut Pixmap,
    path: &Path,
    stroke: &Stroke,
    ctm: &Matrix,
    color: Color,
    clip: Option<IRect>,
) {
    if let Some(mut fill) = stroke_to_path(path, stroke) {
        fill.fill_type = FillType::Winding;
        let dev = fill.transformed(ctm);
        fill_path(pm, &dev, color, clip);
    }
}
