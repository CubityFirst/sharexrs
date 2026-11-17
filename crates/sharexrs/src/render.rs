//! `sharexrs --render <screen> --out <png> [--theme dark|light]
//! [--width W] [--height H] [--scale S] [--hover X,Y] [--unregistered 0,3]
//! [--open 12,0] [--work-area L,T,R,B] [--monitors 1024x768,...]`: draws one screen offscreen, for
//! comparing with a screenshot of ShareX.
//!
//! Screens: `main` (the main window with no tasks), `main-<menu>-menu` (the
//! main window with a sidebar menu open: `capture`, `upload`, `workflows`,
//! `tools`, `after-capture-tasks`, `after-upload-tasks`, `destinations`,
//! `debug`), `app-icon` (ShareXrs's icon on transparency, `--width` pixels
//! square), `hotkey-failed` (the "Hotkey registration failed" message box,
//! for the hotkeys `--unregistered` lists: Ctrl and Shift + Print Screen by
//! default).
//!
//! `--open` opens submenus along a path of entry indices (separators
//! count: 12 is Capture › Screenshot delay). `--monitors` lists the
//! monitors Capture › Monitor shows (the offscreen renderer has none). `--work-area` is where menus must stay, in window
//! pixels (Avalonia keeps popups on the monitor's work area, not in the
//! window: for ShareX at 62,5 on a 1024x720 work area, `-62,-5,962,715`).
//! `--hover` moves the pointer there and lets 0.7s pass, so submenus open
//! as they do after `MenuShowDelay`.

use sharex_ui::offscreen::Offscreen;

use sharex_settings::{HotkeySettings, HotkeyStatus, HotkeysConfig, TaskSettings};

use crate::main_window::{self, MainWindow};
use crate::message_box::{self, MessageBox, MessageBoxIcon};

/// ShareX's default hotkeys, registered except those at `failed`.
fn default_hotkeys(failed: &[usize]) -> Vec<HotkeySettings> {
    let mut hotkeys = HotkeysConfig::new(&TaskSettings::default()).Hotkeys;
    for (i, h) in hotkeys.iter_mut().enumerate() {
        h.HotkeyInfo.status = if failed.contains(&i) { HotkeyStatus::Failed } else { HotkeyStatus::Registered };
    }
    hotkeys
}

