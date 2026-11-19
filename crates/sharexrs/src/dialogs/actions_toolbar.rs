//! `ShareX/Presentation/ActionsToolbar`: `ActionsToolbarWindow` (a small
//! borderless bar of job buttons, `ApplicationConfig.ActionsToolbarList`,
//! dragged by its title, files dropped on it uploaded) and
//! `ActionsToolbarEditorWindow` (add, remove and move the actions).
//!
//! The title says ShareXrs where ShareX's says ShareX.

use std::path::PathBuf;

use egui::{Id, Rect, Sense, Vec2, pos2, vec2};
use sharex_settings::{ApplicationConfig, HotkeyType, TaskSettings};
use sharex_strings::sharex as s;
use sharex_ui::border::{self, Brush};
use sharex_ui::controls::Button;
use sharex_ui::keys::app_icons as icon;
use sharex_ui::menu::{Flyout, MenuEntry, MenuIcon, MenuStyle, Placement};
use sharex_ui::theme::{Thickness, palette};
use sharex_ui::widgets::{center_y, font, galley, line_height, text_width};
use sharex_ui::icons;

use super::{Cx, Dialog, Effect, Spec, UploadRequest};
use crate::history_ui::common::docked_toolbar;
use crate::main_menu::find_menu_icon;

/// The title (`TextBlock Text="ShareX"`).
pub const BRAND: &str = "ShareXrs";
/// `Button.toolbar-action`: 34x34, 4px corners; the frame's 1px border,
/// `Padding="2"`, items `Spacing="2"`.
const BUTTON: f32 = 34.0;
const FRAME: f32 = 3.0;
const SPACING: f32 = 2.0;
/// `Border.toolbar-title`: `Padding="8,0"`.
const TITLE_PADDING: f32 = 8.0;

/// What the title's context menu does (`CreateToolbarMenu`).
#[derive(Clone, Debug, PartialEq)]
pub enum ToolbarCmd {
    Close,
    LockPosition,
    StayOnTop,
    OpenAtStartup,
    Edit,
}

/// `CreateToolbarMenu`'s items, checked as the settings are.
pub fn toolbar_menu(settings: &ApplicationConfig) -> Vec<MenuEntry<ToolbarCmd>> {
    use sharex_ui::menu::Toggle;
    vec![
        MenuEntry::item(s::ActionsToolbarWindow_Close(), "", ToolbarCmd::Close),
        MenuEntry::separator(),
        MenuEntry::item(s::ActionsToolbarWindow_LockPosition(), "", ToolbarCmd::LockPosition).toggle(Toggle::CheckBox, settings.ActionsToolbarLockPosition),
        MenuEntry::item(s::ActionsToolbarWindow_StayOnTop(), "", ToolbarCmd::StayOnTop).toggle(Toggle::CheckBox, settings.ActionsToolbarStayTopMost),
        MenuEntry::item(s::ActionsToolbarWindow_OpenAtStartup(), "", ToolbarCmd::OpenAtStartup).toggle(Toggle::CheckBox, settings.ActionsToolbarRunAtStartup),
        MenuEntry::separator(),
        MenuEntry::item(s::ActionsToolbarWindow_Edit(), "", ToolbarCmd::Edit),
    ]
}

pub struct ActionsToolbarWindow {
    /// The actions shown (`RefreshToolbar` reads the settings each frame).
    actions: Vec<HotkeyType>,
    /// Files dragged over it (`DropOverlay`).
    pub dragging: bool,
    pub title: String,
    /// The settings' saved position and the topmost flag, read at open.
    position: sharex_settings::types::Point,
    topmost: bool,
    /// The width last asked for (`SizeToContent`).
    width: f32,
    /// For `--render`: a button under the pointer.
    pub hover: Option<usize>,
}

