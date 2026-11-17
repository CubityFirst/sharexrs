//! `ShareX.ScreenCaptureLib/Screenshot.cs` and the screen helpers of
//! `ShareX.HelpersLib/Helpers/CaptureHelpers.cs`.

use image::RgbaImage;

use crate::geometry::{Rect, combine};
use crate::{CursorImage, Screen, platform};

/// The monitors (`DesktopScreen.AllScreens`).
pub fn screens() -> Vec<Screen> {
    platform::screens()
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

/// `CaptureHelpers.GetCursorPosition`, where the platform can tell.
pub fn cursor_position() -> Option<(i32, i32)> {
    platform::cursor_position()
}

/// `Screenshot`: how the screen is grabbed.
#[derive(Clone, Debug)]
pub struct Screenshot {
    pub capture_cursor: bool,
    /// Clip what's asked for to the monitors.
    pub remove_outside_screen_area: bool,
}

impl Default for Screenshot {
    fn default() -> Self {
        Self { capture_cursor: false, remove_outside_screen_area: true }
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
        let mut image = platform::capture_rectangle(rect)?;
        if self.capture_cursor
            && let Some(cursor) = platform::cursor_image(&image, (rect.x, rect.y))
        {
            draw_cursor(&mut image, &cursor);
        }
        Some(image)
    }

    /// `CaptureFullscreen`: every monitor.
    pub fn capture_fullscreen(&self, screens: &[Screen]) -> Option<RgbaImage> {
        self.capture_rectangle(screens, screen_bounds(screens))
    }

    /// `CaptureActiveMonitor`: the monitor under the pointer.
    pub fn capture_active_monitor(&self, screens: &[Screen], cursor: (i32, i32)) -> Option<RgbaImage> {
        self.capture_rectangle(screens, active_screen_bounds(screens, cursor))
    }
}

/// `CursorData`: the pointer as it shows now, positioned for a screenshot
/// of `image` taken at `origin` (desktop pixels); `None` when it's hidden
/// or the platform can't tell.
pub fn cursor_image(image: &RgbaImage, origin: (i32, i32)) -> Option<CursorImage> {
    platform::cursor_image(image, origin)
}

/// Draws `cursor` over `image` (source over), as the region capture inserts
/// the pointer as an image annotation.
pub fn draw_cursor(image: &mut RgbaImage, cursor: &CursorImage) {
    let (ox, oy) = cursor.position;
    for (x, y, p) in cursor.image.enumerate_pixels() {
        let (tx, ty) = (ox + x as i32, oy + y as i32);
        if tx < 0 || ty < 0 || tx as u32 >= image.width() || ty as u32 >= image.height() || p[3] == 0 {
            continue;
        }
        let d = image.get_pixel_mut(tx as u32, ty as u32);
        let a = p[3] as u32;
        for c in 0..3 {
            d[c] = ((p[c] as u32 * a + d[c] as u32 * (255 - a) + 127) / 255) as u8;
        }
        d[3] = 255;
    }
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
