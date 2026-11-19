//! `ShareX.ImageEffectsLib/Presentation/ImageEffectOptionsPanel` and
//! `GradientOptionsPanel`: an editor per property of the selected effect
//! (`TypeDescriptor.GetProperties`, browsable and writable, in declaration
//! order), chosen from the property's type as `CreateEditor` does: a
//! `ToggleSwitch`, an enum `ComboBox`, a `NumericUpDown`, the colour button,
//! X/Y or width/height boxes, the four margin boxes, Edit gradient... or a
//! `TextBox` through the type's converter; a value that doesn't convert
//! shows its error under the editors (`ValidationText`).

use egui::{Color32, Id, Rect, Ui, pos2, vec2};
use serde_json::Value;
use sharex_image_effects::dotnet::{Color, GradientInfo, GradientMode, GradientStop, Margins, Point, Prop, PropKind, Size};
use sharex_image_effects::localization;
use sharex_image_effects::preset::ImageEffect;
use sharex_strings::image_effects_lib as s;
use sharex_ui::border::{self, Brush};
use sharex_ui::controls::{self, Button, ComboBox, ComboItem, DecimalUpDown, TextBox, ToggleSwitch};
use sharex_ui::theme::{Thickness, fluent, palette};
use sharex_ui::widgets::{center_y, font, line_height, text_width};

use crate::image_tools as tools;

/// A row: `Grid ColumnDefinitions="140,*" ColumnSpacing="10" MinHeight="36"`.
const LABEL_WIDTH: f32 = 140.0;
const ROW_MIN: f32 = 36.0;
/// `NumericUpDown`'s width when its column is narrower (the inner
/// `TextBox`'s `MinWidth` 64 less its -1 margins).
const NUMBER_MIN_WIDTH: f32 = 62.0;

/// The flyouts the panel opens.
#[derive(Clone, Debug, PartialEq)]
enum Flyout {
    /// A colour property's `ColorView` (the shared colour popup).
    Color(String),
    /// A gradient property's `GradientOptionsPanel`.
    Gradient(String),
    /// A gradient stop's colour (the property, the stop).
    StopColor(String, usize),
}

#[derive(Default)]
pub struct OptionsPanel {
    /// The text boxes' text while edited (by property), so a value that
    /// doesn't convert stays as typed.
    texts: std::collections::HashMap<String, String>,
    /// `ValidationText`.
    pub validation: String,
    flyout: Option<Flyout>,
    /// Which effect the editors were built for (`SetEffect`).
    built_for: Option<usize>,
    scroll: f32,
}

/// .NET's `{0:0.###}`.
pub fn format_number(v: f64) -> String {
    let s = format!("{:.3}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    if s == "-0" { "0".into() } else { s }
}

/// A colour button's text (`UpdateColorPreview`): `#RRGGBB`, or `#AARRGGBB`
/// when not opaque.
pub fn color_text(c: &Color) -> String {
    if c.a == 255 { format!("#{:02X}{:02X}{:02X}", c.r, c.g, c.b) } else { format!("#{:02X}{:02X}{:02X}{:02X}", c.a, c.r, c.g, c.b) }
}

fn color32(c: &Color) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a)
}

impl OptionsPanel {
    /// `SetEffect`: the editors made again for another effect (`key`).
    pub fn set_effect(&mut self, key: Option<usize>) {
        if self.built_for != key {
            self.built_for = key;
            self.texts.clear();
            self.validation.clear();
            self.flyout = None;
            self.scroll = 0.0;
        }
    }

    /// One editor row's height for `kind`.
    fn row_height(ui: &Ui, kind: PropKind) -> f32 {
        let editor = match kind {
            PropKind::Bool => ToggleSwitch::SIZE.y,
            PropKind::Margins => line_height(ui, &font(13.0, 400)) + 3.0 + 40.0,
            _ => 40.0,
        };
        editor.max(ROW_MIN)
    }

