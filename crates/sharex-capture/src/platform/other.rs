//! macOS and Linux: the screen through xcap, as snapr grabs it (monitor by
//! monitor, placed in one desktop of pixels by `screenshot::untangle`).
//! On Linux the monitors come from winit and Wayland captures go through
//! `linux.rs`. Every xcap call is [`guarded`]: a capture library's panic
//! is a failed capture, never a crash.
//! Not testable on Windows; see `docs/PORTING.md`.

use std::sync::Mutex;

use image::RgbaImage;

use crate::geometry::Rect;
use crate::screenshot::{crop_placed, untangle};
use crate::{CursorImage, Screen, SimpleWindowInfo, WindowInfo};

/// Runs a capture library's call, turning a panic into `None` and a line
/// in the log (libwayshot, under xcap on Wayland, panics where it should
/// return an error). The release profile unwinds, so this holds there too.
pub fn guarded<T>(what: &str, f: impl FnOnce() -> T) -> Option<T> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        Ok(v) => Some(v),
        Err(e) => {
            let message = e
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| e.downcast_ref::<&str>().copied())
                .unwrap_or("unknown panic");
            eprintln!("Screen capture: {what} failed ({message}).");
            None
        }
    }
}

/// Each monitor with its scale, placed as `capture_monitors` places shots.
fn monitors() -> Vec<(xcap::Monitor, Rect, f64, bool)> {
    guarded("listing the monitors", monitors_unguarded).unwrap_or_default()
}

fn monitors_unguarded() -> Vec<(xcap::Monitor, Rect, f64, bool)> {
    // Each xcap coordinate on Wayland asks libwayshot for the outputs'
    // scale, which panics without xdg-output version 3.
    #[cfg(target_os = "linux")]
    if super::linux::wayland() && !super::linux::compositor().xdg_output_v3 {
        return Vec::new();
    }
    let Ok(all) = xcap::Monitor::all() else { return Vec::new() };
    let placed: Vec<(xcap::Monitor, Rect, f64, bool)> = all
        .into_iter()
        .filter_map(|m| {
            // macOS gives monitor frames in points.
            let scale = if cfg!(target_os = "macos") { m.scale_factor().ok()? as f64 } else { 1.0 };
            let s = scale as f32;
            let rect = Rect::new(
                (m.x().ok()? as f32 * s).round() as i32,
                (m.y().ok()? as f32 * s).round() as i32,
                (m.width().ok()? as f32 * s).round() as i32,
                (m.height().ok()? as f32 * s).round() as i32,
            );
            let primary = m.is_primary().unwrap_or(false);
            Some((m, rect, scale, primary))
        })
        .collect();
    let rects: Vec<(Rect, f64)> = placed.iter().map(|(_, r, s, _)| (*r, *s)).collect();
    placed
        .into_iter()
        .zip(untangle(&rects))
        .map(|((m, _, s, p), r)| (m, r, s, p))
        .collect()
}

/// The monitors: on Linux as winit reports them (once the event loop
/// runs), so windows never wait on a capture library; else xcap's.
pub fn screens() -> Vec<Screen> {
    #[cfg(target_os = "linux")]
    {
        let screens = super::linux::remembered_screens();
        if !screens.is_empty() {
            return screens;
        }
    }
    monitors()
        .into_iter()
        .map(|(_, bounds, scale, primary)| Screen { bounds, work_area: bounds, scale, primary })
        .collect()
}

/// Where the pointer is isn't cheap to ask outside Windows: the overlay
/// learns it from the first move.
pub fn cursor_position() -> Option<(i32, i32)> {
    None
}

pub fn set_cursor_position(_x: i32, _y: i32) {}

/// The pointer isn't drawn into screenshots outside Windows yet.
pub fn cursor_image(_background: &RgbaImage, _origin: (i32, i32)) -> Option<CursorImage> {
    None
}

/// Every monitor's picture, stitched for `r`.
pub fn capture_rectangle(r: Rect) -> Option<RgbaImage> {
    #[cfg(target_os = "macos")]
    if !super::macos::screen_recording_allowed() {
        return None;
    }
    // Wayland: the whole desktop at once, its top left at the monitors'.
    #[cfg(target_os = "linux")]
    if super::linux::wayland() {
        let desktop = match super::linux::capture_desktop() {
            Ok(image) => image,
            Err(e) => {
                eprintln!("Screen capture: {e}");
                return None;
            }
        };
        let origin = crate::geometry::combine(screens().iter().map(|s| s.bounds));
        let placed = Rect::new(origin.x, origin.y, desktop.width() as i32, desktop.height() as i32);
        return crop_placed(&[(desktop, placed)], r);
    }
    let shots: Vec<(RgbaImage, Rect)> = monitors()
        .into_iter()
        .filter(|(_, rect, ..)| !rect.intersect(&r).is_empty())
        .filter_map(|(m, rect, ..)| Some((guarded("capturing a monitor", || m.capture_image().ok()).flatten()?, rect)))
        .collect();
    crop_placed(&shots, r)
}

