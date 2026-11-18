//! `NewImageDialogView` with its `NewImageDialogViewModel`: the width and
//! height (1 to 10000, 800x600 at first) and a transparent or solid colour
//! background (white at first).

use egui::{Color32, Id, Rect, Ui, Vec2, pos2, vec2};
use sharex_capture::editor_popups::{ColorDropdown, PopupResult};
use sharex_strings::image_editor as ie;
use sharex_ui::controls::{NumericUpDown, RadioButton};
use sharex_ui::{Palette, widgets};

use super::modal::{self, text_font};

const WIDTH: f32 = 340.0;

pub struct NewImage {
    pub width: i64,
    pub height: i64,
    pub transparent: bool,
    pub color: Color32,
    pub dropdown: ColorDropdown,
    /// The dropdown button, as last drawn.
    pub button: Rect,
}

impl Default for NewImage {
    fn default() -> Self {
        Self { width: 800, height: 600, transparent: true, color: Color32::WHITE, dropdown: ColorDropdown::default(), button: Rect::NOTHING }
    }
}

pub enum Outcome {
    Ok,
    Cancel,
}

impl NewImage {
    pub fn size(&self, ui: &Ui) -> Vec2 {
        let content = 40.0 + 8.0 + 40.0 + 12.0 + widgets::line_height(ui, &text_font(13.0, 400)) + 8.0 + 40.0;
        vec2(WIDTH, 2.0 + modal::title_height(ui, false) + 20.0 + content + 20.0 + modal::BUTTON_BAR)
    }

    /// Whether `p` is on the dialog's own popup (the colour dropdown's).
    pub fn popup_contains(&self, p: egui::Pos2) -> bool {
        self.dropdown.contains(p)
    }

    pub fn show(&mut self, ui: &mut Ui, p: &Palette, rect: Rect) -> Option<Outcome> {
        let mut out = None;
        let inner = modal::frame(ui, p, rect, modal::SHADOW_SMALL);
        let (title, _) = modal::title_bar(ui, p, inner, ie::NewImageDialogView_Title(), p.text_secondary, None);
        let content = Rect::from_min_max(pos2(inner.min.x + 20.0, title.max.y + 20.0), pos2(inner.max.x - 20.0, inner.max.y - modal::BUTTON_BAR - 20.0));
        let font = text_font(13.0, 400);
        let lh = widgets::line_height(ui, &font);
        let labels = [ie::NewImageDialogView_Width(), ie::NewImageDialogView_Height()];
        let label_w = labels.iter().map(|l| widgets::text_width(ui, l, font.clone())).fold(0.0, f32::max).ceil();
        let pixels = ie::NewImageDialogView_Pixels();
        let pixels_w = widgets::text_width(ui, pixels, font.clone()).ceil();
        let label = |ui: &Ui, r: Rect, s: &str| {
            let g = widgets::galley(ui, s, font.clone(), p.text_secondary, f32::INFINITY);
            ui.painter().galley(pos2(r.min.x, widgets::center_y(r, lh)), g, p.text_secondary);
        };
        for (row, text) in labels.iter().enumerate() {
            let y = content.min.y + row as f32 * 48.0;
            label(ui, Rect::from_min_size(pos2(content.min.x, y), vec2(label_w, 40.0)), text);
            let nr = Rect::from_min_max(pos2(content.min.x + label_w + 8.0, y), pos2(content.max.x - pixels_w - 8.0, y + 40.0));
            let v = if row == 0 { &mut self.width } else { &mut self.height };
            NumericUpDown { minimum: 1, maximum: 10000, enabled: true }.show(ui, nr, Id::new(("new-image-size", row)), v);
            label(ui, Rect::from_min_size(pos2(content.max.x - pixels_w, y), vec2(pixels_w, 40.0)), pixels);
        }
        let mut y = content.min.y + 88.0 + 12.0;
        label(ui, Rect::from_min_size(pos2(content.min.x, y), vec2(content.width(), lh)), ie::NewImageDialogView_Background());
        y += lh + 8.0;
        let row = Rect::from_min_size(pos2(content.min.x, y), vec2(content.width(), 40.0));
        let radio = |text| RadioButton { text, text_font: font.clone(), text_color: None, padding_left: 8.0, enabled: true };
        let t = radio(ie::NewImageDialogView_Transparent());
        let tw = 20.0 + 8.0 + widgets::text_width(ui, t.text, font.clone()).ceil();
        if t.show(ui, Rect::from_min_size(row.min, vec2(tw, 40.0)), Id::new("new-image-transparent"), self.transparent).clicked() {
            self.transparent = true;
        }
        let s = radio(ie::NewImageDialogView_SolidColor());
        let sx = row.min.x + tw + 16.0;
        let sw = 20.0 + 8.0 + widgets::text_width(ui, s.text, font.clone()).ceil();
        if s.show(ui, Rect::from_min_size(pos2(sx, row.min.y), vec2(sw, 40.0)), Id::new("new-image-solid"), !self.transparent).clicked() {
            self.transparent = false;
        }
        // `ColorPickerDropdown` 72x40, disabled with a transparent
        // background: the whole button at 35%.
        let br = Rect::from_min_size(pos2(sx + sw + 8.0, row.min.y), vec2(72.0, 40.0));
        self.button = br;
        let enabled = !self.transparent;
        let resp = ui.interact(br, Id::new("new-image-color"), if enabled { egui::Sense::click() } else { egui::Sense::hover() });
        let mut painter = ui.painter().clone();
        if !enabled {
            painter.set_opacity(0.35);
            painter.rect_filled(br, 4.0, p.control_background);
            sharex_capture::region::options_row::color_dropdown_content(ui, &painter, p, br, self.color, false);
        } else {
            let hovered = resp.hovered();
            sharex_capture::region::options_row::color_dropdown_button(ui, &painter, p, br, self.color, hovered, self.dropdown.open);
        }
        if enabled && resp.clicked() {
            self.dropdown.toggle(self.color);
        }
        if let Some(i) = modal::button_bar(ui, p, inner, Id::new("new-image-buttons"), &[ie::NewImageDialogView_OK(), ie::NewImageDialogView_Cancel()]) {
            out = Some(if i == 0 { Outcome::Ok } else { Outcome::Cancel });
        }
        out
    }

    /// The colour dropdown's popup, over everything (`bounds`: the view).
    pub fn show_popup(&mut self, ui: &Ui, p: &Palette, bounds: Rect) -> PopupResult {
        let mut c = self.color;
        let r = self.dropdown.show(ui, p, self.button, bounds, "new-image", &mut c);
        self.color = c;
        r
    }
}
