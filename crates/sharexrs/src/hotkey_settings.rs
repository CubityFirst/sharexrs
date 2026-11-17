//! Hotkey settings: `ShareX/Presentation/HotkeySettings`
//! (`HotkeySettingsWindow.axaml`/`.cs`, `HotkeySettingsViewModel.cs`,
//! `HotkeySettingsModels.cs`) over the real hotkeys, as
//! `HotkeySettingsAvaloniaService` drives `HotkeyManager`: every change
//! re-registers the hotkey at once.

use egui::{CornerRadius, Id, Rect, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};
use sharex_settings::{ApplicationConfig, HotkeyInfo, HotkeySettings, HotkeyStatus, HotkeyType, InputKey, TaskSettings};
use sharex_strings::sharex as s;
use sharex_ui::controls::{self, Button};
use sharex_ui::icons;
use sharex_ui::keys::app_icons as icon;
use sharex_ui::menu::{Flyout, MenuStyle, Placement};
use sharex_ui::theme::{Thickness, palette};
use sharex_ui::widgets::{center_y, font, galley, line_height, text_width};

use crate::hotkeys::HotkeyManager;
use crate::main_menu::{find_menu_icon, task_menu};

/// `Width="680" Height="600"`, `MinWidth="620" MinHeight="480"`.
pub const WIDTH: f32 = 680.0;
pub const HEIGHT: f32 = 600.0;
pub const MIN_WIDTH: f32 = 620.0;
pub const MIN_HEIGHT: f32 = 480.0;

/// What the window asks of the app.
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    /// "Edit task..." / the row's edit button: Task settings for this
    /// hotkey (`TaskSettingsIntegration.Show(settings.TaskSettings, false)`).
    EditTask(usize),
    /// `TaskHelpers.ToggleHotkeys(false)`: hotkeys back on.
    EnableHotkeys,
    /// Hotkeys were added, removed, moved, retargeted or re-registered:
    /// the main window's hotkey panel and menus follow.
    Changed,
    /// The hotkey at this index was removed (a Task settings window open
    /// on it lets go of it).
    Removed(usize),
    /// Two hotkeys swapped places (`Move`): from, to.
    Moved(usize, usize),
}

/// The hotkeys and what registers them (`IHotkeySettingsService`). The
/// manager is `None` when rendering offscreen: statuses stay as they are.
pub struct Service<'a> {
    pub hotkeys: &'a mut Vec<HotkeySettings>,
    pub manager: Option<&'a mut HotkeyManager>,
    pub settings: &'a mut ApplicationConfig,
}

impl Service<'_> {
    /// `RegisterAndRetry`: the hotkey, then any that failed.
    fn register_and_retry(&mut self, i: usize) {
        let disabled = self.settings.DisableHotkeys;
        if let Some(m) = self.manager.as_deref_mut() {
            m.register_hotkey(&mut self.hotkeys[i], disabled);
            m.register_failed(self.hotkeys, disabled);
        }
    }

    fn set_ignore(&mut self, ignore: bool) {
        if let Some(m) = self.manager.as_deref_mut() {
            m.ignore_hotkeys = ignore;
        }
    }

    fn unregister(&mut self, i: usize) {
        if let Some(m) = self.manager.as_deref_mut() {
            m.unregister_hotkey(&mut self.hotkeys[i]);
        }
    }
}

/// A key press while recording (`HotkeyGesture`): the key as an
/// `InputKey` key code (`ConvertGesture`; `None` for the Windows keys) and
/// the modifiers held.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Gesture {
    pub key: InputKey,
    pub control: bool,
    pub shift: bool,
    pub alt: bool,
    pub win: bool,
}

impl Gesture {
    /// `ConvertGesture`: the key code with the modifier flags.
    fn input_key(&self) -> InputKey {
        let mut k = self.key;
        if self.control {
            k = k | InputKey::Control;
        }
        if self.shift {
            k = k | InputKey::Shift;
        }
        if self.alt {
            k = k | InputKey::Alt;
        }
        k
    }

    /// Whether the key itself is a modifier (`isModifierKey`).
    fn is_modifier_key(&self) -> bool {
        self.key == InputKey::ControlKey || self.key == InputKey::ShiftKey || self.key == InputKey::Menu || (self.key == InputKey::None && self.win)
    }
}

/// The window's state (`HotkeySettingsViewModel`'s fields).
#[derive(Default)]
pub struct HotkeySettingsWindow {
    pub selected: Option<usize>,
    /// The hotkey being recorded (`_capturingItem`).
    pub capturing: Option<usize>,
    pub reset_confirmation: bool,
    /// The open task picker and the row it's for.
    task_menu: Option<(usize, Flyout<HotkeyType>)>,
    scroll: f32,
    effects: Vec<Effect>,
}

/// `Button.compact` here: `Height="34"`, `Padding="9,4"`.
const COMPACT_PADDING: Thickness = Thickness { left: 9.0, top: 4.0, right: 9.0, bottom: 4.0 };
const ROW_BUTTON: f32 = 34.0;

impl HotkeySettingsWindow {
    /// The window opens: the service registers the failed hotkeys again
    /// (`HotkeySettingsAvaloniaService`'s constructor).
    pub fn open(svc: &mut Service) -> Self {
        let disabled = svc.settings.DisableHotkeys;
        if let Some(m) = svc.manager.as_deref_mut() {
            m.register_failed(svc.hotkeys, disabled);
        }
        Self::default()
    }

    /// The window closes (`Dispose`): recording stops, presses count again.
    pub fn close(&mut self, svc: &mut Service) {
        self.stop_capture(svc);
        svc.set_ignore(false);
    }

    fn select(&mut self, i: Option<usize>) {
        self.selected = i;
    }

