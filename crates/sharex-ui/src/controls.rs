//! ShareX's form controls: each one Fluent's template (Avalonia 12.1.6,
//! `Avalonia.Themes.Fluent/Controls/*.xaml`) with ShareX's overrides from
//! `ShareX.Avalonia/Theming/ControlStyles.axaml` on top, in every state
//! (rest, pointer over, pressed, focused, disabled).
//!
//! Controls are placed by their caller: each takes the `Rect` Avalonia's
//! layout gives it, so a window can reproduce ShareX's grids and stack
//! panels exactly, and returns its [`Response`] (or what changed). Use
//! [`Button::width`] and friends to measure a control first.

use egui::{Color32, CornerRadius, CursorIcon, FontId, Id, Key, Order, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};

use crate::menu::paint_accent_gradient;
use crate::theme::{Thickness, fluent, palette};
use crate::widgets::{center_y, font, galley, line_height, text_width};
use crate::{fonts, icons};

/// The interaction of a control at `rect`: hover, press, click.
fn interact(ui: &Ui, rect: Rect, id: Id, enabled: bool) -> Response {
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    ui.interact(rect, id, sense)
}

/// Pointer over and pressed, as Avalonia's `:pointerover` / `:pressed`.
fn states(r: &Response, enabled: bool) -> (bool, bool) {
    if !enabled {
        return (false, false);
    }
    (r.hovered() || r.is_pointer_button_down_on(), r.is_pointer_button_down_on())
}

/// `Border`'s fill and 1px stroke drawn inside `rect`.
fn bordered(ui: &Ui, rect: Rect, radius: f32, fill: Color32, stroke: Color32) {
    let r = CornerRadius::same(radius as u8);
    ui.painter().rect_filled(rect, r, fill);
    ui.painter().rect_stroke(rect, r, Stroke::new(1.0, stroke), StrokeKind::Inside);
}

/// Text in `font` centred vertically in `rect`, starting at `x`.
fn text_at(ui: &Ui, rect: Rect, x: f32, text: &str, font: FontId, color: Color32) {
    let y = center_y(rect, line_height(ui, &font));
    let g = galley(ui, text, font, color, (rect.max.x - x).max(0.0));
    ui.painter().galley(pos2(x, y), g, color);
}

/// A string `Content` of a `Button`, `CheckBox` and the like: drawn by its
/// `ContentPresenter` in Avalonia's default family, Inter (see [`fonts`]).
pub fn content_font(size: f32, weight: u16) -> FontId {
    FontId::new(size, fonts::inter(weight))
}

fn inset(rect: Rect, t: Thickness, border: f32) -> Rect {
    Rect::from_min_max(
        pos2(rect.min.x + border + t.left, rect.min.y + border + t.top),
        pos2(rect.max.x - border - t.right, rect.max.y - border - t.bottom),
    )
}

/// `Button` (ControlStyles.axaml): control background, 1px border, 4px
/// corners, Normal text centred. `.primary` is the accent fill with
/// Fluent's `SystemAccentColorLight1` / `Dark1` hover and press fills.
#[derive(Clone, Copy, Debug)]
pub struct Button {
    pub padding: Thickness,
    pub corner_radius: f32,
    pub font_size: f32,
    pub weight: u16,
    pub primary: bool,
}

impl Default for Button {
    /// ShareX's `Button` style: `Padding="10,6"`, `CornerRadius="4"`.
    fn default() -> Self {
        Self { padding: Thickness::hv(10.0, 6.0), corner_radius: 4.0, font_size: 13.0, weight: 400, primary: false }
    }
}

impl Button {
    /// `Button.compact` in the settings windows: `ButtonPadding`.
    pub fn compact(padding: Thickness) -> Self {
        Self { padding, ..Self::default() }
    }

    /// `Button.primary` as the Start screen and icon pack prompt style it.
    pub fn primary(padding: Thickness, corner_radius: f32) -> Self {
        Self { padding, corner_radius, weight: 600, primary: true, ..Self::default() }
    }

    fn font(&self) -> FontId {
        content_font(self.font_size, self.weight)
    }

    /// The width the button measures to for `text` (border included).
    pub fn width(&self, ui: &Ui, text: &str) -> f32 {
        text_width(ui, text, self.font()) + self.padding.left + self.padding.right + 2.0
    }

