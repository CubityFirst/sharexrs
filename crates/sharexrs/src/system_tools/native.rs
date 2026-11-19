//! The System tools' native side, behind the traits the windows take (so
//! tests and `--render` never touch the real clipboard, other apps'
//! windows or the network):
//!
//! - [`ClipboardSource`]: `ClipboardHelpers.CaptureData()` as
//!   `AvaloniaClipboard.Capture` reads it: on Windows the clipboard's OLE
//!   data object, its formats in the order it lists them, named and read as
//!   Avalonia's Win32 backend does; on macOS the general pasteboard and on
//!   Linux the X11 `CLIPBOARD` selection as Avalonia.Native and
//!   Avalonia.X11 do (`sharex_tools::system::clipboard_formats`); and
//!   `ClipboardHelpers.Clear`;
//! - [`WindowSource`]: `InspectWindowService` (the window list, a window's
//!   `WindowInfo`, `WindowFromPoint`/`GetAncestor`, top-most and opacity);
//! - [`IcmpPinger`]: `Ping.SendPingAsync` (`IcmpSendEcho` on Windows, the
//!   system `ping` elsewhere);
//! - [`NativeWindowOps`]: `BorderlessWindowManager`'s calls.

use sharex_tools::system::clipboard_viewer::ClipboardSnapshot;
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
use sharex_tools::system::clipboard_viewer::{self as cv, ClipboardValue};
use sharex_tools::system::inspect_window::{Rect, WindowEntry, WindowSnapshot};

/// What reads and clears the clipboard.
pub trait ClipboardSource {
    /// `ClipboardViewerData.Capture()`.
    fn capture(&self) -> Result<ClipboardSnapshot, String>;
    /// `ClipboardHelpers.Clear()`.
    fn clear(&self);
}

/// A window of the lists (`InspectWindowListItem`): the entry and its icon.
#[derive(Clone, Debug)]
pub struct ListedWindow {
    pub entry: WindowEntry,
    pub icon: Option<image::RgbaImage>,
}

/// What lists, reads and changes other windows.
pub trait WindowSource {
    /// `InspectWindowService.GetVisibleWindows(ignored)`.
    fn visible_windows(&self, ignored: isize) -> Vec<ListedWindow>;
    /// A `WindowInfo`'s properties (`UpdateWindowInfo`).
    fn snapshot(&self, handle: isize) -> Option<WindowSnapshot>;
    /// `GetWindowIcon(handle)`.
    fn icon(&self, handle: isize) -> Option<image::RgbaImage>;
    /// `GetWindowAtPoint(x, y, topLevel)`.
    fn window_at_point(&self, x: i32, y: i32, top_level: bool) -> isize;
    /// `WindowInfo.TopMost = value`.
    fn set_top_most(&self, handle: isize, top_most: bool);
    /// `WindowInfo.Opacity = value`.
    fn set_opacity(&self, handle: isize, opacity: u8);
}

/// The real clipboard.
pub struct NativeClipboard {
    /// A window of ours to own the emptied clipboard.
    pub owner: Option<isize>,
}

impl ClipboardSource for NativeClipboard {
    fn capture(&self) -> Result<ClipboardSnapshot, String> {
        capture_clipboard()
    }

    fn clear(&self) {
        if let Err(e) = sharex_capture::clipboard::clear(self.owner) {
            eprintln!("ClipboardHelpers.Clear: {e}");
        }
    }
}

/// The real desktop's windows.
pub struct NativeWindows;

impl WindowSource for NativeWindows {
    fn visible_windows(&self, ignored: isize) -> Vec<ListedWindow> {
        let entries: Vec<WindowEntry> = sharex_capture::platform::visible_windows()
            .into_iter()
            .map(|w| WindowEntry {
                handle: w.handle,
                title: w.title,
                class_name: class_name(w.handle),
                process_name: sharex_capture::platform::window_info(w.handle).process_name.unwrap_or_default(),
                visible: true,
                cloaked: false,
                rectangle: Rect { x: w.rect.x, y: w.rect.y, width: w.rect.w, height: w.rect.h },
            })
            .collect();
        sharex_tools::system::inspect_window::visible_windows(entries, ignored)
            .into_iter()
            .map(|entry| ListedWindow { icon: sharex_capture::platform::window_icon(entry.handle), entry })
            .collect()
    }

    fn snapshot(&self, handle: isize) -> Option<WindowSnapshot> {
        window_snapshot(handle)
    }

    fn icon(&self, handle: isize) -> Option<image::RgbaImage> {
        (handle != 0).then(|| sharex_capture::platform::window_icon(handle)).flatten()
    }