    // ---- the view model ------------------------------------------------

    /// `Add`: a hotkey with the default task settings, selected.
    pub fn add(&mut self, svc: &mut Service) {
        let task = TaskSettings::default_task_settings(&svc.settings.DefaultTaskSettings);
        svc.hotkeys.push(HotkeySettings { HotkeyInfo: HotkeyInfo::default(), TaskSettings: task, extra: Default::default() });
        self.select(Some(svc.hotkeys.len() - 1));
        self.effects.push(Effect::Changed);
    }

    /// `RemoveSelected`: the next one (or the last) is selected.
    pub fn remove_selected(&mut self, svc: &mut Service) {
        let Some(i) = self.selected.filter(|&i| i < svc.hotkeys.len()) else { return };
        self.stop_capture(svc);
        svc.unregister(i);
        svc.hotkeys.remove(i);
        let n = svc.hotkeys.len();
        self.select((n > 0).then(|| i.min(n - 1)));
        self.effects.push(Effect::Removed(i));
        self.effects.push(Effect::Changed);
    }

    /// `DuplicateSelected`: a copy of the task (without its watch
    /// folders) with no hotkey.
    pub fn duplicate_selected(&mut self, svc: &mut Service) {
        let Some(i) = self.selected.filter(|&i| i < svc.hotkeys.len()) else { return };
        let mut task = svc.hotkeys[i].TaskSettings.clone();
        task.WatchFolderEnabled = false;
        task.WatchFolderList = Vec::new();
        svc.hotkeys.push(HotkeySettings { HotkeyInfo: HotkeyInfo::default(), TaskSettings: task, extra: Default::default() });
        self.select(Some(svc.hotkeys.len() - 1));
        self.effects.push(Effect::Changed);
    }

    /// `MoveSelected`: up or down one, wrapping around.
    pub fn move_selected(&mut self, svc: &mut Service, offset: isize) {
        let n = svc.hotkeys.len();
        let Some(old) = self.selected.filter(|&i| i < n) else { return };
        if n < 2 {
            return;
        }
        let new = (old as isize + offset + n as isize).rem_euclid(n as isize) as usize;
        let h = svc.hotkeys.remove(old);
        svc.hotkeys.insert(new, h);
        // The recording follows its row.
        if let Some(c) = self.capturing {
            self.capturing = Some(moved_index(c, old, new));
        }
        self.select(Some(new));
        self.effects.push(Effect::Moved(old, new));
        self.effects.push(Effect::Changed);
    }

    /// `ChangeTask`.
    pub fn change_task(&mut self, svc: &mut Service, i: usize, job: HotkeyType) {
        self.select(Some(i));
        if svc.hotkeys[i].TaskSettings.Job != job {
            svc.hotkeys[i].TaskSettings.Job = job;
            self.effects.push(Effect::Changed);
        }
    }

    /// `ToggleCapture`: clicking the hotkey starts recording it (clearing
    /// it), clicking it again stops.
    pub fn toggle_capture(&mut self, svc: &mut Service, i: usize) {
        self.select(Some(i));
        if self.capturing == Some(i) {
            self.stop_capture(svc);
            return;
        }
        self.stop_capture(svc);
        self.capturing = Some(i);
        svc.set_ignore(true);
        self.set_hotkey(svc, i, Gesture::default(), false);
    }

    /// `SetHotkey`: the gesture becomes the hotkey and is registered; true
    /// when it's a complete hotkey.
    fn set_hotkey(&mut self, svc: &mut Service, i: usize, g: Gesture, finalize: bool) -> bool {
        let info = &mut svc.hotkeys[i].HotkeyInfo;
        info.hotkey = g.input_key();
        info.win = g.win;
        if finalize && info.is_only_modifiers() {
            info.hotkey = InputKey::None;
            info.win = false;
        }
        svc.register_and_retry(i);
        self.effects.push(Effect::Changed);
        svc.hotkeys[i].HotkeyInfo.is_valid_hotkey()
    }

    /// `ApplyGesture` (`OnHotkeyKeyDown`): Escape clears the hotkey; a
    /// complete hotkey stops recording.
    pub fn key_down(&mut self, svc: &mut Service, g: Gesture, escape: bool) {
        let Some(i) = self.capturing else { return };
        if escape {
            self.set_hotkey(svc, i, Gesture::default(), true);
            self.stop_capture(svc);
            return;
        }
        let complete = self.set_hotkey(svc, i, g, false);
        if complete && !g.is_modifier_key() {
            self.stop_capture(svc);
        }
    }

    /// `StopCapture` (`FinalizeHotkey`): a hotkey of modifiers only is dropped.
    pub fn stop_capture(&mut self, svc: &mut Service) {
        let Some(i) = self.capturing.take() else { return };
        if i < svc.hotkeys.len() {
            let info = &mut svc.hotkeys[i].HotkeyInfo;
            if info.is_only_modifiers() {
                info.hotkey = InputKey::None;
                info.win = false;
            }
            svc.register_and_retry(i);
        }
        svc.set_ignore(false);
        self.effects.push(Effect::Changed);
    }

    /// `ConfirmReset` (`HotkeyManager.ResetHotkeys`): every hotkey goes, the
    /// defaults come back registered, and hotkeys are turned back on.
    pub fn confirm_reset(&mut self, svc: &mut Service) {
        self.stop_capture(svc);
        for i in (0..svc.hotkeys.len()).rev() {
            svc.unregister(i);
            self.effects.push(Effect::Removed(i));
        }
        svc.hotkeys.clear();
        svc.hotkeys.extend(sharex_settings::default_hotkey_list(&svc.settings.DefaultTaskSettings));
        let disabled = svc.settings.DisableHotkeys;
        if let Some(m) = svc.manager.as_deref_mut() {
            m.register_all(svc.hotkeys, disabled);
        }
        if disabled {
            self.effects.push(Effect::EnableHotkeys);
        }
        self.reset_confirmation = false;
        self.select(None);
        self.effects.push(Effect::Changed);
    }

