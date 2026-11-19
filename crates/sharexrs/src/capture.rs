//! The capture jobs that don't need the region overlay, as ShareX runs
//! them: `ShareX/CaptureHelpers/CaptureFullscreen.cs`, `CaptureWindow.cs`,
//! `CaptureMonitor.cs`, `CaptureActiveWindow.cs`, `CaptureActiveMonitor.cs`,
//! `CaptureCustomRegion.cs`, `CaptureCustomWindow.cs`, `CaptureLastRegion.cs`,
//! with `CaptureBase.CreateMetadata` and `TaskHelpers.GetScreenshot`. The
//! delay, hiding the main window and the after capture tasks around them
//! (`CaptureBase.CaptureAsync`) are the app's (`main.rs`).

use std::time::Duration;

use image::RgbaImage;
use sharex_capture::screenshot::{self, Screenshot};
use sharex_capture::{Rect, platform};
use sharex_settings::{HotkeyType, TaskSettings};

use crate::capture_job::Metadata;

/// Which `CaptureBase` subclass runs.
#[derive(Clone, Debug, PartialEq)]
pub enum CaptureJob {
    /// `CaptureFullscreen`.
    Fullscreen,
    /// `CaptureActiveWindow`.
    ActiveWindow,
    /// `CaptureActiveMonitor`.
    ActiveMonitor,
    /// `CaptureWindow(handle)`, from Capture › Window.
    Window(isize),
    /// `CaptureMonitor(bounds)`, from Capture › Monitor.
    Monitor(Rect),
    /// `CaptureRegion`: the region overlay over a frozen screen.
    Region,
    /// `CaptureRegionTransparent`: a region drawn over the live desktop.
    RegionTransparent,
    /// `CaptureLastRegion`.
    LastRegion,
    /// `CaptureCustomRegion`.
    CustomRegion,
    /// `CaptureCustomWindow`.
    CustomWindow,
}

impl CaptureJob {
    /// The capture a hotkey type runs (`TaskHelpers.ExecuteJob`).
    pub fn from_hotkey(job: &HotkeyType) -> Option<Self> {
        Some(match job {
            HotkeyType::PrintScreen => Self::Fullscreen,
            HotkeyType::ActiveWindow => Self::ActiveWindow,
            HotkeyType::ActiveMonitor => Self::ActiveMonitor,
            HotkeyType::RectangleRegion => Self::Region,
            HotkeyType::RectangleTransparent => Self::RegionTransparent,
            HotkeyType::CustomRegion => Self::CustomRegion,
            HotkeyType::CustomWindow => Self::CustomWindow,
            HotkeyType::LastRegion => Self::LastRegion,
            _ => return None,
        })
    }

    /// `AllowAutoHideForm`: Capture › Window doesn't hide the main window
    /// when that's the window it captures.
    pub fn allow_auto_hide(&self, main_window: Option<isize>) -> bool {
        match self {
            Self::Window(handle) => main_window.is_none_or(|m| m == 0 || *handle != m),
            _ => true,
        }
    }
}

/// Why a capture produced nothing.
#[derive(Debug)]
pub enum CaptureError {
    /// `CaptureCustomWindow`: no window has the title asked for (ShareX
    /// says so in a message box).
    WindowNotFound,
    Failed(String),
}

/// `TaskHelpers.GetScreenshot(taskSettings)`: the pointer drawn in
/// (`ShowCursor`), a window's client area only, clipped to the monitors,
/// the shadow and its offset for transparent window captures, hiding the
/// taskbar over a window, HDR correction (`HDRScreenshotColorCorrection`).
pub(crate) fn task_screenshot(task: &TaskSettings) -> Screenshot {
    screenshot(task)
}

fn screenshot(task: &TaskSettings) -> Screenshot {
    let capture = task.CaptureSettings.clone().unwrap_or_default();
    Screenshot {
        capture_cursor: capture.ShowCursor,
        capture_client_area: capture.CaptureClientArea,
        remove_outside_screen_area: true,
        capture_shadow: capture.CaptureShadow,
        shadow_offset: capture.CaptureShadowOffset,
        auto_hide_taskbar: capture.CaptureAutoHideTaskbar,
        hdr_color_correction: capture.HDRScreenshotColorCorrection,
    }
}

