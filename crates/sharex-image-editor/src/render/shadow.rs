//! `DropShadowEffect` on an annotation's visual (`ShadowEffectHelper`):
//! Avalonia draws the visual into a Skia layer bounded by its rectangle
//! grown by the effect's padding, and restores it through
//! `SkImageFilters::DropShadow(dx, dy, sigma, sigma, color)`: the layer
//! blurred, coloured (`kSrcIn`), offset, with the layer itself drawn over
//! it (lowp srcover), composited onto the image (srcover).

use sharex_skia::Pixmap;

use super::color::parse_color;
use super::layout::AMatrix;
use crate::model::Common;

/// `EffectExtensions.GetEffectOutputPadding` for a drop shadow:
/// (left, top, right, bottom).
fn padding(blur: f64, dx: f64, dy: f64) -> (f64, f64, f64, f64) {
    let r = if blur <= 0.0 { 0.0 } else { blur.ceil() + 1.0 };
    let (x, y, w, h) = (-r + dx, -r + dy, r * 2.0, r * 2.0);
    (
        (-x).max(0.0),
        (-y).max(0.0),
        (x + w).max(0.0),
        (y + h).max(0.0),
    )
}

/// `SkBlurRadiusToSigma`.
fn sigma(radius: f64) -> f32 {
    if radius <= 0.0 {
        0.0
    } else {
        0.288675f32 * radius as f32 + 0.5
    }
}

/// `SkMatrix::decomposeScale`: the lengths of the matrix's columns
/// (`SkPoint::Length`, in float).
fn decompose_scale(m: &sharex_skia::Matrix) -> (f32, f32) {
    let len = |x: f32, y: f32| {
        let mag2 = x * x + y * y;
        if mag2.is_finite() { mag2.sqrt() } else { ((x as f64) * (x as f64) + (y as f64) * (y as f64)).sqrt() as f32 }
    };
    (len(m.sx, m.ky), len(m.kx, m.sy))
}

/// The drop shadow's colour and blend as [`with_shadow`] applies them, for
/// one pixel: `src` the layer's pixel, `da` the blurred alpha under the
/// shadow's offset.
fn shadow_pixel(src: u32, da: u32, alpha: u32, rgb: (u32, u32, u32)) -> u32 {
    let div255 = |v: u32| {
        let v = v + 128;
        (v + (v / 256)) / 256
    };
    let shadow = (div255(alpha * da) << 24) | (div255(rgb.0 * da) << 16) | (div255(rgb.1 * da) << 8) | div255(rgb.2 * da);
    let sa = src >> 24;
    let mut merged = 0u32;
    for sh in [0, 8, 16, 24] {
        let s_c = (src >> sh) & 0xFF;
        let d_c = (shadow >> sh) & 0xFF;
        merged |= ((s_c + div255(d_c * (255 - sa))) & 0xFF) << sh;
    }
    merged
}