    fn window_at_point(&self, x: i32, y: i32, top_level: bool) -> isize {
        window_at_point(x, y, top_level)
    }

    fn set_top_most(&self, handle: isize, top_most: bool) {
        set_top_most(handle, top_most);
    }

    fn set_opacity(&self, handle: isize, opacity: u8) {
        set_opacity(handle, opacity);
    }
}

fn to_rect(r: sharex_capture::Rect) -> Rect {
    Rect { x: r.x, y: r.y, width: r.w, height: r.h }
}

// ----------------------------------------------------------------------
// Windows.

#[cfg(windows)]
mod win {
    use super::*;
    use windows_sys::Win32::Foundation::{HWND, POINT};
    use windows_sys::Win32::UI::WindowsAndMessaging as wm;

    pub fn class_name(handle: isize) -> String {
        let mut buf = [0u16; 256];
        // SAFETY: the buffer's length is passed.
        let n = unsafe { wm::GetClassNameW(handle as HWND, buf.as_mut_ptr(), buf.len() as i32) };
        String::from_utf16_lossy(&buf[..n.max(0) as usize])
    }

    /// `Process.MainModule.FileName` by way of `QueryFullProcessImageName`, and the id.
    fn process(handle: isize) -> (u32, String) {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW};
        let mut pid = 0u32;
        // SAFETY: a u32 out pointer.
        unsafe { wm::GetWindowThreadProcessId(handle as HWND, &mut pid) };
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        // SAFETY: the process handle is checked and closed; the buffer is described by `len`.
        let ok = unsafe {
            let p = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if p.is_null() {
                return (pid, String::new());
            }
            let ok = QueryFullProcessImageNameW(p, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len);
            CloseHandle(p);
            ok != 0
        };
        (pid, if ok { String::from_utf16_lossy(&buf[..len as usize]) } else { String::new() })
    }

    pub fn window_snapshot(handle: isize) -> Option<WindowSnapshot> {
        let hwnd = handle as HWND;
        // SAFETY: plain queries on a window handle.
        if handle == 0 || unsafe { wm::IsWindow(hwnd) } == 0 {
            return None;
        }
        let info = sharex_capture::platform::window_info(handle);
        let (pid, path) = process(handle);
        // SAFETY: plain queries on a window handle.
        let (style, ex_style) = unsafe { (wm::GetWindowLongW(hwnd, wm::GWL_STYLE) as u32, wm::GetWindowLongW(hwnd, wm::GWL_EXSTYLE) as u32) };
        let layered = ex_style & wm::WS_EX_LAYERED != 0;
        let mut opacity = 255u8;
        if layered {
            let mut alpha = 255u8;
            // SAFETY: out pointers for the attributes; nulls for the ones not wanted.
            unsafe { wm::GetLayeredWindowAttributes(hwnd, std::ptr::null_mut(), &mut alpha, std::ptr::null_mut()) };
            opacity = alpha;
        }
        Some(WindowSnapshot {
            handle,
            title: info.title,
            class_name: class_name(handle),
            process_name: info.process_name.unwrap_or_default(),
            // `FileHelpers.GetFileNameSafe(ProcessFilePath)`.
            process_file_name: std::path::Path::new(&path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            process_id: if pid == 0 { String::new() } else { pid.to_string() },
            rectangle: to_rect(sharex_capture::platform::window_rect(handle)),
            client_rectangle: to_rect(sharex_capture::platform::client_rect(handle)),
            style: Some(style),
            ex_style: Some(ex_style),
            top_most: ex_style & wm::WS_EX_TOPMOST != 0,
            opacity,
        })
    }

    pub fn window_at_point(x: i32, y: i32, top_level: bool) -> isize {
        // SAFETY: plain window queries.
        unsafe {
            let h = wm::WindowFromPoint(POINT { x, y });
            if top_level && !h.is_null() { wm::GetAncestor(h, wm::GA_ROOT) as isize } else { h as isize }
        }
    }

    pub fn set_top_most(handle: isize, top_most: bool) {
        let after = if top_most { wm::HWND_TOPMOST } else { wm::HWND_NOTOPMOST };
        // SAFETY: moves the window in the z-order only.
        unsafe { wm::SetWindowPos(handle as HWND, after, 0, 0, 0, 0, wm::SWP_NOMOVE | wm::SWP_NOSIZE) };
    }

    pub fn set_opacity(handle: isize, opacity: u8) {
        let hwnd = handle as HWND;
        // SAFETY: the window's extended style and layered attributes.
        unsafe {
            let ex = wm::GetWindowLongW(hwnd, wm::GWL_EXSTYLE) as u32;
            if opacity < 255 {
                wm::SetWindowLongW(hwnd, wm::GWL_EXSTYLE, (ex | wm::WS_EX_LAYERED) as i32);
                wm::SetLayeredWindowAttributes(hwnd, 0, opacity, wm::LWA_ALPHA);
            } else {
                wm::SetWindowLongW(hwnd, wm::GWL_EXSTYLE, (ex & !wm::WS_EX_LAYERED) as i32);
            }
        }
    }

    /// `ClipboardFormatRegistry.GetFormatSystemName`: a registered format's
    /// name, the five Avalonia knows by name, else `Unknown_Format_<id>`.
    fn format_name(id: u16) -> String {
        use windows_sys::Win32::System::DataExchange::GetClipboardFormatNameW;
        let mut buf = [0u16; 260];
        // SAFETY: the buffer's length is passed.
        let n = unsafe { GetClipboardFormatNameW(id as u32, buf.as_mut_ptr(), buf.len() as i32) };
        if n > 0 {
            return String::from_utf16_lossy(&buf[..n as usize]);
        }
        match id {
            2 => "CF_BITMAP".into(),
            8 => "CF_DIB".into(),
            13 => "CF_UNICODETEXT".into(),
            15 => "CF_HDROP".into(),
            17 => "CF_DIBV5".into(),
            _ => format!("Unknown_Format_{id}"),
        }
    }

    /// What `TryGetRawAsync` gives for a format, as bytes, text or files.
    enum Raw {
        Text(String),
        Files(Vec<String>),
        Bytes(Vec<u8>),
        Image(image::RgbaImage),
    }

    /// `AvaloniaClipboard.Capture` over the clipboard's OLE data object.
    pub fn capture() -> Result<ClipboardSnapshot, String> {
        use windows::Win32::System::Com::IDataObject;
        use windows::Win32::System::Ole::{OleGetClipboard, OleInitialize};
        // SAFETY: OLE on this (the UI) thread; already initialised by winit
        // for drag and drop, which makes this a no-op.
        let _ = unsafe { OleInitialize(None) };
        // SAFETY: a COM call returning an owned interface.
        let data: IDataObject = match unsafe { OleGetClipboard() } {
            Ok(d) => d,
            Err(_) => return Ok(ClipboardSnapshot::new()),
        };
        Ok(read_data_object(&data))
    }

    /// An OLE data object (the clipboard's, or a drag's) read as Avalonia's
    /// Win32 `OleDataObject`-backed data transfer reads it: every format in
    /// the object's order, named as Avalonia names them, and `Bitmap`.
    pub fn read_data_object(data: &windows::Win32::System::Com::IDataObject) -> ClipboardSnapshot {
        use windows::Win32::System::Com::{DATADIR_GET, DVASPECT_CONTENT, FORMATETC, TYMED_GDI, TYMED_HGLOBAL};
        use windows::Win32::System::Ole::ReleaseStgMedium;
        let mut snapshot = ClipboardSnapshot::new();
        // `ProvideFormats`: the data object's formats in its order.
        let mut ids: Vec<u16> = Vec::new();
        // SAFETY: COM calls on owned interfaces; `ptd` is freed as Avalonia does.
        if let Ok(e) = unsafe { data.EnumFormatEtc(DATADIR_GET.0 as u32) } {
            unsafe {
                let _ = e.Reset();
                loop {
                    let mut f = [FORMATETC::default()];
                    let mut fetched = 0u32;
                    if e.Next(&mut f, Some(&mut fetched)).is_err() || fetched == 0 {
                        break;
                    }
                    if !f[0].ptd.is_null() {
                        windows::Win32::System::Com::CoTaskMemFree(Some(f[0].ptd as *const _));
                    }
                    ids.push(f[0].cfFormat);
                }
            }
        }
        let get = |id: u16| -> Option<Raw> {
            let fe = FORMATETC {
                cfFormat: id,
                ptd: std::ptr::null_mut(),
                dwAspect: DVASPECT_CONTENT.0,
                lindex: -1,
                tymed: if id == 2 { TYMED_GDI.0 as u32 } else { TYMED_HGLOBAL.0 as u32 },
            };
            // SAFETY: `GetData` hands an owned medium, released below.
            let mut medium = unsafe { data.GetData(&fe) }.ok()?;
            let out = unsafe {
                if medium.tymed == TYMED_HGLOBAL.0 as u32 {
                    let h = medium.u.hGlobal;
                    match id {
                        13 => read_text(h.0 as isize).map(Raw::Text),
                        15 => Some(Raw::Files(read_files(h.0 as isize))),
                        _ => Some(Raw::Bytes(read_bytes(h.0 as isize))),
                    }
                } else if medium.tymed == TYMED_GDI.0 as u32 {
                    hbitmap_image(medium.u.hBitmap.0 as isize).map(Raw::Image)
                } else {
                    None
                }
            };
            unsafe { ReleaseStgMedium(&mut medium) };
            out
        };
        let png = |img: &image::RgbaImage| sharex_capture::clipboard::png_bytes(img).ok();
        let mut has_image = false;
        for &id in &ids {
            let name = format_name(id);
            if ["image/png", "PNG"].contains(&name.as_str()) || matches!(id, 2 | 8 | 17) {
                has_image = true;
            }
            match id {
                // `DataFormat.Text`, `DataFormat.File`.
                13 => {
                    if let Some(Raw::Text(t)) = get(id) {
                        snapshot.set(cv::FORMAT_UNICODE_TEXT, ClipboardValue::Text(t));
                    }
                }
                15 => {
                    if let Some(Raw::Files(f)) = get(id) {
                        snapshot.set(cv::FORMAT_FILE_DROP, ClipboardValue::Files(f));
                    }
                }
                // `CF_DIB` / `CF_DIBV5` are `DataFormat<Bitmap>`s that
                // `ReadDataFromHGlobal` gives no value for: not listed.
                8 | 17 => {}
                // `CF_BITMAP`: the GDI bitmap, saved as PNG.
                2 => {
                    if let Some(Raw::Image(img)) = get(id)
                        && let Some(bytes) = png(&img)
                    {
                        snapshot.set("CF_BITMAP", ClipboardValue::Bytes(bytes));
                    }
                }
                _ => {
                    if let Some(Raw::Bytes(b)) = get(id) {
                        snapshot.set(&name, ClipboardValue::Bytes(b));
                    }
                }
            }
        }
        if has_image {
            // `DataFormat.Bitmap`: the first of PNG, `image/png`, `CF_DIB`,
            // `CF_DIBV5` (whose raw bytes Avalonia can't decode),
            // `CF_BITMAP`, as a bitmap saved to PNG.
            let image = ids
                .iter()
                .find_map(|&id| {
                    let n = format_name(id);
                    (n == "image/png").then(|| get(id)).flatten()
                })
                .or_else(|| ids.iter().find_map(|&id| (format_name(id) == "PNG").then(|| get(id)).flatten()))
                .and_then(|r| match r {
                    Raw::Bytes(b) => image::load_from_memory(&b).ok().map(|i| i.to_rgba8()),
                    _ => None,
                })
                .or_else(|| {
                    ids.contains(&8).then(|| get(8)).flatten().and_then(|r| match r {
                        Raw::Bytes(b) => sharex_capture::clipboard::image_from_dib(&b).map(|mut i| {
                            // `StretchDIBits` into an opaque 32 bpp section.
                            i.pixels_mut().for_each(|p| p[3] = 255);
                            i
                        }),
                        _ => None,
                    })
                })
                .or_else(|| {
                    (!ids.contains(&17)).then_some(()).and(ids.contains(&2).then(|| get(2)).flatten()).and_then(|r| match r {
                        Raw::Image(i) => Some(i),
                        _ => None,
                    })
                });
            if let Some(bytes) = image.as_ref().and_then(png) {
                snapshot.set(cv::FORMAT_BITMAP, ClipboardValue::Bytes(bytes));
            }
        }
        snapshot
    }

    unsafe fn read_bytes(h: isize) -> Vec<u8> {
        use windows_sys::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
        // SAFETY: the caller's HGLOBAL, locked for the copy.
        unsafe {
            let size = GlobalSize(h as _);
            let p = GlobalLock(h as _) as *const u8;
            if p.is_null() {
                return Vec::new();
            }
            let v = std::slice::from_raw_parts(p, size).to_vec();
            GlobalUnlock(h as _);
            v
        }
    }

    /// `ReadStringFromHGlobal`: UTF-16 up to the first NUL.
    unsafe fn read_text(h: isize) -> Option<String> {
        // SAFETY: as `read_bytes`.
        let b = unsafe { read_bytes(h) };
        let units: Vec<u16> = b.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).take_while(|u| *u != 0).collect();
        Some(String::from_utf16_lossy(&units))
    }

    /// `ReadFileNamesFromHGlobal`: `DragQueryFile` on the `HDROP`.
    unsafe fn read_files(h: isize) -> Vec<String> {
        use windows_sys::Win32::UI::Shell::DragQueryFileW;
        // SAFETY: the HDROP from the medium; buffer lengths are passed.
        unsafe {
            let count = DragQueryFileW(h as _, u32::MAX, std::ptr::null_mut(), 0);
            (0..count)
                .map(|i| {
                    let len = DragQueryFileW(h as _, i, std::ptr::null_mut(), 0) as usize;
                    let mut buf = vec![0u16; len + 1];
                    DragQueryFileW(h as _, i, buf.as_mut_ptr(), buf.len() as u32);
                    String::from_utf16_lossy(&buf[..len])
                })
                .filter(|p| !p.is_empty())
                .collect()
        }
    }

    /// `ReadDataFromGdi`: the bitmap drawn into an opaque top-down 32 bpp section.
    unsafe fn hbitmap_image(h: isize) -> Option<image::RgbaImage> {
        use windows_sys::Win32::Graphics::Gdi::{BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, GetDC, GetDIBits, GetObjectW, ReleaseDC};
        // SAFETY: reads the bitmap's header and bits into buffers we own.
        unsafe {
            let mut bm: BITMAP = std::mem::zeroed();
            if GetObjectW(h as _, std::mem::size_of::<BITMAP>() as i32, (&raw mut bm).cast()) == 0 || bm.bmWidth <= 0 || bm.bmHeight <= 0 {
                return None;
            }
            let (w, hgt) = (bm.bmWidth, bm.bmHeight);
            let mut info: BITMAPINFO = std::mem::zeroed();
            info.bmiHeader = BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -hgt,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                ..std::mem::zeroed()
            };
            let mut px = vec![0u8; (w * hgt * 4) as usize];
            let dc = GetDC(std::ptr::null_mut());
            let rows = GetDIBits(dc, h as _, 0, hgt as u32, px.as_mut_ptr().cast(), &mut info, DIB_RGB_COLORS);
            ReleaseDC(std::ptr::null_mut(), dc);
            if rows == 0 {
                return None;
            }
            for p in px.chunks_exact_mut(4) {
                p.swap(0, 2);
                p[3] = 255;
            }
            image::RgbaImage::from_raw(w as u32, hgt as u32, px)
        }
    }

    /// `Ping.SendPingAsync(host, timeout)` as `IcmpSendEcho` (IPv4 literals,
    /// which is all the monitor's targets are).
    pub fn ping(host: &str, timeout: std::time::Duration) -> sharex_tools::system::network_monitor::EchoReply {
        use sharex_tools::system::network_monitor::EchoReply;
        use windows_sys::Win32::NetworkManagement::IpHelper::{ICMP_ECHO_REPLY, IcmpCloseHandle, IcmpCreateFile, IcmpSendEcho};
        let Ok(addr) = host.parse::<std::net::Ipv4Addr>() else {
            return EchoReply::Failed(format!("{host}: not an IPv4 address"));
        };
        let started = std::time::Instant::now();
        // SAFETY: the handle is checked and closed; the reply buffer is
        // sized for one reply and the request data.
        unsafe {
            let h = IcmpCreateFile();
            if h.is_null() || h as isize == -1 {
                return EchoReply::Failed(std::io::Error::last_os_error().to_string());
            }
            let data = [0u8; 32];
            let mut reply = vec![0u8; std::mem::size_of::<ICMP_ECHO_REPLY>() + data.len() + 8];
            let n = IcmpSendEcho(
                h,
                u32::from_ne_bytes(addr.octets()),
                data.as_ptr().cast(),
                data.len() as u16,
                std::ptr::null(),
                reply.as_mut_ptr().cast(),
                reply.len() as u32,
                timeout.as_millis() as u32,
            );
            IcmpCloseHandle(h);
            if n == 0 {
                const IP_REQ_TIMED_OUT: u32 = 11010;
                let err = windows_sys::Win32::Foundation::GetLastError();
                return if err == IP_REQ_TIMED_OUT { EchoReply::TimedOut } else { EchoReply::Failed(std::io::Error::from_raw_os_error(err as i32).to_string()) };
            }
            let r = &*(reply.as_ptr() as *const ICMP_ECHO_REPLY);
            match r.Status {
                0 => {
                    // A 0 ms reply takes the stopwatch's time, as ShareX does.
                    let rtt = r.RoundTripTime as f64;
                    EchoReply::Success { latency_ms: if rtt > 0.0 { rtt } else { started.elapsed().as_secs_f64() * 1000.0 } }
                }
                11010 => EchoReply::TimedOut,
                s => EchoReply::Failed(ip_status_name(s)),
            }
        }
    }

    /// `IPStatus.ToString()` for the statuses an echo can end with.
    fn ip_status_name(status: u32) -> String {
        match status {
            11002 => "DestinationNetworkUnreachable",
            11003 => "DestinationHostUnreachable",
            11004 => "DestinationProtocolUnreachable",
            11005 => "DestinationPortUnreachable",
            11010 => "TimedOut",
            11013 => "TtlExpired",
            11050 => "Unknown",
            _ => return status.to_string(),
        }
        .into()
    }

    /// `BorderlessWindowManager`'s native calls.
    pub mod ops {
        use super::*;

        pub fn style(handle: isize, ex: bool) -> u32 {
            // SAFETY: a window style read.
            unsafe { wm::GetWindowLongW(handle as HWND, if ex { wm::GWL_EXSTYLE } else { wm::GWL_STYLE }) as u32 }
        }

        pub fn set_style(handle: isize, ex: bool, value: u32) {
            // SAFETY: a window style write.
            unsafe { wm::SetWindowLongW(handle as HWND, if ex { wm::GWL_EXSTYLE } else { wm::GWL_STYLE }, value as i32) };
        }

        /// `GetWindowRect`.
        pub fn window_rect(handle: isize) -> Rect {
            let mut r = windows_sys::Win32::Foundation::RECT { left: 0, top: 0, right: 0, bottom: 0 };
            // SAFETY: a RECT out pointer.
            unsafe { wm::GetWindowRect(handle as HWND, &mut r) };
            Rect { x: r.left, y: r.top, width: r.right - r.left, height: r.bottom - r.top }
        }

        /// `CaptureHelpers.GetScreenBounds`: the window's monitor or its working area.
        pub fn screen_bounds(handle: isize, working_area: bool) -> Rect {
            use windows_sys::Win32::Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow};
            // SAFETY: the monitor of a window, and a sized MONITORINFO.
            unsafe {
                let m = MonitorFromWindow(handle as HWND, MONITOR_DEFAULTTONEAREST);
                if m.is_null() {
                    return Rect::default();
                }
                let mut info: MONITORINFO = std::mem::zeroed();
                info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
                if GetMonitorInfoW(m, &mut info) == 0 {
                    return Rect::default();
                }
                let r = if working_area { info.rcWork } else { info.rcMonitor };
                Rect { x: r.left, y: r.top, width: r.right - r.left, height: r.bottom - r.top }
            }
        }

        pub fn set_window_pos(handle: isize, r: Rect, move_and_size: bool) {
            let mut flags = wm::SWP_FRAMECHANGED | wm::SWP_NOOWNERZORDER | wm::SWP_NOZORDER;
            if !move_and_size {
                flags |= wm::SWP_NOMOVE | wm::SWP_NOSIZE;
            }
            // SAFETY: moves and resizes the window.
            unsafe { wm::SetWindowPos(handle as HWND, std::ptr::null_mut(), r.x, r.y, r.width, r.height, flags) };
        }
    }
}