    pub fn show(&self, ui: &Ui, rect: Rect, id: Id, text: &str, enabled: bool) -> Response {
        let p = palette(ui.ctx());
        let r = interact(ui, rect, id, enabled);
        let (over, pressed) = states(&r, enabled);
        let radius = CornerRadius::same(self.corner_radius as u8);
        let painter = ui.painter();
        let opacity = if enabled { 1.0 } else { 0.35 };
        let fg = if self.primary { p.accent_foreground } else { p.text };
        if !enabled {
            // `:disabled`: the control background, the presenter at 35%.
            painter.rect_filled(rect, radius, p.control_background.gamma_multiply(opacity));
            let border = if self.primary { p.accent_border } else { p.border };
            painter.rect_stroke(rect, radius, Stroke::new(1.0, border.gamma_multiply(opacity)), StrokeKind::Inside);
        } else if self.primary {
            if pressed {
                painter.rect_filled(rect, radius, p.accent_fill_dark[0]);
            } else if over {
                painter.rect_filled(rect, radius, p.accent_fill_light[0]);
            } else {
                paint_accent_gradient(ui, rect, radius);
            }
            painter.rect_stroke(rect, radius, Stroke::new(1.0, p.accent_border), StrokeKind::Inside);
        } else {
            let fill = if over && !pressed { p.control_background_hover } else { p.control_background };
            bordered(ui, rect, self.corner_radius, fill, p.border);
        }
        if !text.is_empty() {
            let font = self.font();
            let w = text_width(ui, text, font.clone());
            let inner = inset(rect, self.padding, 1.0);
            let x = inner.min.x + ((inner.width() - w) / 2.0).round_ties_even().max(0.0);
            let color = if enabled { fg } else { fg.gamma_multiply(opacity) };
            text_at(ui, inner, x, text, font, color);
        }
        r
    }
}

/// `CheckBox` (Fluent template, ShareX's colours): a 20x20 box with 3px
/// corners centred in a `min_height` row, the content 8px after it.
/// `text_font` is the content's font: Inter for a string `Content`,
/// Segoe UI (`widgets::font`) for a `TextBlock` one.
pub struct CheckBox<'a> {
    pub text: &'a str,
    pub text_font: FontId,
    pub text_color: Option<Color32>,
    pub min_height: f32,
    pub enabled: bool,
}

impl<'a> CheckBox<'a> {
    /// A `CheckBox` with string content (SemiBold, ShareX's style).
    pub fn new(text: &'a str) -> Self {
        Self { text, text_font: content_font(13.0, 600), text_color: None, min_height: 32.0, enabled: true }
    }

    pub fn width(&self, ui: &Ui) -> f32 {
        20.0 + 8.0 + text_width(ui, self.text, self.text_font.clone())
    }

    /// Draws the check box in `rect` (`min_height` tall) and toggles
    /// `checked` on click.
    pub fn show(&self, ui: &Ui, rect: Rect, id: Id, checked: &mut bool) -> Response {
        let p = palette(ui.ctx());
        let f = fluent(ui.ctx());
        let mut r = interact(ui, rect, id, self.enabled);
        let (over, pressed) = states(&r, self.enabled);
        let box_top = rect.min.y + ((self.min_height - 20.0) / 2.0).floor();
        let b = Rect::from_min_size(pos2(rect.min.x, box_top), Vec2::splat(20.0));
        let radius = CornerRadius::same(3);
        let painter = ui.painter();
        if !self.enabled {
            if *checked {
                painter.rect_filled(b, radius, f.base_low);
            } else {
                bordered(ui, b, 3.0, p.control_background_disabled, p.border);
            }
        } else if *checked {
            paint_accent_gradient(ui, b, radius);
            painter.rect_stroke(b, radius, Stroke::new(1.0, p.accent_border), StrokeKind::Inside);
        } else {
            let fill = if pressed {
                // Fluent's `CheckBoxCheckBackgroundFillUncheckedPressed`
                // (`SystemBaseMediumColor`) isn't overridden by ShareX.
                f.base_medium
            } else if over {
                p.control_background_hover
            } else {
                p.control_background
            };
            bordered(ui, b, 3.0, fill, p.border);
        }
        if *checked {
            let glyph = if self.enabled { p.accent_foreground } else { f.base_medium_low };
            paint_check_glyph(ui, b, glyph);
        }
        let color = if self.enabled { self.text_color.unwrap_or(p.text) } else { p.text_secondary };
        let text_rect = Rect::from_min_max(pos2(b.max.x + 8.0, rect.min.y), pos2(rect.max.x.max(b.max.x + 8.0), rect.min.y + self.min_height));
        text_at(ui, text_rect, text_rect.min.x, self.text, self.text_font.clone(), color);
        if r.clicked() {
            *checked = !*checked;
            r.mark_changed();
        }
        r
    }
}

/// Fluent's `CheckMarkPathData` at `Width="9"` in a 16x16 panel that a
/// Viewbox scales to the 20px box: 11.25 wide, centred.
fn paint_check_glyph(ui: &Ui, b: Rect, color: Color32) {
    const SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 17.207 13.0"><path fill="currentColor" d="M5.5 10.586 1.707 6.793A1 1 0 0 0 .293 8.207l4.5 4.5a1 1 0 0 0 1.414 0l11-11A1 1 0 0 0 15.793.293L5.5 10.586Z"/></svg>"#;
    let w = 9.0 * 1.25;
    let h = w * 13.0 / 17.207;
    let r = Rect::from_center_size(b.center(), vec2(w, h));
    icons::paint_svg(ui.painter(), r, "fluent-check", SVG, color);
}

/// `ToggleSwitch` with empty on/off content: a 40x20 track with a 10px
/// knob, 6px above and below (`ToggleSwitchPre/PostContentMargin`), and
/// the 12px column after it.
pub struct ToggleSwitch {
    pub enabled: bool,
}

impl ToggleSwitch {
    pub const SIZE: Vec2 = vec2(52.0, 32.0);