/// The screen's pixels in `r` (the screen colour picker's magnifier). Not
/// through a screenshot portal, which may flash the screen or ask.
pub fn sample_screen(r: Rect) -> Option<RgbaImage> {
    #[cfg(target_os = "linux")]
    if !super::linux::quiet_grab() {
        return None;
    }
    capture_rectangle(r)
}

/// Something worked out from xcap's windows (XWayland's only, on
/// Wayland), guarded; `T::default()` if it fails.
fn with_xcap_windows<T: Default>(f: impl FnOnce(Vec<xcap::Window>) -> T) -> T {
    guarded("listing the windows", || f(xcap::Window::all().unwrap_or_default())).unwrap_or_default()
}

/// Names of the windows listed last, by handle.
static NAMES: Mutex<Vec<WindowInfo>> = Mutex::new(Vec::new());

/// The titled windows on screen, topmost first, in desktop pixels.
fn listed_windows() -> Vec<(WindowInfo, Rect)> {
    #[cfg(target_os = "macos")]
    let listed = super::macos::window_rects();
    #[cfg(not(target_os = "macos"))]
    let listed: Vec<(WindowInfo, Rect)> = with_xcap_windows(|windows| {
        windows
            .into_iter()
            .filter(|w| !w.is_minimized().unwrap_or(true))
            .filter_map(|w| {
                let rect = Rect::new(w.x().ok()?, w.y().ok()?, w.width().ok()? as i32, w.height().ok()? as i32);
                let info = WindowInfo {
                    handle: w.id().ok()? as isize,
                    title: w.title().unwrap_or_default(),
                    process_name: w.app_name().ok(),
                };
                Some((info, rect))
            })
            .collect()
    });
    listed
}

pub fn window_list(_include_children: bool, ignore: &[isize]) -> Vec<SimpleWindowInfo> {
    let listed = listed_windows();
    let mut names = NAMES.lock().unwrap_or_else(|e| e.into_inner());
    names.clear();
    let mut out = Vec::new();
    for (info, rect) in listed {
        if ignore.contains(&info.handle) || !rect.is_valid() {
            continue;
        }
        out.push(SimpleWindowInfo { handle: info.handle, rect, is_window: true });
        names.push(info);
    }
    out
}

pub fn window_info(handle: isize) -> WindowInfo {
    let names = NAMES.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(info) = names.iter().find(|n| n.handle == handle) {
        return info.clone();
    }
    drop(names);
    listed_windows()
        .into_iter()
        .map(|(i, _)| i)
        .find(|i| i.handle == handle)
        .unwrap_or(WindowInfo { handle, ..Default::default() })
}

pub fn window_handle(_window: &winit::window::Window) -> Option<isize> {
    None
}

/// macOS: above the menu bar and Dock, on every Space, faded in (see
/// `docs/MACOS-LEARNINGS.md` 3.4 and 3.7).
pub fn prepare_overlay(_window: &winit::window::Window) {
    #[cfg(target_os = "macos")]
    super::macos::cover_everything(_window);
}

/// Keeps the window out of screen captures: `setSharingType(None)` on
/// macOS (what snapr does for its recording frame); nothing elsewhere.
pub fn exclude_from_capture(_window: &winit::window::Window) -> bool {
    #[cfg(target_os = "macos")]
    return super::macos::exclude_from_capture(_window);
    #[cfg(not(target_os = "macos"))]
    false
}

/// The taskbar's light theme (Windows only).
pub fn light_taskbar() -> bool {
    false
}

/// A window region (Windows only; elsewhere the window's transparent
/// pixels are clicked through by the window manager or not at all).
pub fn set_window_region(_window: &winit::window::Window, _rects: &[Rect]) {}

/// Called once an overlay's first frame is on screen: macOS fades it in
/// (`docs/MACOS-LEARNINGS.md` 3.7).
pub fn overlay_presented(_window: &winit::window::Window) {
    #[cfg(target_os = "macos")]
    super::macos::fade_in(_window);
}

