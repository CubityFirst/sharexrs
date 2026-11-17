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

use crate::border::{Brush, Part};
use crate::theme::{Thickness, fluent, palette};
use crate::widgets::{center_y, font, galley, line_height, text_width};
use crate::{border, focus, fonts, icons};

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

/// A `Border` in `rect`: `fill` and a 1px `stroke`, as Avalonia's centred
/// border geometry (see [`border`]).
fn bordered(ui: &Ui, rect: Rect, radius: f32, fill: Color32, stroke: Color32) {
    border::border(ui.painter(), rect, radius, 1.0, Some(Brush::Solid(fill)), Some(stroke));
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

/// A disabled `Button`'s presenter `Opacity`.
const DISABLED_OPACITY: f32 = 0.35;

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
        let f = fluent(ui.ctx());
        let r = interact(ui, rect, id, enabled);
        let (over, pressed) = states(&r, enabled);
        let painter = ui.painter();
        let color = if !enabled {
            // `:disabled`: the control background, a transparent border
            // (`.primary` keeps its border and foreground) and the base 40%
            // foreground, then the whole presenter at 35% as one layer.
            let (stroke, fg) = if self.primary { (Some(p.accent_border), p.accent_foreground) } else { (None, f.base_medium_low) };
            let shell = Part::Border { rect, radius: self.corner_radius, thickness: 1.0, background: Some(Brush::Solid(p.control_background)), border: stroke };
            border::paint(painter, &[shell], DISABLED_OPACITY);
            border::group_content(fg, p.control_background, DISABLED_OPACITY, p.background_panel)
        } else if self.primary {
            let fill = if pressed {
                Brush::Solid(p.accent_fill_dark[0])
            } else if over {
                Brush::Solid(p.accent_fill_light[0])
            } else {
                Brush::accent(ui.ctx())
            };
            border::border(painter, rect, self.corner_radius, 1.0, Some(fill), Some(p.accent_border));
            p.accent_foreground
        } else {
            let fill = if over && !pressed { p.control_background_hover } else { p.control_background };
            bordered(ui, rect, self.corner_radius, fill, p.border);
            // `ButtonForegroundPointerOver` / `Pressed`: `SystemBaseHighColor`.
            if over || pressed { f.base_high } else { p.text }
        };
        if !text.is_empty() {
            let font = self.font();
            let w = text_width(ui, text, font.clone());
            let inner = inset(rect, self.padding, 1.0);
            let x = inner.min.x + ((inner.width() - w) / 2.0).round_ties_even().max(0.0);
            text_at(ui, inner, x, text, font, color);
        }
        if focus::visible(ui.ctx(), id) {
            focus::paint_adorner(painter, rect);
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
        if !self.enabled {
            // `:disabled /template/ Border#NormalRectangle`, checked or not.
            bordered(ui, b, 3.0, p.control_background_disabled, p.border);
        } else if *checked {
            border::border(ui.painter(), b, 3.0, 1.0, Some(Brush::accent(ui.ctx())), Some(p.accent_border));
        } else {
            // ShareX's `:pointerover` `NormalRectangle` wins over Fluent's
            // pressed fill as well.
            let fill = if over || pressed { p.control_background_hover } else { p.control_background };
            bordered(ui, b, 3.0, fill, p.border);
        }
        if *checked {
            // Disabled: white on the disabled fill, as ShareX draws it in
            // both themes.
            let glyph = if self.enabled { p.accent_foreground } else { Color32::WHITE };
            paint_check_glyph(ui, b, glyph);
        }
        // Disabled: Fluent's `ContentPresenter` foreground
        // (`CheckBoxForegroundUncheckedDisabled`) wins over ShareX's. A
        // string `Content` takes `CheckBoxForeground*` (base high when
        // checked or under the pointer); a `TextBlock` with its own colour
        // keeps it.
        let color = match self.text_color {
            _ if !self.enabled => f.base_medium_low,
            Some(c) => c,
            None if *checked || over => f.base_high,
            None => p.text,
        };
        let text_rect = Rect::from_min_max(pos2(b.max.x + 8.0, rect.min.y), pos2(rect.max.x.max(b.max.x + 8.0), rect.min.y + self.min_height));
        text_at(ui, text_rect, text_rect.min.x, self.text, self.text_font.clone(), color);
        if focus::visible(ui.ctx(), id) {
            focus::paint_adorner(ui.painter(), rect);
        }
        if r.clicked() {
            *checked = !*checked;
            r.mark_changed();
        }
        r
    }
}

