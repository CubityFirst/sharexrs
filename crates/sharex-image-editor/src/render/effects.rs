//! The effect annotations (`Core/Annotations/Effects`): each computes an
//! effect bitmap from the source image (`UpdateEffectFromInteractionCache`,
//! the path the editor takes when it builds their visuals), which its
//! `Rectangle` visual fills with an `ImageBrush` placed 1:1 on the image's
//! pixel grid (`AnnotationEffectVisualUpdater.ApplyEffectBrush`).

use sharex_skia::Pixmap;
use sharex_skia::raster::Sampling;

use super::color::parse_color;
use crate::model::{Annotation, Kind};

/// `BaseEffectAnnotation.GetEffectPixelBounds`: every image pixel the
/// annotation touches.
fn effect_pixel_bounds(a: &Annotation) -> (i32, i32, i32, i32) {
    let (l, t, r, b) = super::shapes::bounds(&a.common);
    (
        l.floor() as i32,
        t.floor() as i32,
        r.ceil() as i32,
        b.ceil() as i32,
    )
}

pub fn draw_effect(pm: &mut Pixmap, base: &Pixmap, a: &Annotation, cache: &mut super::Cache) {
    if a.common.rotation_angle != 0.0 {
        // Rotated effects (UpdateEffectFromProcessedSource) aren't ported.
        return;
    }
    match &a.kind {
        Kind::Highlight(_) => draw_highlight(pm, base, a),
        Kind::Blur(p) => {
            let source = cache.source(base, 0, p.amount, blurred_source);
            draw_aligned_cache(pm, source, a);
        }
        Kind::Pixelate(p) => {
            let source = cache.source(base, 1, p.amount, pixelated_source);
            draw_aligned_cache(pm, source, a);
        }
        Kind::Magnify(p) => draw_magnify(pm, base, a, p),
        _ => {}
    }
}

/// `MagnifyAnnotation.UpdateEffectCore` (unrotated): the middle of the
/// area under the annotation, drawn `Amount` times bigger (Mitchell),
/// filling the visual's `Rectangle`, with its optional border.
fn draw_magnify(pm: &mut Pixmap, base: &Pixmap, a: &Annotation, p: &crate::model::MagnifyProps) {
    use sharex_skia::{IRect, Rect};
    let (l, t, r, b) = super::shapes::bounds(&a.common);
    let full_w = (r - l) as i32;
    let full_h = (b - t) as i32;
    if full_w <= 0 || full_h <= 0 {
        return;
    }
    let image = IRect::from_ltrb(0, 0, base.width() as i32, base.height() as i32);
    let annotation_rect = IRect::from_ltrb(l as i32, t as i32, r as i32, b as i32);
    let mut valid = annotation_rect;
    let mut effect = Pixmap::new(full_w as u32, full_h as u32);
    if valid.intersect(&image) {
        let zoom = p.amount.max(1.0);
        let capture_w = valid.width() as f32 / zoom;
        let capture_h = valid.height() as f32 / zoom;
        let cx = valid.left as f32 + valid.width() as f32 / 2.0;
        let cy = valid.top as f32 + valid.height() as f32 / 2.0;
        let capture_x = cx - capture_w / 2.0;
        let capture_y = cy - capture_h / 2.0;
        let mut capture = IRect::from_ltrb(
            capture_x as i32,
            capture_y as i32,
            (capture_x + capture_w) as i32,
            (capture_y + capture_h) as i32,
        );
        if capture.intersect(&image) {
            let draw_x = valid.left - annotation_rect.left;
            let draw_y = valid.top - annotation_rect.top;
            sharex_skia::resample::draw_image_rect_mitchell(
                &mut effect,
                base,
                Rect::from_ltrb(
                    capture.left as f32,
                    capture.top as f32,
                    capture.right as f32,
                    capture.bottom as f32,
                ),
                IRect::from_ltrb(
                    draw_x,
                    draw_y,
                    draw_x + valid.width(),
                    draw_y + valid.height(),
                ),
            );
        }
    }
    // ApplyMagnifyShape: the visual is the annotation's Rectangle, clipped to
    // an ellipse of its bounds when IsEllipse, with a border (a transparent
    // one has no width) whose corners are rounded into an ellipse too.
    let color = parse_color(&a.common.stroke_color);
    let thickness = if color.a == 0 {
        0.0
    } else {
        a.common.stroke_width.max(0.0) as f64
    };
    let (width, height) = ((r - l) as f64, (b - t) as f64);
    let round = p.is_ellipse && thickness > 0.0;
    let radii = if round {
        (
            (width - thickness).max(0.0) / 2.0,
            (height - thickness).max(0.0) / 2.0,
        )
    } else {
        (0.0, 0.0)
    };
    let pen = (thickness > 0.0).then(|| super::shapes::Pen {
        color,
        thickness,
        dashes: super::shapes::dash_array(&p.border_style),
        cap: super::shapes::line_cap(&p.border_style),
        join: sharex_skia::Join::Miter,
    });
    let clip = p.is_ellipse.then_some((width, height));
    draw_effect_rectangle(pm, &effect, &a.common, radii, pen.as_ref(), clip, Sampling::Linear);
}

