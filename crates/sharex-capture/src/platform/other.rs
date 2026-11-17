//! macOS and Linux: the screen through xcap, as snapr grabs it (monitor by
//! monitor, placed in one desktop of pixels by `screenshot::untangle`).
//! Not testable on Windows; see `docs/PORTING.md`.

use std::sync::Mutex;

use image::RgbaImage;

use crate::geometry::Rect;
use crate::screenshot::{crop_placed, untangle};
use crate::{CursorImage, Screen, SimpleWindowInfo, WindowInfo};

/// Each monitor with its scale, placed as `capture_monitors` places shots.
fn monitors() -> Vec<(xcap::Monitor, Rect, f64, bool)> {
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

pub fn screens() -> Vec<Screen> {
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
    let shots: Vec<(RgbaImage, Rect)> = monitors()
        .into_iter()
        .filter(|(_, rect, ..)| !rect.intersect(&r).is_empty())
        .filter_map(|(m, rect, ..)| Some((m.capture_image().ok()?, rect)))
        .collect();
    crop_placed(&shots, r)
}

/// Names of the windows listed last, by handle.
static NAMES: Mutex<Vec<WindowInfo>> = Mutex::new(Vec::new());

pub fn window_list(_include_children: bool, ignore: &[isize]) -> Vec<SimpleWindowInfo> {
    #[cfg(target_os = "macos")]
    let listed = super::macos::window_rects();
    #[cfg(not(target_os = "macos"))]
    let listed: Vec<(WindowInfo, Rect)> = xcap::Window::all()
        .unwrap_or_default()
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
        .collect();
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
    names.iter().find(|n| n.handle == handle).cloned().unwrap_or(WindowInfo { handle, ..Default::default() })
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

/// Called once an overlay's first frame is on screen: macOS fades it in
/// (`docs/MACOS-LEARNINGS.md` 3.7).
pub fn overlay_presented(_window: &winit::window::Window) {
    #[cfg(target_os = "macos")]
    super::macos::fade_in(_window);
}
