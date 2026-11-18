//! `ShareX.HelpersLib.PrintWindow` ("Print options": margin, auto rotate,
//! auto scale, allow enlarge, centre; Preview..., Print..., Cancel) and
//! `PrintPreviewWindow` (the page as it prints, Print, Close). The settings
//! are ApplicationConfig's `PrintSettings`, changed as they're edited.

use egui::{Color32, Id, Rect, Ui, Vec2, pos2, vec2};
use image::RgbaImage;
use sharex_strings::helpers_lib as hs;
use sharex_ui::controls::{Button, CheckBox, NumericUpDown, content_font};
use sharex_ui::keys::app_icons as icon;
use sharex_ui::theme::{Thickness, palette};
use sharex_ui::{border, icons, widgets};

use super::{Cx, Dialog, Effect, Spec};
use crate::print::{self, PrintSettings};

/// The heading and description stacked (`Spacing="1"`).
fn hh_dh(ui: &Ui) -> f32 {
    widgets::line_height(ui, &widgets::font(19.0, 600)) + 1.0 + widgets::line_height(ui, &widgets::font(13.0, 400))
}

pub struct PrintWindow {
    image: RgbaImage,
    settings: Option<PrintSettings>,
    preview_only: bool,
}

impl PrintWindow {
    pub fn new(image: RgbaImage, preview_only: bool) -> Self {
        Self { image, settings: None, preview_only }
    }
}

const WIDTH: f32 = 470.0;
const HEADER: f32 = 69.0;
/// The options panel: border, padding 12, the margin row (its
/// `NumericUpDown` 40 high, 3 below), then four 34px check rows 6 apart.
const PANEL: f32 = 1.0 + 12.0 + 40.0 + 3.0 + 6.0 + 4.0 * 34.0 + 3.0 * 6.0 + 12.0 + 1.0;

/// `Button.compact` here: 36 high, at least 92 wide, `Padding="14,5"`.
fn compact(primary: bool) -> Button {
    if primary { Button { padding: Thickness::hv(14.0, 5.0), ..Button::primary(Thickness::hv(14.0, 5.0), 4.0) } } else { Button::compact(Thickness::hv(14.0, 5.0)) }
}

/// Prints on a thread (the dialogs are modal to it, not to the app).
pub fn print_in_background(image: RgbaImage, settings: PrintSettings) {
    std::thread::spawn(move || {
        if let Err(e) = print::print_image(&image, &settings, 0) {
            eprintln!("Print failed: {e}");
        }
    });
}

impl Dialog for PrintWindow {
    fn spec(&self) -> Spec {
        Spec::fit_height(crate::rs_title(hs::PrintWindow_Title()), WIDTH, WIDTH)
    }

    fn content_height(&mut self, ui: &mut Ui) -> f32 {
        let _ = ui;
        // Header, the options panel (margin row + 3 + four check rows,
        // 6 apart, padding 12, border), the buttons.
        HEADER + 14.0 + PANEL + 12.0 + 36.0 + 18.0
    }

