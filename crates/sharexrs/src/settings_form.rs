//! The settings windows' pages (`ApplicationSettingsWindow`,
//! `TaskSettingsWindow`): ShareX builds each from cards, rows, check boxes
//! and form controls (`TaskSettingsPageBuilder`, the Application settings
//! XAML) in StackPanels and Grids. A [`Form`] lays them out and paints them
//! in one pass, or only collects their text for the search
//! (`SettingsSearch.Apply`: a page matches its title and every card's text,
//! a card its own), so search and layout can't disagree.

use egui::{CornerRadius, Id, Rect, Shape, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};
use sharex_settings::Described;
use sharex_ui::controls::{self, Button, CheckBox, ComboBox, ComboItem, DecimalUpDown, NumericUpDown, Slider, TextBox};
use sharex_ui::theme::{Density, fluent, palette};
use sharex_ui::widgets::{center_y, font, galley, line_height};
use sharex_ui::border::{self, Brush, Part};
use sharex_ui::{focus, icons};

/// A card's or page's text matches every term of the query
/// (`SettingsSearch.Matches`, case-insensitive).
pub fn matches(text: &str, terms: &[String]) -> bool {
    let text = text.to_lowercase();
    terms.iter().all(|t| text.contains(t.as_str()))
}

pub fn any_combo_open(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(Id::new("settings-any-combo"))).unwrap_or(false)
}

/// Fluent's collapsed scroll bar: a thin line at the right edge.
pub fn scroll_indicator(ui: &Ui, view: Rect, scroll: f32, content: f32) {
    let f = fluent(ui.ctx());
    let max = (content - view.height()).max(1.0);
    let track = Rect::from_min_max(pos2(view.max.x - 4.0, view.min.y + 2.0), pos2(view.max.x - 2.0, view.max.y - 2.0));
    let h = (track.height() * view.height() / content).max(30.0);
    let y = track.min.y + (track.height() - h) * (scroll / max);
    ui.painter().rect_filled(Rect::from_min_size(pos2(track.min.x, y), vec2(2.0, h)), CornerRadius::same(1), f.base_medium_low);
}

pub enum Mode<'a> {
    Paint,
    Collect(&'a mut Vec<String>),
}

/// Lays out (and paints) one page's controls as ShareX's StackPanels and
/// Grids would, binding each to the settings.
pub struct Form<'u, 'a, S, W> {
    pub ui: &'u mut Ui,
    pub s: &'a mut S,
    pub w: &'a mut W,
    pub paint: bool,
    /// Collect mode: the page's text.
    pub collected: Option<Vec<String>>,
    /// While a card collects its own text for the search.
    pub card_texts: Option<Vec<String>>,
    pub terms: Vec<String>,
    pub page_title: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub first: bool,
    pub spacing: f32,
    pub d: Density,
    pub page: usize,
    /// The card's `IsEnabled` (`EnabledCard`): off disables every control in it.
    pub enabled: bool,
    /// The window's own styles: `TextBlock.section-title`'s top margin,
    /// `TextBlock.hint`'s vertical margin, `Button.compact`'s.
    pub style: FormStyle,
}

/// What differs between the settings windows' `Window.Styles`.
#[derive(Clone, Copy, Debug)]
pub struct FormStyle {
    pub title_top: f32,
    pub hint_margin: f32,
    pub button_margin: f32,
}

impl FormStyle {
    /// ApplicationSettingsWindow.axaml.
    pub const APPLICATION: FormStyle = FormStyle { title_top: 4.0, hint_margin: 0.0, button_margin: 0.0 };
    /// TaskSettingsWindow.axaml: `section-title` `Margin="0,1,0,3"`,
    /// `hint` `Margin="0,4"`, `compact` `Margin="0,4"`.
    pub const TASK: FormStyle = FormStyle { title_top: 1.0, hint_margin: 4.0, button_margin: 4.0 };
}

