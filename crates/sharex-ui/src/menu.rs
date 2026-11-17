//! Menus and flyouts: ShareX's `MainMenuEntry` lists drawn as Avalonia
//! draws them.
//!
//! Two presenters, both ported from ShareX:
//!
//! - **Compact menu** (`MainWindow.BuildContextMenu` / `BuildMenuControls`):
//!   a Fluent `ContextMenu` of `MenuItem.compact-menu-item`s
//!   (`ControlStyles.axaml`), with submenus in Fluent `MenuItem` popups.
//!   Each item is Fluent's `MenuItem` template: a grid of toggle, icon,
//!   header, input gesture and chevron columns, the first two and last two
//!   shared across the menu (`SharedSizeGroup`), so a column appears for every
//!   item as soon as one item uses it.
//! - **Tool grid** (`MainWindow.BuildGridMenu`): a `FlyoutPresenter.tool-grid-flyout`
//!   of category headers and three-column `UniformGrid`s of `Button.tool-grid-button`s.
//!
//! Geometry comes from ShareX's styles and the density keys, and from
//! Avalonia's Fluent theme for what ShareX leaves to it (Avalonia 12.1.6,
//! `Avalonia.Themes.Fluent/Controls/MenuItem.xaml`, `ContextMenu.xaml`,
//! `Accents/FluentControlResources.xaml`):
//! `MenuIconPresenterMargin` 0,0,12,0; `MenuInputGestureTextMargin` 24,0,0,0
//! (an empty gesture keeps its margin, so every item ends 24px after its
//! header); `MenuFlyoutItemChevronMargin` 12,0,0,0 and an 8x16 chevron path;
//! `MenuFlyoutScrollerMargin` 0,4,0,4; `MenuFlyoutSubItemPopupHorizontalOffset`
//! -4; `OverlayCornerRadius` 5 for submenus (ShareX sets 6 on `ContextMenu`).
//!
//! A [`Flyout`] is drawn inside its window, as egui areas on top of
//! everything else; Avalonia opens each popup as its own OS window, so a
//! menu can't extend past the window here (it is kept within `bounds`
//! instead, with Avalonia's flip/slide/resize rules).

use egui::{Color32, CornerRadius, FontId, Id, Key, Order, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};

use crate::icons;
use crate::theme::{FONT_SIZE, density, fluent, palette};
use crate::widgets::{self, font};

/// `MenuMainMenuToggleType`: what a menu item's check column shows when
/// it's checked.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Toggle {
    #[default]
    None,
    /// A check mark (`AppIcons.Check`, `ControlStyles.axaml`).
    CheckBox,
    /// Fluent's 8px dot; checking one unchecks its radio siblings.
    Radio,
}

/// A menu item's icon.
#[derive(Clone, Debug, Default)]
pub enum MenuIcon {
    #[default]
    None,
    /// A ShareX icon key, drawn in the accent colour (`accent-menu-icon`).
    Key(&'static str),
    /// A full-colour bitmap, 16x16 (a window's icon in Capture › Window).
    Texture(egui::TextureId),
}

/// One `MainMenuEntry`: an item, a submenu or a separator.
#[derive(Clone, Debug)]
pub struct MenuEntry<C> {
    pub header: String,
    /// The part of `header` drawn in the accent text colour
    /// (`MainMenuEntry.AccentText`: the chosen destination's name).
    pub accent: Option<String>,
    pub icon: MenuIcon,
    pub command: Option<C>,
    pub children: Option<Vec<MenuEntry<C>>>,
    pub separator: bool,
    pub enabled: bool,
    pub visible: bool,
    pub checked: bool,
    pub toggle: Toggle,
    pub stays_open: bool,
    pub bold_when_checked: bool,
    /// The shortcut shown at the right (`InputGesture`).
    pub gesture: Option<String>,
    /// The tool grid's pill (`BadgeText`: "New").
    pub badge: Option<String>,
    /// The tool grid's id for Application settings › hidden tools.
    pub id: Option<String>,
}

impl<C> MenuEntry<C> {
    /// `MainMenuBuilder.Item`.
    pub fn item(header: impl Into<String>, icon: &'static str, command: C) -> Self {
        Self { command: Some(command), ..Self::base(header.into(), icon) }
    }

    /// `MainMenuBuilder.Parent`.
    pub fn parent(header: impl Into<String>, icon: &'static str, children: Vec<MenuEntry<C>>) -> Self {
        Self { children: Some(children), ..Self::base(header.into(), icon) }
    }

    /// An entry that does nothing (`isEnabled: false` placeholders).
    pub fn label(header: impl Into<String>, icon: &'static str) -> Self {
        Self::base(header.into(), icon)
    }

    pub fn separator() -> Self {
        Self { separator: true, enabled: false, ..Self::base(String::new(), "") }
    }

    fn base(header: String, icon: &'static str) -> Self {
        Self {
            header,
            accent: None,
            icon: if icon.is_empty() { MenuIcon::None } else { MenuIcon::Key(icon) },
            command: None,
            children: None,
            separator: false,
            enabled: true,
            visible: true,
            checked: false,
            toggle: Toggle::None,
            stays_open: false,
            bold_when_checked: false,
            gesture: None,
            badge: None,
            id: None,
        }
    }

    pub fn command(mut self, command: C) -> Self {
        self.command = Some(command);
        self
    }
    pub fn children(mut self, children: Vec<MenuEntry<C>>) -> Self {
        self.children = Some(children);
        self
    }
    pub fn toggle(mut self, toggle: Toggle, checked: bool) -> Self {
        self.toggle = toggle;
        self.checked = checked;
        self
    }
    pub fn stays_open(mut self) -> Self {
        self.stays_open = true;
        self
    }
    pub fn bold_when_checked(mut self) -> Self {
        self.bold_when_checked = true;
        self
    }
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
    pub fn visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }
    pub fn accent(mut self, accent: impl Into<String>) -> Self {
        self.accent = Some(accent.into());
        self
    }
    pub fn gesture(mut self, gesture: impl Into<String>) -> Self {
        self.gesture = Some(gesture.into());
        self
    }
    pub fn badge(mut self, badge: impl Into<String>) -> Self {
        self.badge = Some(badge.into());
        self
    }
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }
    pub fn icon(mut self, icon: MenuIcon) -> Self {
        self.icon = icon;
        self
    }

    fn has_children(&self) -> bool {
        self.children.is_some()
    }

    /// Hit-testable: Avalonia gives disabled items no `:pointerover`.
    fn interactive(&self) -> bool {
        self.visible && !self.separator && self.enabled
    }
}

