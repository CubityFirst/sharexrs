//! `ShareX/Presentation/QuickTaskMenuEditor`: `QuickTaskMenuEditorWindow`,
//! the quick task menu's presets (`ApplicationConfig.QuickTaskPresets`):
//! add, edit (an overlay with the menu text and the task check boxes),
//! remove, move, reset to the defaults (after an overlay's confirmation).
//! Every change is saved at once. 760x640, at least 620x500.

use egui::{Id, Rect, Sense, Vec2, pos2, vec2};
use sharex_settings::{AfterCaptureTasks, AfterUploadTasks, QuickTaskInfo};
use sharex_strings::sharex as s;
use sharex_ui::border::{self, Brush};
use sharex_ui::controls::{Button, CheckBox, TextBox, content_font, wrap_lines};
use sharex_ui::keys::app_icons as icon;
use sharex_ui::theme::{Thickness, fluent, palette};
use sharex_ui::widgets::{center_y, font, galley, line_height, text_width};
use sharex_ui::icons;

use super::{Cx, Dialog, Effect, Spec};
use crate::history_ui::common::docked_toolbar;
use crate::quick_task::display_name;

const MARGIN: f32 = 12.0;
const ROW_SPACING: f32 = 8.0;
/// `Button.compact`: `Height="34"`, `Padding="9,4"`; `Button.square` 34.
const BUTTON_HEIGHT: f32 = 34.0;
const BUTTON_PADDING: f32 = 9.0;
const TOOLBAR_SPACING: f32 = 6.0;
/// A list item: `Padding="2"` around a `Border.quick-task-row`
/// (`MinHeight="42"`, `Padding="8,5"`).
const ITEM_PADDING: f32 = 2.0;
const ROW_PADDING: (f32, f32) = (8.0, 5.0);
const ROW_MIN_HEIGHT: f32 = 42.0;

/// The item editor overlay's state.
pub struct ItemEditor {
    /// The preset edited, or `None` for a new one.
    pub index: Option<usize>,
    pub name: String,
    pub capture: AfterCaptureTasks,
    pub upload: AfterUploadTasks,
    opened: bool,
}

pub struct QuickTaskMenuEditorWindow {
    pub selected: Option<usize>,
    pub editor: Option<ItemEditor>,
    /// `ResetConfirmationOverlay.IsVisible`.
    pub confirm_reset: bool,
}

impl Default for QuickTaskMenuEditorWindow {
    fn default() -> Self {
        Self::new()
    }
}