#[cfg(windows)]
fn class_name(handle: isize) -> String {
    win::class_name(handle)
}

#[cfg(windows)]
fn window_snapshot(handle: isize) -> Option<WindowSnapshot> {
    win::window_snapshot(handle)
}

#[cfg(windows)]
fn window_at_point(x: i32, y: i32, top_level: bool) -> isize {
    win::window_at_point(x, y, top_level)
}

#[cfg(windows)]
fn set_top_most(handle: isize, top_most: bool) {
    win::set_top_most(handle, top_most);
}

#[cfg(windows)]
fn set_opacity(handle: isize, opacity: u8) {
    win::set_opacity(handle, opacity);
}

#[cfg(windows)]
fn capture_clipboard() -> Result<ClipboardSnapshot, String> {
    win::capture()
}

/// A drag's OLE data object read as Avalonia reads it (the drop target's
/// `DragEventArgs.DataTransfer`).
#[cfg(windows)]
pub fn read_data_object(data: &windows::Win32::System::Com::IDataObject) -> ClipboardSnapshot {
    win::read_data_object(data)
}

// ----------------------------------------------------------------------
// Elsewhere: what the platform's clipboard and window lists give.

#[cfg(not(windows))]
fn class_name(_handle: isize) -> String {
    String::new()
}