    fn ui(&mut self, ui: &mut Ui, cx: &mut Cx) -> Vec<Effect> {
        let p = palette(ui.ctx());
        let s = self.settings.get_or_insert_with(|| PrintSettings::from_json(&cx.settings.PrintSettings)).clone();
        let mut s2 = s.clone();
        let full = ui.max_rect();
        ui.painter().rect_filled(full, 0.0, p.background_main);
        // The header: the printer icon in a 40px panel square, the heading
        // and its description.
        let head = Rect::from_min_size(full.min, vec2(full.width(), HEADER));
        ui.painter().rect_filled(head, 0.0, p.background_toolbar);
        let stack = hh_dh(ui);
        let ib = Rect::from_min_size(pos2(head.min.x + 18.0, head.min.y + 12.0 + ((stack - 40.0) / 2.0).floor()), Vec2::splat(40.0));
        border::border(ui.painter(), ib, 6.0, 0.0, Some(border::Brush::Solid(p.background_panel)), None);
        icons::paint(ui.painter(), Rect::from_center_size(ib.center(), Vec2::splat(20.0)), icon::Printer, p.accent_icon);
        let (hf, df) = (widgets::font(19.0, 600), widgets::font(13.0, 400));
        let (hh, dh) = (widgets::line_height(ui, &hf), widgets::line_height(ui, &df));
        let ty = head.min.y + 12.0;
        let tx = ib.max.x + 10.0;
        ui.painter().galley(pos2(tx, ty), widgets::galley(ui, hs::PrintWindow_Heading(), hf, p.accent_text, f32::INFINITY), p.accent_text);
        ui.painter().galley(pos2(tx, ty + hh + 1.0), widgets::galley(ui, hs::PrintWindow_Description(), df.clone(), p.text_secondary, f32::INFINITY), p.text_secondary);
        // The options panel.
        let panel = Rect::from_min_max(pos2(full.min.x + 18.0, head.max.y + 14.0), pos2(full.max.x - 18.0, head.max.y + 14.0 + PANEL));
        border::border(ui.painter(), panel, 6.0, 1.0, Some(border::Brush::Solid(p.background_panel)), Some(p.border));
        let inner = Rect::from_min_max(pos2(panel.min.x + 15.0, panel.min.y + 13.0), pos2(panel.max.x - 15.0, panel.max.y - 13.0));
        let _ = Color32::TRANSPARENT;
        let mut y = inner.min.y;
        let row = Rect::from_min_size(pos2(inner.min.x, y), vec2(inner.width(), 40.0));
        let lw = widgets::text_width(ui, hs::PrintWindow_Margin(), df.clone()).ceil();
        let cy = widgets::center_y(row, dh);
        ui.painter().galley(pos2(row.min.x, cy), widgets::galley(ui, hs::PrintWindow_Margin(), df.clone(), p.text_secondary, f32::INFINITY), p.text_secondary);
        let nb = Rect::from_min_size(pos2(row.min.x + lw + 8.0, row.min.y), vec2(130.0, 40.0));
        let mut m = s.margin as i64;
        if (NumericUpDown { minimum: 0, maximum: 1000, enabled: true }).show(ui, nb, Id::new("print-margin"), &mut m) {
            s2.margin = m as i32;
        }
        ui.painter().galley(pos2(nb.max.x + 8.0, cy), widgets::galley(ui, hs::PrintWindow_Pixels(), df.clone(), p.text_secondary, f32::INFINITY), p.text_secondary);
        y += 40.0 + 3.0 + 6.0;
        let checks: [(&str, f32, bool, bool); 4] = [
            (hs::PrintWindow_Auto_rotate_image(), 0.0, s.auto_rotate, true),
            (hs::PrintWindow_Auto_scale_image(), 0.0, s.auto_scale, true),
            (hs::PrintWindow_Allow_image_to_enlarge(), 24.0, s.allow_enlarge, s.auto_scale),
            (hs::PrintWindow_Center_image_position(), 24.0, s.center, s.auto_scale),
        ];
        for (k, (text, indent, value, enabled)) in checks.into_iter().enumerate() {
            let r = Rect::from_min_size(pos2(inner.min.x + indent, y + 1.0), vec2(inner.width() - indent, 32.0));
            let mut v = value;
            let cb = CheckBox { text_font: content_font(13.0, 400), enabled, ..CheckBox::new(text) };
            if cb.show(ui, r, Id::new(("print-check", k)), &mut v).changed() {
                match k {
                    0 => s2.auto_rotate = v,
                    1 => s2.auto_scale = v,
                    2 => s2.allow_enlarge = v,
                    _ => s2.center = v,
                }
            }
            y += 34.0 + 6.0;
        }
        if s2 != s {
            s2.write(&mut cx.settings.PrintSettings);
            self.settings = Some(s2.clone());
        }
        // The buttons: Preview..., then Print(...) and Cancel at the right.
        let by = panel.max.y + 12.0;
        let mut effects = Vec::new();
        let preview_t = hs::PrintWindow_Preview();
        let pw = compact(false).width(ui, preview_t).max(92.0);
        if compact(false).show(ui, Rect::from_min_size(pos2(full.min.x + 18.0, by), vec2(pw, 36.0)), Id::new("print-preview"), preview_t, true).clicked() {
            effects.push(Effect::OpenDialog(Box::new(PrintPreview::new(self.image.clone(), s2.clone()))));
        }
        let cancel_t = hs::MyMessageBox_MyMessageBox_Cancel();
        let cw = compact(false).width(ui, cancel_t).max(92.0);
        let cancel = Rect::from_min_size(pos2(full.max.x - 18.0 - cw, by), vec2(cw, 36.0));
        let print_t = format!("{}{}", hs::PrintForm_LoadSettings_Print(), if s2.show_print_dialog { "..." } else { "" });
        let prw = compact(true).width(ui, &print_t).max(92.0);
        let print_r = Rect::from_min_size(pos2(cancel.min.x - 7.0 - prw, by), vec2(prw, 36.0));
        let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
        let clicked = compact(true).show(ui, print_r, Id::new("print-print"), &print_t, !self.preview_only).clicked();
        if (clicked || enter) && !self.preview_only {
            print_in_background(self.image.clone(), s2.clone());
            effects.push(Effect::Close);
        }
        if compact(false).show(ui, cancel, Id::new("print-cancel"), cancel_t, true).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            effects.push(Effect::Close);
        }
        effects
    }
}

