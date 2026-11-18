//! The toolbar editor: `ToolbarCustomizationDialogView` over Task settings
//! (`TaskSettingsWindow.ShowImageEditorToolbarEditor` and
//! `ShowRegionCaptureToolbarEditor`, its `ToolbarEditorOverlay`): each item
//! moved up or down, starting a group, shown or hidden, with its two
//! hotkeys (click a box and press a shortcut; Backspace or Delete clears
//! it); Reset toolbar to default, OK (unless a hotkey is invalid or used
//! twice: `RefreshValidation`) and Cancel.

use egui::{Color32, CornerRadius, Id, Key, Rect, Sense, Ui, Vec2, pos2, vec2};
use serde_json::Value;
use sharex_capture::toolbar_items::{self as ti, Item, Mods, hotkey};
use sharex_strings::image_editor as ie;
use sharex_ui::border::{self, Brush};
use sharex_ui::controls::{Button, ToggleSwitch, content_font};
use sharex_ui::keys::app_icons as icon;
use sharex_ui::theme::{Thickness, fluent, palette};
use sharex_ui::{icons, widgets};

use crate::task_editors::Outcome;

/// Which toolbar the editor edits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    ImageEditor,
    RegionCapture,
}

/// `ToolbarCustomizationDialogViewModel`.
pub struct ToolbarEditor {
    pub target: Target,
    items: Vec<Item>,
    scroll: f32,
    /// The hotkey box being typed into: the item and whether it's the
    /// secondary one.
    focus: Option<(usize, bool)>,
}

const WIDTH: f32 = 720.0;
const MAX_HEIGHT: f32 = 620.0;
const ROW: f32 = 57.0;
const HEADER: f32 = 49.0;
const COLUMNS: f32 = 31.0;
const FOOTER: f32 = 61.0;
const BOX: f32 = 130.0;

/// Avalonia's `Key` name of an egui key (`Key.ToString()`).
fn key_name(k: Key) -> Option<String> {
    let n = k.name();
    if n.len() == 1 {
        let c = n.chars().next()?;
        if c.is_ascii_alphabetic() {
            return Some(c.to_ascii_uppercase().to_string());
        }
        if c.is_ascii_digit() {
            return Some(format!("D{c}"));
        }
    }
    Some(
        match k {
            Key::Space => "Space",
            Key::Enter => "Enter",
            Key::Tab => "Tab",
            Key::Escape => "Escape",
            Key::Insert => "Insert",
            Key::Home => "Home",
            Key::End => "End",
            Key::PageUp => "PageUp",
            Key::PageDown => "PageDown",
            Key::ArrowLeft => "Left",
            Key::ArrowUp => "Up",
            Key::ArrowRight => "Right",
            Key::ArrowDown => "Down",
            Key::Plus | Key::Equals => "OemPlus",
            Key::Minus => "OemMinus",
            Key::Comma => "OemComma",
            Key::Period => "OemPeriod",
            _ => {
                let f = n.strip_prefix('F')?.parse::<u32>().ok()?;
                return Some(format!("F{f}"));
            }
        }
        .to_string(),
    )
}

impl ToolbarEditor {
    /// The items as saved (`CreateFromOptions` / `RegionCaptureToolbar.CreateItems`).
    pub fn new(target: Target, saved: &[Value]) -> Self {
        let items = match target {
            Target::ImageEditor => ti::editor_items(saved),
            Target::RegionCapture => ti::region_items(saved),
        };
        Self { target, items, scroll: 0.0, focus: None }
    }

    /// `ResetCommand`: the defaults.
    fn reset(&mut self) {
        self.items = match self.target {
            Target::ImageEditor => ti::editor_default_items(),
            Target::RegionCapture => ti::region_items(&[]),
        };
        self.focus = None;
    }

    /// `OkCommand`: the hotkeys normalised, the items as options.
    fn save(&mut self) -> Vec<Value> {
        for it in self.items.iter_mut().filter(|i| i.def.hotkey_editable) {
            if !it.hotkey.trim().is_empty() {
                it.hotkey = hotkey::normalize(&it.hotkey);
            }
            if !it.secondary_hotkey.trim().is_empty() {
                it.secondary_hotkey = hotkey::normalize(&it.secondary_hotkey);
            }
        }
        ti::options_json(&self.items)
    }

