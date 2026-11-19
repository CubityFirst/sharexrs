//! `sharexrs --render <screen>` for the System tool windows, in the states
//! of `tools-system` in the reference shot manifest. Every render runs on
//! synthetic sources: no clipboard is read, no other window is listed or
//! changed, no echo is sent.
//!
//! - `clipboard-viewer` (`--content empty|unicode|raw-format|file-drop|image|cleared`,
//!   `--text` the text or the raw bytes as hex, `--files` the file drop,
//!   `--image` the PNG an image clipboard holds),
//! - `inspect-window` (`--state empty|selected|details-scrolled|window-menu|pick-window|pick-control`),
//! - `monitor-test` (`--state grayscale|rgb|gradient|pattern|motion|controls-hidden|rgb-picker|gradient-direction|pattern-menu|motion-direction`),
//! - `network-monitor` (`--state idle|chart|outage|events|target-menu|interval-menu|range-menu`),
//! - `borderless-window` (`--state empty|window-menu|selected|toggled|not-found`).

use std::sync::Arc;

use chrono::{Local, TimeZone};
use sharex_settings::TaskSettings;
use sharex_tools::system::clipboard_viewer::{self as cv, ClipboardSnapshot, ClipboardValue};
use sharex_tools::system::inspect_window::{Rect as WRect, WindowEntry, WindowSnapshot};
use sharex_tools::system::network_monitor as nm;

use super::native::{ClipboardSource, ListedWindow, WindowSource};
use super::{borderless_window, clipboard_viewer, inspect_window, monitor_test, network_monitor};
use crate::data_tools::ComboPopupShot;
use crate::dialogs::Dialog;
use crate::dialogs::render::Options;

pub fn is_render_screen(name: &str) -> bool {
    matches!(name, "clipboard-viewer" | "inspect-window" | "monitor-test" | "network-monitor" | "borderless-window" | "inspect-picker")
}

/// A clipboard in memory.
struct FixedClipboard(ClipboardSnapshot);

impl ClipboardSource for FixedClipboard {
    fn capture(&self) -> Result<ClipboardSnapshot, String> {
        Ok(self.0.clone())
    }
    fn clear(&self) {}
}

/// Windows that don't exist: a test pattern, ShareXrs and a text editor.
struct SyntheticWindows;

fn synthetic_entries() -> Vec<WindowEntry> {
    let e = |handle, title: &str, class: &str, process: &str| WindowEntry {
        handle,
        title: title.into(),
        class_name: class.into(),
        process_name: process.into(),
        visible: true,
        cloaked: false,
        rectangle: WRect { x: 100, y: 80, width: 640, height: 400 },
    };
    vec![
        e(0x0001_02A4, "ShareXrs test pattern", "Window Class", "sharexrs"),
        e(0x0001_0358, "Notes.txt - Editor", "EditorWindow", "editor"),
        e(0x0002_01F0, "ShareXrs", "Window Class", "sharexrs"),
    ]
}

/// The test pattern window as `UpdateWindowInfo` reads it.
pub fn synthetic_snapshot() -> WindowSnapshot {
    WindowSnapshot {
        handle: 0x0001_02A4,
        title: "ShareXrs test pattern".into(),
        class_name: "Window Class".into(),
        process_name: "sharexrs".into(),
        process_file_name: "sharexrs.exe".into(),
        process_id: "4242".into(),
        rectangle: WRect { x: 100, y: 80, width: 640, height: 400 },
        client_rectangle: WRect { x: 108, y: 111, width: 624, height: 361 },
        style: Some(0x16CF_0000),
        ex_style: Some(0x0000_0100),
        top_most: false,
        opacity: 255,
    }
}

impl WindowSource for SyntheticWindows {
    fn visible_windows(&self, ignored: isize) -> Vec<ListedWindow> {
        sharex_tools::system::inspect_window::visible_windows(synthetic_entries(), ignored).into_iter().map(|entry| ListedWindow { entry, icon: None }).collect()
    }
    fn snapshot(&self, handle: isize) -> Option<WindowSnapshot> {
        (handle == 0x0001_02A4).then(synthetic_snapshot)
    }
    fn icon(&self, _: isize) -> Option<image::RgbaImage> {
        None
    }
    fn window_at_point(&self, _: i32, _: i32, _: bool) -> isize {
        0x0001_02A4
    }
    fn set_top_most(&self, _: isize, _: bool) {}
    fn set_opacity(&self, _: isize, _: u8) {}
}

/// Window calls that change nothing: the synthetic test pattern window.
struct SyntheticOps;

