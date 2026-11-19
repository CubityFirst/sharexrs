//! `ShareX.ScreenCaptureLib/Screenshot_Transparent.cs`: a window captured
//! with its transparency (rounded corners, the Windows 11 border) and, with
//! `CaptureShadow`, its shadow. A helper window right under it is made
//! white, black and white again; the white and black captures give each
//! pixel's alpha (`CreateTransparentImage`), the result is cropped to its
//! opaque part (`AutoCropImage`) and, without the shadow, the shadow left in
//! the corners is cleared (`TrimShadow`).
//!
//! The pixels go through the same conversions as ShareX's: its bitmaps are
//! premultiplied (`SKAlphaType.Premul`), `SkiaPixelBuffer` reads them
//! unpremultiplied and writes them back premultiplied, and the PNG encoder
//! unpremultiplies once more ([`encode`]).

use image::RgbaImage;
use sharex_skia::raster::{Bitmap, premul, unpremul};

use crate::geometry::Rect;
use crate::{Screen, platform};

/// A premultiplied picture (`SKBitmap`, Bgra8888 Premul), RGBA bytes.
pub type Premul = Bitmap;

/// An opaque screen capture as ShareX holds it (premultiplied: the same
/// bytes).
pub fn from_capture(image: &RgbaImage) -> Premul {
    Bitmap::from_straight(image.width(), image.height(), image.as_raw())
}

/// `SkiaPixelBuffer.Lock`: the pixels unpremultiplied (`ReadPixels`).
fn lock(bmp: &Premul) -> Vec<[u8; 4]> {
    (0..(bmp.width() * bmp.height()) as usize).map(|i| unpremul(bmp.px(i))).collect()
}

/// `SkiaPixelBuffer.Unlock`: written back premultiplied.
fn unlock(bmp: &mut Premul, pixels: &[[u8; 4]]) {
    for (i, p) in pixels.iter().enumerate() {
        bmp.set_px(i, premul(*p));
    }
}

/// The PNG encoder's unpremultiply (`sharex_tools::imaging::unpremul_encode`):
/// the float pipeline's `unpremul`, stored as `v * 255 + 0.5` truncated.
pub fn encode(bmp: &Premul) -> RgbaImage {
    use sharex_skia::raster::{load, unpremul_f};
    let mut out = Vec::with_capacity((bmp.width() * bmp.height() * 4) as usize);
    for i in 0..(bmp.width() * bmp.height()) as usize {
        let p = bmp.px(i);
        match p[3] {
            255 => out.extend_from_slice(&p),
            0 => out.extend_from_slice(&[0, 0, 0, 0]),
            _ => {
                let c = unpremul_f(load(p));
                let st = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
                out.extend_from_slice(&[st(c[0]), st(c[1]), st(c[2]), p[3]]);
            }
        }
    }
    RgbaImage::from_raw(bmp.width(), bmp.height(), out).unwrap_or_default()
}

/// `CreateTransparentImage(white, black)`: alpha from how much the black
/// capture's red is darker than the white one's, the colour from the black
/// capture divided by it (both truncated, as C#'s byte casts do).
pub fn create_transparent_image(white: &Premul, black: &Premul) -> Premul {
    let (w, h) = (white.width(), white.height());
    let whites = lock(white);
    let blacks = lock(black);
    let mut out = vec![[0u8; 4]; whites.len()];
    for i in 0..whites.len() {
        let (wp, bp) = (whites[i], blacks[i]);
        // ColorBgra: Red is p[0] here (RGBA bytes).
        let alpha = (bp[0] as f64 - wp[0] as f64 + 255.0) / 255.0;
        if alpha == 1.0 {
            out[i] = wp;
        } else if alpha > 0.0 {
            out[i] = [
                (bp[0] as f64 / alpha) as u8,
                (bp[1] as f64 / alpha) as u8,
                (bp[2] as f64 / alpha) as u8,
                (255.0 * alpha) as u8,
            ];
        }
    }
    // `SkiaImageHelpers.CreateBitmap` locked write-only, then unlocked.
    let mut result = Bitmap::new(w, h);
    unlock(&mut result, &out);
    result
}

