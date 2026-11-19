//! `ShareX.Tools/Tools/BorderlessWindow` (`BorderlessWindowWindow.axaml`,
//! `BorderlessWindowViewModel`) with `TaskHelpers.OpenBorderlessWindow`'s
//! services: the window list (reloaded when it drops down), the title (or
//! one typed in), the options written into the task's
//! `BorderlessWindowSettings`, and Toggle borderless through the session's
//! `BorderlessWindowManager` (shared with the Make active window
//! borderless hotkey). `SizeToContent="Height"`: the error line comes and goes.

use std::sync::Mutex;

use egui::{Id, Rect, Ui, pos2, vec2};
use sharex_settings::TaskSettings;
use sharex_strings::tools as s;
use sharex_tools::system::borderless_window::{self as bw, BorderlessWindowManager, BorderlessWindowSettings, ToggleOutcome, WindowOps};
use sharex_ui::controls::{ComboBox, TextBox, ToggleSwitch};
use sharex_ui::keys::app_icons as icon;
use sharex_ui::theme::{Thickness, palette};
use sharex_ui::widgets::{center_y, font};

use super::WindowList;
use super::native::WindowSource;
use crate::dialogs::{Cx, Dialog, Effect, Spec};
use crate::image_tools as tools;

pub const WIDTH: f32 = 580.0;
const LABEL_W: f32 = 100.0;
const TOGGLE_ROW: f32 = 32.0;
/// The title box's `Height="44"`.
const TITLE_H: f32 = 44.0;

/// `BorderlessWindowManager`: static in ShareX, one for the session.
pub static MANAGER: Mutex<Option<BorderlessWindowManager>> = Mutex::new(None);

/// `BorderlessWindowManager.ToggleBorderlessWindow(handle, useWorkingArea)`
/// on the session's manager.
pub fn toggle_handle(ops: &mut dyn WindowOps, handle: isize, working_area: bool) {
    let mut m = MANAGER.lock().unwrap_or_else(|e| e.into_inner());
    m.get_or_insert_with(BorderlessWindowManager::new).toggle(ops, handle, working_area);
}

pub struct BorderlessWindowWindow {
    settings: BorderlessWindowSettings,
    task: TaskSettings,
    windows: Box<dyn WindowSource>,
    ops: Box<dyn WindowOps>,
    list: WindowList,
    /// `SelectedWindow`.
    pub selected: Option<usize>,
    pub window_title: String,
    pub error_message: String,
    /// The window's own handle, left out of the list (`SetIgnoredWindowHandle`).
    pub ignored: isize,
    opened: bool,
    settings_changed: bool,
    play_sound: bool,
}

impl BorderlessWindowWindow {
    pub fn new(task: TaskSettings, defaults: &TaskSettings, windows: Box<dyn WindowSource>, ops: Box<dyn WindowOps>) -> Self {
        let settings = BorderlessWindowSettings::from_json(&crate::data_tools::tools_settings(&task, defaults).BorderlessWindowSettings);
        Self {
            window_title: settings.initial_title().to_owned(),
            play_sound: task.GeneralSettings.as_ref().is_none_or(|g| g.PlaySoundAfterAction),
            settings,
            task,
            windows,
            ops,
            list: WindowList::default(),
            selected: None,
            error_message: String::new(),
            ignored: 0,
            opened: false,
            settings_changed: false,
        }
    }

    /// No sound, no window list read when it opens (`--render`, tests).
    pub fn set_offline(&mut self) {
        self.play_sound = false;
        self.opened = true;
    }

    /// `ReloadWindowList`.
    pub fn reload(&mut self) {
        self.list.set(self.windows.visible_windows(self.ignored));
        self.selected = None;
    }

    /// `Toggle`; true when the window should close (`AutoCloseWindow`).
    pub fn toggle(&mut self) -> bool {
        let mut m = MANAGER.lock().unwrap_or_else(|e| e.into_inner());
        let manager = m.get_or_insert_with(BorderlessWindowManager::new);
        match bw::toggle(manager, self.ops.as_mut(), &mut self.settings, &self.window_title) {
            ToggleOutcome::Ignored => false,
            ToggleOutcome::NotFound(message) => {
                self.error_message = message;
                false
            }
            ToggleOutcome::Done { close } => {
                self.settings_changed = true;
                self.error_message.clear();
                if self.play_sound {
                    crate::sound::play_action_completed();
                }
                close
            }
        }
    }

    fn error_height(&self, ui: &Ui, width: f32) -> f32 {
        if self.error_message.trim().is_empty() {
            return 0.0;
        }
        8.0 + crate::video_tools::wrapped_text(ui, pos2(0.0, 0.0), width - 110.0, &self.error_message, font(13.0, 400), palette(ui.ctx()).cancel_border, false)
    }

