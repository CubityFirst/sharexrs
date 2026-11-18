//! The image-backed annotations (`ImageAnnotation` and the Clone tool's
//! images, `EmojiAnnotation`, `CursorAnnotation`): each is an Avalonia
//! `Image` control on the annotation canvas (`*.Visual.cs`), laid out by
//! Avalonia (`Layoutable.MeasureCore`/`ArrangeCore`, `Image.ArrangeOverride`)
//! and drawn by `Image.Render` → `DrawingContextImpl.DrawBitmap` → Skia's
//! `drawImageRect` (bilinear: `BitmapInterpolationMode.Unspecified`;
//! anti-aliased edges). And `SmartEraserAnnotation`, a `Rectangle` filled
//! with its sampled colour or a stretched edge (`ApplyFill`).

use sharex_skia::Pixmap;
use sharex_skia::raster::{
    Bitmap, Canvas, Matrix as RMatrix, Paint, Rect as RRect, Sampling, Shader,
};

use super::layout::{self, Arranged};
use crate::model::{Common, SmartEraserFillMode, SmartEraserProps};

/// `Avalonia.Media.Stretch` as the image annotations use it.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Stretch {
    /// `ImageAnnotation.CreateVisual`: the image fills its bounds.
    Fill,
    /// `Image`'s default, kept by the emoji and cursor visuals.
    Uniform,
}

/// `Stretch.CalculateScaling` (`StretchDirection.Both`, finite sizes).
fn scaling(stretch: Stretch, dest: (f64, f64), src: (f64, f64)) -> (f64, f64) {
    let is_zero = |v: f64| v.abs() < 2.220_446_049_250_313e-15;
    let sx = if is_zero(src.0) { 0.0 } else { dest.0 / src.0 };
    let sy = if is_zero(src.1) { 0.0 } else { dest.1 / src.1 };
    match stretch {
        Stretch::Fill => (sx, sy),
        Stretch::Uniform => {
            let s = if sx < sy { sx } else { sy };
            (s, s)
        }
    }
}

/// Where the `Image` control of an annotation with bounds `(l, t, r, b)`
/// ends up: `Canvas.Left/Top` = the bounds' corner, `Width/Height` = their
/// size (at least 1), measured (`DesiredSize` rounded up) and arranged by
/// the canvas (`ArrangeCore`: the arranged size from `ArrangeOverride`,
/// centred in the slot, its position rounded).
fn arrange_image(l: f32, t: f32, r: f32, b: f32, stretch: Stretch, src: (f64, f64)) -> Arranged {
    let w = ((r - l) as f64).max(1.0);
    let h = ((b - t) as f64).max(1.0);
    // MeasureCore: the explicit Width/Height, rounded up.
    let slot = (w.ceil(), h.ceil());
    // ArrangeCore: constraints = Width/Height, rounded up; ArrangeOverride
    // = Stretch.CalculateSize(constraints, source), constrained.
    let constraints = (w.ceil(), h.ceil());
    let (sx, sy) = scaling(stretch, constraints, src);
    let size = (
        (src.0 * sx).min(constraints.0),
        (src.1 * sy).min(constraints.1),
    );
    let x = l as f64 + (slot.0 - size.0) / 2.0;
    let y = t as f64 + (slot.1 - size.1) / 2.0;
    Arranged {
        x: x.round_ties_even(),
        y: y.round_ties_even(),
        width: size.0,
        height: size.1,
    }
}

