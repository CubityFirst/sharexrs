//! `ShareX.ScreenCaptureLib/Screenshot.cs` and the screen helpers of
//! `ShareX.HelpersLib/Helpers/CaptureHelpers.cs`.

use image::RgbaImage;

use crate::geometry::{Rect, combine};
use crate::{CursorImage, Screen, platform};

/// The monitors (`DesktopScreen.AllScreens`).
pub fn screens() -> Vec<Screen> {
    platform::screens()
}

/// Keeps the monitors winit reports: on Linux they are what [`screens`]
/// returns, so no window waits on a capture library. Call it as the event
/// loop starts and after each batch of events. Nothing elsewhere.
pub fn remember_screens(_event_loop: &winit::event_loop::ActiveEventLoop) {
    #[cfg(target_os = "linux")]
    platform::linux::remember_screens(_event_loop);
}

/// Finds out on a thread what can capture a Wayland desktop (screencopy,
/// the screenshot portal), and logs it. Nothing elsewhere.
pub fn probe_capture() {
    #[cfg(target_os = "linux")]
    platform::linux::probe_in_background();
}

/// Whether a grab is unseen by the user: not through a screenshot portal,
/// which may flash the screen or ask. What an idle warm-up may do.
pub fn quiet_grab() -> bool {
    #[cfg(target_os = "linux")]
    return platform::linux::quiet_grab();
    #[cfg(not(target_os = "linux"))]
    true
}

/// `CaptureHelpers.GetScreenBounds`: every monitor together.
pub fn screen_bounds(screens: &[Screen]) -> Rect {
    combine(screens.iter().map(|s| s.bounds))
}

/// `CaptureHelpers.GetActiveScreenBounds`: the monitor under the pointer
/// (else the nearest, as `Screen.FromPoint` does).
pub fn active_screen_bounds(screens: &[Screen], cursor: (i32, i32)) -> Rect {
    screen_from_point(screens, cursor).map(|s| s.bounds).unwrap_or_default()
}

/// `DesktopScreen.FromPoint`: the monitor containing the point, else the
/// nearest one.
pub fn screen_from_point(screens: &[Screen], (x, y): (i32, i32)) -> Option<&Screen> {
    screens.iter().find(|s| s.bounds.contains(x, y)).or_else(|| {
        screens.iter().min_by_key(|s| {
            let dx = (s.bounds.x - x).max(x - s.bounds.right() + 1).max(0) as i64;
            let dy = (s.bounds.y - y).max(y - s.bounds.bottom() + 1).max(0) as i64;
            dx * dx + dy * dy
        })
    })
}

/// `DesktopScreen.FromRectangle`: the monitor the rectangle overlaps most,
/// else the one nearest its centre.
pub fn screen_from_rectangle(screens: &[Screen], r: Rect) -> Option<&Screen> {
    let area = |s: &&Screen| {
        let i = s.bounds.intersect(&r);
        i.w as i64 * i.h as i64
    };
    match screens.iter().max_by_key(area) {
        Some(s) if area(&s) > 0 => Some(s),
        _ => screen_from_point(screens, (r.x + r.w / 2, r.y + r.h / 2)),
    }
}

/// `CaptureHelpers.GetCursorPosition`, where the platform can tell.
pub fn cursor_position() -> Option<(i32, i32)> {
    platform::cursor_position()
}

/// `Screenshot`: how the screen is grabbed.
#[derive(Clone, Debug)]
pub struct Screenshot {
    pub capture_cursor: bool,
    /// A window's client area only (`CaptureClientArea`).
    pub capture_client_area: bool,
    /// Clip what's asked for to the monitors.
    pub remove_outside_screen_area: bool,
    /// A transparent window capture keeps its shadow (`CaptureShadow`).
    pub capture_shadow: bool,
    /// How far around the window the shadow is captured (`ShadowOffset`).
    pub shadow_offset: i32,
    /// Hide the taskbar while capturing a window over it
    /// (`AutoHideTaskbar`).
    pub auto_hide_taskbar: bool,
    /// `HDRScreenshotColorCorrection`: on Windows, the GDI capture is
    /// corrected from a DXGI duplication of every HDR monitor it touches
    /// (`HDRScreenCapture`, [`crate::hdr`]); ShareX does nothing with it
    /// elsewhere.
    pub hdr_color_correction: bool,
}

