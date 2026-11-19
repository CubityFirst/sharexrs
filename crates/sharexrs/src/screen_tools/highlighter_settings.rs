//! `MouseHighlighterSettingsControl` (`ShareX.Tools/Tools/MouseHighlighter`):
//! the highlighter's options as the Mouse highlighter window and the
//! recorder bar's Mouse highlighter panel show them. A `StackPanel
//! Spacing="12"`: Mode, the three buttons' `EnabledColorRow`s (with their
//! release crosshairs in Ripple mode), then Ripple's numbers and Follow
//! cursor, or Circle's always colour, its help and numbers, then Auto
//! activate. Rows are `ColumnDefinitions="*,180" ColumnSpacing="12"`.
//!
//! The colour buttons' `Flyout` holds a `ColorView` (alpha on); egui's HSV
//! picker stands in for it (`controls::color_popup_with_alpha`), as in the
//! other ported colour flyouts.

use egui::{Color32, Id, Rect, Ui, pos2, vec2};
use sharex_strings::tools as t;
use sharex_tools::color::Color;
use sharex_tools::mouse_highlighter::{Mode, MouseHighlighterOptions};
use sharex_ui::controls::{self, ComboBox, ComboItem, DecimalUpDown};
use sharex_ui::theme::{Thickness, palette};
use sharex_ui::widgets::font;

/// The help text under Circle's always colour: Normal 13px, grey.
fn help_lines(ui: &Ui, width: f32) -> usize {
    controls::wrap_lines(ui, t::MouseHighlighter_AlwaysColorHelp(), &font(13.0, 400), width).len().max(1)
}

/// A wrapped `TextBlock` (Normal 13px): its lines a font line height apart
/// (unrounded, as Avalonia's text layout places them).
pub fn wrapped(ui: &Ui, at: egui::Pos2, width: f32, text: &str, color: Color32) {
    let f = font(13.0, 400);
    let lh = ui.fonts_mut(|fo| fo.row_height(&f));
    for (k, line) in controls::wrap_lines(ui, text, &f, width).iter().enumerate() {
        let g = ui.painter().layout_no_wrap(line.clone(), f.clone(), color);
        ui.painter().galley(pos2(at.x, (at.y + k as f32 * lh).round()), g, color);
    }
}

/// A wrapped text's height: its lines at the exact line height, rounded up once.
fn wrapped_height(ui: &Ui, lines: usize) -> f32 {
    let lh = ui.fonts_mut(|fo| fo.row_height(&font(13.0, 400)));
    (lines as f32 * lh).ceil()
}

/// A row's control cell: 180 wide at the right.
fn cell(r: Rect) -> Rect {
    Rect::from_min_size(pos2(r.max.x - 180.0, r.min.y), vec2(180.0, r.height()))
}

fn check_box<'a>(text: &'a str, enabled: bool, ui: &Ui) -> controls::CheckBox<'a> {
    controls::CheckBox { min_height: 32.0, enabled, text_font: font(13.0, 400), text_color: Some(palette(ui.ctx()).text), ..controls::CheckBox::new(text) }
}

/// The colour button: a 28x20 swatch with a grey border.
fn color_button(ui: &Ui, r: Rect, id: Id, c: &Color, enabled: bool) -> (bool, Rect) {
    let b = Rect::from_min_size(r.min, vec2(50.0, 40.0));
    let resp = controls::Button::default().show(ui, b, id, "", enabled);
    let sw = Rect::from_center_size(b.center(), vec2(28.0, 20.0));
    let op = if enabled { 1.0 } else { 0.35 };
    let fill = Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a).gamma_multiply(op);
    ui.painter().rect_filled(sw, 0.0, fill);
    ui.painter().rect_stroke(sw, 0.0, egui::Stroke::new(1.0, Color32::GRAY.gamma_multiply(op)), egui::StrokeKind::Inside);
    (resp.clicked(), b)
}

/// The control's state: which colour's flyout is open (0–2: primary,
/// secondary, middle; 3: always).
#[derive(Default)]
pub struct SettingsControl {
    picking: Option<(usize, Rect, bool)>,
    id: &'static str,
}

impl SettingsControl {
    /// `id` keeps two controls' widgets apart.
    pub fn new(id: &'static str) -> Self {
        Self { picking: None, id }
    }

    /// The control's height at `width`.
    pub fn height(ui: &Ui, o: &MouseHighlighterOptions, width: f32) -> f32 {
        let ripple = o.mode == Mode::Ripple;
        let mut h = 40.0 + 12.0 + 3.0 * (40.0 + 12.0);
        if ripple {
            h += 3.0 * (32.0 + 12.0) + 3.0 * 50.0 + 32.0 + 12.0;
        } else {
            h += 40.0 + 10.0 + wrapped_height(ui, help_lines(ui, width)) + 10.0 + 3.0 * 50.0 + 2.0;
        }
        h + 32.0
    }

