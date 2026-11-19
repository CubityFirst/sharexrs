//! Borderless window: `BorderlessWindowSettings` (in ShareX's JSON),
//! `BorderlessWindowManager` (which windows were made borderless and how to
//! restore them, the styles it removes) and `BorderlessWindowViewModel`'s
//! toggle. The window operations themselves go through [`WindowOps`].

use serde_json::{json, Value};

use super::inspect_window::{styles, Rect};
use crate::options::{json_bool, json_str_or};

/// `BorderlessWindowSettings` (`TaskSettingsTools.BorderlessWindowSettings`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BorderlessWindowSettings {
    pub remember_window_title: bool,
    pub window_title: String,
    pub auto_close_window: bool,
    pub exclude_taskbar_area: bool,
}

impl Default for BorderlessWindowSettings {
    fn default() -> Self {
        BorderlessWindowSettings {
            remember_window_title: true,
            window_title: String::new(),
            auto_close_window: false,
            exclude_taskbar_area: false,
        }
    }
}

impl BorderlessWindowSettings {
    pub fn from_json(v: &Value) -> Self {
        let d = Self::default();
        let b = |k: &str, d: bool| v.get(k).and_then(json_bool).unwrap_or(d);
        BorderlessWindowSettings {
            remember_window_title: b("RememberWindowTitle", d.remember_window_title),
            window_title: v.get("WindowTitle").map_or(d.window_title.clone(), |x| json_str_or(x, "")),
            auto_close_window: b("AutoCloseWindow", d.auto_close_window),
            exclude_taskbar_area: b("ExcludeTaskbarArea", d.exclude_taskbar_area),
        }
    }

    /// In ShareX's property order.
    pub fn to_json(&self) -> Value {
        json!({
            "RememberWindowTitle": self.remember_window_title,
            "WindowTitle": self.window_title,
            "AutoCloseWindow": self.auto_close_window,
            "ExcludeTaskbarArea": self.exclude_taskbar_area,
        })
    }

    /// The window's title box at open: the remembered title, or empty.
    pub fn initial_title(&self) -> &str {
        if self.remember_window_title { &self.window_title } else { "" }
    }
}

/// The styles `MakeWindowBorderless` removes.
pub const REMOVED_STYLES: u32 = styles::WS_CAPTION | styles::WS_MAXIMIZEBOX | styles::WS_SYSMENU | styles::WS_THICKFRAME;
/// The extended styles it removes.
pub const REMOVED_EX_STYLES: u32 = styles::WS_EX_CLIENTEDGE | styles::WS_EX_DLGMODALFRAME | styles::WS_EX_STATICEDGE;

/// What a window looked like before it was made borderless
/// (`BorderlessWindowInfo`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SavedWindow {
    pub rectangle: Rect,
    pub style: u32,
    pub ex_style: u32,
}

/// The native calls the manager makes.
pub trait WindowOps {
    /// `NativeMethods.SearchWindow`: `FindWindow` by exact title, else the
    /// first process whose main window title contains it (invariant,
    /// ignoring case).
    fn search_window(&self, title: &str) -> Option<isize>;
    fn is_minimized(&self, handle: isize) -> bool;
    fn restore(&mut self, handle: isize);
    /// `GetWindowRect`.
    fn window_rect(&self, handle: isize) -> Rect;
    fn style(&self, handle: isize) -> u32;
    fn ex_style(&self, handle: isize) -> u32;
    fn set_style(&mut self, handle: isize, style: u32);
    fn set_ex_style(&mut self, handle: isize, style: u32);
    /// `CaptureHelpers.GetScreenBounds(handle, useWorkingArea)`: the
    /// window's monitor (or its working area); empty when there's none.
    fn screen_bounds(&self, handle: isize, working_area: bool) -> Rect;
    /// `SetWindowPos` with `SWP_FRAMECHANGED | SWP_NOOWNERZORDER |
    /// SWP_NOZORDER`, plus `SWP_NOMOVE | SWP_NOSIZE` when `move_and_size`
    /// is false.
    fn set_window_pos(&mut self, handle: isize, rect: Rect, move_and_size: bool);
}

/// `BorderlessWindowManager`: the windows made borderless, kept for the
/// session (a toggle restores them).
#[derive(Debug, Default, Clone)]
pub struct BorderlessWindowManager {
    windows: std::collections::HashMap<isize, SavedWindow>,
}