/// `MainMenuCategory`: a heading in the tool grid and its tools, sorted by
/// header (ShareX: `StringComparer.CurrentCultureIgnoreCase`).
#[derive(Clone, Debug)]
pub struct MenuCategory<C> {
    pub header: String,
    pub entries: Vec<MenuEntry<C>>,
}

impl<C> MenuCategory<C> {
    pub fn new(header: impl Into<String>, mut entries: Vec<MenuEntry<C>>) -> Self {
        entries.sort_by_cached_key(|e| e.header.to_lowercase());
        Self { header: header.into(), entries }
    }
}

/// Where a flyout opens relative to the control it's for (Avalonia's
/// `PlacementMode`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Placement {
    /// To the right, top edges aligned (the sidebar's menus).
    RightEdgeAlignedTop,
    /// Below, left edges aligned.
    BottomEdgeAlignedLeft,
    /// At the pointer (context menus).
    Pointer(Pos2),
}

enum Content<C> {
    Menu(Vec<MenuEntry<C>>),
    Grid(Vec<MenuCategory<C>>),
}

/// What a frame of a flyout did.
#[derive(Debug)]
pub struct FlyoutOutput<C> {
    /// The command of the entry the user chose.
    pub command: Option<C>,
    /// The flyout closed (a command ran, Escape, or the tool grid's choice).
    pub closed: bool,
}

/// `MenuShowDelay` in Avalonia's `DefaultMenuInteractionHandler`: how long
/// the pointer rests on an item before its submenu opens (or a sibling's
/// closes).
const MENU_SHOW_DELAY: f64 = 0.4;

/// An open context menu or tool grid, with its submenus.
pub struct Flyout<C> {
    id: Id,
    content: Content<C>,
    anchor: Rect,
    placement: Placement,
    /// The entry whose submenu is open, per level from the root down.
    open: Vec<usize>,
    /// The highlighted entry per level (`MenuItem.IsSelected`).
    selected: Vec<Option<usize>>,
    /// A submenu to open or close once the pointer has rested on an item.
    pending: Option<(usize, usize, f64)>,
    /// Ids of tools hidden from the grid (`ApplicationConfig.HiddenTools`).
    pub hidden_tools: Vec<String>,
    /// The tool grid's edit button text (`MainMenuBuilder_EditTools`).
    pub edit_label: String,
    /// Where each level was drawn last frame, for light dismiss.
    rects: Vec<Rect>,
}

/// A level's geometry, before it's placed.
struct MenuMetrics {
    size: Vec2,
    toggle_col: f32,
    icon_col: f32,
    header_col: f32,
    gesture_col: f32,
    chevron_col: f32,
}

impl<C: Clone> Flyout<C> {
    /// A compact menu of `entries` (`MainWindow.TryOpenContextMenu`).
    pub fn menu(id: Id, entries: Vec<MenuEntry<C>>, anchor: Rect, placement: Placement) -> Self {
        Self::new(id, Content::Menu(entries), anchor, placement)
    }

    /// The tool grid (`MainWindow.TryOpenGridMenu`).
    pub fn grid(id: Id, categories: Vec<MenuCategory<C>>, anchor: Rect, placement: Placement) -> Self {
        Self::new(id, Content::Grid(categories), anchor, placement)
    }

    fn new(id: Id, content: Content<C>, anchor: Rect, placement: Placement) -> Self {
        Self {
            id,
            content,
            anchor,
            placement,
            open: Vec::new(),
            selected: vec![None],
            pending: None,
            hidden_tools: Vec::new(),
            edit_label: String::new(),
            rects: Vec::new(),
        }
    }

    /// Whether `pos` is on the flyout or one of its submenus (as drawn
    /// last frame). A press anywhere else closes it (light dismiss), and
    /// Avalonia swallows that press, so the host should too.
    pub fn contains(&self, pos: Pos2) -> bool {
        self.rects.iter().any(|r| r.contains(pos))
    }

    /// Opens the submenus along `path` (entry indices from the root),
    /// highlighting each parent as if the pointer had passed over it, for
    /// rendering a state without scripted hovers.
    pub fn open_path(&mut self, path: &[usize]) {
        self.open = path.to_vec();
        self.selected = path.iter().map(|i| Some(*i)).chain([None]).collect();
    }

    /// The entries of level `depth` (0 is the root menu).
    fn entries(&self, depth: usize) -> Option<&[MenuEntry<C>]> {
        let Content::Menu(root) = &self.content else { return None };
        let mut entries: &[MenuEntry<C>] = root;
        for &i in &self.open[..depth] {
            entries = entries.get(i)?.children.as_deref()?;
        }
        Some(entries)
    }

    fn entries_mut(&mut self, depth: usize) -> Option<&mut Vec<MenuEntry<C>>> {
        let Content::Menu(root) = &mut self.content else { return None };
        let mut entries = root;
        for &i in &self.open[..depth] {
            entries = entries.get_mut(i)?.children.as_mut()?;
        }
        Some(entries)
    }

    /// Draws the flyout on top of `ui`'s window, keeping it within
    /// `bounds` (the screen's work area in Avalonia), and handles its input.
    pub fn show(&mut self, ui: &Ui, bounds: Rect) -> FlyoutOutput<C> {
        // An open menu has the keyboard (Avalonia focuses it), so the
        // control that opened it mustn't take arrows and Enter.
        ui.ctx().memory_mut(|m| {
            if let Some(id) = m.focused() {
                m.surrender_focus(id);
            }
        });
        match self.content {
            Content::Menu(_) => self.show_menu(ui, bounds),
            Content::Grid(_) => self.show_grid(ui, bounds),
        }
    }

