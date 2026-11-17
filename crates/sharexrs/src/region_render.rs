//! `sharexrs --render region-*`: the region capture overlay over the test
//! pattern, driven by the same input a live session gets, for comparing
//! with screenshots of ShareX's overlay over `--test-pattern`.

use std::time::{Duration, Instant};

use sharex_capture::region::{Button, Modifiers, OverlayView, RegionCapture, Request};
use sharex_capture::{PointF, Rect, RegionCaptureOptions, Screen, SimpleWindowInfo};
use sharex_ui::offscreen::Offscreen;

fn numbers(v: &str, n: usize, what: &str) -> Result<Vec<i32>, String> {
    let v: Vec<i32> = v.split(',').filter_map(|s| s.trim().parse().ok()).collect();
    if v.len() == n { Ok(v) } else { Err(format!("{what} needs {n} numbers")) }
}

pub fn render(
    args: &[(String, String)],
    width: Option<u32>,
    height: Option<u32>,
    scale: f32,
    dark: bool,
) -> Result<image::RgbaImage, String> {
    let (w, h) = (width.unwrap_or(1024), height.unwrap_or(768));
    let mut pointer = (300, 300);
    let mut drag = None;
    let mut window = None;
    for (k, v) in args {
        match k.as_str() {
            "--pointer" => {
                let p = numbers(v, 2, k)?;
                pointer = (p[0], p[1]);
            }
            "--drag" => drag = Some(numbers(v, 4, k)?),
            "--window" => window = Some(numbers(v, 4, k)?),
            _ => {}
        }
    }
    // The frozen screen: the pattern, and the test window over it.
    let mut shot = image::RgbaImage::new(w, h);
    for (x, y, p) in shot.enumerate_pixels_mut() {
        let inside = window.as_ref().is_some_and(|r| {
            x as i32 >= r[0] && (x as i32) < r[0] + r[2] && y as i32 >= r[1] && (y as i32) < r[1] + r[3]
        });
        *p = image::Rgba(crate::test_pattern::pixel(x as i32, y as i32, inside));
    }
    let bounds = Rect::new(0, 0, w as i32, h as i32);
    let mut windows = Vec::new();
    if let Some(r) = &window {
        windows.push(SimpleWindowInfo { handle: 2, rect: Rect::new(r[0], r[1], r[2], r[3]), is_window: true });
    }
    windows.push(SimpleWindowInfo { handle: 1, rect: bounds, is_window: true });
    let screen = Screen { bounds, work_area: bounds, scale: scale as f64, primary: true };
    let mut session = RegionCapture::new(Request {
        screenshot: shot,
        screen_bounds: bounds,
        screens: vec![screen],
        options: RegionCaptureOptions::default(),
        windows,
        cursor: Some(pointer),
        last_regions: Vec::new(),
    });
    let none = Modifiers::default();
    if let Some(d) = drag {
        let from = PointF::new(d[0] as f64, d[1] as f64);
        session.pointer_moved(from, none);
        session.pointer_pressed(from, Button::Left, none, scale as f64);
        for i in 1..=12 {
            let t = i as f64 / 12.0;
            let p = PointF::new(d[0] as f64 + (d[2] - d[0]) as f64 * t, d[1] as f64 + (d[3] - d[1]) as f64 * t);
            session.pointer_moved(p, none);
        }
    } else {
        session.pointer_moved(PointF::new(pointer.0 as f64, pointer.1 as f64), none);
    }
    let later = Instant::now() + Duration::from_secs(1);
    session.tick(later);
    let mut view = OverlayView::new(bounds);
    let mut screen = Offscreen::new([w, h], scale, dark)?;
    Ok(screen.render(|ui| session.paint(ui, &mut view, later)))
}
