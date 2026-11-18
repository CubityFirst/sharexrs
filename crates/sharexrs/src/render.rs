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
//! `tray-menu` draws the tray menu (`BuildTrayMenu`) as ShareX opens it
//! with the pointer at `--at X,Y` (default 682,563) on a `--width` x
//! `--height` work area (default 1024x720), cropped to one menu level
//! (`--level N`, 0 the root): each is its own popup window in ShareX, which
//! `winshot.ps1` grabs on black where the corners are rounded off. `--open`
//! and `--hover` as above.
//! `main-tasks` is the main window with tasks: `--tasks a.png,b.png,...`
//! (finished region captures of these files, oldest first, so the last is
//! shown first), `--list` for the list view, `--select 0,2` (by position
//! shown), `--context X,Y` (a right click there, which selects and opens
//! the context menu; `--open` then opens its submenus), `--hover X,Y`.
//!
//! `file-delete-confirm` is the "File delete confirmation" message box.
//!
//! `start-screen` is the Start screen of a fresh profile (`--height` caps
//! it as a short screen does, `--scroll N` scrolls it); `settings-<page>`
//! is Application settings on a page (`general`, `theme`, `integration`,
//! `paths`, `settings`, `main-window`, `clipboard-formats`, `upload`,
//! `history`, `print`, `proxy`, `advanced`). Both take `--accent #RRGGBB`,
//! `--legacy` (icon pack) and `--compact`, and `--click X,Y[;X,Y]` (clicks
//! before drawing, to open a dropdown).
//!
//! `controls-gallery` is every shared control in every state (see
//! `controls_gallery`), for comparing with the Avalonia.Headless probe's.
//! `hotkeys` is Hotkey settings with ShareX's default hotkeys
//! (`--unregistered 0,3` failed, `--select N`, `--recording N` recording
//! hotkey N, `--none 1` hotkey 1 cleared, `--disabled` with hotkeys disabled, `--reset` the reset
//! confirmation, `--empty` without hotkeys; `--click`/`--hover` as above).
//! `task-settings-<page>` is Task settings on a page by its id (`task`,
//! `general`, `general-notifications`, `image`, `capture-region`, ...): the
//! default task settings, or `--hotkey` the first default hotkey's. As the
//! window does when it opens, the actions get the image editors ShareX
//! finds installed (`AddDefaultExternalPrograms`, the registry on Windows).
//! `--content sample` adds two made-up actions and a watch folder;
//! `action-editor` (Add action), `action-editor-edit` (the first sample
//! action), `action-editor-error` (Save with no name),
//! `watch-folder-editor` and `watch-folder-editor-edit` open the editors
//! (with the samples).
//!
//! `ffmpeg-options` is the FFmpeg options window on a page (`--content
//! sources|video|audio|advanced`; `--text` is a custom FFmpeg path, whose
//! DirectShow devices are listed). `recording-toolbar` is the recording
//! frame's toolbar (`--content` waiting, before, working, recording,
//! paused, confirm; `-labels` with button labels, `-notimer` without the
//! timer; `--text` the timer's text). `recorder-bar` is the recorder bar
//! (`--content` default, labels, options, options-bar, gif). Both draw on
//! grey (#808080).
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

/// A fresh profile's settings with these theme options, applied
/// (`ThemeManager`: accent, icon pack, spacing).
fn theme_settings(dark: bool, accent: Option<&str>, legacy: bool, compact: bool) -> sharex_settings::ApplicationConfig {
    let mut settings = sharex_settings::ApplicationConfig::default();
    let o = settings.theme_options();
    o.Theme = if dark { "Dark" } else { "Light" }.into();
    if let Some(a) = accent {
        o.AccentColorHex = a.into();
    }
    if legacy {
        o.IconPack = sharex_settings::IconPack::Legacy;
    }
    o.CompactMode = compact;
    crate::appearance::apply(&settings, None);
    settings
}