/// The drop shadow under a transform that isn't a translation (a rotated
/// annotation). Skia (`SkCanvas::internalSaveLayer`, `skif::Mapping::
/// decomposeCTM`) splits the transform into a scale (the layer matrix,
/// its columns' lengths) and the rest (the rotation and the translation):
/// the visual is drawn into a layer in that unrotated space, the filter
/// (blur, `kSrcIn` colour, the offset, the layer over it) runs there, so
/// the shadow's offset turns with the annotation, and the result is drawn
/// back through the rest of the transform, sampled bilinearly
/// (`SkDevice::drawFilteredImage`, `drawSpecial`: the raster pipeline, the
/// image's rect anti-aliased). `local` is the effect's bounds in the
/// visual's space (the `saveLayer` bounds).
fn rotated(
    pm: &mut Pixmap,
    c: &Common,
    local: sharex_skia::Rect,
    m: &sharex_skia::Matrix,
    draw: impl FnOnce(&mut Pixmap),
) {
    use sharex_skia::IRect;
    use sharex_skia::raster::{Bitmap, Canvas, Matrix as RMatrix, Paint, Rect as RRect, Sampling, Shader};
    let (sx, sy) = decompose_scale(m);
    if sx == 0.0 || sy == 0.0 || !sx.is_finite() || !sy.is_finite() {
        return;
    }
    // The rest of the transform: m.preScale(1/sx, 1/sy).
    let (isx, isy) = (1.0 / sx, 1.0 / sy);
    let rest = RMatrix([m.sx * isx, m.kx * isy, m.tx, m.ky * isx, m.sy * isy, m.ty, 0.0, 0.0, 1.0]);
    let Some(rest_inv) = rest.invert() else { return };
    let blur = c.shadow_blur_radius.max(0.0);
    let s = sigma(blur);
    // Layer space: the effect bounds scaled, rounded out, cut to what the
    // device's clip needs (the filter's input for that output: the shadow
    // offset back and its blur kernel, the layer itself).
    let scaled = RRect::ltrb(local.left * sx, local.top * sy, local.right * sx, local.bottom * sy);
    // skif's rounding: within kRoundEpsilon (1e-3) of a whole pixel counts
    // as on it.
    const EPS: f64 = 1e-3;
    let round_out = |r: RRect| {
        IRect::from_ltrb(
            (r.left as f64 + EPS).floor() as i32,
            (r.top as f64 + EPS).floor() as i32,
            (r.right as f64 - EPS).ceil() as i32,
            (r.bottom as f64 - EPS).ceil() as i32,
        )
    };
    let mut device = IRect::from_ltrb(0, 0, pm.width() as i32, pm.height() as i32);
    if let Some(d) = sharex_skia::blitter::device_clip_rect()
        && !device.intersect(&d)
    {
        return;
    }
    let target = round_out(rest_inv.map_rect(RRect::ltrb(
        device.left as f32,
        device.top as f32,
        device.right as f32,
        device.bottom as f32,
    )));
    // The blur's kernel bounds: 3 sigma (in layer space), rounded up.
    let k = if s > 0.0 { ((3.0 * s * sx.max(sy)) as f64 - EPS).ceil() as i32 } else { 0 };
    let (ox, oy) = (
        (c.shadow_offset_x as f32 * sx).round() as i32,
        (c.shadow_offset_y as f32 * sy).round() as i32,
    );
    let mut needed = IRect::from_ltrb(target.left - ox - k, target.top - oy - k, target.right - ox + k, target.bottom - oy + k);
    needed = IRect::from_ltrb(
        needed.left.min(target.left),
        needed.top.min(target.top),
        needed.right.max(target.right),
        needed.bottom.max(target.bottom),
    );
    let mut lb = round_out(scaled);
    if !lb.intersect(&needed) {
        return;
    }
    // The visual, drawn in layer space at the layer's origin.
    let mut layer = Pixmap::new(lb.width() as u32, lb.height() as u32);
    super::layout::with_layer_space((sx, sy), || {
        super::layout::with_device_offset(-lb.left as f64, -lb.top as f64, || {
            sharex_skia::blitter::with_no_device_clip(|| {
                sharex_skia::draw::with_no_aa_clip(|| {
                    sharex_skia::blitter::with_blitters(false, || draw(&mut layer))
                })
            })
        })
    });
    // The filter's output: the layer and its blurred, offset copy, within
    // the device's needs.
    let mut out = IRect::from_ltrb(
        (lb.left).min(lb.left - k + ox),
        (lb.top).min(lb.top - k + oy),
        (lb.right).max(lb.right + k + ox),
        (lb.bottom).max(lb.bottom + k + oy),
    );
    if !out.intersect(&target) {
        return;
    }
    // The blur over the layer padded by its kernel.
    let (pw, ph) = (lb.width() + 2 * k, lb.height() + 2 * k);
    let mut padded = Pixmap::new(pw as u32, ph as u32);
    for y in 0..lb.height() {
        for x in 0..lb.width() {
            *padded.pixel_mut(x + k, y + k) = layer.pixel(x, y);
        }
    }
    let blurred = if s > 0.0 { sharex_skia::blur::blur(&padded, s * sx, s * sy) } else { padded };
    let color = parse_color(&c.shadow_color);
    let alpha = (color.a as f64 * c.shadow_opacity.clamp(0.0, 1.0)).clamp(0.0, 255.0) as u8 as u32;
    let pm_c = |v: u8| ((v as f32 / 255.0) * (alpha as f32 / 255.0) * 255.0 + 0.5) as u32;
    let rgb = (pm_c(color.r), pm_c(color.g), pm_c(color.b));
    let (rw, rh) = (out.width(), out.height());
    let mut result = vec![0u8; (rw * rh * 4) as usize];
    for y in 0..rh {
        for x in 0..rw {
            // Layer-space pixel.
            let (lx, ly) = (out.left + x, out.top + y);
            let src = if lx >= lb.left && lx < lb.right && ly >= lb.top && ly < lb.bottom {
                layer.pixel(lx - lb.left, ly - lb.top)
            } else {
                0
            };
            let (bx, by) = (lx - ox - lb.left + k, ly - oy - lb.top + k);
            let da = if bx >= 0 && by >= 0 && bx < pw && by < ph { blurred.pixel(bx, by) >> 24 } else { 0 };
            let p = shadow_pixel(src, da, alpha, rgb);
            let i = ((y * rw + x) * 4) as usize;
            result[i..i + 4].copy_from_slice(&[(p >> 16) as u8, (p >> 8) as u8, p as u8, (p >> 24) as u8]);
        }
    }
    // Drawn back: the rest of the transform after the result's origin,
    // bilinear, the image's rect anti-aliased, srcover.
    let image = Bitmap::from_premultiplied(rw as u32, rh as u32, result);
    // The layer device's deviceToGlobal: the rest, SkM44::preTranslate by
    // the layer's origin (col0 * x + col1 * y + col2 * 0 + col3); then
    // drawFilteredImage's SkMatrix::preTranslate by the result's offset in
    // the layer (sdot).
    let r = rest.0;
    let (lx, ly) = (lb.left as f32, lb.top as f32);
    let t1x = ((r[0] * lx + r[1] * ly) + 0.0) + r[2];
    let t1y = ((r[3] * lx + r[4] * ly) + 0.0) + r[5];
    let (fx, fy) = ((out.left - lb.left) as f32, (out.top - lb.top) as f32);
    let to_device = RMatrix([
        r[0],
        r[1],
        t1x + (r[0] * fx + r[1] * fy),
        r[3],
        r[4],
        t1y + (r[3] * fx + r[4] * fy),
        0.0,
        0.0,
        1.0,
    ]);
    let mut bmp = Bitmap::from_premultiplied(pm.width(), pm.height(), pm.to_premul_rgba());
    {
        let mut canvas = Canvas::new(&mut bmp);
        canvas.matrix = to_device;
        canvas.aa_clip = sharex_skia::draw::aa_clip();
        let mut paint = Paint::shader(Shader::Color([255, 255, 255, 255]));
        paint.anti_alias = true;
        let r = RRect::xywh(0.0, 0.0, rw as f32, rh as f32);
        canvas.draw_image_rect(&image, None, r, Sampling::Linear, Some(&paint));
    }
    *pm = Pixmap::from_premul_rgba(pm.width(), pm.height(), bmp.data());
}