/// An `Image` control showing `bitmap` over the bounds `(l, t, r, b)`,
/// rotated about its centre by `rotation` degrees (`ApplyRotationTransform`).
pub(super) fn draw_image_control(
    pm: &mut Pixmap,
    bounds: (f32, f32, f32, f32),
    rotation: f32,
    bitmap: &Pixmap,
    stretch: Stretch,
) {
    if bitmap.width() == 0 || bitmap.height() == 0 {
        return;
    }
    let src = (bitmap.width() as f64, bitmap.height() as f64);
    let a = arrange_image(bounds.0, bounds.1, bounds.2, bounds.3, stretch, src);
    if a.width <= 0.0 || a.height <= 0.0 {
        return;
    }
    // Image.Render.
    let (sx, sy) = scaling(stretch, (a.width, a.height), src);
    let size2 = (src.0 * sx, src.1 * sy);
    // rect.CenterRect(new Rect(size2)).Intersect(rect)
    let cx = (a.width - size2.0) / 2.0;
    let cy = (a.height - size2.1) / 2.0;
    let dl = cx.max(0.0);
    let dt = cy.max(0.0);
    let dr = (cx + size2.0).min(a.width);
    let db = (cy + size2.1).min(a.height);
    if dr <= dl || db <= dt {
        return;
    }
    let dest = (dl, dt, dr - dl, db - dt);
    // new Rect(sourceSize).CenterRect(new Rect(destRect.Size / scaling))
    let (sw, sh) = (dest.2 / sx, dest.3 / sy);
    let source = ((src.0 - sw) / 2.0, (src.1 - sh) / 2.0, sw, sh);

    let ctm = layout::visual_transform(&a, rotation).to_sk();
    let mut bmp = Bitmap::from_premultiplied(pm.width(), pm.height(), pm.to_premul_rgba());
    {
        let mut canvas = Canvas::new(&mut bmp);
        canvas.matrix = RMatrix([
            ctm.sx, ctm.kx, ctm.tx, ctm.ky, ctm.sy, ctm.ty, 0.0, 0.0, 1.0,
        ]);
        let image =
            Bitmap::from_premultiplied(bitmap.width(), bitmap.height(), bitmap.to_premul_rgba());
        // DrawingContextImpl.DrawBitmap: white at the opacity, anti-aliased.
        let mut paint = Paint::shader(Shader::Color([255, 255, 255, 255]));
        paint.anti_alias = true;
        let sk = |r: (f64, f64, f64, f64)| {
            RRect::ltrb(
                r.0 as f32,
                r.1 as f32,
                (r.0 + r.2) as f32,
                (r.1 + r.3) as f32,
            )
        };
        canvas.draw_image_rect(
            &image,
            Some(sk(source)),
            sk(dest),
            Sampling::Linear,
            Some(&paint),
        );
    }
    *pm = Pixmap::from_premul_rgba(pm.width(), pm.height(), bmp.data());
}

/// `SKBitmap.SetPixel` of a straight colour: premultiplied in floats
/// (`SkPixmap::erase`), rounded.
pub(super) fn premultiply_straight(c: [u8; 4]) -> [u8; 4] {
    let a = c[3] as f32 / 255.0;
    let ch = |v: u8| ((v as f32 / 255.0 * a) * 255.0 + 0.5) as u8;
    [ch(c[0]), ch(c[1]), ch(c[2]), c[3]]
}

/// `SmartEraserAnnotation.UnpackColor` then `SetPixel`: a packed straight
/// ARGB colour, premultiplied (RGBA).
fn premultiplied(c: u32) -> [u8; 4] {
    premultiply_straight([(c >> 16) as u8, (c >> 8) as u8, c as u8, (c >> 24) as u8])
}

/// The smart eraser's `Rectangle` (`RectangleAnnotation.CreateVisual`,
/// no stroke) with its fill (`ApplyFill`): the fill colour, or the stored
/// edge as a 1-pixel-wide (or -high) bitmap stretched over the rectangle
/// through an `ImageBrush` with `BitmapInterpolationMode.None`.
pub(super) fn draw_smart_eraser(pm: &mut Pixmap, c: &Common, p: &SmartEraserProps) {
    let edge = match (&p.fill_mode, &p.edge_pixels) {
        (SmartEraserFillMode::SolidColor, _) | (_, None) => None,
        (_, Some(px)) if px.is_empty() => None,
        (mode, Some(px)) => Some((mode.clone(), px)),
    };
    let Some((mode, pixels)) = edge else {
        super::shapes::draw_rectangle(pm, c, p.border_style.clone(), p.corner_radius);
        return;
    };
    let n = pixels.len() as u32;
    let (w, h) = if mode == SmartEraserFillMode::StretchVertically {
        (n, 1)
    } else {
        (1, n)
    };
    let mut data = Vec::with_capacity((w * h * 4) as usize);
    for &px in pixels {
        data.extend_from_slice(&premultiplied(px));
    }
    let edge = Pixmap::from_premul_rgba(w, h, &data);
    super::effects::draw_brush_rectangle(
        pm,
        &edge,
        c,
        &p.border_style,
        p.corner_radius,
        Sampling::Nearest,
    );
}