impl ActionsToolbarWindow {
    pub fn new(settings: &ApplicationConfig) -> Self {
        Self {
            actions: settings.ActionsToolbarList.clone(),
            dragging: false,
            title: BRAND.to_owned(),
            position: settings.ActionsToolbarPosition,
            topmost: settings.ActionsToolbarStayTopMost,
            width: 0.0,
            hover: None,
        }
    }

    fn title_font() -> egui::FontId {
        font(13.0, 600)
    }

    fn width_of(&self, ui: &egui::Ui) -> f32 {
        let title = TITLE_PADDING * 2.0 + text_width(ui, &self.title, Self::title_font());
        let items: f32 = self.actions.iter().map(|a| if *a == HotkeyType::None { 7.0 } else { BUTTON }).sum();
        2.0 * FRAME + title + (SPACING + 0.0) * self.actions.len() as f32 + items
    }
}

impl Dialog for ActionsToolbarWindow {
    fn spec(&self) -> Spec {
        Spec {
            topmost: self.topmost,
            skip_taskbar: true,
            maximizable: false,
            borderless: true,
            ..Spec::fit_height(crate::rs_title(s::ActionsToolbarWindow_Title()), self.width.max(1.0), 0.0)
        }
    }

    fn single(&self) -> Option<&'static str> {
        Some("actions-toolbar")
    }

    fn content_width(&mut self, ui: &mut egui::Ui) -> Option<f32> {
        self.width = self.width_of(ui);
        Some(self.width)
    }

    fn content_height(&mut self, _ui: &mut egui::Ui) -> f32 {
        2.0 * FRAME + BUTTON
    }

    /// `RestorePosition`: the saved position when it's on a work area,
    /// else the bottom-right corner of the pointer's monitor's.
    fn position(&self, _work_area: sharex_capture::Rect, size: (u32, u32)) -> Option<(i32, i32)> {
        let screens = sharex_capture::screenshot::screens();
        let (x, y) = (self.position.x, self.position.y);
        if (x != 0 || y != 0) && screens.iter().any(|s| s.work_area.contains(x, y)) {
            return Some((x, y));
        }
        let cursor = sharex_capture::screenshot::cursor_position().unwrap_or((0, 0));
        let screen = screens.iter().find(|s| s.bounds.contains(cursor.0, cursor.1)).or(screens.iter().find(|s| s.primary)).or(screens.first())?;
        let a = screen.work_area;
        Some((a.x + a.w - size.0 as i32, a.y + a.h - size.1 as i32))
    }

    /// `ClampAndSavePosition`'s saving part.
    fn moved(&mut self, x: i32, y: i32, cx: &mut Cx) {
        cx.settings.ActionsToolbarPosition = sharex_settings::types::Point::new(x, y);
    }

    fn drag(&mut self, hover: bool) {
        self.dragging = hover;
    }

    /// `OnDrop`: `UploadManager.DragDropUpload`.
    fn dropped(&mut self, files: Vec<PathBuf>, cx: &mut Cx) -> Vec<Effect> {
        self.dragging = false;
        let task = TaskSettings::default_task_settings(&cx.settings.DefaultTaskSettings);
        vec![Effect::Upload(Box::new(UploadRequest::Files { files, task, confirmed: false }))]
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Cx) -> Vec<Effect> {
        let p = palette(ui.ctx());
        let mut out = Vec::new();
        if self.actions != cx.settings.ActionsToolbarList {
            self.actions = cx.settings.ActionsToolbarList.clone();
        }
        let width = self.width_of(ui);
        if (width - self.width).abs() > 0.5 {
            self.width = width;
            out.push(Effect::SetInnerSize(width, 2.0 * FRAME + BUTTON));
        }
        let full = ui.max_rect();
        ui.painter().rect_filled(full, 0.0, p.background_toolbar);
        let frame = Rect::from_min_size(full.min, vec2(width, 2.0 * FRAME + BUTTON));
        border::border(ui.painter(), frame, 5.0, 1.0, Some(Brush::Solid(p.background_toolbar)), Some(p.border));
        let top = frame.min.y + FRAME;
        let mut x = frame.min.x + FRAME;

        // The title: drag handle, context menu, middle click closes.
        let tf = Self::title_font();
        let tw = TITLE_PADDING * 2.0 + text_width(ui, &self.title, tf.clone());
        let title = Rect::from_min_size(pos2(x, top), vec2(tw, BUTTON));
        let tr = ui.interact(title, ui.id().with("title"), Sense::click_and_drag());
        let locked = cx.settings.ActionsToolbarLockPosition;
        let tr = tr.on_hover_cursor(if locked { egui::CursorIcon::Default } else { egui::CursorIcon::Move });
        if tr.hovered() {
            border::border(ui.painter(), title, 4.0, 0.0, Some(Brush::Solid(p.control_background_hover)), None);
        }
        ui.painter().galley(pos2(title.min.x + TITLE_PADDING, center_y(title, line_height(ui, &tf))), galley(ui, &self.title, tf, p.text_secondary, f32::INFINITY), p.text_secondary);
        if tr.is_pointer_button_down_on() && ui.input(|i| i.pointer.primary_pressed()) && !locked {
            out.push(Effect::DragWindow);
        }
        if tr.middle_clicked() {
            out.push(Effect::Close);
            return out;
        }
        if tr.secondary_clicked() {
            out.push(Effect::ActionsToolbarMenu);
        }
        x += tw + SPACING;

        // The actions.
        for (i, a) in self.actions.clone().iter().enumerate() {
            if *a == HotkeyType::None {
                // A separator: 1x22 with `Margin="3,0"`, whose background
                // `this.FindResource("ShareX.Brush.Border")` looks up before
                // the window has its theme: null, so ShareX shows only the gap.
                x += 7.0 + SPACING;
                continue;
            }
            let rect = Rect::from_min_size(pos2(x, top), Vec2::splat(BUTTON));
            let r = ui.interact(rect, ui.id().with(("action", i)), Sense::click());
            let pressed = r.is_pointer_button_down_on();
            let over = r.hovered() || self.hover == Some(i);
            if pressed {
                border::border(ui.painter(), rect, 4.0, 0.0, Some(Brush::Solid(p.control_background)), None);
            } else if over {
                border::border(ui.painter(), rect, 4.0, 0.0, Some(Brush::Solid(p.control_background_hover)), None);
            }
            // Centred as layout rounding places it (8.5 to even: 8).
            let off = ((BUTTON - 17.0) / 2.0).round_ties_even();
            icons::paint(ui.painter(), Rect::from_min_size(rect.min + Vec2::splat(off), Vec2::splat(17.0)), find_menu_icon(a), p.accent_icon);
            if r.clicked() {
                // `TaskHelpers.ExecuteJob(action)`.
                out.push(Effect::RunJob(a.clone()));
            }
            x += BUTTON + SPACING;
        }

        if self.dragging {
            // `DropOverlay`.
            let overlay = frame.shrink(FRAME);
            border::border(ui.painter(), overlay, 4.0, 0.0, Some(Brush::Solid(p.overlay_modal)), None);
            let text = s::ActionsToolbarWindow_DropToUpload();
            let bf = font(13.0, 400);
            let w = 16.0 + 7.0 + text_width(ui, text, bf.clone());
            let x = overlay.min.x + ((overlay.width() - w) / 2.0).round_ties_even();
            icons::paint(ui.painter(), Rect::from_min_size(pos2(x, center_y(overlay, 16.0)), Vec2::splat(16.0)), icon::Upload, p.accent_icon);
            ui.painter().galley(pos2(x + 23.0, center_y(overlay, line_height(ui, &bf))), galley(ui, text, bf, p.text_secondary, f32::INFINITY), p.text_secondary);
        }
        out
    }

    /// `ActionsToolbarWindowIntegration`: closing saves the settings.
    fn closed(&mut self, _cx: &mut Cx) -> Vec<Effect> {
        vec![Effect::SaveSettings]
    }
}