    /// Draws the switch with its 52x32 box at `pos`.
    pub fn show(&self, ui: &Ui, pos: Pos2, id: Id, on: &mut bool) -> Response {
        let p = palette(ui.ctx());
        let f = fluent(ui.ctx());
        let area = Rect::from_min_size(pos, Self::SIZE);
        // `SwitchAreaGrid` (`Margin="0,5"`) takes the pointer.
        let hit = Rect::from_min_max(pos2(area.min.x, area.min.y + 5.0), pos2(area.max.x, area.max.y - 5.0));
        let mut r = interact(ui, hit, id, self.enabled);
        let (over, pressed) = states(&r, self.enabled);
        let track = Rect::from_min_size(pos2(area.min.x, area.min.y + 6.0), vec2(40.0, 20.0));
        let radius = CornerRadius::same(10);
        let painter = ui.painter();
        if *on {
            if !self.enabled {
                painter.rect_filled(track, radius, f.base_low);
            } else if pressed {
                painter.rect_filled(track, radius, p.accent_fill_dark[0]);
            } else if over {
                painter.rect_filled(track, radius, p.accent_fill_light[0]);
            } else {
                paint_accent_gradient(ui, track, radius);
            }
            let knob = if self.enabled { p.accent_foreground } else { f.base_medium_low };
            painter.circle_filled(pos2(track.max.x - 10.0, track.center().y), 5.0, knob);
        } else {
            let (fill, stroke, knob) = if !self.enabled {
                (Color32::TRANSPARENT, f.base_medium_low, f.base_medium_low)
            } else if pressed {
                (f.base_medium_low, f.base_high, f.base_high)
            } else if over {
                (Color32::TRANSPARENT, f.base_medium_high, f.base_high)
            } else {
                (Color32::TRANSPARENT, f.base_medium, f.base_high)
            };
            painter.rect_filled(track, radius, fill);
            painter.rect_stroke(track, radius, Stroke::new(1.0, stroke), StrokeKind::Inside);
            painter.circle_filled(pos2(track.min.x + 10.0, track.center().y), 5.0, knob);
        }
        if r.clicked() {
            *on = !*on;
            r.mark_changed();
        }
        r
    }
}

/// An item of a [`ComboBox`]: what the selection box and each row show.
#[derive(Clone)]
pub struct ComboItem {
    pub text: String,
    /// An icon before the text (the language list's `AppIcons.Language`).
    pub icon: Option<&'static str>,
    /// A flag before the text (`LanguageFlags`), 16x12.
    pub flag: Option<egui::TextureId>,
}

impl ComboItem {
    pub fn text(text: impl Into<String>) -> Self {
        Self { text: text.into(), icon: None, flag: None }
    }

    fn has_image(&self) -> bool {
        self.icon.is_some() || self.flag.is_some()
    }
}

/// `ComboBox` (Fluent template, ShareX's colours): the selected item inside
/// `padding`, a 12px chevron 10px from the right edge (in a 32px column),
/// and a dropdown of the items below it.
pub struct ComboBox {
    pub padding: Thickness,
    pub enabled: bool,
}

impl ComboBox {
    /// Items are `TextBlock`s (ShareX's global SemiBold style).
    pub fn item_font() -> FontId {
        font(13.0, 600)
    }

    /// The width the combo box measures to for its widest item.
    pub fn content_width(&self, ui: &Ui, items: &[ComboItem]) -> f32 {
        let widest = items.iter().map(|i| item_width(ui, i)).fold(0.0, f32::max);
        widest + self.padding.left + self.padding.right + 32.0
    }

    /// Draws the combo box in `rect`; returns a newly selected index.
    pub fn show(&self, ui: &Ui, rect: Rect, id: Id, items: &[ComboItem], selected: Option<usize>) -> Option<usize> {
        let p = palette(ui.ctx());
        let f = fluent(ui.ctx());
        let r = interact(ui, rect, id, self.enabled);
        let open_id = id.with("open");
        let mut open = Self::is_open(ui.ctx(), id) && self.enabled;
        let (over, pressed) = states(&r, self.enabled);
        let fill = if !self.enabled {
            p.control_background_disabled
        } else if over && !pressed {
            p.control_background_hover
        } else {
            p.control_background
        };
        bordered(ui, rect, 4.0, fill, p.border);
        // The content's margin is the padding, from the control's edge: the
        // `Background` border is its sibling, not its parent.
        let inner = Rect::from_min_max(
            pos2(rect.min.x + self.padding.left, rect.min.y + self.padding.top),
            pos2(rect.max.x - 32.0, rect.max.y - self.padding.bottom),
        );
        let fg = if self.enabled { p.text } else { p.text_secondary };
        if let Some(item) = selected.and_then(|i| items.get(i)) {
            paint_item(ui, inner, item, fg);
        }
        // `DropDownGlyph`: 12x12, `Margin="0,0,10,0"`, centred.
        let glyph = Rect::from_min_size(pos2(rect.max.x - 10.0 - 12.0, rect.center().y - 6.0), Vec2::splat(12.0));
        let glyph_color = if self.enabled { f.base_medium_high } else { f.base_medium_low };
        paint_chevron_down(ui, glyph, glyph_color);
        let just_opened = r.clicked() && !open;
        if r.clicked() {
            open = !open;
        }
        let mut chosen = None;
        if open {
            let (picked, keep) = dropdown(ui, id, rect, items, selected, just_opened);
            chosen = picked;
            open = keep && picked.is_none();
        }
        ui.ctx().data_mut(|d| d.insert_temp(open_id, open));
        chosen.filter(|&c| Some(c) != selected)
    }