#[cfg(not(windows))]
fn window_snapshot(handle: isize) -> Option<WindowSnapshot> {
    let info = sharex_capture::platform::window_info(handle);
    Some(WindowSnapshot {
        handle,
        title: info.title,
        process_name: info.process_name.unwrap_or_default(),
        rectangle: to_rect(sharex_capture::platform::window_rect(handle)),
        client_rectangle: to_rect(sharex_capture::platform::client_rect(handle)),
        opacity: 255,
        ..Default::default()
    })
}

#[cfg(not(windows))]
fn window_at_point(_x: i32, _y: i32, _top_level: bool) -> isize {
    0
}

#[cfg(not(windows))]
fn set_top_most(_handle: isize, _top_most: bool) {}

#[cfg(not(windows))]
fn set_opacity(_handle: isize, _opacity: u8) {}

/// `AvaloniaClipboard.Capture` on macOS: the general pasteboard as
/// Avalonia.Native lists and reads it (`sharex_tools::system::clipboard_formats`).
#[cfg(target_os = "macos")]
fn capture_clipboard() -> Result<ClipboardSnapshot, String> {
    Ok(sharex_tools::system::clipboard_formats::capture_macos(&mac::GeneralPasteboard::new()))
}

/// `AvaloniaClipboard.Capture` on Linux: the `CLIPBOARD` selection as
/// Avalonia.X11 lists and reads it; nothing without an X server (Avalonia
/// has no other backend there).
#[cfg(target_os = "linux")]
fn capture_clipboard() -> Result<ClipboardSnapshot, String> {
    Ok(match super::x11_selection::X11Selection::open() {
        Some(sel) => sharex_tools::system::clipboard_formats::capture_x11(&sel),
        None => ClipboardSnapshot::new(),
    })
}