/// `RadioButton` (Fluent template, ShareX's pointer-over styles): a 20px
/// `OuterEllipse` (1px base medium ring; accent ring under the pointer),
/// checked a solid accent `CheckOuterEllipse` with an 8px `CheckGlyph` in
/// the accent's foreground; the content `padding_left` after the circle.
pub struct RadioButton<'a> {
    pub text: &'a str,
    pub text_font: FontId,
    pub text_color: Option<Color32>,
    pub padding_left: f32,
    pub enabled: bool,
}

impl RadioButton<'_> {
    /// Draws the radio button with its circle centred vertically in `rect`;
    /// returns its response (clicked when it should become checked).
    pub fn show(&self, ui: &Ui, rect: Rect, id: Id, checked: bool) -> Response {
        let p = palette(ui.ctx());
        let f = fluent(ui.ctx());
        let r = interact(ui, rect, id, self.enabled);
        let (over, _) = states(&r, self.enabled);
        let circle = Rect::from_min_size(pos2(rect.min.x, center_y(rect, 20.0)), Vec2::splat(20.0));
        let painter = ui.painter();
        if checked {
            border::ellipse(painter, circle, p.accent_start);
            border::ellipse(painter, Rect::from_center_size(circle.center(), Vec2::splat(8.0)), p.accent_foreground);
        } else {
            let stroke = if over { p.accent_border } else { f.base_medium };
            border::border(painter, circle, 10.0, 1.0, None, Some(stroke));
        }
        let color = self.text_color.unwrap_or(p.text_secondary);
        let x = circle.max.x + self.padding_left;
        text_at(ui, rect, x, self.text, self.text_font.clone(), color);
        if focus::visible(ui.ctx(), id) {
            focus::paint_adorner(painter, rect);
        }
        r
    }
}

