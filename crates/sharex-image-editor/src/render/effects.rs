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

pub fn draw_effect(pm: &mut Pixmap, base: &Pixmap, a: &Annotation) {
    if a.common.rotation_angle != 0.0 {
        // Rotated effects (UpdateEffectFromProcessedSource) aren't ported.
        return;
    }
    if let Kind::Highlight(_) = &a.kind {
        draw_highlight(pm, base, a);
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

/// The shared spotlight overlay (`SpotlightOverlayControl`); not ported yet.
pub fn draw_spotlights(_pm: &mut Pixmap, _base: &Pixmap, _annotations: &[Annotation]) {}