/// Elsewhere (the BSDs): text, files and an image, as arboard reads them.
#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
fn capture_clipboard() -> Result<ClipboardSnapshot, String> {
    let mut s = ClipboardSnapshot::new();
    if let Some(t) = sharex_capture::clipboard::get_text() {
        s.set(cv::FORMAT_UNICODE_TEXT, ClipboardValue::Text(t));
    }
    if let Some(files) = sharex_capture::clipboard::get_file_drop_list() {
        s.set(cv::FORMAT_FILE_DROP, ClipboardValue::Files(files.iter().map(|p| p.to_string_lossy().into_owned()).collect()));
    }
    if let Some(img) = sharex_capture::clipboard::get_image()
        && let Ok(png) = sharex_capture::clipboard::png_bytes(&img)
    {
        s.set(cv::FORMAT_BITMAP, ClipboardValue::Bytes(png));
    }
    Ok(s)
}

/// The general pasteboard as `libAvaloniaNative`'s `Clipboard` reads it.
#[cfg(target_os = "macos")]
mod mac {
    use objc2::rc::Retained;
    use objc2::runtime::{AnyClass, AnyObject, Bool};
    use objc2::msg_send;
    use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep, NSPasteboard, NSPasteboardItem};
    use objc2_foundation::{NSArray, NSDictionary, NSString, NSURL};
    use sharex_tools::system::clipboard_formats::{self as cf, Pasteboard};

    pub struct GeneralPasteboard {
        pasteboard: Retained<NSPasteboard>,
        items: Vec<Retained<NSPasteboardItem>>,
    }

    impl GeneralPasteboard {
        pub fn new() -> Self {
            let pasteboard = NSPasteboard::generalPasteboard();
            let items = pasteboard.pasteboardItems().map(|a| a.to_vec()).unwrap_or_default();
            Self { pasteboard, items }
        }
    }

    fn strings(types: &NSArray<NSString>) -> Vec<String> {
        types.iter().map(|t| t.to_string()).collect()
    }

    impl Pasteboard for GeneralPasteboard {
        fn types(&self) -> Vec<String> {
            self.pasteboard.types().map(|t| strings(&t)).unwrap_or_default()
        }

        fn item_types(&self) -> Vec<Vec<String>> {
            self.items.iter().map(|i| strings(&i.types())).collect()
        }

        fn string(&self, item: usize, ty: &str) -> Option<String> {
            self.items.get(item)?.stringForType(&NSString::from_str(ty)).map(|s| s.to_string())
        }

        fn bytes(&self, item: usize, ty: &str) -> Option<Vec<u8>> {
            let item = self.items.get(item)?;
            if let Some(data) = item.dataForType(&NSString::from_str(ty)) {
                return (data.length() > 0).then(|| data.to_vec());
            }
            if ty != cf::PASTEBOARD_PNG {
                return None;
            }
            // The TIFF's, else the JPEG's, bitmap representation as PNG.
            let data = item.dataForType(&NSString::from_str(cf::PASTEBOARD_TIFF)).or_else(|| item.dataForType(&NSString::from_str(cf::PASTEBOARD_JPEG)))?;
            let rep = NSBitmapImageRep::imageRepWithData(&data)?;
            let props = NSDictionary::new();
            // SAFETY: an empty properties dictionary.
            let png = unsafe { rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &props) }?;
            Some(png.to_vec())
        }

        fn is_text_format(&self, ty: &str) -> bool {
            // `[[UTType typeWithIdentifier:format] conformsToType:UTTypeText]`.
            let Some(class) = AnyClass::get(c"UTType") else { return false };
            let id = NSString::from_str(ty);
            let text = NSString::from_str("public.text");
            // SAFETY: UTType's class methods, returning autoreleased objects or nil.
            unsafe {
                let t: Option<Retained<AnyObject>> = msg_send![class, typeWithIdentifier: &*id];
                let base: Option<Retained<AnyObject>> = msg_send![class, typeWithIdentifier: &*text];
                match (t, base) {
                    (Some(t), Some(base)) => {
                        let yes: Bool = msg_send![&*t, conformsToType: &*base];
                        yes.as_bool()
                    }
                    _ => false,
                }
            }
        }

        fn file_path(&self, url: &str) -> Option<String> {
            let mut path = cf::file_uri_local_path(url)?;
            // `TryResolveFileReferenceUri`.
            if path.starts_with("/.file/id=") {
                path = NSURL::URLWithString(&NSString::from_str(url))?.filePathURL()?.path()?.to_string();
            }
            let p = std::path::Path::new(&path);
            (p.is_dir() || p.is_file()).then_some(path)
        }
    }
}

