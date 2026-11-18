//! The image operations `EditorCore` applies
//! (`Core/ImageEffects/Helpers/ImageHelpers.cs` and the manipulation
//! effects it calls: `RotateImageEffect`, `FlipImageEffect`,
//! `ResizeImageEffect`), on premultiplied pixmaps as ShareX's `SKBitmap`s.

use sharex_skia::Pixmap;
use sharex_skia::raster::{Bitmap, Blend, Canvas, Paint, Rect as RRect, Sampling, Shader};

/// `ImageHelpers.Crop` / `SKBitmap.ExtractSubset` (a copy): the pixels in
/// `x, y, w, h`, bounded by the image; `None` when nothing's left.
pub fn crop(src: &Pixmap, x: i32, y: i32, w: i32, h: i32) -> Option<Pixmap> {
    let l = x.max(0);
    let t = y.max(0);
    let r = (x + w).min(src.width() as i32);
    let b = (y + h).min(src.height() as i32);
    if r <= l || b <= t {
        return None;
    }
    let mut out = Pixmap::new((r - l) as u32, (b - t) as u32);
    for row in t..b {
        for col in l..r {
            *out.pixel_mut(col - l, row - t) = src.pixel(col, row);
        }
    }
    Some(out)
}

/// `ImageHelpers.CutOut`: columns (`vertical`) or rows `start..end`
/// removed and the two sides joined; `None` for an empty or whole-image
/// cut (ShareX returns the same bitmap).
pub fn cut_out(src: &Pixmap, start: i32, end: i32, vertical: bool) -> Option<Pixmap> {
    let (start, end) = if start > end {
        (end, start)
    } else {
        (start, end)
    };
    let (w, h) = (src.width() as i32, src.height() as i32);
    let len = end - start;
    let full = if vertical { w } else { h };
    if len <= 0 || len >= full {
        return None;
    }
    let (nw, nh) = if vertical { (w - len, h) } else { (w, h - len) };
    let mut out = Pixmap::new(nw as u32, nh as u32);
    for y in 0..nh {
        for x in 0..nw {
            let (sx, sy) = if vertical {
                (if x < start { x } else { x + len }, y)
            } else {
                (x, if y < start { y } else { y + len })
            };
            *out.pixel_mut(x, y) = src.pixel(sx, sy);
        }
    }
    Some(out)
}

/// `RotateImageEffect.RotateOrthogonal`: 90, 180 or 270 degrees clockwise.
pub fn rotate_orthogonal(src: &Pixmap, degrees: i32) -> Pixmap {
    let angle = degrees.rem_euclid(360);
    let (w, h) = (src.width() as i32, src.height() as i32);
    let (nw, nh) = if angle == 90 || angle == 270 {
        (h, w)
    } else {
        (w, h)
    };
    let mut out = Pixmap::new(nw as u32, nh as u32);
    for y in 0..h {
        for x in 0..w {
            let (dx, dy) = match angle {
                90 => (h - 1 - y, x),
                180 => (w - 1 - x, h - 1 - y),
                270 => (y, w - 1 - x),
                _ => (x, y),
            };
            *out.pixel_mut(dx, dy) = src.pixel(x, y);
        }
    }
    out
}

/// `FlipImageEffect`: mirrored left-right and/or top-bottom.
pub fn flip(src: &Pixmap, horizontal: bool, vertical: bool) -> Pixmap {
    let (w, h) = (src.width() as i32, src.height() as i32);
    let mut out = Pixmap::new(w as u32, h as u32);
    for y in 0..h {
        for x in 0..w {
            let sx = if horizontal { w - 1 - x } else { x };
            let sy = if vertical { h - 1 - y } else { y };
            *out.pixel_mut(x, y) = src.pixel(sx, sy);
        }
    }
    out
}

fn to_bitmap(p: &Pixmap) -> Bitmap {
    Bitmap::from_premultiplied(p.width(), p.height(), p.to_premul_rgba())
}

fn from_bitmap(b: &Bitmap) -> Pixmap {
    Pixmap::from_premul_rgba(b.width(), b.height(), b.data())
}

/// `RotateImageEffect` for an angle that isn't a multiple of 90 (or not
/// auto-resized): the image drawn rotated about its centre (default,
/// nearest sampling, no anti-aliasing) into the rotated bounds
/// (`auto_resize`) or the same size.
pub fn rotate(src: &Pixmap, angle: f32, auto_resize: bool) -> Pixmap {
    if angle % 90.0 == 0.0 && auto_resize {
        return rotate_orthogonal(src, angle as i32);
    }
    let (w, h) = (src.width() as f32, src.height() as f32);
    let (nw, nh) = if auto_resize {
        // SKMatrix.CreateRotationDegrees(angle, w/2, h/2).MapRect
        let m = sharex_skia::raster::Matrix::rotate(angle);
        let (cx, cy) = (w / 2.0, h / 2.0);
        let r = rotate_about(&m, cx, cy).map_rect(RRect::xywh(0.0, 0.0, w, h));
        (
            (r.right - r.left).ceil() as u32,
            (r.bottom - r.top).ceil() as u32,
        )
    } else {
        (src.width(), src.height())
    };
    let mut out = Bitmap::new(nw, nh);
    {
        let mut canvas = Canvas::new(&mut out);
        canvas.clear([0, 0, 0, 0]);
        canvas.translate(nw as f32 / 2.0, nh as f32 / 2.0);
        canvas.rotate(angle);
        canvas.translate(-w / 2.0, -h / 2.0);
        canvas.draw_bitmap(&to_bitmap(src), 0.0, 0.0, None);
    }
    from_bitmap(&out)
}