    /// Whether the dropdown of the combo box `id` is open.
    pub fn is_open(ctx: &egui::Context, id: Id) -> bool {
        ctx.data(|d| d.get_temp::<bool>(id.with("open"))).unwrap_or(false)
    }

    pub fn set_open(ctx: &egui::Context, id: Id, open: bool) {
        ctx.data_mut(|d| d.insert_temp(id.with("open"), open));
    }
}

fn item_width(ui: &Ui, i: &ComboItem) -> f32 {
    text_width(ui, &i.text, ComboBox::item_font()) + if i.has_image() { 24.0 } else { 0.0 }
}

/// One item: an optional 16px image cell and 8px spacing, then the text.
fn paint_item(ui: &Ui, rect: Rect, item: &ComboItem, color: Color32) {
    let mut x = rect.min.x;
    if item.has_image() {
        let cell = Rect::from_min_size(pos2(x, center_y(rect, 16.0)), Vec2::splat(16.0));
        if let Some(icon) = item.icon {
            icons::paint(ui.painter(), cell, icon, color);
        }
        if let Some(tex) = item.flag {
            let flag = Rect::from_min_size(pos2(cell.min.x, cell.min.y + 2.0), vec2(16.0, 12.0));
            let radius = CornerRadius::same(2);
            ui.painter().add(
                egui::epaint::RectShape::filled(flag, radius, Color32::WHITE)
                    .with_texture(tex, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0))),
            );
            ui.painter().rect_stroke(flag, radius, Stroke::new(1.0, Color32::from_black_alpha(0x26)), StrokeKind::Inside);
        }
        x += 24.0;
    }
    text_at(ui, rect, x, &item.text, ComboBox::item_font(), color);
}

/// Fluent's ComboBox `DropDownGlyph` path in a 12x12 `PathIcon`.
pub fn paint_chevron_down(ui: &Ui, rect: Rect, color: Color32) {
    const SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="19 486 2010 1095"><path fill="currentColor" d="M1939 486L2029 576L1024 1581L19 576L109 486L1024 1401L1939 486Z"/></svg>"#;
    let h = rect.width() * 1095.0 / 2010.0;
    let r = Rect::from_center_size(rect.center(), vec2(rect.width(), h));
    icons::paint_svg(ui.painter(), r, "fluent-combo-chevron", SVG, color);
}

