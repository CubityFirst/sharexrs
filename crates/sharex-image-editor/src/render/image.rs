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
    let sk = |r: (f64, f64, f64, f64)| {
        RRect::ltrb(
            r.0 as f32,
            r.1 as f32,
            (r.0 + r.2) as f32,
            (r.1 + r.3) as f32,
        )
    };
    // The canvas is the whole image, as before, but only the image's
    // device bounds (2 pixels round them for the anti-aliased edges) go
    // to premultiplied RGBA and back: the draw neither reads nor writes
    // the rest, and the round trip leaves a pixel as it was. An image the
    // size of a pointer over a screenshot then costs what the pointer does.
    let (w, h) = (pm.width() as i32, pm.height() as i32);
    let d = sk(dest);
    let corners = [(d.left, d.top), (d.right, d.top), (d.left, d.bottom), (d.right, d.bottom)]
        .map(|(x, y)| (ctm.sx * x + ctm.kx * y + ctm.tx, ctm.ky * x + ctm.sy * y + ctm.ty));
    let (mut l, mut t, mut r, mut b) = (0, 0, w, h);
    if corners.iter().all(|(x, y)| x.is_finite() && y.is_finite()) {
        let fold = |f: fn(f32, f32) -> f32, i: usize| {
            corners.iter().map(|c| if i == 0 { c.0 } else { c.1 }).reduce(f).unwrap_or(0.0)
        };
        l = (fold(f32::min, 0).floor() as i32 - 2).max(0);
        t = (fold(f32::min, 1).floor() as i32 - 2).max(0);
        r = (fold(f32::max, 0).ceil() as i32 + 2).min(w);
        b = (fold(f32::max, 1).ceil() as i32 + 2).min(h);
    }
    if r <= l || b <= t {
        return;
    }
    let mut rgba = vec![0u8; pm.width() as usize * pm.height() as usize * 4];
    for y in t..b {
        for x in l..r {
            let c = pm.pixel(x, y);
            let i = ((y * w + x) * 4) as usize;
            rgba[i..i + 4].copy_from_slice(&[(c >> 16) as u8, (c >> 8) as u8, c as u8, (c >> 24) as u8]);
        }
    }
    let mut bmp = Bitmap::from_premultiplied(pm.width(), pm.height(), rgba);
    {
        let mut canvas = Canvas::new(&mut bmp);
        canvas.aa_clip = sharex_skia::draw::aa_clip();
        canvas.matrix = RMatrix([
            ctm.sx, ctm.kx, ctm.tx, ctm.ky, ctm.sy, ctm.ty, 0.0, 0.0, 1.0,
        ]);
        let image =
            Bitmap::from_premultiplied(bitmap.width(), bitmap.height(), bitmap.to_premul_rgba());
        // DrawingContextImpl.DrawBitmap: white at the opacity, anti-aliased.
        let mut paint = Paint::shader(Shader::Color([255, 255, 255, 255]));
        paint.anti_alias = true;
        canvas.draw_image_rect(
            &image,
            Some(sk(source)),
            sk(dest),
            Sampling::Linear,
            Some(&paint),
        );
    }
    let data = bmp.data();
    for y in t..b {
        for x in l..r {
            let i = ((y * w + x) * 4) as usize;
            let p = &data[i..i + 4];
            *pm.pixel_mut(x, y) = sharex_skia::pixmap::pack(p[3] as u32, p[0] as u32, p[1] as u32, p[2] as u32);
        }
    }
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

#[cfg(test)]
mod bounded_draw_tests {
    use super::*;