    /// The panel in `rect` (the `tool-panel`'s content box). Returns whether
    /// the effect changed.
    pub fn ui(&mut self, ui: &mut Ui, rect: Rect, effect: Option<&mut ImageEffect>, live: bool) -> bool {
        let p = palette(ui.ctx());
        let hh = tools::heading_height(ui);
        let nf = font(13.0, 400);
        let nl = line_height(ui, &nf);
        let title = match &effect {
            Some(e) => s::ImageEffectOptionsPanel_Effect_options_format().replace("{0}", e.kind.display_name()),
            None => s::ImageEffectOptionsPanel_Effect_options().to_string(),
        };
        tools::heading(ui, Rect::from_min_size(rect.min, vec2(rect.width(), hh)), &title);
        // The validation text's row keeps a line even when empty.
        let vlines = if self.validation.is_empty() { 1 } else { controls::wrap_lines(ui, &self.validation, &nf, rect.width()).len() };
        let validation_h = nl * vlines as f32;
        let view = Rect::from_min_max(pos2(rect.min.x, rect.min.y + hh + 10.0), pos2(rect.max.x, rect.max.y - validation_h - 10.0));
        if !self.validation.is_empty() {
            controls::wrapped(ui, pos2(rect.min.x, view.max.y + 10.0), rect.width(), &self.validation, nf.clone(), p.cancel_border, true);
        }
        // `StackPanel Spacing="8" Margin="0,4,12,4"` in the `ScrollViewer`.
        let Some(effect) = effect else {
            tools::text_at(ui, pos2(view.min.x, view.min.y + 4.0), s::ImageEffectOptionsPanel_Select_an_effect(), nf, p.text_secondary);
            return false;
        };
        let names = effect.kind.property_names();
        let kinds = effect.kind.property_kinds();
        if names.is_empty() {
            tools::text_at(ui, pos2(view.min.x, view.min.y + 4.0), s::ImageEffectOptionsPanel_No_configurable_properties(), nf, p.text_secondary);
            return false;
        }
        let heights: Vec<f32> = kinds.iter().map(|k| Self::row_height(ui, *k)).collect();
        let content_h = 8.0 + tools::stack_height(&heights, 8.0);
        let max_scroll = (content_h - view.height()).max(0.0);
        if live && ui.rect_contains_pointer(view) && self.flyout.is_none() {
            self.scroll -= ui.input(|i| i.smooth_scroll_delta.y);
        }
        self.scroll = self.scroll.clamp(0.0, max_scroll);
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(view));
        child.set_clip_rect(view.intersect(ui.clip_rect()));
        let type_name = effect.kind.type_name();
        let mut changed = false;
        let width = view.width() - 12.0;
        let mut y = view.min.y + 4.0 - self.scroll;
        for (k, (name, kind)) in names.iter().zip(kinds).enumerate() {
            let h = heights[k];
            let row = Rect::from_min_size(pos2(view.min.x, y), vec2(width, h));
            y += h + 8.0;
            if row.max.y < view.min.y || row.min.y > view.max.y {
                continue;
            }
            let label = localization::property_name(name);
            let lr = Rect::from_min_size(row.min, vec2(LABEL_WIDTH, h));
            let lines = controls::wrap_lines(&child, &label, &nf, LABEL_WIDTH);
            let top = center_y(lr, nl * lines.len() as f32);
            for (i, l) in lines.iter().enumerate() {
                tools::text_at(&child, pos2(lr.min.x, top + nl * i as f32), l, nf.clone(), p.text_secondary);
            }
            if let Some(tip) = localization::property_description(type_name, name) {
                let _ = child.interact(lr, Id::new(("effect-label", *name)), egui::Sense::hover()).on_hover_text(tip);
            }
            let editor = Rect::from_min_max(pos2(row.min.x + LABEL_WIDTH + 10.0, row.min.y), row.max);
            if let Some(v) = self.editor(&mut child, editor, effect, name, *kind, live) {
                if effect.kind.set_property(name, &v) {
                    self.validation.clear();
                    changed = true;
                } else {
                    self.validation = format!("{} is not a valid value for {}.", value_text(&v), label);
                }
            }
        }
        if max_scroll > 0.0 {
            crate::dialogs::layout::scroll_bar(ui, view, content_h, self.scroll, 16.0);
        }
        changed
    }

    /// An editor; returns the property's new JSON value when it changed.
    fn editor(&mut self, ui: &mut Ui, rect: Rect, effect: &ImageEffect, name: &str, kind: PropKind, live: bool) -> Option<Value> {
        let value = effect.kind.get_property(name).unwrap_or(Value::Null);
        let id = Id::new(("effect-prop", name));
        let nf = font(13.0, 400);
        match kind {
            PropKind::Bool => {
                let mut on = value.as_bool().unwrap_or(false);
                let before = on;
                ToggleSwitch { enabled: live }.show(ui, pos2(rect.min.x, center_y(rect, ToggleSwitch::SIZE.y)), id, &mut on);
                (on != before).then_some(Value::Bool(on))
            }
            PropKind::Enum { names, resource } => {
                let items: Vec<ComboItem> = names.iter().map(|n| ComboItem::normal(localization::enum_value(resource, n))).collect();
                let current = value.as_str().and_then(|v| names.iter().position(|n| n.eq_ignore_ascii_case(v)));
                let combo = ComboBox { padding: Thickness::hv(8.0, 6.0), enabled: live };
                combo.show(ui, Rect::from_min_size(rect.min, vec2(rect.width(), 40.0)), id, &items, current).map(|i| Value::String(names[i].to_string()))
            }
            PropKind::Int | PropKind::Float => {
                let float = kind == PropKind::Float;
                let mut v = value.as_f64().unwrap_or(0.0);
                let before = v;
                let spinner = DecimalUpDown { minimum: -1_000_000.0, maximum: 1_000_000.0, increment: if float { 0.1 } else { 1.0 }, decimals: if float { 3 } else { 0 }, enabled: live };
                let r = Rect::from_min_size(rect.min, vec2(rect.width(), 40.0));
                spinner.show_with(ui, r, id, &mut v, &|x| if float { format_number(x) } else { format!("{}", x.round() as i64) }, true);
                (v != before).then(|| if float { serde_json::json!(v) } else { serde_json::json!(v.round() as i64) })
            }
            PropKind::Color => {
                let c = Color::from_json(&value).unwrap_or(Color::argb(0, 0, 0, 0));
                let text = color_text(&c);
                // `Button` with a 28x24 swatch (3px corners, #707070 border) and the text, `Spacing="8"`.
                let w = 1.0 + 10.0 + 28.0 + 8.0 + text_width(ui, &text, nf.clone()) + 10.0 + 1.0;
                let br = Rect::from_min_size(rect.min, vec2(w.min(rect.width()), 40.0));
                let resp = Button::default().show(ui, br, id, "", live);
                let sw = Rect::from_min_size(pos2(br.min.x + 11.0, center_y(br, 24.0)), vec2(28.0, 24.0));
                border::border(ui.painter(), sw, 3.0, 1.0, Some(Brush::Solid(color32(&c))), Some(Color32::from_rgb(112, 112, 112)));
                let p = palette(ui.ctx());
                tools::text_at(ui, pos2(sw.max.x + 8.0, center_y(br, line_height(ui, &nf))), &text, nf, p.text_secondary);
                let key = Flyout::Color(name.to_string());
                if resp.clicked() {
                    self.flyout = if self.flyout.as_ref() == Some(&key) { None } else { Some(key.clone()) };
                }
                if self.flyout.as_ref() == Some(&key) {
                    let mut rgba = [c.r, c.g, c.b, c.a];
                    // `ColorView` (`MinWidth="320"`, alpha, preview) in a `Flyout`.
                    let (ch, keep) = controls::color_popup_styled(ui, id, br, &mut rgba, resp.clicked(), controls::ColorPopup::Flyout { alpha: true, size: None });
                    if !keep {
                        self.flyout = None;
                    }
                    if ch {
                        // `ToDrawing(picker.Color)`, alpha included.
                        let [r, g, b, a] = rgba;
                        return Some(Color::argb(a, r, g, b).to_json());
                    }
                }
                None
            }
            PropKind::Point | PropKind::Size => {
                let (a, b) = if kind == PropKind::Point {
                    let pt = Point::from_json(&value).unwrap_or(Point::new(0, 0));
                    (pt.x, pt.y)
                } else {
                    let sz = Size::from_json(&value).unwrap_or(Size::new(0, 0));
                    (sz.width, sz.height)
                };
                let (la, lb) = if kind == PropKind::Point {
                    (s::ImageEffectOptionsPanel_X_short(), s::ImageEffectOptionsPanel_Y_short())
                } else {
                    (s::ImageEffectOptionsPanel_Width_short(), s::ImageEffectOptionsPanel_Height_short())
                };
                // `ColumnDefinitions="Auto,*,Auto,*" ColumnSpacing="6"`.
                let (wa, wb) = (text_width(ui, la, nf.clone()), text_width(ui, lb, nf.clone()));
                let star = ((rect.width() - wa - wb - 18.0) / 2.0).max(0.0);
                let r = Rect::from_min_size(rect.min, vec2(rect.width(), 40.0));
                let p = palette(ui.ctx());
                let lh = line_height(ui, &nf);
                tools::text_at(ui, pos2(r.min.x, center_y(r, lh)), la, nf.clone(), p.text_secondary);
                let xa = r.min.x + wa + 6.0;
                let mut va = a as f64;
                let ca = number_box(ui, Rect::from_min_size(pos2(xa, r.min.y), vec2(star, 40.0)), id.with("a"), &mut va, live);
                let xl = xa + star + 6.0;
                tools::text_at(ui, pos2(xl, center_y(r, lh)), lb, nf.clone(), p.text_secondary);
                let xb = xl + wb + 6.0;
                let mut vb = b as f64;
                let cb = number_box(ui, Rect::from_min_size(pos2(xb, r.min.y), vec2(star, 40.0)), id.with("b"), &mut vb, live);
                (ca || cb).then(|| {
                    let (a, b) = (va as i32, vb as i32);
                    if kind == PropKind::Point { Point::new(a, b).to_json() } else { Size::new(a, b).to_json() }
                })
            }
            PropKind::Margins => {
                let m = Margins::from_json(&value).unwrap_or(Margins::all(0));
                let mut vals = [m.left as f64, m.top as f64, m.right as f64, m.bottom as f64];
                let labels = [
                    s::ImageEffectOptionsPanel_Left_short(),
                    s::ImageEffectOptionsPanel_Top_short(),
                    s::ImageEffectOptionsPanel_Right_short(),
                    s::ImageEffectOptionsPanel_Bottom_short(),
                ];
                // `ColumnDefinitions="*,*,*,*" ColumnSpacing="6"`; each a label
                // over its box (`Spacing="3"`), the boxes centred on their
                // columns when wider than them.
                let col = ((rect.width() - 18.0) / 4.0).max(0.0);
                let lh = line_height(ui, &nf);
                let p = palette(ui.ctx());
                let mut changed = false;
                for (i, label) in labels.iter().enumerate() {
                    let cx = rect.min.x + i as f32 * (col + 6.0) + col / 2.0;
                    let lw = text_width(ui, label, nf.clone());
                    tools::text_at(ui, pos2(cx - (lw / 2.0).round_ties_even(), rect.min.y), label, nf.clone(), p.text_secondary);
                    let w = col.max(NUMBER_MIN_WIDTH);
                    let br = Rect::from_min_size(pos2((cx - w / 2.0).round_ties_even(), rect.min.y + lh + 3.0), vec2(w, 40.0));
                    changed |= number_box(ui, br, id.with(i), &mut vals[i], live);
                }
                changed.then(|| Margins::new(vals[0] as i32, vals[1] as i32, vals[2] as i32, vals[3] as i32).to_json())
            }
            PropKind::Gradient => {
                let text = s::ImageEffectOptionsPanel_Edit_gradient();
                let bw = tools::text_button_width(ui, text);
                let br = Rect::from_min_size(rect.min, vec2(bw.min(rect.width()), 40.0));
                let key = Flyout::Gradient(name.to_string());
                let resp = tools::text_button(ui, br, id, text, live);
                let mut out = None;
                if resp.clicked() {
                    if self.flyout.as_ref() == Some(&key) {
                        self.flyout = None;
                    } else {
                        self.flyout = Some(key.clone());
                        // A null gradient becomes a new one (`Apply(property, gradient)`).
                        if value.is_null() {
                            out = Some(GradientInfo::new(GradientMode::Vertical, &[]).to_json_value());
                        }
                    }
                }
                if matches!(&self.flyout, Some(Flyout::Gradient(n)) | Some(Flyout::StopColor(n, _)) if n == name) {
                    let mut g = <Option<GradientInfo>>::from_json(out.as_ref().unwrap_or(&value)).flatten().unwrap_or_else(|| GradientInfo::new(GradientMode::Vertical, &[]));
                    let (changed, keep) = self.gradient_flyout(ui, br, name, &mut g, resp.clicked());
                    if !keep {
                        // `flyout.Closed += gradient.Sort()`.
                        g.colors.sort_by(|a, b| a.location.partial_cmp(&b.location).unwrap_or(std::cmp::Ordering::Equal));
                        self.flyout = None;
                        return Some(g.to_json_value());
                    }
                    if changed {
                        out = Some(g.to_json_value());
                    }
                }
                out
            }
            PropKind::Text => {
                let shown = value_text(&value);
                let text = self.texts.entry(name.to_string()).or_insert_with(|| shown.clone());
                if !ui.ctx().memory(|m| m.has_focus(id)) && self.validation.is_empty() {
                    *text = shown.clone();
                }
                let before = text.clone();
                TextBox { enabled: live, ..TextBox::default() }.show(ui, Rect::from_min_size(rect.min, vec2(rect.width(), 40.0)), id, text);
                // `TextChanged` → `SetFromText`.
                (*text != before).then(|| Value::String(text.clone()))
            }
        }
    }

    /// `GradientOptionsPanel` in its flyout under `anchor`: Direction, Add
    /// stop, the stops (colour button, location, Remove). Returns whether
    /// the gradient changed and whether the flyout stays open.
    fn gradient_flyout(&mut self, ui: &Ui, anchor: Rect, name: &str, g: &mut GradientInfo, just_opened: bool) -> (bool, bool) {
        let mut changed = false;
        let f = fluent(ui.ctx());
        let p = palette(ui.ctx());
        let width = 380.0f32;
        let stops_h = (g.colors.len() as f32 * 48.0 - 8.0).clamp(0.0, 300.0);
        let height = 12.0 + 40.0 + 10.0 + 40.0 + 10.0 + stops_h + 12.0;
        let pos = pos2(anchor.min.x, anchor.max.y + 4.0);
        let mut stop_color: Option<(usize, Rect)> = None;
        let area = egui::Area::new(Id::new(("gradient-flyout", name))).order(egui::Order::Foreground).fixed_pos(pos).fade_in(false).show(ui.ctx(), |ui| {
            let (rect, _) = ui.allocate_exact_size(vec2(width, height), egui::Sense::click());
            // `FlyoutPresenter` (transient border, 5px corners) and the
            // panel's `MenuFlyoutPresenterBackground`.
            let bg = if ui.ctx().theme() == egui::Theme::Dark { Color32::from_rgb(0x2C, 0x2C, 0x2C) } else { Color32::from_rgb(0xF9, 0xF9, 0xF9) };
            border::border(ui.painter(), rect, 5.0, 1.0, Some(Brush::Solid(bg)), Some(f.transient_border));
            let c = rect.shrink(13.0);
            let nf = font(13.0, 400);
            let lh = line_height(ui, &nf);
            let row = Rect::from_min_size(c.min, vec2(c.width(), 40.0));
            tools::text_at(ui, pos2(row.min.x, center_y(row, lh)), s::GradientOptionsPanel_Direction(), nf.clone(), p.text_secondary);
            let modes: Vec<ComboItem> = GradientMode::NAMES.iter().map(|n| ComboItem::text(localization::enum_value("LinearGradientMode", n))).collect();
            let combo = ComboBox { padding: Thickness::hv(8.0, 6.0), enabled: true };
            let current = GradientMode::ALL.iter().position(|m| *m == g.kind);
            let cw = combo.auto_width(ui, &modes, current);
            if let Some(i) = combo.show(ui, Rect::from_min_size(pos2(row.max.x - cw, row.min.y), vec2(cw, 40.0)), Id::new(("gradient-direction", name)), &modes, current) {
                g.kind = GradientMode::ALL[i];
                changed = true;
            }
            let row = Rect::from_min_size(pos2(c.min.x, row.max.y + 10.0), vec2(c.width(), 40.0));
            tools::text_at(ui, pos2(row.min.x, center_y(row, lh)), s::GradientOptionsPanel_Color_stops(), nf.clone(), p.text_secondary);
            let add = s::GradientOptionsPanel_Add_stop();
            let aw = tools::text_button_width(ui, add);
            if tools::text_button(ui, Rect::from_min_size(pos2(row.max.x - aw, row.min.y), vec2(aw, 40.0)), Id::new(("gradient-add", name)), add, true).clicked() {
                // `OnAddStopClick`: white, 10 after the last stop.
                let location = if g.colors.is_empty() { 0.0 } else { (g.colors.iter().map(|s| s.location).fold(f32::MIN, f32::max) + 10.0).min(100.0) };
                g.colors.push(GradientStop { color: Color::rgb(255, 255, 255), location });
                changed = true;
            }
            // The stops, by location (`OrderBy`).
            let mut order: Vec<usize> = (0..g.colors.len()).collect();
            order.sort_by(|a, b| g.colors[*a].location.partial_cmp(&g.colors[*b].location).unwrap_or(std::cmp::Ordering::Equal));
            let mut y = row.max.y + 10.0;
            let mut remove = None;
            for &i in &order {
                let r = Rect::from_min_size(pos2(c.min.x, y), vec2(c.width(), 40.0));
                let br = Rect::from_min_size(r.min, vec2(42.0, 40.0));
                if Button::compact(Thickness::new(4.0, 4.0, 4.0, 4.0)).show(ui, br, Id::new(("gradient-stop-color", name, i)), "", true).clicked() {
                    stop_color = Some((i, br));
                }
                let sw = Rect::from_center_size(br.center(), vec2(28.0, 28.0));
                border::border(ui.painter(), sw, 3.0, 0.0, Some(Brush::Solid(color32(&g.colors[i].color))), None);
                let rm = s::GradientOptionsPanel_Remove();
                let rw = tools::text_button_width(ui, rm);
                let rr = Rect::from_min_size(pos2(r.max.x - rw, r.min.y), vec2(rw, 40.0));
                let nr = Rect::from_min_max(pos2(br.max.x + 8.0, r.min.y), pos2(rr.min.x - 8.0, r.max.y));
                let mut loc = g.colors[i].location as f64;
                let spinner = DecimalUpDown { minimum: 0.0, maximum: 100.0, increment: 1.0, decimals: 0, enabled: true };
                let mut child = ui.new_child(egui::UiBuilder::new().max_rect(nr));
                if spinner.show_with(&mut child, nr, Id::new(("gradient-stop-loc", name, i)), &mut loc, &|v| format!("{}%", v.round() as i64), true) {
                    g.colors[i].location = loc as f32;
                    changed = true;
                }
                if tools::text_button(ui, rr, Id::new(("gradient-stop-remove", name, i)), rm, true).clicked() {
                    remove = Some(i);
                }
                y += 48.0;
            }
            if let Some(i) = remove {
                g.colors.remove(i);
                changed = true;
            }
        });
        if let Some((i, _)) = stop_color {
            self.flyout = Some(Flyout::StopColor(name.to_string(), i));
        }
        if let Some(Flyout::StopColor(n, i)) = self.flyout.clone()
            && n == name
            && let Some(stop) = g.colors.get_mut(i)
        {
            let anchor = Rect::from_min_size(area.response.rect.max - vec2(42.0, 40.0), vec2(42.0, 40.0));
            let mut rgba = [stop.color.r, stop.color.g, stop.color.b, stop.color.a];
            let (ch, keep) = controls::color_popup_styled(ui, Id::new(("gradient-stop-popup", name, i)), anchor, &mut rgba, stop_color.is_some(), controls::ColorPopup::Flyout { alpha: true, size: None });
            if ch {
                let [r, g2, b, a] = rgba;
                stop.color = Color::argb(a, r, g2, b);
                changed = true;
            }
            if !keep {
                self.flyout = Some(Flyout::Gradient(name.to_string()));
            }
            return (changed, true);
        }
        let ctx = ui.ctx();
        let mut keep = !ctx.input(|i| i.key_pressed(egui::Key::Escape));
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
}

/// A `NumericUpDown` without its spinner (`CreateIntegerInput`).
fn number_box(ui: &mut Ui, rect: Rect, id: Id, value: &mut f64, live: bool) -> bool {
    let spinner = DecimalUpDown { minimum: -1_000_000.0, maximum: 1_000_000.0, increment: 1.0, decimals: 0, enabled: live };
    spinner.show_with(ui, rect, id, value, &|v| format!("{}", v.round() as i64), false)
}

/// A property's value as its `TypeConverter.ConvertToString` gives it.
fn value_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

trait GradientJson {
    fn to_json_value(&self) -> Value;
}

impl GradientJson for GradientInfo {
    fn to_json_value(&self) -> Value {
        Some(self.clone()).to_json()
    }
}