    /// Opens the task picker's submenus along `path` (rendering a state).
    pub fn open_menu_path(&mut self, path: &[usize]) {
        if let Some((_, flyout)) = self.task_menu.as_mut() {
            flyout.open_path(path);
        }
    }

    // ---- the view ------------------------------------------------------

    /// Draws the window over all of `ui`; returns what the app should do.
    pub fn ui(&mut self, ui: &mut Ui, svc: &mut Service) -> Vec<Effect> {
        let p = palette(ui.ctx());
        let full = ui.max_rect();
        ui.painter().rect_filled(full, 0.0, p.background_main);
        if self.selected.is_some_and(|i| i >= svc.hotkeys.len()) {
            self.selected = None;
        }
        // `Grid x:Name="ContentGrid" Margin="10,0,10,10" RowSpacing="5"`.
        let content = Rect::from_min_max(pos2(full.min.x + 10.0, full.min.y), pos2(full.max.x - 10.0, full.max.y - 10.0));
        let modal = self.reset_confirmation;
        // A menu opened this frame isn't dismissed by the press that opened
        // it (a click's press and release can arrive in one frame).
        let had_menu = self.task_menu.is_some();
        let toolbar = self.toolbar(ui, content, svc, modal);
        // The banner's row keeps its `RowSpacing` when it's collapsed.
        let mut y = toolbar.max.y + 5.0;
        if svc.settings.DisableHotkeys {
            y = self.disabled_banner(ui, content, y, modal);
        }
        y += 5.0;
        let panel = Rect::from_min_max(pos2(content.min.x, y), content.max);
        self.list(ui, panel, svc, modal);
        if self.reset_confirmation {
            self.reset_dialog(ui, full, svc);
        }
        if let Some((row, flyout)) = self.task_menu.as_mut() {
            let row = *row;
            let out = flyout.show(ui, full);
            let pressed_outside = had_menu
                && ui.input(|i| i.pointer.any_pressed())
                && ui.input(|i| i.pointer.interact_pos()).is_some_and(|pos| !flyout.contains(pos));
            if let Some(job) = out.command {
                self.task_menu = None;
                if row < svc.hotkeys.len() {
                    self.change_task(svc, row, job);
                }
            } else if out.closed || pressed_outside {
                self.task_menu = None;
            }
        }
        std::mem::take(&mut self.effects)
    }

    /// `Border.tool-panel.docked-top-toolbar`: the buttons, Reset at the right.
    fn toolbar(&mut self, ui: &mut Ui, content: Rect, svc: &mut Service, modal: bool) -> Rect {
        let p = palette(ui.ctx());
        // Padding 8, no top border, 1px elsewhere, bottom corners 4.
        let h = 8.0 + ROW_BUTTON + 8.0 + 1.0;
        let r = Rect::from_min_size(content.min, vec2(content.width(), h));
        let radius = CornerRadius { sw: 4, se: 4, ..Default::default() };
        ui.painter().rect_filled(r, radius, p.border);
        ui.painter().rect_filled(Rect::from_min_max(pos2(r.min.x + 1.0, r.min.y), pos2(r.max.x - 1.0, r.max.y - 1.0)), CornerRadius { sw: 3, se: 3, ..Default::default() }, p.background_panel);
        let has = self.selected.is_some();
        let can_move = has && svc.hotkeys.len() > 1;
        let by = r.min.y + 8.0;
        let mut x = r.min.x + 1.0 + 8.0;
        let live = !modal;
        let items = [
            (icon::Plus, s::HotkeySettingsWindow_Add(), true),
            (icon::Trash, s::HotkeySettingsWindow_Remove(), has),
            (icon::Settings, s::HotkeySettingsWindow_EditTask(), has),
            (icon::Duplicate, s::HotkeySettingsWindow_Duplicate(), has),
        ];
        let mut clicked = None;
        for (i, (key, text, enabled)) in items.into_iter().enumerate() {
            let w = icon_text_button_width(ui, text);
            let br = Rect::from_min_size(pos2(x, by), vec2(w, ROW_BUTTON));
            if icon_text_button(ui, br, Id::new(("hk-toolbar", i)), key, text, enabled && live).clicked() {
                clicked = Some(i);
            }
            x += w + 6.0;
        }
        for (i, key) in [icon::ChevronUp, icon::ChevronDown].into_iter().enumerate() {
            let br = Rect::from_min_size(pos2(x, by), Vec2::splat(ROW_BUTTON));
            if icon_button(ui, br, Id::new(("hk-move", i)), key, 16.0, can_move && live).clicked() {
                clicked = Some(4 + i);
            }
            x += ROW_BUTTON + 6.0;
        }
        let reset = s::HotkeySettingsWindow_ResetWithEllipsis();
        let w = icon_text_button_width(ui, reset);
        let br = Rect::from_min_size(pos2(r.max.x - 1.0 - 8.0 - w, by), vec2(w, ROW_BUTTON));
        if icon_text_button(ui, br, Id::new("hk-reset"), icon::Reset, reset, live).clicked() {
            clicked = Some(6);
        }
        match clicked {
            Some(0) => self.add(svc),
            Some(1) => self.remove_selected(svc),
            Some(2) => {
                if let Some(i) = self.selected {
                    self.effects.push(Effect::EditTask(i));
                }
            }
            Some(3) => self.duplicate_selected(svc),
            Some(4) => self.move_selected(svc, -1),
            Some(5) => self.move_selected(svc, 1),
            Some(_) => self.reset_confirmation = true,
            None => {}
        }
        r
    }