/// The dropdown: Fluent's `PopupBorder` (`SystemControlTransientBackgroundBrush`,
/// the transient border, `OverlayCornerRadius` 5) with ShareX's 6px
/// `ComboBoxDropdownContentMargin`, items `Padding="10,8"` with 4px corners,
/// the selected one on the accent. Returns the clicked item and whether
/// the dropdown stays open.
fn dropdown(ui: &Ui, id: Id, anchor: Rect, items: &[ComboItem], selected: Option<usize>, just_opened: bool) -> (Option<usize>, bool) {
    let p = palette(ui.ctx());
    let f = fluent(ui.ctx());
    let popup_bg = if ui.ctx().theme() == egui::Theme::Dark {
        Color32::from_rgb(0x2B, 0x2B, 0x2B)
    } else {
        Color32::from_rgb(0xF2, 0xF2, 0xF2)
    };
    let item_font = ComboBox::item_font();
    let row_h = line_height(ui, &item_font) + 16.0;
    let widest = items.iter().map(|i| item_width(ui, i) + 20.0).fold(0.0, f32::max);
    let width = anchor.width().max(widest + 14.0);
    let full = row_h * items.len() as f32 + 14.0;
    let screen = ui.ctx().content_rect();
    // Below the combo box, else above it, else wherever it fits best.
    let below = screen.max.y - anchor.max.y;
    let above = anchor.min.y - screen.min.y;
    let (top, height) = if full <= below || below >= above {
        (anchor.max.y, full.min(below).max(row_h + 14.0))
    } else {
        let h = full.min(above);
        (anchor.min.y - h, h)
    };
    let popup = Rect::from_min_size(pos2(anchor.min.x, top), vec2(width, height));
    let mut picked = None;
    let scroll_id = id.with("scroll");
    let area = egui::Area::new(id.with("dropdown")).order(Order::Foreground).fixed_pos(popup.min).constrain(false).fade_in(false).show(ui.ctx(), |ui| {
        let (rect, _) = ui.allocate_exact_size(popup.size(), Sense::click());
        bordered(ui, rect, 5.0, popup_bg, f.transient_border);
        let view = rect.shrink(1.0);
        let max_scroll = (full - height).max(0.0);
        let mut scroll: f32 = ui.ctx().data(|d| d.get_temp(scroll_id)).unwrap_or_else(|| {
            // Opening shows the selected item.
            selected.map_or(0.0, |s| (row_h * s as f32 - (height - row_h) / 2.0).clamp(0.0, max_scroll))
        });
        if just_opened {
            scroll = selected.map_or(0.0, |s| (row_h * s as f32 - (height - row_h) / 2.0).clamp(0.0, max_scroll));
        }
        if ui.rect_contains_pointer(view) {
            scroll = (scroll - ui.input(|i| i.smooth_scroll_delta.y)).clamp(0.0, max_scroll);
        }
        ui.ctx().data_mut(|d| d.insert_temp(scroll_id, scroll));
        let painter = ui.painter().with_clip_rect(view);
        let child = ui.new_child(egui::UiBuilder::new().max_rect(view));
        let mut child = child;
        child.set_clip_rect(view);
        for (i, item) in items.iter().enumerate() {
            let row = Rect::from_min_size(pos2(rect.min.x + 7.0, rect.min.y + 7.0 + row_h * i as f32 - scroll), vec2(rect.width() - 14.0, row_h));
            if !row.intersects(view) {
                continue;
            }
            let resp = child.interact(row.intersect(view), id.with(("item", i)), Sense::click());
            let sel = Some(i) == selected;
            // `ComboBoxItem:selected`: the accent fill (its border brush is
            // set, but a `ComboBoxItem` has no border thickness).
            let fg = if sel {
                paint_accent_gradient(&child, row, CornerRadius::same(4));
                p.accent_foreground
            } else {
                if resp.hovered() {
                    painter.rect_filled(row, CornerRadius::same(4), p.control_background_hover);
                }
                p.text
            };
            let inner = Rect::from_min_max(pos2(row.min.x + 10.0, row.min.y + 8.0), pos2(row.max.x - 10.0, row.max.y - 8.0));
            paint_item(&child, inner, item, fg);
            if resp.clicked() {
                picked = Some(i);
            }
        }
        if max_scroll > 0.0 {
            // Fluent's collapsed scroll bar: a thin indicator on the right.
            let track = view.height();
            let thumb_h = (track * height / full).max(20.0);
            let y = view.min.y + (track - thumb_h) * scroll / max_scroll;
            let bar = Rect::from_min_size(pos2(view.max.x - 4.0, y), vec2(2.0, thumb_h));
            painter.rect_filled(bar, CornerRadius::same(1), f.base_medium_low);
        }
    });
    let ctx = ui.ctx();
    let mut keep = !ctx.input(|i| i.key_pressed(Key::Escape));
    if !just_opened
        && ctx.input(|i| i.pointer.any_pressed())
        && let Some(pos) = ctx.input(|i| i.pointer.interact_pos())
        && !area.response.rect.contains(pos)
        && !anchor.contains(pos)
    {
        keep = false;
    }
    if !keep || picked.is_some() {
        ctx.data_mut(|d| d.remove::<f32>(scroll_id));
    }
    (picked, keep)
}

/// `TextBox`: control background (hover background), 1px border, 4px
/// corners, the text inside `padding` in Avalonia's default font.
pub struct TextBox<'a> {
    pub padding: Thickness,
    pub enabled: bool,
    pub placeholder: &'a str,
    pub password: bool,
    pub multiline: bool,
}

impl Default for TextBox<'_> {
    fn default() -> Self {
        Self { padding: Thickness::hv(8.0, 6.0), enabled: true, placeholder: "", password: false, multiline: false }
    }
}

impl TextBox<'_> {
    /// A `TextPresenter` takes the control's font: Inter, Normal.
    pub fn font() -> FontId {
        content_font(13.0, 400)
    }

    /// Draws the text box in `rect`, editing `text`.
    pub fn show(&self, ui: &mut Ui, rect: Rect, id: Id, text: &mut String) -> Response {
        let p = palette(ui.ctx());
        let hover = self.enabled && ui.rect_contains_pointer(rect);
        let focused = ui.ctx().memory(|m| m.has_focus(id));
        let fill = if !self.enabled {
            p.control_background_disabled
        } else if hover && !focused {
            p.control_background_hover
        } else {
            p.control_background
        };
        bordered(ui, rect, 4.0, fill, p.border);
        let inner = inset(rect, self.padding, 1.0);
        let text_font = Self::font();
        let lh = line_height(ui, &text_font);
        let fg = if self.enabled { p.text } else { p.text_secondary };
        let edit_rect = if self.multiline {
            inner
        } else {
            Rect::from_min_size(pos2(inner.min.x, center_y(inner, lh)), vec2(inner.width(), lh))
        };
        let mut edit = if self.multiline { egui::TextEdit::multiline(text) } else { egui::TextEdit::singleline(text) };
        edit = edit
            .id(id)
            .font(text_font)
            .text_color(fg)
            .frame(egui::Frame::NONE)
            .margin(egui::Margin::ZERO)
            .desired_width(edit_rect.width())
            .password(self.password)
            .interactive(self.enabled);
        if !self.placeholder.is_empty() {
            // `PART_Placeholder`: a `TextBlock` (ShareX's SemiBold Segoe UI)
            // in `TextControlPlaceholderForeground` at 50% opacity.
            let f = fluent(ui.ctx());
            edit = edit.hint_text(egui::RichText::new(self.placeholder).font(font(13.0, 600)).color(f.base_medium.gamma_multiply(0.5)));
        }
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(edit_rect).layout(egui::Layout::top_down(egui::Align::Min)));
        child.set_clip_rect(inner.intersect(ui.clip_rect()));
        let r = child.add_sized(edit_rect.size(), edit);
        // The whole box focuses the text, as Avalonia's does.
        let outer = ui.interact(rect, id.with("box"), Sense::click());
        if self.enabled && outer.clicked() {
            ui.ctx().memory_mut(|m| m.request_focus(id));
        }
        if self.enabled { outer.on_hover_cursor(CursorIcon::Text); }
        r
    }
}

