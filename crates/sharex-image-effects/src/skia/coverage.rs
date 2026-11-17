//! Which pixels a draw touches and how much: Skia's scan conversion. Rects are exact
//! (`SkScan::FillRect` rounds, `AntiFillRect` covers fractional edges); paths go through
//! tiny-skia, a port of Skia's scan converter: non-anti-aliased paths match Skia, but
//! anti-aliased ones use Skia's older supersampler (4x vertical, 16 levels per pixel)
//! where Skia m119 uses analytic AA, so their edge pixels can differ slightly.

use super::Rect;

/// Coverage over the destination: full rows of spans, or a per-pixel mask.
pub(crate) enum Coverage {
    /// Every pixel of `[left, right) x [top, bottom)` fully covered (clipped).
    Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    },
    /// Per-pixel coverage for the whole destination (0 = untouched).
    Mask(Vec<u8>),
}

/// `SkScan::FillRect` (non-AA): edges rounded to the nearest pixel.
pub(crate) fn fill_rect(r: Rect, width: u32, height: u32) -> Coverage {
    let round = |v: f32| (v + 0.5).floor() as i32;
    Coverage::Rect {
        left: round(r.left).max(0),
        top: round(r.top).max(0),
        right: round(r.right).min(width as i32),
        bottom: round(r.bottom).min(height as i32),
    }
}

/// An anti-aliased rect: `SkScan::AntiFillRect`, exact when the edges are whole pixels.
pub(crate) fn anti_fill_rect(r: Rect, width: u32, height: u32) -> Coverage {
    let whole = |v: f32| v == v.floor();
    if whole(r.left) && whole(r.top) && whole(r.right) && whole(r.bottom) {
        return fill_rect(r, width, height);
    }
    match tiny_skia::Rect::from_ltrb(r.left, r.top, r.right, r.bottom) {
        Some(rect) => mask_from(width, height, |pm, paint| {
            pm.fill_rect(rect, paint, tiny_skia::Transform::identity(), None);
        }),
        None => Coverage::Mask(vec![0; width as usize * height as usize]),
    }
}

/// A filled path (already in device space), anti-aliased or not.
pub(crate) fn fill_path(
    path: &tiny_skia::Path,
    anti_alias: bool,
    width: u32,
    height: u32,
) -> Coverage {
    mask_from(width, height, |pm, paint| {
        let mut paint = paint.clone();
        paint.anti_alias = anti_alias;
        pm.fill_path(
            path,
            &paint,
            tiny_skia::FillRule::Winding,
            tiny_skia::Transform::identity(),
            None,
        );
    })
}

fn mask_from(
    width: u32,
    height: u32,
    draw: impl FnOnce(&mut tiny_skia::Pixmap, &tiny_skia::Paint),
) -> Coverage {
    let Some(mut pm) = tiny_skia::Pixmap::new(width.max(1), height.max(1)) else {
        return Coverage::Mask(Vec::new());
    };
    let mut paint = tiny_skia::Paint::default();
    paint.set_color_rgba8(255, 255, 255, 255);
    paint.blend_mode = tiny_skia::BlendMode::Source;
    paint.anti_alias = true;
    draw(&mut pm, &paint);
    // Opaque white with "source" blending leaves the coverage in alpha, exactly
    // (lowp `lerp(0, 255, c)` = `div255(255 * c)` = c).
    Coverage::Mask(pm.data().chunks_exact(4).map(|p| p[3]).collect())
}