    /// "Global hotkeys are currently disabled." and "Enable hotkeys".
    fn disabled_banner(&mut self, ui: &mut Ui, content: Rect, y: f32, modal: bool) -> f32 {
        let p = palette(ui.ctx());
        let body = font(13.0, 400);
        let label = s::HotkeySettingsWindow_EnableHotkeys();
        let button = Button::compact(COMPACT_PADDING);
        let bw = button.width(ui, label);
        let inner_h = line_height(ui, &body).max(18.0).max(ROW_BUTTON);
        let r = Rect::from_min_size(pos2(content.min.x, y), vec2(content.width(), 1.0 + 6.0 + inner_h + 6.0 + 1.0));
        ui.painter().rect_filled(r, CornerRadius::same(4), p.control_background);
        ui.painter().rect_stroke(r, CornerRadius::same(4), Stroke::new(1.0, p.cancel_border), StrokeKind::Inside);
        let inner = Rect::from_min_max(pos2(r.min.x + 9.0, r.min.y + 7.0), pos2(r.max.x - 9.0, r.max.y - 7.0));
        icons::paint(ui.painter(), Rect::from_min_size(pos2(inner.min.x, center_y(inner, 18.0)), Vec2::splat(18.0)), icon::Warning, p.cancel_border);
        let text_rect = Rect::from_min_max(pos2(inner.min.x + 18.0 + 9.0, inner.min.y), pos2(inner.max.x - bw - 9.0, inner.max.y));
        controls::label(ui, text_rect, s::HotkeySettingsWindow_GlobalHotkeysAreCurrentlyDisabled(), body, p.text_secondary);
        let br = Rect::from_min_size(pos2(inner.max.x - bw, center_y(inner, ROW_BUTTON)), vec2(bw, ROW_BUTTON));
        if button.show(ui, br, Id::new("hk-enable"), label, !modal).clicked() {
            self.effects.push(Effect::EnableHotkeys);
        }
        r.max.y
    }