/// A `Rectangle` (`RectangleAnnotation.CreateVisual`: its stroke, its
/// corner radius) filled with `brush` through an `ImageBrush` stretched
/// over it, the intermediate image sampled with `sampling` (the
/// rectangle's `BitmapInterpolationMode`).
pub(super) fn draw_brush_rectangle(
    pm: &mut Pixmap,
    brush: &Pixmap,
    c: &crate::model::Common,
    border_style: &crate::model::BorderStyle,
    corner_radius: i32,
    sampling: Sampling,
) {
    let radius = corner_radius.max(0) as f64;
    let pen = super::shapes::Pen {
        color: parse_color(&c.stroke_color),
        thickness: c.stroke_width as f64,
        dashes: super::shapes::dash_array(border_style),
        cap: super::shapes::line_cap(border_style),
        join: sharex_skia::Join::Miter,
    };
    draw_effect_rectangle(pm, brush, c, (radius, radius), Some(&pen), None, sampling);
}

/// An effect annotation's `Rectangle` visual as Avalonia renders it
/// (`Shape.Render` → `DrawGeometry`): its geometry (`RectangleGeometry` of
/// the bounds deflated by half the stroke, rounded with `radii` through
/// `GeometryBuilder.DrawRoundedCornersRectangle`) filled with the effect
/// bitmap's `ImageBrush` (`Stretch.Fill` over the geometry's bounds:
/// `ConfigureTileBrush`'s intermediate image, then a decal image shader),
/// then stroked with `pen`; all inside an anti-aliased elliptical clip
/// (`Control.Clip = EllipseGeometry(0, 0, w, h)`) when `clip` is given.
fn draw_effect_rectangle(
    pm: &mut Pixmap,
    effect: &Pixmap,
    common: &crate::model::Common,
    radii: (f64, f64),
    pen: Option<&super::shapes::Pen>,
    clip: Option<(f64, f64)>,
    sampling: Sampling,
) {
    use sharex_skia::raster::{Bitmap, Canvas, Matrix as RMatrix, Paint, Rect as RRect, Shader, TileMode};
    let (ar, ctm) = super::shapes::arranged(common);
    let thickness = pen.map_or(0.0, |p| p.thickness);

    // Rectangle.CreateDefiningGeometry
    let h = thickness / 2.0;
    let gx = h;
    let gy = h;
    let gw = (ar.width - h - h).max(0.0);
    let gh = (ar.height - h - h).max(0.0);
    let (rx, ry) = radii;
    let path = if rx == 0.0 && ry == 0.0 {
        let mut p = sharex_skia::Path::new();
        p.add_rect(
            sharex_skia::Rect::from_ltrb(gx as f32, gy as f32, (gx + gw) as f32, (gy + gh) as f32),
            sharex_skia::Direction::Cw,
            0,
        );
        p
    } else {
        let (right, bottom) = (gx + gw, gy + gh);
        let mut g = super::shapes::StreamGeometry::new();
        g.begin_figure(gx + rx, gy, true);
        g.line_to(right - rx, gy);
        g.arc_to(right, gy + ry, rx, ry, false, true);
        g.line_to(right, bottom - ry);
        g.arc_to(right - rx, bottom, rx, ry, false, true);
        g.line_to(gx + rx, bottom);
        g.arc_to(gx, bottom - ry, rx, ry, false, true);
        g.line_to(gx, gy + ry);
        g.arc_to(gx + rx, gy, rx, ry, false, true);
        g.end_figure(true);
        g.finish().0
    };
    // Geometry.Bounds: the path's tight bounds.
    let tb = path.tight_bounds();
    let target = (
        tb.left as f64,
        tb.top as f64,
        (tb.right - tb.left) as f64,
        (tb.bottom - tb.top) as f64,
    );

    let mut bmp = Bitmap::from_premultiplied(pm.width(), pm.height(), pm.to_premul_rgba());
    let m = ctm.to_sk();
    let mut canvas = Canvas::new(&mut bmp);
    canvas.matrix = RMatrix([m.sx, m.kx, m.tx, m.ky, m.sy, m.ty, 0.0, 0.0, 1.0]);
    if let Some((w, h)) = clip {
        let mut oval = sharex_skia::Path::new();
        oval.add_oval(
            sharex_skia::Rect::from_ltrb(0.0, 0.0, w as f32, h as f32),
            sharex_skia::Direction::Cw,
        );
        canvas.clip_path_aa(&oval);
    }

    // The fill: ConfigureTileBrush with TileMode None, Stretch Fill.
    let content = (effect.width() as f64, effect.height() as f64);
    let (tw, th) = (target.2, target.3);
    if tw > 0.0 && th > 0.0 && content.0 > 0.0 && content.1 > 0.0 {
        let scale = (tw / content.0, th / content.1);
        // CalculateTranslate (centred).
        let tx = (tw - content.0 * scale.0) / 2.0;
        let ty = (th - content.1 * scale.1) / 2.0;
        let (iw, ih) = (tw.ceil() as u32, th.ceil() as u32);
        let mut inter = Bitmap::new(iw, ih);
        {
            let source = Bitmap::from_premultiplied(
                effect.width(),
                effect.height(),
                effect.to_premul_rgba(),
            );
            let mut c = Canvas::new(&mut inter);
            c.matrix = RMatrix([
                scale.0 as f32,
                0.0,
                tx as f32,
                0.0,
                scale.1 as f32,
                ty as f32,
                0.0,
                0.0,
                1.0,
            ]);
            let mut paint = Paint::shader(Shader::Color([255, 255, 255, 255]));
            paint.anti_alias = true;
            let r = RRect::xywh(0.0, 0.0, content.0 as f32, content.1 as f32);
            // DrawBitmap with the visual's RenderOptions (the default
            // interpolation is linear).
            c.draw_image_rect(&source, Some(r), r, sampling, Some(&paint));
        }
        let mut fill = path.clone();
        fill.fill_type = sharex_skia::FillType::EvenOdd;
        let mut paint = Paint::shader(Shader::Image {
            image: &inter,
            matrix: RMatrix::translate(target.0 as f32, target.1 as f32),
            sampling: Sampling::Nearest,
            tile_x: TileMode::Decal,
            tile_y: TileMode::Decal,
        });
        paint.anti_alias = true;
        canvas.fill_sk_path(&fill, &paint);
    }

    if let Some(pen) = pen
        && pen.color.a != 0
        && let Some(stroke) = pen.stroke()
    {
        let ctx = canvas.stroke_context();
        if let Some(mut outline) = sharex_skia::draw::stroke_to_path(&path, &stroke, &ctx) {
            outline.fill_type = sharex_skia::FillType::Winding;
            let c = pen.color;
            canvas.fill_sk_path(&outline, &Paint::fill([c.r, c.g, c.b, c.a]));
        }
    }
    drop(canvas);
    *pm = Pixmap::from_premul_rgba(pm.width(), pm.height(), bmp.data());
}

