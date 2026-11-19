//! Windows: what ShareX does through Win32 (`WindowsImageInterop`,
//! `NativeMethods`, `CursorData`, `WindowsRectangleList`,
//! `WindowsClipboard`).

use std::ffi::c_void;
use std::time::{Duration, Instant};

use image::RgbaImage;
use windows_sys::Win32::Foundation::{CloseHandle, GlobalFree, HWND, LPARAM, POINT, RECT};
use windows_sys::Win32::Graphics::Dwm::{
    DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS, DWMWA_TRANSITIONS_FORCEDISABLED, DwmGetWindowAttribute,
    DwmSetWindowAttribute,
};
use windows_sys::Win32::Graphics::Gdi::{
    BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CAPTUREBLT, ClientToScreen, CreateCompatibleDC,
    CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, EnumDisplayMonitors, GdiFlush, GetMonitorInfoW,
    GetObjectW, GetWindowDC, HDC, HMONITOR, MONITORINFO, ReleaseDC, SRCCOPY, SelectObject,
};
use windows_sys::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
};
use windows_sys::Win32::System::Memory::GlobalSize;
use windows_sys::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
use windows_sys::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows_sys::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CURSOR_SHOWING, CURSORINFO, CopyIcon, DI_NORMAL, DestroyIcon, DrawIconEx, EnumChildWindows, EnumWindows,
    GWL_EXSTYLE, GetClassNameW, GetClientRect, GetCursorInfo, GetCursorPos, GetDesktopWindow, GetIconInfo,
    GetWindowLongPtrW, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId, HICON, ICONINFO,
    IsWindowVisible, SetCursorPos, SetWindowLongPtrW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

use crate::geometry::Rect;
use crate::{CursorImage, Screen, SimpleWindowInfo, WindowInfo};

fn rect(r: RECT) -> Rect {
    Rect::new(r.left, r.top, r.right - r.left, r.bottom - r.top)
}

/// Every monitor (`DesktopScreen.AllScreens`), in physical pixels.
pub fn screens() -> Vec<Screen> {
    unsafe extern "system" fn each(m: HMONITOR, _: HDC, _: *mut RECT, data: LPARAM) -> i32 {
        // SAFETY: `data` is the `Vec` passed below, alive for the call.
        let list = unsafe { &mut *(data as *mut Vec<HMONITOR>) };
        list.push(m);
        1
    }
    let mut handles: Vec<HMONITOR> = Vec::new();
    // SAFETY: the callback only pushes into `handles`.
    unsafe { EnumDisplayMonitors(std::ptr::null_mut(), std::ptr::null(), Some(each), &raw mut handles as LPARAM) };
    handles
        .into_iter()
        .filter_map(|m| {
            let mut info: MONITORINFO = unsafe { std::mem::zeroed() };
            info.cbSize = size_of::<MONITORINFO>() as u32;
            // SAFETY: a live monitor handle and a sized MONITORINFO.
            if unsafe { GetMonitorInfoW(m, &mut info) } == 0 {
                return None;
            }
            let (mut dx, mut dy) = (96u32, 96u32);
            // SAFETY: as above; the out pointers are valid.
            unsafe { GetDpiForMonitor(m, MDT_EFFECTIVE_DPI, &mut dx, &mut dy) };
            Some(Screen {
                bounds: rect(info.rcMonitor),
                work_area: rect(info.rcWork),
                scale: dx as f64 / 96.0,
                primary: info.dwFlags & 1 != 0,
            })
        })
        .collect()
}

/// `CaptureHelpers.GetCursorPosition`.
pub fn cursor_position() -> Option<(i32, i32)> {
    let mut p = POINT { x: 0, y: 0 };
    // SAFETY: writes into a valid POINT.
    (unsafe { GetCursorPos(&mut p) } != 0).then_some((p.x, p.y))
}

/// `CaptureHelpers.SetCursorPosition`.
pub fn set_cursor_position(x: i32, y: i32) {
    // SAFETY: no pointers.
    unsafe { SetCursorPos(x, y) };
}

/// A 32 bpp top-down DIB section selected into a memory DC, which `draw`
/// paints, as `WindowsImageInterop.DrawNative`. Returns the pixels as RGBA
/// (BGRA swapped), alpha left as GDI wrote it.
fn draw_native(w: i32, h: i32, background: Option<[u8; 3]>, draw: impl FnOnce(HDC)) -> Option<RgbaImage> {
    if w <= 0 || h <= 0 {
        return None;
    }
    // SAFETY: plain GDI calls on objects created and freed here.
    unsafe {
        let dc = CreateCompatibleDC(std::ptr::null_mut());
        if dc.is_null() {
            return None;
        }
        let mut bi: BITMAPINFO = std::mem::zeroed();
        bi.bmiHeader = BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w,
            biHeight: -h,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            biSizeImage: (w * h * 4) as u32,
            ..std::mem::zeroed()
        };
        let mut bits: *mut c_void = std::ptr::null_mut();
        let bmp = CreateDIBSection(dc, &bi, DIB_RGB_COLORS, &mut bits, std::ptr::null_mut(), 0);
        if bmp.is_null() || bits.is_null() {
            DeleteDC(dc);
            return None;
        }
        let previous = SelectObject(dc, bmp);
        let len = (w * h * 4) as usize;
        let pixels = std::slice::from_raw_parts_mut(bits as *mut u8, len);
        if let Some([r, g, b]) = background {
            for px in pixels.chunks_exact_mut(4) {
                px.copy_from_slice(&[b, g, r, 255]);
            }
        }
        draw(dc);
        GdiFlush();
        let mut out = vec![0u8; len];
        for (o, i) in out.chunks_exact_mut(4).zip(pixels.chunks_exact(4)) {
            o.copy_from_slice(&[i[2], i[1], i[0], i[3]]);
        }
        SelectObject(dc, previous);
        DeleteObject(bmp);
        DeleteDC(dc);
        RgbaImage::from_raw(w as u32, h as u32, out)
    }
}