    /// The width it asks for when nothing constrains it (`SizeToContent`):
    /// its widest check box, label and cell, or unwrapped help.
    pub fn desired_width(ui: &Ui, o: &MouseHighlighterOptions) -> f32 {
        let ripple = o.mode == Mode::Ripple;
        let f = font(13.0, 400);
        let mut labels = vec![t::MouseHighlighter_Mode()];
        let mut checks = vec![t::MouseHighlighter_PrimaryColor(), t::MouseHighlighter_SecondaryColor(), t::MouseHighlighter_MiddleColor(), t::MouseHighlighter_AutoActivate()];
        let mut help = 0.0f32;
        if ripple {
            checks.extend([
                t::MouseHighlighter_PrimaryReleaseCrosshairs(),
                t::MouseHighlighter_ReleaseCrosshairs(),
                t::MouseHighlighter_MiddleReleaseCrosshairs(),
                t::MouseHighlighter_FollowCursor(),
            ]);
            labels.extend([t::MouseHighlighter_RippleSize(), t::MouseHighlighter_RippleIntensity(), t::MouseHighlighter_RippleDuration()]);
        } else {
            labels.extend([t::MouseHighlighter_AlwaysColor(), t::MouseHighlighter_Radius(), t::MouseHighlighter_FadeDelay(), t::MouseHighlighter_FadeDuration()]);
            help = sharex_ui::widgets::text_width(ui, t::MouseHighlighter_AlwaysColorHelp(), f.clone());
        }
        let label_w = labels.iter().map(|l| sharex_ui::widgets::text_width(ui, l, f.clone())).fold(0.0, f32::max);
        // An `EnabledColorRow`'s check box sits in the label column too.
        let check_w = checks.iter().map(|c| check_box(c, true, ui).width(ui)).fold(0.0, f32::max);
        (label_w.max(check_w) + 12.0 + 180.0).max(help)
    }

