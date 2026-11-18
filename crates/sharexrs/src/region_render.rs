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
    let mut steps: Option<String> = None;
    // The monitor's work area (ShareX's reference machine: a 48px taskbar).
    let mut work_area = None;

    for (k, v) in args {
        match k.as_str() {
            "--pointer" => {
                let p = numbers(v, 2, k)?;
                pointer = (p[0], p[1]);
            }
            "--drag" => drag = Some(numbers(v, 4, k)?),
            "--window" => window = Some(numbers(v, 4, k)?),
            "--steps" => steps = Some(v.clone()),
            "--work-area" => work_area = Some(numbers(v, 4, k)?),
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
    // `--work-area L,T,R,B`.
    let work_area = work_area.map(|r| Rect::new(r[0], r[1], r[2] - r[0], r[3] - r[1])).unwrap_or(bounds);
    let screen = Screen { bounds, work_area, scale: scale as f64, primary: true };
    let mut session = RegionCapture::new(Request {
        screenshot: shot,
        screen_bounds: bounds,
        screens: vec![screen],
        options: RegionCaptureOptions::default(),
        windows,
        cursor: Some(pointer),
        last_regions: Vec::new(),
        enable_annotations: true,
        editor_options: Default::default(),
    });
    let none = Modifiers::default();
    let later = Instant::now() + Duration::from_secs(1);
    session.tick(later);
    let mut view = OverlayView::new(bounds);
    let mut screen = Offscreen::new([w, h], scale, dark)?;
    if let Some(steps) = steps {
        // The toolbar under the pointer comes from the painted overlay, as
        // in a live session (a frame is drawn before each pointer move).
        let mut hover = |s: &mut RegionCapture, p: PointF| {
            screen.render(|ui| s.paint(ui, &mut view, later));
            let part = view.toolbar_part_at(egui::pos2(p.x as f32 / scale, p.y as f32 / scale));
            s.set_toolbar_hover(part);
        };
        run_steps(&mut session, &steps, pointer, scale as f64, &mut hover)?;
    } else if let Some(d) = drag {
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
    Ok(screen.render(|ui| session.paint(ui, &mut view, later)))
}

/// Runs `--steps` (`;`-separated, as `scripts/annotate-session.ps1` takes
/// them): `tool:NAME` clicks a toolbar tool (`region` the region button),
/// `option:NAME` an options row toggle, `move:X,Y`, `click:X,Y`,
/// `down:X,Y`, `to:X,Y` (12 moves with the button held), `up`,
/// `rclick:X,Y`, `key:NAME`, `ctrl:NAME`, `type:TEXT`.
fn run_steps(
    session: &mut RegionCapture,
    steps: &str,
    start: (i32, i32),
    scale: f64,
    hover: &mut dyn FnMut(&mut RegionCapture, PointF),
) -> Result<(), String> {
    use sharex_capture::region::{Key, toolbar};
    let none = Modifiers::default();
    let mut at = PointF::new(start.0 as f64, start.1 as f64);
    // A move: the toolbar hover first while no button is down (as the
    // live session does), then the overlay.
    let mut moved = |s: &mut RegionCapture, p: PointF, down: bool| {
        if !down {
            hover(s, p);
        }
        s.pointer_moved(p, none);
    };
    let xy = |v: &str| -> Result<PointF, String> {
        let n = numbers(v, 2, "a step")?;
        Ok(PointF::new(n[0] as f64, n[1] as f64))
    };
    let key = |name: &str| -> Key {
        match name {
            "Tab" => Key::Tab,
            "Escape" => Key::Escape,
            "Enter" => Key::Enter,
            "Delete" => Key::Delete,
            "Back" => Key::Backspace,
            "Left" => Key::Left,
            "Right" => Key::Right,
            "Up" => Key::Up,
            "Down" => Key::Down,
            n if n.len() == 1 && n.chars().all(|c| c.is_ascii_alphabetic()) => Key::Letter(n.chars().next().unwrap().to_ascii_uppercase()),
            _ => Key::Other,
        }
    };
    for step in steps.split(';').map(str::trim).filter(|s| !s.is_empty()) {
        let (verb, arg) = step.split_once(':').unwrap_or((step, ""));
        match verb {
            "tool" | "option" => {
                let part = if verb == "tool" {
                    if arg == "region" {
                        toolbar::Part::RegionButton
                    } else {
                        let i = toolbar::TOOLS.iter().position(|t| t.id == arg).ok_or(format!("no tool {arg}"))?;
                        toolbar::Part::Tool(i)
                    }
                } else {
                    let item = sharex_capture::region::options_row::OptionItem::from_name(arg).ok_or(format!("no option {arg}"))?;
                    toolbar::Part::Option(item)
                };
                session.set_toolbar_hover(Some(part));
                session.pointer_pressed(at, Button::Left, none, scale);
                session.pointer_released(at, Button::Left, none);
                session.set_toolbar_hover(None);
            }
            "move" => {
                at = xy(arg)?;
                moved(session, at, false);
            }
            "click" => {
                at = xy(arg)?;
                moved(session, at, false);
                session.pointer_pressed(at, Button::Left, none, scale);
                session.pointer_released(at, Button::Left, none);
            }
            "rclick" => {
                at = xy(arg)?;
                moved(session, at, false);
                session.pointer_pressed(at, Button::Right, none, scale);
                session.pointer_released(at, Button::Right, none);
            }
            "down" => {
                at = xy(arg)?;
                moved(session, at, false);
                session.pointer_pressed(at, Button::Left, none, scale);
            }
            "to" => {
                let to = xy(arg)?;
                let from = at;
                for i in 1..=12 {
                    let t = i as f64 / 12.0;
                    at = PointF::new((from.x + (to.x - from.x) * t).round_ties_even(), (from.y + (to.y - from.y) * t).round_ties_even());
                    moved(session, at, true);
                }
            }
            "up" => {
                session.pointer_released(at, Button::Left, none);
            }
            "key" => {
                session.key_down(key(arg), none, &mut |_, _| {});
                session.key_up(key(arg), none);
            }
            "ctrl" => {
                let m = Modifiers { ctrl: true, ..none };
                session.key_down(key(arg), m, &mut |_, _| {});
                session.key_up(key(arg), m);
            }
            "type" => session.text_input(arg),
            other => return Err(format!("unknown step {other}")),
        }
    }
    Ok(())
}