/// Draws `draw` (which paints the visual in device space) with the
/// annotation's drop shadow. `size` is the visual's arranged size and
/// `ctm` its transform.
pub fn with_shadow(
    pm: &mut Pixmap,
    c: &Common,
    size: (f64, f64),
    ctm: &AMatrix,
    draw: impl FnOnce(&mut Pixmap),
) {
    let (w, h) = (pm.width() as i32, pm.height() as i32);
    let blur = c.shadow_blur_radius.max(0.0);
    let (dx, dy) = (c.shadow_offset_x, c.shadow_offset_y);
    let (pl, pt, pr, pb) = padding(blur, dx, dy);
    // The layer: the local rectangle grown by the padding, in the device
    // (translations only here), rounded out.
    let m = ctm.to_sk();
    if m.kx != 0.0 || m.ky != 0.0 || m.sx != 1.0 || m.sy != 1.0 {
        let local = sharex_skia::Rect::from_ltrb((-pl) as f32, (-pt) as f32, (size.0 + pr) as f32, (size.1 + pb) as f32);
        rotated(pm, c, local, &m, draw);
        return;
    }
    let rect = sharex_skia::Rect::from_ltrb(
        (-pl) as f32 + m.tx,
        (-pt) as f32 + m.ty,
        (size.0 + pr) as f32 + m.tx,
        (size.1 + pb) as f32 + m.ty,
    );
    let mut l = rect.round_out();
    if !l.intersect(&sharex_skia::IRect::from_ltrb(0, 0, w, h)) {
        return;
    }
    let mut layer = Pixmap::new(w as u32, h as u32);
    // The layer is drawn with the legacy blitters even inside a picture
    // shader's tile (where the device itself takes the pipeline's).
    sharex_skia::blitter::with_blitters(false, || draw(&mut layer));
    for y in 0..h {
        for x in 0..w {
            if x < l.left || x >= l.right || y < l.top || y >= l.bottom {
                *layer.pixel_mut(x, y) = 0;
            }
        }
    }
    let s = sigma(blur);
    let blurred = sharex_skia::blur::blur(&layer, s, s);
    // DrawingContextImpl.CreateEffect: the colour's alpha times the opacity.
    let color = parse_color(&c.shadow_color);
    let alpha = (color.a as f64 * c.shadow_opacity.clamp(0.0, 1.0)).clamp(0.0, 255.0) as u8 as u32;
    // lowp uniform_color: the premultiplied colour, rounded.
    let pm_c = |v: u8| ((v as f32 / 255.0) * (alpha as f32 / 255.0) * 255.0 + 0.5) as u32;
    let (cr, cg, cb) = (pm_c(color.r), pm_c(color.g), pm_c(color.b));
    // The SSE build's lowp blend modes (srcin, srcover) use div255_accurate.
    let div255 = |v: u32| {
        let v = v + 128;
        (v + (v / 256)) / 256
    };
    let (ox, oy) = (dx.round() as i32, dy.round() as i32);
    for y in 0..h {
        for x in 0..w {
            let (sx, sy) = (x - ox, y - oy);
            let da = if sx >= 0 && sy >= 0 && sx < w && sy < h {
                blurred.pixel(sx, sy) >> 24
            } else {
                0
            };
            // srcin: colour times the blurred alpha.
            let shadow = (div255(alpha * da) << 24)
                | (div255(cr * da) << 16)
                | (div255(cg * da) << 8)
                | div255(cb * da);
            let src = layer.pixel(x, y);
            let sa = src >> 24;
            // lowp srcover: s + div255_accurate(d * (255 - sa)).
            let mut merged = 0u32;
            for sh in [0, 8, 16, 24] {
                let s_c = (src >> sh) & 0xFF;
                let d_c = (shadow >> sh) & 0xFF;
                merged |= ((s_c + div255(d_c * (255 - sa))) & 0xFF) << sh;
            }
            if merged != 0 {
                let d = pm.pixel_mut(x, y);
                *d = super::effects::src_over(merged, *d);
            }
        }
    }
}