/// `WindowsImageInterop.Capture`: the screen's pixels in `r` (desktop
/// pixels), `BitBlt` with `CAPTUREBLT`, opaque.
///
/// ShareX blits into a DIB section; here it's a bitmap compatible with the
/// screen read with `GetDIBits`, the same pixels (the same `BitBlt` from
/// the same DC) about 20 ms sooner for a 2560x1600 screen, most of the
/// rest being DWM's wait, whatever the size (docs/PORTING.md, "Region
/// capture performance"). The BGRA rows are swapped to RGBA and made
/// opaque in one pass, in place.
pub fn capture_rectangle(r: Rect) -> Option<RgbaImage> {
    use windows_sys::Win32::Graphics::Gdi::{CreateCompatibleBitmap, GetDIBits};
    if r.w <= 0 || r.h <= 0 {
        return None;
    }
    let len = r.w as usize * r.h as usize * 4;
    // SAFETY: the desktop window's DC, a memory DC and bitmap created here,
    // all released below; `GetDIBits` writes `r.h` rows of `r.w` 32-bit
    // pixels into a buffer of that size.
    let mut pixels = unsafe {
        let desktop = GetDesktopWindow();
        let source = GetWindowDC(desktop);
        if source.is_null() {
            return None;
        }
        let dc = CreateCompatibleDC(source);
        let bmp = if dc.is_null() { std::ptr::null_mut() } else { CreateCompatibleBitmap(source, r.w, r.h) };
        let mut out = None;
        if !bmp.is_null() {
            let previous = SelectObject(dc, bmp);
            let blitted = BitBlt(dc, 0, 0, r.w, r.h, source, r.x, r.y, SRCCOPY | CAPTUREBLT) != 0;
            SelectObject(dc, previous);
            crate::trace::mark("grab: BitBlt");
            // A failed blit (the secure desktop) leaves black, as ShareXrs's
            // zeroed DIB section did.
            if !blitted {
                out = Some(vec![0u8; len]);
            } else {
                let mut bi: BITMAPINFO = std::mem::zeroed();
                bi.bmiHeader = BITMAPINFOHEADER {
                    biSize: size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: r.w,
                    biHeight: -r.h,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB,
                    biSizeImage: len as u32,
                    ..std::mem::zeroed()
                };
                let mut buf: Vec<u8> = Vec::with_capacity(len);
                let lines = GetDIBits(dc, bmp, 0, r.h as u32, buf.as_mut_ptr().cast(), &mut bi, DIB_RGB_COLORS);
                if lines == r.h {
                    buf.set_len(len);
                    out = Some(buf);
                }
                crate::trace::mark("grab: GetDIBits");
            }
            DeleteObject(bmp);
        }
        if !dc.is_null() {
            DeleteDC(dc);
        }
        ReleaseDC(desktop, source);
        out?
    };
    // `SetOpaque`, with BGRA to RGBA.
    bgra_to_opaque_rgba(&mut pixels);
    crate::trace::mark("grab: BGRA to opaque RGBA");
    RgbaImage::from_raw(r.w as u32, r.h as u32, pixels)
}

/// BGRA pixels to RGBA with alpha 255, in place (a screen's worth split
/// over threads).
fn bgra_to_opaque_rgba(pixels: &mut [u8]) {
    sharex_skia::par::for_each_chunk(pixels, 4, |_, chunk| {
        for p in chunk.chunks_exact_mut(4) {
            let v = u32::from_le_bytes([p[0], p[1], p[2], p[3]]);
            let rgba = (v & 0x0000_ff00) | ((v >> 16) & 0xff) | ((v & 0xff) << 16) | 0xff00_0000;
            p.copy_from_slice(&rgba.to_le_bytes());
        }
    });
}

/// `ScreenColorPickerWindow.TryUpdateMagnifier`: the screen DC's pixels in
/// `r` (`GetDC(NULL)`, `BitBlt` with `SRCCOPY` only, so layered windows
/// aren't in it), opaque; black where no monitor is.
pub fn sample_screen(r: Rect) -> Option<RgbaImage> {
    use windows_sys::Win32::Graphics::Gdi::GetDC;
    // SAFETY: the screen DC, released below.
    let source = unsafe { GetDC(std::ptr::null_mut()) };
    if source.is_null() {
        return None;
    }
    let mut image = draw_native(r.w, r.h, Some([0, 0, 0]), |dc| {
        // SAFETY: both DCs are live for the call.
        unsafe { BitBlt(dc, 0, 0, r.w, r.h, source, r.x, r.y, SRCCOPY) };
    });
    // SAFETY: as above.
    unsafe { ReleaseDC(std::ptr::null_mut(), source) };
    if let Some(img) = &mut image {
        for p in img.pixels_mut() {
            p[3] = 255;
        }
    }
    image
}

fn bitmap_size(handle: *mut c_void) -> Option<(i32, i32)> {
    let mut bm: BITMAP = unsafe { std::mem::zeroed() };
    // SAFETY: a live bitmap handle and a BITMAP-sized buffer.
    let n = unsafe { GetObjectW(handle, size_of::<BITMAP>() as i32, (&raw mut bm).cast()) };
    (n != 0).then_some((bm.bmWidth, bm.bmHeight))
}

/// `CursorData.GetCursorSizeMultiplier`: the accessibility cursor size.
fn cursor_size_multiplier() -> f32 {
    let key: Vec<u16> = "SOFTWARE\\Microsoft\\Accessibility\0".encode_utf16().collect();
    let name: Vec<u16> = "CursorSize\0".encode_utf16().collect();
    let mut value = 0u32;
    let mut size = 4u32;
    // SAFETY: valid strings and a DWORD buffer of `size` bytes.
    let ok = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            name.as_ptr(),
            RRF_RT_REG_DWORD,
            std::ptr::null_mut(),
            (&raw mut value).cast(),
            &mut size,
        )
    } == 0;
    if ok && value > 1 { 1.0 + (value as f32 - 1.0) * 0.5 } else { 1.0 }
}

/// `CursorData`: the mouse pointer as it's showing, ready to be drawn into
/// a screenshot whose top-left corner is at `origin` (desktop pixels):
/// `ToBitmap(background, offset)` and `DrawPosition - offset`. XOR pixels
/// (the I-beam) take the colours of `background` beneath them, as ShareX's.
pub fn cursor_image(background: &RgbaImage, origin: (i32, i32)) -> Option<CursorImage> {
    let mut info: CURSORINFO = unsafe { std::mem::zeroed() };
    info.cbSize = size_of::<CURSORINFO>() as u32;
    // SAFETY: a sized CURSORINFO.
    if unsafe { GetCursorInfo(&mut info) } == 0 || info.flags != CURSOR_SHOWING {
        return None;
    }
    cursor_image_of(info.hCursor, (info.ptScreenPos.x, info.ptScreenPos.y), background, origin)
}

/// [`cursor_image`] of the standard cursor `IDC_*` `id` (`LoadCursor`) as
/// if the pointer were at `position` (desktop pixels), for checking the
/// drawing against ShareX's (`tools/tools-harness --cursors`).
pub fn system_cursor_image(id: u16, position: (i32, i32), background: &RgbaImage, origin: (i32, i32)) -> Option<CursorImage> {
    use windows_sys::Win32::UI::WindowsAndMessaging::LoadCursorW;
    // SAFETY: a shared system cursor (MAKEINTRESOURCE id), not destroyed.
    let handle = unsafe { LoadCursorW(std::ptr::null_mut(), id as usize as *const u16) };
    if handle.is_null() {
        return None;
    }
    cursor_image_of(handle, position, background, origin)
}

/// `CursorData` of the cursor `handle` at `position`, `ToBitmap(background,
/// offset)` placed at `DrawPosition - offset`.
fn cursor_image_of(handle: HICON, position: (i32, i32), background: &RgbaImage, origin: (i32, i32)) -> Option<CursorImage> {
    let (hotspot, size) = cursor_geometry(handle);
    let draw_at = (position.0 - hotspot.0 - origin.0, position.1 - hotspot.1 - origin.1);
    let image = from_icon(handle, size, background, draw_at)?;
    Some(CursorImage { image, position: draw_at })
}

/// `CursorData.DrawCursor(hdc, offset)` on a screenshot taken at `origin`:
/// the pointer as it shows now, drawn with `DrawIconEx` onto the pixels as
/// `Screenshot.CaptureRectangleGDI` draws it onto the capture's DC (false
/// when it's hidden).
pub fn draw_cursor_gdi(image: &mut RgbaImage, origin: (i32, i32)) -> bool {
    let mut info: CURSORINFO = unsafe { std::mem::zeroed() };
    info.cbSize = size_of::<CURSORINFO>() as u32;
    // SAFETY: a sized CURSORINFO.
    if unsafe { GetCursorInfo(&mut info) } == 0 || info.flags != CURSOR_SHOWING {
        return false;
    }
    draw_cursor_gdi_of(image, info.hCursor, (info.ptScreenPos.x, info.ptScreenPos.y), origin)
}