/// ICMP echoes for the network monitor.
pub struct IcmpPinger;

impl sharex_tools::system::network_monitor::Pinger for IcmpPinger {
    #[cfg(windows)]
    fn ping(&self, host: &str, timeout: std::time::Duration) -> sharex_tools::system::network_monitor::EchoReply {
        win::ping(host, timeout)
    }

    /// .NET's `Ping` on Unix: an ICMP socket where allowed, else the
    /// system `ping` (`sharex_tools::system::unix_ping`).
    #[cfg(not(windows))]
    fn ping(&self, host: &str, timeout: std::time::Duration) -> sharex_tools::system::network_monitor::EchoReply {
        sharex_tools::system::unix_ping::ping(host, timeout)
    }
}

/// `BorderlessWindowManager`'s calls on real windows.
pub struct NativeWindowOps;

impl sharex_tools::system::borderless_window::WindowOps for NativeWindowOps {
    fn search_window(&self, title: &str) -> Option<isize> {
        let h = sharex_capture::platform::search_window(title);
        (h != 0).then_some(h)
    }

    fn is_minimized(&self, handle: isize) -> bool {
        sharex_capture::platform::is_minimized(handle)
    }

    fn restore(&mut self, handle: isize) {
        sharex_capture::platform::restore_window(handle);
    }