    // ---- compact menu -------------------------------------------------

    fn show_menu(&mut self, ui: &Ui, bounds: Rect) -> FlyoutOutput<C> {
        let ctx = ui.ctx().clone();
        let d = density(&ctx);
        let now = ctx.input(|i| i.time);
        let mut out = FlyoutOutput { command: None, closed: false };
        // Drop open submenus whose entry went away.
        while !self.open.is_empty() && self.entries(self.open.len()).is_none() {
            self.open.pop();
        }
        self.selected.resize(self.open.len() + 1, None);

        // Place every level: the root at its anchor, each submenu beside the
        // item that opened it.
        let mut placed: Vec<(Rect, MenuMetrics, Vec<Rect>)> = Vec::new();
        for depth in 0..=self.open.len() {
            let entries = self.entries(depth).expect("checked above");
            let root = depth == 0;
            let m = menu_metrics(ui, entries, root);
            let rect = if root {
                place(self.anchor_rect(), self.placement, m.size, 0.0, bounds)
            } else {
                // `PART_Popup`: RightEdgeAlignedTop of the item, -4px.
                let (_, _, items) = &placed[depth - 1];
                let item = items[self.open[depth - 1]];
                place(item, Placement::RightEdgeAlignedTop, m.size, -4.0, bounds)
            };
            let items = item_rects(entries, rect, root, &d);
            placed.push((rect, m, items));
        }
        self.rects = placed.iter().map(|(r, ..)| *r).collect();

        // What the pointer is over, deepest level first (submenus are on top).
        let pointer = ctx.pointer_latest_pos();
        let mut hovered: Option<(usize, usize)> = None;
        if let Some(p) = pointer {
            for depth in (0..placed.len()).rev() {
                let (rect, _, items) = &placed[depth];
                if rect.contains(p) {
                    let entries = self.entries(depth).unwrap();
                    hovered = items
                        .iter()
                        .enumerate()
                        .find(|(i, r)| r.contains(p) && entries[*i].interactive())
                        .map(|(i, _)| (depth, i));
                    break;
                }
            }
        }

        // Draw, and collect the clicks.
        let mut clicked = None;
        let mut pressed_parent = None;
        for (depth, (rect, m, items)) in placed.iter().enumerate() {
            let entries = self.entries(depth).unwrap();
            let layer = Id::new((self.id, "level", depth));
            let selected = self.selected[depth];
            let open = self.open.get(depth).copied();
            let response = egui::Area::new(layer)
                .order(Order::Foreground)
                .fixed_pos(rect.min)
                .constrain(false)
                .show(&ctx, |ui| {
                    ui.set_clip_rect(*rect);
                    let (frame, _) = ui.allocate_exact_size(rect.size(), Sense::click());
                    paint_menu_frame(ui, frame, depth == 0);
                    for (i, (entry, item)) in entries.iter().zip(items).enumerate() {
                        if !entry.visible {
                            continue;
                        }
                        if entry.separator {
                            paint_separator(ui, *item, &d);
                            continue;
                        }
                        let id = layer.with(i);
                        let sense = if entry.interactive() { Sense::click() } else { Sense::hover() };
                        let r = ui.interact(*item, id, sense);
                        let state = ItemState {
                            hovered: hovered == Some((depth, i)),
                            selected: selected == Some(i),
                            open: open == Some(i),
                        };
                        paint_item(ui, *item, entry, m, state);
                        if entry.interactive() {
                            if r.clicked() {
                                clicked = Some((depth, i));
                            } else if entry.has_children() && r.is_pointer_button_down_on() && open != Some(i) {
                                pressed_parent = Some((depth, i));
                            }
                        }
                    }
                });
            ctx.move_to_top(response.response.layer_id);
        }

        // Hover: highlight, and open/close submenus after `MenuShowDelay`.
        // Leaving an item un-highlights it unless its submenu is open
        // (keyboard selection stays until the pointer moves).
        let mut changed = false;
        let moved = ctx.input(|i| i.pointer.delta() != Vec2::ZERO);
        for depth in 0..placed.len() {
            let sel = match hovered {
                Some((d, i)) if d == depth => Some(i),
                _ if moved => self.open.get(depth).copied(),
                _ => continue,
            };
            if self.selected[depth] != sel {
                self.selected[depth] = sel;
                changed = true;
            }
        }
        match hovered {
            Some((depth, i)) => {
                let has_children = self.entries(depth).unwrap()[i].has_children();
                let already_open = self.open.get(depth) == Some(&i);
                let needs = if has_children { !already_open } else { self.open.len() > depth };
                match self.pending {
                    Some((pd, pi, at)) if pd == depth && pi == i => {
                        if now >= at {
                            self.pending = None;
                            self.open.truncate(depth);
                            if has_children {
                                self.open.push(i);
                            }
                            changed = true;
                        } else {
                            ctx.request_repaint_after(std::time::Duration::from_secs_f64(at - now));
                        }
                    }
                    _ if needs => {
                        self.pending = Some((depth, i, now + MENU_SHOW_DELAY));
                        ctx.request_repaint_after(std::time::Duration::from_secs_f64(MENU_SHOW_DELAY));
                    }
                    _ => self.pending = None,
                }
            }
            None => self.pending = None,
        }

        // Pressing a parent opens its submenu at once.
        if let Some((depth, i)) = pressed_parent {
            self.open.truncate(depth);
            self.open.push(i);
            self.pending = None;
            changed = true;
        }
        if let Some((depth, i)) = clicked {
            self.activate(depth, i, &mut out);
            changed = true;
        }

        // Keyboard (`DefaultMenuInteractionHandler.KeyDown`), on the deepest level.
        if !out.closed {
            self.keyboard(&ctx, &mut out, &mut changed);
        }
        if changed {
            ctx.request_repaint();
        }
        out
    }

    fn anchor_rect(&self) -> Rect {
        match self.placement {
            Placement::Pointer(p) => Rect::from_min_size(p, Vec2::ZERO),
            _ => self.anchor,
        }
    }