    /// `OnHotkeyKeyDown` for the focused box.
    fn keys(&mut self, ui: &Ui) {
        let Some((i, secondary)) = self.focus else { return };
        let events = ui.input(|inp| inp.events.clone());
        for ev in events {
            let egui::Event::Key { key, pressed: true, modifiers, .. } = ev else { continue };
            let value = if matches!(key, Key::Backspace | Key::Delete) {
                String::new()
            } else {
                let Some(name) = key_name(key) else { continue };
                let m = Mods { ctrl: modifiers.ctrl || modifiers.command, shift: modifiers.shift, alt: modifiers.alt, meta: false };
                hotkey::format(&name, m)
            };
            if let Some(it) = self.items.get_mut(i)
                && it.def.hotkey_editable
            {
                if secondary {
                    it.secondary_hotkey = value;
                } else {
                    it.hotkey = value;
                }
            }
        }
    }

    /// Draws the dialog over `full`.
    pub fn show(&mut self, ui: &mut Ui, full: Rect) -> Outcome<Vec<Value>> {
        let p = palette(ui.ctx());
        let f = fluent(ui.ctx());
        // `ToolbarEditorOverlay`: the modal backdrop on its own layer.
        let layer = egui::LayerId::new(egui::Order::Foreground, Id::new("toolbar-editor"));
        let mut ui = ui.new_child(egui::UiBuilder::new().layer_id(layer).max_rect(full));
        ui.painter().rect_filled(full, 0.0, p.overlay_modal);
        let backdrop = ui.interact(full, Id::new("toolbar-editor-backdrop"), Sense::click());
        let list_h = self.items.len() as f32 * ROW;
        let height = (2.0 + HEADER + 14.0 + 2.0 + COLUMNS + list_h + 14.0 + FOOTER).min(MAX_HEIGHT);
        let c = Rect::from_center_size(full.center(), vec2(WIDTH, height));
        let card = Rect::from_min_size(pos2(c.min.x.round(), c.min.y.round()), vec2(WIDTH, height));
        let _ = ui.interact(card, Id::new("toolbar-editor-card"), Sense::click());
        // `BoxShadow="0 4 10 0 #40000000"`, the popup background and border.
        for k in 1..=5 {
            let a = (0x40 as f32 * (1.0 - k as f32 / 6.0) / 5.0) as u8;
            ui.painter().rect_filled(card.translate(vec2(0.0, 4.0)).expand(k as f32), CornerRadius::same(4 + k), Color32::from_black_alpha(a));
        }
        border::border(ui.painter(), card, 4.0, 1.0, Some(Brush::Solid(p.background_panel)), Some(p.border));
        let inner = card.shrink(1.0);
        // The header: the title, Reset toolbar to default.
        let head = Rect::from_min_size(inner.min, vec2(inner.width(), HEADER - 1.0));
        ui.painter().rect_filled(head, CornerRadius { nw: 3, ne: 3, sw: 0, se: 0 }, p.background_toolbar);
        ui.painter().rect_filled(Rect::from_min_size(pos2(head.min.x, head.max.y), vec2(head.width(), 1.0)), 0.0, p.border);
        let title_font = widgets::font(15.0, 600);
        let g = widgets::galley(&ui, ie::ToolbarCustomizationDialogView_CustomizeToolbar(), title_font.clone(), p.text_secondary, f32::INFINITY);
        ui.painter().galley(pos2(head.min.x + 14.0, widgets::center_y(head, widgets::line_height(&ui, &title_font))), g, p.text_secondary);
        let reset_text = ie::ToolbarCustomizationDialogView_Resettoolbartodefault();
        let rb = Button { padding: Thickness::hv(10.0, 0.0), ..Button::default() };
        let rw = rb.width(&ui, reset_text);
        let reset = Rect::from_min_size(pos2(head.max.x - 14.0 - rw, head.min.y + 8.0), vec2(rw, 32.0));
        if rb.show(&ui, reset, Id::new("toolbar-reset"), reset_text, true).clicked() {
            self.reset();
        }
        // The body: a bordered box with the column titles and the rows.
        let body = Rect::from_min_max(pos2(inner.min.x + 14.0, head.max.y + 1.0 + 14.0), pos2(inner.max.x - 14.0, inner.max.y - FOOTER - 14.0));
        border::border(ui.painter(), body, 4.0, 1.0, None, Some(p.border));
        let bi = body.shrink(1.0);
        let cols = Rect::from_min_size(bi.min, vec2(bi.width(), COLUMNS - 1.0));
        ui.painter().rect_filled(Rect::from_min_size(pos2(cols.min.x, cols.max.y), vec2(cols.width(), 1.0)), 0.0, p.border);
        let label = widgets::font(13.0, 600);
        let lh = widgets::line_height(&ui, &label);
        let right = cols.max.x - 16.0;
        for (k, text) in [ie::ToolbarCustomizationDialogView_Primary(), ie::ToolbarCustomizationDialogView_Secondary()].iter().enumerate() {
            let x0 = right - BOX * (2 - k) as f32 - 8.0 * (1 - k) as f32;
            let w = widgets::text_width(&ui, text, label.clone());
            let g = widgets::galley(&ui, text, label.clone(), p.text_secondary, f32::INFINITY);
            ui.painter().galley(pos2((x0 + (BOX - w) / 2.0).round_ties_even(), cols.min.y + 6.0), g, p.text_secondary);
        }
        let _ = lh;
        // The rows, scrolled (`Padding="0,0,8,0"` leaves the scroll bar's room).
        let view = Rect::from_min_max(pos2(bi.min.x, cols.max.y + 1.0), bi.max);
        let max_scroll = (list_h - view.height()).max(0.0);
        if ui.rect_contains_pointer(view) {
            self.scroll -= ui.input(|i| i.smooth_scroll_delta.y);
        }
        self.scroll = self.scroll.clamp(0.0, max_scroll);
        let mut rows = ui.new_child(egui::UiBuilder::new().max_rect(view));
        rows.set_clip_rect(view.intersect(ui.clip_rect()));
        let count = self.items.len();
        let mut action: Option<(usize, i32)> = None;
        let clicked_outside_boxes = ui.input(|i| i.pointer.any_pressed());
        let mut box_clicked = false;
        for i in 0..count {
            let top = (view.min.y + i as f32 * ROW - self.scroll).round();
            let row = Rect::from_min_size(pos2(view.min.x, top), vec2(view.width() - 8.0, ROW - 1.0));
            if row.max.y < view.min.y || row.min.y > view.max.y {
                continue;
            }
            rows.painter().rect_filled(Rect::from_min_size(pos2(view.min.x, row.max.y), vec2(view.width(), 1.0)), 0.0, p.border);
            let content = row.shrink(8.0);
            let cy = content.center().y;
            let it = self.items[i].clone();
            // Move up, move down, begin group, show.
            let mut x = content.min.x;
            for (k, (key, enabled, tip)) in [
                (icon::ChevronUp, i > 0, ie::ToolbarCustomizationDialogView_Moveup()),
                (icon::ChevronDown, i + 1 < count, ie::ToolbarCustomizationDialogView_Movedown()),
            ]
            .into_iter()
            .enumerate()
            {
                let b = Rect::from_min_size(pos2(x, (cy - 16.0).round()), Vec2::splat(32.0));
                let r = Button { padding: Thickness::all(0.0), ..Button::default() }.show(&rows, b, Id::new(("tb-move", i, k)), "", enabled);
                let fg = if enabled { p.text } else { f.base_medium_low.gamma_multiply(0.35) };
                icons::paint(rows.painter(), Rect::from_center_size(b.center(), Vec2::splat(16.0)), key, fg);
                if r.on_hover_text(tip).clicked() {
                    action = Some((i, if k == 0 { -1 } else { 1 }));
                }
                x += 32.0 + 10.0;
            }
            let tb = Rect::from_min_size(pos2(x, (cy - 16.0).round()), Vec2::splat(32.0));
            let r = rows.interact(tb, Id::new(("tb-group", i)), Sense::click());
            // `ToggleButton`: Fluent's base-low fill, the accent when checked.
            if it.begin_group {
                border::border(rows.painter(), tb, 4.0, 1.0, Some(Brush::accent(rows.ctx())), Some(Color32::TRANSPARENT));
            } else {
                let fill = if r.hovered() { f.base_medium_low } else { f.base_low };
                border::border(rows.painter(), tb, 4.0, 1.0, Some(Brush::Solid(fill)), Some(Color32::TRANSPARENT));
            }
            let fg = if it.begin_group { p.accent_foreground } else { p.text };
            icons::paint(rows.painter(), Rect::from_center_size(tb.center(), Vec2::splat(16.0)), icon::ArrowRightFromLine, fg);
            if r.on_hover_text(ie::ToolbarCustomizationDialogView_Begingroup()).clicked() {
                self.items[i].begin_group = !self.items[i].begin_group;
            }
            x += 32.0 + 10.0;
            let sw_pos = pos2(x, (cy - 16.0).round());
            let mut on = it.visible;
            let editable = it.def.visibility_editable;
            if editable {
                let r = ToggleSwitch { enabled: true }.show(&rows, sw_pos, Id::new(("tb-show", i)), &mut on);
                if r.on_hover_text(ie::ToolbarCustomizationDialogView_ShowHide()).changed() {
                    self.items[i].visible = on;
                }
            } else {
                // A disabled switch that's on: the base-low track, the
                // knob at 36% of the base colour (as ShareX draws it).
                let track = Rect::from_min_size(pos2(sw_pos.x, sw_pos.y + 6.0), vec2(40.0, 20.0));
                border::border(rows.painter(), track, 10.0, 0.0, Some(Brush::Solid(f.base_low)), None);
                let knob = f.base_high.gamma_multiply(0.36);
                border::ellipse(rows.painter(), Rect::from_center_size(pos2(track.max.x - 10.0, track.center().y), Vec2::splat(10.0)), knob);
            }
            x += 44.0 + 10.0;
            // The icon (18px) in a 32px column, the name.
            icons::paint(rows.painter(), Rect::from_center_size(pos2(x + 16.0, cy), Vec2::splat(18.0)), it.def.icon, p.text);
            let g = widgets::galley(&rows, it.name(), label.clone(), p.text, f32::INFINITY);
            rows.painter().galley(pos2(x + 32.0 + 10.0, widgets::center_y(content, widgets::line_height(&rows, &label))), g, p.text);
            // The two hotkey boxes (read-only `TextBox`es, the text centred).
            for (k, value) in [(false, &it.hotkey), (true, &it.secondary_hotkey)] {
                let bx = content.max.x - if k { BOX } else { BOX * 2.0 + 8.0 };
                let r = Rect::from_min_size(pos2(bx, content.min.y), vec2(BOX, 40.0));
                let focused = self.focus == Some((i, k));
                let resp = rows.interact(r, Id::new(("tb-key", i, k)), if it.def.hotkey_editable { Sense::click() } else { Sense::hover() });
                let fill = if !it.def.hotkey_editable { p.control_background_disabled } else if resp.hovered() && !focused { p.control_background_hover } else { p.control_background };
                let stroke = if focused { p.accent_border } else { p.border };
                border::border(rows.painter(), r, 4.0, 1.0, Some(Brush::Solid(fill)), Some(stroke));
                if focused {
                    // Fluent's focused `TextBox`: a 2px accent bottom edge.
                    rows.painter().rect_filled(Rect::from_min_max(pos2(r.min.x + 1.0, r.max.y - 2.0), pos2(r.max.x - 1.0, r.max.y - 1.0)), 0.0, p.accent_border);
                }
                let font = content_font(14.0, 400);
                let w = widgets::text_width(&rows, value, font.clone());
                let g = widgets::galley(&rows, value, font.clone(), p.text, f32::INFINITY);
                rows.painter().galley(pos2((r.center().x - w / 2.0).round(), widgets::center_y(r, widgets::line_height(&rows, &font))), g, p.text);
                let resp = resp.on_hover_text(ie::ToolbarCustomizationDialogView_ClickandpressashortcutBackspaceorDeleteclearsit());
                if resp.clicked() {
                    self.focus = Some((i, k));
                    box_clicked = true;
                }
            }
        }
        if clicked_outside_boxes && !box_clicked && self.focus.is_some() {
            let pos = ui.input(|i| i.pointer.interact_pos());
            if pos.is_some_and(|q| !q.y.is_nan()) {
                self.focus = None;
            }
        }
        if let Some((i, d)) = action {
            let j = (i as i32 + d) as usize;
            self.items.swap(i, j);
            self.focus = None;
        }
        self.keys(&ui);
        // The collapsed scroll bar.
        if max_scroll > 0.0 {
            let track = (view.min.y + 16.0, view.max.y - 16.0);
            let len = track.1 - track.0;
            let th = (len * view.height() / list_h).max(16.0);
            let ty = track.0 + (len - th) * self.scroll / max_scroll;
            ui.painter().rect_filled(Rect::from_min_size(pos2(view.max.x - 4.0, ty), vec2(2.0, th)), CornerRadius::same(1), f.base_medium);
        }
        // The footer: the validation message, OK and Cancel.
        let foot = Rect::from_min_max(pos2(inner.min.x, inner.max.y - FOOTER + 1.0), inner.max);
        ui.painter().rect_filled(Rect::from_min_size(pos2(foot.min.x, foot.min.y - 1.0), vec2(foot.width(), 1.0)), 0.0, p.border);
        ui.painter().rect_filled(foot, CornerRadius { nw: 0, ne: 0, sw: 3, se: 3 }, p.background_toolbar);
        let validation = ti::validation(&self.items);
        let bb = Button { padding: Thickness::hv(20.0, 0.0), ..Button::default() };
        let (ok_t, cancel_t) = (ie::ToolbarCustomizationDialogView_OK(), ie::ToolbarCustomizationDialogView_Cancel());
        let (ow, cw) = (bb.width(&ui, ok_t), bb.width(&ui, cancel_t));
        let cancel = Rect::from_min_size(pos2(foot.max.x - 10.0 - cw, foot.min.y + 10.0), vec2(cw, 40.0));
        let ok = Rect::from_min_size(pos2(cancel.min.x - 5.0 - ow, foot.min.y + 10.0), vec2(ow, 40.0));
        if let Some(v) = &validation {
            let font = widgets::font(13.0, 600);
            let width = ok.min.x - 16.0 - (foot.min.x + 10.0 + 4.0);
            let lines = sharex_ui::controls::wrap_lines(&ui, v, &font, width);
            let line = widgets::line_height(&ui, &font);
            let mut y = foot.center().y - line * lines.len() as f32 / 2.0;
            for l in lines {
                let g = widgets::galley(&ui, &l, font.clone(), p.cancel_border, f32::INFINITY);
                ui.painter().galley(pos2(foot.min.x + 14.0, y.round()), g, p.cancel_border);
                y += line;
            }
        }
        let ok_clicked = bb.show(&ui, ok, Id::new("toolbar-ok"), ok_t, validation.is_none()).clicked();
        let cancel_clicked = bb.show(&ui, cancel, Id::new("toolbar-cancel"), cancel_t, true).clicked();
        let escape = self.focus.is_none() && ui.input(|i| i.key_pressed(Key::Escape));
        if cancel_clicked || backdrop.clicked() || escape {
            return Outcome::Cancelled;
        }
        if ok_clicked && validation.is_none() {
            return Outcome::Saved(self.save());
        }
        Outcome::Open
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_names() {
        assert_eq!(key_name(Key::R).as_deref(), Some("R"));
        assert_eq!(key_name(Key::Num1).as_deref(), Some("D1"));
        assert_eq!(key_name(Key::F5).as_deref(), Some("F5"));
        assert_eq!(key_name(Key::PageUp).as_deref(), Some("PageUp"));
    }

    #[test]
    fn saves_normalised_options() {
        let mut ed = ToolbarEditor::new(Target::ImageEditor, &[]);
        ed.items[2].hotkey = "ctrl+shift+r".into();
        let v = ed.save();
        assert_eq!(v[2]["Hotkey"], "Ctrl+Shift+R");
        let r = ToolbarEditor::new(Target::RegionCapture, &[]);
        assert_eq!(r.items.len(), 21);
    }

    #[test]
    fn region_save_is_sharexs() {
        // ShareX's Task settings › Region capture › Customize toolbar...,
        // the Ellipse region shown, OK: its RegionCaptureOptions.ToolbarItems.
        let expected: Vec<Value> = serde_json::from_str(include_str!("toolbar_editor_region_sharex.json")).unwrap();
        let mut ed = ToolbarEditor::new(Target::RegionCapture, &[]);
        ed.items[0].visible = true;
        assert_eq!(ed.save(), expected);
    }
}