impl Default for Screenshot {
    fn default() -> Self {
        Self {
            capture_cursor: false,
            capture_client_area: false,
            remove_outside_screen_area: true,
            capture_shadow: false,
            shadow_offset: 20,
            auto_hide_taskbar: false,
            hdr_color_correction: false,
        }
    }
}

/// `SetTaskbarVisibilityIfIntersect(false, rect)` until dropped, which
/// shows the taskbar again (`finally { SetTaskbarVisibility(true) }`).
#[derive(Debug)]
pub struct TaskbarHidden(bool);

impl TaskbarHidden {
    /// Hides the taskbar when it overlaps `rect`.
    pub fn if_intersects(rect: Rect) -> Self {
        Self(platform::set_taskbar_visibility_if_intersect(false, rect))
    }

    /// Whether it was hidden.
    pub fn hidden(&self) -> bool {
        self.0
    }
}

impl Drop for TaskbarHidden {
    fn drop(&mut self) {
        if self.0 {
            platform::set_taskbar_visibility(true);
        }
    }
}

/// `CaptureBase`'s `CaptureAutoHideDesktopIcons`: the desktop icons hidden
/// until dropped, which shows them again (its `finally`).
#[derive(Debug)]
pub struct DesktopIconsHidden(());

impl DesktopIconsHidden {
    /// Hides the icons unless the active window is fullscreen or they're
    /// hidden already; `None` when nothing was hidden.
    pub fn hide(screens: &[Screen]) -> Option<Self> {
        if platform::is_active_window_fullscreen(screens) || !platform::are_desktop_icons_visible() {
            return None;
        }
        platform::set_desktop_icons_visibility(false);
        Some(Self(()))
    }
}

impl Drop for DesktopIconsHidden {
    fn drop(&mut self) {
        platform::set_desktop_icons_visibility(true);
    }
}

impl Screenshot {
    /// `CaptureRectangle`: the screen's pixels in `rect` (desktop pixels).
    pub fn capture_rectangle(&self, screens: &[Screen], mut rect: Rect) -> Option<RgbaImage> {
        if self.remove_outside_screen_area {
            rect = screen_bounds(screens).intersect(&rect);
        }
        if rect.w <= 0 || rect.h <= 0 {
            return None;
        }
        capture_rectangle_native(rect, self.capture_cursor, self.hdr_color_correction)
    }

    /// `CaptureFullscreen`: every monitor.
    pub fn capture_fullscreen(&self, screens: &[Screen]) -> Option<RgbaImage> {
        self.capture_rectangle(screens, screen_bounds(screens))
    }

    /// `CaptureActiveMonitor`: the monitor under the pointer.
    pub fn capture_active_monitor(&self, screens: &[Screen], cursor: (i32, i32)) -> Option<RgbaImage> {
        self.capture_rectangle(screens, active_screen_bounds(screens, cursor))
    }

    /// `CaptureWindow(handle)`: the window's frame (DWM's, without the
    /// invisible resize borders), or its client area, with the taskbar
    /// hidden meanwhile when it overlaps and `auto_hide_taskbar` is on.
    pub fn capture_window(&self, screens: &[Screen], handle: isize) -> Option<RgbaImage> {
        if handle <= 0 {
            return None;
        }
        let rect = if self.capture_client_area { platform::client_rect(handle) } else { platform::window_rect(handle) };
        let _taskbar = self.auto_hide_taskbar.then(|| TaskbarHidden::if_intersects(rect));
        self.capture_rectangle(screens, rect)
    }

    /// `CaptureWindowTransparent(handle)`: with the window's transparency
    /// and, with `capture_shadow`, its shadow (falls back to
    /// [`Self::capture_window`] when the helper window can't be placed).
    pub fn capture_window_transparent(&self, screens: &[Screen], handle: isize) -> Option<RgbaImage> {
        self.transparent(screens, handle, None)
    }