fn flag_names(value: u64, names: &[(u64, &'static str)], enum_type: &str) -> Vec<String> {
    names
        .iter()
        .filter(|(b, _)| *b != 0 && value & b == *b)
        .map(|(_, n)| sharex_strings::helpers_lib::lookup(&format!("{enum_type}_{n}")).unwrap_or(n).to_owned())
        .collect()
}

/// `QuickTaskPresetItem.Title`.
pub fn title(info: &QuickTaskInfo) -> String {
    if info.is_valid() { display_name(info) } else { s::QuickTaskMenuEditorWindow_Separator().to_owned() }
}

/// `QuickTaskPresetItem.Summary`.
pub fn summary(info: &QuickTaskInfo) -> String {
    if !info.is_valid() {
        return String::new();
    }
    let capture = flag_names(info.AfterCaptureTasks.0 as u64, AfterCaptureTasks::NAMES, "AfterCaptureTasks").join(", ");
    let upload = flag_names(info.AfterUploadTasks.0 as u64, AfterUploadTasks::NAMES, "AfterUploadTasks").join(", ");
    if upload.is_empty() { capture } else { s::QuickTaskMenuEditorWindow_AfterUploadSummary().replace("{0}", &capture).replace("{1}", &upload) }
}

impl QuickTaskMenuEditorWindow {
    pub fn new() -> Self {
        Self { selected: Some(0), editor: None, confirm_reset: false }
    }

    fn button_font() -> egui::FontId {
        font(13.0, 400)
    }

    fn hint_font() -> egui::FontId {
        font(12.0, 400)
    }

    fn button_width(ui: &egui::Ui, text: &str) -> f32 {
        2.0 + 2.0 * BUTTON_PADDING + 13.0 + 7.0 + text_width(ui, text, Self::button_font())
    }

    /// A toolbar button: icon and `TextBlock`, `Spacing="7"`.
    fn tool_button(ui: &egui::Ui, rect: Rect, id: Id, key: &str, text: &str, enabled: bool) -> egui::Response {
        let p = palette(ui.ctx());
        let b = Button { padding: Thickness::hv(BUTTON_PADDING, 4.0), ..Button::default() };
        let r = b.show(ui, rect, id, "", enabled);
        let inner = rect.shrink2(vec2(1.0 + BUTTON_PADDING, 5.0));
        let bf = Self::button_font();
        let content = 13.0 + 7.0 + text_width(ui, text, bf.clone());
        let x = inner.min.x + ((inner.width() - content) / 2.0).round_ties_even().max(0.0);
        let ic = b.foreground(ui, &r, enabled, p.background_panel);
        icons::paint(ui.painter(), Rect::from_min_size(pos2(x, center_y(inner, 13.0)), Vec2::splat(13.0)), key, ic);
        let tc = b.content(ui, enabled, p.text_secondary, p.background_panel);
        ui.painter().galley(pos2(x + 20.0, center_y(inner, line_height(ui, &bf))), galley(ui, text, bf, tc, f32::INFINITY), tc);
        r
    }

    /// `Button.square` with a 16px icon.
    fn square_button(ui: &egui::Ui, rect: Rect, id: Id, key: &str, enabled: bool) -> egui::Response {
        let p = palette(ui.ctx());
        let b = Button { padding: Thickness::all(0.0), ..Button::default() };
        let r = b.show(ui, rect, id, "", enabled);
        let ic = b.foreground(ui, &r, enabled, p.background_panel);
        icons::paint(ui.painter(), Rect::from_center_size(rect.center(), Vec2::splat(16.0)), key, ic);
        r
    }

    /// A `Button.compact` with string content (`MinWidth="84"`).
    fn text_button(ui: &egui::Ui, right: f32, top: f32, id: Id, text: &str, min_width: f32) -> (Rect, bool) {
        let b = Button { padding: Thickness::hv(BUTTON_PADDING, 4.0), ..Button::default() };
        let w = b.width(ui, text).max(min_width);
        let rect = Rect::from_min_size(pos2(right - w, top), vec2(w, BUTTON_HEIGHT));
        let clicked = b.show(ui, rect, id, text, true).clicked();
        (rect, clicked)
    }

    fn item_height(ui: &egui::Ui, info: &QuickTaskInfo) -> f32 {
        let content = if info.is_valid() { line_height(ui, &font(13.0, 400)) + 1.0 + line_height(ui, &Self::hint_font()) } else { line_height(ui, &Self::hint_font()) };
        2.0 * ITEM_PADDING + (content + 2.0 * ROW_PADDING.1).max(ROW_MIN_HEIGHT)
    }

    /// For `--render`: the item editor on the default presets.
    pub fn open_editor(&mut self, index: Option<usize>) {
        self.show_editor(&QuickTaskInfo::default_presets(), index);
    }

    fn show_editor(&mut self, presets: &[QuickTaskInfo], index: Option<usize>) {
        let task = index.and_then(|i| presets.get(i)).cloned().unwrap_or_default();
        self.editor = Some(ItemEditor { index, name: task.Name.clone().unwrap_or_default(), capture: task.AfterCaptureTasks, upload: task.AfterUploadTasks, opened: false });
    }

    /// `OnSaveItemClick`.
    fn save_editor(&mut self, presets: &mut Vec<QuickTaskInfo>) -> Vec<Effect> {
        let Some(e) = self.editor.take() else { return Vec::new() };
        let index = match e.index.filter(|i| *i < presets.len()) {
            Some(i) => i,
            None => {
                presets.push(QuickTaskInfo::default());
                presets.len() - 1
            }
        };
        let task = &mut presets[index];
        task.Name = Some(e.name);
        task.AfterCaptureTasks = e.capture;
        task.AfterUploadTasks = e.upload;
        self.selected = Some(index);
        vec![Effect::SaveSettings]
    }

    /// `OnRemoveClick`.
    fn remove(&mut self, presets: &mut Vec<QuickTaskInfo>) -> Vec<Effect> {
        let Some(i) = self.selected.filter(|i| *i < presets.len()) else { return Vec::new() };
        presets.remove(i);
        self.selected = if presets.is_empty() { None } else { Some(i.min(presets.len() - 1)) };
        vec![Effect::SaveSettings]
    }

    /// `MoveSelected(offset)`.
    fn move_selected(&mut self, presets: &mut [QuickTaskInfo], offset: i32) -> Vec<Effect> {
        let Some(i) = self.selected else { return Vec::new() };
        let j = i as i32 + offset;
        if j < 0 || j as usize >= presets.len() {
            return Vec::new();
        }
        presets.swap(i, j as usize);
        self.selected = Some(j as usize);
        vec![Effect::SaveSettings]
    }

    /// The item editor overlay (700 wide, at most 600 tall).
    fn editor_ui(&mut self, ui: &mut egui::Ui, full: Rect, presets: &mut Vec<QuickTaskInfo>) -> Vec<Effect> {
        let p = palette(ui.ctx());
        let f = fluent(ui.ctx());
        ui.painter().rect_filled(full, 0.0, p.overlay_modal);
        let width = 700.0_f32.min(full.width());
        let height = 600.0_f32.min(full.height());
        let card = Rect::from_min_size(pos2(full.min.x + ((full.width() - width) / 2.0).round_ties_even(), full.min.y + ((full.height() - height) / 2.0).round_ties_even()), vec2(width, height));
        border::border(ui.painter(), card, 8.0, 1.0, Some(Brush::Solid(p.background_panel)), Some(p.border));
        let inner = card.shrink(17.0);
        let Some(e) = self.editor.as_mut() else { return Vec::new() };
        // The header: the workflows icon, the title and the hint.
        let tf = font(19.0, 600);
        let hf = Self::hint_font();
        let (th, hh) = (line_height(ui, &tf), line_height(ui, &hf));
        let head_h = th + 1.0 + hh;
        icons::paint(ui.painter(), Rect::from_min_size(pos2(inner.min.x, inner.min.y + ((head_h - 21.0) / 2.0).round_ties_even()), Vec2::splat(21.0)), icon::Workflows, p.accent_icon);
        let tx = inner.min.x + 21.0 + 9.0;
        let title = if e.index.is_none() { s::QuickTaskMenuEditorWindow_AddQuickTask() } else { s::QuickTaskMenuEditorWindow_EditQuickTask() };
        ui.painter().galley(pos2(tx, inner.min.y), galley(ui, title, tf, p.text_secondary, inner.max.x - tx), p.text_secondary);
        ui.painter().galley(pos2(tx, inner.min.y + th + 1.0), galley(ui, s::QuickTaskMenuEditorWindow_ChooseActions(), hf.clone(), p.text_secondary, inner.max.x - tx), p.text_secondary);
        // The buttons.
        let buttons_top = inner.max.y - BUTTON_HEIGHT;
        let (cancel_rect, cancel) = Self::text_button(ui, inner.max.x, buttons_top, ui.id().with("item-cancel"), s::QuickTaskMenuEditorWindow_Cancel(), 84.0);
        let (_, save) = Self::text_button(ui, cancel_rect.min.x - 8.0, buttons_top, ui.id().with("item-save"), s::QuickTaskMenuEditorWindow_Save(), 84.0);
        // The scrolling form.
        let view = Rect::from_min_max(pos2(inner.min.x, inner.min.y + head_h + 12.0), pos2(inner.max.x, buttons_top - 12.0));
        let w = view.width() - 4.0;
        let lf = font(13.0, 400);
        let lh = line_height(ui, &lf);
        let box_h = (line_height(ui, &TextBox::font()) + 8.0 + 2.0).max(32.0);
        let cf = font(14.0, 600);
        let ch = line_height(ui, &cf);
        let captures: Vec<(u64, &str)> = AfterCaptureTasks::NAMES.iter().filter(|(b, _)| *b != 0).copied().collect();
        let uploads: Vec<(u64, &str)> = AfterUploadTasks::NAMES.iter().filter(|(b, _)| *b != 0).copied().collect();
        let column_h = |n: usize| 22.0 + ch + 5.0 + 34.0 * n as f32;
        let content = lh + 3.0 + box_h + 3.0 + hh + 10.0 + column_h(captures.len()).max(column_h(uploads.len()));
        let scroll = super::layout::scroll_offset(ui, ui.id().with("item-scroll"), view, content);
        let clip = ui.clip_rect();
        ui.set_clip_rect(view.intersect(clip));
        let mut y = view.min.y - scroll;
        ui.painter().galley(pos2(view.min.x, y), galley(ui, s::QuickTaskMenuEditorWindow_MenuText(), lf.clone(), p.text_secondary, w), p.text_secondary);
        y += lh + 3.0;
        // The generated name is the placeholder (`UpdateEditorPreview`).
        let generated = display_name(&QuickTaskInfo::new(None, e.capture, e.upload));
        let placeholder = if generated.is_empty() { s::QuickTaskMenuEditorWindow_Separator().to_owned() } else { generated };
        let box_rect = Rect::from_min_size(pos2(view.min.x, y), vec2(w, box_h));
        let box_id = ui.id().with("item-name");
        if !e.opened {
            e.opened = true;
            super::text_upload::focus_text_box(ui, box_id, &e.name, true);
        }
        TextBox { padding: Thickness::hv(8.0, 4.0), placeholder: &placeholder, ..TextBox::default() }.show(ui, box_rect, box_id, &mut e.name);
        y += box_h + 3.0;
        ui.painter().galley(pos2(view.min.x, y), galley(ui, s::QuickTaskMenuEditorWindow_MenuTextHint(), hf.clone(), p.text_secondary, w), p.text_secondary);
        y += hh + 10.0;
        let col_w = ((w - 8.0) / 2.0).floor();
        for (c, (heading, list)) in [(s::QuickTaskMenuEditorWindow_AfterCaptureTasks(), &captures), (s::QuickTaskMenuEditorWindow_AfterUploadTasks(), &uploads)].into_iter().enumerate() {
            let x = view.min.x + (col_w + 8.0) * c as f32;
            let col = Rect::from_min_size(pos2(x, y), vec2(if c == 0 { col_w } else { w - col_w - 8.0 }, column_h(captures.len().max(uploads.len()))));
            border::border(ui.painter(), col, 5.0, 1.0, Some(Brush::Solid(p.control_background)), Some(p.border));
            let ci = col.shrink(11.0);
            ui.painter().galley(ci.min, galley(ui, heading, cf.clone(), p.text_secondary, ci.width()), p.text_secondary);
            let mut cy = ci.min.y + ch + 5.0;
            for (k, (bit, name)) in list.iter().enumerate() {
                cy += 1.0;
                let enum_type = if c == 0 { "AfterCaptureTasks" } else { "AfterUploadTasks" };
                let text = sharex_strings::helpers_lib::lookup(&format!("{enum_type}_{name}")).unwrap_or(name).to_owned();
                let mut checked = if c == 0 { e.capture.0 as u64 & bit == *bit } else { e.upload.0 as u64 & bit == *bit };
                let rect = Rect::from_min_size(pos2(ci.min.x, cy), vec2(ci.width(), 32.0));
                let cb = CheckBox { text: "", text_font: content_font(13.0, 400), text_color: None, min_height: 32.0, enabled: true };
                let r = cb.show(ui, rect, ui.id().with(("flag", c, k)), &mut checked);
                let over = r.hovered() || r.is_pointer_button_down_on();
                let color = if checked || over { f.base_high } else { p.text };
                let font = content_font(13.0, 400);
                ui.painter().galley(pos2(rect.min.x + 28.0, center_y(rect, line_height(ui, &font))), galley(ui, &text, font, color, rect.width() - 28.0), color);
                if r.changed() {
                    if c == 0 {
                        if checked { e.capture.insert(AfterCaptureTasks(*bit as u32)) } else { e.capture.remove(AfterCaptureTasks(*bit as u32)) }
                    } else if checked {
                        e.upload.insert(AfterUploadTasks(*bit as u32));
                    } else {
                        e.upload.remove(AfterUploadTasks(*bit as u32));
                    }
                }
                cy += 33.0;
            }
        }
        ui.set_clip_rect(clip);
        super::layout::scroll_bar(ui, view, content, scroll, 0.0);
        if save {
            return self.save_editor(presets);
        }
        if cancel || sharex_ui::popup::escape_pressed(ui.ctx()) {
            self.editor = None;
        }
        Vec::new()
    }

    /// The reset confirmation overlay (420 wide).
    fn reset_ui(&mut self, ui: &mut egui::Ui, full: Rect, presets: &mut Vec<QuickTaskInfo>) -> Vec<Effect> {
        let p = palette(ui.ctx());
        ui.painter().rect_filled(full, 0.0, p.overlay_modal);
        let width = 420.0_f32.min(full.width());
        let tf = font(18.0, 600);
        let bf = font(13.0, 400);
        let text_w = width - 2.0 * 19.0;
        let lines = wrap_lines(ui, s::QuickTaskMenuEditorWindow_ResetConfirmation(), &bf, text_w);
        let (th, bh) = (line_height(ui, &tf), line_height(ui, &bf));
        let height = 2.0 + 36.0 + th.max(20.0) + 12.0 + bh * lines.len().max(1) as f32 + 12.0 + BUTTON_HEIGHT;
        let card = Rect::from_min_size(pos2(full.min.x + ((full.width() - width) / 2.0).round_ties_even(), full.min.y + ((full.height() - height) / 2.0).round_ties_even()), vec2(width, height));
        border::border(ui.painter(), card, 8.0, 1.0, Some(Brush::Solid(p.background_panel)), Some(p.border));
        let inner = card.shrink(19.0);
        let head = Rect::from_min_size(inner.min, vec2(inner.width(), th.max(20.0)));
        icons::paint(ui.painter(), Rect::from_min_size(pos2(head.min.x, center_y(head, 20.0)), Vec2::splat(20.0)), icon::Reset, p.accent_icon);
        ui.painter().galley(pos2(head.min.x + 29.0, head.min.y), galley(ui, s::QuickTaskMenuEditorWindow_ResetPrompt(), tf, p.text_secondary, inner.width() - 29.0), p.text_secondary);
        for (i, line) in lines.iter().enumerate() {
            ui.painter().galley(pos2(inner.min.x, head.max.y + 12.0 + bh * i as f32), galley(ui, line, bf.clone(), p.text_secondary, f32::INFINITY), p.text_secondary);
        }
        let top = inner.max.y - BUTTON_HEIGHT;
        let (cancel_rect, cancel) = Self::text_button(ui, inner.max.x, top, ui.id().with("reset-cancel"), s::QuickTaskMenuEditorWindow_Cancel(), 84.0);
        let (_, reset) = Self::text_button(ui, cancel_rect.min.x - 8.0, top, ui.id().with("reset-ok"), s::QuickTaskMenuEditorWindow_ResetAction(), 84.0);
        if reset {
            // `OnConfirmResetClick`.
            *presets = QuickTaskInfo::default_presets();
            self.confirm_reset = false;
            self.selected = (!presets.is_empty()).then_some(0);
            return vec![Effect::SaveSettings];
        }
        if cancel || sharex_ui::popup::escape_pressed(ui.ctx()) {
            self.confirm_reset = false;
        }
        Vec::new()
    }
}

impl Dialog for QuickTaskMenuEditorWindow {
    fn spec(&self) -> Spec {
        Spec::sized(crate::rs_title(s::QuickTaskMenuEditorWindow_Title()), 760.0, 640.0, 620.0, 500.0)
    }

    fn single(&self) -> Option<&'static str> {
        Some("quick-task-editor")
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Cx) -> Vec<Effect> {
        let p = palette(ui.ctx());
        let full = super::layout::background(ui);
        let presets = &mut cx.settings.QuickTaskPresets;
        if self.selected.is_some_and(|i| i >= presets.len()) {
            self.selected = presets.len().checked_sub(1);
        }
        let modal = self.editor.is_some() || self.confirm_reset;
        let mut out = Vec::new();
        let left = full.min.x + MARGIN;
        let right = full.max.x - MARGIN;

        // The toolbar.
        let bar = Rect::from_min_max(pos2(left, full.min.y), pos2(right, full.min.y + 8.0 + BUTTON_HEIGHT + 8.0 + 1.0));
        docked_toolbar(ui, bar);
        let top = bar.min.y + 8.0;
        let mut x = bar.min.x + 1.0 + 8.0;
        let index = self.selected;
        let mut clicked = None;
        for (k, (key, text, enabled)) in [
            (icon::Plus, s::QuickTaskMenuEditorWindow_Add(), true),
            (icon::Settings, s::QuickTaskMenuEditorWindow_Edit(), index.is_some()),
            (icon::Trash, s::QuickTaskMenuEditorWindow_Remove(), index.is_some()),
        ]
        .into_iter()
        .enumerate()
        {
            let w = Self::button_width(ui, text);
            if Self::tool_button(ui, Rect::from_min_size(pos2(x, top), vec2(w, BUTTON_HEIGHT)), ui.id().with(("tool", k)), key, text, enabled).clicked() {
                clicked = Some(k);
            }
            x += w + TOOLBAR_SPACING;
        }
        let up = index.is_some_and(|i| i > 0);
        let down = index.is_some_and(|i| i + 1 < presets.len());
        if Self::square_button(ui, Rect::from_min_size(pos2(x, top), Vec2::splat(BUTTON_HEIGHT)), ui.id().with("up"), icon::ChevronUp, up).clicked() {
            clicked = Some(3);
        }
        x += BUTTON_HEIGHT + TOOLBAR_SPACING;
        if Self::square_button(ui, Rect::from_min_size(pos2(x, top), Vec2::splat(BUTTON_HEIGHT)), ui.id().with("down"), icon::ChevronDown, down).clicked() {
            clicked = Some(4);
        }
        let reset_text = s::QuickTaskMenuEditorWindow_Reset();
        let rw = Self::button_width(ui, reset_text);
        if Self::tool_button(ui, Rect::from_min_size(pos2(bar.max.x - 1.0 - 8.0 - rw, top), vec2(rw, BUTTON_HEIGHT)), ui.id().with("reset"), icon::Reset, reset_text, true).clicked() {
            clicked = Some(5);
        }

        // The footer: the hint and Close.
        let foot_top = full.max.y - MARGIN - BUTTON_HEIGHT;
        let close_text = s::QuickTaskMenuEditorWindow_Close();
        let close_rect = Rect::from_min_size(pos2(right - 88.0, foot_top), vec2(88.0, BUTTON_HEIGHT));
        let close = Button::default().show(ui, close_rect, ui.id().with("close"), close_text, true).clicked();
        let hf = Self::hint_font();
        let hint_w = close_rect.min.x - 10.0 - left;
        let hint = wrap_lines(ui, s::QuickTaskMenuEditorWindow_SeparatorHint(), &hf, hint_w);
        let hh = line_height(ui, &hf);
        let mut hy = center_y(Rect::from_min_size(pos2(left, foot_top), vec2(hint_w, BUTTON_HEIGHT)), hh * hint.len() as f32);
        for line in &hint {
            ui.painter().galley(pos2(left, hy), galley(ui, line, hf.clone(), p.text_secondary, f32::INFINITY), p.text_secondary);
            hy += hh;
        }

        // The list.
        let list = Rect::from_min_max(pos2(left, bar.max.y + ROW_SPACING), pos2(right, foot_top - ROW_SPACING));
        border::border(ui.painter(), list, 0.0, 1.0, Some(Brush::Solid(p.background_main)), Some(p.border));
        let view = list.shrink(1.0);
        let heights: Vec<f32> = presets.iter().map(|t| Self::item_height(ui, t)).collect();
        let content: f32 = heights.iter().sum();
        let scroll = super::layout::scroll_offset(ui, ui.id().with("list-scroll"), view, content);
        let clip = ui.clip_rect();
        ui.set_clip_rect(view.intersect(clip));
        let mut y = view.min.y - scroll;
        let mut double = None;
        for (i, t) in presets.iter().enumerate() {
            let item = Rect::from_min_size(pos2(view.min.x, y), vec2(view.width(), heights[i]));
            y += heights[i];
            if !item.intersects(view) {
                continue;
            }
            let r = ui.interact(item.intersect(view), ui.id().with(("item", i)), if modal { Sense::hover() } else { Sense::click() });
            if r.clicked() {
                self.selected = Some(i);
            }
            if r.double_clicked() {
                double = Some(i);
            }
            let row = item.shrink(ITEM_PADDING);
            if self.selected == Some(i) || (!modal && r.hovered()) {
                border::border(ui.painter(), row, 4.0, 0.0, Some(Brush::Solid(p.control_background_hover)), None);
            }
            let inner = row.shrink2(vec2(ROW_PADDING.0, ROW_PADDING.1));
            if t.is_valid() {
                let tf = font(13.0, 400);
                let th = line_height(ui, &tf);
                let stack = th + 1.0 + hh;
                let ty = center_y(inner, stack);
                ui.painter().galley(pos2(inner.min.x, ty), galley(ui, &title(t), tf, p.text_secondary, inner.width()), p.text_secondary);
                ui.painter().galley(pos2(inner.min.x, ty + th + 1.0), galley(ui, &summary(t), hf.clone(), p.text_secondary, inner.width()), p.text_secondary);
            } else {
                // A separator: a line, "Separator", a line.
                let label = s::QuickTaskMenuEditorWindow_Separator();
                let lw = text_width(ui, label, hf.clone());
                let side = ((inner.width() - lw - 20.0) / 2.0).max(0.0);
                let cy = center_y(inner, 1.0);
                ui.painter().rect_filled(Rect::from_min_size(pos2(inner.min.x, cy), vec2(side, 1.0)), 0.0, p.border);
                ui.painter().rect_filled(Rect::from_min_size(pos2(inner.min.x + side + 10.0 + lw + 10.0, cy), vec2(inner.max.x - (inner.min.x + side + 20.0 + lw), 1.0)), 0.0, p.border);
                ui.painter().galley(pos2(inner.min.x + side + 10.0, center_y(inner, hh)), galley(ui, label, hf.clone(), p.text_secondary, f32::INFINITY), p.text_secondary);
            }
        }
        ui.set_clip_rect(clip);
        super::layout::scroll_bar(ui, view, content, scroll, 0.0);

        if self.editor.is_some() {
            return self.editor_ui(ui, full, presets);
        }
        if self.confirm_reset {
            return self.reset_ui(ui, full, presets);
        }
        match clicked {
            Some(0) => self.show_editor(presets, None),
            Some(1) => self.show_editor(presets, self.selected),
            Some(2) => out.extend(self.remove(presets)),
            Some(3) => out.extend(self.move_selected(presets, -1)),
            Some(4) => out.extend(self.move_selected(presets, 1)),
            Some(5) => self.confirm_reset = true,
            _ => {}
        }
        if let Some(i) = double {
            self.show_editor(presets, Some(i));
        }
        let (escape, delete) = ui.input(|i| (i.key_pressed(egui::Key::Escape), i.key_pressed(egui::Key::Delete)));
        let escape = escape && !sharex_ui::popup::was_open(ui.ctx());
        if delete {
            out.extend(self.remove(presets));
        }
        if close || escape {
            out.push(Effect::Close);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_and_summaries() {
        let presets = QuickTaskInfo::default_presets();
        assert_eq!(title(&presets[0]), "Save, Upload, Copy URL");
        assert!(summary(&presets[0]).contains("Copy URL to clipboard"));
        assert_eq!(summary(&presets[1]), "Copy image to clipboard, Save image to file");
        assert_eq!(title(&presets[4]), s::QuickTaskMenuEditorWindow_Separator());
        assert_eq!(summary(&presets[4]), "");
    }

    #[test]
    fn editing_the_list() {
        let mut presets = QuickTaskInfo::default_presets();
        let mut w = QuickTaskMenuEditorWindow::new();
        w.selected = Some(1);
        w.move_selected(&mut presets, -1);
        assert_eq!(w.selected, Some(0));
        assert_eq!(presets[0].Name.as_deref(), Some("Save, Copy image"));
        w.show_editor(&presets, None);
        if let Some(e) = w.editor.as_mut() {
            e.capture = AfterCaptureTasks::SaveImageToFile;
        }
        w.save_editor(&mut presets);
        assert_eq!(presets.len(), 10);
        assert_eq!(w.selected, Some(9));
        assert_eq!(display_name(&presets[9]), "Save image to file");
        w.remove(&mut presets);
        assert_eq!(presets.len(), 9);
        assert_eq!(w.selected, Some(8));
    }
}