/// [`draw_cursor_gdi`] of the standard cursor `id` at `position`, for the
/// comparison with ShareX's.
pub fn draw_system_cursor_gdi(image: &mut RgbaImage, id: u16, position: (i32, i32), origin: (i32, i32)) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::LoadCursorW;
    // SAFETY: a shared system cursor, not destroyed.
    let handle = unsafe { LoadCursorW(std::ptr::null_mut(), id as usize as *const u16) };
    !handle.is_null() && draw_cursor_gdi_of(image, handle, position, origin)
}

fn draw_cursor_gdi_of(image: &mut RgbaImage, handle: HICON, position: (i32, i32), origin: (i32, i32)) -> bool {
    let (hotspot, size) = cursor_geometry(handle);
    let at = (position.0 - hotspot.0 - origin.0, position.1 - hotspot.1 - origin.1);
    let (w, h) = (image.width() as i32, image.height() as i32);
    let src = image.clone();
    let drawn = draw_native(w, h, None, |dc| {
        // SAFETY: the DIB section of `dc`, filled with the screenshot, then
        // the icon drawn over it.
        unsafe {
            use windows_sys::Win32::Graphics::Gdi::SetDIBitsToDevice;
            let mut bi: BITMAPINFO = std::mem::zeroed();
            bi.bmiHeader = BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                ..std::mem::zeroed()
            };
            let bgra: Vec<u8> = src.pixels().flat_map(|p| [p[2], p[1], p[0], 255]).collect();
            SetDIBitsToDevice(dc, 0, 0, w as u32, h as u32, 0, 0, 0, h as u32, bgra.as_ptr().cast(), &bi, DIB_RGB_COLORS);
            DrawIconEx(dc, at.0, at.1, handle, size.0, size.1, 0, std::ptr::null_mut(), DI_NORMAL);
        }
    });
    let Some(mut drawn) = drawn else { return false };
    // `WindowsImageInterop.Capture`: a screenshot is opaque.
    for p in drawn.pixels_mut() {
        p[3] = 255;
    }
    *image = drawn;
    true
}

/// The cursor's hotspot and drawing size (0x0: its own) as `CursorData`
/// reads them, scaled by the accessibility cursor size.
fn cursor_geometry(handle: HICON) -> ((i32, i32), (i32, i32)) {
    let multiplier = cursor_size_multiplier();
    let default_size = multiplier == 1.0;
    let mut hotspot = (0, 0);
    let mut size = (0, 0);
    // SAFETY: the copy is destroyed below; the icon info's bitmaps are
    // deleted after reading their sizes.
    unsafe {
        let copy = CopyIcon(handle);
        if !copy.is_null() {
            let mut ii: ICONINFO = std::mem::zeroed();
            if GetIconInfo(copy, &mut ii) != 0 {
                hotspot = if default_size {
                    (ii.xHotspot as i32, ii.yHotspot as i32)
                } else {
                    (
                        (ii.xHotspot as f32 * multiplier).round() as i32,
                        (ii.yHotspot as f32 * multiplier).round() as i32,
                    )
                };
                if !ii.hbmMask.is_null() {
                    if !default_size && let Some((mw, mh)) = bitmap_size(ii.hbmMask) {
                        let h = if ii.hbmColor.is_null() { mh / 2 } else { mh };
                        size = ((mw as f32 * multiplier).round() as i32, (h as f32 * multiplier).round() as i32);
                    }
                    DeleteObject(ii.hbmMask);
                }
                if !ii.hbmColor.is_null() {
                    DeleteObject(ii.hbmColor);
                }
            }
            DestroyIcon(copy);
        }
    }
    (hotspot, size)
}

/// `WindowsImageInterop.FromIcon`: the icon drawn on black and on white,
/// alpha recovered from the difference; XOR pixels from the icon drawn
/// over `background` at `position`.
fn from_icon(icon: HICON, mut size: (i32, i32), background: &RgbaImage, position: (i32, i32)) -> Option<RgbaImage> {
    if size == (0, 0) {
        let mut ii: ICONINFO = unsafe { std::mem::zeroed() };
        // SAFETY: the info's bitmaps are deleted after reading their sizes.
        unsafe {
            if GetIconInfo(icon, &mut ii) != 0 {
                let sized = if ii.hbmColor.is_null() { ii.hbmMask } else { ii.hbmColor };
                if let Some((w, h)) = bitmap_size(sized) {
                    size = (w, if ii.hbmColor.is_null() { h / 2 } else { h });
                }
                if !ii.hbmColor.is_null() {
                    DeleteObject(ii.hbmColor);
                }
                if !ii.hbmMask.is_null() {
                    DeleteObject(ii.hbmMask);
                }
            }
        }
    }
    let (w, h) = size;
    if w <= 0 || h <= 0 {
        return None;
    }
    // SAFETY: the DC is the DIB section's, live for the call.
    let draw = |dc: HDC| unsafe {
        DrawIconEx(dc, 0, 0, icon, w, h, 0, std::ptr::null_mut(), DI_NORMAL);
    };
    let black = draw_native(w, h, Some([0, 0, 0]), draw)?;
    let white = draw_native(w, h, Some([255, 255, 255]), draw)?;
    // The background under the cursor, on black where it's off the image.
    let mut under = RgbaImage::from_pixel(w as u32, h as u32, image::Rgba([0, 0, 0, 255]));
    for y in 0..h {
        for x in 0..w {
            let (bx, by) = (position.0 + x, position.1 + y);
            if bx >= 0 && by >= 0 && (bx as u32) < background.width() && (by as u32) < background.height() {
                under.put_pixel(x as u32, y as u32, *background.get_pixel(bx as u32, by as u32));
            }
        }
    }
    let composited = draw_native(w, h, None, |dc| {
        // Paint the background into the DIB, then the icon over it.
        // SAFETY: the DIB section of `dc`, written through SetDIBitsToDevice.
        unsafe {
            use windows_sys::Win32::Graphics::Gdi::SetDIBitsToDevice;
            let mut bi: BITMAPINFO = std::mem::zeroed();
            bi.bmiHeader = BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                ..std::mem::zeroed()
            };
            let bgra: Vec<u8> = under.pixels().flat_map(|p| [p[2], p[1], p[0], 255]).collect();
            SetDIBitsToDevice(dc, 0, 0, w as u32, h as u32, 0, 0, 0, h as u32, bgra.as_ptr().cast(), &bi, DIB_RGB_COLORS);
            DrawIconEx(dc, 0, 0, icon, w, h, 0, std::ptr::null_mut(), DI_NORMAL);
        }
    })?;
    let mut out = RgbaImage::new(w as u32, h as u32);
    for ((o, d), (l, c)) in out.pixels_mut().zip(black.pixels()).zip(white.pixels().zip(composited.pixels())) {
        if l[0] < d[0] || l[1] < d[1] || l[2] < d[2] {
            // XOR pixels depend on the destination: alpha can't express them.
            *o = image::Rgba([c[0], c[1], c[2], 255]);
            continue;
        }
        let diff = (l[0] as i32 - d[0] as i32).max(l[1] as i32 - d[1] as i32).max(l[2] as i32 - d[2] as i32);
        let alpha = 255 - diff.clamp(0, 255);
        if alpha == 0 {
            continue;
        }
        let un = |v: u8| ((v as i32 * 255 / alpha).min(255)) as u8;
        *o = image::Rgba([un(d[0]), un(d[1]), un(d[2]), alpha as u8]);
    }
    Some(out)
}

