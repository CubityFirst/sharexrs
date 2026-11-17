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
    CloseClipboard, EmptyClipboard, OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
};
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
pub fn capture_rectangle(r: Rect) -> Option<RgbaImage> {
    // SAFETY: the desktop window's DC, released below.
    let desktop = unsafe { GetDesktopWindow() };
    let source = unsafe { GetWindowDC(desktop) };
    if source.is_null() {
        return None;
    }
    let mut image = draw_native(r.w, r.h, None, |dc| {
        // SAFETY: both DCs are live for the call.
        unsafe { BitBlt(dc, 0, 0, r.w, r.h, source, r.x, r.y, SRCCOPY | CAPTUREBLT) };
    });
    // SAFETY: as above.
    unsafe { ReleaseDC(desktop, source) };
    // `SetOpaque`
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
    let handle = info.hCursor;
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
    let draw_at = (info.ptScreenPos.x - hotspot.0 - origin.0, info.ptScreenPos.y - hotspot.1 - origin.1);
    let image = from_icon(handle, size, background, draw_at)?;
    Some(CursorImage { image, position: draw_at })
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
pub fn prepare_overlay(window: &winit::window::Window) {
    use windows_sys::Win32::Foundation::{LRESULT, WPARAM};
    use windows_sys::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
    use windows_sys::Win32::UI::WindowsAndMessaging::{SC_KEYMENU, WM_SYSCOMMAND};

    unsafe extern "system" fn proc(h: HWND, msg: u32, w: WPARAM, l: LPARAM, _: usize, _: usize) -> LRESULT {
        if msg == WM_SYSCOMMAND && (w & 0xfff0) == SC_KEYMENU as usize {
            return 0;
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
