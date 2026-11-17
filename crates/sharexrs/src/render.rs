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
//!
//! `main-tasks` is the main window with tasks: `--tasks a.png,b.png,...`
//! (finished region captures of these files, oldest first, so the last is
//! shown first), `--list` for the list view, `--select 0,2` (by position
//! shown), `--context X,Y` (a right click there, which selects and opens
//! the context menu; `--open` then opens its submenus), `--hover X,Y`.
//!
//! `file-delete-confirm` is the "File delete confirmation" message box.
//!
//! `region-rest`, `region-drag` and `region-hover` draw the region capture
//! overlay over `--test-pattern`'s picture (`--width`/`--height`, default
//! 1024x768): `--pointer X,Y` is where the pointer is, `--drag X1,Y1,X2,Y2`
//! a selection being drawn, `--window X,Y,W,H` a window on top of the
//! pattern to snap to (as `--test-pattern --window` shows it).

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

struct TaskScene<'a> {
    files: &'a [std::path::PathBuf],
    list: bool,
    select: &'a [usize],
    context: Option<egui::Pos2>,
    open: &'a [usize],
    hover: Option<egui::Pos2>,
    width: Option<u32>,
    height: Option<u32>,
    scale: f32,
    dark: bool,
    work_area: Option<egui::Rect>,
}

/// The main window with finished region capture tasks of `files`.
fn render_tasks(scene: TaskScene) -> Result<image::RgbaImage, String> {
    use sharex_settings::TaskViewMode;
    let mut state = MainWindow::new();
    state.hotkey_tips = main_window::hotkey_tips(&default_hotkeys(&[]));
    let probe = Offscreen::new([1, 1], scene.scale, scene.dark)?;
    let h = scene.height.unwrap_or_else(|| (state.opening_height(&probe.ctx) * scene.scale).round() as u32);
    drop(probe);
    let w = scene.width.unwrap_or((main_window::WIDTH * scene.scale) as u32);
    let mut screen = Offscreen::new([w, h], scene.scale, scene.dark)?;
    state.work_area = scene
        .work_area
        .map(|r| egui::Rect::from_min_max((r.min.to_vec2() / scene.scale).to_pos2(), (r.max.to_vec2() / scene.scale).to_pos2()));
    let mut settings = state.tasks.settings.clone();
    if scene.list {
        settings.mode = TaskViewMode::ListView;
    }
    state.tasks.set_settings(settings);
    for file in scene.files {
        let name = file.file_stem().map(|n| format!("{}.bmp", n.to_string_lossy())).unwrap_or_default();
        let id = state.tasks.manager.start_image_task(name);
        state.tasks.add_task(id);
        state.tasks.manager.complete(id, Some(file.clone()), None, 10);
    }
    let n = scene.files.len() as u64;
    for (k, i) in scene.select.iter().enumerate() {
        let id = n - *i as u64;
        if k == 0 {
            state.tasks.select_only(id);
        } else {
            state.tasks.add_to_selection(id);
        }
    }
    let button = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Secondary,
        pressed,
        modifiers: Default::default(),
    };
    for frame in 0..48 {
        let mut events = Vec::new();
        if let Some(c) = scene.context {
            match frame {
                3 => events.extend([egui::Event::PointerMoved(c), button(c, true)]),
                4 => events.push(button(c, false)),
                5 => {
                    if let Some(f) = &mut state.tasks.flyout {
                        f.open_path(scene.open);
                    }
                }
                _ => {}
            }
        }
        if frame >= 6
            && let Some(p) = scene.hover
        {
            events.push(egui::Event::PointerMoved(p));
        }
        screen.run(events, &mut |ui: &mut egui::Ui| {
            state.ui(ui);
        });
        state.tasks.load_images_now(&screen.ctx);
    }
    Ok(screen.draw())
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
    let mut rest_args: Vec<(String, String)> = Vec::new();
    let mut task_files: Vec<std::path::PathBuf> = Vec::new();
    let mut list_view = false;
    let mut select: Vec<usize> = Vec::new();
    let mut context = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut val = || it.next().cloned().ok_or(format!("{a} needs a value"));
        match a.as_str() {
            "--pointer" | "--drag" | "--window" => rest_args.push((a.clone(), val()?)),
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
            // With the platform's separators, as the saved files' paths have them.
            "--tasks" => {
                task_files = val()?
                    .split(',')
                    .map(|p| std::path::PathBuf::from(p.replace('/', std::path::MAIN_SEPARATOR_STR)))
                    .collect()
            }
            "--list" => list_view = true,
            "--select" => select = val()?.split(',').filter_map(|i| i.trim().parse().ok()).collect(),
            "--context" => {
                let v = val()?;
                let (x, y) = v.split_once(',').ok_or("--context X,Y")?;
                context = Some(egui::pos2(x.parse().map_err(|_| "--context X,Y")?, y.parse().map_err(|_| "--context X,Y")?));
            }
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
        "main-tasks" => render_tasks(TaskScene {
            files: &task_files,
            list: list_view,
            select: &select,
            context,
            open: &open,
            hover,
            width,
            height,
            scale,
            dark,
            work_area,
        })?,
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
        s if s.starts_with("region-") => crate::region_render::render(&rest_args, width, height, scale, dark)?,
        "hotkey-failed" => {
            // The message box ShareX shows when hotkeys fail to register
            // (`--unregistered`, default 0,3: Ctrl/Shift + Print Screen).
            if unregistered.is_empty() {
                unregistered = vec![0, 3];
            }
            let failed = crate::hotkeys::failed_hotkeys(&default_hotkeys(&unregistered)).ok_or("no failed hotkeys")?;
            let mut dialog = MessageBox::ok(failed.caption, failed.text, MessageBoxIcon::Warning);
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
        "file-delete-confirm" => {
            // `TaskHelpers.ConfirmFileDelete`'s message box.
            let ss = sharex_strings::sharex::MainForm_tsmiDeleteSelectedFile_Click_File_delete_confirmation;
            let mut dialog = MessageBox::yes_no_with_check_box(
                format!("ShareXrs - {}", ss()),
                sharex_strings::sharex::MainForm_tsmiDeleteSelectedFile_Click_Do_you_really_want_to_delete_this_file_(),
                sharex_strings::sharex::FileDeleteConfirmation_DontRemindAgain(),
            );
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
