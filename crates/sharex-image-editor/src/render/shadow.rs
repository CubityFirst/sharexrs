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
    draw(&mut layer);
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