fn window_rectangle(hwnd: HWND) -> Rect {
    // `CaptureHelpers.GetWindowRectangle`: DWM's frame without the
    // invisible resize borders, else the window rectangle.
    let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    // SAFETY: a RECT-sized out buffer.
    let ok = unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS as u32,
            (&raw mut r).cast(),
            size_of::<RECT>() as u32,
        )
    } == 0;
    let rr = rect(r);
    if ok && !rr.is_empty() {
        return rr;
    }
    // SAFETY: as above.
    unsafe { GetWindowRect(hwnd, &mut r) };
    rect(r)
}

/// `NativeMethods.GetClientRect`: the client area in screen pixels.
fn client_rectangle(hwnd: HWND) -> Rect {
    let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    let mut p = POINT { x: 0, y: 0 };
    // SAFETY: valid out pointers.
    unsafe {
        GetClientRect(hwnd, &mut r);
        ClientToScreen(hwnd, &mut p);
    }
    Rect::new(p.x, p.y, r.right - r.left, r.bottom - r.top)
}

fn class_name(hwnd: HWND) -> String {
    let mut buf = [0u16; 256];
    // SAFETY: the buffer's length is passed.
    let n = unsafe { GetClassNameW(hwnd, buf.as_mut_ptr(), buf.len() as i32) };
    String::from_utf16_lossy(&buf[..n.max(0) as usize])
}

fn is_cloaked(hwnd: HWND) -> bool {
    let mut cloaked = 0u32;
    // SAFETY: a u32 out buffer.
    let ok = unsafe { DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED as u32, (&raw mut cloaked).cast(), 4) } == 0;
    ok && cloaked != 0
}

/// `WindowsRectangleList`: the windows (and, with `include_children`, the
/// controls in them) under the pointer to snap the region to, topmost
/// first, each window's client area before the window itself.
struct WindowList {
    ignore: Vec<isize>,
    include_children: bool,
    deadline: Instant,
    windows: Vec<SimpleWindowInfo>,
    parents: std::collections::HashSet<isize>,
}

impl WindowList {
    /// `CheckHandle`; false stops the enumeration (timed out).
    fn check(&mut self, hwnd: HWND, clip: Option<Rect>) -> bool {
        if Instant::now() > self.deadline {
            return false;
        }
        let handle = hwnd as isize;
        if self.ignore.contains(&handle) {
            return true;
        }
        // SAFETY: a window handle from the enumeration.
        if unsafe { IsWindowVisible(hwnd) } == 0 {
            return true;
        }
        let is_window = clip.is_none();
        if is_window {
            if is_cloaked(hwnd) {
                return true;
            }
            // NVIDIA GeForce Overlay DT
            if class_name(hwnd).eq_ignore_ascii_case("CEF-OSC-WIDGET") {
                return true;
            }
            // SAFETY: as above.
            let ex = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) } as u32;
            let both = WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE;
            if ex & both == both {
                return true;
            }
        }
        let rect = match clip {
            None => window_rectangle(hwnd),
            Some(clip) => {
                let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
                // SAFETY: a RECT out buffer.
                unsafe { GetWindowRect(hwnd, &mut r) };
                rect(r).intersect(&clip)
            }
        };
        if !rect.is_valid() {
            return true;
        }
        if self.include_children && self.parents.insert(handle) {
            unsafe extern "system" fn child(hwnd: HWND, data: LPARAM) -> i32 {
                // SAFETY: `data` is the (list, clip) pair below, alive for
                // the enumeration.
                let (list, clip) = unsafe { &mut *(data as *mut (&mut WindowList, Rect)) };
                list.check(hwnd, Some(*clip)) as i32
            }
            let mut ctx: (&mut WindowList, Rect) = (self, rect);
            // SAFETY: the callback only uses `ctx` during the call.
            unsafe { EnumChildWindows(hwnd, Some(child), &raw mut ctx as LPARAM) };
        }
        if is_window {
            let client = client_rectangle(hwnd);
            if client.is_valid() && !same_as_window_rectangle(&client, &rect) {
                self.windows.push(SimpleWindowInfo { handle, rect: client, is_window: false });
            }
        }
        self.windows.push(SimpleWindowInfo { handle, rect, is_window });
        true
    }
}

/// `WindowsRectangleList.IsSameAsWindowRectangle`: a client area that
/// covers its window but for a thin border (a window drawing its own title
/// bar) isn't offered separately.
fn same_as_window_rectangle(client: &Rect, window: &Rect) -> bool {
    const TOLERANCE: i32 = 8;
    let v = client.intersect(window);
    !v.is_empty()
        && v.x - window.x <= TOLERANCE
        && v.y - window.y <= TOLERANCE
        && window.right() - v.right() <= TOLERANCE
        && window.bottom() - v.bottom() <= TOLERANCE
}

/// `WindowsRectangleList.GetWindowInfoList` (5 s timeout), skipping the
/// windows in `ignore`.
pub fn window_list(include_children: bool, ignore: &[isize]) -> Vec<SimpleWindowInfo> {
    let mut list = WindowList {
        ignore: ignore.to_vec(),
        include_children,
        deadline: Instant::now() + Duration::from_millis(5000),
        windows: Vec::new(),
        parents: Default::default(),
    };
    unsafe extern "system" fn top(hwnd: HWND, data: LPARAM) -> i32 {
        // SAFETY: `data` is `list` below, alive for the enumeration.
        let list = unsafe { &mut *(data as *mut WindowList) };
        list.check(hwnd, None) as i32
    }
    // SAFETY: the callback only uses `list` during the call.
    unsafe { EnumWindows(Some(top), &raw mut list as LPARAM) };
    crate::windows_list::remove_hidden_controls(list.windows)
}

/// `WindowInfo`: a window's title and process name, for naming the file.
pub fn window_info(handle: isize) -> WindowInfo {
    let hwnd = handle as HWND;
    let mut buf = [0u16; 512];
    // SAFETY: the buffer's length is passed.
    let n = unsafe { GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32) };
    let title = String::from_utf16_lossy(&buf[..n.max(0) as usize]);
    let mut pid = 0u32;
    // SAFETY: a u32 out pointer.
    unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
    WindowInfo { handle, title, process_name: process_name(pid) }
}

/// `Process.ProcessName`: the executable's name without its extension.
fn process_name(pid: u32) -> Option<String> {
    let mut buf = [0u16; 1024];
    let mut len = buf.len() as u32;
    // SAFETY: the handle is checked and closed; `buf`/`len` describe a
    // valid buffer.
    let ok = unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return None;
        }
        let ok = QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len);
        CloseHandle(handle);
        ok != 0
    };
    let path = String::from_utf16_lossy(&buf[..len as usize]);
    ok.then(|| std::path::Path::new(&path).file_stem().map(|s| s.to_string_lossy().into_owned()))
        .flatten()
}

