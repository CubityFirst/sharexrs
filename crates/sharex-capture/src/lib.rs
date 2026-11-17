//! `ShareX.ScreenCaptureLib`, ported.
//!
//! - [`screenshot`]: `Screenshot.cs` and `CaptureHelpers`: the monitors,
//!   grabbing the screen and the mouse pointer.
//! - [`windows_list`]: `WindowsRectangleList`: windows and controls to snap
//!   a region to.
//! - [`options`]: `RegionCaptureOptions`.
//! - [`region`]: the region capture overlay (`Presentation/RegionCapture`):
//!   its state, input handling and drawing, independent of any window so
//!   it can be rendered offscreen and tested.
//! - [`clipboard`]: `ClipboardHelpers.CopyImage`.
//! - [`platform`]: what each OS does differently (Win32, AppKit, xcap).

pub mod clipboard;
pub mod geometry;
pub mod options;
pub mod platform;
pub mod region;
pub mod screenshot;
pub mod windows_list;

pub use geometry::{PointF, Rect, RectF};
pub use options::RegionCaptureOptions;

/// A monitor (`DesktopScreen` / Avalonia's `Screen`), in desktop pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Screen {
    pub bounds: Rect,
    pub work_area: Rect,
    /// Its scaling (1.0 = 96 DPI).
    pub scale: f64,
    pub primary: bool,
}

/// `SimpleWindowInfo`: a window, or a control or client area in one, to
/// snap the region to.
#[derive(Clone, Debug, PartialEq)]
pub struct SimpleWindowInfo {
    /// The platform's handle (`HWND`, macOS window number).
    pub handle: isize,
    /// Desktop pixels.
    pub rect: Rect,
    /// A top-level window, not a control or client area.
    pub is_window: bool,
}

/// `WindowInfo`, for naming the file (`%t`, `%pn`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WindowInfo {
    pub handle: isize,
    pub title: String,
    pub process_name: Option<String>,
}

/// The mouse pointer, ready to draw into a screenshot: its picture
/// (straight alpha) and where its top-left corner goes, in the
/// screenshot's pixels.
#[derive(Clone, Debug)]
pub struct CursorImage {
    pub image: image::RgbaImage,
    pub position: (i32, i32),
}