/// A `ComboBox.form-control`'s sizes.
#[derive(Clone, Copy, Default)]
pub struct ComboSize {
    /// A fixed `Width`.
    pub width: Option<f32>,
    /// `MinWidth` (250 unless set).
    pub min_width: Option<f32>,
}

impl<S, W> Form<'_, '_, S, W> {
    /// Whether this pass only collects text.
    pub fn collecting(&self) -> bool {
        !self.paint || self.card_texts.is_some()
    }

    pub fn text(&mut self, t: &str) {
        if let Some(c) = self.card_texts.as_mut() {
            c.push(t.to_string());
        } else if let Some(c) = self.collected.as_mut() {
            c.push(t.to_string());
        }
    }

    /// The next item's rect in the current stack, `h` tall.
    pub fn slot(&mut self, h: f32) -> Rect {
        if !self.first {
            self.y += self.spacing;
        }
        self.first = false;
        let r = Rect::from_min_size(pos2(self.x, self.y), vec2(self.width, h));
        self.y += h;
        r
    }

    pub fn label_font() -> egui::FontId {
        font(13.0, 400)
    }

    pub fn id(&self, name: &str) -> Id {
        Id::new(("settings", self.page, name.to_string()))
    }

    /// `Grid.page-title`: the accent icon, 9px, the title in accent text.
    pub fn page_title(&mut self, key: &str, title: &str) {
        if self.collecting() {
            return;
        }
        let d = &self.d;
        let tf = font(d.settings_page_title_font_size, 600);
        let lh = line_height(self.ui, &tf);
        let icon_size = d.settings_page_title_icon_size;
        let h = lh.max(icon_size);
        let r = Rect::from_min_size(pos2(self.x, self.y), vec2(self.width, h));
        let p = palette(self.ui.ctx());
        icons::paint(self.ui.painter(), Rect::from_min_size(pos2(r.min.x, center_y(r, icon_size)), Vec2::splat(icon_size)), key, p.accent_icon);
        controls::label(self.ui, Rect::from_min_max(pos2(r.min.x + icon_size + 9.0, r.min.y), r.max), title, tf, p.accent_text);
        self.y += h + d.settings_page_title_margin.bottom;
    }

    /// `Border.section-card` around what `build` adds, its children stacked
    /// `spacing` apart. Hidden when the search leaves it out.
    pub fn card(&mut self, spacing: f32, build: &mut dyn FnMut(&mut Self)) {
        self.card_if(true, spacing, build);
    }

    /// A card inside an availability container that's shown only when
    /// `available` (`SettingsSearch.IsAvailabilityContainer`).
    pub fn card_if(&mut self, available: bool, spacing: f32, build: &mut dyn FnMut(&mut Self)) {
        if !available {
            return;
        }
        // The card's own text, for the search and the page's text.
        let saved = (self.y, self.first, self.spacing);
        self.card_texts = Some(Vec::new());
        build(self);
        let texts = self.card_texts.take().unwrap_or_default();
        (self.y, self.first, self.spacing) = saved;
        if !self.paint {
            if let Some(c) = self.collected.as_mut() {
                c.extend(texts);
            }
            return;
        }
        if !self.terms.is_empty() && !matches(&format!("{} {}", self.page_title, texts.join(" ")), &self.terms) {
            return;
        }
        let d = self.d.clone();
        let pad = d.settings_card_padding;
        let bg = self.ui.painter().add(Shape::Noop);
        let top = self.y;
        let (x, width) = (self.x, self.width);
        self.x = x + 1.0 + pad.left;
        self.width = width - 2.0 - pad.left - pad.right;
        self.y = top + 1.0 + pad.top;
        self.first = true;
        self.spacing = spacing;
        build(self);
        let bottom = self.y + pad.bottom + 1.0;
        let p = palette(self.ui.ctx());
        let rect = Rect::from_min_max(pos2(x, top), pos2(x + width, bottom));
        self.ui.painter().set(
            bg,
            Shape::Vec(vec![
                egui::epaint::RectShape::filled(rect, CornerRadius::same(6), p.background_panel).into(),
                egui::epaint::RectShape::stroke(rect, CornerRadius::same(6), Stroke::new(1.0, p.border), StrokeKind::Inside).into(),
            ]),
        );
        (self.x, self.width) = (x, width);
        self.y = bottom + d.settings_card_margin.bottom;
        self.first = saved.1;
        self.spacing = saved.2;
    }