    /// A click on (or Enter on) entry `i` of level `depth`: run its
    /// command and toggle it, open its submenu, close the menu unless it
    /// `StaysOpenOnClick`.
    fn activate(&mut self, depth: usize, i: usize, out: &mut FlyoutOutput<C>) {
        let Some(entries) = self.entries_mut(depth) else { return };
        let entry = &entries[i];
        let has_children = entry.has_children();
        let command = entry.command.clone();
        let stays_open = entry.stays_open;
        match entry.toggle {
            Toggle::CheckBox => entries[i].checked = !entries[i].checked,
            Toggle::Radio => {
                for (j, e) in entries.iter_mut().enumerate() {
                    if e.toggle == Toggle::Radio {
                        e.checked = j == i;
                    }
                }
            }
            Toggle::None => {}
        }
        if has_children {
            self.open.truncate(depth);
            self.open.push(i);
            self.pending = None;
        }
        if let Some(command) = command {
            out.command = Some(command);
            if !stays_open && !has_children {
                out.closed = true;
            }
        }
    }

    /// Every navigation key pressed this frame, in order (several can
    /// arrive between two frames).
    fn take_keys(ctx: &egui::Context) -> Vec<Key> {
        ctx.input_mut(|i| {
            let mut keys = Vec::new();
            i.events.retain(|e| match e {
                egui::Event::Key { key, pressed: true, modifiers, .. }
                    if modifiers.is_none()
                        && matches!(
                            key,
                            Key::Escape | Key::ArrowDown | Key::ArrowUp | Key::ArrowLeft | Key::ArrowRight | Key::Enter
                        ) =>
                {
                    keys.push(*key);
                    false
                }
                _ => true,
            });
            keys
        })
    }

    fn keyboard(&mut self, ctx: &egui::Context, out: &mut FlyoutOutput<C>, changed: &mut bool) {
        for key in Self::take_keys(ctx) {
            if out.closed {
                break;
            }
            self.key(key, out);
            *changed = true;
        }
    }

    /// `DefaultMenuInteractionHandler.KeyDown`, on the deepest open level.
    fn key(&mut self, key: Key, out: &mut FlyoutOutput<C>) {
        let depth = self.open.len();
        self.selected.resize(depth + 1, None);
        if key == Key::Escape {
            if self.open.pop().is_none() {
                out.closed = true;
            }
            self.selected.truncate(self.open.len() + 1);
            return;
        }
        let Some(entries) = self.entries(depth) else { return };
        let usable: Vec<usize> = (0..entries.len()).filter(|&i| entries[i].interactive()).collect();
        let current = self.selected[depth];
        let step = |forward: bool| -> Option<usize> {
            if usable.is_empty() {
                return None;
            }
            let pos = current.and_then(|c| usable.iter().position(|&u| u == c));
            Some(match (pos, forward) {
                (None, true) => usable[0],
                (None, false) => usable[usable.len() - 1],
                (Some(p), true) => usable[(p + 1) % usable.len()],
                (Some(p), false) => usable[(p + usable.len() - 1) % usable.len()],
            })
        };
        match key {
            Key::ArrowDown => self.selected[depth] = step(true),
            Key::ArrowUp => self.selected[depth] = step(false),
            Key::ArrowRight => {
                if let Some(i) = current.filter(|&i| entries[i].has_children()) {
                    self.open.push(i);
                    let first = self.entries(depth + 1).and_then(|e| (0..e.len()).find(|&j| e[j].interactive()));
                    self.selected.push(first);
                }
            }
            Key::ArrowLeft if depth > 0 => {
                self.open.pop();
                self.selected.truncate(depth);
            }
            Key::Enter => {
                if let Some(i) = current {
                    self.activate(depth, i, out);
                }
            }
            _ => {}
        }
    }

    // ---- tool grid ----------------------------------------------------

    fn show_grid(&mut self, ui: &Ui, bounds: Rect) -> FlyoutOutput<C> {
        let ctx = ui.ctx().clone();
        let mut out = FlyoutOutput { command: None, closed: false };
        let Content::Grid(categories) = &self.content else { unreachable!() };
        let g = GridLayout::new(ui, categories, &self.hidden_tools, &self.edit_label);
        let rect = place(self.anchor_rect(), self.placement, g.size, 0.0, bounds);
        self.rects = vec![rect];
        let layer = Id::new((self.id, "grid"));
        let mut chosen = None;
        let response = egui::Area::new(layer)
            .order(Order::Foreground)
            .fixed_pos(rect.min)
            .constrain(false)
            .show(&ctx, |ui| {
                ui.set_clip_rect(rect);
                let (frame, _) = ui.allocate_exact_size(rect.size(), Sense::click());
                chosen = g.paint(ui, frame, categories, &self.hidden_tools, &self.edit_label, layer);
            });
        ctx.move_to_top(response.response.layer_id);
        if let Some(command) = chosen {
            out.command = Some(command);
            out.closed = true;
        }
        if Self::take_keys(&ctx).contains(&Key::Escape) {
            out.closed = true;
        }
        out
    }
}

/// Avalonia's `ManagedPopupPositioner` for the placements ShareX uses, with
/// the default constraint adjustment (flip, then slide, then resize).
fn place(anchor: Rect, placement: Placement, size: Vec2, offset_x: f32, bounds: Rect) -> Rect {
    let (mut x, mut y) = match placement {
        Placement::RightEdgeAlignedTop => (anchor.max.x + offset_x, anchor.min.y),
        Placement::BottomEdgeAlignedLeft => (anchor.min.x, anchor.max.y),
        Placement::Pointer(p) => (p.x, p.y),
    };
    let fits_x = |x: f32| x >= bounds.min.x && x + size.x <= bounds.max.x;
    let fits_y = |y: f32| y >= bounds.min.y && y + size.y <= bounds.max.y;
    if !fits_x(x) {
        let flipped = match placement {
            Placement::RightEdgeAlignedTop => anchor.min.x - offset_x - size.x,
            Placement::BottomEdgeAlignedLeft => anchor.max.x - size.x,
            Placement::Pointer(p) => p.x - size.x,
        };
        if fits_x(flipped) {
            x = flipped;
        }
    }
    if x + size.x > bounds.max.x {
        x = bounds.max.x - size.x;
    }
    x = x.max(bounds.min.x);
    if !fits_y(y) {
        let flipped = match placement {
            Placement::RightEdgeAlignedTop => anchor.max.y - size.y,
            Placement::BottomEdgeAlignedLeft => anchor.min.y - size.y,
            Placement::Pointer(p) => p.y - size.y,
        };
        if fits_y(flipped) {
            y = flipped;
        }
    }
    if y + size.y > bounds.max.y {
        y = bounds.max.y - size.y;
    }
    y = y.max(bounds.min.y);
    let size = vec2(size.x.min(bounds.width()), size.y.min(bounds.height()));
    Rect::from_min_size(pos2(x.round(), y.round()), size)
}