/// `CopyOpaquePixels(source, destination, offset)`: `source` (an earlier
/// capture of the window, annotations and all) over `destination` as
/// opaque pixels, except in its corner squares and outer 2px where the
/// destination isn't opaque, so the rounded corners and the translucent
/// border stay transparent.
pub fn copy_opaque_pixels(source: &Premul, destination: &mut Premul, offset: (i32, i32)) {
    const MAX_CORNER: i32 = 32;
    const EDGE: i32 = 2;
    let (sw, sh) = (source.width() as i32, source.height() as i32);
    let corner = MAX_CORNER.min(sw.min(sh) / 2);
    let area = Rect::new(offset.0, offset.1, sw, sh).intersect(&Rect::new(0, 0, destination.width() as i32, destination.height() as i32));
    if area.is_empty() {
        return;
    }
    let src = lock(source);
    let mut dst = lock(destination);
    let dw = destination.width() as i32;
    for y in area.y..area.bottom() {
        let sy = y - offset.1;
        let corner_row = sy < corner || sy >= sh - corner;
        let edge_row = sy < EDGE || sy >= sh - EDGE;
        for x in area.x..area.right() {
            let sx = x - offset.0;
            let keep = edge_row || sx < EDGE || sx >= sw - EDGE || (corner_row && (sx < corner || sx >= sw - corner));
            let di = (x + y * dw) as usize;
            if !keep || dst[di][3] == 255 {
                let mut c = src[(sx + sy * sw) as usize];
                c[3] = 255;
                dst[di] = c;
            }
        }
    }
    unlock(destination, &dst);
}

/// `FindAutoCropRectangle(bmp)`: the part inside the edges that are all the
/// colour of the top-left pixel (only its alpha, when that's transparent),
/// the right and bottom edges against the bottom-right pixel.
pub fn find_auto_crop_rectangle(bmp: &Premul) -> Rect {
    let (w, h) = (bmp.width() as i32, bmp.height() as i32);
    let mut crop = Rect::new(0, 0, w, h);
    if w == 0 || h == 0 {
        return crop;
    }
    let px = lock(bmp);
    let at = |x: i32, y: i32| px[(x + y * w) as usize];
    // `ColorBgra.Bgra & mask`: the whole colour, or the alpha alone.
    let key = |c: [u8; 4]| -> (bool, [u8; 4]) { (c[3] == 0, c) };
    let differs = |c: [u8; 4], (alpha_only, check): (bool, [u8; 4])| if alpha_only { c[3] != check[3] } else { c != check };
    let mut check = key(at(0, 0));
    let mut leave = false;
    'left: for x in 0..w {
        for y in 0..h {
            if differs(at(x, y), check) {
                crop.x = x;
                crop.w -= x;
                leave = true;
                break 'left;
            }
        }
    }
    if !leave {
        return crop;
    }
    'top: for y in 0..h {
        for x in 0..w {
            if differs(at(x, y), check) {
                crop.y = y;
                crop.h -= y;
                break 'top;
            }
        }
    }
    check = key(at(w - 1, h - 1));
    'right: for x in (0..w).rev() {
        for y in 0..h {
            if differs(at(x, y), check) {
                crop.w = x - crop.x + 1;
                break 'right;
            }
        }
    }
    'bottom: for y in (0..h).rev() {
        for x in 0..w {
            if differs(at(x, y), check) {
                crop.h = y - crop.y + 1;
                break 'bottom;
            }
        }
    }
    crop
}

/// `AutoCropImage(bmp)`: cropped to [`find_auto_crop_rectangle`]
/// (`CropBitmap`: drawn onto a new transparent bitmap, the same bytes).
pub fn auto_crop(bmp: Premul) -> Premul {
    let r = find_auto_crop_rectangle(&bmp);
    if r == Rect::new(0, 0, bmp.width() as i32, bmp.height() as i32) || r.w <= 0 || r.h <= 0 {
        return bmp;
    }
    let mut out = Bitmap::new(r.w as u32, r.h as u32);
    for y in 0..r.h {
        for x in 0..r.w {
            out.set_pixel(x as u32, y as u32, bmp.pixel((r.x + x) as u32, (r.y + y) as u32));
        }
    }
    out
}