    /// The list panel: the column headings, then a row per hotkey.
    fn list(&mut self, ui: &mut Ui, panel: Rect, svc: &mut Service, modal: bool) {
        let p = palette(ui.ctx());
        controls::card(ui, panel, 4.0);
        let inner = panel.shrink(1.0 + 8.0);
        let heading = font(12.0, 400);
        let body = font(13.0, 400);
        // The shared hotkey column: the widest hotkey button, 4, the 32px status.
        let texts: Vec<String> = (0..svc.hotkeys.len()).map(|i| self.display_hotkey(svc, i)).collect();
        let button_w = texts.iter().map(|t| text_width(ui, t, body.clone())).fold(0.0, f32::max) + 16.0 + 16.0 + 2.0;
        let group_w = button_w + 4.0 + 32.0;
        // Headings: `Margin="4,0,4,2"`, `ColumnSpacing="6"`.
        let hh = line_height(ui, &heading);
        let head = Rect::from_min_size(pos2(inner.min.x + 4.0, inner.min.y), vec2(inner.width() - 8.0, hh));
        controls::label(ui, head, s::HotkeySettingsWindow_Task(), heading.clone(), p.text_secondary);
        let hx = head.max.x - group_w;
        controls::label(ui, Rect::from_min_max(pos2(hx, head.min.y), head.max), s::HotkeySettingsWindow_Hotkey(), heading, p.text_secondary);
        let list = Rect::from_min_max(pos2(inner.min.x, head.max.y + 2.0 + 4.0), inner.max);
        if svc.hotkeys.is_empty() {
            self.empty(ui, list);
            return;
        }
        let row_h = 3.0 + ROW_BUTTON + 3.0;
        let content_h = row_h * svc.hotkeys.len() as f32;
        let max_scroll = (content_h - list.height()).max(0.0);
        if ui.rect_contains_pointer(list) && self.task_menu.is_none() && !modal {
            self.scroll -= ui.input(|i| i.smooth_scroll_delta.y);
        }
        self.scroll = self.scroll.clamp(0.0, max_scroll);
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(list));
        child.set_clip_rect(list.intersect(ui.clip_rect()));
        let live = !modal && self.task_menu.is_none();
        let mut action = None;
        for (i, text) in texts.iter().enumerate() {
            let row = Rect::from_min_size(pos2(list.min.x, list.min.y + row_h * i as f32 - self.scroll), vec2(list.width(), row_h));
            if !row.intersects(list) {
                continue;
            }
            if let Some(a) = self.row(&mut child, row, i, svc, text, button_w, live) {
                action = Some((i, a));
            }
        }
        if max_scroll > 0.0 {
            crate::settings_form::scroll_indicator(&child, list, self.scroll, content_h);
        }
        // Clicking anywhere else ends a recording (`LostFocus`).
        if self.capturing.is_some()
            && ui.input(|i| i.pointer.any_pressed())
            && !matches!(action, Some((_, RowAction::Hotkey)))
            && ui.input(|i| i.pointer.interact_pos()).is_some_and(|pos| ui.max_rect().contains(pos))
        {
            self.stop_capture(svc);
        }
        match action {
            Some((i, RowAction::Select)) => self.select(Some(i)),
            Some((i, RowAction::Task(anchor))) => {
                self.select(Some(i));
                let current = svc.hotkeys[i].TaskSettings.Job.clone();
                let mut flyout = Flyout::menu(Id::new(("hk-task-menu", i)), task_menu(&current), anchor, Placement::BottomEdgeAlignedLeft);
                flyout.style = MenuStyle::Standard;
                self.task_menu = Some((i, flyout));
            }
            Some((i, RowAction::Edit)) => {
                self.select(Some(i));
                self.effects.push(Effect::EditTask(i));
            }
            Some((i, RowAction::Hotkey)) => self.toggle_capture(svc, i),
            None => {}
        }
    }

    /// `HotkeySettingsItem.DisplayHotkey`.
    fn display_hotkey(&self, svc: &Service, i: usize) -> String {
        if self.capturing == Some(i) { s::HotkeySettingsWindow_PressAHotkey().to_string() } else { svc.hotkeys[i].HotkeyInfo.to_string() }
    }

    /// One `Border.hotkey-item` (`Padding="4,3"`): the task picker, its
    /// edit button, the hotkey and its status.
    #[allow(clippy::too_many_arguments)]
    fn row(&mut self, ui: &mut Ui, row: Rect, i: usize, svc: &Service, hotkey_text: &str, button_w: f32, live: bool) -> Option<RowAction> {
        let p = palette(ui.ctx());
        let resp = ui.interact(row, Id::new(("hk-row", i)), egui::Sense::click());
        let selected = self.selected == Some(i);
        if selected || (live && resp.hovered()) {
            ui.painter().rect_filled(row, CornerRadius::same(4), p.control_background_hover);
        }
        let inner = Rect::from_min_max(pos2(row.min.x + 4.0, row.min.y + 3.0), pos2(row.max.x - 4.0, row.max.y - 3.0));
        let h = &svc.hotkeys[i];
        let group_w = button_w + 4.0 + 32.0;
        let left_w = inner.width() - 4.0 - group_w;
        let task_rect = Rect::from_min_size(inner.min, vec2(left_w - 4.0 - ROW_BUTTON, ROW_BUTTON));
        let edit_rect = Rect::from_min_size(pos2(task_rect.max.x + 4.0, inner.min.y), Vec2::splat(ROW_BUTTON));
        let hk_rect = Rect::from_min_size(pos2(inner.max.x - group_w, inner.min.y), vec2(button_w, ROW_BUTTON));
        let status_rect = Rect::from_min_size(pos2(hk_rect.max.x + 4.0, inner.min.y), vec2(32.0, ROW_BUTTON));
        let mut action = None;
        // `Button.task-selector`: Padding 8,4; icon (18 wide), name, the
        // "custom settings" mark, the chevron; `ColumnSpacing="7"`.
        let task_resp = Button::compact(Thickness::hv(8.0, 4.0)).show(ui, task_rect, Id::new(("hk-task", i)), "", live);
        let c = Rect::from_min_max(pos2(task_rect.min.x + 1.0 + 8.0, task_rect.min.y + 1.0 + 4.0), pos2(task_rect.max.x - 1.0 - 8.0, task_rect.max.y - 1.0 - 4.0));
        let job = &h.TaskSettings.Job;
        let icon_rect = Rect::from_min_size(pos2(c.min.x + 1.0, center_y(c, 16.0)), Vec2::splat(16.0));
        icons::paint(ui.painter(), icon_rect, find_menu_icon(job), p.accent_icon);
        let chevron = Rect::from_min_size(pos2(c.max.x - 14.0, center_y(c, 14.0)), Vec2::splat(14.0));
        icons::paint(ui.painter(), chevron, icon::ChevronDown, p.text);
        let customized = !h.TaskSettings.is_using_default_settings();
        let mark_w = if customized { 14.0 } else { 0.0 };
        if customized {
            let mark = Rect::from_min_size(pos2(chevron.min.x - 7.0 - 14.0, center_y(c, 14.0)), Vec2::splat(14.0));
            icons::paint(ui.painter(), mark, icon::Edit, p.accent_icon);
        }
        let name_rect = Rect::from_min_max(pos2(c.min.x + 18.0 + 7.0, c.min.y), pos2(chevron.min.x - 7.0 - mark_w - 7.0, c.max.y));
        controls::label(ui, name_rect, &job.localized_description(), font(13.0, 400), p.text_secondary);
        if task_resp.clicked() {
            action = Some(RowAction::Task(task_rect));
        }
        // `Button.row-icon-button`: Edit task settings.
        if icon_button(ui, edit_rect, Id::new(("hk-edit", i)), icon::Edit, 17.0, live).clicked() {
            action = Some(RowAction::Edit);
        }
        // `Button.hotkey-capture`: Padding 16,4, the hotkey centred.
        let hk_resp = Button::compact(Thickness::hv(16.0, 4.0)).show(ui, hk_rect, Id::new(("hk-key", i)), "", live);
        let body = font(13.0, 400);
        let tw = text_width(ui, hotkey_text, body.clone());
        let hc = Rect::from_min_max(pos2(hk_rect.min.x + 17.0, hk_rect.min.y), pos2(hk_rect.max.x - 17.0, hk_rect.max.y));
        let x = hc.min.x + ((hc.width() - tw) / 2.0).round_ties_even().max(0.0);
        let g = galley(ui, hotkey_text, body.clone(), p.text_secondary, hc.max.x - x);
        ui.painter().galley(pos2(x, center_y(hc, line_height(ui, &body))), g, p.text_secondary);
        if hk_resp.clicked() {
            action = Some(RowAction::Hotkey);
        }
        // The status: registered, failed or not configured, 19px.
        let (key, color) = match h.HotkeyInfo.status {
            HotkeyStatus::Registered => (icon::CircleCheck, p.continue_border),
            HotkeyStatus::Failed => (icon::CircleX, p.cancel_border),
            HotkeyStatus::NotConfigured => (icon::CircleDashed, p.text_secondary),
        };
        // `HorizontalAlignment="Center"` (rounded), drawn in the middle of
        // its stretched height.
        let st = Rect::from_min_size(pos2(status_rect.min.x + ((32.0f32 - 19.0) / 2.0).round_ties_even(), status_rect.min.y + (ROW_BUTTON - 19.0) / 2.0), Vec2::splat(19.0));
        icons::paint(ui.painter(), st, key, color);
        if action.is_none() && live && resp.clicked() {
            action = Some(RowAction::Select);
        }
        action
    }

    /// `IsEmpty`: the keyboard and "Add a hotkey to create a workflow."
    fn empty(&self, ui: &Ui, list: Rect) {
        let p = palette(ui.ctx());
        let body = font(13.0, 400);
        let text = s::HotkeySettingsWindow_AddAHotkeyToCreateAWorkflow();
        let lh = line_height(ui, &body);
        let h = 42.0 + 8.0 + lh;
        let top = center_y(list, h);
        icons::paint(ui.painter(), Rect::from_min_size(pos2((list.center().x - 21.0).round(), top), Vec2::splat(42.0)), icon::Keyboard, p.text_secondary);
        let tw = text_width(ui, text, body.clone());
        let x = list.min.x + ((list.width() - tw) / 2.0).round_ties_even();
        ui.painter().galley(pos2(x, top + 50.0), galley(ui, text, body, p.text_secondary, f32::INFINITY), p.text_secondary);
    }

    /// The reset confirmation: a `tool-panel` card (390 wide, `Padding="18"`)
    /// over `ShareX.Brush.Overlay.Modal`.
    fn reset_dialog(&mut self, ui: &mut Ui, full: Rect, svc: &mut Service) {
        let p = palette(ui.ctx());
        let layer = egui::LayerId::new(egui::Order::Foreground, Id::new("hk-reset-dialog"));
        let ui = ui.new_child(egui::UiBuilder::new().layer_id(layer).max_rect(full));
        ui.painter().rect_filled(full, 0.0, p.overlay_modal);
        let _ = ui.interact(full, Id::new("hk-reset-block"), egui::Sense::click());
        let w = 390.0;
        let inner_w = w - 2.0 - 36.0;
        let title_font = font(15.0, 600);
        let body = font(13.0, 400);
        let title_h = line_height(&ui, &title_font).max(20.0);
        let desc = s::HotkeySettingsWindow_ResetHotkeysDescription();
        let desc_h = controls::wrapped(&ui, pos2(0.0, 0.0), inner_w, desc, body.clone(), p.text_secondary, false);
        let cancel_b = Button::default();
        let bh = 40.0;
        let h = 1.0 + 18.0 + title_h + 12.0 + desc_h + 12.0 + bh + 18.0 + 1.0;
        let c = Rect::from_center_size(full.center(), vec2(w, h));
        let card = Rect::from_min_size(pos2(c.min.x.round_ties_even(), c.min.y.round_ties_even()), vec2(w, h));
        controls::card(&ui, card, 4.0);
        let inner = card.shrink(19.0);
        let head = Rect::from_min_size(inner.min, vec2(inner.width(), title_h));
        icons::paint(ui.painter(), Rect::from_min_size(pos2(head.min.x, center_y(head, 20.0)), Vec2::splat(20.0)), icon::Reset, p.accent_icon);
        controls::label(&ui, Rect::from_min_max(pos2(head.min.x + 30.0, head.min.y), head.max), s::HotkeySettingsWindow_ResetHotkeysQuestion(), title_font, p.text);
        controls::wrapped(&ui, pos2(inner.min.x, head.max.y + 12.0), inner_w, desc, body, p.text_secondary, true);
        let cancel = s::HotkeySettingsWindow_Cancel();
        let reset = s::HotkeySettingsWindow_Reset();
        let rw = cancel_b.width(&ui, reset);
        let cw = cancel_b.width(&ui, cancel);
        let by = inner.max.y - bh;
        let reset_rect = Rect::from_min_size(pos2(inner.max.x - rw, by), vec2(rw, bh));
        let cancel_rect = Rect::from_min_size(pos2(reset_rect.min.x - 8.0 - cw, by), vec2(cw, bh));
        let cancelled = cancel_b.show(&ui, cancel_rect, Id::new("hk-reset-cancel"), cancel, true).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape));
        if danger_button(&ui, reset_rect, Id::new("hk-reset-confirm"), reset).clicked() {
            self.confirm_reset(svc);
        } else if cancelled {
            self.reset_confirmation = false;
        }
    }
}