    /// A nested `StackPanel` (`Spacing`), one item of the outer stack.
    pub fn stack(&mut self, spacing: f32, build: &mut dyn FnMut(&mut Self)) {
        if !self.first {
            self.y += self.spacing;
        }
        self.first = false;
        let saved = self.spacing;
        self.first = true;
        self.spacing = spacing;
        build(self);
        self.first = false;
        self.spacing = saved;
    }

    /// `TextBlock.section-title`: 15 SemiBold (SectionTitleFontSize),
    /// `Margin="0,4,0,3"`.
    pub fn title(&mut self, t: &str) {
        self.text(t);
        if self.collecting() {
            return;
        }
        let tf = font(self.d.settings_section_title_font_size, 600);
        let lh = line_height(self.ui, &tf);
        let top = self.style.title_top;
        let r = self.slot(top + lh + 3.0);
        let p = palette(self.ui.ctx());
        self.ui.painter().galley(pos2(r.min.x, r.min.y + top), galley(self.ui, t, tf, p.text, f32::INFINITY), p.text);
    }

    /// `TextBlock.hint`: 12px Normal, secondary text, wrapped.
    pub fn hint(&mut self, t: &str) {
        self.text(t);
        if self.collecting() {
            return;
        }
        let hf = font(12.0, 400);
        let p = palette(self.ui.ctx());
        let m = self.style.hint_margin;
        let h = controls::wrapped(self.ui, pos2(0.0, 0.0), self.width, t, hf.clone(), p.text_secondary, false);
        let r = self.slot(m + h + m);
        controls::wrapped(self.ui, pos2(r.min.x, r.min.y + m), self.width, t, hf, p.text_secondary, true);
    }

    /// `CheckBox.setting`: `MinHeight` CheckBoxMinHeight, `Margin="0,1"`,
    /// Normal. Returns whether it was toggled.
    pub fn check(&mut self, t: &str, value: &mut bool, enabled: bool) -> bool {
        self.text(t);
        if self.collecting() {
            return false;
        }
        let h = self.d.settings_check_box_min_height;
        let r = self.slot(h + 2.0);
        self.check_at(r.min.x, r.min.y + 1.0, t, value, enabled)
    }

    pub fn check_at(&mut self, x: f32, y: f32, t: &str, value: &mut bool, enabled: bool) -> bool {
        let h = self.d.settings_check_box_min_height;
        let cb = CheckBox { min_height: h, enabled: enabled && self.enabled, ..CheckBox::new(t) };
        let cb = CheckBox { text_font: controls::content_font(13.0, 400), ..cb };
        let w = cb.width(self.ui);
        let id = self.id(t);
        cb.show(self.ui, Rect::from_min_size(pos2(x, y), vec2(w, h)), id, value).changed()
    }

    /// Check boxes side by side (`StackPanel Orientation="Horizontal"`).
    pub fn checks_row(&mut self, items: &mut [(&str, &mut bool)], spacing: f32) -> Option<usize> {
        for (t, _) in items.iter() {
            self.text(t);
        }
        if self.collecting() {
            return None;
        }
        let h = self.d.settings_check_box_min_height;
        let r = self.slot(h + 2.0);
        let mut x = r.min.x;
        let mut changed = None;
        for (i, (t, v)) in items.iter_mut().enumerate() {
            let cb = CheckBox { min_height: h, text_font: controls::content_font(13.0, 400), ..CheckBox::new(t) };
            let w = cb.width(self.ui);
            if self.check_at(x, r.min.y + 1.0, t, v, true) {
                changed = Some(i);
            }
            x += w + spacing;
        }
        changed
    }