/// `ActionsToolbarEditorWindow`: 620x570, at least 520x430.
pub struct ActionsToolbarEditorWindow {
    pub selected: Option<usize>,
    add_menu: Option<Flyout<HotkeyType>>,
    /// For `--render`: the add menu open.
    pub open_add_menu: bool,
}

impl Default for ActionsToolbarEditorWindow {
    fn default() -> Self {
        Self::new()
    }
}

/// `BuildActionMenu`: every job (`None` as the separator), the uncategorised
/// ones first, then a submenu per category.
pub fn action_menu() -> Vec<MenuEntry<HotkeyType>> {
    let item = |h: &HotkeyType| {
        let title = if *h == HotkeyType::None { s::ActionsToolbarEditorWindow_Separator().to_owned() } else { h.localized_description() };
        MenuEntry::item(title, "", h.clone()).icon(MenuIcon::Plain(find_menu_icon(h), 15.0))
    };
    let mut values: Vec<&HotkeyType> = HotkeyType::ALL.iter().collect();
    values.sort_by_key(|h| h.value().unwrap_or(i64::MAX));
    let mut root = Vec::new();
    let mut groups: Vec<(String, Vec<MenuEntry<HotkeyType>>)> = Vec::new();
    for h in values {
        match h.localized_category() {
            None => root.push(item(h)),
            Some(c) => match groups.iter_mut().find(|(k, _)| *k == c) {
                Some((_, items)) => items.push(item(h)),
                None => groups.push((c, vec![item(h)])),
            },
        }
    }
    root.extend(groups.into_iter().map(|(c, items)| MenuEntry::parent(c, "", items)));
    root
}