/// What a row was clicked for.
enum RowAction {
    Select,
    /// The task picker, opened under its button.
    Task(Rect),
    Edit,
    Hotkey,
}

/// An index after the item at `from` moved to `to`.
pub fn moved_index(i: usize, from: usize, to: usize) -> usize {
    if i == from {
        to
    } else if from < to && i > from && i <= to {
        i - 1
    } else if to < from && i >= to && i < from {
        i + 1
    } else {
        i
    }
}

/// A compact toolbar button's width: icon (the button's 13px font), 7,
/// the label (a `TextBlock`, Segoe UI Normal).
fn icon_text_button_width(ui: &Ui, text: &str) -> f32 {
    1.0 + COMPACT_PADDING.left + 13.0 + 7.0 + text_width(ui, text, font(13.0, 400)) + COMPACT_PADDING.right + 1.0
}

/// `Button.compact` with an icon and a label (`StackPanel Spacing="7"`).
fn icon_text_button(ui: &Ui, rect: Rect, id: Id, key: &str, text: &str, enabled: bool) -> egui::Response {
    let p = palette(ui.ctx());
    let r = Button::compact(COMPACT_PADDING).show(ui, rect, id, "", enabled);
    let opacity = if enabled { 1.0 } else { 0.35 };
    let body = font(13.0, 400);
    let tw = text_width(ui, text, body.clone());
    let content_w = 13.0 + 7.0 + tw;
    let inner = Rect::from_min_max(
        pos2(rect.min.x + 1.0 + COMPACT_PADDING.left, rect.min.y + 1.0 + COMPACT_PADDING.top),
        pos2(rect.max.x - 1.0 - COMPACT_PADDING.right, rect.max.y - 1.0 - COMPACT_PADDING.bottom),
    );
    let x = inner.min.x + ((inner.width() - content_w) / 2.0).round_ties_even().max(0.0);
    // The StackPanel is as tall as its text, centred; the icon is drawn
    // in the middle of its stretched slot (not snapped).
    let lh = line_height(ui, &body);
    let top = center_y(inner, lh);
    icons::paint(ui.painter(), Rect::from_min_size(pos2(x, top + (lh - 13.0) / 2.0), Vec2::splat(13.0)), key, p.text.gamma_multiply(opacity));
    let color = p.text_secondary.gamma_multiply(opacity);
    ui.painter().galley(pos2(x + 20.0, top), galley(ui, text, body, color, f32::INFINITY), color);
    r
}