/// `NumericUpDown`: a `ButtonSpinner` with the value centred in a text box
/// and two 34px repeat buttons on the right (Fluent's 16x8 chevrons), each
/// with a 1px border on its left.
pub struct NumericUpDown {
    pub minimum: i64,
    pub maximum: i64,
    pub enabled: bool,
}

impl NumericUpDown {
    pub fn show(&self, ui: &mut Ui, rect: Rect, id: Id, value: &mut i64) -> bool {
        let p = palette(ui.ctx());
        let f = fluent(ui.ctx());
        let mut changed = false;
        let hover = self.enabled && ui.rect_contains_pointer(rect);
        let fill = if !self.enabled {
            p.control_background_disabled
        } else if hover {
            p.control_background_hover
        } else {
            p.control_background
        };
        bordered(ui, rect, 4.0, fill, p.border);
        let up = Rect::from_min_max(pos2(rect.max.x - 69.0, rect.min.y + 1.0), pos2(rect.max.x - 35.0, rect.max.y - 1.0));
        let down = Rect::from_min_max(pos2(rect.max.x - 35.0, rect.min.y + 1.0), pos2(rect.max.x - 1.0, rect.max.y - 1.0));
        let now = ui.input(|i| i.time);
        for (i, (b, step)) in [(up, 1i64), (down, -1)].into_iter().enumerate() {
            let r = interact(ui, b, id.with(("spin", i)), self.enabled);
            let (over, pressed) = states(&r, self.enabled);
            let cell = Rect::from_min_max(pos2(b.min.x + 1.0, b.min.y), b.max);
            // `RepeatButtonBackgroundPointerOver` / `Pressed`.
            let bg = if pressed { Some(f.base_medium_low) } else if over { Some(f.base_high.gamma_multiply(0.1)) } else { None };
            if let Some(bg) = bg {
                let radius = if i == 1 { CornerRadius { ne: 3, se: 3, ..Default::default() } } else { CornerRadius::ZERO };
                ui.painter().rect_filled(cell, radius, bg);
            }
            ui.painter().rect_filled(Rect::from_min_size(b.min, vec2(1.0, b.height())), 0.0, p.border);
            let glyph = Rect::from_center_size(cell.center(), vec2(16.0, 8.0));
            let color = if self.enabled { p.text } else { p.text_secondary };
            paint_spinner_chevron(ui, glyph, step > 0, color);
            // `RepeatButton`: once on press, then every 33ms after 300ms.
            let key = id.with(("repeat", i));
            let mut fire = false;
            if r.is_pointer_button_down_on() {
                let last: Option<f64> = ui.ctx().data(|d| d.get_temp(key));
                match last {
                    None => {
                        fire = true;
                        ui.ctx().data_mut(|d| d.insert_temp(key, now + 0.3));
                    }
                    Some(next) if now >= next => {
                        fire = true;
                        ui.ctx().data_mut(|d| d.insert_temp(key, now + 0.033));
                    }
                    _ => {}
                }
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(16));
            } else {
                ui.ctx().data_mut(|d| d.remove::<f64>(key));
            }
            if fire {
                let v = (*value + step).clamp(self.minimum, self.maximum);
                if v != *value {
                    *value = v;
                    changed = true;
                }
            }
        }
        // The text box, centred (`TextAlignment="Center"`, `Padding="8,0"`).
        let text_rect = Rect::from_min_max(pos2(rect.min.x + 1.0, rect.min.y + 1.0), pos2(up.min.x, rect.max.y - 1.0));
        let buf_id = id.with("text");
        let focused = ui.ctx().memory(|m| m.has_focus(buf_id));
        let mut text = if focused {
            ui.ctx().data(|d| d.get_temp::<String>(buf_id)).unwrap_or_else(|| value.to_string())
        } else {
            value.to_string()
        };
        let font = TextBox::font();
        let lh = line_height(ui, &font);
        let edit_rect = Rect::from_min_size(pos2(text_rect.min.x + 8.0, center_y(text_rect, lh)), vec2(text_rect.width() - 16.0, lh));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(edit_rect));
        let r = child.add_sized(
            edit_rect.size(),
            egui::TextEdit::singleline(&mut text)
                .id(buf_id)
                .font(font)
                .text_color(if self.enabled { p.text } else { p.text_secondary })
                .frame(egui::Frame::NONE)
                .margin(egui::Margin::ZERO)
                .horizontal_align(egui::Align::Center)
                .interactive(self.enabled),
        );
        if r.has_focus() {
            ui.ctx().data_mut(|d| d.insert_temp(buf_id, text.clone()));
        }
        if r.lost_focus() {
            if let Ok(v) = text.trim().parse::<i64>() {
                let v = v.clamp(self.minimum, self.maximum);
                if v != *value {
                    *value = v;
                    changed = true;
                }
            }
            ui.ctx().data_mut(|d| d.remove::<String>(buf_id));
        }
        changed
    }
}