impl sharex_tools::system::borderless_window::WindowOps for SyntheticOps {
    fn search_window(&self, title: &str) -> Option<isize> {
        (title == "ShareXrs test pattern").then_some(0x0001_02A4)
    }
    fn is_minimized(&self, _: isize) -> bool {
        false
    }
    fn restore(&mut self, _: isize) {}
    fn window_rect(&self, _: isize) -> WRect {
        WRect { x: 100, y: 80, width: 640, height: 400 }
    }
    fn style(&self, _: isize) -> u32 {
        0x16CF_0000
    }
    fn ex_style(&self, _: isize) -> u32 {
        0
    }
    fn set_style(&mut self, _: isize, _: u32) {}
    fn set_ex_style(&mut self, _: isize, _: u32) {}
    fn screen_bounds(&self, _: isize, _: bool) -> WRect {
        WRect { x: 0, y: 0, width: 1024, height: 768 }
    }
    fn set_window_pos(&mut self, _: isize, _: WRect, _: bool) {}
}

fn hex_bytes(text: &str) -> Vec<u8> {
    let digits: Vec<u8> = text.bytes().filter(u8::is_ascii_hexdigit).collect();
    digits.chunks(2).filter_map(|c| u8::from_str_radix(std::str::from_utf8(c).ok()?, 16).ok()).collect()
}