/// `TrimShadow`: in each corner's 10x10 square, the pixels before the first
/// one this opaque are cleared, row by row from the outside in (Windows
/// 11: 75 for the top corners and 123 for the bottom ones; else 200).
pub fn trim_shadow(bmp: &mut Premul, windows_11: bool) {
    let (w, h) = (bmp.width() as i32, bmp.height() as i32);
    let corner = 10.min(w.min(h));
    if corner <= 0 {
        return;
    }
    let mut px = lock(bmp);
    let mut alpha_offset: u8 = 200;
    let clear_run =|px: &mut Vec<[u8; 4]>, y: i32, xs: &mut dyn Iterator<Item = i32>, limit: u8| {
        for x in xs {
            let i = (x + y * w) as usize;
            if px[i][3] < limit {
                px[i] = [0, 0, 0, 0];
            } else {
                break;
            }
        }
    };
    for i in 0..corner {
        let y = i;
        if windows_11 {
            alpha_offset = 75;
        }
        clear_run(&mut px, y, &mut (0..corner), alpha_offset);
        clear_run(&mut px, y, &mut ((w - corner)..w).rev(), alpha_offset);
        if windows_11 {
            alpha_offset = 123;
        }
        let y = h - i - 1;
        clear_run(&mut px, y, &mut (0..corner), alpha_offset);
        clear_run(&mut px, y, &mut ((w - corner)..w).rev(), alpha_offset);
    }
    unlock(bmp, &px);
}

/// `Screenshot.CaptureWindowTransparent(handle)` (`window_image` none) or
/// `CaptureWindowTransparent(handle, windowImage, windowRectangle)`: the
/// window from the live screen with its transparency, and its shadow with
/// `capture_shadow` (`shadow_offset` pixels around it). With a window image
/// (a region capture of the whole window), its pixels are kept inside and
/// `None` means keep that image (the window moved, or the captures didn't
/// agree). `fallback` is `CaptureWindow(handle)`, for when the helper
/// window can't go under the target.
pub struct TransparentCapture<'a> {
    pub screens: &'a [Screen],
    pub capture_shadow: bool,
    pub shadow_offset: i32,
    pub capture_cursor: bool,
}

impl TransparentCapture<'_> {
    pub fn capture(&self, handle: isize, window_image: Option<(&RgbaImage, Rect)>, fallback: &dyn Fn() -> Option<RgbaImage>) -> Option<RgbaImage> {
        if handle == 0 {
            return None;
        }
        let mut rect = platform::window_rect(handle);
        if let Some((img, r)) = window_image
            && (rect != r || img.width() as i32 != rect.w || img.height() as i32 != rect.h)
        {
            eprintln!("Transparent capture skipped. Reason: Window rectangle changed.");
            return None;
        }
        let window_bounds = rect;
        if self.capture_shadow && !platform::is_zoomed(handle) {
            rect = Rect::new(rect.x - self.shadow_offset, rect.y - self.shadow_offset, rect.w + 2 * self.shadow_offset, rect.h + 2 * self.shadow_offset);
            let on = crate::geometry::combine(self.screens.iter().map(|s| s.bounds).filter(|b| b.intersect(&rect).is_valid()));
            rect = rect.intersect(&on);
        }
        let captures = {
            let Some(mut background) = platform::BackgroundWindow::new(rect) else {
                return if window_image.is_none() { fallback() } else { None };
            };
            background.set_color([255, 255, 255]);
            if !background.show_behind(handle) {
                eprintln!("Transparent capture failed. Reason: SetWindowPos fail.");
                return if window_image.is_none() { fallback() } else { None };
            }
            let grab = |b: &mut platform::BackgroundWindow, c: [u8; 3]| {
                b.set_color(c);
                platform::BackgroundWindow::flush();
                platform::capture_rectangle(rect)
            };
            platform::BackgroundWindow::flush();
            let white = platform::capture_rectangle(rect);
            let black = grab(&mut background, [0, 0, 0]);
            let white2 = grab(&mut background, [255, 255, 255]);
            (white, black, white2)
        };
        let (Some(white), Some(black), Some(white2)) = captures else { return None };
        let (white, black, white2) = (from_capture(&white), from_capture(&black), from_capture(&white2));
        let same = white.width() == white2.width() && white.height() == white2.height() && lock(&white) == lock(&white2);
        let (mut image, transparent) = if same {
            (create_transparent_image(&white, &black), true)
        } else if window_image.is_some() {
            eprintln!("Transparent capture skipped. Reason: Images not equal.");
            return None;
        } else {
            eprintln!("Transparent capture failed. Reason: Images not equal.");
            (white2, false)
        };
        if let Some((img, _)) = window_image {
            copy_opaque_pixels(&from_capture(img), &mut image, (window_bounds.x - rect.x, window_bounds.y - rect.y));
        }
        // A window image already shows the pointer if the region capture
        // drew it.
        if self.capture_cursor && window_image.is_none() {
            let straight = encode(&image);
            if let Some(cursor) = platform::cursor_image(&straight, (rect.x, rect.y)) {
                draw_cursor_premul(&mut image, &cursor);
            }
        }
        if transparent {
            image = auto_crop(image);
            if !self.capture_shadow {
                trim_shadow(&mut image, platform::is_windows_11());
            }
        }
        Some(encode(&image))
    }
}