    fn total_height(&self, ui: &Ui) -> f32 {
        let hh = tools::heading_height(ui);
        let inner_w = WIDTH - 32.0 - 26.0;
        let window = 26.0 + hh + 10.0 + tools::CONTROL_HEIGHT + 10.0 + TITLE_H + self.error_height(ui, inner_w);
        let options = 26.0 + tools::grid_height(&[hh, TOGGLE_ROW, TOGGLE_ROW, TOGGLE_ROW], 10.0);
        tools::MARGIN * 2.0 + window + 12.0 + options + 12.0 + tools::BUTTON_HEIGHT
    }
}

impl Dialog for BorderlessWindowWindow {
    fn spec(&self) -> Spec {
        Spec::fit_height(crate::rs_title(s::BorderlessWindowWindow_ShareX_Borderless_Window()), WIDTH, 520.0)
    }

    fn content_height(&mut self, ui: &mut Ui) -> f32 {
        self.total_height(ui)
    }

    fn ui(&mut self, ui: &mut Ui, cx: &mut Cx) -> Vec<Effect> {
        let mut out = Vec::new();
        if !self.opened {
            // `OnOpened` → `SetIgnoredWindowHandle` → `ReloadWindowList`.
            self.opened = true;
            self.reload();
        }
        let p = palette(ui.ctx());
        let before_error = self.error_message.clone();
        let c = tools::background(ui);
        let hh = tools::heading_height(ui);
        let wh = 26.0 + hh + 10.0 + tools::CONTROL_HEIGHT + 10.0 + TITLE_H + self.error_height(ui, c.width() - 26.0);
        let panel = Rect::from_min_size(c.min, vec2(c.width(), wh));
        tools::panel(ui, panel);
        let inner = panel.shrink(13.0);
        tools::heading(ui, Rect::from_min_size(inner.min, vec2(inner.width(), hh)), s::BorderlessWindowWindow_Window());
        let row = Rect::from_min_size(pos2(inner.min.x, inner.min.y + hh + 10.0), vec2(inner.width(), tools::CONTROL_HEIGHT));
        let control = tools::label_row(ui, row, LABEL_W, s::BorderlessWindowWindow_Window_list());
        let combo = ComboBox { padding: Thickness::hv(8.0, 6.0), enabled: true };
        let placeholder = s::BorderlessWindowWindow_Select_window();
        let ctx = ui.ctx().clone();
        let items = self.list.items(&ctx, 16.0);
        let cw = match self.selected {
            Some(_) => combo.auto_width(ui, &items, self.selected),
            None => combo.placeholder_width(ui, placeholder),
        }
        .min(control.width());
        let id = Id::new("borderless-windows");
        let was_open = ComboBox::is_open(ui.ctx(), id);
        if let Some(i) = combo.show_with_placeholder(ui, Rect::from_min_size(control.min, vec2(cw, control.height())), id, &items, self.selected, placeholder) {
            // `OnSelectedWindowChanged`.
            self.selected = Some(i);
            self.window_title = self.list.windows[i].entry.title.clone();
            self.error_message.clear();
        }
        if !was_open && ComboBox::is_open(ui.ctx(), id) {
            // `OnWindowListDropDownOpened`.
            self.reload();
        }
        let row = Rect::from_min_size(pos2(inner.min.x, row.max.y + 10.0), vec2(inner.width(), TITLE_H));
        let control = tools::label_row(ui, row, LABEL_W, s::BorderlessWindowWindow_Window_title());
        let before = self.window_title.clone();
        TextBox { placeholder: s::BorderlessWindowWindow_Enter_a_window_title_manually(), ..TextBox::default() }.show(ui, control, Id::new("borderless-title"), &mut self.window_title);
        if before != self.window_title {
            self.error_message.clear();
        }
        if !self.error_message.trim().is_empty() {
            crate::video_tools::wrapped_text(ui, pos2(inner.min.x + 110.0, row.max.y + 8.0), inner.width() - 110.0, &self.error_message, font(13.0, 400), p.cancel_border, true);
        }

        // Options.
        let oh = 26.0 + tools::grid_height(&[hh, TOGGLE_ROW, TOGGLE_ROW, TOGGLE_ROW], 10.0);
        let panel = Rect::from_min_size(pos2(c.min.x, panel.max.y + 12.0), vec2(c.width(), oh));
        tools::panel(ui, panel);
        let inner = panel.shrink(13.0);
        tools::heading(ui, Rect::from_min_size(inner.min, vec2(inner.width(), hh)), s::BorderlessWindowWindow_Options());
        let rows: [(&str, &mut bool); 3] = [
            (s::BorderlessWindowWindow_Remember_window_title(), &mut self.settings.remember_window_title),
            (s::BorderlessWindowWindow_Close_after_toggling(), &mut self.settings.auto_close_window),
            (s::BorderlessWindowWindow_Keep_taskbar_visible(), &mut self.settings.exclude_taskbar_area),
        ];
        for (k, (text, on)) in rows.into_iter().enumerate() {
            let r = Rect::from_min_size(pos2(inner.min.x, inner.min.y + hh + 10.0 + k as f32 * (TOGGLE_ROW + 10.0)), vec2(inner.width(), TOGGLE_ROW));
            sharex_ui::controls::label(ui, Rect::from_min_max(r.min, pos2(r.max.x - ToggleSwitch::SIZE.x, r.max.y)), text, tools::normal_font(), p.text);
            let before = *on;
            ToggleSwitch { enabled: true }.show(ui, pos2(r.max.x - ToggleSwitch::SIZE.x, center_y(r, ToggleSwitch::SIZE.y)), Id::new(("borderless-option", k)), on);
            if before != *on {
                self.settings_changed = true;
            }
        }

        // The hint and Toggle borderless.
        let bottom = Rect::from_min_size(pos2(c.min.x, panel.max.y + 12.0), vec2(c.width(), tools::BUTTON_HEIGHT));
        let text = s::BorderlessWindowWindow_Toggle_borderless();
        let bwid = tools::icon_text_button_width(ui, text);
        let br = Rect::from_min_size(pos2(bottom.max.x - bwid, bottom.min.y), vec2(bwid, tools::BUTTON_HEIGHT));
        let nf = tools::normal_font();
        tools::text_at(ui, pos2(bottom.min.x, center_y(bottom, sharex_ui::widgets::line_height(ui, &nf))), s::BorderlessWindowWindow_Toggle_again_to_restore_the_original_border_and_window_size(), nf, p.text_secondary);
        let can_toggle = !self.window_title.trim().is_empty();
        if tools::icon_text_button(ui, br, Id::new("borderless-toggle"), icon::Resize, text, can_toggle).clicked() && self.toggle() {
            out.push(Effect::Close);
        }
        if std::mem::take(&mut self.settings_changed) {
            let json = self.settings.to_json();
            crate::data_tools::save_tools_setting(cx, &mut self.task, |t| t.BorderlessWindowSettings = json.clone());
        }
        if before_error.is_empty() != self.error_message.is_empty() {
            out.push(Effect::SetHeight(self.total_height(ui)));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system_tools::native::ListedWindow;
    use sharex_tools::system::inspect_window::{Rect as WRect, WindowEntry, WindowSnapshot};
    use std::sync::{Arc, Mutex as M};

    struct Windows;
    impl WindowSource for Windows {
        fn visible_windows(&self, _: isize) -> Vec<ListedWindow> {
            vec![ListedWindow {
                entry: WindowEntry { handle: 7, title: "Game".into(), class_name: "C".into(), process_name: "game".into(), visible: true, cloaked: false, rectangle: WRect { x: 0, y: 0, width: 10, height: 10 } },
                icon: None,
            }]
        }
        fn snapshot(&self, _: isize) -> Option<WindowSnapshot> {
            None
        }
        fn icon(&self, _: isize) -> Option<image::RgbaImage> {
            None
        }
        fn window_at_point(&self, _: i32, _: i32, _: bool) -> isize {
            0
        }
        fn set_top_most(&self, _: isize, _: bool) {}
        fn set_opacity(&self, _: isize, _: u8) {}
    }

    /// A pretend window "Game" (handle 7); the calls are recorded.
    #[derive(Clone, Default)]
    struct Ops(Arc<M<Vec<String>>>);
    impl WindowOps for Ops {
        fn search_window(&self, title: &str) -> Option<isize> {
            (title == "Game").then_some(7)
        }
        fn is_minimized(&self, _: isize) -> bool {
            false
        }
        fn restore(&mut self, _: isize) {}
        fn window_rect(&self, _: isize) -> WRect {
            WRect { x: 10, y: 10, width: 300, height: 200 }
        }
        fn style(&self, _: isize) -> u32 {
            0x16CF_0000
        }
        fn ex_style(&self, _: isize) -> u32 {
            0
        }
        fn set_style(&mut self, _: isize, s: u32) {
            self.0.lock().unwrap().push(format!("style {s:08X}"));
        }
        fn set_ex_style(&mut self, _: isize, _: u32) {}
        fn screen_bounds(&self, _: isize, _: bool) -> WRect {
            WRect { x: 0, y: 0, width: 1024, height: 768 }
        }
        fn set_window_pos(&mut self, _: isize, r: WRect, _: bool) {
            self.0.lock().unwrap().push(format!("pos {}x{}", r.width, r.height));
        }
    }

    #[test]
    fn toggles_a_listed_window_and_reports_a_missing_one() {
        let defaults = TaskSettings::default();
        let task = TaskSettings::default_task_settings(&defaults);
        let ops = Ops::default();
        let mut w = BorderlessWindowWindow::new(task, &defaults, Box::new(Windows), Box::new(ops.clone()));
        w.set_offline();
        w.reload();
        assert_eq!(w.list.windows.len(), 1);
        w.window_title = "Nope".into();
        assert!(!w.toggle());
        assert_eq!(w.error_message, s::BorderlessWindowViewModel_Window_not_found());
        w.window_title = " Game ".into();
        assert!(!w.toggle());
        assert!(w.error_message.is_empty());
        let calls = ops.0.lock().unwrap().clone();
        assert!(calls.iter().any(|c| c == "pos 1024x768"), "{calls:?}");
        // Toggled back: the original size again.
        w.toggle();
        assert!(ops.0.lock().unwrap().iter().any(|c| c == "pos 300x200"));
    }
}