/// A square button with only an icon of `size`, centred.
fn icon_button(ui: &Ui, rect: Rect, id: Id, key: &str, size: f32, enabled: bool) -> egui::Response {
    let p = palette(ui.ctx());
    let r = Button::compact(Thickness::hv(0.0, 0.0)).show(ui, rect, id, "", enabled);
    let opacity = if enabled { 1.0 } else { 0.35 };
    // Centred in the content box, inside the 1px border.
    let inner = rect.shrink(1.0);
    let b = Rect::from_min_size(pos2(inner.min.x + ((inner.width() - size) / 2.0).round_ties_even(), center_y(inner, size)), Vec2::splat(size));
    icons::paint(ui.painter(), b, key, p.text.gamma_multiply(opacity));
    r
}

/// `Button.task-mode-button.danger`: the cancel colours, the border colour
/// as the fill on pointer over and press; its string content in Inter.
fn danger_button(ui: &Ui, rect: Rect, id: Id, text: &str) -> egui::Response {
    let p = palette(ui.ctx());
    let r = ui.interact(rect, id, egui::Sense::click());
    let fill = if r.hovered() || r.is_pointer_button_down_on() { p.cancel_border } else { p.cancel_background };
    ui.painter().rect_filled(rect, CornerRadius::same(4), fill);
    ui.painter().rect_stroke(rect, CornerRadius::same(4), Stroke::new(1.0, p.cancel_border), StrokeKind::Inside);
    let f = controls::content_font(13.0, 400);
    let tw = text_width(ui, text, f.clone());
    let inner = rect.shrink(1.0);
    let x = inner.min.x + ((inner.width() - tw) / 2.0).round_ties_even();
    ui.painter().galley(pos2(x, center_y(inner, line_height(ui, &f))), galley(ui, text, f, p.task_mode_foreground, f32::INFINITY), p.task_mode_foreground);
    r
}

