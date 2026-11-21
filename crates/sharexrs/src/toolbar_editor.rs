//! The toolbar editor: `ToolbarCustomizationDialogView` over Task settings
//! (`TaskSettingsWindow.ShowImageEditorToolbarEditor` and
//! `ShowRegionCaptureToolbarEditor`, its `ToolbarEditorOverlay`), as ShareX
//! #9278 (3104786) redesigned it: a mock of the toolbar, shrunk to fit,
//! whose items and spacers are dragged along it, onto it from the
//! available items (a spacer with a line, a blank one and the hidden
//! items) and off it onto them (hiding the item); a spacer's right edge is
//! dragged to change its width. Under them, the selected item: its two
//! hotkeys (click a box and press a shortcut; Backspace or Delete clears
//! it), only those since ShareX #9324 (85fc349). Reset toolbar to default,
//! OK (unless a hotkey is invalid or used twice: `RefreshValidation`) and
//! Cancel.

use egui::{Color32, CornerRadius, Id, Key, Pos2, Rect, Sense, Ui, Vec2, pos2, vec2};
use serde_json::Value;
use sharex_capture::toolbar_items::{self as ti, Item, Mods, hotkey};
use sharex_strings::image_editor as ie;
use sharex_ui::border::{self, Brush};
use sharex_ui::controls::{Button, content_font};
use sharex_ui::keys::app_icons as icon;
use sharex_ui::theme::{Palette, Thickness, fluent, palette};
use sharex_ui::{icons, widgets};

use crate::task_editors::Outcome;

/// Which toolbar the editor edits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    ImageEditor,
    RegionCapture,
}

/// `ToolbarLayoutSlot`: a toolbar item, or a spacer (`item` `None`) that
/// starts a new group of items.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Slot {
    item: Option<&'static str>,
    /// For a spacer, the extra space it adds.
    spacer_width: i32,
    /// For a spacer, whether it draws a separator line or only leaves space.
    show_line: bool,
}

impl Slot {
    fn item(id: &'static str) -> Self {
        Self { item: Some(id), spacer_width: 0, show_line: true }
    }

    fn spacer(spacer_width: i32, show_line: bool) -> Self {
        Self { item: None, spacer_width: ti::clamp_spacer_width(spacer_width), show_line }
    }

    fn is_spacer(&self) -> bool {
        self.item.is_none()
    }
}

/// A press on a slot that may become a drag (`_pressedSlot`).
#[derive(Clone, Copy, Debug)]
struct Press {
    slot: Slot,
    /// Its index on the toolbar; `None` for an available item.
    toolbar_index: Option<usize>,
    pos: Pos2,
}

/// Where a drag would drop (`DropKind`, `_dropIndex`).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Drop {
    None,
    /// Before the toolbar slot at this index; where the indicator goes.
    Toolbar(usize, Rect),
    Palette,
}

/// A spacer being resized by its grip (`_resizingSlot`).
#[derive(Clone, Copy, Debug)]
struct Resize {
    index: usize,
    start_width: i32,
    start_x: f32,
    /// The row's shrink-to-fit scale when it started.
    scale: f32,
    width: i32,
}

/// `ToolbarCustomizationDialogViewModel` and its view's state.
pub struct ToolbarEditor {
    pub target: Target,
    /// Every item in toolbar order. Once the layout is edited, hidden items
    /// follow the visible ones.
    items: Vec<Item>,
    /// The default items' ids, in order (`_defaultOrder`).
    default_order: Vec<&'static str>,
    selected: Option<&'static str>,
    scroll: f32,
    /// The selected item's hotkey box being typed into: whether it's the
    /// secondary one.
    focus: Option<bool>,
    /// The toolbar slot that has keyboard focus (pressed last).
    slot_focus: Option<usize>,
    press: Option<Press>,
    /// A press dragged past the threshold.
    dragging: Option<Drop>,
    resize: Option<Resize>,
}

// `DialogBorder` and its host.
const MIN_WIDTH: f32 = 600.0;
const MAX_WIDTH: f32 = 980.0;
const HOST_MARGIN: f32 = 32.0;
const MAX_HEIGHT: f32 = 640.0;
/// The `ScrollViewer`'s `StackPanel`: `Margin="14" Spacing="10"`.
const MARGIN: f32 = 14.0;
const SPACING: f32 = 10.0;
/// `Button.toolbar-slot`: 40x40, the File menu 54 wide; a spacer
/// `BaseSpacerSlotWidth` (15) plus its width, inside its 1px border.
const SLOT: f32 = 40.0;
const FILE_SLOT: f32 = 54.0;
const BASE_SPACER: f32 = 15.0;
/// The toolbar row's `Spacing="1"`.
const SLOT_SPACING: f32 = 1.0;
/// `Button.palette-tile`: 84x72, `Padding="4"`; the `WrapPanel`s' spacing.
const TILE: Vec2 = vec2(84.0, 72.0);
const TILE_SPACING: f32 = 4.0;
/// The drop areas' `MinHeight`s.
const TOOLBAR_MIN: f32 = 50.0;
const PALETTE_MIN: f32 = 82.0;
/// `DragThreshold`, `ToolbarDropMargin`, `SpacerWidthKeyStep`.
const DRAG_THRESHOLD: f32 = 4.0;
const TOOLBAR_DROP_MARGIN: f32 = 12.0;
const SPACER_WIDTH_KEY_STEP: i32 = 4;
/// The hotkey boxes and the spacer width box.
const BOX: f32 = 100.0;

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

/// A size as Avalonia measures it: rounded up to whole device pixels.
fn up(ppp: f32, v: f32) -> f32 {
    ((v * ppp) - 1e-3).ceil() / ppp
}

/// A position as Avalonia arranges it: the nearest device pixel.
fn snap(ppp: f32, v: f32) -> f32 {
    (v * ppp).round_ties_even() / ppp
}

impl ToolbarEditor {
    /// The items as saved (`CreateFromOptions` / `RegionCaptureToolbar.CreateItems`).
    pub fn new(target: Target, saved: &[Value]) -> Self {
        let items = match target {
            Target::ImageEditor => ti::editor_items(saved),
            Target::RegionCapture => ti::region_items(saved),
        };
        let default_order = Self::default_items(target).iter().map(Item::id).collect();
        let mut ed = Self {
            target,
            items,
            default_order,
            selected: None,
            scroll: 0.0,
            focus: None,
            slot_focus: None,
            press: None,
            dragging: None,
            resize: None,
        };
        ed.selected = ed.first_visible();
        ed
    }

    /// `--render ... --state`: `;`-separated steps as the dialog's controls
    /// would make them: `select:ID`, `spacer:ID:WIDTH[:blank]` (the item's
    /// spacer), `hide:ID` (dragged off the toolbar) and `drag:ID` (its
    /// toolbar slot picked up, dragged to the pointer).
    pub fn apply_state(&mut self, spec: &str) {
        for step in spec.split(';').map(str::trim).filter(|s| !s.is_empty()) {
            let parts: Vec<&str> = step.split(':').collect();
            let Some(id) = parts.get(1).and_then(|id| self.items.iter().map(Item::id).find(|i| i.eq_ignore_ascii_case(id))) else { continue };
            match parts[0] {
                "select" => self.selected = Some(id),
                "spacer" => {
                    let width = parts.get(2).and_then(|w| w.parse().ok()).unwrap_or(0);
                    let blank = parts.get(3) == Some(&"blank");
                    if let Some(item) = self.item_mut(id) {
                        item.spacer_width = ti::clamp_spacer_width(width);
                        item.show_spacer_line = !blank;
                    }
                }
                "hide" => {
                    if let Some(item) = self.item_mut(id) {
                        item.visible = false;
                    }
                }
                "drag" => {
                    if let Some(k) = self.index_of_slot(id) {
                        self.press = Some(Press { slot: Slot::item(id), toolbar_index: Some(k), pos: Pos2::ZERO });
                        self.dragging = Some(Drop::None);
                        self.selected = Some(id);
                    }
                }
                _ => {}
            }
        }
    }