    /// The height of a combo box or text box row.
    pub fn control_height(&self, font: &egui::FontId) -> f32 {
        let pad = self.d.settings_control_padding;
        (1.0 + pad.top + line_height(self.ui, font) + pad.bottom + 1.0).max(self.d.settings_control_min_height)
    }

    /// A label column `label_w` wide (8px column spacing), then a
    /// `ComboBox.form-control`. Returns the newly selected index.
    pub fn combo(&mut self, label: &str, label_w: f32, items: &[ComboItem], selected: Option<usize>, size: ComboSize, enabled: bool) -> Option<usize> {
        self.text(label);
        for i in items {
            self.text(&i.text);
        }
        if self.collecting() {
            return None;
        }
        let ch = self.control_height(&ComboBox::item_font());
        let lh = line_height(self.ui, &Self::label_font());
        let r = self.slot(ch.max(lh));
        let p = palette(self.ui.ctx());
        controls::label(self.ui, Rect::from_min_max(r.min, pos2(r.min.x + label_w, r.max.y)), label, Self::label_font(), p.text_secondary);
        let combo = ComboBox { padding: self.d.settings_control_padding, enabled: enabled && self.enabled };
        // Avalonia sizes a combo box to its selected item.
        let selected_items: Vec<ComboItem> = selected.and_then(|i| items.get(i)).cloned().into_iter().collect();
        let natural = combo.content_width(self.ui, &selected_items);
        let w = size.width.unwrap_or_else(|| natural.max(size.min_width.unwrap_or(250.0)));
        let x = r.min.x + label_w + 8.0;
        let w = w.min(r.max.x - x);
        let rect = Rect::from_min_size(pos2(x, center_y(r, ch)), vec2(w, ch));
        let id = self.id(label);
        let out = combo.show(self.ui, rect, id, items, selected);
        if ComboBox::is_open(self.ui.ctx(), id) {
            self.ui.ctx().data_mut(|d| d.insert_temp(Id::new("settings-any-combo-next"), true));
        }
        out
    }

    /// An enum `ComboBox` bound to `value` (`EnumOption<T>`).
    pub fn enum_combo<T: Described>(&mut self, label: &str, label_w: f32, value: &mut T, enabled: bool) -> bool {
        let items: Vec<ComboItem> = T::all().iter().map(|v| ComboItem::text(v.localized_description())).collect();
        let selected = T::all().iter().position(|v| v == value);
        if let Some(i) = self.combo(label, label_w, &items, selected, ComboSize::default(), enabled) {
            *value = T::all()[i].clone();
            return true;
        }
        false
    }

    /// A `TextBox.form-control` after an optional label column; it
    /// stretches (`MinWidth="300"`). Returns whether the text changed.
    pub fn text_row(&mut self, label: Option<(&str, f32)>, value: &mut String, enabled: bool, password: bool) -> bool {
        self.text_row_with_box(label, value, enabled, password).0
    }

    /// [`Form::text_row`], also returning the text box (its response, rect
    /// and id) when it was drawn: for a variable menu attached to it.
    pub fn text_row_with_box(&mut self, label: Option<(&str, f32)>, value: &mut String, enabled: bool, password: bool) -> (bool, Option<(egui::Response, Rect, Id)>) {
        if let Some((l, _)) = label {
            self.text(l);
        }
        if self.collecting() {
            return (false, None);
        }
        let th = self.control_height(&TextBox::font());
        let lh = line_height(self.ui, &Self::label_font());
        let r = self.slot(th.max(lh));
        let mut x = r.min.x;
        if let Some((l, w)) = label {
            let p = palette(self.ui.ctx());
            controls::label(self.ui, Rect::from_min_max(r.min, pos2(r.min.x + w, r.max.y)), l, Self::label_font(), p.text_secondary);
            x += w + 8.0;
        }
        let rect = Rect::from_min_max(pos2(x, center_y(r, th)), pos2(r.max.x.max(x + 300.0), center_y(r, th) + th));
        let tb = TextBox { padding: self.d.settings_control_padding, enabled: enabled && self.enabled, password, ..Default::default() };
        let id = self.id(&format!("text-{}", label.map_or("", |l| l.0)));
        let before = value.clone();
        let response = tb.show(self.ui, rect, id, value);
        (*value != before, Some((response, rect, id)))
    }