    /// `CaptureWindowTransparent(handle, windowImage, windowRectangle)`: a
    /// region capture of the whole window (`window_image`, of `rect`) given
    /// its rounded corners and, with `capture_shadow`, its shadow; `None`
    /// keeps the region capture.
    pub fn capture_window_transparent_with(
        &self,
        screens: &[Screen],
        handle: isize,
        window_image: &RgbaImage,
        rect: Rect,
    ) -> Option<RgbaImage> {
        self.transparent(screens, handle, Some((window_image, rect)))
    }

    fn transparent(&self, screens: &[Screen], handle: isize, window_image: Option<(&RgbaImage, Rect)>) -> Option<RgbaImage> {
        if handle == 0 {
            return None;
        }
        // `SetTaskbarVisibilityIfIntersect(false, rect)` around it all.
        let mut rect = platform::window_rect(handle);
        if self.capture_shadow && !platform::is_zoomed(handle) {
            rect = Rect::new(rect.x - self.shadow_offset, rect.y - self.shadow_offset, rect.w + 2 * self.shadow_offset, rect.h + 2 * self.shadow_offset);
        }
        let _taskbar = self.auto_hide_taskbar.then(|| TaskbarHidden::if_intersects(rect));
        let t = crate::transparent::TransparentCapture {
            screens,
            capture_shadow: self.capture_shadow,
            shadow_offset: self.shadow_offset,
            capture_cursor: self.capture_cursor,
            hdr_color_correction: self.hdr_color_correction,
        };
        t.capture(handle, window_image, &|| self.capture_window(screens, handle))
    }
}

/// `CaptureRectangleNative`: the screen's pixels in `rect` through GDI,
/// with the pointer drawn by `DrawIconEx`; with HDR correction (Windows),
/// GDI without the pointer, corrected from a DXGI duplication of the HDR
/// monitors (`HDRScreenCapture.ApplyColorCorrection`; a failure is logged
/// and leaves GDI's), then the pointer drawn as
/// `CursorData.DrawCursor(SKBitmap, rect.Location)` does.
pub(crate) fn capture_rectangle_native(rect: Rect, capture_cursor: bool, hdr_color_correction: bool) -> Option<RgbaImage> {
    if rect.w == 0 || rect.h == 0 {
        return None;
    }
    #[cfg(windows)]
    if hdr_color_correction {
        let mut image = platform::capture_rectangle(rect)?;
        if let Err(e) = platform::windows_hdr::apply_color_correction(&mut image, rect) {
            eprintln!("HDR screenshot color correction failed. {e}");
        }
        if capture_cursor && let Some(cursor) = cursor_image(&image, (rect.x, rect.y)) {
            draw_cursor(&mut image, &cursor);
        }
        return Some(image);
    }
    #[cfg(not(windows))]
    let _ = hdr_color_correction;
    let mut image = platform::capture_rectangle(rect)?;
    // `CaptureRectangleGDI`: `CursorData.DrawCursor(dc, rect.Location)`.
    if capture_cursor {
        platform::draw_cursor_gdi(&mut image, (rect.x, rect.y));
    }
    Some(image)
}

/// `CursorData`: the pointer as it shows now, positioned for a screenshot
/// of `image` taken at `origin` (desktop pixels); `None` when it's hidden
/// or the platform can't tell.
pub fn cursor_image(image: &RgbaImage, origin: (i32, i32)) -> Option<CursorImage> {
    platform::cursor_image(image, origin)
}

/// Draws `cursor` over the opaque `image` as the region capture shows the
/// pointer: `CursorData.ToBitmap` (premultiplied) drawn with
/// `SKCanvas.DrawBitmap` (source over), as `DrawCursor(SKBitmap)` does.
pub fn draw_cursor(image: &mut RgbaImage, cursor: &CursorImage) {
    use sharex_skia::raster::{Bitmap, Canvas};
    let mut frame = Bitmap::from_straight(image.width(), image.height(), image.as_raw());
    let pointer = Bitmap::from_straight(cursor.image.width(), cursor.image.height(), cursor.image.as_raw());
    Canvas::new(&mut frame).draw_bitmap(&pointer, cursor.position.0 as f32, cursor.position.1 as f32, None);
    // Opaque: the premultiplied bytes are the colours.
    *image = RgbaImage::from_raw(image.width(), image.height(), frame.into_data()).unwrap_or_default();
}