pub fn render_dialog(name: &str, o: &Options) -> Result<Box<dyn Dialog>, String> {
    let state = o.state.as_deref().unwrap_or("");
    let defaults = TaskSettings::default();
    let task = TaskSettings::default_task_settings(&defaults);
    Ok(match name {
        "clipboard-viewer" => {
            let mut snap = ClipboardSnapshot::new();
            match o.content.as_deref().unwrap_or("empty") {
                "empty" | "cleared" => {}
                "unicode" => snap.set(cv::FORMAT_UNICODE_TEXT, ClipboardValue::Text(o.text.clone().unwrap_or_default())),
                "raw-format" => snap.set("ReferenceFixtureBytes", ClipboardValue::Bytes(hex_bytes(o.text.as_deref().unwrap_or("")))),
                // A file drop set by a .NET program: Avalonia lists the
                // data object's three formats.
                "file-drop" => {
                    let files: Vec<String> = o.files.iter().map(|f| f.to_string_lossy().into_owned()).collect();
                    snap.set(cv::FORMAT_FILE_DROP, ClipboardValue::Files(files.clone()));
                    let wide: Vec<u8> = files.join("\0").encode_utf16().chain([0, 0]).flat_map(u16::to_le_bytes).collect();
                    snap.set("FileNameW", ClipboardValue::Bytes(wide));
                    let narrow: Vec<u8> = files.first().map(|f| f.bytes().chain([0]).collect()).unwrap_or_default();
                    snap.set("FileName", ClipboardValue::Bytes(narrow));
                }
                // `Clipboard.SetImage` from .NET: its serialised bitmap
                // (stood in for by the PNG's bytes), `CF_BITMAP` and Avalonia's `Bitmap`.
                "image" => {
                    let path = o.image.as_ref().ok_or("--image <png> is required")?;
                    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
                    snap.set("System.Drawing.Bitmap", ClipboardValue::Bytes(bytes.clone()));
                    snap.set("CF_BITMAP", ClipboardValue::Bytes(bytes.clone()));
                    snap.set(cv::FORMAT_BITMAP, ClipboardValue::Bytes(bytes));
                }
                other => return Err(format!("no clipboard content {other}")),
            }
            let mut w = clipboard_viewer::ClipboardViewerWindow::new(Box::new(FixedClipboard(snap)));
            if let Some(&i) = o.select.first() {
                let f = w.formats.get(i).cloned();
                w.select(f);
            }
            Box::new(w)
        }
        "inspect-window" => {
            if state == "window-menu" {
                let mut list = super::WindowList::default();
                list.set(SyntheticWindows.visible_windows(0));
                let items = list.items(&egui::Context::default(), 20.0);
                let mut shot = ComboPopupShot::new(items, None, None);
                shot.placeholder = Some(sharex_strings::tools::InspectWindowWindow_Select_window());
                return Ok(Box::new(shot));
            }
            let mut w = inspect_window::InspectWindowWindow::new(Box::new(SyntheticWindows));
            w.set_offline();
            match state {
                "" | "empty" => {}
                "selected" | "details-scrolled" | "opacity-pressed" => w.show_snapshot(&synthetic_snapshot(), true),
                "control" => w.show_snapshot(&synthetic_snapshot(), false),
                other => return Err(format!("no inspect-window state {other}")),
            }
            Box::new(w)
        }
        "inspect-picker" => {
            // `--state window|control`: the overlay alone, on black.
            let bounds = sharex_capture::Rect::new(0, 0, o.width.unwrap_or(1024) as i32, o.height.unwrap_or(768) as i32);
            Box::new(inspect_window::PickerOverlay { owner: None, top_level: state != "control", bounds, scale: 1.0 })
        }
        "monitor-test" => {
            let bounds = sharex_capture::Rect::new(0, 0, o.width.unwrap_or(1024) as i32, o.height.unwrap_or(768) as i32);
            let mut w = monitor_test::MonitorTestWindow::on(bounds, 1.0);
            w.frozen = true;
            // `--count`: how far the moving bars had gone (px).
            w.motion_offset = o.count as f64;
            use sharex_tools::monitor_test::Mode;
            let popup = |items, sel| -> Box<dyn Dialog> { Box::new(ComboPopupShot::new(items, sel, None)) };
            match state {
                "" | "grayscale" => {}
                "rgb" | "rgb-picker" => w.test.mode = Mode::Rgb,
                "gradient" => w.test.mode = Mode::Gradient,
                "pattern" => w.test.mode = Mode::Pattern,
                "motion" => w.test.mode = Mode::Motion,
                "controls-hidden" => w.controls_visible = false,
                "gradient-direction" => return Ok(popup(monitor_test::direction_items(), monitor_test::DIRECTIONS.iter().position(|d| *d == w.test.gradient_direction))),
                "pattern-menu" => return Ok(popup(monitor_test::pattern_items(), monitor_test::PATTERNS.iter().position(|p| *p == w.test.pattern))),
                "motion-direction" => return Ok(popup(monitor_test::motion_items(), monitor_test::MOTIONS.iter().position(|m| *m == w.test.motion_direction))),
                other => return Err(format!("no monitor-test state {other}")),
            }
            Box::new(w)
        }
        "network-monitor" => {
            let popup = |items: Vec<String>, sel: usize, width: f32| -> Box<dyn Dialog> {
                Box::new(ComboPopupShot::new(items.into_iter().map(sharex_ui::controls::ComboItem::normal).collect(), Some(sel), Some(width)))
            };
            match state {
                "target-menu" => return Ok(popup(nm::targets().into_iter().map(|t| t.display_name).collect(), 0, 235.0)),
                "interval-menu" => return Ok(popup(nm::INTERVALS.iter().map(|(n, _)| n.to_string()).collect(), nm::DEFAULT_INTERVAL, 78.0)),
                "range-menu" => return Ok(popup(nm::time_ranges().into_iter().map(|(n, _)| n).collect(), 0, 150.0)),
                _ => {}
            }
            let never: network_monitor::Prober = Arc::new(|t: &nm::Target| nm::probe_result(Local::now(), t, false, &[]));
            let mut w = network_monitor::NetworkMonitorWindow::new(never, None);
            w.set_offline();
            // A fixed clock: 2026-11-19 12:00:00 local.
            let now = Local.with_ymd_and_hms(2026, 11, 19, 12, 0, 0).single().ok_or("no such local time")?;
            w.now = Some(now);
            let target = nm::targets().remove(0);
            let result = |secs_ago: i64, latency: Option<f64>| {
                let replies: Vec<nm::EchoReply> = match latency {
                    Some(l) => vec![nm::EchoReply::Success { latency_ms: l }; 3],
                    None => vec![nm::EchoReply::TimedOut; 3],
                };
                nm::probe_result(now - chrono::Duration::seconds(secs_ago), &target, true, &replies)
            };
            let feed = |w: &mut network_monitor::NetworkMonitorWindow, outage: bool| {
                // Four minutes of samples every two seconds, 12-30 ms, an
                // outage of twenty seconds a minute ago.
                for k in (0..120).rev() {
                    let secs = k * 2;
                    let down = outage && (60..80).contains(&secs);
                    let latency = (!down).then(|| 12.0 + ((k * 37) % 19) as f64);
                    w.take_result(result(secs, latency));
                    if down {
                        w.take_result(result(secs, None));
                    }
                }
                w.monitor.monitoring = true;
            };
            match state {
                "" | "idle" => w.monitor.monitoring = true,
                "chart" => feed(&mut w, false),
                "outage" | "events" => feed(&mut w, true),
                other => return Err(format!("no network-monitor state {other}")),
            }
            Box::new(w)
        }
        "borderless-window" => {
            if state == "window-menu" {
                let mut list = super::WindowList::default();
                list.set(SyntheticWindows.visible_windows(0));
                let items = list.items(&egui::Context::default(), 16.0);
                let mut shot = ComboPopupShot::new(items, None, None);
                shot.placeholder = Some(sharex_strings::tools::BorderlessWindowWindow_Select_window());
                return Ok(Box::new(shot));
            }
            let mut w = borderless_window::BorderlessWindowWindow::new(task, &defaults, Box::new(SyntheticWindows), Box::new(SyntheticOps));
            w.set_offline();
            w.reload();
            match state {
                "" | "empty" => {}
                "selected" => {
                    w.selected = Some(0);
                    w.window_title = "ShareXrs test pattern".into();
                }
                "toggled" | "restored" => {
                    w.window_title = "ShareXrs test pattern".into();
                    w.toggle();
                    if state == "restored" {
                        w.toggle();
                    }
                }
                "not-found" => {
                    w.window_title = "No such window".into();
                    w.toggle();
                }
                other => return Err(format!("no borderless-window state {other}")),
            }
            Box::new(w)
        }
        other => return Err(format!("no system tool window {other}")),
    })
}
