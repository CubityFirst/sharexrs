//! The effect annotations (`Core/Annotations/Effects`): each computes an
//! effect bitmap from the source image (`UpdateEffectFromInteractionCache`,
//! the path the editor takes when it builds their visuals), which its
//! `Rectangle` visual fills with an `ImageBrush` placed 1:1 on the image's
//! pixel grid (`AnnotationEffectVisualUpdater.ApplyEffectBrush`).

use sharex_skia::Pixmap;

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
    if p.is_ellipse {
        // The elliptical clip (an anti-aliased SkAAClip) isn't ported.
        return;
    }
    // The visual: the effect bitmap stretched over the arranged rectangle
    // (1:1 for whole-pixel bounds).
    let (ar, ctm) = super::shapes::arranged(&a.common);
    let (ox, oy) = (ar.x as i32, ar.y as i32);
    for y in 0..full_h {
        for x in 0..full_w {
            let (dx, dy) = (ox + x, oy + y);
            if dx < 0 || dy < 0 || dx >= pm.width() as i32 || dy >= pm.height() as i32 {
                continue;
            }
            let d = pm.pixel_mut(dx, dy);
            *d = src_over(effect.pixel(x, y), *d);
        }
    }
    // ApplyMagnifyBorder: a transparent border has no width.
    let color = parse_color(&a.common.stroke_color);
    let thickness = if color.a == 0 {
        0.0
    } else {
        a.common.stroke_width.max(0.0) as f64
    };
    if thickness > 0.0 {
        let h = thickness / 2.0;
        let rect = Rect::from_ltrb(
            h as f32,
            h as f32,
            (h + (ar.width - thickness).max(0.0)) as f32,
            (h + (ar.height - thickness).max(0.0)) as f32,
        );
        let mut path = sharex_skia::Path::new();
        path.add_rect(rect, sharex_skia::Direction::Cw, 0);
        let pen = super::shapes::Pen {
            color,
            thickness,
            dashes: super::shapes::dash_array(&p.border_style),
            cap: super::shapes::line_cap(&p.border_style),
            join: sharex_skia::Join::Miter,
        };
        super::shapes::draw_geometry(pm, &ctm, None, &path, Some(&pen), &path);
    }
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