/// The header's font. A plain string header is drawn by the item's
/// `ContentPresenter`, not a `TextBlock`, so ShareX's Segoe UI style
/// doesn't reach it: it's Avalonia's default family, Inter (ShareX calls
/// `.WithInterFont()`), at the `ContextMenu MenuItem` style's 13px and the
/// item's weight (Normal; Bold when a `bold-when-checked` item is checked).
/// A header with accent text is a `TextBlock` in ShareX's family, Segoe
/// UI 13px, at the item's weight (measured: ShareX's global SemiBold
/// `TextBlock` style doesn't apply to it).
fn header_font<C>(entry: &MenuEntry<C>) -> FontId {
    let bold = entry.bold_when_checked && entry.checked;
    if entry.accent.is_some() {
        return font(FONT_SIZE, if bold { 700 } else { 400 });
    }
    FontId::new(FONT_SIZE, crate::fonts::inter(if bold { 700 } else { 400 }))
}

/// The input gesture is a `TextBlock` too (Fluent template), so SemiBold.
fn gesture_font() -> FontId {
    font(FONT_SIZE, 600)
}

fn menu_metrics<C>(ui: &Ui, entries: &[MenuEntry<C>], root: bool) -> MenuMetrics {
    let d = density(ui.ctx());
    let shown = || entries.iter().filter(|e| e.visible && !e.separator);
    let icon_margin = d.menu_icon_presenter_margin.right;
    // The toggle presenter is only visible on checked items.
    let toggle_col = if shown().any(|e| e.checked && e.toggle != Toggle::None) { 16.0 + icon_margin } else { 0.0 };
    let icon_col = if shown().any(|e| !matches!(e.icon, MenuIcon::None)) { 16.0 + icon_margin } else { 0.0 };
    let header_col = shown().map(|e| widgets::text_width(ui, &e.header, header_font(e))).fold(0.0, f32::max);
    let gesture_col = 24.0
        + shown()
            .filter_map(|e| e.gesture.as_deref())
            .map(|g| widgets::text_width(ui, g, gesture_font()))
            .fold(0.0, f32::max);
    let chevron_col = if shown().any(|e| e.has_children()) { 12.0 + 8.0 } else { 0.0 };
    let pad = d.menu_item_padding;
    let item_w = pad.left + toggle_col + icon_col + header_col + gesture_col + chevron_col + pad.right;
    let mut h = 0.0;
    for e in entries.iter().filter(|e| e.visible) {
        h += if e.separator { 1.0 + d.menu_separator_margin.top + d.menu_separator_margin.bottom } else { d.menu_item_height };
    }
    let frame = frame_padding(&d, root);
    let size = vec2(
        1.0 + frame.left + item_w + frame.right + 1.0,
        (1.0 + frame.top + 4.0 + h + 4.0 + frame.bottom + 1.0).max(32.0),
    );
    MenuMetrics { size, toggle_col, icon_col, header_col, gesture_col, chevron_col }
}

/// A root `ContextMenu` takes `ShareX.Density.Menu.Padding`; a submenu's
/// popup `MenuFlyoutPresenterThemePadding`.
fn frame_padding(d: &crate::theme::Density, root: bool) -> crate::theme::Thickness {
    if root { d.menu_padding } else { d.menu_flyout_presenter_padding }
}

fn item_rects<C>(entries: &[MenuEntry<C>], rect: Rect, root: bool, d: &crate::theme::Density) -> Vec<Rect> {
    let frame = frame_padding(d, root);
    let left = rect.min.x + 1.0 + frame.left;
    let right = rect.max.x - 1.0 - frame.right;
    let mut y = rect.min.y + 1.0 + frame.top + 4.0;
    entries
        .iter()
        .map(|e| {
            let h = if !e.visible {
                0.0
            } else if e.separator {
                1.0 + d.menu_separator_margin.top + d.menu_separator_margin.bottom
            } else {
                d.menu_item_height
            };
            let r = Rect::from_min_size(pos2(left, y), vec2(right - left, h));
            y += h;
            r
        })
        .collect()
}

/// The popup's border: the root `ContextMenu` in ShareX's border colour,
/// rounded 6; a submenu in Fluent's translucent `MenuFlyoutPresenterBorderBrush`,
/// rounded `OverlayCornerRadius` (5). The background sits inside the border.
fn paint_menu_frame(ui: &Ui, rect: Rect, root: bool) {
    let p = palette(ui.ctx());
    let (radius, border) = if root { (6, p.border) } else { (5, fluent(ui.ctx()).transient_border) };
    ui.painter().rect_filled(rect.shrink(1.0), CornerRadius::same(radius - 1), p.background_menu);
    ui.painter().rect_stroke(rect, CornerRadius::same(radius), Stroke::new(1.0, border), StrokeKind::Inside);
}

/// `Separator.compact-menu-separator`: 1px of `ShareX.Brush.Separator`
/// inside `ShareX.Density.Menu.SeparatorMargin`.
fn paint_separator(ui: &Ui, rect: Rect, d: &crate::theme::Density) {
    let m = d.menu_separator_margin;
    let line = Rect::from_min_max(
        pos2(rect.min.x + m.left, rect.min.y + m.top),
        pos2(rect.max.x - m.right, rect.min.y + m.top + 1.0),
    );
    ui.painter().rect_filled(line, 0.0, palette(ui.ctx()).separator);
}