impl ActionsToolbarEditorWindow {
    pub fn new() -> Self {
        Self { selected: Some(0), add_menu: None, open_add_menu: false }
    }

    fn tool_button(ui: &egui::Ui, rect: Rect, id: Id, key: &str, text: &str, enabled: bool) -> egui::Response {
        let p = palette(ui.ctx());
        let b = Button { padding: Thickness::hv(9.0, 4.0), ..Button::default() };
        let r = b.show(ui, rect, id, "", enabled);
        let inner = rect.shrink2(vec2(10.0, 5.0));
        let bf = font(13.0, 400);
        let content = 13.0 + 7.0 + text_width(ui, text, bf.clone());
        let x = inner.min.x + ((inner.width() - content) / 2.0).round_ties_even().max(0.0);
        let ic = b.foreground(ui, &r, enabled, p.background_panel);
        icons::paint(ui.painter(), Rect::from_min_size(pos2(x, center_y(inner, 13.0)), Vec2::splat(13.0)), key, ic);
        let tc = b.content(ui, enabled, p.text_secondary, p.background_panel);
        ui.painter().galley(pos2(x + 20.0, center_y(inner, line_height(ui, &bf))), galley(ui, text, bf, tc, f32::INFINITY), tc);
        r
    }

    fn square(ui: &egui::Ui, rect: Rect, id: Id, key: &str, enabled: bool) -> egui::Response {
        let p = palette(ui.ctx());
        let b = Button { padding: Thickness::all(0.0), ..Button::default() };
        let r = b.show(ui, rect, id, "", enabled);
        let ic = b.foreground(ui, &r, enabled, p.background_panel);
        icons::paint(ui.painter(), Rect::from_center_size(rect.center(), Vec2::splat(16.0)), key, ic);
        r
    }
}

impl Dialog for ActionsToolbarEditorWindow {
    fn spec(&self) -> Spec {
        Spec::sized(crate::rs_title(s::ActionsToolbarEditorWindow_Title()), 620.0, 570.0, 520.0, 430.0)
    }