/// Fluent's `ButtonSpinnerIncreaseButtonIcon` / `Decrease…` (20x10
/// geometry) in a 16x8 `PathIcon`.
fn paint_spinner_chevron(ui: &Ui, rect: Rect, up: bool, color: Color32) {
    const UP: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 10"><path fill="currentColor" d="M0,9 L10,0 20,9 19,10 10,2 1,10 z"/></svg>"#;
    const DOWN: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 10"><path fill="currentColor" d="M0,1 L10,10 20,1 19,0 10,8 1,0 z"/></svg>"#;
    let (key, svg) = if up { ("fluent-spin-up", UP) } else { ("fluent-spin-down", DOWN) };
    icons::paint_svg(ui.painter(), rect, key, svg, color);
}

/// `Slider` (horizontal): a 2px track, the part before the thumb in the
/// accent border colour (ShareX's `Foreground`), the rest
/// `SliderTrackFill`, and a 20px thumb in the accent border colour with a
/// 4px ring of control background (3px under the pointer).
pub struct Slider {
    pub minimum: f32,
    pub maximum: f32,
    /// `TickFrequency` with `IsSnapToTickEnabled`.
    pub step: f32,
    pub enabled: bool,
}

impl Slider {
    /// Draws the slider in `rect` (the track is centred vertically).
    pub fn show(&self, ui: &Ui, rect: Rect, id: Id, value: &mut f32) -> bool {
        let p = palette(ui.ctx());
        let f = fluent(ui.ctx());
        let sense = if self.enabled { Sense::click_and_drag() } else { Sense::hover() };
        let r = ui.interact(rect, id, sense);
        let over = self.enabled && (r.hovered() || r.dragged());
        let range = (self.maximum - self.minimum).max(f32::EPSILON);
        let mut changed = false;
        if (r.dragged() || r.clicked() || r.drag_started())
            && let Some(pos) = r.interact_pointer_pos()
        {
            let t = ((pos.x - rect.min.x - 10.0) / (rect.width() - 20.0)).clamp(0.0, 1.0);
            let mut v = self.minimum + t * range;
            if self.step > 0.0 {
                v = ((v - self.minimum) / self.step).round() * self.step + self.minimum;
            }
            if v != *value {
                *value = v;
                changed = true;
            }
        }
        let t = ((*value - self.minimum) / range).clamp(0.0, 1.0);
        let cx = (rect.min.x + 10.0 + (rect.width() - 20.0) * t).round();
        // The track row's centre: 14px down a 32px slider (the thumb's 20px
        // row, centred in what the 2px margins leave).
        let cy = (rect.min.y + (rect.height() - 4.0) / 2.0).floor();
        let radius = CornerRadius::same(1);
        let rest_color = if !self.enabled { f.base_medium_low } else if over { f.base_medium } else { f.base_medium_low };
        let done_color = if self.enabled { p.accent_border } else { f.base_medium_low };
        ui.painter().rect_filled(Rect::from_min_max(pos2(cx, cy - 1.0), pos2(rect.max.x, cy + 1.0)), radius, rest_color);
        ui.painter().rect_filled(Rect::from_min_max(pos2(rect.min.x, cy - 1.0), pos2(cx, cy + 1.0)), radius, done_color);
        let ring = if over { 3.0 } else { 4.0 };
        let c = pos2(cx, cy);
        ui.painter().circle_filled(c, 10.0, p.control_background);
        let fill = if !self.enabled { f.base_medium_low } else { p.accent_border };
        ui.painter().circle_filled(c, 10.0 - ring, fill);
        changed
    }
}

/// `ColorPickerDropdown`'s button: a standard `Button` with a 24x24
/// swatch over the small checkerboard (4px corners, 6px after it) and a
/// 10px chevron in the secondary text colour on the right.
pub struct ColorButton {
    pub enabled: bool,
}

impl ColorButton {
    pub fn show(&self, ui: &Ui, rect: Rect, id: Id, color: Color32) -> Response {
        let p = palette(ui.ctx());
        let b = Button::default();
        let r = b.show(ui, rect, id, "", self.enabled);
        let inner = inset(rect, b.padding, 1.0);
        let opacity = if self.enabled { 1.0 } else { 0.35 };
        let swatch = Rect::from_min_size(pos2(inner.min.x, center_y(inner, 24.0)), Vec2::splat(24.0));
        paint_checkered_small(ui, swatch, opacity);
        ui.painter().rect_filled(swatch, CornerRadius::same(4), color.gamma_multiply(opacity));
        let chevron = Rect::from_min_size(pos2(inner.max.x - 10.0, center_y(inner, 10.0)), Vec2::splat(10.0));
        icons::paint(ui.painter(), chevron, crate::keys::app_icons::ChevronDown, p.text_secondary.gamma_multiply(opacity));
        r
    }
}