#[derive(Clone, Copy)]
struct ItemState {
    /// The pointer is on it (`:pointerover`).
    hovered: bool,
    /// Highlighted without the pointer: its submenu is open, or the
    /// keyboard moved to it (`:selected`).
    selected: bool,
    /// Its submenu is open (`:open`).
    open: bool,
}

/// `MenuFlyoutSubItemChevron`'s path, `MenuItemChevronPathData`.
const CHEVRON_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 20" width="10" height="20"><path fill="currentColor" d="M 1,0 10,10 l -9,10 -1,-1 L 8,10 -0,1 Z"/></svg>"#;

/// One `MenuItem.compact-menu-item`.
fn paint_item<C>(ui: &Ui, rect: Rect, entry: &MenuEntry<C>, m: &MenuMetrics, state: ItemState) {
    let ctx = ui.ctx();
    let p = palette(ctx);
    let f = fluent(ctx);
    let d = density(ctx);
    let painter = ui.painter();
    let disabled = !entry.enabled;
    // ShareX's `:pointerover` fill wins over Fluent's `:selected` one; an
    // item highlighted without the pointer shows Fluent's.
    if state.hovered && !disabled {
        painter.rect_filled(rect, CornerRadius::same(4), p.control_background_hover);
    } else if state.selected && !disabled {
        painter.rect_filled(rect, CornerRadius::same(4), f.list_low);
    }
    let header_color = if disabled {
        f.base_medium_low
    } else if state.hovered {
        p.text
    } else if state.selected {
        f.base_high
    } else {
        p.text
    };

    let pad = d.menu_item_padding;
    let content = Rect::from_min_max(pos2(rect.min.x + pad.left, rect.min.y + pad.top), pos2(rect.max.x - pad.right, rect.max.y - pad.bottom));
    let mut x = content.min.x;
    let box_at = |x: f32| Rect::from_min_size(pos2(x, widgets::center_y(content, 16.0)), Vec2::splat(16.0));

    if m.toggle_col > 0.0 {
        if entry.checked {
            let b = box_at(x);
            match entry.toggle {
                Toggle::CheckBox => icons::paint(painter, b, crate::keys::app_icons::Check, p.accent_icon),
                // An 8x8 `Ellipse` with Margin 4, in the item's foreground.
                Toggle::Radio => {
                    painter.circle_filled(b.center(), 4.0, p.text);
                }
                Toggle::None => {}
            }
        }
        x += m.toggle_col;
    }
    if m.icon_col > 0.0 {
        let b = box_at(x);
        match entry.icon {
            MenuIcon::Key(key) => icons::paint(painter, b, key, p.accent_icon),
            MenuIcon::Texture(tex) => {
                painter.image(tex, b, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            }
            MenuIcon::None => {}
        }
        x += m.icon_col;
    }

    // Header, centred vertically.
    let hf = header_font(entry);
    let y = widgets::center_y(content, widgets::line_height(ui, &hf));
    let mut job = egui::text::LayoutJob::default();
    let plain = egui::TextFormat::simple(hf.clone(), header_color);
    match entry.accent.as_deref().and_then(|a| entry.header.find(a).map(|at| (a, at))) {
        Some((accent, at)) => {
            // A `TextBlock`: its own `Text.Secondary` foreground, whatever the item's state.
            let text = egui::TextFormat::simple(hf.clone(), p.text_secondary);
            job.append(&entry.header[..at], 0.0, text.clone());
            job.append(accent, 0.0, egui::TextFormat::simple(hf.clone(), p.accent_text));
            job.append(&entry.header[at + accent.len()..], 0.0, text);
        }
        None => job.append(&entry.header, 0.0, plain),
    }
    job.wrap = egui::text::TextWrapping::truncate_at_width(m.header_col + 1.0);
    let galley = ui.fonts_mut(|fonts| fonts.layout_job(job));
    painter.galley(pos2(x, y), galley, header_color);
    x += m.header_col;

    if let Some(g) = &entry.gesture {
        let gf = gesture_font();
        let w = widgets::text_width(ui, g, gf.clone());
        let right = x + m.gesture_col;
        let color = if disabled { f.base_medium_low } else { p.text_secondary };
        let galley = widgets::galley(ui, g, gf.clone(), color, f32::INFINITY);
        painter.galley(pos2(right - w, widgets::center_y(content, widgets::line_height(ui, &gf))), galley, color);
    }
    x += m.gesture_col;

    if entry.has_children() && m.chevron_col > 0.0 {
        let color = if disabled {
            f.base_medium_low
        } else if state.hovered || state.selected || state.open {
            f.base_high
        } else {
            f.base_medium_high
        };
        let c = Rect::from_min_size(pos2(x + m.chevron_col - 8.0, widgets::center_y(content, 16.0)), vec2(8.0, 16.0));
        icons::paint_svg(painter, c, "menu-chevron", CHEVRON_SVG, color);
    }
}

// ---- tool grid layout -------------------------------------------------

/// `MainWindow.BuildGridMenuContent`, measured.
struct GridLayout {
    size: Vec2,
    /// Each column's width (`UniformGrid Columns="3"`).
    cell_w: f32,
    header_h: f32,
    edit_w: f32,
}

/// `TextBlock.tool-grid-category-header`.
fn grid_header_font(d: &crate::theme::Density) -> FontId {
    font(d.tool_grid_header_font_size, 400)
}

/// A tool's label: a `TextBlock`, Segoe UI 13px Regular (measured: ShareX's
/// global SemiBold `TextBlock` style doesn't apply inside its popups).
fn tool_font() -> FontId {
    font(FONT_SIZE, 400)
}

/// `Border.tool-grid-badge > TextBlock`.
fn badge_font() -> FontId {
    font(10.0, 600)
}

impl GridLayout {
    fn visible<'a, C>(category: &'a MenuCategory<C>, hidden: &'a [String]) -> impl Iterator<Item = &'a MenuEntry<C>> + 'a {
        category
            .entries
            .iter()
            .filter(move |e| e.visible && !e.separator && !e.id.as_ref().is_some_and(|id| hidden.contains(id)))
    }

    fn new<C>(ui: &Ui, categories: &[MenuCategory<C>], hidden: &[String], edit_label: &str) -> Self {
        let d = density(ui.ctx());
        let bp = d.tool_grid_button_padding;
        let sp = d.tool_grid_item_spacing;
        let cell = |e: &MenuEntry<C>| {
            let mut w = bp.left + 16.0 + sp + widgets::text_width(ui, &e.header, tool_font()) + bp.right;
            if let Some(b) = &e.badge {
                w += sp + 6.0 + widgets::text_width(ui, b, badge_font()) + 6.0;
            }
            w
        };
        let header_h = widgets::line_height(ui, &grid_header_font(&d));
        let mut content_w: f32 = 0.0;
        let mut h = 0.0;
        let mut first = true;
        for c in categories {
            let entries: Vec<_> = Self::visible(c, hidden).collect();
            if entries.is_empty() {
                continue;
            }
            let m = if first { d.tool_grid_first_header_margin } else { d.tool_grid_header_margin };
            if !first {
                let s = d.tool_grid_separator_margin;
                h += s.top + 1.0 + s.bottom;
            }
            let header_w = m.left + widgets::text_width(ui, &c.header, grid_header_font(&d)) + m.right;
            let max_cell = entries.iter().map(|e| cell(e)).fold(0.0, f32::max);
            content_w = content_w.max(3.0 * max_cell).max(header_w);
            h += m.top + header_h + m.bottom + entries.len().div_ceil(3) as f32 * d.tool_grid_button_height;
            first = false;
        }
        // `Button.tool-grid-edit-button`: 14px icon, 6, "Edit", padding 5,1, margin 2,0.
        let edit_w = 5.0 + 14.0 + 6.0 + widgets::text_width(ui, edit_label, tool_font()) + 5.0;
        content_w = content_w.max(2.0 + edit_w + 2.0).ceil();
        let pad = d.tool_grid_padding;
        let size = vec2(1.0 + pad.left + content_w + pad.right + 1.0, 1.0 + pad.top + h.max(22.0) + pad.bottom + 1.0);
        Self { size, cell_w: content_w / 3.0, header_h, edit_w }
    }

    /// Draws the grid in `rect`; returns the command of a tool clicked.
    #[allow(clippy::too_many_arguments)]
    fn paint<C: Clone>(
        &self,
        ui: &Ui,
        rect: Rect,
        categories: &[MenuCategory<C>],
        hidden: &[String],
        edit_label: &str,
        layer: Id,
    ) -> Option<C> {
        let ctx = ui.ctx();
        let p = palette(ctx);
        let d = density(ctx);
        let painter = ui.painter();
        // `FlyoutPresenter.tool-grid-flyout`: menu background, ShareX border, radius 8.
        painter.rect_filled(rect.shrink(1.0), CornerRadius::same(7), p.background_menu);
        painter.rect_stroke(rect, CornerRadius::same(8), Stroke::new(1.0, p.border), StrokeKind::Inside);
        let pad = d.tool_grid_padding;
        let content = Rect::from_min_max(
            pos2(rect.min.x + 1.0 + pad.left, rect.min.y + 1.0 + pad.top),
            pos2(rect.max.x - 1.0 - pad.right, rect.max.y - 1.0 - pad.bottom),
        );
        let hf = grid_header_font(&d);
        let mut y = content.min.y;
        let mut first = true;
        let mut chosen = None;
        for (ci, c) in categories.iter().enumerate() {
            let entries: Vec<_> = Self::visible(c, hidden).collect();
            if entries.is_empty() {
                continue;
            }
            if !first {
                // `Border.tool-grid-category-separator`.
                let s = d.tool_grid_separator_margin;
                let line = Rect::from_min_max(pos2(content.min.x + s.left, y + s.top), pos2(content.max.x - s.right, y + s.top + 1.0));
                painter.rect_filled(line, 0.0, p.separator);
                y += s.top + 1.0 + s.bottom;
            }
            let m = if first { d.tool_grid_first_header_margin } else { d.tool_grid_header_margin };
            // Centred (`TextAlignment="Center"`) between the margins.
            let header = Rect::from_min_max(pos2(content.min.x + m.left, y + m.top), pos2(content.max.x - m.right, y + m.top + self.header_h));
            let w = widgets::text_width(ui, &c.header, hf.clone());
            let g = widgets::galley(ui, &c.header, hf.clone(), p.accent_text, f32::INFINITY);
            painter.galley(pos2(header.min.x + ((header.width() - w) / 2.0).round_ties_even(), header.min.y), g, p.accent_text);
            y = header.max.y + m.bottom;
            for (i, e) in entries.iter().enumerate() {
                let (row, col) = (i / 3, i % 3);
                let cell = Rect::from_min_size(
                    pos2(content.min.x + (col as f32 * self.cell_w).round(), y + row as f32 * d.tool_grid_button_height),
                    vec2(((col + 1) as f32 * self.cell_w).round() - (col as f32 * self.cell_w).round(), d.tool_grid_button_height),
                );
                let sense = if e.enabled { Sense::click() } else { Sense::hover() };
                let r = ui.interact(cell, layer.with((ci, i)), sense);
                if e.enabled && r.hovered() {
                    painter.rect_filled(cell, CornerRadius::same(5), p.control_background_hover);
                }
                if e.enabled && r.clicked() {
                    chosen = e.command.clone();
                }
                paint_tool(ui, cell, e);
            }
            y += entries.len().div_ceil(3) as f32 * d.tool_grid_button_height;
            first = false;
        }

        // The edit button (`Button.tool-grid-edit-button`), over the first
        // header at the top right. Its edit mode (a visibility switch on
        // every tool) isn't ported yet.
        let edit = Rect::from_min_size(pos2(content.max.x - 2.0 - self.edit_w, content.min.y), vec2(self.edit_w, 22.0));
        let r = ui.interact(edit, layer.with("edit"), Sense::click());
        if r.hovered() {
            painter.rect_filled(edit, CornerRadius::same(5), p.control_background_hover);
        }
        let inner = Rect::from_min_max(pos2(edit.min.x + 5.0, edit.min.y + 1.0), pos2(edit.max.x - 5.0, edit.max.y - 1.0));
        icons::paint(painter, Rect::from_min_size(pos2(inner.min.x, widgets::center_y(inner, 14.0)), Vec2::splat(14.0)), crate::keys::app_icons::Edit, p.accent_icon);
        let tf = tool_font();
        let g = widgets::galley(ui, edit_label, tf.clone(), p.text_secondary, f32::INFINITY);
        painter.galley(pos2(inner.min.x + 14.0 + 6.0, widgets::center_y(inner, widgets::line_height(ui, &tf))), g, p.text_secondary);
        chosen
    }
}