/// Where each monitor is in the desktop the screenshot is cut from, given
/// where the platform places it (its rect and scale). macOS and Wayland put
/// a monitor at its logical position times its own scale but give it its
/// full size in pixels, so mixed scales overlap; then each axis is laid out
/// again so monitors keep their order and no longer overlap. Where nothing
/// overlaps (Windows, one scale), they're as given. From snapr.
pub fn untangle(monitors: &[(Rect, f64)]) -> Vec<Rect> {
    let overlapping = monitors
        .iter()
        .enumerate()
        .any(|(i, (a, _))| monitors[i + 1..].iter().any(|(b, _)| !a.intersect(b).is_empty() && a.intersect(b).is_valid()));
    if !overlapping {
        return monitors.iter().map(|(r, _)| *r).collect();
    }
    let max_scale = monitors.iter().map(|(_, s)| *s).fold(1.0, f64::max);
    let axis = |get: fn(&Rect) -> (i32, i32)| -> Vec<i32> {
        let spans: Vec<(f64, f64, f64)> = monitors
            .iter()
            .map(|(r, s)| {
                let s = s.max(f64::MIN_POSITIVE);
                let (at, len) = get(r);
                (at as f64 / s, len as f64 / s, s)
            })
            .collect();
        let mut stops: Vec<f64> = spans.iter().flat_map(|&(a, l, _)| [a, a + l]).chain([0.0]).collect();
        stops.sort_by(f64::total_cmp);
        stops.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
        let pixels: Vec<i32> = stops
            .windows(2)
            .map(|w| {
                let scale = spans
                    .iter()
                    .filter(|&&(a, l, _)| a <= w[0] + 1e-9 && a + l >= w[1] - 1e-9)
                    .map(|&(.., s)| s)
                    .reduce(f64::max)
                    .unwrap_or(max_scale);
                ((w[1] - w[0]) * scale - 1e-6).ceil() as i32
            })
            .collect();
        let zero = stops.iter().position(|s| s.abs() < 1e-9).expect("0 is a stop");
        let mut at = vec![0; stops.len()];
        for i in zero + 1..stops.len() {
            at[i] = at[i - 1] + pixels[i - 1];
        }
        for i in (0..zero).rev() {
            at[i] = at[i + 1] - pixels[i];
        }
        spans
            .iter()
            .map(|&(a, ..)| {
                let i = stops.iter().position(|s| (s - a).abs() < 1e-9).expect("a stop");
                at[i]
            })
            .collect()
    };
    let xs = axis(|r| (r.x, r.w));
    let ys = axis(|r| (r.y, r.h));
    monitors.iter().zip(xs.into_iter().zip(ys)).map(|((r, _), (x, y))| Rect { x, y, ..*r }).collect()
}