/// `CursorData.DrawCursor(bitmap, offset)` on a premultiplied bitmap:
/// source over.
fn draw_cursor_premul(bmp: &mut Premul, cursor: &crate::CursorImage) {
    let (ox, oy) = cursor.position;
    for (x, y, p) in cursor.image.enumerate_pixels() {
        let (tx, ty) = (ox + x as i32, oy + y as i32);
        if tx < 0 || ty < 0 || tx as u32 >= bmp.width() || ty as u32 >= bmp.height() || p[3] == 0 {
            continue;
        }
        let s = premul(p.0);
        let d = bmp.pixel(tx as u32, ty as u32);
        let inv = 255 - s[3] as u32;
        let blend = |a: u8, b: u8| (a as u32 + (b as u32 * inv + 127) / 255).min(255) as u8;
        bmp.set_pixel(tx as u32, ty as u32, [blend(s[0], d[0]), blend(s[1], d[1]), blend(s[2], d[2]), blend(s[3], d[3])]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: u32, h: u32, c: [u8; 4]) -> Premul {
        let mut b = Bitmap::new(w, h);
        for i in 0..(w * h) as usize {
            b.set_px(i, premul(c));
        }
        b
    }

    #[test]
    fn white_and_black_give_the_alpha() {
        // A 50% grey shadow pixel: 128 over white, 0 over black... and an
        // opaque window pixel the same on both.
        let mut white = solid(2, 1, [255, 255, 255, 255]);
        let mut black = solid(2, 1, [0, 0, 0, 255]);
        white.set_px(0, [128, 128, 128, 255]);
        black.set_px(0, [0, 0, 0, 255]);
        white.set_px(1, [10, 20, 30, 255]);
        black.set_px(1, [10, 20, 30, 255]);
        let t = create_transparent_image(&white, &black);
        // alpha = (0 - 128 + 255) / 255 = 0.498: 127, colour 0.
        assert_eq!(t.px(0), premul([0, 0, 0, 127]));
        assert_eq!(t.px(1), [10, 20, 30, 255]);
    }

    #[test]
    fn auto_crop_drops_the_transparent_edges() {
        let mut b = Bitmap::new(6, 5);
        b.set_pixel(2, 1, [255, 0, 0, 255]);
        b.set_pixel(3, 3, [0, 255, 0, 255]);
        assert_eq!(find_auto_crop_rectangle(&b), Rect::new(2, 1, 2, 3));
        assert_eq!(auto_crop(b).width(), 2);
    }

    #[test]
    fn trim_shadow_clears_faint_corner_pixels() {
        let mut b = solid(20, 20, [0, 0, 0, 255]);
        b.set_pixel(0, 0, premul([0, 0, 0, 40]));
        b.set_pixel(1, 0, premul([0, 0, 0, 100]));
        b.set_pixel(0, 19, premul([0, 0, 0, 100]));
        trim_shadow(&mut b, true);
        assert_eq!(b.pixel(0, 0), [0, 0, 0, 0]);
        // 100 >= 75: the top row stops there.
        assert_eq!(b.pixel(1, 0)[3], 100);
        // The bottom corners clear below 123.
        assert_eq!(b.pixel(0, 19), [0, 0, 0, 0]);
    }

    #[test]
    fn opaque_pixels_are_copied_but_corners_stay_transparent() {
        let src = solid(70, 70, [9, 9, 9, 255]);
        let mut dst = Bitmap::new(70, 70);
        dst.set_pixel(35, 35, [1, 1, 1, 255]);
        copy_opaque_pixels(&src, &mut dst, (0, 0));
        assert_eq!(dst.pixel(35, 35), [9, 9, 9, 255]);
        assert_eq!(dst.pixel(0, 0), [0, 0, 0, 0]);
        assert_eq!(dst.pixel(1, 35), [0, 0, 0, 0]);
    }
}