/// `Button.tool-grid-button`'s content (`CreateGridMenuItemContent`): icon,
/// label and badge in a row, `ShareX.Density.ToolGrid.ItemSpacing` apart.
fn paint_tool<C>(ui: &Ui, cell: Rect, e: &MenuEntry<C>) {
    let p = palette(ui.ctx());
    let d = density(ui.ctx());
    let bp = d.tool_grid_button_padding;
    let sp = d.tool_grid_item_spacing;
    let inner = Rect::from_min_max(pos2(cell.min.x + bp.left, cell.min.y + bp.top), pos2(cell.max.x - bp.right, cell.max.y - bp.bottom));
    let painter = ui.painter();
    // The content is a StackPanel centred vertically (Fluent's
    // `VerticalContentAlignment`), as tall as its tallest child.
    let tf = tool_font();
    let text_h = widgets::line_height(ui, &tf);
    let badge_h = e.badge.as_ref().map_or(0.0, |_| 1.0 + widgets::line_height(ui, &badge_font()) + 1.0);
    let row_h = text_h.max(16.0).max(badge_h);
    let row = Rect::from_min_size(pos2(inner.min.x, widgets::center_y(inner, row_h)), vec2(inner.width(), row_h));
    if let MenuIcon::Key(key) = e.icon {
        icons::paint(painter, Rect::from_min_size(pos2(row.min.x, widgets::center_y(row, 16.0)), Vec2::splat(16.0)), key, p.accent_icon);
    }
    let x = row.min.x + 16.0 + sp;
    let badge_w = e.badge.as_ref().map_or(0.0, |b| sp + 6.0 + widgets::text_width(ui, b, badge_font()) + 6.0);
    let label_w = widgets::text_width(ui, &e.header, tf.clone()).min((row.max.x - x - badge_w).max(0.0));
    let g = widgets::galley(ui, &e.header, tf.clone(), p.text_secondary, label_w + 0.5);
    painter.galley(pos2(x, widgets::center_y(row, text_h)), g, p.text_secondary);
    if let Some(b) = &e.badge {
        let bf = badge_font();
        let bw = 6.0 + widgets::text_width(ui, b, bf.clone()) + 6.0;
        let bx = x + label_w + sp;
        let pill = Rect::from_min_size(pos2(bx, widgets::center_y(row, badge_h)), vec2(bw, badge_h));
        // `ShareX.Brush.Accent`: a diagonal gradient, `CornerRadius="999"`.
        paint_accent_gradient(ui, pill, CornerRadius::same((badge_h / 2.0) as u8));
        let g = widgets::galley(ui, b, bf.clone(), p.accent_foreground, f32::INFINITY);
        painter.galley(pos2(pill.min.x + 6.0, pill.min.y + 1.0), g, p.accent_foreground);
    }
}