/// Fluent's `CheckMarkPathData` in the `CheckGlyph` `Path` (`Width="9"`,
/// `Stretch="Uniform"`) at 3.5,4.6556594 in a 16x16 panel that a `Viewbox`
/// scales by 1.25 into the 20px box. The path's stretch (its geometry
/// bounds, 0.0122951,0.0122808 to 17.4877, 12.9998, as Avalonia's arcs
/// approximate them) maps it by 0.5150089 from -0.0063321,-0.0063247.
fn paint_check_glyph(ui: &Ui, b: Rect, color: Color32) {
    const SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 17.5 13"><path fill="currentColor" d="M5.5 10.586 1.707 6.793A1 1 0 0 0 .293 8.207l4.5 4.5a1 1 0 0 0 1.414 0l11-11A1 1 0 0 0 15.793.293L5.5 10.586Z"/></svg>"#;
    const VIEWBOX: f32 = 1.25;
    const STRETCH: f32 = 0.515_008_9;
    let offset = vec2(VIEWBOX * (3.5 - 0.006_332_077), VIEWBOX * (4.655_659 - 0.006_324_709));
    icons::paint_svg_exact(ui.painter(), b, "fluent-check", SVG, VIEWBOX * STRETCH, offset, color);
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
        let painter = ui.painter();
        if *on {
            // `SwitchKnobBounds`: ShareX's accent gradient whatever the
            // pointer does (its `:checked` styles win over Fluent's
            // pointer-over and pressed fills); `ToggleSwitchFillOnDisabled`
            // and `ToggleSwitchKnobFillOnDisabled` are both base 20%.
            let (fill, knob) = if self.enabled { (Brush::accent(ui.ctx()), p.accent_foreground) } else { (Brush::Solid(f.base_low), f.base_low) };
            border::border(painter, track, 10.0, 0.0, Some(fill), None);
            border::ellipse(painter, Rect::from_center_size(pos2(track.max.x - 10.0, track.center().y), Vec2::splat(10.0)), knob);
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
            border::border(painter, track, 10.0, 1.0, Some(Brush::Solid(fill)), Some(stroke));
            border::ellipse(painter, Rect::from_center_size(pos2(track.min.x + 10.0, track.center().y), Vec2::splat(10.0)), knob);
        }
        if focus::visible(ui.ctx(), id) {
            // Around `SwitchAreaGrid`, 52x22.
            focus::paint_adorner(painter, hit);
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
        if focus::visible(ui.ctx(), id) {
            // `:focus-visible`: `HighlightBackground`, the accent at
            // `SystemControlHighlightListAccentLowBrush`'s opacity with a
            // transparent 1px border, over `Background` (no adorner).
            let highlight = p.accent_start.gamma_multiply(f.list_accent_low_opacity);
            border::border(ui.painter(), rect, 4.0, 1.0, Some(Brush::Solid(highlight)), None);
        }
        // The content's margin is the padding, from the control's edge: the
        // `Background` border is its sibling, not its parent.
        let inner = Rect::from_min_max(
            pos2(rect.min.x + self.padding.left, rect.min.y + self.padding.top),
            pos2(rect.max.x - 32.0, rect.max.y - self.padding.bottom),
        );
        // Items are `TextBlock`s, whose ShareX style keeps the text colour
        // when the combo box is disabled or focused.
        let fg = p.text;
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

/// Fluent's ComboBox `DropDownGlyph` path in a 12x12 `PathIcon`: its
/// `Viewbox` scales the 2010x1095 geometry (from 19,486) to the cell's
/// width and centres it, 2.7313433 down in a 12px cell, kept fractional.
pub fn paint_chevron_down(ui: &Ui, rect: Rect, color: Color32) {
    const PATH: [(f32, f32); 6] = [(1939.0, 486.0), (2029.0, 576.0), (1024.0, 1581.0), (19.0, 576.0), (109.0, 486.0), (1024.0, 1401.0)];
    let s = rect.width() / 2010.0;
    let top = rect.min.y + (rect.height() - 1095.0 * s) / 2.0;
    let points: Vec<Pos2> = PATH.iter().map(|&(x, y)| pos2(rect.min.x + (x - 19.0) * s, top + (y - 486.0) * s)).collect();
    border::polygon(ui.painter(), &points, color);
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
        border::border(ui.painter(), rect, 5.0, 1.0, Some(Brush::Solid(popup_bg)), Some(f.transient_border));
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
                border::border(&painter, row, 4.0, 0.0, Some(Brush::accent(ui.ctx())), None);
                p.accent_foreground
            } else {
                if resp.hovered() {
                    border::border(&painter, row, 4.0, 0.0, Some(Brush::Solid(p.control_background_hover)), None);
                }
                p.text
            };
            let inner = Rect::from_min_max(pos2(row.min.x + 10.0, row.min.y + 8.0), pos2(row.max.x - 10.0, row.max.y - 8.0));
            paint_item(&child, inner, item, fg);
            if focus::visible(ui.ctx(), resp.id) {
                focus::paint_adorner(&painter, row);
            }
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
    /// `Background="Transparent" BorderThickness="0"` on the text box
    /// itself (a text box filling a card): nothing behind the text until it
    /// has focus or the pointer.
    pub plain: bool,
    /// `IsReadOnly`: the text can be selected and copied, not changed.
    pub read_only: bool,
    /// `TextWrapping="Wrap"` (multiline boxes; otherwise lines run on).
    pub wrap: bool,
    /// `TextBoxLinkHighlighter`: URLs underlined in this colour.
    pub links: Option<Color32>,
}

impl Default for TextBox<'_> {
    fn default() -> Self {
        Self {
            padding: Thickness::hv(8.0, 6.0),
            enabled: true,
            placeholder: "",
            password: false,
            multiline: false,
            plain: false,
            read_only: false,
            wrap: false,
            links: None,
        }
    }
}

/// `TextBoxLinkHighlighter.LinkRegex`: `(?:(?:https?|ftp)://|www\.)[^\s<>"]+`
/// (case-insensitive), as char ranges.
pub fn find_links(text: &str) -> Vec<std::ops::Range<usize>> {
    let chars: Vec<char> = text.chars().collect();
    let lower: Vec<char> = chars.iter().map(|c| c.to_ascii_lowercase()).collect();
    let starts_with = |i: usize, p: &str| p.chars().enumerate().all(|(k, c)| lower.get(i + k) == Some(&c));
    let word = |c: char| c.is_alphanumeric() || c == '_';
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let boundary = i == 0 || !word(chars[i - 1]);
        let prefix = ["https://", "http://", "ftp://", "www."].into_iter().find(|p| starts_with(i, p));
        if let (true, Some(p)) = (boundary, prefix) {
            let mut end = i + p.chars().count();
            while end < chars.len() && !chars[end].is_whitespace() && !matches!(chars[end], '<' | '>' | '"') {
                end += 1;
            }
            if end > i + p.chars().count() {
                out.push(i..end);
                i = end;
                continue;
            }
        }
        i += 1;
    }
    out
}