/// `BlurAnnotation.CreateBlurredSourceCache`: the whole image, its edges
/// extended (a clamp shader) by 3 sigma, blurred, cropped back.
pub(super) fn blurred_source(source: &Pixmap, amount: f32) -> Pixmap {
    let sigma = amount.max(0.0);
    if sigma <= 0.0 {
        return source.clone();
    }
    let padding = ((sigma * 3.0) as f64).ceil() as i32;
    let (w, h) = (source.width() as i32, source.height() as i32);
    let (ew, eh) = (w + 2 * padding, h + 2 * padding);
    let mut extended = Pixmap::new(ew as u32, eh as u32);
    for y in 0..eh {
        for x in 0..ew {
            let sx = (x - padding).clamp(0, w - 1);
            let sy = (y - padding).clamp(0, h - 1);
            *extended.pixel_mut(x, y) = source.pixel(sx, sy);
        }
    }
    let blurred = sharex_skia::blur::blur(&extended, sigma, sigma);
    let mut out = Pixmap::new(w as u32, h as u32);
    for y in 0..h {
        for x in 0..w {
            *out.pixel_mut(x, y) = blurred.pixel(x + padding, y + padding);
        }
    }
    out
}

/// `PixelateAnnotation.CreatePixelatedSourceCache`: scaled down by the
/// pixel size (linear), then back up (nearest).
fn pixelated_source(source: &Pixmap, amount: f32) -> Pixmap {
    let pixel_size = amount.max(1.0) as u32;
    let w = (source.width() / pixel_size).max(1);
    let h = (source.height() / pixel_size).max(1);
    let small = sharex_skia::resample::resize_linear(source, w, h);
    sharex_skia::resample::resize_nearest(&small, source.width(), source.height())
}

/// `UpdateEffectFromAlignedCache` and the visual's `ImageBrush`: the
/// cache's pixels under the annotation, drawn 1:1 (srcover).
fn draw_aligned_cache(pm: &mut Pixmap, cache: &Pixmap, a: &Annotation) {
    let (l, t, r, b) = effect_pixel_bounds(a);
    let (w, h) = (cache.width() as i32, cache.height() as i32);
    for y in t.max(0)..b.min(h) {
        for x in l.max(0)..r.min(w) {
            let src = cache.pixel(x, y);
            let d = pm.pixel_mut(x, y);
            *d = src_over(src, *d);
        }
    }
}