/// The picture of `region` from monitor shots placed in the desktop;
/// areas no monitor covers stay transparent. From snapr's `crop`.
pub fn crop_placed(shots: &[(RgbaImage, Rect)], region: Rect) -> Option<RgbaImage> {
    if region.w <= 0 || region.h <= 0 {
        return None;
    }
    let mut out = RgbaImage::new(region.w as u32, region.h as u32);
    let mut any = false;
    for (image, rect) in shots {
        let inter = region.intersect(rect);
        if !inter.is_valid() {
            continue;
        }
        any = true;
        let (iw, ih) = image.dimensions();
        let sx = iw as f64 / rect.w as f64;
        let sy = ih as f64 / rect.h as f64;
        for gy in inter.y..inter.bottom() {
            let src_y = (((gy - rect.y) as f64 * sy) as u32).min(ih - 1);
            for gx in inter.x..inter.right() {
                let src_x = (((gx - rect.x) as f64 * sx) as u32).min(iw - 1);
                out.put_pixel((gx - region.x) as u32, (gy - region.y) as u32, *image.get_pixel(src_x, src_y));
            }
        }
    }
    any.then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    #[test]
    fn mixed_scales_are_untangled_side_by_side() {
        let placed = untangle(&[(Rect::new(0, 0, 3840, 2160), 2.0), (Rect::new(1920, 0, 1920, 1080), 1.0)]);
        assert_eq!(placed, [Rect::new(0, 0, 3840, 2160), Rect::new(3840, 0, 1920, 1080)]);
    }

    #[test]
    fn monitors_that_dont_overlap_stay_put() {
        let windows = [(Rect::new(0, 0, 3840, 2160), 1.5), (Rect::new(3840, 0, 1920, 1080), 1.0)];
        assert_eq!(untangle(&windows), [windows[0].0, windows[1].0]);
    }

    #[test]
    fn crop_stitches_across_monitors() {
        let left = RgbaImage::from_pixel(10, 10, Rgba([255, 0, 0, 255]));
        let right = RgbaImage::from_pixel(10, 10, Rgba([0, 0, 255, 255]));
        let shots = [(left, Rect::new(0, 0, 10, 10)), (right, Rect::new(10, 0, 10, 10))];
        let out = crop_placed(&shots, Rect::new(8, 2, 4, 2)).unwrap();
        assert_eq!(out.get_pixel(1, 0), &Rgba([255, 0, 0, 255]));
        assert_eq!(out.get_pixel(2, 1), &Rgba([0, 0, 255, 255]));
    }

    #[test]
    fn the_nearest_screen_stands_in_for_a_point_off_every_screen() {
        let s = |x, primary| Screen { bounds: Rect::new(x, 0, 100, 100), work_area: Rect::new(x, 0, 100, 100), scale: 1.0, primary };
        let screens = [s(0, true), s(100, false)];
        assert_eq!(screen_from_point(&screens, (150, 50)).unwrap().bounds.x, 100);
        assert_eq!(screen_from_point(&screens, (500, 50)).unwrap().bounds.x, 100);
        assert_eq!(active_screen_bounds(&screens, (5, 5)), Rect::new(0, 0, 100, 100));
    }

    #[test]
    fn cursors_blend_over_the_screenshot() {
        let mut image = RgbaImage::from_pixel(4, 4, Rgba([0, 0, 0, 255]));
        let cursor = CursorImage { image: RgbaImage::from_pixel(2, 2, Rgba([255, 255, 255, 128])), position: (3, 3) };
        draw_cursor(&mut image, &cursor);
        assert_eq!(image.get_pixel(3, 3), &Rgba([128, 128, 128, 255]));
        assert_eq!(image.get_pixel(2, 2), &Rgba([0, 0, 0, 255]));
    }
}

/// The pointer drawn into a frame as ShareXrs draws it, for comparing with
/// ShareX's drawing (`tools/tools-harness --cursors <dir>`): with
/// `SHAREXRS_CURSOR_OUT=<dir>`, writes `<name>.png` for each standard
/// cursor with its hotspot at (32, 32) of the 64x64 pattern frame.
#[cfg(all(test, windows))]
mod cursor_frames {
    use super::*;

    #[test]
    fn standard_cursors_into_the_pattern() {
        let Ok(out) = std::env::var("SHAREXRS_CURSOR_OUT") else { return };
        std::fs::create_dir_all(&out).unwrap();
        let pattern = |x: i32, y: i32| -> image::Rgba<u8> {
            if x % 64 == 0 || y % 64 == 0 {
                return image::Rgba([255, 255, 255, 255]);
            }
            let checker = ((x / 16) + (y / 16)) % 2 == 0;
            image::Rgba([(x % 256) as u8, (y % 256) as u8, if checker { 200 } else { 70 }, 255])
        };
        let frame = RgbaImage::from_fn(64, 64, |x, y| pattern(x as i32 + 100, y as i32 + 100));
        for (name, id) in [("arrow", 32512u16), ("ibeam", 32513), ("cross", 32515), ("hand", 32649), ("sizeall", 32646), ("no", 32648)] {
            // The region capture's way.
            let mut img = frame.clone();
            let cursor = platform::system_cursor_image(id, (32, 32), &img, (0, 0)).expect("a cursor");
            draw_cursor(&mut img, &cursor);
            img.save(format!("{out}/{name}-bitmap.png")).unwrap();
            // A screenshot's way.
            let mut img = frame.clone();
            assert!(platform::draw_system_cursor_gdi(&mut img, id, (32, 32), (0, 0)));
            img.save(format!("{out}/{name}-gdi.png")).unwrap();
        }
    }
}