    /// `draw_image_control` as it was: the whole image converted for the
    /// canvas and back.
    fn whole_canvas(pm: &mut Pixmap, bounds: (f32, f32, f32, f32), rotation: f32, bitmap: &Pixmap, stretch: Stretch) {
        let src = (bitmap.width() as f64, bitmap.height() as f64);
        let a = arrange_image(bounds.0, bounds.1, bounds.2, bounds.3, stretch, src);
        if a.width <= 0.0 || a.height <= 0.0 {
            return;
        }
        let (sx, sy) = scaling(stretch, (a.width, a.height), src);
        let size2 = (src.0 * sx, src.1 * sy);
        let cx = (a.width - size2.0) / 2.0;
        let cy = (a.height - size2.1) / 2.0;
        let (dl, dt) = (cx.max(0.0), cy.max(0.0));
        let (dr, db) = ((cx + size2.0).min(a.width), (cy + size2.1).min(a.height));
        if dr <= dl || db <= dt {
            return;
        }
        let dest = (dl, dt, dr - dl, db - dt);
        let (sw, sh) = (dest.2 / sx, dest.3 / sy);
        let source = ((src.0 - sw) / 2.0, (src.1 - sh) / 2.0, sw, sh);
        let ctm = layout::visual_transform(&a, rotation).to_sk();
        let mut bmp = Bitmap::from_premultiplied(pm.width(), pm.height(), pm.to_premul_rgba());
        {
            let mut canvas = Canvas::new(&mut bmp);
            canvas.aa_clip = sharex_skia::draw::aa_clip();
            canvas.matrix = RMatrix([ctm.sx, ctm.kx, ctm.tx, ctm.ky, ctm.sy, ctm.ty, 0.0, 0.0, 1.0]);
            let image = Bitmap::from_premultiplied(bitmap.width(), bitmap.height(), bitmap.to_premul_rgba());
            let mut paint = Paint::shader(Shader::Color([255, 255, 255, 255]));
            paint.anti_alias = true;
            let sk = |r: (f64, f64, f64, f64)| RRect::ltrb(r.0 as f32, r.1 as f32, (r.0 + r.2) as f32, (r.1 + r.3) as f32);
            canvas.draw_image_rect(&image, Some(sk(source)), sk(dest), Sampling::Linear, Some(&paint));
        }
        *pm = Pixmap::from_premul_rgba(pm.width(), pm.height(), bmp.data());
    }

    #[test]
    fn drawing_into_the_image_bounds_only_gives_the_whole_canvas_pixels() {
        let mut seed = 0x9e37_79b9u32;
        let mut next = |n: u32| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed % n
        };
        for case in 0..250 {
            let (w, h) = (20 + next(140), 20 + next(110));
            let mut base = Pixmap::new(w, h);
            for p in base.pixels_mut() {
                let a = [255, 255, 128, 0][next(4) as usize];
                let c = next(256) * a / 255;
                *p = (a << 24) | (c << 16) | (((255 - c) * a / 255) << 8) | (c / 2);
            }
            let (iw, ih) = (1 + next(40), 1 + next(40));
            let mut image = Pixmap::new(iw, ih);
            for p in image.pixels_mut() {
                let a = [255, 200, 60, 0][next(4) as usize];
                let c = next(256) * a / 255;
                *p = (a << 24) | (c << 16) | ((c / 3) << 8) | (a - c / 2);
            }
            let x = next(w + 60) as f32 - 30.0 + next(4) as f32 * 0.25;
            let y = next(h + 60) as f32 - 30.0 + next(4) as f32 * 0.25;
            let bounds = (x, y, x + 1.0 + next(70) as f32 + next(3) as f32 * 0.5, y + 1.0 + next(70) as f32 + next(3) as f32 * 0.5);
            let rotation = [0.0, 0.0, 15.0, 45.0, 90.0, 137.5, 200.0, -30.0][next(8) as usize];
            let stretch = if next(2) == 0 { Stretch::Fill } else { Stretch::Uniform };
            // Half the cases as region capture's export draws: a tile's
            // device clip and the pipeline blitters.
            let tile = sharex_skia::IRect::from_ltrb(next(w / 2) as i32, next(h / 2) as i32, (w / 2 + next(w / 2) + 1) as i32, (h / 2 + next(h / 2) + 1) as i32);
            let exported = case % 2 == 1;
            let run = |f: &mut dyn FnMut()| {
                if exported {
                    sharex_skia::blitter::with_device_clip(tile, || sharex_skia::blitter::with_pipeline_blitters(f))
                } else {
                    f()
                }
            };
            let mut expected = base.clone();
            run(&mut || whole_canvas(&mut expected, bounds, rotation, &image, stretch));
            let mut got = base.clone();
            run(&mut || draw_image_control(&mut got, bounds, rotation, &image, stretch));
            assert!(got == expected, "case {case}: {w}x{h}, {iw}x{ih} at {bounds:?}, rotated {rotation}, exported {exported}");
        }
    }
}