pub fn run(args: &[String]) -> Result<(), String> {
    let mut screen = None;
    let mut out = None;
    let mut dark = true;
    let (mut width, mut height, mut scale) = (None, None, 1.0f32);
    let mut hover = None;
    let mut unregistered: Vec<usize> = Vec::new();
    let mut drop_overlay = false;
    let mut maximized = false;
    let mut open: Vec<usize> = Vec::new();
    let mut work_area = None;
    let mut monitors: Vec<(i32, i32, u32, u32)> = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut val = || it.next().cloned().ok_or(format!("{a} needs a value"));
        match a.as_str() {
            "--out" => out = Some(val()?),
            "--theme" => dark = val()? != "light",
            "--width" => width = Some(val()?.parse::<u32>().map_err(|e| e.to_string())?),
            "--height" => height = Some(val()?.parse::<u32>().map_err(|e| e.to_string())?),
            "--scale" => scale = val()?.parse().map_err(|e: std::num::ParseFloatError| e.to_string())?,
            "--hover" => {
                let v = val()?;
                let (x, y) = v.split_once(',').ok_or("--hover X,Y")?;
                hover = Some(egui::pos2(x.parse().map_err(|_| "--hover X,Y")?, y.parse().map_err(|_| "--hover X,Y")?));
            }
            "--open" => open = val()?.split(',').filter_map(|i| i.trim().parse().ok()).collect(),
            "--work-area" => {
                let v: Vec<f32> = val()?.split(',').filter_map(|i| i.trim().parse().ok()).collect();
                let [l, t, r, b] = v[..] else { return Err("--work-area L,T,R,B".into()) };
                work_area = Some(egui::Rect::from_min_max(egui::pos2(l, t), egui::pos2(r, b)));
            }
            "--monitors" => {
                for m in val()?.split(',') {
                    let (w, h) = m.split_once('x').ok_or("--monitors WxH,...")?;
                    monitors.push((0, 0, w.parse().map_err(|_| "--monitors WxH")?, h.parse().map_err(|_| "--monitors WxH")?));
                }
            }
            "--drop-overlay" => drop_overlay = true,
            "--maximized" => maximized = true,
            "--unregistered" => {
                unregistered = val()?.split(',').filter_map(|i| i.trim().parse().ok()).collect();
            }
            s if !s.starts_with("--") && screen.is_none() => screen = Some(s.to_string()),
            other => return Err(format!("unknown argument {other}")),
        }
    }
    let screen = screen.ok_or("which screen? (main)")?;
    let out = out.ok_or("--out <png> is required")?;

    let menu = screen.strip_prefix("main-").and_then(|m| m.strip_suffix("-menu"));
    let image = match screen.as_str() {
        _ if screen == "main" || menu.is_some() => {
            let mut state = MainWindow::new();
            state.hotkey_tips = main_window::hotkey_tips(&default_hotkeys(&unregistered));
            state.drop_overlay = drop_overlay;
            state.maximized = maximized;
            // The window's height defaults to the sidebar's, as in ShareX.
            let probe = Offscreen::new([1, 1], scale, dark)?;
            let h = height.unwrap_or_else(|| (state.opening_height(&probe.ctx) * scale).round() as u32);
            drop(probe);
            let w = width.unwrap_or((main_window::WIDTH * scale) as u32);
            let mut screen = Offscreen::new([w, h], scale, dark)?;
            state.menu_state.monitors = monitors.clone();
            state.work_area = work_area.map(|r: egui::Rect| egui::Rect::from_min_max((r.min.to_vec2() / scale).to_pos2(), (r.max.to_vec2() / scale).to_pos2()));
            let nav_index = match menu {
                None => None,
                Some(m) => Some(match m {
                    "capture" => 0,
                    "upload" => 1,
                    "workflows" => 2,
                    "tools" => 3,
                    "after-capture-tasks" => 4,
                    "after-upload-tasks" => 5,
                    "destinations" => 6,
                    "debug" => 16,
                    other => return Err(format!("no menu named {other}")),
                }),
            };
            let events: Vec<egui::Event> = hover.map(egui::Event::PointerMoved).into_iter().collect();
            // Two frames to lay out the sidebar, then open the menu beside
            // its entry, as a click would; then enough frames (at 60 per
            // second) for a hovered submenu to open.
            for frame in 0..48 {
                if frame == 2
                    && let Some(i) = nav_index
                {
                    let anchor = state.nav_rects.iter().find(|(j, _)| *j == i).map(|(_, r)| *r).ok_or("no such entry")?;
                    state.open_nav(i, anchor);
                    if let Some(f) = &mut state.flyout {
                        f.open_path(&open);
                    }
                }
                let ev = if frame >= 3 { events.clone() } else { Vec::new() };
                screen.run(ev, &mut |ui: &mut egui::Ui| {
                    state.ui(ui);
                });
            }
            screen.draw()
        }
        "hotkey-failed" => {
            // The message box ShareX shows when hotkeys fail to register
            // (`--unregistered`, default 0,3: Ctrl/Shift + Print Screen).
            if unregistered.is_empty() {
                unregistered = vec![0, 3];
            }
            let failed = crate::hotkeys::failed_hotkeys(&default_hotkeys(&unregistered)).ok_or("no failed hotkeys")?;
            let dialog = MessageBox::ok(failed.caption, failed.text, MessageBoxIcon::Warning);
            let mut probe = Offscreen::new([1, 1], scale, dark)?;
            let mut h = 0.0;
            probe.run(Vec::new(), &mut |ui: &mut egui::Ui| h = dialog.height(ui));
            let w = width.unwrap_or((message_box::WIDTH * scale) as u32);
            let h = height.unwrap_or((h * scale).round() as u32);
            let mut screen = Offscreen::new([w, h], scale, dark)?;
            screen.render(|ui| {
                dialog.ui(ui);
            })
        }
        "app-icon" => {
            let side = width.unwrap_or(256);
            let mut screen = Offscreen::new([side, side], 1.0, dark)?;
            screen.transparent = true;
            screen.render(|ui| {
                let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::splat(side as f32));
                crate::app_icon::paint(ui, rect);
            })
        }
        other => return Err(format!("no screen named {other}")),
    };
    if let Some(dir) = std::path::Path::new(&out).parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    image.save(&out).map_err(|e| e.to_string())?;
    println!("{screen}: {}x{} -> {out}", image.width(), image.height());
    Ok(())
}