    #[cfg(windows)]
    fn window_rect(&self, handle: isize) -> Rect {
        win::ops::window_rect(handle)
    }
    #[cfg(not(windows))]
    fn window_rect(&self, handle: isize) -> Rect {
        to_rect(sharex_capture::platform::window_rect(handle))
    }

    #[cfg(windows)]
    fn style(&self, handle: isize) -> u32 {
        win::ops::style(handle, false)
    }
    #[cfg(not(windows))]
    fn style(&self, _handle: isize) -> u32 {
        0
    }

    #[cfg(windows)]
    fn ex_style(&self, handle: isize) -> u32 {
        win::ops::style(handle, true)
    }
    #[cfg(not(windows))]
    fn ex_style(&self, _handle: isize) -> u32 {
        0
    }

    #[cfg(windows)]
    fn set_style(&mut self, handle: isize, style: u32) {
        win::ops::set_style(handle, false, style);
    }
    #[cfg(not(windows))]
    fn set_style(&mut self, _handle: isize, _style: u32) {}

    #[cfg(windows)]
    fn set_ex_style(&mut self, handle: isize, style: u32) {
        win::ops::set_style(handle, true, style);
    }
    #[cfg(not(windows))]
    fn set_ex_style(&mut self, _handle: isize, _style: u32) {}

    #[cfg(windows)]
    fn screen_bounds(&self, handle: isize, working_area: bool) -> Rect {
        win::ops::screen_bounds(handle, working_area)
    }
    #[cfg(not(windows))]
    fn screen_bounds(&self, _handle: isize, _working_area: bool) -> Rect {
        Rect::default()
    }

    #[cfg(windows)]
    fn set_window_pos(&mut self, handle: isize, rect: Rect, move_and_size: bool) {
        win::ops::set_window_pos(handle, rect, move_and_size);
    }
    #[cfg(not(windows))]
    fn set_window_pos(&mut self, _handle: isize, _rect: Rect, _move_and_size: bool) {}
}