/// `CaptureTransparent && !CaptureClientArea`: window captures keep their
/// transparency (`CaptureWindowTransparent`).
fn transparent_window(task: &TaskSettings) -> bool {
    task.CaptureSettings.as_ref().is_some_and(|c| c.CaptureTransparent && !c.CaptureClientArea)
}

/// `CaptureBase.CreateMetadata(insideRect, ignoreProcess)`: the foreground
/// window names the file when it isn't `ignore_process` and holds all of
/// `inside` (always, with no `inside`).
pub fn create_metadata(image: RgbaImage, inside: Option<Rect>, ignore_process: Option<&str>) -> Metadata {
    let handle = platform::foreground_window();
    let info = platform::window_info(handle);
    let ignored = ignore_process.is_some_and(|p| info.process_name.as_deref().is_some_and(|n| n.eq_ignore_ascii_case(p)));
    let inside_ok = inside.is_none_or(|r| r.is_empty() || platform::window_rect(handle).contains_rect(&r));
    let (window_title, process_name) =
        if handle != 0 && !ignored && inside_ok { (Some(info.title), info.process_name) } else { (None, None) };
    Metadata { image, window_title, process_name }
}

/// `CreateMetadata(insideRect)`, which ignores Explorer (the desktop and
/// taskbar).
fn metadata_inside(image: RgbaImage, inside: Rect) -> Metadata {
    create_metadata(image, Some(inside), Some("explorer"))
}

/// `CaptureHelpers.GetScreenWorkingArea`: every monitor's work area
/// together.
fn screen_working_area(screens: &[sharex_capture::Screen]) -> Rect {
    sharex_capture::geometry::combine(screens.iter().map(|s| s.work_area))
}

/// `CaptureWindowTransparent(handle)` with `CaptureTransparent` (and not
/// the client area), else `CaptureWindow(handle)`: the window's frame
/// (DWM's, without the invisible resize borders) or its client area.
fn capture_window(task: &TaskSettings, screens: &[sharex_capture::Screen], handle: isize) -> Option<RgbaImage> {
    let shot = screenshot(task);
    if transparent_window(task) { shot.capture_window_transparent(screens, handle) } else { shot.capture_window(screens, handle) }
}

/// `CaptureRegion.CaptureWindowTransparentAsync`: a detected window
/// captured whole by the region overlay, with `CaptureTransparent`, gets
/// its rounded corners and (with `CaptureShadow`) shadow from the live
/// screen 300 ms after the overlay closed; the pixels inside stay the
/// region capture's (annotations included). `None` keeps the region
/// capture as it is.
pub fn region_window_transparent(task: &TaskSettings, window_handle: isize, image: &RgbaImage, rect: Rect) -> Option<RgbaImage> {
    if window_handle == 0 || !task.CaptureSettings.as_ref().is_some_and(|c| c.CaptureTransparent) {
        return None;
    }
    std::thread::sleep(Duration::from_millis(300));
    let mut shot = screenshot(task);
    shot.capture_cursor = false;
    shot.capture_window_transparent_with(&screenshot::screens(), window_handle, image, rect)
}

/// `RegionCaptureTasks.ApplyRegionMask`: only the pixels inside `regions`
/// (relative to the image) are kept, the rest made transparent.
pub fn apply_region_mask(image: &RgbaImage, regions: &[Rect]) -> RgbaImage {
    let mut out = RgbaImage::new(image.width(), image.height());
    for (x, y, p) in image.enumerate_pixels() {
        if regions.iter().any(|r| r.contains(x as i32, y as i32)) {
            out.put_pixel(x, y, *p);
        }
    }
    out
}