/// `HighlightAnnotation.Highlight`.
fn highlight(value: u8, limit: u8, alpha: i32) -> u8 {
    let h = value.min(limit);
    if alpha >= 255 {
        return h;
    }
    (value as i32 - (((value as i32 - h as i32) * alpha) + 127) / 255) as u8
}

/// The highlight: each pixel's channels capped at the colour's
/// (`ApplyHighlightToBitmap`, on the premultiplied pixels), applied as
/// strongly as the colour is opaque.
fn draw_highlight(pm: &mut Pixmap, base: &Pixmap, a: &Annotation) {
    let color = parse_color(&a.common.fill_color);
    if color.a == 0 {
        return;
    }
    let (l, t, r, b) = effect_pixel_bounds(a);
    let (w, h) = (base.width() as i32, base.height() as i32);
    for y in t.max(0)..b.min(h) {
        for x in l.max(0)..r.min(w) {
            let p = base.pixel(x, y);
            let (pa, pr, pg, pb) = ((p >> 24) as u8, (p >> 16) as u8, (p >> 8) as u8, p as u8);
            let alpha = color.a as i32;
            let nr = highlight(pr, color.r, alpha);
            let ng = highlight(pg, color.g, alpha);
            let nb = highlight(pb, color.b, alpha);
            let src = ((pa as u32) << 24) | ((nr as u32) << 16) | ((ng as u32) << 8) | nb as u32;
            let d = pm.pixel_mut(x, y);
            *d = src_over(src, *d);
        }
    }
}

/// `SkPMSrcOver`.
pub(super) fn src_over(src: u32, dst: u32) -> u32 {
    let a = src >> 24;
    if a == 255 {
        return src;
    }
    let scale = 256 - a;
    let mask = 0x00FF_00FFu32;
    let mut rb = (((dst & mask) * scale) >> 8) & mask;
    let mut ag = (((dst >> 8) & mask) * scale) & !mask;
    rb += src & mask;
    ag += src & !mask;
    (rb & 0x1FF).min(0xFF)
        | (ag & 0x1FF00).min(0xFF00)
        | (rb & 0x01FF_0000).min(0x00FF_0000)
        | (ag & 0xFF00_0000)
}

/// The shared spotlight overlay (`SpotlightOverlayControl.UpdateSpotlights`),
/// drawn over the image below every annotation: the (optionally blurred)
/// image darkened, with each spotlight cleared out of it.
pub fn draw_spotlights(pm: &mut Pixmap, base: &Pixmap, annotations: &[Annotation], cache: &mut super::Cache) {
    let spots: Vec<(&Annotation, &crate::model::SpotlightProps)> = annotations
        .iter()
        .filter_map(|a| match &a.kind {
            Kind::Spotlight(p) => Some((a, p)),
            _ => None,
        })
        .collect();
    if spots.is_empty() {
        return;
    }
    let (w, h) = (base.width(), base.height());
    let darken = spots.iter().map(|s| s.1.darken_opacity).max().unwrap_or(0);
    let blur = spots
        .iter()
        .map(|s| s.1.blur_amount)
        .fold(f32::MIN, f32::max);
    let mut overlay = if blur > 0.0 {
        cache.source(base, 0, blur, blurred_source).clone()
    } else {
        Pixmap::new(w, h)
    };
    {
        let mut dark = sharex_skia::blitter::ColorBlitter::new(&mut overlay, darken, 0, 0, 0, None);
        sharex_skia::blitter::Blitter::blit_rect(&mut dark, 0, 0, w as i32, h as i32);
    }
    for (a, p) in &spots {
        let (l, t, r, b) = super::shapes::bounds(&a.common);
        let rect = sharex_skia::Rect::from_ltrb(l, t, r, b);
        let mut clear = sharex_skia::blitter::ClearBlitter::new(&mut overlay);
        let clip = sharex_skia::IRect::from_ltrb(0, 0, w as i32, h as i32);
        if p.is_ellipse {
            let mut path = sharex_skia::Path::new();
            path.add_oval(rect, sharex_skia::Direction::Cw);
            sharex_skia::scan::anti_fill_path(&path, clip, &mut clear);
        } else {
            // SkScan::AntiFillRect; whole-pixel rectangles only for now.
            let ir = rect.round();
            sharex_skia::blitter::Blitter::blit_rect(
                &mut clear,
                ir.left,
                ir.top,
                ir.width(),
                ir.height(),
            );
        }
    }
    for (d, s) in pm.pixels_mut().iter_mut().zip(overlay.pixels()) {
        *d = src_over(*s, *d);
    }
}