    fn single(&self) -> Option<&'static str> {
        Some("actions-toolbar-editor")
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Cx) -> Vec<Effect> {
        let p = palette(ui.ctx());
        let full = super::layout::background(ui);
        let list = &mut cx.settings.ActionsToolbarList;
        if self.selected.is_some_and(|i| i >= list.len()) {
            self.selected = list.len().checked_sub(1);
        }
        let mut out = Vec::new();
        let mut changed = false;
        let left = full.min.x + 12.0;
        let right = full.max.x - 12.0;
        let bar = Rect::from_min_max(pos2(left, full.min.y), pos2(right, full.min.y + 8.0 + 34.0 + 8.0 + 1.0));
        docked_toolbar(ui, bar);
        let top = bar.min.y + 8.0;
        let mut x = bar.min.x + 9.0;
        let w_of = |t: &str| 2.0 + 18.0 + 13.0 + 7.0 + text_width(ui, t, font(13.0, 400));
        let add_text = s::ActionsToolbarEditorWindow_Add();
        let add_rect = Rect::from_min_size(pos2(x, top), vec2(w_of(add_text), 34.0));
        let modal = self.add_menu.is_some();
        if Self::tool_button(ui, add_rect, ui.id().with("add"), icon::Plus, add_text, true).clicked() && !modal || (self.open_add_menu && self.add_menu.is_none()) {
            self.open_add_menu = false;
            let mut f = Flyout::menu(Id::new("toolbar-add-menu"), action_menu(), add_rect, Placement::BottomEdgeAlignedLeft);
            f.style = MenuStyle::Standard;
            self.add_menu = Some(f);
        }
        x = add_rect.max.x + 6.0;
        let remove_text = s::ActionsToolbarEditorWindow_Remove();
        let remove_rect = Rect::from_min_size(pos2(x, top), vec2(w_of(remove_text), 34.0));
        let index = self.selected;
        if Self::tool_button(ui, remove_rect, ui.id().with("remove"), icon::Trash, remove_text, index.is_some()).clicked()
            && !modal
            && let Some(i) = index.filter(|i| *i < list.len())
        {
            // `OnRemoveClick`.
            list.remove(i);
            self.selected = if list.is_empty() { None } else { Some(i.min(list.len() - 1)) };
            changed = true;
        }
        x = remove_rect.max.x + 6.0;
        let index = self.selected;
        let up = index.is_some_and(|i| i > 0);
        let down = index.is_some_and(|i| i + 1 < list.len());
        let mut moved = None;
        if Self::square(ui, Rect::from_min_size(pos2(x, top), Vec2::splat(34.0)), ui.id().with("up"), icon::ChevronUp, up).clicked() && !modal {
            moved = Some(-1);
        }
        x += 40.0;
        if Self::square(ui, Rect::from_min_size(pos2(x, top), Vec2::splat(34.0)), ui.id().with("down"), icon::ChevronDown, down).clicked() && !modal {
            moved = Some(1);
        }
        if let (Some(offset), Some(i)) = (moved, index) {
            let j = i as i32 + offset;
            if j >= 0 && (j as usize) < list.len() {
                list.swap(i, j as usize);
                self.selected = Some(j as usize);
                changed = true;
            }
        }

        // Close.
        let foot_top = full.max.y - 12.0 - 34.0;
        let close = Button::default().show(ui, Rect::from_min_size(pos2(right - 88.0, foot_top), vec2(88.0, 34.0)), ui.id().with("close"), s::ActionsToolbarEditorWindow_Close(), true).clicked();

        // The list (`Border.action-row`: 36 high, `Padding="8,4"`, in a
        // `Padding="2"` item).
        let area = Rect::from_min_max(pos2(left, bar.max.y + 8.0), pos2(right, foot_top - 8.0));
        border::border(ui.painter(), area, 0.0, 1.0, Some(Brush::Solid(p.background_main)), Some(p.border));
        let view = area.shrink(1.0);
        let row_h = 4.0 + 36.0;
        let content = row_h * list.len() as f32;
        let scroll = super::layout::scroll_offset(ui, ui.id().with("list-scroll"), view, content);
        let clip = ui.clip_rect();
        ui.set_clip_rect(view.intersect(clip));
        let hf = font(12.0, 400);
        let tf = font(13.0, 400);
        for (i, a) in list.iter().enumerate() {
            let item = Rect::from_min_size(pos2(view.min.x, view.min.y + row_h * i as f32 - scroll), vec2(view.width(), row_h));
            if !item.intersects(view) {
                continue;
            }
            let r = ui.interact(item.intersect(view), ui.id().with(("row", i)), if modal { Sense::hover() } else { Sense::click() });
            if r.clicked() {
                self.selected = Some(i);
            }
            let row = item.shrink(2.0);
            if self.selected == Some(i) || (!modal && r.hovered()) {
                border::border(ui.painter(), row, 4.0, 0.0, Some(Brush::Solid(p.control_background_hover)), None);
            }
            let inner = row.shrink2(vec2(8.0, 4.0));
            if *a == HotkeyType::None {
                let label = s::ActionsToolbarEditorWindow_Separator();
                let lw = text_width(ui, label, hf.clone());
                let side = ((inner.width() - lw - 20.0) / 2.0).max(0.0);
                let cy = center_y(inner, 1.0);
                ui.painter().rect_filled(Rect::from_min_size(pos2(inner.min.x, cy), vec2(side, 1.0)), 0.0, p.border);
                ui.painter().rect_filled(Rect::from_min_max(pos2(inner.min.x + side + 20.0 + lw, cy), pos2(inner.max.x, cy + 1.0)), 0.0, p.border);
                ui.painter().galley(pos2(inner.min.x + side + 10.0, center_y(inner, line_height(ui, &hf))), galley(ui, label, hf.clone(), p.text_secondary, f32::INFINITY), p.text_secondary);
            } else {
                icons::paint(ui.painter(), Rect::from_min_size(pos2(inner.min.x, center_y(inner, 16.0)), Vec2::splat(16.0)), find_menu_icon(a), p.accent_icon);
                let tx = inner.min.x + 24.0 + 7.0;
                ui.painter().galley(pos2(tx, center_y(inner, line_height(ui, &tf))), galley(ui, &a.localized_description(), tf.clone(), p.text_secondary, inner.max.x - tx), p.text_secondary);
            }
        }
        ui.set_clip_rect(clip);
        super::layout::scroll_bar(ui, view, content, scroll, 0.0);

        // The add menu.
        if let Some(f) = self.add_menu.as_mut() {
            let o = f.show(ui, full);
            if let Some(action) = o.command {
                // `AddAction`.
                list.push(action);
                self.selected = Some(list.len() - 1);
                changed = true;
            }
            if o.closed {
                self.add_menu = None;
            }
        } else {
            let (escape, delete) = ui.input(|i| (i.key_pressed(egui::Key::Escape), i.key_pressed(egui::Key::Delete)));
            if delete && let Some(i) = self.selected.filter(|i| *i < list.len()) {
                list.remove(i);
                self.selected = if list.is_empty() { None } else { Some(i.min(list.len() - 1)) };
                changed = true;
            }
            if close || escape {
                out.push(Effect::Close);
            }
        }
        if changed {
            // `NotifyChanged`: the toolbar follows, the settings are saved.
            out.push(Effect::RefreshActionsToolbar);
            out.push(Effect::SaveSettings);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_add_menu_is_sharex_s() {
        let m = action_menu();
        let headers: Vec<&str> = m.iter().map(|e| e.header.as_str()).collect();
        assert_eq!(headers, ["Separator", "Upload", "Screen capture", "Screen record", "Tools", "Other"]);
    }
}