    /// Draws it at `(x0, y)`, `w` wide, editing `o`; true when an option
    /// changed (`Changed()`).
    pub fn ui(&mut self, ui: &mut Ui, x0: f32, mut y: f32, w: f32, o: &mut MouseHighlighterOptions) -> bool {
        let p = palette(ui.ctx());
        let id = |s: &str| Id::new((self.id, s.to_owned()));
        let mut changed = false;
        let ripple = o.mode == Mode::Ripple;
        // Mode.
        let row = Rect::from_min_size(pos2(x0, y), vec2(w, 40.0));
        controls::label(ui, row, t::MouseHighlighter_Mode(), font(13.0, 400), p.text);
        let items = [ComboItem::text(t::MouseHighlighter_Circle()), ComboItem::text(t::MouseHighlighter_Ripple())];
        let combo = ComboBox { padding: Thickness::hv(8.0, 6.0), enabled: true };
        if let Some(i) = combo.show(ui, cell(row), id("mode"), &items, Some(usize::from(ripple))) {
            o.mode = if i == 0 { Mode::Circle } else { Mode::Ripple };
            changed = true;
        }
        y += 40.0 + 12.0;
        let check = |ui: &Ui, r: Rect, key: &str, text: &str, value: &mut bool, enabled: bool| -> bool {
            let cb = check_box(text, enabled, ui);
            let cw = cb.width(ui);
            cb.show(ui, Rect::from_min_size(r.min, vec2(cw, 32.0)), id(key), value).changed()
        };
        for k in 0..3 {
            let (label, cross_label) = [
                (t::MouseHighlighter_PrimaryColor(), t::MouseHighlighter_PrimaryReleaseCrosshairs()),
                (t::MouseHighlighter_SecondaryColor(), t::MouseHighlighter_ReleaseCrosshairs()),
                (t::MouseHighlighter_MiddleColor(), t::MouseHighlighter_MiddleReleaseCrosshairs()),
            ][k];
            let row = Rect::from_min_size(pos2(x0, y), vec2(w, 40.0));
            let (on, cross, color) = match k {
                0 => (&mut o.highlight_primary_clicks, &mut o.show_primary_release_crosshairs, &o.primary_color),
                1 => (&mut o.highlight_secondary_clicks, &mut o.show_secondary_release_crosshairs, &o.secondary_color),
                _ => (&mut o.highlight_middle_clicks, &mut o.show_middle_release_crosshairs, &o.middle_color),
            };
            let color = color.clone();
            changed |= check(ui, Rect::from_min_size(pos2(x0, row.min.y + 4.0), vec2(w - 192.0, 32.0)), &format!("on{k}"), label, on, true);
            let (clicked, button) = color_button(ui, cell(row), id(&format!("c{k}")), &color, *on);
            if clicked {
                self.picking = Some((k, button, true));
            }
            y += 40.0 + 12.0;
            if ripple {
                let enabled = *on;
                changed |= check(ui, Rect::from_min_size(pos2(x0, y), vec2(w, 32.0)), &format!("x{k}"), cross_label, cross, enabled);
                y += 32.0 + 12.0;
            }
        }
        let number = |ui: &mut Ui, y: f32, key: &str, label: &str, v: &mut f64, (min, max, step, dec): (f64, f64, f64, usize)| -> bool {
            let row = Rect::from_min_size(pos2(x0, y), vec2(w, 40.0));
            controls::label(ui, row, label, font(13.0, 400), p.text);
            DecimalUpDown { minimum: min, maximum: max, increment: step, decimals: dec, enabled: true }.show(ui, cell(row), id(key), v)
        };
        if ripple {
            let mut size = o.ripple_size as f64;
            if number(ui, y, "size", t::MouseHighlighter_RippleSize(), &mut size, (10.0, 300.0, 1.0, 0)) {
                o.ripple_size = size as i32;
                changed = true;
            }
            y += 50.0;
            if number(ui, y, "intensity", t::MouseHighlighter_RippleIntensity(), &mut o.ripple_intensity, (0.15, 1.35, 0.05, 2)) {
                changed = true;
            }
            y += 50.0;
            let mut d = o.ripple_duration as f64;
            if number(ui, y, "duration", t::MouseHighlighter_RippleDuration(), &mut d, (60.0, 2000.0, 10.0, 0)) {
                o.ripple_duration = d as i32;
                changed = true;
            }
            y += 50.0;
            changed |= check(ui, Rect::from_min_size(pos2(x0, y), vec2(w, 32.0)), "follow", t::MouseHighlighter_FollowCursor(), &mut o.follow_cursor_while_held, true);
            y += 32.0 + 12.0;
        } else {
            let row = Rect::from_min_size(pos2(x0, y), vec2(w, 40.0));
            controls::label(ui, row, t::MouseHighlighter_AlwaysColor(), font(13.0, 400), p.text);
            let (clicked, button) = color_button(ui, cell(row), id("always"), &o.always_color.clone(), true);
            if clicked {
                self.picking = Some((3, button, true));
            }
            y += 40.0 + 10.0;
            wrapped(ui, pos2(x0, y), w, t::MouseHighlighter_AlwaysColorHelp(), Color32::GRAY);
            y += wrapped_height(ui, help_lines(ui, w)) + 10.0;
            for (k, (label, v, range)) in [
                (t::MouseHighlighter_Radius(), &mut o.radius, (5.0, 500.0, 1.0, 0)),
                (t::MouseHighlighter_FadeDelay(), &mut o.fade_delay, (0.0, 10000.0, 10.0, 0)),
                (t::MouseHighlighter_FadeDuration(), &mut o.fade_duration, (0.0, 10000.0, 10.0, 0)),
            ]
            .into_iter()
            .enumerate()
            {
                let mut f = *v as f64;
                if number(ui, y, &format!("n{k}"), label, &mut f, range) {
                    *v = f as i32;
                    changed = true;
                }
                y += 50.0;
            }
            y += 2.0;
        }
        changed |= check(ui, Rect::from_min_size(pos2(x0, y), vec2(w, 32.0)), "auto", t::MouseHighlighter_AutoActivate(), &mut o.auto_activate, true);
        // The open colour flyout.
        if let Some((k, anchor, just_opened)) = self.picking {
            let target = match k {
                0 => &mut o.primary_color,
                1 => &mut o.secondary_color,
                2 => &mut o.middle_color,
                _ => &mut o.always_color,
            };
            let mut c32 = Color32::from_rgba_unmultiplied(target.r, target.g, target.b, target.a);
            let (picked, keep) = controls::color_popup_with_alpha(ui, id(&format!("flyout{k}")), anchor, &mut c32, just_opened, true);
            if picked {
                let [r, g, b, a] = c32.to_srgba_unmultiplied();
                *target = Color::argb(a, r, g, b);
                changed = true;
            }
            self.picking = keep.then_some((k, anchor, false));
        }
        changed
    }

    /// Where its popups are (the mode's dropdown, a colour flyout), for a
    /// window that only takes the pointer where it draws.
    pub fn popup_rects(&self, ctx: &egui::Context) -> Vec<Rect> {
        let mode = Id::new((self.id, "mode".to_owned()));
        let mut ids = Vec::new();
        if ComboBox::is_open(ctx, mode) {
            ids.push(mode.with("dropdown"));
        }
        if let Some((k, _, _)) = self.picking {
            ids.push(Id::new((self.id, format!("flyout{k}"))).with("color-popup"));
        }
        ids.into_iter().filter_map(|id| ctx.memory(|m| m.area_rect(id))).collect()
    }
}