impl BorderlessWindowManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_borderless(&self, handle: isize) -> bool {
        self.windows.contains_key(&handle)
    }

    /// `ToggleBorderlessWindow(windowTitle, useWorkingArea)`: false when the
    /// title is blank or no window has it.
    pub fn toggle_by_title(&mut self, ops: &mut dyn WindowOps, title: &str, working_area: bool) -> bool {
        if title.trim().is_empty() {
            return false;
        }
        match ops.search_window(title) {
            Some(h) if h != 0 => {
                self.toggle(ops, h, working_area);
                true
            }
            _ => false,
        }
    }

    /// `ToggleBorderlessWindow(handle, useWorkingArea)`.
    pub fn toggle(&mut self, ops: &mut dyn WindowOps, handle: isize, working_area: bool) {
        match self.windows.get(&handle).copied() {
            Some(saved) => {
                if ops.is_minimized(handle) {
                    ops.restore(handle);
                }
                ops.set_style(handle, saved.style);
                ops.set_ex_style(handle, saved.ex_style);
                ops.set_window_pos(handle, saved.rectangle, true);
                self.windows.remove(&handle);
            }
            None => {
                if ops.is_minimized(handle) {
                    ops.restore(handle);
                }
                let saved = SavedWindow { rectangle: ops.window_rect(handle), style: ops.style(handle), ex_style: ops.ex_style(handle) };
                let style = ops.style(handle) & !REMOVED_STYLES;
                ops.set_style(handle, style);
                let ex = ops.ex_style(handle) & !REMOVED_EX_STYLES;
                ops.set_ex_style(handle, ex);
                let bounds = ops.screen_bounds(handle, working_area);
                ops.set_window_pos(handle, bounds, !bounds.is_empty());
                self.windows.entry(handle).or_insert(saved);
            }
        }
    }
}

/// What the window does after Toggle (`BorderlessWindowViewModel.Toggle`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToggleOutcome {
    /// Nothing to do (a blank title).
    Ignored,
    /// `BorderlessWindowViewModel_Window_not_found`.
    NotFound(String),
    /// Done: settings saved, the action sound played; `close` when
    /// `AutoCloseWindow`.
    Done { close: bool },
}

/// `Toggle`: the trimmed title toggled, the title remembered (or cleared)
/// in `settings`.
pub fn toggle(
    manager: &mut BorderlessWindowManager,
    ops: &mut dyn WindowOps,
    settings: &mut BorderlessWindowSettings,
    window_title: &str,
) -> ToggleOutcome {
    let title = window_title.trim();
    if title.is_empty() {
        return ToggleOutcome::Ignored;
    }
    if !manager.toggle_by_title(ops, title, settings.exclude_taskbar_area) {
        return ToggleOutcome::NotFound(sharex_strings::tools::BorderlessWindowViewModel_Window_not_found().into());
    }
    settings.window_title = if settings.remember_window_title { title.to_string() } else { String::new() };
    ToggleOutcome::Done { close: settings.auto_close_window }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Fake {
        style: u32,
        ex: u32,
        minimized: bool,
        pos: Vec<(Rect, bool)>,
    }

    impl WindowOps for Fake {
        fn search_window(&self, title: &str) -> Option<isize> {
            (title == "Game").then_some(7)
        }
        fn is_minimized(&self, _: isize) -> bool {
            self.minimized
        }
        fn restore(&mut self, _: isize) {
            self.minimized = false;
        }
        fn window_rect(&self, _: isize) -> Rect {
            Rect { x: 10, y: 20, width: 800, height: 600 }
        }
        fn style(&self, _: isize) -> u32 {
            self.style
        }
        fn ex_style(&self, _: isize) -> u32 {
            self.ex
        }
        fn set_style(&mut self, _: isize, s: u32) {
            self.style = s;
        }
        fn set_ex_style(&mut self, _: isize, s: u32) {
            self.ex = s;
        }
        fn screen_bounds(&self, _: isize, wa: bool) -> Rect {
            Rect { x: 0, y: 0, width: 1920, height: if wa { 1040 } else { 1080 } }
        }
        fn set_window_pos(&mut self, _: isize, r: Rect, m: bool) {
            self.pos.push((r, m));
        }
    }

    #[test]
    fn toggles_and_restores() {
        let mut ops = Fake { style: 0x16CF_0000, ex: 0x0000_0300, minimized: true, ..Default::default() };
        let mut m = BorderlessWindowManager::new();
        let mut s = BorderlessWindowSettings { exclude_taskbar_area: true, ..Default::default() };
        assert_eq!(toggle(&mut m, &mut ops, &mut s, "  "), ToggleOutcome::Ignored);
        assert!(matches!(toggle(&mut m, &mut ops, &mut s, "Nope"), ToggleOutcome::NotFound(_)));
        assert_eq!(toggle(&mut m, &mut ops, &mut s, " Game "), ToggleOutcome::Done { close: false });
        assert_eq!(s.window_title, "Game");
        assert_eq!((ops.style, ops.ex, ops.minimized), (0x1602_0000, 0x0000_0100, false));
        assert_eq!(ops.pos[0], (Rect { x: 0, y: 0, width: 1920, height: 1040 }, true));
        assert!(m.is_borderless(7));
        toggle(&mut m, &mut ops, &mut s, "Game");
        assert_eq!((ops.style, ops.ex), (0x16CF_0000, 0x0000_0300));
        assert_eq!(ops.pos[1], (Rect { x: 10, y: 20, width: 800, height: 600 }, true));
        assert!(!m.is_borderless(7));
    }

    #[test]
    fn settings_json() {
        let s = BorderlessWindowSettings::from_json(&json!({"RememberWindowTitle": false, "WindowTitle": "x", "AutoCloseWindow": "true"}));
        assert_eq!(s.initial_title(), "");
        assert!(s.auto_close_window);
        assert_eq!(BorderlessWindowSettings::default().to_json().to_string(), r#"{"RememberWindowTitle":true,"WindowTitle":"","AutoCloseWindow":false,"ExcludeTaskbarArea":false}"#);
    }
}