    /// A label column, then a multiline `TextBox.form-control` that wraps
    /// (`AcceptsReturn`, `TextWrapping="Wrap"`) at least `min_height` tall;
    /// the label centred on it.
    pub fn text_area_row(&mut self, label: (&str, f32), value: &mut String, enabled: bool, min_height: f32) -> bool {
        self.text(label.0);
        if self.collecting() {
            return false;
        }
        let th = self.control_height(&TextBox::font());
        let r = self.slot(min_height.max(th));
        let p = palette(self.ui.ctx());
        controls::label(self.ui, Rect::from_min_max(r.min, pos2(r.min.x + label.1, r.max.y)), label.0, Self::label_font(), p.text_secondary);
        let x = r.min.x + label.1 + 8.0;
        let rect = Rect::from_min_max(pos2(x, r.min.y), pos2(r.max.x.max(x + 300.0), r.max.y));
        let tb = TextBox { padding: self.d.settings_control_padding, enabled: enabled && self.enabled, multiline: true, ..Default::default() };
        let id = self.id(&format!("text-{}", label.0));
        let before = value.clone();
        tb.show(self.ui, rect, id, value);
        *value != before
    }

    /// A stretching text box with buttons after it (`ColumnDefinitions="*,Auto,Auto"`).
    pub fn text_with_buttons(&mut self, key: &str, value: &mut String, buttons: &[&str]) -> (bool, Option<usize>) {
        for b in buttons {
            self.text(b);
        }
        if self.collecting() {
            return (false, None);
        }
        let th = self.control_height(&TextBox::font());
        let bh = self.d.settings_control_min_height;
        let r = self.slot(th.max(bh));
        let button = Button::compact(self.d.settings_button_padding);
        let widths: Vec<f32> = buttons.iter().map(|b| button.width(self.ui, b)).collect();
        let total: f32 = widths.iter().map(|w| w + 8.0).sum();
        let tb_rect = Rect::from_min_size(pos2(r.min.x, center_y(r, th)), vec2((r.width() - total).max(300.0), th));
        let tb = TextBox { padding: self.d.settings_control_padding, enabled: self.enabled, ..Default::default() };
        let before = value.clone();
        let id = self.id(key);
        tb.show(self.ui, tb_rect, id, value);
        let mut x = tb_rect.max.x + 8.0;
        let mut clicked = None;
        for (i, (b, w)) in buttons.iter().zip(&widths).enumerate() {
            let br = Rect::from_min_size(pos2(x, center_y(r, bh)), vec2(*w, bh));
            if button.show(self.ui, br, self.id(&format!("{key}-{b}")), b, self.enabled).clicked() {
                clicked = Some(i);
            }
            x += w + 8.0;
        }
        (*value != before, clicked)
    }