/// `ShareX.Brush.Checkered.Small` under a 4px-corner swatch.
fn paint_checkered_small(ui: &Ui, rect: Rect, opacity: f32) {
    let p = palette(ui.ctx());
    ui.painter().rect_filled(rect, CornerRadius::same(4), p.checkered[0].gamma_multiply(opacity));
    let painter = ui.painter().with_clip_rect(rect.shrink(1.0));
    let cell = 4.0;
    let (nx, ny) = ((rect.width() / cell).ceil() as i32, (rect.height() / cell).ceil() as i32);
    for y in 0..ny {
        for x in 0..nx {
            if (x + y) % 2 == 1 {
                let r = Rect::from_min_size(pos2(rect.min.x + x as f32 * cell, rect.min.y + y as f32 * cell), Vec2::splat(cell));
                painter.rect_filled(r, 0.0, p.checkered[1].gamma_multiply(opacity));
            }
        }
    }
}

/// The colour popup under `anchor` (ColorPickerDropdown's `Popup`,
/// `VerticalOffset="4"`: panel background, 1px border, 8px corners).
/// Avalonia's `ColorView` isn't ported: egui's HSV picker stands in for it.
/// Returns whether the colour changed and whether the popup stays open.
pub fn color_popup(ui: &Ui, id: Id, anchor: Rect, color: &mut Color32, just_opened: bool) -> (bool, bool) {
    let p = palette(ui.ctx());
    let mut changed = false;
    let pos = pos2(anchor.min.x, anchor.max.y + 4.0);
    let area = egui::Area::new(id.with("color-popup")).order(Order::Foreground).fixed_pos(pos).fade_in(false).show(ui.ctx(), |ui| {
        egui::Frame::new()
            .fill(p.background_panel)
            .stroke(Stroke::new(1.0, p.border))
            .corner_radius(CornerRadius::same(8))
            .inner_margin(egui::Margin::same(8))
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                ui.spacing_mut().slider_width = 240.0;
                changed |= egui::widgets::color_picker::color_picker_color32(ui, color, egui::widgets::color_picker::Alpha::Opaque);
            });
    });
    let ctx = ui.ctx();
    let mut keep = !ctx.input(|i| i.key_pressed(Key::Escape));
    if !just_opened
        && ctx.input(|i| i.pointer.any_pressed())
        && let Some(pos) = ctx.input(|i| i.pointer.interact_pos())
        && !area.response.rect.contains(pos)
        && !anchor.contains(pos)
    {
        keep = false;
    }
    (changed, keep)
}

/// `Border.section-card`, `.settings-list`, `.tile`: panel background,
/// 1px border, `radius` corners.
pub fn card(ui: &Ui, rect: Rect, radius: f32) {
    let p = palette(ui.ctx());
    bordered(ui, rect, radius, p.background_panel, p.border);
}

/// Text left-aligned in `rect` and centred vertically, trimmed to it.
pub fn label(ui: &Ui, rect: Rect, text: &str, font: FontId, color: Color32) {
    text_at(ui, rect, rect.min.x, text, font, color);
}

/// A `TextBlock` with `TextWrapping="Wrap"` at `pos`, `width` wide: lays
/// it out (painting it when `paint`) and returns its height.
pub fn wrapped(ui: &Ui, pos: Pos2, width: f32, text: &str, font: FontId, color: Color32, paint: bool) -> f32 {
    let lines = wrap_lines(ui, text, &font, width);
    let lh = line_height(ui, &font);
    if paint {
        for (i, line) in lines.iter().enumerate() {
            let g = galley(ui, line, font.clone(), color, f32::INFINITY);
            ui.painter().galley(pos2(pos.x, pos.y + lh * i as f32), g, color);
        }
    }
    lh * lines.len().max(1) as f32
}

/// Breaks `text` at spaces into lines no wider than `width` (a word wider
/// than that gets a line of its own), as Avalonia's `TextWrapping.Wrap`.
pub fn wrap_lines(ui: &Ui, text: &str, font: &FontId, width: f32) -> Vec<String> {
    let mut lines = Vec::new();
    for para in text.split('\n') {
        let mut line = String::new();
        for word in para.split(' ') {
            let candidate = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
            if text_width(ui, &candidate, font.clone()) <= width {
                line = candidate;
                continue;
            }
            if !line.is_empty() {
                lines.push(std::mem::take(&mut line));
            }
            // A word wider than the line breaks between characters.
            for c in word.chars() {
                let next = format!("{line}{c}");
                if !line.is_empty() && text_width(ui, &next, font.clone()) > width {
                    lines.push(std::mem::take(&mut line));
                    line.push(c);
                } else {
                    line = next;
                }
            }
        }
        lines.push(line);
    }
    lines
}

/// The 1px `Border.divider`.
pub fn divider(ui: &Ui, rect: Rect) {
    ui.painter().rect_filled(rect, 0.0, palette(ui.ctx()).border);
}

/// Draws `text` centred on `x` with its line box at `y`.
pub fn text_centered_at(ui: &Ui, x: f32, y: f32, text: &str, font: FontId, color: Color32) {
    let w = text_width(ui, text, font.clone());
    let g = galley(ui, text, font, color, f32::INFINITY);
    ui.painter().galley(pos2(x - (w / 2.0).round_ties_even(), y), g, color);
}