impl TextBox<'_> {
    /// A `TextPresenter` takes the control's font: Inter, Normal, at
    /// Fluent's `ControlContentThemeFontSize` (14; ShareX's `TextBox` style
    /// doesn't set one).
    pub fn font() -> FontId {
        content_font(14.0, 400)
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
        // `:focus`: a 2px border (`TextControlBorderThemeThicknessFocused`);
        // the content keeps its place, the border element being its sibling.
        let thickness = if focused && self.enabled { 2.0 } else { 1.0 };
        if !self.plain || focused {
            border::border(ui.painter(), rect, 4.0, thickness, Some(Brush::Solid(fill)), Some(p.border));
        } else if hover {
            border::border(ui.painter(), rect, 4.0, 0.0, Some(Brush::Solid(fill)), None);
        }
        // The content sits in the box's own `BorderThickness` (0 when plain).
        let inner = inset(rect, self.padding, if self.plain { 0.0 } else { 1.0 });
        let text_font = Self::font();
        let lh = line_height(ui, &text_font);
        // `TextPresenter` keeps the text colour when disabled.
        let fg = p.text;
        let edit_rect = if self.multiline {
            inner
        } else {
            Rect::from_min_size(pos2(inner.min.x, center_y(inner, lh)), vec2(inner.width(), lh))
        };
        let empty = text.is_empty();
        // A read-only box edits a copy that's thrown away.
        let mut copy = self.read_only.then(|| text.clone());
        let text = copy.as_mut().unwrap_or(text);
        let links = self.links.map(|c| (c, find_links(text)));
        let mut edit = if self.multiline { egui::TextEdit::multiline(text) } else { egui::TextEdit::singleline(text) };
        let width = if self.multiline && !self.wrap { f32::INFINITY } else { edit_rect.width() };
        edit = edit
            .id(id)
            .font(text_font)
            .text_color(fg)
            .frame(egui::Frame::NONE)
            .margin(egui::Margin::ZERO)
            .desired_width(width)
            .min_size(edit_rect.size())
            .password(self.password)
            .interactive(self.enabled);
        if !self.placeholder.is_empty() && empty {
            // `PART_Placeholder`: a `TextBlock` of its own (ShareX's SemiBold
            // Segoe UI), centred in the content like the text, in
            // `TextControlPlaceholderForeground` at 50% opacity.
            let f = fluent(ui.ctx());
            let pf = font(13.0, 600);
            let y = if self.multiline { inner.min.y } else { center_y(inner, line_height(ui, &pf)) };
            let g = galley(ui, self.placeholder, pf, f.base_medium.gamma_multiply(0.5), inner.width().max(0.0));
            ui.painter().with_clip_rect(inner.intersect(ui.clip_rect())).galley(pos2(inner.min.x, y), g, f.base_medium);
        }
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(edit_rect).layout(egui::Layout::top_down(egui::Align::Min)));
        child.set_clip_rect(inner.intersect(ui.clip_rect()));
        // ShareX's `SelectionBrush` (the accent start) and
        // `SelectionForegroundBrush`; the caret in the text colour.
        let v = child.visuals_mut();
        v.selection.bg_fill = p.accent_start;
        v.selection.stroke.color = p.accent_foreground;
        // Avalonia hides the caret while there's a selection.
        let selecting = egui::text_edit::TextEditState::load(ui.ctx(), id)
            .and_then(|s| s.cursor.char_range())
            .is_some_and(|r| r.primary != r.secondary);
        v.text_cursor.stroke = Stroke::new(1.0, if selecting { Color32::TRANSPARENT } else { p.text });
        let out = edit.show(&mut child);
        let r = out.response.response;
        if let Some((color, ranges)) = links {
            // A 1px line under each link, at the bottom of its line box.
            let painter = ui.painter().with_clip_rect(inner.intersect(ui.clip_rect()));
            let galley = &out.galley;
            let mut first = 0usize;
            for row in &galley.rows {
                let count = row.char_count_excluding_newline().0;
                for range in &ranges {
                    let (a, b) = (range.start.max(first), range.end.min(first + count));
                    if a >= b {
                        continue;
                    }
                    let x0 = out.galley_pos.x + row.pos.x + row.x_offset(egui::text::CharIndex(a - first));
                    let x1 = out.galley_pos.x + row.pos.x + row.x_offset(egui::text::CharIndex(b - first));
                    let y = (out.galley_pos.y + row.pos.y + row.height()).round() - 0.5;
                    painter.line_segment([pos2(x0, y), pos2(x1, y)], Stroke::new(1.0, color));
                }
                first += count + usize::from(row.ends_with_newline);
            }
        }
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
        let spinner = DecimalUpDown {
            minimum: self.minimum as f64,
            maximum: self.maximum as f64,
            increment: 1.0,
            decimals: 0,
            enabled: self.enabled,
        };
        let mut v = *value as f64;
        if spinner.show(ui, rect, id, &mut v) {
            *value = v.round() as i64;
            return true;
        }
        false
    }
}