/// `WindowsClipboard.SetImage`: CF_DIB and "PNG", owned by `owner` (a null
/// owner makes `SetClipboardData` fail after `EmptyClipboard`).
pub fn set_clipboard_image(owner: isize, png: &[u8], dib: &[u8]) -> Result<(), String> {
    set_clipboard(owner, &[(ClipboardFormat::Id(8), dib), (ClipboardFormat::Named("PNG"), png)]) // CF_DIB
}

/// `IsClipboardFormatAvailable`: whether the clipboard holds `format`.
pub fn clipboard_has_format(format: u32) -> bool {
    // SAFETY: no pointers.
    unsafe { windows_sys::Win32::System::DataExchange::IsClipboardFormatAvailable(format) != 0 }
}

/// The id of a registered clipboard format (`RegisterClipboardFormat`).
pub fn clipboard_format_id(name: &str) -> u32 {
    let wide: Vec<u16> = name.encode_utf16().chain([0]).collect();
    // SAFETY: a NUL-terminated string.
    unsafe { RegisterClipboardFormatW(wide.as_ptr()) }
}

/// `GetClipboardData(format)`'s bytes, or `None` when the clipboard
/// doesn't hold it (or can't be opened, after retrying as when setting it).
pub fn get_clipboard_data(format: u32) -> Option<Vec<u8>> {
    if !clipboard_has_format(format) {
        return None;
    }
    let mut opened = false;
    for _ in 0..=10 {
        // SAFETY: no pointers.
        if unsafe { OpenClipboard(std::ptr::null_mut()) } != 0 {
            opened = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    if !opened {
        return None;
    }
    // SAFETY: the clipboard is open; the handle's memory is read within
    // its size while locked.
    let data = unsafe {
        let handle = GetClipboardData(format);
        if handle.is_null() {
            None
        } else {
            let size = GlobalSize(handle);
            let p = GlobalLock(handle);
            if p.is_null() {
                None
            } else {
                let bytes = std::slice::from_raw_parts(p as *const u8, size).to_vec();
                GlobalUnlock(handle);
                Some(bytes)
            }
        }
    };
    // SAFETY: opened above.
    unsafe { CloseClipboard() };
    data
}

/// A clipboard format: a standard one (`CF_*`) or a registered name.
pub enum ClipboardFormat<'a> {
    Id(u32),
    Named(&'a str),
}

/// Empties the clipboard and puts `items` on it, owned by `owner`.
pub fn set_clipboard(owner: isize, items: &[(ClipboardFormat, &[u8])]) -> Result<(), String> {
    let owner = owner as HWND;
    let mut opened = false;
    // Another process may briefly hold the clipboard (a clipboard manager
    // reading the last copy): retried 10 times, 50 ms apart.
    for _ in 0..=10 {
        // SAFETY: no pointers.
        if unsafe { OpenClipboard(owner) } != 0 {
            opened = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    if !opened {
        return Err("the clipboard is in use".into());
    }
    let set = |format: u32, data: &[u8]| -> Result<(), String> {
        // SAFETY: the global block is filled within its size and given to
        // the clipboard, or freed when that fails.
        unsafe {
            let mem = GlobalAlloc(GMEM_MOVEABLE, data.len());
            if mem.is_null() {
                return Err("out of memory".into());
            }
            let p = GlobalLock(mem);
            if p.is_null() {
                GlobalFree(mem);
                return Err("couldn't lock memory".into());
            }
            std::ptr::copy_nonoverlapping(data.as_ptr(), p as *mut u8, data.len());
            GlobalUnlock(mem);
            if SetClipboardData(format, mem).is_null() {
                GlobalFree(mem);
                return Err("SetClipboardData failed".into());
            }
        }
        Ok(())
    };
    // SAFETY: the clipboard is open.
    let result = (|| {
        if unsafe { EmptyClipboard() } == 0 {
            return Err("EmptyClipboard failed".to_string());
        }
        for (format, data) in items {
            let id = match format {
                ClipboardFormat::Id(id) => *id,
                ClipboardFormat::Named(name) => {
                    let wide: Vec<u16> = name.encode_utf16().chain([0]).collect();
                    // SAFETY: a NUL-terminated string.
                    let id = unsafe { RegisterClipboardFormatW(wide.as_ptr()) };
                    if id == 0 {
                        return Err("RegisterClipboardFormat failed".to_string());
                    }
                    id
                }
            };
            set(id, data)?;
        }
        Ok(())
    })();
    // SAFETY: opened above.
    unsafe { CloseClipboard() };
    result
}

fn hwnd(window: &winit::window::Window) -> Option<HWND> {
    match window.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(h) => Some(h.hwnd.get() as HWND),
        _ => None,
    }
}

/// A winit window's `HWND`, for the window list to skip and the clipboard
/// to be owned by.
pub fn window_handle(window: &winit::window::Window) -> Option<isize> {
    hwnd(window).map(|h| h as isize)
}

/// The overlay as ShareX's (`ShowInTaskbar="False"`, no close animation
/// when a window may be captured right after), plus what snapr learnt: a
/// tool window, so browsers' occlusion tracking doesn't stall, and no Alt
/// menu mode (which captures the mouse until the next click).
///
/// The tool window style is set before the window is first shown and kept
/// whenever winit rewrites the styles (it does as it shows the window), as
/// ShareX's `Win32Properties.AddWindowStylesCallback` keeps it (ShareX
/// #9242): tools such as MicaForEveryone put a system backdrop on new
/// windows unless they're tool windows, which fills a see-through overlay.
pub fn prepare_overlay(window: &winit::window::Window) {
    use windows_sys::Win32::Foundation::{LRESULT, WPARAM};
    use windows_sys::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
    use windows_sys::Win32::UI::WindowsAndMessaging::{SC_KEYMENU, STYLESTRUCT, WM_STYLECHANGING, WM_SYSCOMMAND, WS_EX_LAYERED};

    unsafe extern "system" fn proc(h: HWND, msg: u32, w: WPARAM, l: LPARAM, _: usize, _: usize) -> LRESULT {
        if msg == WM_SYSCOMMAND && (w & 0xfff0) == SC_KEYMENU as usize {
            return 0;
        }
        if msg == WM_STYLECHANGING && w as i32 == GWL_EXSTYLE && l != 0 {
            // SAFETY: WM_STYLECHANGING's lParam is the STYLESTRUCT being applied.
            // A layered overlay (the transparent region) stays layered too:
            // winit doesn't know it made it one.
            unsafe {
                let ss = &mut *(l as *mut STYLESTRUCT);
                ss.styleNew |= WS_EX_TOOLWINDOW | (ss.styleOld & WS_EX_LAYERED);
            }
        }
        // SAFETY: forwards to the next window procedure.
        unsafe { DefSubclassProc(h, msg, w, l) }
    }

    let Some(h) = hwnd(window) else { return };
    let disabled: i32 = 1;
    // SAFETY: our own window, still hidden; the attribute is a BOOL.
    unsafe {
        let style = GetWindowLongPtrW(h, GWL_EXSTYLE);
        SetWindowLongPtrW(h, GWL_EXSTYLE, style | WS_EX_TOOLWINDOW as isize);
        DwmSetWindowAttribute(
            h,
            DWMWA_TRANSITIONS_FORCEDISABLED as u32,
            (&raw const disabled).cast(),
            size_of::<i32>() as u32,
        );
        SetWindowSubclass(h, Some(proc), 1, 0);
    }
}

/// Called once an overlay's first frame is on screen (macOS fades it in).
pub fn overlay_presented(_window: &winit::window::Window) {}

/// A kept overlay, still hidden, back on top of the topmost windows before
/// it's shown again, as a new one starts (`HWND_TOPMOST`, not activated:
/// the overlay is activated once shown, as before).
pub fn raise_overlay(window: &winit::window::Window) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetWindowPos};
    let Some(h) = hwnd(window) else { return };
    // SAFETY: our own window; no pointers.
    unsafe { SetWindowPos(h, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE) };
}

/// `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)`: the window doesn't
/// show in screen captures (the recording frame with the native recorder).
pub fn exclude_from_capture(window: &winit::window::Window) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::SetWindowDisplayAffinity;
    let Some(h) = hwnd(window) else { return false };
    // SAFETY: our own window.
    unsafe { SetWindowDisplayAffinity(h, 0x11) != 0 }
}

