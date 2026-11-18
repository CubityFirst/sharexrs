//! `WindowsCursorBitmapRenderer.CreateAnnotationBitmap`: the bitmap a
//! cursor annotation shows. Thirteen cursor types are ShareX's bundled
//! `.cur` files (`Assets/Cursors`, decoded as Skia decodes them: the
//! cursor directory rewritten as an icon's, then `SkIcoCodec`'s 1-bit BMP
//! with its AND mask); the others are Windows' own cursors
//! (`LoadCursor` + `DrawIconEx` into a 32-bit DIB), so on macOS and Linux
//! only the bundled ones exist, as in ShareX.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use sharex_skia::Pixmap;

use crate::model::CursorType;

/// `BundledCursors`: the cursor types ShareX ships as `.cur` files.
fn bundled(t: &CursorType) -> Option<&'static [u8]> {
    Some(match t {
        CursorType::HSplit => include_bytes!("../../assets/cursors/hsplit.cur"),
        CursorType::VSplit => include_bytes!("../../assets/cursors/vsplit.cur"),
        CursorType::NoMove2D => include_bytes!("../../assets/cursors/nomove2d.cur"),
        CursorType::NoMoveHoriz => include_bytes!("../../assets/cursors/nomoveh.cur"),
        CursorType::NoMoveVert => include_bytes!("../../assets/cursors/nomovev.cur"),
        CursorType::PanEast => include_bytes!("../../assets/cursors/east.cur"),
        CursorType::PanNE => include_bytes!("../../assets/cursors/ne.cur"),
        CursorType::PanNorth => include_bytes!("../../assets/cursors/north.cur"),
        CursorType::PanNW => include_bytes!("../../assets/cursors/nw.cur"),
        CursorType::PanSE => include_bytes!("../../assets/cursors/se.cur"),
        CursorType::PanSouth => include_bytes!("../../assets/cursors/south.cur"),
        CursorType::PanSW => include_bytes!("../../assets/cursors/sw.cur"),
        CursorType::PanWest => include_bytes!("../../assets/cursors/west.cur"),
        _ => return None,
    })
}

/// Whether `t` is one of ShareX's bundled cursors (available everywhere);
/// the others are the system's and exist only on Windows.
pub fn is_bundled(t: &CursorType) -> bool {
    bundled(t).is_some()
}

/// `CreateAnnotationBitmap`: the cursor's bitmap (cached per type, as
/// ShareX caches it), or `None` where ShareX has none (a system cursor
/// off Windows, or one Windows can't load).
pub fn cursor_bitmap(t: &CursorType) -> Option<Arc<Pixmap>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<Arc<Pixmap>>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    let key = format!("{t:?}");
    if let Some(v) = cache.lock().unwrap().get(&key) {
        return v.clone();
    }
    let v = render_cursor_bitmap(t).map(Arc::new);
    cache.lock().unwrap().insert(key, v.clone());
    v
}

fn render_cursor_bitmap(t: &CursorType) -> Option<Pixmap> {
    if let Some(data) = bundled(t) {
        return decode_cur(data);
    }
    system::cursor_bitmap(t)
}

fn u16_at(d: &[u8], i: usize) -> Option<u16> {
    Some(u16::from_le_bytes(d.get(i..i + 2)?.try_into().ok()?))
}

fn u32_at(d: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(i..i + 4)?.try_into().ok()?))
}