    /// A label column then a `NumericUpDown.form-control` (130 wide,
    /// NumberBoxHeight tall), and an optional hint after it.
    // The `NumericUpDown`'s own properties, as the XAML sets them.
    #[allow(clippy::too_many_arguments)]
    pub fn number(&mut self, label: &str, label_w: f32, min: i64, max: i64, value: &mut i64, enabled: bool, hint: Option<&str>) -> bool {
        self.text(label);
        if let Some(h) = hint {
            self.text(h);
        }
        if self.collecting() {
            return false;
        }
        let nh = self.d.settings_number_box_height;
        let lh = line_height(self.ui, &Self::label_font());
        let r = self.slot(nh.max(lh));
        let p = palette(self.ui.ctx());
        controls::label(self.ui, Rect::from_min_max(r.min, pos2(r.min.x + label_w, r.max.y)), label, Self::label_font(), p.text_secondary);
        let rect = Rect::from_min_size(pos2(r.min.x + label_w + 8.0, center_y(r, nh)), vec2(130.0, nh));
        if let Some(h) = hint {
            let hx = rect.max.x + 5.0;
            controls::label(self.ui, Rect::from_min_max(pos2(hx, r.min.y), r.max), h, font(12.0, 400), p.text_secondary);
        }
        let id = self.id(label);
        NumericUpDown { minimum: min, maximum: max, enabled: enabled && self.enabled }.show(self.ui, rect, id, value)
    }

    /// A `Button.compact`, left-aligned. Returns whether it was clicked.
    pub fn button(&mut self, t: &str, enabled: bool) -> bool {
        self.buttons(&[(t, enabled)], 5.0) == Some(0)
    }

    /// Compact buttons side by side, `spacing` apart.
    pub fn buttons(&mut self, items: &[(&str, bool)], spacing: f32) -> Option<usize> {
        for (t, _) in items {
            self.text(t);
        }
        if self.collecting() {
            return None;
        }
        let bh = self.d.settings_control_min_height;
        let m = self.style.button_margin;
        let r = self.slot(m + bh + m);
        let button = Button::compact(self.d.settings_button_padding);
        let mut x = r.min.x;
        let mut clicked = None;
        for (i, (t, enabled)) in items.iter().enumerate() {
            let w = button.width(self.ui, t);
            if button.show(self.ui, Rect::from_min_size(pos2(x, r.min.y + m), vec2(w, bh)), self.id(&format!("button-{t}")), t, *enabled && self.enabled).clicked() {
                clicked = Some(i);
            }
            x += w + spacing;
        }
        clicked
    }

    /// A spacing slider row: label (190), slider (*), value text (80).
    pub fn slider(&mut self, label: &str, value: &mut i32, text: &str) -> bool {
        self.text(label);
        if self.collecting() {
            return false;
        }
        let lh = line_height(self.ui, &Self::label_font());
        // The Fluent slider's 32px `MinHeight` with 2px pre/post margins.
        let r = self.slot(32f32.max(lh));
        let p = palette(self.ui.ctx());
        controls::label(self.ui, Rect::from_min_max(r.min, pos2(r.min.x + 190.0, r.max.y)), label, Self::label_font(), p.text_secondary);
        let slider_rect = Rect::from_min_max(pos2(r.min.x + 198.0, r.min.y), pos2(r.max.x - 88.0, r.max.y));
        controls::label(self.ui, Rect::from_min_max(pos2(r.max.x - 80.0, r.min.y), r.max), text, Self::label_font(), p.text_secondary);
        let mut v = *value as f32;
        let id = self.id(label);
        if (Slider { minimum: 0.0, maximum: 200.0, step: 5.0, enabled: self.enabled }).show(self.ui, slider_rect, id, &mut v) {
            let v = ((v / 5.0).round() * 5.0).clamp(0.0, 200.0) as i32;
            if v != *value {
                *value = v;
                return true;
            }
        }
        false
    }

    /// A label column then a `NumericUpDown.form-control` over a decimal
    /// (`Increment` 0.1 and the like), shown with `decimals` places.
    #[allow(clippy::too_many_arguments)]
    pub fn number_f64(&mut self, label: &str, label_w: f32, min: f64, max: f64, increment: f64, decimals: usize, value: &mut f64, enabled: bool) -> bool {
        self.number_f64_w(label, label_w, 130.0, min, max, increment, decimals, value, enabled)
    }