/// A `NumericUpDown` over a `decimal` (`Increment` 0.1 and the like): the
/// value shown with `decimals` places, as .NET prints a decimal of that
/// scale.
pub struct DecimalUpDown {
    pub minimum: f64,
    pub maximum: f64,
    pub increment: f64,
    pub decimals: usize,
    pub enabled: bool,
}

impl DecimalUpDown {
    fn format(&self, v: f64) -> String {
        format!("{v:.prec$}", prec = self.decimals)
    }

    pub fn show(&self, ui: &mut Ui, rect: Rect, id: Id, value: &mut f64) -> bool {
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
        let scale = 10f64.powi(self.decimals as i32);
        for (i, (b, step)) in [(up, 1.0f64), (down, -1.0)].into_iter().enumerate() {
            // `ValidSpinDirections`: no increase at the maximum, no
            // decrease at the minimum.
            let valid = if step > 0.0 { *value < self.maximum } else { *value > self.minimum };
            let enabled = self.enabled && valid;
            // The repeat buttons aren't tab stops (`IsTabStop="False"`).
            let sense = if enabled { Sense::CLICK } else { Sense::hover() };
            let r = ui.interact(b, id.with(("spin", i)), sense);
            let (over, pressed) = states(&r, enabled);
            let cell = Rect::from_min_max(pos2(b.min.x + 1.0, b.min.y), b.max);
            // The presenter's square background: at rest the spinner's own
            // (opaque, so it squares off the inside of the right-hand
            // corners), else `RepeatButtonBackgroundPointerOver` /
            // `Pressed` / `Disabled` over the spinner's border.
            let bg = if !enabled {
                f.base_low
            } else if pressed {
                f.base_medium_low
            } else if over {
                f.base_high.gamma_multiply(0.1)
            } else {
                fill
            };
            ui.painter().rect_filled(cell, 0.0, bg);
            ui.painter().rect_filled(Rect::from_min_size(b.min, vec2(1.0, b.height())), 0.0, p.border);
            let glyph = Rect::from_center_size(cell.center(), vec2(16.0, 8.0));
            let color = if enabled { f.base_high } else { f.base_medium_low };
            paint_spinner_chevron(ui, glyph, step > 0.0, color);
            if focus::visible(ui.ctx(), r.id) {
                focus::paint_adorner(ui.painter(), b);
            }
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
                // Decimal arithmetic: round to the shown places.
                let v = ((*value + step * self.increment) * scale).round() / scale;
                let v = v.clamp(self.minimum, self.maximum);
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
            ui.ctx().data(|d| d.get_temp::<String>(buf_id)).unwrap_or_else(|| self.format(*value))
        } else {
            self.format(*value)
        };
        let font = TextBox::font();
        let lh = line_height(ui, &font);
        // The inner `TextBox` (`Margin="-1"`) spans the spinner's whole
        // height, borders included, and centres its line in that.
        let edit_rect = Rect::from_min_size(pos2(text_rect.min.x + 8.0, center_y(rect, lh)), vec2(text_rect.width() - 16.0, lh));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(edit_rect));
        let r = child.add_sized(
            edit_rect.size(),
            egui::TextEdit::singleline(&mut text)
                .id(buf_id)
                .font(font)
                .text_color(p.text)
                .frame(egui::Frame::NONE)
                .margin(egui::Margin::ZERO)
                .horizontal_align(egui::Align::Center)
                .interactive(self.enabled),
        );
        if r.has_focus() {
            ui.ctx().data_mut(|d| d.insert_temp(buf_id, text.clone()));
        }
        if r.lost_focus() {
            if let Ok(v) = text.trim().parse::<f64>() {
                let v = ((v * scale).round() / scale).clamp(self.minimum, self.maximum);
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
        // The track row's centre: 14px down a 32px slider. The `Track` is
        // the 21px row the 2px margins leave (rounded), its 2px
        // `TrackBackground`s 10px down it and the 20px `Thumb` at its top,
        // so the thumb's centre is 1px above the track's.
        let cy = (rect.min.y + (rect.height() - 4.0) / 2.0).floor();
        let thumb = Rect::from_center_size(pos2(cx, cy - 1.0), Vec2::splat(20.0));
        let pressed = self.enabled && (r.is_pointer_button_down_on() || r.dragged());
        // Only the pointer over the thumb narrows its ring (the thumb's own
        // `:pointerover`); the slider's hover and press recolour the track.
        let over_thumb = self.enabled && ui.rect_contains_pointer(thumb) && (over || pressed);
        let (done_color, rest_color) = if !self.enabled {
            (f.chrome_disabled_high, f.chrome_disabled_high)
        } else if pressed {
            (p.accent_start, f.base_medium_low)
        } else if over {
            (p.accent_start, f.base_medium)
        } else {
            (p.accent_border, f.base_medium_low)
        };
        // `PART_DecreaseButton` / `PART_IncreaseButton`: the track either
        // side of the thumb, `CornerRadius="3"` fitted to 2px.
        let painter = ui.painter();
        let track = |x0: f32, x1: f32, color: Color32| {
            if x1 > x0 {
                border::border(painter, Rect::from_min_max(pos2(x0, cy - 1.0), pos2(x1, cy + 1.0)), 3.0, 0.0, Some(Brush::Solid(color)), None);
            }
        };
        track(rect.min.x, thumb.min.x, done_color);
        track(thumb.max.x, rect.max.x, rest_color);
        // The thumb: the accent border colour with a ring of control
        // background (ShareX's horizontal `Thumb` keeps it when disabled).
        let ring = if over_thumb { 3.0 } else { 4.0 };
        border::border(painter, thumb, 10.0, ring, Some(Brush::Solid(p.accent_border)), Some(p.control_background));
        if focus::visible(ui.ctx(), id) {
            focus::paint_adorner(painter, rect);
        }
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
        let inner = inset(rect, b.padding, 1.0);
        let swatch = Rect::from_min_size(pos2(inner.min.x, center_y(inner, 24.0)), Vec2::splat(24.0));
        let swatch_parts = checkered_swatch(ui, swatch, color);
        let chevron = Rect::from_min_size(pos2(inner.max.x - 10.0, center_y(inner, 10.0)), Vec2::splat(10.0));
        let (r, chevron_color) = if self.enabled {
            let r = b.show(ui, rect, id, "", true);
            border::paint(ui.painter(), &swatch_parts, 1.0);
            (r, p.text_secondary)
        } else {
            // The disabled button's presenter, swatch and chevron included,
            // composited at 35% as one layer.
            let r = interact(ui, rect, id, false);
            let shell = Part::Border { rect, radius: b.corner_radius, thickness: 1.0, background: Some(Brush::Solid(p.control_background)), border: None };
            let mut parts = vec![shell];
            parts.extend(swatch_parts);
            border::paint(ui.painter(), &parts, DISABLED_OPACITY);
            (r, border::group_content(p.text_secondary, p.control_background, DISABLED_OPACITY, p.background_panel))
        };
        icons::paint(ui.painter(), chevron, crate::keys::app_icons::ChevronDown, chevron_color);
        r
    }
}

/// The swatch: `ShareX.Brush.Checkered.Small` in a 4px-corner `Border`
/// (`ClipToBounds`), the colour in another over it.
fn checkered_swatch(ui: &Ui, rect: Rect, color: Color32) -> [Part; 2] {
    let p = palette(ui.ctx());
    let board = Brush::Checkered { colors: p.checkered, cell: 4.0 };
    [
        Part::Border { rect, radius: 4.0, thickness: 0.0, background: Some(board), border: None },
        Part::Border { rect, radius: 4.0, thickness: 0.0, background: Some(Brush::Solid(color)), border: None },
    ]
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