/// Puts a kept overlay back on top before it's shown again (Windows only;
/// overlays aren't kept elsewhere).
pub fn raise_overlay(_window: &winit::window::Window) {}

// ---- windows for Capture › Window and the other capture jobs -------------

/// The focused window, where the platform tells (xcap's `is_focused`).
pub fn foreground_window() -> isize {
    with_xcap_windows(|windows| {
        windows
            .into_iter()
            .find(|w| w.is_focused().unwrap_or(false))
            .and_then(|w| w.id().ok())
            .map_or(0, |id| id as isize)
    })
}

pub fn window_rect(handle: isize) -> Rect {
    listed_windows().into_iter().find(|(i, _)| i.handle == handle).map(|(_, r)| r).unwrap_or_default()
}

/// No client area apart from the frame outside Windows.
pub fn client_rect(handle: isize) -> Rect {
    window_rect(handle)
}

/// Listed windows are the visible ones.
pub fn is_window_visible(handle: isize) -> bool {
    listed_windows().iter().any(|(i, _)| i.handle == handle)
}

pub fn is_minimized(handle: isize) -> bool {
    with_xcap_windows(|windows| {
        windows
            .into_iter()
            .find(|w| w.id().ok().map(|id| id as isize) == Some(handle))
            .is_some_and(|w| w.is_minimized().unwrap_or(false))
    })
}

/// Restoring and activating another app's window isn't ported outside
/// Windows.
pub fn restore_window(_handle: isize) {}

pub fn activate_window(_handle: isize) {}

/// Sending input to other apps (scrolling capture) isn't ported outside
/// Windows yet.
pub fn send_key_press(_vk: u16) {}

pub fn send_mouse_wheel(_delta: i32) {}

pub fn send_vscroll(_handle: isize, _code: usize) {}

/// `WindowsList.GetVisibleWindowsList`.
pub fn visible_windows() -> Vec<crate::VisibleWindow> {
    listed_windows()
        .into_iter()
        .filter(|(i, r)| !i.title.is_empty() && r.w > 0 && r.h > 0)
        .map(|(i, rect)| crate::VisibleWindow { handle: i.handle, title: i.title, rect })
        .collect()
}

/// Windows' icons aren't read outside Windows.
pub fn window_icon(_handle: isize) -> Option<RgbaImage> {
    None
}

/// `NativeMethods.SearchWindow`: the window titled exactly so, else one
/// whose title contains it.
pub fn search_window(title: &str) -> isize {
    let windows = visible_windows();
    let needle = title.to_lowercase();
    windows
        .iter()
        .find(|w| w.title == title)
        .or_else(|| windows.iter().find(|w| w.title.to_lowercase().contains(&needle)))
        .map_or(0, |w| w.handle)
}

/// ShareX locks the pointer to the overlay on Windows only
/// (`Helpers.LockCursorToWindow`).
pub fn clip_cursor_to_window(_handle: Option<isize>) {}

pub fn is_zoomed(_handle: isize) -> bool {
    false
}

pub fn is_windows_11() -> bool {
    false
}

/// Transparent window capture's helper window is Win32's (ShareX's
/// `WindowsNativeWindow`); elsewhere window captures stay opaque.
pub struct BackgroundWindow;

impl BackgroundWindow {
    pub fn new(_bounds: Rect) -> Option<Self> {
        None
    }

    pub fn set_color(&mut self, _rgb: [u8; 3]) {}

    pub fn show_behind(&self, _target: isize) -> bool {
        false
    }

    pub fn flush() {}
}

/// The taskbar and desktop icons are Windows'; elsewhere nothing changes.
pub fn set_taskbar_visibility_if_intersect(_visible: bool, _rect: Rect) -> bool {
    false
}

pub fn set_taskbar_visibility(_visible: bool) -> bool {
    false
}

pub fn is_taskbar_visible() -> Option<bool> {
    None
}

pub fn are_desktop_icons_visible() -> bool {
    false
}

pub fn set_desktop_icons_visibility(_show: bool) -> bool {
    false
}

pub fn is_active_window_fullscreen(_screens: &[Screen]) -> bool {
    false
}

/// The pointer isn't drawn into screenshots outside Windows yet.
pub fn draw_cursor_gdi(_image: &mut RgbaImage, _origin: (i32, i32)) -> bool {
    false
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_capture_library_panic_is_a_failure() {
        assert_eq!(super::guarded("test", || 1), Some(1));
        assert_eq!(super::guarded("test", || -> i32 { panic!("UnsupportedVersion") }), None);
    }
}