/// `TrayIconImage.IsLightTaskbarTheme`: `SystemUsesLightTheme` (missing
/// means light, as ShareX reads it).
pub fn light_taskbar() -> bool {
    let key: Vec<u16> = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize\0".encode_utf16().collect();
    let name: Vec<u16> = "SystemUsesLightTheme\0".encode_utf16().collect();
    let mut value = 0u32;
    let mut size = 4u32;
    // SAFETY: valid strings and a DWORD buffer of `size` bytes.
    let ok = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            name.as_ptr(),
            RRF_RT_REG_DWORD,
            std::ptr::null_mut(),
            (&raw mut value).cast(),
            &mut size,
        )
    } == 0;
    !ok || value != 0
}

/// `SetWindowRgn` to these rectangles (window pixels).
pub fn set_window_region(window: &winit::window::Window, rects: &[Rect]) {
    use windows_sys::Win32::Graphics::Gdi::{CombineRgn, CreateRectRgn, DeleteObject, RGN_OR, SetWindowRgn};
    let Some(h) = hwnd(window) else { return };
    // SAFETY: regions created here; SetWindowRgn owns the combined one.
    unsafe {
        let region = CreateRectRgn(0, 0, 0, 0);
        for r in rects {
            let part = CreateRectRgn(r.x, r.y, r.right(), r.bottom());
            CombineRgn(region, region, part, RGN_OR);
            DeleteObject(part);
        }
        if SetWindowRgn(h, region, 1) == 0 {
            DeleteObject(region);
        }
    }
}

// ---- windows for Capture › Window and the other capture jobs -------------

/// `NativeMethods.GetForegroundWindow`.
pub fn foreground_window() -> isize {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
    // SAFETY: no arguments.
    unsafe { GetForegroundWindow() as isize }
}

/// `CaptureHelpers.GetWindowRectangle`.
pub fn window_rect(handle: isize) -> Rect {
    window_rectangle(handle as HWND)
}

/// `NativeMethods.GetClientRect`: the client area in screen pixels.
pub fn client_rect(handle: isize) -> Rect {
    client_rectangle(handle as HWND)
}

/// `WindowInfo.IsMinimized`.
pub fn is_minimized(handle: isize) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::IsIconic;
    // SAFETY: no pointers.
    unsafe { IsIconic(handle as HWND) != 0 }
}

/// `WindowInfo.IsVisible`.
pub fn is_window_visible(handle: isize) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::IsWindowVisible;
    // SAFETY: no pointers.
    unsafe { IsWindowVisible(handle as HWND) != 0 }
}

/// `WindowInfo.Restore`.
pub fn restore_window(handle: isize) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{SW_RESTORE, ShowWindow};
    // SAFETY: no pointers.
    unsafe { ShowWindow(handle as HWND, SW_RESTORE) };
}

/// `WindowInfo.Activate`.
pub fn activate_window(handle: isize) {
    use windows_sys::Win32::UI::WindowsAndMessaging::SetForegroundWindow;
    // SAFETY: no pointers.
    unsafe { SetForegroundWindow(handle as HWND) };
}

/// `InputHelpers.SendKeyPress(keyCode)`: the key down and up through
/// `SendInput`, by virtual key alone (no scan code), as ShareX's
/// `InputManager.AddKeyPress`.
pub fn send_key_press(vk: u16) {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput};
    let key = |flags| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: vk, wScan: 0, dwFlags: flags, time: 0, dwExtraInfo: 0 } },
    };
    let inputs = [key(0), key(KEYEVENTF_KEYUP)];
    // SAFETY: a valid array of INPUTs and its element size.
    unsafe { SendInput(inputs.len() as u32, inputs.as_ptr(), std::mem::size_of::<INPUT>() as i32) };
}

/// `InputHelpers.SendMouseWheel(delta)`: one wheel event of `delta` at the
/// pointer (`MOUSEEVENTF_WHEEL`).
pub fn send_mouse_wheel(delta: i32) {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_WHEEL, MOUSEINPUT, SendInput};
    let input = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 { mi: MOUSEINPUT { dx: 0, dy: 0, mouseData: delta as u32, dwFlags: MOUSEEVENTF_WHEEL, time: 0, dwExtraInfo: 0 } },
    };
    // SAFETY: one valid INPUT and its size.
    unsafe { SendInput(1, &input, std::mem::size_of::<INPUT>() as i32) };
}

/// `NativeMethods.SendMessage(handle, WM_VSCROLL, code, 0)` (`SB_TOP` = 6,
/// `SB_LINEDOWN` = 1).
pub fn send_vscroll(handle: isize, code: usize) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{SendMessageW, WM_VSCROLL};
    // SAFETY: no pointers; an invalid handle only fails.
    unsafe { SendMessageW(handle as HWND, WM_VSCROLL, code, 0) };
}

/// `WindowsList.GetVisibleWindowsList`: top-level windows in z-order that
/// are visible, not cloaked, titled, not the desktop (`Progman`) or a
/// `Button`, with a non-empty rectangle.
pub fn visible_windows() -> Vec<crate::VisibleWindow> {
    unsafe extern "system" fn each(hwnd: HWND, data: LPARAM) -> i32 {
        // SAFETY: `data` is the Vec below, alive for the enumeration.
        let list = unsafe { &mut *(data as *mut Vec<HWND>) };
        list.push(hwnd);
        1
    }
    let mut handles: Vec<HWND> = Vec::new();
    // SAFETY: the callback only pushes into `handles`.
    unsafe { EnumWindows(Some(each), &raw mut handles as LPARAM) };
    handles
        .into_iter()
        .filter_map(|hwnd| {
            // SAFETY: a window handle from the enumeration.
            if unsafe { IsWindowVisible(hwnd) } == 0 || is_cloaked(hwnd) {
                return None;
            }
            let title = window_info(hwnd as isize).title;
            if title.is_empty() {
                return None;
            }
            let class = class_name(hwnd);
            if ["Progman", "Button"].iter().any(|c| class.eq_ignore_ascii_case(c)) {
                return None;
            }
            let rect = window_rectangle(hwnd);
            // `Rectangle.IsValid`: a positive width and height.
            (rect.w > 0 && rect.h > 0).then_some(crate::VisibleWindow { handle: hwnd as isize, title, rect })
        })
        .collect()
}

