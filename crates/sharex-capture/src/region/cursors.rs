//! `ShareX.Avalonia/Input/CursorAssetLoader.cs`: the overlay's own
//! cursors (crosshair, open and closed hand) from ShareX's `.cur` files,
//! scaled for the monitor (Catmull-Rom, as Skia's).

/// `CursorAssetLoader.CustomCursorKind`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CustomCursor {
    ClosedHand,
    Crosshair,
    OpenHand,
}

/// A cursor's picture (straight RGBA) and hotspot.
#[derive(Clone, Debug, PartialEq)]
pub struct CursorBitmap {
    pub rgba: Vec<u8>,
    pub width: u16,
    pub height: u16,
    pub hotspot: (u16, u16),
}

fn data(kind: CustomCursor) -> &'static [u8] {
    match kind {
        CustomCursor::ClosedHand => include_bytes!("../../assets/cursors/closedhand.cur"),
        CustomCursor::Crosshair => include_bytes!("../../assets/cursors/Crosshair.cur"),
        CustomCursor::OpenHand => include_bytes!("../../assets/cursors/openhand.cur"),
    }
}

fn u16_at(d: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes(d.get(o..o + 2)?.try_into().ok()?))
}

fn u32_at(d: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(o..o + 4)?.try_into().ok()?))
}

fn bit(d: &[u8], row: usize, x: usize) -> bool {
    d.get(row + x / 8).is_some_and(|b| b & (1 << (7 - x % 8)) != 0)
}

/// `LoadCursor`: the first image of a `.cur` (PNG, 1 bpp or 32 bpp), as
/// RGBA with its hotspot.
pub fn decode(d: &[u8]) -> Option<CursorBitmap> {
    if d.len() < 22 || u16_at(d, 2)? != 2 || u16_at(d, 4)? == 0 {
        return None;
    }
    let hotspot = (u16_at(d, 10)?, u16_at(d, 12)?);
    let length = u32_at(d, 14)? as usize;
    let offset = u32_at(d, 18)? as usize;
    let img = d.get(offset..offset + length)?;
    if img.starts_with(&[0x89, b'P', b'N', b'G']) {
        let png = image::load_from_memory_with_format(img, image::ImageFormat::Png).ok()?.to_rgba8();
        let (w, h) = png.dimensions();
        return Some(CursorBitmap { rgba: png.into_raw(), width: w as u16, height: h as u16, hotspot });
    }
    let header = u32_at(img, 0)? as usize;
    let raw_w = u32_at(img, 4)? as i32;
    let raw_h = u32_at(img, 8)? as i32;
    let bpp = u16_at(img, 14)?;
    if header < 40 || u32_at(img, 16)? != 0 {
        return None;
    }
    let colors_used = u32_at(img, 32)? as usize;
    let (w, h) = (raw_w.unsigned_abs() as usize, raw_h.unsigned_abs() as usize / 2);
    let bottom_up = raw_h > 0;
    let source_row = |y: usize| if bottom_up { h - 1 - y } else { y };
    let mask_stride = (w.div_ceil(8) + 3) & !3;
    let mut rgba = vec![0u8; w * h * 4];
    match bpp {
        1 => {
            let palette = header;
            let count = if colors_used > 0 { colors_used } else { 2 };
            let color = |o: usize| [img[palette + o + 2], img[palette + o + 1], img[palette + o], 255];
            let (background, foreground) = (color(0), color(4));
            let xor = palette + count * 4;
            let and = xor + mask_stride * h;
            for y in 0..h {
                let row = source_row(y);
                for x in 0..w {
                    let x_bit = bit(img, xor + row * mask_stride, x);
                    let a_bit = bit(img, and + row * mask_stride, x);
                    let px = match (a_bit, x_bit) {
                        (true, false) => [0, 0, 0, 0],
                        // Monochrome cursors can ask for the screen to be
                        // inverted here; a static bitmap shows white.
                        (true, true) => [255, 255, 255, 255],
                        (false, true) => foreground,
                        (false, false) => background,
                    };
                    rgba[(y * w + x) * 4..][..4].copy_from_slice(&px);
                }
            }
        }
        32 => {
            let xor = header;
            let stride = w * 4;
            let and = xor + stride * h;
            let explicit_alpha = (0..h).any(|y| (0..w).any(|x| img[xor + y * stride + x * 4 + 3] != 0));
            for y in 0..h {
                let row = source_row(y);
                for x in 0..w {
                    let s = xor + row * stride + x * 4;
                    let a = if explicit_alpha {
                        img[s + 3]
                    } else if bit(img, and + row * mask_stride, x) {
                        0
                    } else {
                        255
                    };
                    rgba[(y * w + x) * 4..][..4].copy_from_slice(&[img[s + 2], img[s + 1], img[s], a]);
                }
            }
        }
        _ => return None,
    }
    Some(CursorBitmap { rgba, width: w as u16, height: h as u16, hotspot })
}

/// `GetCursor(kind, renderScaling)`: the cursor for a monitor at `scale`
/// (100% steps, never below 1).
pub fn cursor(kind: CustomCursor, scale: f64) -> Option<CursorBitmap> {
    let mut c = decode(data(kind))?;
    let scale = if scale.is_finite() && scale > 1.0 { scale } else { 1.0 };
    let scale = ((scale * 100.0).round() as i32).max(100) as f64 / 100.0;
    let (w, h) = (
        ((c.width as f64 * scale).round() as u32).max(1),
        ((c.height as f64 * scale).round() as u32).max(1),
    );
    if (w, h) != (c.width as u32, c.height as u32) {
        let src = image::RgbaImage::from_raw(c.width as u32, c.height as u32, c.rgba.clone())?;
        let scaled = image::imageops::resize(&src, w, h, image::imageops::FilterType::CatmullRom);
        let hx = ((c.hotspot.0 as f64 * w as f64 / c.width as f64).round() as u32).min(w - 1);
        let hy = ((c.hotspot.1 as f64 * h as f64 / c.height as f64).round() as u32).min(h - 1);
        c = CursorBitmap { rgba: scaled.into_raw(), width: w as u16, height: h as u16, hotspot: (hx as u16, hy as u16) };
    }
    Some(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sharex_cursors_decode() {
        let c = cursor(CustomCursor::Crosshair, 1.0).unwrap();
        assert_eq!((c.width, c.height, c.hotspot), (32, 32, (15, 15)));
        assert!(c.rgba.chunks(4).any(|p| p[3] == 255));
        let hand = cursor(CustomCursor::OpenHand, 1.5).unwrap();
        assert_eq!((hand.width, hand.height), (48, 48));
        assert_eq!(cursor(CustomCursor::ClosedHand, 1.0).unwrap().hotspot, (7, 5));
    }
}