/// Fills `rect` with `ShareX.Brush.Accent`: a `LinearGradientBrush` from
/// `Accent.Start` at the top left to `Accent.End` at the bottom right.
pub fn paint_accent_gradient(ui: &Ui, rect: Rect, radius: CornerRadius) {
    let ctx = ui.ctx();
    let p = palette(ctx);
    let key = Id::new(("accent-gradient", p.accent_start, p.accent_end));
    let tex = ctx.data_mut(|d| d.get_temp::<egui::TextureHandle>(key)).unwrap_or_else(|| {
        const N: usize = 64;
        let (a, b) = (egui::Rgba::from(p.accent_start), egui::Rgba::from(p.accent_end));
        let pixels = (0..N * N)
            .map(|i| {
                let (x, y) = ((i % N) as f32 / (N - 1) as f32, (i / N) as f32 / (N - 1) as f32);
                Color32::from(a * (1.0 - (x + y) / 2.0) + b * ((x + y) / 2.0))
            })
            .collect();
        let image = egui::ColorImage { size: [N, N], pixels, source_size: vec2(N as f32, N as f32) };
        let tex = ctx.load_texture("accent-gradient", image, egui::TextureOptions::LINEAR);
        ctx.data_mut(|d| d.insert_temp(key, tex.clone()));
        tex
    });
    let shape = egui::epaint::RectShape::filled(rect, radius, Color32::WHITE)
        .with_texture(tex.id(), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)));
    ui.painter().add(shape);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placement_slides_into_bounds() {
        let bounds = Rect::from_min_max(pos2(0.0, 0.0), pos2(1000.0, 700.0));
        let anchor = Rect::from_min_max(pos2(7.0, 135.0), pos2(206.0, 165.0));
        // Fits: right of the anchor, tops aligned.
        let r = place(anchor, Placement::RightEdgeAlignedTop, vec2(278.0, 355.0), 0.0, bounds);
        assert_eq!(r.min, pos2(206.0, 135.0));
        // Too tall either way: slides up to the bottom edge.
        let r = place(anchor, Placement::RightEdgeAlignedTop, vec2(589.0, 680.0), 0.0, bounds);
        assert_eq!(r.min, pos2(206.0, 20.0));
        // Flips up when that fits: bottoms aligned.
        let anchor = Rect::from_min_max(pos2(7.0, 600.0), pos2(206.0, 630.0));
        let r = place(anchor, Placement::RightEdgeAlignedTop, vec2(200.0, 300.0), 0.0, bounds);
        assert_eq!(r.min, pos2(206.0, 330.0));
    }

    #[test]
    fn categories_sort_by_header() {
        let c = MenuCategory::new("x", vec![MenuEntry::item("b", "", 1), MenuEntry::item("A", "", 2)]);
        assert_eq!(c.entries[0].header, "A");
    }
}