/// `Execute` of the captures that need no overlay. `last_region` is
/// `RegionCaptureIntegration.LastRegionRectangle(s)`: Capture › Last region
/// with none goes through the region overlay instead (not here).
pub fn execute(
    job: &CaptureJob,
    task: &TaskSettings,
    last_regions: &[Rect],
) -> Result<Option<Metadata>, CaptureError> {
    let screens = screenshot::screens();
    if screens.is_empty() {
        return Err(CaptureError::Failed("no monitors found".into()));
    }
    let shot = screenshot(task);
    let capture = task.CaptureSettings.clone().unwrap_or_default();
    let image = |i: Option<RgbaImage>| i.ok_or_else(|| CaptureError::Failed("couldn't capture the screen".into()));
    let metadata = match job {
        CaptureJob::Fullscreen => {
            let inside = screen_working_area(&screens);
            metadata_inside(image(shot.capture_fullscreen(&screens))?, inside)
        }
        CaptureJob::ActiveMonitor => {
            let cursor = screenshot::cursor_position().unwrap_or((screens[0].bounds.x, screens[0].bounds.y));
            let inside = screenshot::screen_from_point(&screens, cursor).map(|s| s.work_area).unwrap_or_default();
            metadata_inside(image(shot.capture_active_monitor(&screens, cursor))?, inside)
        }
        CaptureJob::Monitor(bounds) => metadata_inside(image(shot.capture_rectangle(&screens, *bounds))?, *bounds),
        CaptureJob::CustomRegion => {
            let r = capture.CaptureCustomRegion;
            let rect = Rect::new(r.x, r.y, r.width, r.height);
            metadata_inside(image(shot.capture_rectangle(&screens, rect))?, rect)
        }
        CaptureJob::ActiveWindow => {
            let handle = platform::foreground_window();
            create_metadata(image(capture_window(task, &screens, handle))?, None, None)
        }
        CaptureJob::Window(handle) => window(task, &screens, *handle)?,
        CaptureJob::CustomWindow => {
            let title = capture.CaptureCustomWindow.clone();
            if title.is_empty() {
                return Ok(None);
            }
            match platform::search_window(&title) {
                0 => return Err(CaptureError::WindowNotFound),
                handle => window(task, &screens, handle)?,
            }
        }
        CaptureJob::LastRegion => {
            let Some(bounds) = last_regions.iter().copied().reduce(|a, b| a.union(&b)) else { return Ok(None) };
            let Some(mut bmp) = shot.capture_rectangle(&screens, bounds) else { return Ok(None) };
            if last_regions.len() > 1 {
                let local: Vec<Rect> = last_regions.iter().map(|r| r.offset(-bounds.x, -bounds.y)).collect();
                bmp = apply_region_mask(&bmp, &local);
            }
            Metadata { image: bmp, window_title: None, process_name: None }
        }
        CaptureJob::Region | CaptureJob::RegionTransparent => return Ok(None),
    };
    Ok(Some(metadata))
}

/// `CaptureWindow.Execute`: a minimized window is restored (250 ms to
/// settle) and an inactive one activated (100 ms) before it's captured.
fn window(task: &TaskSettings, screens: &[sharex_capture::Screen], handle: isize) -> Result<Metadata, CaptureError> {
    if platform::is_minimized(handle) {
        platform::restore_window(handle);
        std::thread::sleep(Duration::from_millis(250));
    }
    if platform::foreground_window() != handle {
        platform::activate_window(handle);
        std::thread::sleep(Duration::from_millis(100));
    }
    let info = platform::window_info(handle);
    let image = capture_window(task, screens, handle)
        .ok_or_else(|| CaptureError::Failed("couldn't capture the window".into()))?;
    Ok(Metadata { image, window_title: Some(info.title), process_name: info.process_name })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hotkeys_map_to_their_captures() {
        assert_eq!(CaptureJob::from_hotkey(&HotkeyType::PrintScreen), Some(CaptureJob::Fullscreen));
        assert_eq!(CaptureJob::from_hotkey(&HotkeyType::LastRegion), Some(CaptureJob::LastRegion));
        assert_eq!(CaptureJob::from_hotkey(&HotkeyType::ScreenRecorder), None);
    }

    #[test]
    fn capturing_the_main_window_keeps_it_shown() {
        assert!(!CaptureJob::Window(42).allow_auto_hide(Some(42)));
        assert!(CaptureJob::Window(7).allow_auto_hide(Some(42)));
        assert!(CaptureJob::Fullscreen.allow_auto_hide(Some(42)));
    }

    #[test]
    fn region_masks_keep_only_the_regions() {
        let image = RgbaImage::from_pixel(4, 2, image::Rgba([9, 9, 9, 255]));
        let masked = apply_region_mask(&image, &[Rect::new(0, 0, 1, 2), Rect::new(3, 1, 1, 1)]);
        assert_eq!(masked.get_pixel(0, 1)[3], 255);
        assert_eq!(masked.get_pixel(1, 0)[3], 0);
        assert_eq!(masked.get_pixel(3, 1)[3], 255);
        assert_eq!(masked.get_pixel(3, 0)[3], 0);
    }
}