    fn default_items(target: Target) -> Vec<Item> {
        match target {
            Target::ImageEditor => ti::editor_default_items(),
            Target::RegionCapture => ti::region_items(&[]),
        }
    }

    fn first_visible(&self) -> Option<&'static str> {
        self.items.iter().find(|i| i.visible).map(Item::id)
    }

    fn item(&self, id: &str) -> Option<&Item> {
        self.items.iter().find(|i| i.id() == id)
    }

    fn item_mut(&mut self, id: &str) -> Option<&mut Item> {
        self.items.iter_mut().find(|i| i.id() == id)
    }

    fn selected_item(&self) -> Option<&Item> {
        self.selected.and_then(|id| self.item(id))
    }

    /// `GetDefaultOrder`.
    fn default_index(&self, id: &str) -> usize {
        self.default_order.iter().position(|d| d.eq_ignore_ascii_case(id)).unwrap_or(usize::MAX)
    }

    // `RebuildLayout`.

    /// `ToolbarSlots`: the visible items and the spacers between them.
    fn toolbar_slots(&self) -> Vec<Slot> {
        let mut slots = Vec::new();
        for item in self.items.iter().filter(|i| i.visible) {
            if item.begin_group {
                slots.push(Slot::spacer(item.spacer_width, item.show_spacer_line));
            }
            slots.push(Slot::item(item.id()));
        }
        slots
    }

    /// `AvailableSlots`: a spacer with a line, one without, and the hidden
    /// items in their default order.
    fn palette_slots(&self) -> Vec<Slot> {
        let mut slots = vec![Slot::spacer(0, true), Slot::spacer(0, false)];
        let mut hidden: Vec<&Item> = self.items.iter().filter(|i| !i.visible).collect();
        hidden.sort_by_key(|i| self.default_index(i.id()));
        slots.extend(hidden.into_iter().map(|i| Slot::item(i.id())));
        slots
    }

    /// The toolbar's slots with a spacer being resized at its live width.
    fn shown_toolbar_slots(&self) -> Vec<Slot> {
        let mut slots = self.toolbar_slots();
        if let Some(r) = self.resize
            && let Some(s) = slots.get_mut(r.index)
        {
            s.spacer_width = r.width;
        }
        slots
    }

    /// `IndexOfSlot`.
    fn index_of_slot(&self, id: &str) -> Option<usize> {
        self.toolbar_slots().iter().position(|s| s.item == Some(id))
    }

    /// `ApplyLayoutTokens`: a toolbar layout written back to the items. A
    /// spacer becomes `BeginGroup` and `SpacerWidth` on the item after it,
    /// so neighbouring spacers merge into the widest of them, with a line
    /// if either has one, and a spacer at the end is dropped. Items left out
    /// are hidden, after the visible ones in their default order.
    fn apply_layout(&mut self, tokens: &[Slot]) {
        let mut order: Vec<&'static str> = Vec::new();
        let (mut begin_group, mut spacer_width, mut show_line) = (false, 0, true);
        for token in tokens {
            let Some(id) = token.item else {
                spacer_width = if begin_group { spacer_width.max(token.spacer_width) } else { token.spacer_width };
                show_line = if begin_group { show_line || token.show_line } else { token.show_line };
                begin_group = true;
                continue;
            };
            if let Some(item) = self.item_mut(id) {
                item.visible = true;
                item.begin_group = begin_group;
                item.spacer_width = if begin_group { spacer_width } else { 0 };
                item.show_spacer_line = !begin_group || show_line;
            }
            begin_group = false;
            order.push(id);
        }
        let mut hidden: Vec<&'static str> = self.items.iter().map(Item::id).filter(|id| !order.contains(id)).collect();
        hidden.sort_by_key(|id| self.default_index(id));
        for id in &hidden {
            if let Some(item) = self.item_mut(id) {
                item.visible = false;
                item.begin_group = false;
                item.spacer_width = 0;
                item.show_spacer_line = true;
            }
        }
        order.extend(hidden);
        let mut items = std::mem::take(&mut self.items);
        self.items = order.iter().filter_map(|id| items.iter().position(|i| i.id() == *id).map(|k| items.swap_remove(k))).collect();
    }

    /// `PlaceItem`: `id` on the toolbar before the slot at `slot_index` (at
    /// the end when it's the slot count); an item already on it moves.
    fn place_item(&mut self, id: &'static str, slot_index: usize) {
        let mut tokens = self.toolbar_slots();
        let mut slot_index = slot_index;
        if let Some(old) = tokens.iter().position(|t| t.item == Some(id)) {
            tokens.remove(old);
            if old < slot_index {
                slot_index -= 1;
            }
        }
        tokens.insert(slot_index.min(tokens.len()), Slot::item(id));
        self.apply_layout(&tokens);
        self.selected = Some(id);
    }

    /// `PlaceSpacer`: a spacer before the slot at `slot_index`; when `from`
    /// is a spacer on the toolbar, it moves (keeping its width and line)
    /// instead of a new one being added with `show_line`.
    fn place_spacer(&mut self, slot_index: usize, from: Option<usize>, show_line: bool) {
        let mut tokens = self.toolbar_slots();
        let mut slot_index = slot_index;
        let mut spacer = Slot::spacer(0, show_line);
        if let Some(from) = from
            && tokens.get(from).is_some_and(Slot::is_spacer)
        {
            spacer = tokens.remove(from);
            if from < slot_index {
                slot_index -= 1;
            }
        }
        tokens.insert(slot_index.min(tokens.len()), spacer);
        self.apply_layout(&tokens);
    }

    /// `SetSpacerWidth`: the extra space of the spacer at `slot_index`,
    /// kept by the item after it.
    fn set_spacer_width(&mut self, slot_index: usize, width: i32) {
        let slots = self.toolbar_slots();
        if slot_index + 1 >= slots.len() || !slots[slot_index].is_spacer() {
            return;
        }
        if let Some(id) = slots[slot_index + 1].item
            && let Some(item) = self.item_mut(id)
        {
            item.spacer_width = ti::clamp_spacer_width(width);
        }
    }

    /// `CanRemove`: false for items that must stay on the toolbar (File).
    fn can_remove(&self, slot: &Slot) -> bool {
        slot.item.is_none_or(|id| self.item(id).is_some_and(|i| i.def.visibility_editable))
    }

    /// `RemoveSlot`: the spacer or item at `slot_index` off the toolbar;
    /// items taken off are hidden.
    fn remove_slot(&mut self, slot_index: usize) {
        let mut tokens = self.toolbar_slots();
        if slot_index >= tokens.len() || !self.can_remove(&tokens[slot_index]) {
            return;
        }
        tokens.remove(slot_index);
        self.apply_layout(&tokens);
    }

    /// `MoveLeft`.
    fn move_left(&mut self, id: &'static str) {
        if let Some(i) = self.index_of_slot(id)
            && i > 0
        {
            self.place_item(id, i - 1);
        }
    }

    /// `MoveRight`.
    fn move_right(&mut self, id: &'static str) {
        if let Some(i) = self.index_of_slot(id)
            && i + 1 < self.toolbar_slots().len()
        {
            self.place_item(id, i + 2);
        }
    }

    /// `ResetCommand`: the defaults, the first visible item selected.
    fn reset(&mut self) {
        self.items = Self::default_items(self.target);
        self.selected = self.first_visible();
        self.focus = None;
        self.slot_focus = None;
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

    /// `OnPreviewKeyDown` for the focused hotkey box.
    fn hotkey_keys(&mut self, ui: &Ui) {
        let (Some(secondary), Some(id)) = (self.focus, self.selected) else { return };
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
            if let Some(it) = self.item_mut(id)
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

    /// `OnToolbarSlotKeyDown`: Delete takes the focused slot off the
    /// toolbar, Ctrl+Left or Ctrl+Right moves the focused item, and
    /// Shift+Left or Shift+Right narrows or widens the focused spacer.
    fn slot_keys(&mut self, ui: &Ui) {
        let Some(index) = self.slot_focus else { return };
        let slots = self.toolbar_slots();
        let Some(slot) = slots.get(index).copied() else {
            self.slot_focus = None;
            return;
        };
        let events = ui.input(|inp| inp.events.clone());
        for ev in events {
            let egui::Event::Key { key, pressed: true, modifiers: m, .. } = ev else { continue };
            let ctrl = (m.ctrl || m.command) && !m.shift && !m.alt;
            let shift = m.shift && !m.ctrl && !m.command && !m.alt;
            match key {
                Key::Delete if self.can_remove(&slot) => {
                    self.remove_slot(index);
                    let count = self.toolbar_slots().len();
                    self.slot_focus = (count > 0).then(|| index.min(count - 1));
                }
                Key::ArrowLeft | Key::ArrowRight if ctrl && slot.item.is_some() => {
                    let id = slot.item.expect("an item");
                    if key == Key::ArrowLeft {
                        self.move_left(id);
                    } else {
                        self.move_right(id);
                    }
                    self.slot_focus = self.index_of_slot(id);
                }
                Key::ArrowLeft | Key::ArrowRight if shift && slot.is_spacer() => {
                    let step = if key == Key::ArrowLeft { -SPACER_WIDTH_KEY_STEP } else { SPACER_WIDTH_KEY_STEP };
                    self.set_spacer_width(index, slot.spacer_width + step);
                }
                _ => continue,
            }
            return;
        }
    }

    /// Draws the dialog over `full`.
    pub fn show(&mut self, ui: &mut Ui, full: Rect) -> Outcome<Vec<Value>> {
        let p = palette(ui.ctx());
        let ppp = ui.ctx().pixels_per_point();
        // `ToolbarEditorOverlay`: the modal backdrop on its own layer.
        let layer = egui::LayerId::new(egui::Order::Foreground, Id::new("toolbar-editor"));
        let mut ui = ui.new_child(egui::UiBuilder::new().layer_id(layer).max_rect(full));
        ui.painter().rect_filled(full, 0.0, p.overlay_modal);
        let backdrop = ui.interact(full, Id::new("toolbar-editor-backdrop"), Sense::click());

        // `MeasureOverride`: the dialog widens with its host.
        let width = (full.width() - HOST_MARGIN).clamp(MIN_WIDTH, MAX_WIDTH);
        let inner_w = width - 2.0;
        let content_w = inner_w - 2.0 * MARGIN;
        let caption = widgets::font(12.0, 600);
        let caption_h = up(ppp, widgets::line_height(&ui, &caption));
        let hint_lines = sharex_ui::controls::wrap_lines(&ui, ie::ToolbarCustomizationDialogView_DragHint(), &caption, content_w);
        let hint_h = up(ppp, hint_lines.len() as f32 * widgets::line_height(&ui, &caption));
        // The mock toolbar: a row shrunk to fit (`Viewbox`, `DownOnly`).
        let slots = self.shown_toolbar_slots();
        let row_w = self.row_width(&slots);
        let row_avail = content_w - 2.0 * (1.0 + 4.0);
        let scale = if row_w > row_avail { row_avail / row_w } else { 1.0 };
        let toolbar_h = up(ppp, (SLOT * scale + 2.0 * (1.0 + 4.0)).max(TOOLBAR_MIN));
        // The available items: tiles in a `WrapPanel`.
        let palette_slots = self.palette_slots();
        let pal_inner = content_w - 2.0 * (1.0 + 4.0);
        let per_line = (((pal_inner + TILE_SPACING) / (TILE.x + TILE_SPACING)).floor() as usize).max(1);
        let lines = palette_slots.len().div_ceil(per_line);
        let tiles_h = lines as f32 * TILE.y + (lines.saturating_sub(1)) as f32 * TILE_SPACING;
        let palette_h = up(ppp, (tiles_h + 10.0).max(PALETTE_MIN));
        // Borders, paddings and margins are whole device pixels; each band's
        // height is rounded up once.
        let th = |v: f32| snap(ppp, v);
        let margin = th(MARGIN);
        let caption_top = th(4.0);
        let body_natural = 2.0 * margin + up(ppp, hint_h + SPACING + toolbar_h + SPACING + caption_top + caption_h + SPACING + palette_h);
        // The header: `Padding="14,8"` round a 32px button, and its bottom border.
        let header_h = 2.0 * th(8.0) + up(ppp, 32.0) + th(1.0);
        // The selected item: its top border, `Padding="14,10"`, a caption over a 40px box.
        let selected_inner = up(ppp, caption_h + 2.0 + 40.0);
        let selected_h = if self.selected.is_some() { th(1.0) + 2.0 * th(10.0) + selected_inner } else { 0.0 };
        // The footer: its top border, `Padding="10"` and the 40px buttons.
        let footer_h = th(1.0) + 2.0 * th(10.0) + up(ppp, 40.0);
        let chrome = 2.0 * th(1.0) + header_h + selected_h + footer_h;
        let height = (chrome + body_natural).min(MAX_HEIGHT).min(full.height());
        let card = Rect::from_min_size(
            pos2(full.min.x + snap(ppp, (full.width() - width) / 2.0), full.min.y + snap(ppp, (full.height() - height) / 2.0)),
            vec2(width, height),
        );
        let _ = ui.interact(card, Id::new("toolbar-editor-card"), Sense::click());
        // `BoxShadow="0 4 10 0 #40000000"`, the popup background and border.
        for k in 1..=5 {
            let a = (0x40 as f32 * (1.0 - k as f32 / 6.0) / 5.0) as u8;
            ui.painter().rect_filled(card.translate(vec2(0.0, 4.0)).expand(k as f32), CornerRadius::same(4 + k), Color32::from_black_alpha(a));
        }
        border::border(ui.painter(), card, 4.0, 1.0, Some(Brush::Solid(p.background_panel)), Some(p.border));
        let inner = card.shrink(th(1.0));

        // The header: the title, Reset toolbar to default.
        let head = Rect::from_min_size(inner.min, vec2(inner.width(), header_h - th(1.0)));
        ui.painter().rect_filled(head, CornerRadius { nw: 3, ne: 3, sw: 0, se: 0 }, p.background_toolbar);
        ui.painter().rect_filled(Rect::from_min_size(pos2(head.min.x, head.max.y), vec2(head.width(), th(1.0))), 0.0, p.border);
        let title_font = widgets::font(15.0, 600);
        let g = widgets::galley(&ui, ie::ToolbarCustomizationDialogView_CustomizeToolbar(), title_font.clone(), p.text_secondary, f32::INFINITY);
        ui.painter().galley(pos2(head.min.x + 14.0, widgets::center_y(head, widgets::line_height(&ui, &title_font))), g, p.text_secondary);
        let reset_text = ie::ToolbarCustomizationDialogView_Resettoolbartodefault();
        let rb = Button { padding: Thickness::hv(10.0, 0.0), ..Button::default() };
        let rw = rb.width(&ui, reset_text);
        let reset = Rect::from_min_size(pos2(head.max.x - 14.0 - rw, head.min.y + th(8.0)), vec2(rw, 32.0));
        if rb.show(&ui, reset, Id::new("toolbar-reset"), reset_text, true).clicked() {
            self.reset();
        }

        // The footer and the selected item sit at the bottom; the body
        // (`LayoutScrollViewer`) takes the rest and scrolls.
        let foot = Rect::from_min_max(pos2(inner.min.x, inner.max.y - footer_h + th(1.0)), inner.max);
        let sel_rect = Rect::from_min_max(pos2(inner.min.x, foot.min.y - th(1.0) - selected_h), pos2(inner.max.x, foot.min.y - th(1.0)));
        let view = Rect::from_min_max(pos2(inner.min.x, head.max.y + th(1.0)), pos2(inner.max.x, sel_rect.min.y));
        // Less than a device pixel over is rounding, not something to scroll.
        let over = body_natural - view.height();
        let max_scroll = if over * ppp > 0.5 { over } else { 0.0 };
        let pointer = ui.ctx().pointer_latest_pos();
        if pointer.is_some_and(|q| view.contains(q)) && self.dragging.is_none() && self.resize.is_none() {
            self.scroll -= ui.input(|i| i.smooth_scroll_delta.y);
        }
        self.scroll = self.scroll.clamp(0.0, max_scroll);
        let top = view.min.y - self.scroll;
        let x0 = inner.min.x + MARGIN;
        let mut y = top + margin;
        let clip = ui.painter().with_clip_rect(view.intersect(ui.clip_rect()));
        // `DragHint`.
        let lh = widgets::line_height(&ui, &caption);
        for (k, line) in hint_lines.iter().enumerate() {
            let g = widgets::galley(&ui, line, caption.clone(), p.text_secondary, f32::INFINITY);
            clip.galley(pos2(x0, snap(ppp, y + k as f32 * lh)), g, p.text_secondary);
        }
        y += hint_h + SPACING;
        let toolbar_area = Rect::from_min_size(pos2(x0, snap(ppp, y)), vec2(content_w, toolbar_h));
        y += toolbar_h + SPACING + caption_top;
        let g = widgets::galley(&ui, ie::ToolbarCustomizationDialogView_AvailableItems(), caption.clone(), p.text_secondary, f32::INFINITY);
        clip.galley(pos2(x0, snap(ppp, y)), g, p.text_secondary);
        y += caption_h + SPACING;
        let palette_area = Rect::from_min_size(pos2(x0, snap(ppp, y)), vec2(content_w, palette_h));

        // The slots' places: the row scaled into the toolbar area, centred
        // vertically, and the tiles.
        let row_origin = pos2(toolbar_area.min.x + 5.0, toolbar_area.min.y + 5.0 + ((toolbar_h - 10.0) - SLOT * scale) / 2.0);
        let row_origin = pos2(snap(ppp, row_origin.x), snap(ppp, row_origin.y));
        let slot_rects = self.slot_rects(&slots, row_origin, scale, ppp);
        let tile_rects: Vec<Rect> = (0..palette_slots.len())
            .map(|k| {
                let (line, col) = (k / per_line, k % per_line);
                Rect::from_min_size(
                    pos2(palette_area.min.x + 5.0 + col as f32 * (TILE.x + TILE_SPACING), palette_area.min.y + 5.0 + line as f32 * (TILE.y + TILE_SPACING)),
                    TILE,
                )
            })
            .collect();

        // Pointer input (`OnPreviewPointerPressed`/`Moved`/`Released`).
        self.pointer_input(&ui, &slots, &slot_rects, &palette_slots, &tile_rects, toolbar_area, palette_area, view, scale);

        // The drop areas.
        border::border(&clip, toolbar_area, 4.0, 1.0, Some(Brush::Solid(p.background_toolbar)), Some(p.border));
        let palette_target = self.dragging == Some(Drop::Palette);
        let normal_border = (!palette_target).then_some(p.border);
        border::border(&clip, palette_area, 4.0, 1.0, Some(Brush::Solid(p.background_main)), normal_border);
        if palette_target {
            // `.drop-target`: the accent border (the area's own background stays).
            accent_border(&clip, palette_area, 4.0, 1.0);
        }
        let hover = pointer.filter(|q| view.contains(*q) && self.dragging.is_none());
        // The toolbar's slots.
        let row_clip = clip.with_clip_rect(toolbar_area.shrink(1.0).intersect(view));
        for (k, (slot, r)) in slots.iter().zip(&slot_rects).enumerate() {
            let dragging = self.dragging.is_some() && self.press.is_some_and(|pr| pr.toolbar_index == Some(k));
            let resizing = self.resize.is_some_and(|rs| rs.index == k);
            let hovered = hover.is_some_and(|q| r.contains(q)) && self.resize.is_none();
            self.paint_slot(&ui, &row_clip, &p, slot, *r, scale, hovered, dragging, resizing, hover);
            if let Some(q) = hover
                && r.contains(q)
                && self.press.is_none()
            {
                let tip = match slot.item {
                    Some(id) => self.item(id).map(|i| i.name().to_string()).unwrap_or_default(),
                    None => ie::ToolbarCustomizationDialogView_SpacerToolTip().to_string(),
                };
                sharex_ui::tooltip::show(&ui, r.intersect(view), Id::new(("toolbar-slot-tip", k)), &tip);
            }
        }
        // The available items, then "Every item is on the toolbar."
        for (k, (slot, r)) in palette_slots.iter().zip(&tile_rects).enumerate() {
            let hovered = hover.is_some_and(|q| r.contains(q));
            let dragging = self.dragging.is_some() && self.press.is_some_and(|pr| pr.toolbar_index.is_none() && pr.slot == *slot);
            self.paint_tile(&ui, &clip, &p, slot, *r, hovered, dragging);
            if hovered && self.press.is_none() {
                sharex_ui::tooltip::show(&ui, r.intersect(view), Id::new(("toolbar-tile-tip", k)), &self.slot_name(slot));
            }
        }
        if !palette_slots.iter().any(|s| s.item.is_some()) {
            let text = ie::ToolbarCustomizationDialogView_AllItemsOnToolbar();
            let tiles_w = palette_slots.len().min(per_line) as f32 * (TILE.x + TILE_SPACING) - TILE_SPACING;
            let x = palette_area.min.x + 5.0 + tiles_w + TILE_SPACING + 8.0;
            let row = Rect::from_min_size(pos2(x, palette_area.min.y + 5.0), vec2(content_w, TILE.y));
            let g = widgets::galley(&ui, text, caption.clone(), p.text_secondary, f32::INFINITY);
            clip.galley(pos2(x, widgets::center_y(row, lh)), g, p.text_secondary);
        }
        // Fluent's collapsed scroll bar.
        if max_scroll > 0.0 {
            let f = fluent(ui.ctx());
            let track = (view.min.y + 16.0, view.max.y - 16.0);
            let len = track.1 - track.0;
            let th = (len * view.height() / body_natural).max(16.0);
            let ty = track.0 + (len - th) * self.scroll / max_scroll;
            ui.painter().rect_filled(Rect::from_min_size(pos2(view.max.x - 4.0, ty), vec2(2.0, th)), CornerRadius::same(1), f.base_medium);
        }

        // The selected item's settings.
        if self.selected.is_some() {
            ui.painter().rect_filled(Rect::from_min_size(sel_rect.min, vec2(sel_rect.width(), th(1.0))), 0.0, p.border);
            let content = Rect::from_min_max(pos2(sel_rect.min.x + th(14.0), sel_rect.min.y + th(1.0) + th(10.0)), pos2(sel_rect.max.x - th(14.0), sel_rect.max.y - th(10.0)));
            self.selected_panel(&mut ui, content);
        }

        // The footer: the validation message, OK and Cancel.
        ui.painter().rect_filled(Rect::from_min_size(pos2(foot.min.x, foot.min.y - th(1.0)), vec2(foot.width(), th(1.0))), 0.0, p.border);
        ui.painter().rect_filled(foot, CornerRadius { nw: 0, ne: 0, sw: 3, se: 3 }, p.background_toolbar);
        let validation = ti::validation(&self.items);
        let bb = Button { padding: Thickness::hv(20.0, 0.0), ..Button::default() };
        let (ok_t, cancel_t) = (ie::ToolbarCustomizationDialogView_OK(), ie::ToolbarCustomizationDialogView_Cancel());
        let (ow, cw) = (bb.width(&ui, ok_t), bb.width(&ui, cancel_t));
        let cancel = Rect::from_min_size(pos2(foot.max.x - th(10.0) - cw, foot.min.y + th(10.0)), vec2(cw, 40.0));
        let ok = Rect::from_min_size(pos2(cancel.min.x - 5.0 - ow, foot.min.y + th(10.0)), vec2(ow, 40.0));
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

        // The drag layer: where the item will land, and the item under the pointer.
        self.paint_drag_layer(&ui, &p, pointer);

        // Keys: Escape ends a drag or resize first; then the focused hotkey
        // box or toolbar slot; else it closes the dialog.
        let escape = ui.input(|i| i.key_pressed(Key::Escape));
        if self.dragging.is_some() || self.resize.is_some() {
            if escape {
                // `EndDrag`, or `EndResize(commit: false)`: the width as it was.
                self.end_drag();
                self.resize = None;
            }
            return Outcome::Open;
        }
        if self.focus.is_some() {
            self.hotkey_keys(&ui);
        } else {
            self.slot_keys(&ui);
        }
        if cancel_clicked || backdrop.clicked() || (escape && self.focus.is_none()) {
            return Outcome::Cancelled;
        }
        if ok_clicked && validation.is_none() {
            return Outcome::Saved(self.save());
        }
        Outcome::Open
    }

    /// The row's natural width: the slots and the spacing between them.
    fn row_width(&self, slots: &[Slot]) -> f32 {
        let w: f32 = slots.iter().map(|s| self.slot_width(s)).sum();
        w + slots.len().saturating_sub(1) as f32 * SLOT_SPACING
    }

    fn slot_width(&self, s: &Slot) -> f32 {
        match s.item {
            None => BASE_SPACER + s.spacer_width as f32 + 2.0,
            Some(id) if self.item(id).is_some_and(Item::is_file_menu) => FILE_SLOT,
            Some(_) => SLOT,
        }
    }

    /// The slots' rects: laid out in the row's own coordinates (each
    /// position a whole device pixel, as layout rounding leaves it inside
    /// the `Viewbox`), then scaled.
    fn slot_rects(&self, slots: &[Slot], origin: Pos2, scale: f32, ppp: f32) -> Vec<Rect> {
        let mut x = 0.0;
        slots
            .iter()
            .map(|s| {
                let w = self.slot_width(s);
                let r = Rect::from_min_size(pos2(origin.x + snap(ppp, x) * scale, origin.y), vec2(w, SLOT) * scale);
                x += w + SLOT_SPACING;
                r
            })
            .collect()
    }

    /// A tile's name: the item's, `Spacer` or `Blank spacer`.
    fn slot_name(&self, s: &Slot) -> String {
        match s.item {
            Some(id) => self.item(id).map(|i| i.name().to_string()).unwrap_or_default(),
            None if s.show_line => ie::ToolbarCustomizationDialogView_Spacer().to_string(),
            None => ie::ToolbarCustomizationDialogView_BlankSpacer().to_string(),
        }
    }

    /// The grip at a toolbar spacer's right edge (`Border.spacer-grip`:
    /// 6 wide, `Margin="0,6"`, inside the button's border).
    fn grip(r: Rect, scale: f32) -> Rect {
        let inner = r.shrink(scale);
        Rect::from_min_max(pos2(inner.max.x - 6.0 * scale, inner.min.y + 6.0 * scale), pos2(inner.max.x, inner.max.y - 6.0 * scale))
    }

    #[allow(clippy::too_many_arguments)]
    fn pointer_input(
        &mut self,
        ui: &Ui,
        slots: &[Slot],
        slot_rects: &[Rect],
        palette_slots: &[Slot],
        tile_rects: &[Rect],
        toolbar_area: Rect,
        palette_area: Rect,
        view: Rect,
        scale: f32,
    ) {
        let (pressed, released, pos, double) = ui.input(|i| {
            (
                i.pointer.primary_pressed(),
                i.pointer.primary_released(),
                i.pointer.interact_pos().or(i.pointer.latest_pos()),
                i.pointer.button_double_clicked(egui::PointerButton::Primary),
            )
        });
        let Some(pos) = pos else { return };
        if pressed && self.dragging.is_none() && self.resize.is_none() {
            self.press = None;
            let in_view = view.contains(pos);
            // `TryStartResize`: a toolbar spacer's grip.
            let grip = slots.iter().zip(slot_rects).position(|(s, r)| s.is_spacer() && Self::grip(*r, scale).contains(pos));
            if in_view && let Some(index) = grip {
                let start_width = slots[index].spacer_width;
                self.resize = Some(Resize { index, start_width, start_x: pos.x, scale, width: start_width });
                self.slot_focus = Some(index);
                return;
            }
            if in_view {
                if let Some(k) = slot_rects.iter().position(|r| r.contains(pos)) {
                    self.press = Some(Press { slot: slots[k], toolbar_index: Some(k), pos });
                } else if let Some(k) = tile_rects.iter().position(|r| r.contains(pos)) {
                    self.press = Some(Press { slot: palette_slots[k], toolbar_index: None, pos });
                }
            }
            if self.press.is_none() {
                self.slot_focus = None;
            }
        }
        if let Some(r) = &mut self.resize {
            let offset = (pos.x - r.start_x) / r.scale;
            r.width = ti::clamp_spacer_width((r.start_width as f32 + offset).round() as i32);
            if released {
                let (index, width) = (r.index, r.width);
                self.resize = None;
                self.set_spacer_width(index, width);
            }
            return;
        }
        let Some(press) = self.press else { return };
        if self.dragging.is_none() {
            let d = pos - press.pos;
            if !released && (d.x.abs() >= DRAG_THRESHOLD || d.y.abs() >= DRAG_THRESHOLD) && ui.input(|i| i.pointer.primary_down()) {
                // `StartDrag`: the item is selected as it's picked up.
                if let Some(id) = press.slot.item {
                    self.selected = Some(id);
                    self.focus = None;
                }
                self.dragging = Some(Drop::None);
            } else if released {
                // A press without a drag is a click (`OnSlotClick`).
                self.press = None;
                let over = match press.toolbar_index {
                    Some(k) => slot_rects.get(k).is_some_and(|r| r.contains(pos)),
                    None => tile_rects.iter().zip(palette_slots).any(|(r, s)| *s == press.slot && r.contains(pos)),
                };
                if over {
                    if let Some(id) = press.slot.item {
                        if self.selected != Some(id) {
                            self.focus = None;
                        }
                        self.selected = Some(id);
                    }
                    self.slot_focus = press.toolbar_index;
                    // `OnPaletteTileDoubleTapped`: onto the end of the toolbar.
                    if double
                        && press.toolbar_index.is_none()
                        && let Some(id) = press.slot.item
                    {
                        self.place_item(id, slots.len());
                    }
                }
                return;
            } else {
                return;
            }
        }
        // `UpdateDrag`.
        let drop = self.find_drop(pos, &press, slot_rects, toolbar_area, palette_area, view);
        self.dragging = Some(drop);
        if released {
            self.end_drag();
            match drop {
                Drop::Toolbar(index, _) => match press.slot.item {
                    Some(id) => self.place_item(id, index),
                    None => self.place_spacer(index, press.toolbar_index, press.slot.show_line),
                },
                Drop::Palette => {
                    if let Some(k) = press.toolbar_index {
                        self.remove_slot(k);
                    }
                }
                Drop::None => {}
            }
        }
    }

    /// `UpdateDrag`'s drop: the toolbar (with `ToolbarDropMargin` round it),
    /// or the available items for something dragged off the toolbar.
    fn find_drop(&self, pos: Pos2, press: &Press, slot_rects: &[Rect], toolbar_area: Rect, palette_area: Rect, view: Rect) -> Drop {
        if toolbar_area.expand(TOOLBAR_DROP_MARGIN).intersect(view).contains(pos) {
            let (index, indicator) = Self::find_gap(pos, slot_rects, toolbar_area);
            return Drop::Toolbar(index, indicator);
        }
        if press.toolbar_index.is_some() && self.can_remove(&press.slot) && palette_area.intersect(view).contains(pos) {
            return Drop::Palette;
        }
        Drop::None
    }

    /// `FindToolbarGap`: the gap nearest the pointer, as the index of the
    /// slot after it, and where the 3px indicator goes.
    fn find_gap(pos: Pos2, slot_rects: &[Rect], area: Rect) -> (usize, Rect) {
        const W: f32 = 3.0;
        let Some(first) = slot_rects.first() else {
            return (0, Rect::from_min_size(pos2(area.min.x + 4.0, area.min.y + 4.0), vec2(W, (area.height() - 8.0).max(0.0))));
        };
        let (top, h) = (first.min.y, first.height());
        for (i, r) in slot_rects.iter().enumerate() {
            if pos.x < r.center().x {
                return (i, Rect::from_min_size(pos2(r.min.x - W / 2.0 - 0.5, top), vec2(W, h)));
            }
        }
        let last = slot_rects[slot_rects.len() - 1];
        (slot_rects.len(), Rect::from_min_size(pos2(last.max.x - W / 2.0 + 0.5, top), vec2(W, h)))
    }

    /// `EndDrag`.
    fn end_drag(&mut self) {
        self.dragging = None;
        self.press = None;
    }

    /// One `Button.slot.toolbar-slot`, scaled with the row.
    #[allow(clippy::too_many_arguments)]
    fn paint_slot(&self, ui: &Ui, painter: &egui::Painter, p: &Palette, slot: &Slot, r: Rect, s: f32, hovered: bool, dragging: bool, resizing: bool, pointer: Option<Pos2>) {
        let selected = slot.item.is_some() && slot.item == self.selected;
        let mut painter = painter.clone();
        if dragging {
            // `.dragging`: `Opacity="0.35"`.
            painter.set_opacity(0.35);
        }
        let radius = 4.0 * s;
        if selected {
            border::border(&painter, r, radius, s, Some(Brush::Solid(p.control_background_hover)), None);
            accent_border(&painter, r, radius, s);
        } else if hovered || resizing {
            border::border(&painter, r, radius, s, Some(Brush::Solid(p.control_background_hover)), Some(p.border));
        }
        let content = r.shrink(s);
        match slot.item {
            None => {
                // The line (1x26) or, blank, a 2-3 dashed line; the grip.
                let cx = content.center().x;
                let line = Rect::from_center_size(pos2(cx, content.center().y), vec2(1.0, 26.0) * s);
                if slot.show_line {
                    painter.rect_filled(line, 0.0, p.separator);
                } else {
                    dashes(&painter, line, s, p.text_secondary);
                }
                let grip = Self::grip(r, s);
                let on_grip = pointer.is_some_and(|q| grip.contains(q));
                let fill = if on_grip || resizing {
                    Some(Brush::accent(ui.ctx()))
                } else if hovered {
                    Some(Brush::Solid(p.border))
                } else {
                    None
                };
                if let Some(fill) = fill {
                    border::border(&painter, grip, 2.0 * s, 0.0, Some(fill), None);
                }
            }
            Some(id) => {
                let Some(item) = self.item(id) else { return };
                if item.is_file_menu() {
                    // The 20px icon and the 10px chevron, 4 apart, centred.
                    let left = content.min.x + (content.width() - 34.0 * s) / 2.0;
                    icons::paint(&painter, Rect::from_min_size(pos2(left, content.center().y - 10.0 * s), Vec2::splat(20.0 * s)), item.def.icon, p.text);
                    icons::paint(&painter, Rect::from_min_size(pos2(left + 24.0 * s, content.center().y - 5.0 * s), Vec2::splat(10.0 * s)), icon::ChevronDown, p.text_secondary);
                } else {
                    icons::paint(&painter, Rect::from_center_size(content.center(), Vec2::splat(20.0 * s)), item.def.icon, p.text);
                }
            }
        }
    }

    /// One `Button.slot.palette-tile`: the spacer or icon in a 30px row, the
    /// name under it (12px, centred, at most two lines).
    #[allow(clippy::too_many_arguments)]
    fn paint_tile(&self, ui: &Ui, painter: &egui::Painter, p: &Palette, slot: &Slot, r: Rect, hovered: bool, dragging: bool) {
        let selected = slot.item.is_some() && slot.item == self.selected;
        let mut painter = painter.clone();
        if dragging {
            painter.set_opacity(0.35);
        }
        if selected {
            border::border(&painter, r, 4.0, 1.0, Some(Brush::Solid(p.control_background_hover)), None);
            accent_border(&painter, r, 4.0, 1.0);
        } else if hovered {
            border::border(&painter, r, 4.0, 1.0, Some(Brush::Solid(p.control_background_hover)), Some(p.border));
        }
        let content = r.shrink(1.0 + 4.0);
        let font = widgets::font(12.0, 600);
        let name = self.slot_name(slot);
        let mut lines = sharex_ui::controls::wrap_lines(ui, &name, &font, content.width());
        if lines.len() > 2 {
            // `MaxLines="2"`, `TextTrimming="CharacterEllipsis"`.
            let mut second = lines[1].clone();
            while !second.is_empty() && widgets::text_width(ui, &format!("{second}…"), font.clone()) > content.width() {
                second.pop();
            }
            lines.truncate(1);
            lines.push(format!("{}…", second.trim_end()));
        }
        let lh = widgets::line_height(ui, &font);
        let ppp = ui.ctx().pixels_per_point();
        // `VerticalContentAlignment="Center"`: the grid (a 30px row, then
        // the name) is centred in the tile.
        let grid_h = 30.0 + up(ppp, lines.len() as f32 * lh);
        let grid_top = content.min.y + snap(ppp, (content.height() - grid_h) / 2.0);
        let top = Rect::from_min_size(pos2(content.min.x, grid_top), vec2(content.width(), 30.0));
        match slot.item {
            None => {
                let line = Rect::from_center_size(top.center(), vec2(1.0, 24.0));
                if slot.show_line {
                    painter.rect_filled(line, 0.0, p.separator);
                } else {
                    dashes(&painter, line, 1.0, p.text_secondary);
                }
            }
            Some(id) => {
                if let Some(item) = self.item(id) {
                    icons::paint(&painter, Rect::from_center_size(top.center(), Vec2::splat(20.0)), item.def.icon, p.text);
                }
            }
        }
        for (k, line) in lines.iter().enumerate() {
            let w = widgets::text_width(ui, line, font.clone());
            let g = widgets::galley(ui, line, font.clone(), p.text, f32::INFINITY);
            painter.galley(pos2((content.center().x - w / 2.0).round_ties_even(), top.max.y + k as f32 * lh), g, p.text);
        }
    }

    /// `DropIndicator` and `DragGhost` (hung under the pointer so the
    /// indicator stays visible).
    fn paint_drag_layer(&self, ui: &Ui, p: &Palette, pointer: Option<Pos2>) {
        let (Some(drop), Some(press), Some(at)) = (self.dragging, self.press, pointer) else { return };
        let painter = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Tooltip, Id::new("toolbar-editor-drag")));
        if let Drop::Toolbar(_, indicator) = drop {
            border::border(&painter, indicator, 1.5, 0.0, Some(Brush::accent(ui.ctx())), None);
        }
        let ghost = Rect::from_min_size(pos2(at.x - 20.0, at.y + 14.0), Vec2::splat(40.0));
        let mut g = painter.clone();
        g.set_opacity(0.9);
        // `BoxShadow="0 3 8 0 #50000000"`.
        for k in 1..=4 {
            let a = (0x50 as f32 * (1.0 - k as f32 / 5.0) / 4.0) as u8;
            g.rect_filled(ghost.translate(vec2(0.0, 3.0)).expand(k as f32), CornerRadius::same(4 + k), Color32::from_black_alpha(a));
        }
        border::border(&g, ghost, 4.0, 1.0, Some(Brush::Solid(p.background_toolbar)), None);
        accent_border(&g, ghost, 4.0, 1.0);
        match press.slot.item {
            None => {
                let line = Rect::from_center_size(ghost.center(), vec2(1.0, 26.0));
                if press.slot.show_line {
                    g.rect_filled(line, 0.0, p.separator);
                } else {
                    dashes(&g, line, 1.0, p.text_secondary);
                }
            }
            Some(id) => {
                if let Some(item) = self.item(id) {
                    icons::paint(&g, Rect::from_center_size(ghost.center(), Vec2::splat(20.0)), item.def.icon, p.text);
                }
            }
        }
    }

    /// `SelectedItemPanel`: the icon and name, then Primary and Secondary
    /// (ShareX #9324, 85fc349: only the hotkeys; moving, grouping, spacers
    /// and showing or hiding are done on the toolbar preview or with its
    /// keys).
    fn selected_panel(&mut self, ui: &mut Ui, content: Rect) {
        let p = palette(ui.ctx());
        let Some(it) = self.selected_item().cloned() else { return };
        let caption = widgets::font(12.0, 600);
        let caption_h = up(ui.ctx().pixels_per_point(), widgets::line_height(ui, &caption));
        // Column 1 (`ColumnDefinitions="*,Auto"`, 16 apart): the boxes, 8 apart.
        let col1_w = BOX + 8.0 + BOX;
        let col1_x = content.max.x - col1_w;
        let col1_h = caption_h + 2.0 + 40.0;
        let col1_y = content.min.y + ((content.height() - col1_h) / 2.0).round_ties_even();
        // Column 0: the icon and name, centred.
        let name_font = widgets::font(13.0, 600);
        icons::paint(ui.painter(), Rect::from_min_size(pos2(content.min.x, content.center().y - 10.0), Vec2::splat(20.0)), it.def.icon, p.text);
        let name_w = (col1_x - 16.0 - (content.min.x + 30.0)).max(0.0);
        // `TextTrimming="CharacterEllipsis"`.
        let g = widgets::galley(ui, it.name(), name_font.clone(), p.text, name_w);
        ui.painter().galley(pos2(content.min.x + 30.0, widgets::center_y(content, widgets::line_height(ui, &name_font))), g, p.text);

        // Primary and Secondary: read-only boxes that take a shortcut.
        let mut x = col1_x;
        let clicked_somewhere = ui.input(|i| i.pointer.any_pressed());
        let mut box_clicked = false;
        for (secondary, label, value) in [(false, ie::ToolbarCustomizationDialogView_Primary(), &it.hotkey), (true, ie::ToolbarCustomizationDialogView_Secondary(), &it.secondary_hotkey)] {
            let g = widgets::galley(ui, label, caption.clone(), p.text_secondary, f32::INFINITY);
            ui.painter().galley(pos2(x, col1_y), g, p.text_secondary);
            let r = Rect::from_min_size(pos2(x, col1_y + caption_h + 2.0), vec2(BOX, 40.0));
            let editable = it.def.hotkey_editable;
            let focused = self.focus == Some(secondary);
            let resp = ui.interact(r, Id::new(("tb-key", secondary)), if editable { Sense::click() } else { Sense::hover() });
            let fill = if !editable { p.control_background_disabled } else if resp.hovered() && !focused { p.control_background_hover } else { p.control_background };
            let stroke = if focused { p.accent_border } else { p.border };
            border::border(ui.painter(), r, 4.0, 1.0, Some(Brush::Solid(fill)), Some(stroke));
            if focused {
                // Fluent's focused `TextBox`: a 2px accent bottom edge.
                ui.painter().rect_filled(Rect::from_min_max(pos2(r.min.x + 1.0, r.max.y - 2.0), pos2(r.max.x - 1.0, r.max.y - 1.0)), 0.0, p.accent_border);
            }
            let font = content_font(14.0, 400);
            let w = widgets::text_width(ui, value, font.clone());
            let g = widgets::galley(ui, value, font.clone(), p.text, f32::INFINITY);
            ui.painter().galley(pos2((r.center().x - w / 2.0).round(), widgets::center_y(r, widgets::line_height(ui, &font))), g, p.text);
            let resp = resp.on_hover_text(ie::ToolbarCustomizationDialogView_ClickandpressashortcutBackspaceorDeleteclearsit());
            if resp.clicked() {
                self.focus = Some(secondary);
                self.slot_focus = None;
                box_clicked = true;
            }
            x += BOX + 8.0;
        }
        if clicked_somewhere && !box_clicked && self.focus.is_some() {
            self.focus = None;
        }
    }
}