/// `PrintPreviewWindow`: the page (`RenderPreviewPage`: the default
/// printer's page, or Letter) over dim grey, Print and Close.
pub struct PrintPreview {
    image: RgbaImage,
    settings: PrintSettings,
    tex: Option<egui::TextureHandle>,
}

impl PrintPreview {
    pub fn new(image: RgbaImage, settings: PrintSettings) -> Self {
        Self { image, settings, tex: None }
    }
}

impl Dialog for PrintPreview {
    fn spec(&self) -> Spec {
        Spec::sized(crate::rs_title(hs::PrintWindow_Preview().trim_end_matches('.')), 850.0, 900.0, 400.0, 300.0)
    }

    fn ui(&mut self, ui: &mut Ui, _cx: &mut Cx) -> Vec<Effect> {
        let p = palette(ui.ctx());
        let dark = ui.style().visuals.dark_mode;
        let full = ui.max_rect();
        // The window's own (Fluent) background, the dim grey `Border`.
        ui.painter().rect_filled(full, 0.0, if dark { Color32::BLACK } else { Color32::WHITE });
        let mut effects = Vec::new();
        let font = content_font(13.0, 400);
        let mut x = full.min.x + 8.0;
        let top = full.min.y + 8.0;
        let b = Button::default();
        for (k, text) in ["←", "→"].iter().enumerate() {
            let w = b.width(ui, text);
            if k == 1 {
                let g = widgets::galley(ui, "1", font.clone(), p.text, f32::INFINITY);
                let nr = Rect::from_min_size(pos2(x, top), vec2(widgets::text_width(ui, "1", font.clone()), 40.0));
                ui.painter().galley(pos2(nr.min.x, widgets::center_y(nr, widgets::line_height(ui, &font))), g, p.text);
                x = nr.max.x + 8.0;
            }
            // One page: neither arrow is enabled.
            b.show(ui, Rect::from_min_size(pos2(x, top), vec2(w, 40.0)), Id::new(("preview-arrow", k)), text, false);
            x += w + 8.0;
        }
        let print_t = hs::PrintForm_LoadSettings_Print();
        let w = b.width(ui, print_t);
        if b.show(ui, Rect::from_min_size(pos2(x, top), vec2(w, 40.0)), Id::new("preview-print"), print_t, true).clicked() {
            print_in_background(self.image.clone(), self.settings.clone());
            effects.push(Effect::Close);
        }
        x += w + 8.0;
        let close_t = hs::OutputBoxWindow_Close();
        let w = b.width(ui, close_t);
        if b.show(ui, Rect::from_min_size(pos2(x, top), vec2(w, 40.0)), Id::new("preview-close"), close_t, true).clicked() {
            effects.push(Effect::Close);
        }
        let area = Rect::from_min_max(pos2(full.min.x, top + 40.0 + 8.0), full.max);
        ui.painter().rect_filled(area, 0.0, Color32::from_rgb(0x69, 0x69, 0x69));
        let inner = area.shrink(12.0);
        let tex = self.tex.get_or_insert_with(|| {
            let page = print::render_image_page(&self.image, print::default_page_size(&self.settings), &self.settings);
            let img = egui::ColorImage::from_rgba_unmultiplied([page.width() as usize, page.height() as usize], page.as_raw());
            ui.ctx().load_texture("print-preview", img, egui::TextureOptions::LINEAR)
        });
        // `Stretch.Uniform`, centred.
        let s = tex.size_vec2();
        let k = (inner.width() / s.x).min(inner.height() / s.y);
        let r = Rect::from_center_size(inner.center(), s * k);
        ui.painter().image(tex.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        effects
    }
}