    /// [`Form::number_f64`] with its own `Width`.
    #[allow(clippy::too_many_arguments)]
    pub fn number_f64_w(&mut self, label: &str, label_w: f32, width: f32, min: f64, max: f64, increment: f64, decimals: usize, value: &mut f64, enabled: bool) -> bool {
        self.text(label);
        if self.collecting() {
            return false;
        }
        let nh = self.d.settings_number_box_height;
        let lh = line_height(self.ui, &Self::label_font());
        let r = self.slot(nh.max(lh));
        let p = palette(self.ui.ctx());
        controls::label(self.ui, Rect::from_min_max(r.min, pos2(r.min.x + label_w, r.max.y)), label, Self::label_font(), p.text_secondary);
        let rect = Rect::from_min_size(pos2(r.min.x + label_w + 8.0, center_y(r, nh)), vec2(width, nh));
        let id = self.id(label);
        DecimalUpDown { minimum: min, maximum: max, increment, decimals, enabled: enabled && self.enabled }.show(self.ui, rect, id, value)
    }

    /// `Border.override-card` (accent border, `Padding="12,8"`,
    /// `Margin="0,0,0,8"`) around a check box. Returns whether it changed.
    pub fn override_card(&mut self, t: &str, value: &mut bool) -> bool {
        self.text(t);
        if self.collecting() {
            return false;
        }
        if !self.terms.is_empty() && !matches(&format!("{} {}", self.page_title, t), &self.terms) {
            return false;
        }
        let h = self.d.settings_check_box_min_height;
        let card_h = 1.0 + 8.0 + h + 2.0 + 8.0 + 1.0;
        if !self.first {
            self.y += self.spacing;
        }
        self.first = false;
        let r = Rect::from_min_size(pos2(self.x, self.y), vec2(self.width, card_h));
        self.y += card_h + 8.0;
        let p = palette(self.ui.ctx());
        self.ui.painter().rect_filled(r, CornerRadius::same(6), p.background_panel);
        self.ui.painter().rect_stroke(r, CornerRadius::same(6), Stroke::new(1.0, p.accent_border), StrokeKind::Inside);
        let saved = self.enabled;
        self.enabled = true;
        let changed = self.check_at(r.min.x + 13.0, r.min.y + 9.0 + 1.0, t, value, true);
        self.enabled = saved;
        changed
    }

    /// A stretching text box with buttons after it, `spacing` apart
    /// (`InputWithButton`: `ColumnDefinitions="*,Auto" ColumnSpacing="6"`).
    pub fn text_with_buttons_spaced(&mut self, key: &str, value: &mut String, buttons: &[&str], spacing: f32) -> (bool, Option<usize>) {
        for b in buttons {
            self.text(b);
        }
        if self.collecting() {
            return (false, None);
        }
        let th = self.control_height(&TextBox::font());
        let bh = self.d.settings_control_min_height;
        let m = self.style.button_margin;
        let r = self.slot(th.max(m + bh + m));
        let button = Button::compact(self.d.settings_button_padding);
        let widths: Vec<f32> = buttons.iter().map(|b| button.width(self.ui, b)).collect();
        let total: f32 = widths.iter().map(|w| w + spacing).sum();
        // The `TextBox` stretches to the row, which the button's margins make
        // taller than the box would be.
        let tb_rect = Rect::from_min_size(r.min, vec2((r.width() - total).max(300.0), r.height()));
        let tb = TextBox { padding: self.d.settings_control_padding, enabled: self.enabled, ..Default::default() };
        let before = value.clone();
        let id = self.id(key);
        tb.show(self.ui, tb_rect, id, value);
        let mut x = tb_rect.max.x + spacing;
        let mut clicked = None;
        for (i, (b, w)) in buttons.iter().zip(&widths).enumerate() {
            let br = Rect::from_min_size(pos2(x, center_y(r, bh)), vec2(*w, bh));
            if button.show(self.ui, br, self.id(&format!("{key}-{b}")), b, self.enabled).clicked() {
                clicked = Some(i);
            }
            x += w + spacing;
        }
        (*value != before, clicked)
    }