/// A `Border`'s 1px `ShareX.Brush.Accent` border (scaled by `s` in the
/// shrunk toolbar): inside `rect`, a whole device pixel thick.
fn accent_border(painter: &egui::Painter, rect: Rect, radius: f32, s: f32) {
    let ppp = painter.ctx().pixels_per_point();
    let t = snap(ppp, 1.0) * s;
    border::stroke(painter, rect.shrink(t / 2.0), (radius - t / 2.0).max(0.0), t, Brush::accent(painter.ctx()));
}

/// `Line.spacer-dash`: a 1px line dashed 2 on, 3 off, from the top of `line`.
fn dashes(painter: &egui::Painter, line: Rect, s: f32, color: Color32) {
    let mut y = 0.0;
    while y < line.height() {
        let h = (2.0 * s).min(line.height() - y);
        painter.rect_filled(Rect::from_min_size(pos2(line.min.x, line.min.y + y), vec2(line.width(), h)), 0.0, color);
        y += 5.0 * s;
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
        assert_eq!(r.items.len(), 22);
    }

    #[test]
    fn region_save_is_sharexs() {
        // ShareX ca3f01f's Task settings › Region capture › Customize
        // toolbar..., the Rectangle region (ShareX #9326) dragged off the
        // toolbar, OK: its RegionCaptureOptions.ToolbarItems (the hidden
        // region items last, Tab and NumPad0 kept).
        let expected: Vec<Value> = serde_json::from_str(include_str!("toolbar_editor_region_sharex.json")).unwrap();
        let mut ed = ToolbarEditor::new(Target::RegionCapture, &[]);
        assert_eq!(ed.items[0].id(), ti::REGION_RECTANGLE);
        let k = ed.index_of_slot(ti::REGION_RECTANGLE).unwrap();
        ed.remove_slot(k);
        assert_eq!(ed.save(), expected);
    }

    fn ids(ed: &ToolbarEditor) -> Vec<String> {
        ed.toolbar_slots()
            .iter()
            .map(|s| match s.item {
                Some(id) => id.to_string(),
                None => format!("|{}{}", s.spacer_width, if s.show_line { "" } else { "b" }),
            })
            .collect()
    }

    #[test]
    fn layout_as_sharex() {
        let mut ed = ToolbarEditor::new(Target::ImageEditor, &[]);
        let t = ids(&ed);
        assert_eq!(&t[..4], ["File", "|0", "Select", "Rectangle"]);
        assert_eq!(t.len(), 27);
        assert_eq!(ed.selected, Some("File"));
        assert_eq!(ed.palette_slots().len(), 2);
        // Drag Rectangle before Select: the spacer stays before Rectangle now.
        ed.place_item("Rectangle", 2);
        assert_eq!(&ids(&ed)[..5], ["File", "|0", "Rectangle", "Select", "Ellipse"]);
        assert_eq!(ed.selected, Some("Rectangle"));
        // Off the toolbar: hidden, in the palette after the spacers.
        let k = ed.index_of_slot("Ellipse").unwrap();
        ed.remove_slot(k);
        assert!(!ed.item("Ellipse").unwrap().visible);
        assert_eq!(ed.palette_slots()[2].item, Some("Ellipse"));
        assert_eq!(ed.items.last().unwrap().id(), "Ellipse");
        // File can't be removed.
        ed.remove_slot(0);
        assert_eq!(ids(&ed)[0], "File");
        // A blank spacer between Select and Line, then widened.
        let k = ed.index_of_slot("Line").unwrap();
        ed.place_spacer(k, None, false);
        assert_eq!(&ids(&ed)[3..6], ["Select", "|0b", "Line"]);
        ed.set_spacer_width(k, 23);
        assert_eq!(ids(&ed)[k], "|23b");
        let line = ed.item("Line").unwrap();
        assert!(line.begin_group && !line.show_spacer_line && line.spacer_width == 23);
        // Next to another spacer they merge: the widest, with a line if either has one.
        ed.place_spacer(k, None, true);
        assert_eq!(ids(&ed)[k], "|23");
        // Move the spacer before Select (from the toolbar, keeping its width).
        ed.place_spacer(1, Some(k), true);
        assert_eq!(&ids(&ed)[..4], ["File", "|23", "Rectangle", "Select"]);
        // A spacer at the end is dropped.
        let n = ed.toolbar_slots().len();
        ed.place_spacer(n, None, true);
        assert_eq!(ed.toolbar_slots().len(), n);
        // Double-tapped from the palette: onto the end.
        ed.place_item("Ellipse", n);
        assert_eq!(ids(&ed).last().unwrap(), "Ellipse");
        assert!(ed.item("Ellipse").unwrap().visible);
        // Move left/right.
        ed.move_left("Ellipse");
        let t = ids(&ed);
        assert_eq!(&t[t.len() - 2..], ["Ellipse", "ImageEffects"]);
        ed.move_right("Ellipse");
        assert_eq!(ids(&ed).last().unwrap(), "Ellipse");
        ed.reset();
        assert_eq!(ids(&ed).len(), 27);
        assert_eq!(ed.selected, Some("File"));
    }

    #[test]
    fn saved_spacers_round_trip() {
        let mut ed = ToolbarEditor::new(Target::ImageEditor, &[]);
        let k = ed.index_of_slot("Line").unwrap();
        ed.place_spacer(k, None, false);
        ed.set_spacer_width(k, 40);
        let k = ed.index_of_slot("Crop").unwrap();
        ed.remove_slot(k);
        let saved = ed.save();
        let line = saved.iter().find(|v| v["Id"] == "Line").unwrap();
        assert_eq!((line["BeginGroup"].as_bool(), line["SpacerWidth"].as_i64(), line["HideSpacerLine"].as_bool()), (Some(true), Some(40), Some(true)));
        // Hidden items follow the visible ones.
        assert_eq!(saved.last().unwrap()["Id"], "Crop");
        assert_eq!(saved.last().unwrap()["IsVisible"], false);
        let again = ToolbarEditor::new(Target::ImageEditor, &saved);
        assert_eq!(ids(&again), ids(&ed));
    }

    #[test]
    fn gaps() {
        let rects = [Rect::from_min_size(pos2(10.0, 5.0), vec2(40.0, 40.0)), Rect::from_min_size(pos2(51.0, 5.0), vec2(40.0, 40.0))];
        let area = Rect::from_min_size(pos2(0.0, 0.0), vec2(200.0, 50.0));
        assert_eq!(ToolbarEditor::find_gap(pos2(20.0, 20.0), &rects, area).0, 0);
        let (i, r) = ToolbarEditor::find_gap(pos2(40.0, 20.0), &rects, area);
        assert_eq!((i, r.min.x), (1, 49.0));
        let (i, r) = ToolbarEditor::find_gap(pos2(150.0, 20.0), &rects, area);
        assert_eq!((i, r.min.x), (2, 90.0));
        assert_eq!(ToolbarEditor::find_gap(pos2(5.0, 5.0), &[], area).1.min, pos2(4.0, 4.0));
    }
}