/// `SKBitmap.Decode` of a `.cur` rewritten as an `.ico`: the first image's
/// BMP (its colour table read as opaque), each pixel transparent where the
/// AND mask is set. ShareX's cursors are all 1-bit; other depths aren't
/// decoded.
fn decode_cur(d: &[u8]) -> Option<Pixmap> {
    let offset = u32_at(d, 18)? as usize;
    let header = d.get(offset..offset + 40)?;
    let width = i32::from_le_bytes(header[4..8].try_into().ok()?);
    let height = i32::from_le_bytes(header[8..12].try_into().ok()?) / 2;
    let bpp = u16_at(header, 14)?;
    if bpp != 1 || width <= 0 || height <= 0 {
        return None;
    }
    let (w, h) = (width as usize, height as usize);
    let colors = match u32_at(header, 32)? {
        0 => 2,
        n => n as usize,
    };
    let palette = offset + 40;
    let color = |i: usize| -> Option<u32> {
        let p = d.get(palette + 4 * i..palette + 4 * i + 3)?;
        Some(0xFF00_0000 | (p[2] as u32) << 16 | (p[1] as u32) << 8 | p[0] as u32)
    };
    let stride = (w * bpp as usize).div_ceil(32) * 4;
    let xor = palette + 4 * colors;
    let mask_stride = w.div_ceil(32) * 4;
    let and = xor + stride * h;
    let mut pm = Pixmap::new(w as u32, h as u32);
    for y in 0..h {
        // Bottom-up rows.
        let sy = h - 1 - y;
        for x in 0..w {
            let bit = |base: usize, s: usize| -> Option<u8> {
                Some((d.get(base + sy * s + x / 8)? >> (7 - x % 8)) & 1)
            };
            let index = bit(xor, stride)? as usize;
            let masked = bit(and, mask_stride)? == 1;
            *pm.pixel_mut(x as i32, y as i32) = if masked {
                0
            } else {
                color(index.min(colors - 1))?
            };
        }
    }
    Some(pm)
}

#[cfg(windows)]
mod system {
    use sharex_skia::Pixmap;
    use windows_sys::Win32::Graphics::Gdi::{
        BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, CreateDIBSection,
        DIB_RGB_COLORS, DeleteDC, DeleteObject, GdiFlush, GetObjectW, SelectObject,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        DI_NORMAL, DrawIconEx, GetIconInfo, ICONINFO, LoadCursorW,
    };

    use crate::model::CursorType;

    /// `FallbackCursorIds`: the `IDC_*` resource of each system cursor.
    fn cursor_id(t: &CursorType) -> Option<u16> {
        Some(match t {
            CursorType::AppStarting => 32650,
            CursorType::Arrow | CursorType::Default => 32512,
            CursorType::Cross => 32515,
            CursorType::Hand => 32649,
            CursorType::Help => 32651,
            CursorType::IBeam => 32513,
            CursorType::No => 32648,
            CursorType::SizeAll => 32646,
            CursorType::SizeNESW => 32643,
            CursorType::SizeNS => 32645,
            CursorType::SizeNWSE => 32642,
            CursorType::SizeWE => 32644,
            CursorType::UpArrow => 32516,
            CursorType::WaitCursor => 32514,
            _ => return None,
        })
    }

    /// `TryGetCursorSize`.
    unsafe fn cursor_size(cursor: *mut core::ffi::c_void) -> Option<(i32, i32)> {
        unsafe {
            let mut info: ICONINFO = std::mem::zeroed();
            if GetIconInfo(cursor, &mut info) == 0 {
                return None;
            }
            let handle = if !info.hbmColor.is_null() {
                info.hbmColor
            } else {
                info.hbmMask
            };
            let mut size = None;
            if !handle.is_null() {
                let mut bm: BITMAP = std::mem::zeroed();
                if GetObjectW(
                    handle,
                    std::mem::size_of::<BITMAP>() as i32,
                    &mut bm as *mut _ as *mut _,
                ) != 0
                {
                    let w = bm.bmWidth.abs();
                    let mut h = bm.bmHeight.abs();
                    if info.hbmColor.is_null() && h > 1 {
                        h /= 2;
                    }
                    if w > 0 && h > 0 {
                        size = Some((w, h));
                    }
                }
            }
            if !info.hbmColor.is_null() {
                DeleteObject(info.hbmColor);
            }
            if !info.hbmMask.is_null() {
                DeleteObject(info.hbmMask);
            }
            size
        }
    }