/// `NativeMethods.GetApplicationIcon`: the window's small icon (asked of
/// the window, then its class, then `WM_QUERYDRAGICON`), else its big one,
/// as RGBA at its own size (`WindowsImageInterop.FromIcon`).
pub fn window_icon(handle: isize) -> Option<RgbaImage> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GCLP_HICON, GCLP_HICONSM, GetClassLongPtrW, ICON_BIG, ICON_SMALL, ICON_SMALL2, SMTO_ABORTIFHUNG,
        SendMessageTimeoutW, WM_GETICON, WM_QUERYDRAGICON,
    };
    let hwnd = handle as HWND;
    let ask = |msg: u32, w: usize| -> HICON {
        let mut result = 0usize;
        // SAFETY: a valid out pointer; the timeout keeps a hung window
        // from blocking.
        unsafe { SendMessageTimeoutW(hwnd, msg, w, 0, SMTO_ABORTIFHUNG, 1000, &mut result) };
        result as HICON
    };
    // SAFETY: reads the window class's icon handles.
    let class = |index| unsafe { GetClassLongPtrW(hwnd, index) as HICON };
    let tries: [&dyn Fn() -> HICON; 6] = [
        &|| ask(WM_GETICON, ICON_SMALL2 as usize),
        &|| ask(WM_GETICON, ICON_SMALL as usize),
        &|| class(GCLP_HICONSM),
        &|| ask(WM_QUERYDRAGICON, 0),
        &|| ask(WM_GETICON, ICON_BIG as usize),
        &|| class(GCLP_HICON),
    ];
    let icon = tries.iter().map(|f| f()).find(|i| !i.is_null())?;
    from_icon(icon, (0, 0), &RgbaImage::new(0, 0), (0, 0))
}

/// `NativeMethods.SearchWindow`: the window with exactly this title, else
/// the main window of a process whose main window's title contains it.
pub fn search_window(title: &str) -> isize {
    use windows_sys::Win32::UI::WindowsAndMessaging::{FindWindowW, GW_OWNER, GetWindow};
    let wide: Vec<u16> = title.encode_utf16().chain([0]).collect();
    // SAFETY: a NUL-terminated string.
    let found = unsafe { FindWindowW(std::ptr::null(), wide.as_ptr()) };
    if !found.is_null() {
        return found as isize;
    }
    // `Process.MainWindowHandle`: a process's first visible, unowned
    // top-level window.
    let needle = title.to_lowercase();
    visible_windows()
        .into_iter()
        // SAFETY: a window handle from the list.
        .filter(|w| unsafe { GetWindow(w.handle as HWND, GW_OWNER) }.is_null())
        .find(|w| w.title.to_lowercase().contains(&needle))
        .map_or(0, |w| w.handle)
}

/// `Helpers.LockCursorToWindow`'s `NativeMethods.ClipCursor`: keeps the
/// pointer inside the window's rectangle (`GetWindowRect`), or frees it.
pub fn clip_cursor_to_window(handle: Option<isize>) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{ClipCursor, GetWindowRect};
    match handle {
        Some(h) => {
            let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
            // SAFETY: `r` is a valid RECT to fill and then read.
            unsafe {
                if GetWindowRect(h as HWND, &mut r) != 0 && r.right > r.left && r.bottom > r.top {
                    ClipCursor(&r);
                }
            }
        }
        // SAFETY: a null rectangle frees the pointer.
        None => unsafe {
            ClipCursor(std::ptr::null());
        },
    }
}

/// `GetClipCursor`: the rectangle the pointer is kept in (the virtual
/// screen when it's free).
pub fn cursor_clip() -> Rect {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetClipCursor;
    let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    // SAFETY: `r` is a valid RECT to fill.
    unsafe { GetClipCursor(&mut r) };
    Rect::new(r.left, r.top, r.right - r.left, r.bottom - r.top)
}

/// `NativeMethods.IsZoomed`.
pub fn is_zoomed(handle: isize) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::IsZoomed;
    // SAFETY: no pointers.
    unsafe { IsZoomed(handle as HWND) != 0 }
}

/// `Helpers.IsWindows11OrGreater`: build 22000 or later.
pub fn is_windows_11() -> bool {
    let key: Vec<u16> = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion".encode_utf16().chain([0]).collect();
    let name: Vec<u16> = "CurrentBuildNumber\0".encode_utf16().collect();
    let mut buf = [0u16; 32];
    let mut size = (buf.len() * 2) as u32;
    // SAFETY: the buffer and its size in bytes are passed.
    let ok = unsafe {
        RegGetValueW(
            windows_sys::Win32::System::Registry::HKEY_LOCAL_MACHINE,
            key.as_ptr(),
            name.as_ptr(),
            windows_sys::Win32::System::Registry::RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            buf.as_mut_ptr().cast(),
            &mut size,
        )
    } == 0;
    let n = buf.iter().position(|&c| c == 0).unwrap_or(0);
    ok && String::from_utf16_lossy(&buf[..n]).parse::<u32>().is_ok_and(|b| b >= 22000)
}

/// `WindowsNativeWindow("ShareX - Transparent capture background", rect,
/// WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE)`: a popup filled with
/// one colour (`SetBackground`: `UpdateLayeredWindow` of an opaque bitmap),
/// destroyed when dropped.
pub struct BackgroundWindow {
    hwnd: HWND,
    bounds: Rect,
}