    /// `ListBox.settings-list`: the main background, a 1px border, a row
    /// per item (the selected one on the accent). Returns a clicked row.
    pub fn list_box(&mut self, key: &str, rows: &[String], selected: Option<usize>, min_height: f32) -> Option<usize> {
        for r in rows {
            self.text(r);
        }
        if self.collecting() {
            return None;
        }
        let p = palette(self.ui.ctx());
        let f = fluent(self.ui.ctx());
        let body = font(13.0, 400);
        let lh = line_height(self.ui, &body);
        // Fluent's `ListBoxItem`: `Padding="12,9,12,12"`.
        let row_h = 9.0 + lh + 12.0;
        let r = self.slot((row_h * rows.len() as f32 + 2.0).max(min_height));
        self.ui.painter().rect_filled(r, CornerRadius::same(3), p.background_main);
        self.ui.painter().rect_stroke(r, CornerRadius::same(3), Stroke::new(1.0, p.border), StrokeKind::Inside);
        let mut clicked = None;
        for (i, text) in rows.iter().enumerate() {
            let row = Rect::from_min_size(pos2(r.min.x + 1.0, r.min.y + 1.0 + row_h * i as f32), vec2(r.width() - 2.0, row_h));
            let resp = self.ui.interact(row, self.id(&format!("{key}-row-{i}")), egui::Sense::click());
            let sel = selected == Some(i);
            if sel {
                sharex_ui::menu::paint_accent_gradient(self.ui, row, CornerRadius::same(3));
            } else if resp.hovered() && self.enabled {
                self.ui.painter().rect_filled(row, CornerRadius::same(3), f.list_low);
            }
            let fg = if sel { p.accent_foreground } else { p.text_secondary };
            let g = galley(self.ui, text, body.clone(), fg, row.width() - 24.0);
            self.ui.painter().galley(pos2(row.min.x + 12.0, row.min.y + 9.0), g, fg);
            if resp.clicked() && self.enabled {
                clicked = Some(i);
            }
        }
        clicked
    }

}

/// A `SettingsNavigation` `TreeViewItem`'s background (`CornerRadius="4"`):
/// selected, Fluent's solid `TreeViewItemBackgroundSelected*` with
/// `ShareX.BoxShadow.Accent.Outline` (an inset `BoxShadow`, spread 1, in
/// the accent border colour, clipped to the corners) over it, drawn as one
/// shape; otherwise the list-low / list-medium pointer fills.
pub fn nav_item_background(painter: &egui::Painter, r: Rect, resp: &egui::Response, selected: bool) {
    let ctx = painter.ctx();
    let (p, f) = (palette(ctx), fluent(ctx));
    let fill = |color| Part::Border { rect: r, radius: 4.0, thickness: 0.0, background: Some(Brush::Solid(color)), border: None };
    let pressed = resp.is_pointer_button_down_on();
    if selected {
        let color = if pressed {
            p.accent_fill_dark[0]
        } else if resp.hovered() {
            p.accent_fill_light[0]
        } else {
            p.accent_start
        };
        let outline = Part::InsetShadow { rect: r, radius: 4.0, spread: 1.0, color: p.accent_border };
        border::paint(painter, &[fill(color), outline], 1.0);
    } else if pressed {
        border::paint(painter, &[fill(f.list_medium)], 1.0);
    } else if resp.hovered() {
        border::paint(painter, &[fill(f.list_low)], 1.0);
    }
}

/// Fluent's focus adorner on a navigation item with keyboard focus.
pub fn nav_item_focus(painter: &egui::Painter, r: Rect, resp: &egui::Response) {
    if focus::visible(painter.ctx(), resp.id) {
        focus::paint_adorner(painter, r);
    }
}
