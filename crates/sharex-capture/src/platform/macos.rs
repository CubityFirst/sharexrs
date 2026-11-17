//! macOS: the Screen Recording permission, the window list and the
//! overlay's window level and fade, from snapr's `macos.rs` (see
//! `docs/MACOS-LEARNINGS.md` 3.1, 3.4, 3.7, 3.8, 3.9). Written on Windows
//! and not run yet.

use std::ffi::c_void;

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_app_kit::{
    NSAnimatablePropertyContainer, NSAnimationContext, NSView, NSWindow, NSWindowAnimationBehavior,
    NSWindowCollectionBehavior,
};
use objc2_foundation::{NSArray, NSDictionary, NSNumber, NSString};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

use crate::geometry::Rect;
use crate::screenshot::untangle;
use crate::WindowInfo;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGPreflightScreenCaptureAccess() -> bool;
    fn CGRequestScreenCaptureAccess() -> bool;
    fn CGWindowListCopyWindowInfo(option: u32, relative_to_window: u32) -> *mut c_void;
}

/// Without the permission macOS hands back blank pictures, not errors.
pub fn screen_recording_allowed() -> bool {
    // SAFETY: no arguments; safe from any thread.
    unsafe { CGPreflightScreenCaptureAccess() }
}

/// Has macOS ask for Screen Recording once (it only asks the first time).
pub fn ask_for_screen_recording() {
    if !screen_recording_allowed() {
        // SAFETY: as above.
        unsafe { CGRequestScreenCaptureAccess() };
    }
}

/// The titled, ordinary windows on screen (layer 0, not transparent),
/// topmost first, from one copy of the window list, in desktop pixels:
/// frames are in points, scaled by the monitor under the window's centre
/// (else one it overlaps, else the main one) and moved as `untangle`
/// moved that monitor.
pub fn window_rects() -> Vec<(WindowInfo, Rect)> {
    const ON_SCREEN_ONLY: u32 = 1 << 0;
    const EXCLUDE_DESKTOP_ELEMENTS: u32 = 1 << 4;
    // SAFETY: a Copy function, so the CFArray (an NSArray of NSDictionary,
    // toll-free bridged) is ours to release.
    let windows = unsafe {
        Retained::from_raw(
            CGWindowListCopyWindowInfo(ON_SCREEN_ONLY | EXCLUDE_DESKTOP_ELEMENTS, 0)
                .cast::<NSArray<NSDictionary<NSString, AnyObject>>>(),
        )
    };
    let Some(windows) = windows else { return Vec::new() };
    let monitors: Vec<([f64; 4], f32, bool)> = xcap::Monitor::all()
        .unwrap_or_default()
        .iter()
        .filter_map(|m| {
            let frame = [m.x().ok()? as f64, m.y().ok()? as f64, m.width().ok()? as f64, m.height().ok()? as f64];
            Some((frame, m.scale_factor().ok()?, m.is_primary().unwrap_or(false)))
        })
        .collect();
    let placed: Vec<(Rect, f64)> = monitors
        .iter()
        .map(|&([x, y, w, h], s, _)| {
            let px = |v: f64| (v as f32 * s).round() as i32;
            (Rect::new(px(x), px(y), px(w), px(h)), s as f64)
        })
        .collect();
    let moved: Vec<(i32, i32)> =
        placed.iter().zip(untangle(&placed)).map(|((from, _), to)| (to.x - from.x, to.y - from.y)).collect();
    let contains = |[x, y, w, h]: [f64; 4], (px, py): (f64, f64)| px >= x && px < x + w && py >= y && py < y + h;
    let overlaps =
        |[x, y, w, h]: [f64; 4], [ox, oy, ow, oh]: [f64; 4]| x < ox + ow && ox < x + w && y < oy + oh && oy < y + h;
    let get = |dict: &NSDictionary<NSString, AnyObject>, key: &str| dict.objectForKey(&NSString::from_str(key));
    let number =
        |dict: &NSDictionary<NSString, AnyObject>, key: &str| get(dict, key).and_then(|v| v.downcast::<NSNumber>().ok());
    let string = |dict: &NSDictionary<NSString, AnyObject>, key: &str| {
        get(dict, key).and_then(|v| v.downcast::<NSString>().ok()).map(|s| s.to_string())
    };
    windows
        .iter()
        .filter(|w| number(w, "kCGWindowLayer").is_some_and(|l| l.intValue() == 0))
        .filter(|w| number(w, "kCGWindowAlpha").is_none_or(|a| a.doubleValue() > 0.0))
        .filter_map(|w| {
            let title = string(&w, "kCGWindowName").filter(|t| !t.trim().is_empty())?;
            let bounds = get(&w, "kCGWindowBounds")?.downcast::<NSDictionary>().ok()?;
            // SAFETY: kCGWindowBounds maps strings to numbers.
            let bounds: &NSDictionary<NSString, AnyObject> = unsafe { &*(&*bounds as *const NSDictionary).cast() };
            let f = ["X", "Y", "Width", "Height"].map(|k| number(bounds, k).map(|n| n.doubleValue()));
            let frame = [f[0]?, f[1]?, f[2]?, f[3]?];
            let centre = (frame[0] + frame[2] / 2.0, frame[1] + frame[3] / 2.0);
            let i = monitors
                .iter()
                .position(|(m, ..)| contains(*m, centre) || overlaps(*m, frame))
                .or_else(|| monitors.iter().position(|(.., primary)| *primary))?;
            let (s, (dx, dy)) = (monitors[i].1, moved[i]);
            let rect = Rect::new(
                (frame[0] as i32 as f32 * s).round() as i32 + dx,
                (frame[1] as i32 as f32 * s).round() as i32 + dy,
                (frame[2] as u32 as f32 * s).round() as i32,
                (frame[3] as u32 as f32 * s).round() as i32,
            );
            let info = WindowInfo {
                handle: number(&w, "kCGWindowNumber")?.unsignedIntValue() as isize,
                title,
                process_name: string(&w, "kCGWindowOwnerName"),
            };
            Some((info, rect))
        })
        .collect()
}

/// The AppKit window behind one of winit's.
fn ns_window(window: &Window) -> Option<Retained<NSWindow>> {
    let handle = window.window_handle().ok()?;
    let RawWindowHandle::AppKit(h) = handle.as_raw() else { return None };
    // SAFETY: winit's view, made on this (the main) thread, alive while
    // `window` is.
    let view: &NSView = unsafe { h.ns_view.cast().as_ref() };
    view.window()
}

/// Above the menu bar and Dock (NSScreenSaverWindowLevel), on every Space,
/// with no zoom animation and almost transparent until [`fade_in`].
pub fn cover_everything(window: &Window) {
    const LEVEL: isize = 1000;
    if let Some(ns_window) = ns_window(window) {
        ns_window.setLevel(LEVEL);
        ns_window.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::FullScreenAuxiliary
                | NSWindowCollectionBehavior::Stationary,
        );
        ns_window.setAnimationBehavior(NSWindowAnimationBehavior::None);
        // Not 0: macOS doesn't count a fully transparent window as on
        // screen, and the first frame would wait for it.
        ns_window.setAlphaValue(0.01);
        ns_window.setIgnoresMouseEvents(false);
    }
}

/// Fades an overlay in once its first frame is presented.
pub fn fade_in(window: &Window) {
    if let Some(ns_window) = ns_window(window) {
        NSAnimationContext::beginGrouping();
        NSAnimationContext::currentContext().setDuration(0.15);
        ns_window.animator().setAlphaValue(1.0);
        NSAnimationContext::endGrouping();
    }
}