impl BackgroundWindow {
    pub fn new(bounds: Rect) -> Option<Self> {
        use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, RegisterClassExW, WNDCLASSEXW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_POPUP,
        };
        if bounds.w <= 0 || bounds.h <= 0 {
            return None;
        }
        let class: Vec<u16> = "ShareXrs.NativeOverlay\0".encode_utf16().collect();
        let title: Vec<u16> = "ShareX - Transparent capture background\0".encode_utf16().collect();
        // SAFETY: the class and title strings outlive the calls; a second
        // registration of the class fails harmlessly.
        unsafe {
            let instance = GetModuleHandleW(std::ptr::null());
            let info = WNDCLASSEXW {
                cbSize: size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(DefWindowProcW),
                hInstance: instance,
                lpszClassName: class.as_ptr(),
                ..std::mem::zeroed()
            };
            RegisterClassExW(&info);
            let hwnd = CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                class.as_ptr(),
                title.as_ptr(),
                WS_POPUP,
                bounds.x,
                bounds.y,
                bounds.w,
                bounds.h,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                instance,
                std::ptr::null(),
            );
            (!hwnd.is_null()).then_some(Self { hwnd, bounds })
        }
    }

    /// `SetBackground(color, bounds)`.
    pub fn set_color(&mut self, rgb: [u8; 3]) {
        use windows_sys::Win32::Foundation::SIZE;
        use windows_sys::Win32::Graphics::Gdi::{
            AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION, CreateCompatibleDC, CreateDIBSection,
            DIB_RGB_COLORS, DeleteDC, DeleteObject, GdiFlush, SelectObject,
        };
        use windows_sys::Win32::UI::WindowsAndMessaging::{ULW_ALPHA, UpdateLayeredWindow};
        let (w, h) = (self.bounds.w, self.bounds.h);
        // SAFETY: a DIB section of w*h BGRA pixels, filled, given to the
        // layered window and freed.
        unsafe {
            let dc = CreateCompatibleDC(std::ptr::null_mut());
            let mut bi: BITMAPINFO = std::mem::zeroed();
            bi.bmiHeader = BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                ..std::mem::zeroed()
            };
            let mut bits: *mut c_void = std::ptr::null_mut();
            let bitmap = CreateDIBSection(dc, &bi, DIB_RGB_COLORS, &mut bits, std::ptr::null_mut(), 0);
            if bitmap.is_null() || bits.is_null() {
                DeleteDC(dc);
                return;
            }
            let previous = SelectObject(dc, bitmap);
            let pixels = std::slice::from_raw_parts_mut(bits.cast::<u8>(), (w * h * 4) as usize);
            for px in pixels.chunks_exact_mut(4) {
                px.copy_from_slice(&[rgb[2], rgb[1], rgb[0], 255]);
            }
            GdiFlush();
            let blend = BLENDFUNCTION { BlendOp: AC_SRC_OVER as u8, BlendFlags: 0, SourceConstantAlpha: 255, AlphaFormat: AC_SRC_ALPHA as u8 };
            let at = POINT { x: self.bounds.x, y: self.bounds.y };
            let size = SIZE { cx: w, cy: h };
            let origin = POINT { x: 0, y: 0 };
            UpdateLayeredWindow(self.hwnd, std::ptr::null_mut(), &at, &size, dc, &origin, 0, &blend, ULW_ALPHA);
            SelectObject(dc, previous);
            DeleteObject(bitmap);
            DeleteDC(dc);
        }
    }

    /// `ShowWindow(ShowNoActivate)`, then `SetWindowPos` right under
    /// `target` (false when that fails).
    pub fn show_behind(&self, target: isize) -> bool {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetWindowPos, ShowWindow,
        };
        // SAFETY: our window and the target's handle.
        unsafe {
            ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
            SetWindowPos(self.hwnd, target as HWND, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE) != 0
        }
    }

    /// `FlushCaptureBackground`: `DwmFlush`, then 10 ms.
    pub fn flush() {
        // SAFETY: no arguments.
        unsafe { windows_sys::Win32::Graphics::Dwm::DwmFlush() };
        std::thread::sleep(Duration::from_millis(10));
    }
}

impl Drop for BackgroundWindow {
    fn drop(&mut self) {
        // SAFETY: our window, no longer used.
        unsafe { windows_sys::Win32::UI::WindowsAndMessaging::DestroyWindow(self.hwnd) };
    }
}

fn find_window(class: &str, parent: HWND, after: HWND, title: Option<&str>) -> HWND {
    use windows_sys::Win32::UI::WindowsAndMessaging::FindWindowExW;
    let class: Vec<u16> = class.encode_utf16().chain([0]).collect();
    let title: Option<Vec<u16>> = title.map(|t| t.encode_utf16().chain([0]).collect());
    // SAFETY: NUL-terminated strings that outlive the call.
    unsafe { FindWindowExW(parent, after, class.as_ptr(), title.as_ref().map_or(std::ptr::null(), |t| t.as_ptr())) }
}

/// The taskbar (`FindWindow("Shell_TrayWnd", null)`).
fn taskbar() -> HWND {
    find_window("Shell_TrayWnd", std::ptr::null_mut(), std::ptr::null_mut(), None)
}

fn show_window(hwnd: HWND, visible: bool) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{SW_HIDE, SW_SHOW, ShowWindow};
    // SAFETY: a window handle; no pointers.
    unsafe { ShowWindow(hwnd, if visible { SW_SHOW } else { SW_HIDE }) };
}

/// `NativeMethods.SetTaskbarVisibilityIfIntersect(visible, rect)`: the
/// taskbar is shown or hidden when it overlaps `rect`; true if it was.
/// (ShareX's Vista/7 Start button branch doesn't apply on Windows 10+.)
pub fn set_taskbar_visibility_if_intersect(visible: bool, rect: Rect) -> bool {
    let h = taskbar();
    if h.is_null() {
        return false;
    }
    let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    // SAFETY: a RECT to fill.
    unsafe { GetWindowRect(h, &mut r) };
    let bar = Rect::new(r.left, r.top, r.right - r.left, r.bottom - r.top);
    // `Rectangle.IntersectsWith`.
    let intersects = rect.x < bar.right() && bar.x < rect.right() && rect.y < bar.bottom() && bar.y < rect.bottom();
    if intersects {
        show_window(h, visible);
    }
    intersects
}

/// `NativeMethods.SetTaskbarVisibility(visible)`.
pub fn set_taskbar_visibility(visible: bool) -> bool {
    let h = taskbar();
    if h.is_null() {
        return false;
    }
    show_window(h, visible);
    true
}

/// Whether the taskbar window is visible (`IsWindowVisible`), for checks.
pub fn is_taskbar_visible() -> Option<bool> {
    let h = taskbar();
    (!h.is_null()).then(|| is_window_visible(h as isize))
}

/// `DesktopIconManager.GetDesktopListViewHandle`: the desktop's
/// `SysListView32` "FolderView" under Progman's or a WorkerW's
/// `SHELLDLL_DefView`.
fn desktop_list_view() -> HWND {
    let null = std::ptr::null_mut();
    let progman = find_window("Progman", null, null, None);
    let mut def_view = find_window("SHELLDLL_DefView", progman, null, None);
    if def_view.is_null() {
        let mut worker = null;
        loop {
            worker = find_window("WorkerW", null, worker, None);
            if worker.is_null() {
                break;
            }
            def_view = find_window("SHELLDLL_DefView", worker, null, None);
            if !def_view.is_null() {
                break;
            }
        }
    }
    if def_view.is_null() { null } else { find_window("SysListView32", def_view, null, Some("FolderView")) }
}

/// `DesktopIconManager.AreDesktopIconsVisible`.
pub fn are_desktop_icons_visible() -> bool {
    let h = desktop_list_view();
    !h.is_null() && is_window_visible(h as isize)
}

/// `DesktopIconManager.SetDesktopIconsVisibility(show)`.
pub fn set_desktop_icons_visibility(show: bool) -> bool {
    let h = desktop_list_view();
    if h.is_null() {
        return false;
    }
    show_window(h, show);
    true
}

/// `CaptureHelpers.IsActiveWindowFullscreen`: the foreground window (not
/// the desktop's Progman or WorkerW) covers its whole monitor.
pub fn is_active_window_fullscreen(screens: &[Screen]) -> bool {
    let h = foreground_window();
    if h == 0 {
        return false;
    }
    let class = class_name(h as HWND);
    if ["Progman", "WorkerW"].iter().any(|c| class.eq_ignore_ascii_case(c)) {
        return false;
    }
    let r = window_rect(h);
    crate::screenshot::screen_from_rectangle(screens, r).is_some_and(|s| r.contains_rect(&s.bounds))
}

#[cfg(test)]
mod tests {
    #[test]
    fn bgra_becomes_opaque_rgba_as_the_dib_copy_and_set_opaque_made_it() {
        let bgra: Vec<u8> = (0..=255u8).cycle().take(4 * 300).collect();
        let mut got = bgra.clone();
        super::bgra_to_opaque_rgba(&mut got);
        let expected: Vec<u8> = bgra.chunks_exact(4).flat_map(|i| [i[2], i[1], i[0], 255]).collect();
        assert_eq!(got, expected);
    }
}