    pub(super) fn cursor_bitmap(t: &CursorType) -> Option<Pixmap> {
        let id = cursor_id(t)?;
        unsafe {
            let cursor = LoadCursorW(std::ptr::null_mut(), id as usize as *const u16);
            if cursor.is_null() {
                return None;
            }
            let (width, height) = cursor_size(cursor).unwrap_or((32, 32));
            draw_cursor(cursor, width, height)
        }
    }

    /// `DrawCursorToBitmap`.
    unsafe fn draw_cursor(
        cursor: *mut core::ffi::c_void,
        width: i32,
        height: i32,
    ) -> Option<Pixmap> {
        unsafe {
            let dc = CreateCompatibleDC(std::ptr::null_mut());
            if dc.is_null() {
                return None;
            }
            let mut bmi: BITMAPINFO = std::mem::zeroed();
            bmi.bmiHeader = BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                biSizeImage: (width * height * 4) as u32,
                ..std::mem::zeroed()
            };
            let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
            let dib =
                CreateDIBSection(dc, &bmi, DIB_RGB_COLORS, &mut bits, std::ptr::null_mut(), 0);
            if dib.is_null() || bits.is_null() {
                DeleteDC(dc);
                return None;
            }
            let old = SelectObject(dc, dib);
            let len = (width * height * 4) as usize;
            let buffer = std::slice::from_raw_parts_mut(bits as *mut u8, len);
            buffer.fill(0);
            let mut result = None;
            if DrawIconEx(
                dc,
                0,
                0,
                cursor,
                width,
                height,
                0,
                std::ptr::null_mut(),
                DI_NORMAL,
            ) != 0
            {
                GdiFlush();
                let dark = buffer.to_vec();
                if dark.chunks_exact(4).any(|p| p[3] != 0) {
                    result = Some(from_bgra(width as u32, height as u32, &dark));
                } else {
                    // Monochrome cursors leave alpha empty: recover their mask
                    // by drawing against black and white backgrounds.
                    buffer.fill(255);
                    if DrawIconEx(
                        dc,
                        0,
                        0,
                        cursor,
                        width,
                        height,
                        0,
                        std::ptr::null_mut(),
                        DI_NORMAL,
                    ) != 0
                    {
                        GdiFlush();
                        let light = &*buffer;
                        let mut out = Vec::with_capacity(len);
                        for (d, l) in dark.chunks_exact(4).zip(light.chunks_exact(4)) {
                            let diff = |i: usize| l[i] as i32 - d[i] as i32;
                            let alpha = 255 - diff(0).max(diff(1).max(diff(2))).clamp(0, 255);
                            if alpha == 0 {
                                out.extend_from_slice(&[0, 0, 0, 0]);
                            } else {
                                let c = |v: u8| (v as i32 * 255 / alpha).min(255) as u8;
                                // SKBitmap.SetPixel(new SKColor(r, g, b, alpha)).
                                let p = super::super::image::premultiply_straight([
                                    c(d[2]),
                                    c(d[1]),
                                    c(d[0]),
                                    alpha as u8,
                                ]);
                                out.extend_from_slice(&[p[2], p[1], p[0], p[3]]);
                            }
                        }
                        result = Some(from_bgra(width as u32, height as u32, &out));
                    }
                }
            }
            if !old.is_null() {
                SelectObject(dc, old);
            }
            DeleteObject(dib);
            DeleteDC(dc);
            result
        }
    }

    /// Premultiplied BGRA bytes, as `SKColorType.Bgra8888`/`Premul`.
    fn from_bgra(width: u32, height: u32, bgra: &[u8]) -> Pixmap {
        let rgba: Vec<u8> = bgra
            .chunks_exact(4)
            .flat_map(|p| [p[2], p[1], p[0], p[3]])
            .collect();
        Pixmap::from_premul_rgba(width, height, &rgba)
    }
}

#[cfg(not(windows))]
mod system {
    use sharex_skia::Pixmap;

    use crate::model::CursorType;

    /// `CreateAnnotationBitmap` returns null for system cursors off Windows.
    pub(super) fn cursor_bitmap(_: &CursorType) -> Option<Pixmap> {
        None
    }
}