/// `SkMatrix::setRotate(degrees, px, py)`.
fn rotate_about(m: &sharex_skia::raster::Matrix, px: f32, py: f32) -> sharex_skia::raster::Matrix {
    let (s, c) = (m.ky(), m.sx());
    let one_minus_cos = 1.0 - c;
    sharex_skia::raster::Matrix([
        c,
        -s,
        s * py + one_minus_cos * px,
        s,
        c,
        -s * px + one_minus_cos * py,
        0.0,
        0.0,
        1.0,
    ])
}

/// `ResizeImageEffect.Apply` (no aspect ratio): `SKBitmap.Resize` with
/// Catmull-Rom (`SkPixmap::scalePixels`: the image shader, clamped, drawn
/// over the whole result with `kSrc`).
pub fn resize(src: &Pixmap, width: u32, height: u32) -> Pixmap {
    if width == src.width() && height == src.height() {
        return src.clone();
    }
    let mut out = Bitmap::new(width, height);
    {
        let mut canvas = Canvas::new(&mut out);
        let mut paint = Paint::shader(Shader::Color([0, 0, 0, 255]));
        paint.blend = Blend::Src;
        canvas.draw_image_rect(
            &to_bitmap(src),
            None,
            RRect::xywh(0.0, 0.0, width as f32, height as f32),
            Sampling::CatmullRom,
            Some(&paint),
        );
    }
    from_bitmap(&out)
}

/// `ImageHelpers.ResizeCanvas`: `left`/`top`/`right`/`bottom` pixels
/// added around the image (negative values crop), filled with `background`
/// (straight ARGB), the image drawn at `(left, top)`.
pub fn resize_canvas(
    src: &Pixmap,
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
    background: [u8; 4],
) -> Option<Pixmap> {
    let nw = src.width() as i32 + left + right;
    let nh = src.height() as i32 + top + bottom;
    if nw <= 0 || nh <= 0 {
        return None;
    }
    let mut out = Bitmap::new(nw as u32, nh as u32);
    {
        let mut canvas = Canvas::new(&mut out);
        canvas.clear(background);
        canvas.draw_bitmap(&to_bitmap(src), left as f32, top as f32, None);
    }
    Some(from_bitmap(&out))
}

/// `SKBitmap.GetPixel`: the pixel unpremultiplied (`SkUnPreMultiply`'s
/// table), as straight `[r, g, b, a]`.
pub fn get_pixel(p: &Pixmap, x: i32, y: i32) -> [u8; 4] {
    let c = p.pixel(x, y);
    let a = c >> 24;
    let un = |v: u32| -> u8 {
        if a == 0 {
            0
        } else if a == 255 {
            v as u8
        } else {
            // SkUnPreMultiply::ApplyScale(gTable[a], v), gTable[a] =
            // (255 << 24) / a rounded.
            let scale = ((255u64 << 24) + a as u64 / 2) / a as u64;
            ((v as u64 * scale + (1 << 23)) >> 24) as u8
        }
    };
    [
        un((c >> 16) & 0xFF),
        un((c >> 8) & 0xFF),
        un(c & 0xFF),
        a as u8,
    ]
}

/// `ImageHelpers.ColorsMatch`.
pub fn colors_match(a: [u8; 4], b: [u8; 4], tolerance: i32) -> bool {
    a.iter()
        .zip(b.iter())
        .all(|(x, y)| (*x as i32 - *y as i32).abs() <= tolerance)
}

/// `ImageHelpers.FindContentBounds`: the pixels that differ from
/// `background` (left, top, right, bottom exclusive), or `None` for a
/// uniform image.
pub fn find_content_bounds(
    src: &Pixmap,
    background: [u8; 4],
    tolerance: i32,
) -> Option<(i32, i32, i32, i32)> {
    let (mut min_x, mut min_y, mut max_x, mut max_y) =
        (src.width() as i32, src.height() as i32, -1, -1);
    for y in 0..src.height() as i32 {
        for x in 0..src.width() as i32 {
            if !colors_match(get_pixel(src, x, y), background, tolerance) {
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
    }
    (max_x >= 0).then_some((min_x, min_y, max_x + 1, max_y + 1))
}
