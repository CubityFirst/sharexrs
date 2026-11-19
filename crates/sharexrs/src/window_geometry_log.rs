//! `HelpersLib.WindowGeometryLog`: where the last capture overlays actually
//! opened (Windows' window and client rectangles, its DPI, the scale the
//! window draws at, the pointer), for display diagnostics' "Recent capture
//! windows". At most ten entries. ShareXrs drops no DPI changes, so the
//! entries have no `DpiChanges`; the geometry checks of ShareX's own
//! overlay (its toolbar and magnifier positions) aren't recorded.

use std::sync::Mutex;

/// `MaximumEntries`.
#[cfg_attr(not(windows), allow(dead_code))]
const MAXIMUM_ENTRIES: usize = 10;

/// `WindowGeometryEntry`.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Entry {
    /// `HH:mm:ss` when it opened.
    pub time: String,
    pub source: String,
    pub expected: sharex_capture::Rect,
    pub actual: sharex_capture::Rect,
    pub client_origin: (i32, i32),
    pub image_size: (u32, u32),
    pub window_dpi: u32,
    pub render_scaling: f64,
    /// The client size in DIPs.
    pub client_size: (f64, f64),
    pub monitor_device_name: String,
    pub cursor_screen: Option<(i32, i32)>,
    /// Where the window's input sees the pointer, in client pixels.
    pub cursor_client_pixels: Option<(f64, f64)>,
}

static ENTRIES: Mutex<Vec<Entry>> = Mutex::new(Vec::new());

/// `Entries`, oldest first.
pub fn entries() -> Vec<Entry> {
    ENTRIES.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

#[cfg_attr(not(windows), allow(dead_code))]
fn push(entry: Entry) {
    let mut e = ENTRIES.lock().unwrap_or_else(|e| e.into_inner());
    e.push(entry);
    if e.len() > MAXIMUM_ENTRIES {
        e.remove(0);
    }
}

/// `Record(window, source, expectedBounds, imageSize)`: what Windows made of
/// the window once it's shown. Nothing off Windows, as ShareX.
pub fn record(window: &winit::window::Window, source: &str, expected: sharex_capture::Rect, image_size: (u32, u32)) {
    #[cfg(windows)]
    {
        use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
        use windows_sys::Win32::Foundation::{HWND, POINT, RECT};
        use windows_sys::Win32::Graphics::Gdi::{ClientToScreen, MONITOR_DEFAULTTONEAREST, MonitorFromWindow};
        use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
        use windows_sys::Win32::UI::WindowsAndMessaging::{GetCursorPos, GetWindowRect};
        let Ok(handle) = window.window_handle() else { return };
        let RawWindowHandle::Win32(h) = handle.as_raw() else { return };
        let hwnd = h.hwnd.get() as HWND;
        let scale = window.scale_factor();
        let inner = window.inner_size();
        // SAFETY: plain queries on this app's own window.
        unsafe {
            let mut r: RECT = std::mem::zeroed();
            let actual = if GetWindowRect(hwnd, &mut r) != 0 { sharex_capture::Rect::new(r.left, r.top, r.right - r.left, r.bottom - r.top) } else { sharex_capture::Rect::default() };
            let mut origin = POINT { x: 0, y: 0 };
            let client_origin = if ClientToScreen(hwnd, &mut origin) != 0 { (origin.x, origin.y) } else { (0, 0) };
            let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
            let mut info: windows_sys::Win32::Graphics::Gdi::MONITORINFOEXW = std::mem::zeroed();
            info.monitorInfo.cbSize = std::mem::size_of::<windows_sys::Win32::Graphics::Gdi::MONITORINFOEXW>() as u32;
            let monitor_device_name = if windows_sys::Win32::Graphics::Gdi::GetMonitorInfoW(monitor, (&mut info as *mut windows_sys::Win32::Graphics::Gdi::MONITORINFOEXW).cast()) != 0 {
                let len = info.szDevice.iter().position(|c| *c == 0).unwrap_or(info.szDevice.len());
                String::from_utf16_lossy(&info.szDevice[..len])
            } else {
                String::new()
            };
            let mut p = POINT { x: 0, y: 0 };
            let cursor = (GetCursorPos(&mut p) != 0).then_some((p.x, p.y));
            push(Entry {
                time: chrono::Local::now().format("%H:%M:%S").to_string(),
                source: source.to_owned(),
                expected,
                actual,
                client_origin,
                image_size,
                window_dpi: GetDpiForWindow(hwnd),
                render_scaling: scale,
                client_size: (inner.width as f64 / scale, inner.height as f64 / scale),
                monitor_device_name,
                cursor_screen: cursor,
                // winit maps the pointer straight from Windows' client pixels.
                cursor_client_pixels: cursor.map(|(x, y)| ((x - client_origin.0) as f64, (y - client_origin.1) as f64)),
            });
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (window, source, expected, image_size);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_log_keeps_the_last_ten() {
        for i in 0..12 {
            push(Entry { source: format!("w{i}"), ..Default::default() });
        }
        let e = entries();
        assert!(e.len() <= MAXIMUM_ENTRIES);
        assert_eq!(e.last().map(|x| x.source.as_str()), Some("w11"));
    }
}