/// The key a winit key code is, as an `InputKey` key code (Windows'
/// virtual-key codes); `None` for keys ShareX can't take as hotkeys.
pub fn input_key(code: winit::keyboard::KeyCode) -> Option<InputKey> {
    use winit::keyboard::KeyCode as K;
    let vk = match code {
        K::Backspace => 8,
        K::Tab => 9,
        K::Enter | K::NumpadEnter => 13,
        K::ShiftLeft | K::ShiftRight => 16,
        K::ControlLeft | K::ControlRight => 17,
        K::AltLeft | K::AltRight => 18,
        K::Pause => 19,
        K::CapsLock => 20,
        K::Escape => 27,
        K::Space => 32,
        K::PageUp => 33,
        K::PageDown => 34,
        K::End => 35,
        K::Home => 36,
        K::ArrowLeft => 37,
        K::ArrowUp => 38,
        K::ArrowRight => 39,
        K::ArrowDown => 40,
        K::PrintScreen => 44,
        K::Insert => 45,
        K::Delete => 46,
        K::Digit0 => 48,
        K::Digit1 => 49,
        K::Digit2 => 50,
        K::Digit3 => 51,
        K::Digit4 => 52,
        K::Digit5 => 53,
        K::Digit6 => 54,
        K::Digit7 => 55,
        K::Digit8 => 56,
        K::Digit9 => 57,
        K::KeyA => 65,
        K::KeyB => 66,
        K::KeyC => 67,
        K::KeyD => 68,
        K::KeyE => 69,
        K::KeyF => 70,
        K::KeyG => 71,
        K::KeyH => 72,
        K::KeyI => 73,
        K::KeyJ => 74,
        K::KeyK => 75,
        K::KeyL => 76,
        K::KeyM => 77,
        K::KeyN => 78,
        K::KeyO => 79,
        K::KeyP => 80,
        K::KeyQ => 81,
        K::KeyR => 82,
        K::KeyS => 83,
        K::KeyT => 84,
        K::KeyU => 85,
        K::KeyV => 86,
        K::KeyW => 87,
        K::KeyX => 88,
        K::KeyY => 89,
        K::KeyZ => 90,
        K::SuperLeft | K::SuperRight => 0,
        K::ContextMenu => 93,
        K::Numpad0 => 96,
        K::Numpad1 => 97,
        K::Numpad2 => 98,
        K::Numpad3 => 99,
        K::Numpad4 => 100,
        K::Numpad5 => 101,
        K::Numpad6 => 102,
        K::Numpad7 => 103,
        K::Numpad8 => 104,
        K::Numpad9 => 105,
        K::NumpadMultiply => 106,
        K::NumpadAdd => 107,
        K::NumpadSubtract => 109,
        K::NumpadDecimal => 110,
        K::NumpadDivide => 111,
        K::F1 => 112,
        K::F2 => 113,
        K::F3 => 114,
        K::F4 => 115,
        K::F5 => 116,
        K::F6 => 117,
        K::F7 => 118,
        K::F8 => 119,
        K::F9 => 120,
        K::F10 => 121,
        K::F11 => 122,
        K::F12 => 123,
        K::F13 => 124,
        K::F14 => 125,
        K::F15 => 126,
        K::F16 => 127,
        K::F17 => 128,
        K::F18 => 129,
        K::F19 => 130,
        K::F20 => 131,
        K::F21 => 132,
        K::F22 => 133,
        K::F23 => 134,
        K::F24 => 135,
        K::NumLock => 144,
        K::ScrollLock => 145,
        K::BrowserBack => 166,
        K::BrowserForward => 167,
        K::BrowserRefresh => 168,
        K::BrowserStop => 169,
        K::BrowserSearch => 170,
        K::BrowserFavorites => 171,
        K::BrowserHome => 172,
        K::AudioVolumeMute => 173,
        K::AudioVolumeDown => 174,
        K::AudioVolumeUp => 175,
        K::MediaTrackNext => 176,
        K::MediaTrackPrevious => 177,
        K::MediaStop => 178,
        K::MediaPlayPause => 179,
        K::LaunchMail => 180,
        K::MediaSelect => 181,
        K::LaunchApp1 => 182,
        K::LaunchApp2 => 183,
        K::Semicolon => 186,
        K::Equal => 187,
        K::Comma => 188,
        K::Minus => 189,
        K::Period => 190,
        K::Slash => 191,
        K::Backquote => 192,
        K::BracketLeft => 219,
        K::Backslash => 220,
        K::BracketRight => 221,
        K::Quote => 222,
        K::IntlBackslash => 226,
        _ => return None,
    };
    Some(InputKey(vk))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sharex_settings::TaskSettings;

    fn svc_parts() -> (Vec<HotkeySettings>, ApplicationConfig) {
        let settings = ApplicationConfig::default();
        (sharex_settings::default_hotkey_list(&TaskSettings::default()), settings)
    }

    #[test]
    fn recording_takes_a_complete_hotkey() {
        let (mut hotkeys, mut settings) = svc_parts();
        let mut svc = Service { hotkeys: &mut hotkeys, manager: None, settings: &mut settings };
        let mut w = HotkeySettingsWindow::default();
        w.toggle_capture(&mut svc, 1);
        assert_eq!(svc.hotkeys[1].HotkeyInfo.hotkey, InputKey::None, "recording clears the hotkey");
        // Ctrl alone: "Ctrl + ...", still recording.
        w.key_down(&mut svc, Gesture { key: InputKey::ControlKey, control: true, ..Default::default() }, false);
        assert_eq!(svc.hotkeys[1].HotkeyInfo.to_string(), "Ctrl + ...");
        assert_eq!(w.capturing, Some(1));
        w.key_down(&mut svc, Gesture { key: InputKey::F9, control: true, ..Default::default() }, false);
        assert_eq!(svc.hotkeys[1].HotkeyInfo.to_string(), "Ctrl + F9");
        assert_eq!(w.capturing, None);
        // Modifiers only, then clicking away: the hotkey is dropped.
        w.toggle_capture(&mut svc, 0);
        w.key_down(&mut svc, Gesture { key: InputKey::ShiftKey, shift: true, ..Default::default() }, false);
        w.stop_capture(&mut svc);
        assert_eq!(svc.hotkeys[0].HotkeyInfo.hotkey, InputKey::None);
        // Escape clears.
        w.toggle_capture(&mut svc, 2);
        w.key_down(&mut svc, Gesture::default(), true);
        assert_eq!(svc.hotkeys[2].HotkeyInfo.to_string(), "None");
    }

    #[test]
    fn add_remove_duplicate_move() {
        let (mut hotkeys, mut settings) = svc_parts();
        let mut svc = Service { hotkeys: &mut hotkeys, manager: None, settings: &mut settings };
        let mut w = HotkeySettingsWindow::default();
        w.add(&mut svc);
        assert_eq!(svc.hotkeys.len(), 6);
        assert_eq!(w.selected, Some(5));
        assert_eq!(svc.hotkeys[5].TaskSettings.Job, HotkeyType::None);
        w.select(Some(0));
        w.duplicate_selected(&mut svc);
        assert_eq!(svc.hotkeys[6].TaskSettings.Job, HotkeyType::RectangleRegion);
        assert_eq!(svc.hotkeys[6].HotkeyInfo.hotkey, InputKey::None);
        w.select(Some(0));
        w.move_selected(&mut svc, -1);
        assert_eq!(w.selected, Some(6), "moving the first up wraps to the end");
        assert_eq!(svc.hotkeys[6].TaskSettings.Job, HotkeyType::RectangleRegion);
        w.remove_selected(&mut svc);
        assert_eq!(svc.hotkeys.len(), 6);
        assert_eq!(w.selected, Some(5));
        w.confirm_reset(&mut svc);
        assert_eq!(svc.hotkeys.len(), 5);
        assert_eq!(w.selected, None);
    }

    #[test]
    fn indices_follow_a_move() {
        assert_eq!(moved_index(2, 2, 0), 0);
        assert_eq!(moved_index(0, 2, 0), 1);
        assert_eq!(moved_index(3, 0, 4), 2);
        assert_eq!(moved_index(5, 0, 4), 5);
    }
}