/// Runs `ui` for enough frames to settle, with the pointer at `hover` and a
/// click at each of `clicks` (one every few frames), then draws it.
pub(crate) fn render_frames(screen: &mut Offscreen, hover: Option<egui::Pos2>, clicks: &[egui::Pos2], ui: &mut dyn FnMut(&mut egui::Ui)) -> image::RgbaImage {
    let button = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    let mut frame = 0;
    let mut run = |screen: &mut Offscreen, events: Vec<egui::Event>| {
        screen.run(events, &mut |u: &mut egui::Ui| ui(u));
    };
    run(screen, Vec::new());
    run(screen, Vec::new());
    for &c in clicks {
        run(screen, vec![egui::Event::PointerMoved(c)]);
        run(screen, vec![button(c, true)]);
        run(screen, vec![button(c, false)]);
        run(screen, Vec::new());
        frame += 4;
    }
    let _ = frame;
    if let Some(h) = hover {
        for _ in 0..4 {
            run(screen, vec![egui::Event::PointerMoved(h)]);
        }
    } else {
        run(screen, vec![egui::Event::PointerGone]);
    }
    for _ in 0..3 {
        run(screen, Vec::new());
    }
    screen.draw()
}

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
    let mut at = egui::pos2(682.0, 563.0);
    let mut level = 0usize;
    let mut task_files: Vec<std::path::PathBuf> = Vec::new();
    let mut list_view = false;
    let mut select: Vec<usize> = Vec::new();
    let mut context = None;
    let mut scroll = 0.0f32;
    // Theme options for the Start screen and settings screens.
    let mut accent: Option<String> = None;
    let mut legacy = false;
    let mut compact = false;
    let mut click: Vec<egui::Pos2> = Vec::new();
    // Hotkey and Task settings.
    let mut recording = None;
    let mut cleared: Vec<usize> = Vec::new();
    let (mut hk_disabled, mut hk_reset, mut hk_empty, mut for_hotkey) = (false, false, false, false);
    let mut dialog = crate::dialogs::render::Options::default();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut val = || it.next().cloned().ok_or(format!("{a} needs a value"));
        match a.as_str() {
            "--pointer" | "--drag" | "--window" | "--steps" => rest_args.push((a.clone(), val()?)),
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
                let raw = val()?;
                rest_args.push((a.clone(), raw.clone()));
                let v: Vec<f32> = raw.split(',').filter_map(|i| i.trim().parse().ok()).collect();
                let [l, t, r, b] = v[..] else { return Err("--work-area L,T,R,B".into()) };
                work_area = Some(egui::Rect::from_min_max(egui::pos2(l, t), egui::pos2(r, b)));
            }
            "--monitors" => {
                for m in val()?.split(',') {
                    let (w, h) = m.split_once('x').ok_or("--monitors WxH,...")?;
                    monitors.push((0, 0, w.parse().map_err(|_| "--monitors WxH")?, h.parse().map_err(|_| "--monitors WxH")?));
                }
            }
            "--at" => {
                let v = val()?;
                let (x, y) = v.split_once(',').ok_or("--at X,Y")?;
                at = egui::pos2(x.parse().map_err(|_| "--at X,Y")?, y.parse().map_err(|_| "--at X,Y")?);
            }
            "--level" => level = val()?.parse().map_err(|_| "--level N")?,
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
            "--accent" => accent = Some(val()?),
            "--legacy" => legacy = true,
            "--compact" => compact = true,
            "--scroll" => scroll = val()?.parse().map_err(|_| "--scroll N")?,
            "--click" => {
                for c in val()?.split(';') {
                    let (x, y) = c.split_once(',').ok_or("--click X,Y[;X,Y]")?;
                    click.push(egui::pos2(x.parse().map_err(|_| "--click X,Y")?, y.parse().map_err(|_| "--click X,Y")?));
                }
            }
            "--recording" => recording = Some(val()?.parse::<usize>().map_err(|_| "--recording N")?),
            "--disabled" => hk_disabled = true,
            "--none" => cleared = val()?.split(',').filter_map(|i| i.trim().parse().ok()).collect(),
            "--reset" => hk_reset = true,
            "--empty" => hk_empty = true,
            "--hotkey" => for_hotkey = true,
            "--text" => dialog.text = Some(val()?),
            "--content" => dialog.content = Some(val()?),
            "--image" => dialog.image = Some(val()?.into()),
            "--files" => dialog.files = val()?.split(',').map(std::path::PathBuf::from).collect(),
            "--count" => dialog.count = val()?.parse().map_err(|_| "--count N")?,
            "--viewer-only" => dialog.viewer_only = true,
            "--db" => dialog.db = Some(val()?.into()),
            "--state" => dialog.state = Some(val()?),
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
        s if crate::dialogs::render::is_dialog_screen(s) => {
            crate::appearance::apply(&theme_settings(dark, accent.as_deref(), legacy, compact), None);
            dialog.dark = dark;
            dialog.scale = scale;
            dialog.hover = hover;
            dialog.clicks = click.clone();
            dialog.width = width;
            dialog.height = height;
            dialog.select = select.clone();
            dialog.context = context;
            dialog.open = open.clone();
            dialog.scroll = scroll;
            crate::dialogs::render::render(s, &dialog)?
        }
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
        "tray-menu" => {
            let (w, h) = (width.unwrap_or(1024), height.unwrap_or(720));
            let mut screen = Offscreen::new([w, h], scale, dark)?;
            screen.transparent = true;
            let state = crate::main_menu::MenuState { monitors: monitors.clone(), ..Default::default() };
            let anchor = egui::Rect::from_min_size((at.to_vec2() / scale).to_pos2(), egui::vec2(32.0, 39.0) / scale);
            let mut flyout = sharex_ui::menu::Flyout::menu(
                egui::Id::new("tray-menu"),
                crate::main_menu::tray_menu(&state),
                anchor,
                sharex_ui::menu::Placement::BottomEdgeAlignedLeft,
            );
            flyout.edit_label = sharex_strings::sharex::MainMenuBuilder_EditTools().to_string();
            flyout.open_path(&open);
            let events: Vec<egui::Event> = hover.map(egui::Event::PointerMoved).into_iter().collect();
            for frame in 0..48 {
                let ev = if frame >= 1 { events.clone() } else { Vec::new() };
                screen.run(ev, &mut |ui: &mut egui::Ui| {
                    let bounds = ui.max_rect();
                    flyout.show(ui, bounds);
                });
            }
            let full = screen.draw();
            let r = *flyout.rects().get(level).ok_or("no such menu level")?;
            let (x, y) = ((r.min.x * scale).round() as u32, (r.min.y * scale).round() as u32);
            let (cw, ch) = ((r.width() * scale).round() as u32, (r.height() * scale).round() as u32);
            // Where it is on the work area, for checking placement.
            println!("tray-menu level {level} at {x},{y} {cw}x{ch}");
            let mut crop = image::imageops::crop_imm(&full, x, y, cw, ch).to_image();
            // On black, as a popup window's transparent corners come out.
            for p in crop.pixels_mut() {
                let a = p[3] as u32;
                let c = |v: u8| ((v as u32 * a + 127) / 255) as u8;
                *p = image::Rgba([c(p[0]), c(p[1]), c(p[2]), 255]);
            }
            crop
        }
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
        "start-screen" => {
            // The Start screen of a fresh profile; `--height` caps the window
            // as `FitToWorkingArea` does, `--scroll` scrolls its content.
            let mut settings = theme_settings(dark, accent.as_deref(), legacy, compact);
            let mut state = crate::start_screen::StartScreen::default();
            let mut probe = Offscreen::new([1, 1], scale, dark)?;
            let mut h = 0.0;
            probe.run(Vec::new(), &mut |ui: &mut egui::Ui| h = state.content_height(ui, &settings));
            let full = (h * scale).round() as u32;
            let h = height.map_or(full, |cap| cap.min(full));
            let w = width.unwrap_or((crate::start_screen::WIDTH * scale) as u32);
            let mut screen = Offscreen::new([w, h], scale, dark)?;
            state.scroll = scroll;
            render_frames(&mut screen, hover, &click, &mut |ui| {
                state.ui(ui, &mut settings);
            })
        }
        s if s.starts_with("settings-") => {
            // Application settings on a page (`settings-general`,
            // `settings-main-window`, ...), with a fresh profile's settings.
            let name = &s["settings-".len()..];
            let page = crate::app_settings::PAGES.iter().position(|p| *p == name).ok_or(format!("no settings page {name}"))?;
            let mut settings = theme_settings(dark, accent.as_deref(), legacy, compact);
            let parent = sharex_settings::paths::absolute_path("ShareXrs").join("Screenshots");
            let mut state = crate::app_settings::ApplicationSettings::new(&settings, true, parent);
            state.page = page;
            let w = width.unwrap_or((sharex_ui::theme::density(&egui::Context::default()).settings_window_width * scale) as u32);
            let h = height.unwrap_or((crate::app_settings::HEIGHT * scale) as u32);
            let mut screen = Offscreen::new([w, h], scale, dark)?;
            render_frames(&mut screen, hover, &click, &mut |ui| {
                state.ui(ui, &mut settings);
            })
        }
        "controls-gallery" => crate::controls_gallery::render(dark)?,
        "ffmpeg-options" | "recording-toolbar" | "recorder-bar" => {
            crate::appearance::apply(&theme_settings(dark, accent.as_deref(), legacy, compact), None);
            let o = crate::recording::RenderOptions {
                content: dialog.content.clone().unwrap_or_default(),
                text: dialog.text.clone(),
                width,
                height,
                scale,
                hover,
                clicks: click.clone(),
            };
            crate::recording::render(&screen, dark, &o)?
        }
        "hotkeys" => {
            let mut settings = theme_settings(dark, accent.as_deref(), legacy, compact);
            settings.DisableHotkeys = hk_disabled;
            let mut hotkeys = if hk_empty { Vec::new() } else { default_hotkeys(&unregistered) };
            for &i in &cleared {
                hotkeys[i].HotkeyInfo = sharex_settings::HotkeyInfo::default();
            }
            let mut state = crate::hotkey_settings::HotkeySettingsWindow::default();
            state.selected = select.first().copied();
            state.reset_confirmation = hk_reset;
            if let Some(i) = recording {
                let mut svc = crate::hotkey_settings::Service { hotkeys: &mut hotkeys, manager: None, settings: &mut settings };
                state.toggle_capture(&mut svc, i);
                svc.hotkeys[i].HotkeyInfo.status = HotkeyStatus::NotConfigured;
            }
            let w = width.unwrap_or((crate::hotkey_settings::WIDTH * scale) as u32);
            let h = height.unwrap_or((crate::hotkey_settings::HEIGHT * scale) as u32);
            let mut screen = Offscreen::new([w, h], scale, dark)?;
            render_frames(&mut screen, hover, &click, &mut |ui| {
                if !open.is_empty() {
                    state.open_menu_path(&open);
                }
                let mut svc = crate::hotkey_settings::Service { hotkeys: &mut hotkeys, manager: None, settings: &mut settings };
                state.ui(ui, &mut svc);
            })
        }
        s if s.starts_with("task-settings-") => {
            let name = &s["task-settings-".len()..];
            let page = crate::task_settings_window::page_index(name).ok_or(format!("no task settings page {name}"))?;
            let mut settings = theme_settings(dark, accent.as_deref(), legacy, compact);
            let defaults = settings.DefaultTaskSettings.clone();
            let mut hotkeys = default_hotkeys(&[]);
            let mut state = if for_hotkey {
                crate::task_settings_window::TaskSettingsWindow::new(&mut hotkeys[0].TaskSettings, Some(&defaults))
            } else {
                crate::task_settings_window::TaskSettingsWindow::new(&mut settings.DefaultTaskSettings, None)
            };
            state.page = page;
            {
                let task = if for_hotkey { &mut hotkeys[0].TaskSettings } else { &mut settings.DefaultTaskSettings };
                sharex_settings::external_program::add_default_external_programs(task.ExternalPrograms.get_or_insert_default(), &crate::shell::search_program_path);
                let content = dialog.content.clone().unwrap_or_default();
                if !content.is_empty() {
                    task_settings_samples(task);
                }
                use crate::task_editors::{ActionEditor, WatchFolderEditor};
                let first = |t: &TaskSettings| t.ExternalPrograms.as_ref().and_then(|l| l.iter().position(|a| a.Name.as_deref() == Some("Optimize PNG")));
                match content.as_str() {
                    "action-editor" => state.action_editor = Some(ActionEditor::new(None, None)),
                    "action-editor-edit" => {
                        let i = first(task).unwrap_or(0);
                        state.action_selected = Some(i);
                        state.action_editor = Some(ActionEditor::new(Some(i), task.ExternalPrograms.as_ref().map(|l| &l[i])));
                    }
                    "action-editor-error" => {
                        let mut ed = ActionEditor::new(None, None);
                        let _ = ed.save(&mut Vec::new());
                        state.action_editor = Some(ed);
                    }
                    "watch-folder-editor" => state.watch_editor = Some(WatchFolderEditor::new(None, None)),
                    "watch-folder-editor-edit" => {
                        state.watch_selected = Some(0);
                        state.watch_editor = Some(WatchFolderEditor::new(Some(0), task.WatchFolderList.first()));
                    }
                    _ => {}
                }
            }
            let w = width.unwrap_or((sharex_ui::theme::density(&egui::Context::default()).settings_window_width * scale) as u32);
            let h = height.unwrap_or((crate::task_settings_window::HEIGHT * scale) as u32);
            let mut screen = Offscreen::new([w, h], scale, dark)?;
            render_frames(&mut screen, hover, &click, &mut |ui| {
                let s = &mut settings;
                let mut ctx = if for_hotkey {
                    crate::task_settings_window::TaskCtx { task: &mut hotkeys[0].TaskSettings, auto_increment: &mut s.NameParserAutoIncrementNumber }
                } else {
                    crate::task_settings_window::TaskCtx { task: &mut s.DefaultTaskSettings, auto_increment: &mut s.NameParserAutoIncrementNumber }
                };
                state.ui(ui, &mut ctx);
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

/// Made-up actions and a watch folder for `task-settings-*` `--content`
/// screens (the same values go into ShareX's for the live comparison).
fn task_settings_samples(task: &mut TaskSettings) {
    use sharex_settings::{ExternalProgram, WatchFolderSettings};
    let list = task.ExternalPrograms.get_or_insert_default();
    list.push(ExternalProgram {
        IsActive: true,
        Name: Some("Optimize PNG".into()),
        Path: Some(r"C:\Tools\optipng.exe".into()),
        Args: Some("-o7 %input".into()),
        Extensions: Some("png".into()),
        ..Default::default()
    });
    list.push(ExternalProgram {
        IsActive: false,
        Name: Some("Convert to WebP".into()),
        Path: Some(r"C:\Tools\cwebp.exe".into()),
        Args: Some("%input -o %output".into()),
        OutputExtension: Some("webp".into()),
        DeleteInputFile: true,
        ..Default::default()
    });
    task.WatchFolderList.push(WatchFolderSettings {
        FolderPath: Some(r"C:\Users\Public\Pictures".into()),
        Filter: Some("*.png".into()),
        IncludeSubdirectories: true,
        MoveFilesToScreenshotsFolder: false,
        ..Default::default()
    });
}
