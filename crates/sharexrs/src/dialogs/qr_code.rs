//! `ShareX.Tools/Tools/QRCode` (`QRCodeWindow`, `QRCodeViewModel`) with
//! ShareX's services (`TaskHelpers.ShowQRCodeWindow`): the text, its QR
//! code as ZXing makes it (`TaskHelpers.GenerateQRCode`), copy, save,
//! upload, and scanning the screen or an image file for codes.

use egui::{Rect, Vec2, pos2, vec2};
use image::RgbaImage;
use qrcode::bits::Bits;
use qrcode::{EcLevel, QrCode, Version};
use sharex_strings::tools as s;
use sharex_ui::controls::{Button, DecimalUpDown, TextBox};
use sharex_ui::icons;
use sharex_ui::keys::app_icons as icon;
use sharex_ui::theme::palette;
use sharex_ui::widgets::{center_y, font, line_height};

use super::layout;
use super::{Cx, Dialog, Effect, Spec, UploadRequest};

/// `QRCodeViewModel.MaximumContentBytes` (`CheckQRCodeContent`).
pub const MAXIMUM_CONTENT_BYTES: usize = 2952;

/// `TaskHelpers.CheckQRCodeContent`.
pub fn check_content(text: &str) -> bool {
    !text.is_empty() && text.len() <= MAXIMUM_CONTENT_BYTES
}

/// ZXing's `Encoder.chooseMode` for UTF-8 content: numeric when it's all
/// digits, alphanumeric when it's all of the 45 characters, else bytes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    Numeric,
    Alphanumeric,
    Byte,
}

fn choose_mode(text: &str) -> Mode {
    const ALNUM: &str = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ $%*+-./:";
    let (mut numeric, mut alnum) = (false, false);
    for c in text.chars() {
        if c.is_ascii_digit() {
            numeric = true;
        } else if ALNUM.contains(c) {
            alnum = true;
        } else {
            return Mode::Byte;
        }
    }
    if alnum {
        Mode::Alphanumeric
    } else if numeric {
        Mode::Numeric
    } else {
        Mode::Byte
    }
}

fn bits_for(text: &str, mode: Mode, version: i16) -> Option<Bits> {
    let mut bits = Bits::new(Version::Normal(version));
    let data = text.as_bytes();
    match mode {
        Mode::Numeric => bits.push_numeric_data(data).ok()?,
        Mode::Alphanumeric => bits.push_alphanumeric_data(data).ok()?,
        Mode::Byte => {
            // `CharacterSet = "UTF-8"`: ZXing writes the UTF-8 ECI (26)
            // before byte data.
            bits.push_eci_designator(26).ok()?;
            bits.push_byte_data(data).ok()?;
        }
    }
    bits.push_terminator(EcLevel::L).ok()?;
    Some(bits)
}

/// The modules of `text`'s QR code at ZXing's defaults (error correction
/// L, the smallest version it fits), without the quiet zone.
pub fn qr_matrix(text: &str) -> Option<(usize, Vec<bool>)> {
    let mode = choose_mode(text);
    let bits = (1..=40).find_map(|v| bits_for(text, mode, v))?;
    let code = QrCode::with_bits(bits, EcLevel::L).ok()?;
    let width = code.width();
    Some((width, code.to_colors().into_iter().map(|c| c == qrcode::Color::Dark).collect()))
}

/// `TaskHelpers.GenerateQRCode(text, size)`: `BarcodeWriterGeneric` with
/// `Margin = 1`: the modules scaled by the largest whole number that fits
/// `size` (with a module of quiet zone each side), centred, black on white.
pub fn generate(text: &str, size: u32) -> Option<RgbaImage> {
    if !check_content(text) {
        return None;
    }
    let (width, modules) = qr_matrix(text)?;
    let input = width as u32;
    let qr_width = input + 2;
    let output = size.max(qr_width);
    let multiple = (output / qr_width).max(1);
    let left = (output - input * multiple) / 2;
    let mut image = RgbaImage::from_pixel(output, output, image::Rgba([255, 255, 255, 255]));
    for y in 0..input {
        for x in 0..input {
            if modules[(y * input + x) as usize] {
                for dy in 0..multiple {
                    for dx in 0..multiple {
                        image.put_pixel(left + x * multiple + dx, left + y * multiple + dy, image::Rgba([0, 0, 0, 255]));
                    }
                }
            }
        }
    }
    Some(image)
}

/// `BarcodeScan`: the QR codes' texts in an image.
pub fn scan(image: &RgbaImage) -> Vec<String> {
    let (w, h) = image.dimensions();
    let mut prepared = rqrr::PreparedImage::prepare_from_greyscale(w as usize, h as usize, |x, y| {
        let p = image.get_pixel(x as u32, y as u32);
        ((p[0] as u32 * 299 + p[1] as u32 * 587 + p[2] as u32 * 114) / 1000) as u8
    });
    let mut out: Vec<String> = prepared.detect_grids().into_iter().filter_map(|g| g.decode().ok().map(|(_, t)| t)).filter(|t| !t.is_empty()).collect();
    if out.is_empty() {
        // `TryInverted`.
        let mut inverted = rqrr::PreparedImage::prepare_from_greyscale(w as usize, h as usize, |x, y| {
            let p = image.get_pixel(x as u32, y as u32);
            255 - ((p[0] as u32 * 299 + p[1] as u32 * 587 + p[2] as u32 * 114) / 1000) as u8
        });
        out = inverted.detect_grids().into_iter().filter_map(|g| g.decode().ok().map(|(_, t)| t)).filter(|t| !t.is_empty()).collect();
    }
    out
}

/// `GetSuggestedFileName`: the first 80 characters with invalid ones as
/// `_`, trimmed, `.png`.
fn suggested_file_name(text: &str) -> String {
    const INVALID: &[char] = &['"', '<', '>', '|', ':', '*', '?', '\\', '/'];
    let name: String = text.chars().take(80).map(|c| if INVALID.contains(&c) || (c as u32) < 32 { '_' } else { c }).collect();
    let name = name.trim();
    if name.is_empty() { "qr-code.png".into() } else { format!("{name}.png") }
}

pub struct QrCodeWindow {
    pub text: String,
    /// `QRCodeSize` (0: fit the preview).
    pub size: f64,
    available: u32,
    preview: Option<(String, u32, RgbaImage)>,
    texture: Option<egui::TextureHandle>,
    pub status: String,
}

impl QrCodeWindow {
    /// `TaskHelpers.OpenQRCode(text)`: the clipboard's text when there's none.
    pub fn new(text: Option<String>) -> Self {
        let text = text.or_else(|| sharex_capture::clipboard::get_text().filter(|t| check_content(t))).unwrap_or_default();
        Self { text, size: 0.0, available: 400, preview: None, texture: None, status: String::new() }
    }

    /// `QRCodeWindowOptions { InitialImageFilePath }`: no text, then the
    /// image's codes (`ScanAsync(QRCodeScanMode.ImageFile, path)`).
    pub fn scan_image_file(path: &std::path::Path) -> Self {
        let mut w = Self { text: String::new(), size: 0.0, available: 400, preview: None, texture: None, status: String::new() };
        let results = image::open(path).ok().map(|i| scan(&i.to_rgba8()));
        w.apply_scan(results);
        w
    }

    /// `EffectiveSize`.
    fn effective_size(&self) -> u32 {
        (if self.size > 0.0 { self.size as u32 } else { self.available }).max(64)
    }

    /// `HasValidContent`.
    fn valid(&self) -> bool {
        check_content(&self.text)
    }

    /// `RegenerateAsync`, when the text or size changed.
    fn regenerate(&mut self) {
        let size = self.effective_size();
        if self.preview.as_ref().is_some_and(|(t, s, _)| *t == self.text && *s == size) {
            return;
        }
        self.texture = None;
        if !self.valid() {
            self.preview = None;
            self.status = if self.text.is_empty() { String::new() } else { s::QRCodeViewModel_Content_too_large().to_owned() };
            return;
        }
        match generate(&self.text, size) {
            Some(image) => {
                self.preview = Some((self.text.clone(), size, image));
                self.status.clear();
            }
            None => {
                self.preview = None;
                self.status = s::QRCodeViewModel_Unable_generate().to_owned();
            }
        }
    }

    /// `ApplyScanResultsAsync`.
    fn apply_scan(&mut self, results: Option<Vec<String>>) {
        let results = results.unwrap_or_default();
        self.text = results.join("\r\n\r\n");
        self.status = if results.is_empty() {
            s::QRCodeViewModel_No_QR_code().to_owned()
        } else if results.len() == 1 {
            s::QRCodeViewModel_Decoded_one().replace("{0}", "1")
        } else {
            s::QRCodeViewModel_Decoded_many().replace("{0}", &results.len().to_string())
        };
        crate::sound::play_action_completed();
    }

    fn save(&mut self) {
        let size = self.effective_size();
        let file = rfd::FileDialog::new()
            .set_title(s::QRCodeWindow_Save_QR_code_dialog())
            .set_file_name(suggested_file_name(&self.text))
            .add_filter(s::QRCodeWindow_PNG_image(), &["png"])
            .add_filter(s::QRCodeWindow_JPEG_image(), &["jpg", "jpeg"])
            .add_filter(s::QRCodeWindow_Bitmap_image(), &["bmp"])
            .add_filter(s::QRCodeWindow_SVG_image(), &["svg"])
            .save_file();
        let Some(path) = file else { return };
        let saved = if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("svg")) {
            qr_svg(&self.text, size).map(|svg| std::fs::write(&path, svg).map_err(|e| e.to_string()))
        } else {
            generate(&self.text, size).map(|image| image::DynamicImage::ImageRgba8(image).to_rgb8().save(&path).map_err(|e| e.to_string()))
        };
        match saved {
            Some(Ok(())) => self.status = s::QRCodeViewModel_QRCode_saved().to_owned(),
            Some(Err(e)) => eprintln!("{}: {e}", path.display()),
            None => {}
        }
    }
}

/// `BarcodeWriterSvg` (`Margin` left at ZXing's default, 4 modules): one
/// black rectangle per module on a white square.
fn qr_svg(text: &str, size: u32) -> Option<String> {
    let (width, modules) = qr_matrix(text)?;
    let input = width as u32;
    let qr_width = input + 8;
    let output = size.max(qr_width);
    let multiple = (output / qr_width).max(1);
    let left = (output - input * multiple) / 2;
    let mut svg = format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?><svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{output}\" height=\"{output}\" viewBox=\"0 0 {output} {output}\"><rect x=\"0\" y=\"0\" width=\"{output}\" height=\"{output}\" fill=\"#FFFFFF\"/>"
    );
    for y in 0..input {
        for x in 0..input {
            if modules[(y * input + x) as usize] {
                svg += &format!("<rect x=\"{}\" y=\"{}\" width=\"{multiple}\" height=\"{multiple}\" fill=\"#000000\"/>", left + x * multiple, left + y * multiple);
            }
        }
    }
    svg += "</svg>";
    Some(svg)
}

/// A `Button` with an icon (at the button's 13px font size) and a
/// `TextBlock` (`Spacing="8"`).
#[allow(clippy::too_many_arguments)]
fn icon_text_button(ui: &egui::Ui, x: f32, y: f32, id: egui::Id, key: &str, text: &str, enabled: bool, right_align: bool) -> (Rect, bool) {
    let p = palette(ui.ctx());
    let f = font(13.0, 400);
    let b = Button::default();
    const ICON: f32 = 13.0;
    let w = 1.0 + 10.0 + ICON + 8.0 + layout::width(ui, text, f.clone()) + 10.0 + 1.0;
    let rect = if right_align { Rect::from_min_size(pos2(x - w, y), vec2(w, 40.0)) } else { Rect::from_min_size(pos2(x, y), vec2(w, 40.0)) };
    let r = b.show(ui, rect, id, "", enabled);
    let color = if !enabled {
        sharex_ui::border::group_content(sharex_ui::theme::fluent(ui.ctx()).base_medium_low, p.control_background, 0.35, p.background_panel)
    } else if r.hovered() {
        sharex_ui::theme::fluent(ui.ctx()).base_high
    } else {
        p.text
    };
    let inner = rect.shrink2(vec2(11.0, 7.0));
    icons::paint(ui.painter(), Rect::from_min_size(pos2(inner.min.x, center_y(inner, ICON)), Vec2::splat(ICON)), key, color);
    let y = center_y(inner, line_height(ui, &f));
    layout::text(ui, pos2(inner.min.x + ICON + 8.0, y), text, f, color, f32::INFINITY);
    (rect, r.clicked())
}

impl Dialog for QrCodeWindow {
    fn scanned(&mut self, results: Option<Vec<String>>) {
        self.apply_scan(results);
    }

    fn spec(&self) -> Spec {
        Spec::sized(crate::rs_title(s::QRCodeWindow_ShareX_QR_Code()), 940.0, 610.0, 760.0, 520.0)
    }

    fn ui(&mut self, ui: &mut egui::Ui, _cx: &mut Cx) -> Vec<Effect> {
        let p = palette(ui.ctx());
        let full = layout::background(ui);
        let grid = full.shrink(16.0);
        let mut out = Vec::new();
        // Row 0: the scan buttons.
        let scans = [(icon::QrScan, s::QRCodeWindow_Scan_screen()), (icon::ScanLine, s::QRCodeWindow_Scan_region()), (icon::FileSearch, s::QRCodeWindow_Scan_image())];
        let mut x = grid.min.x;
        let mut scan_clicked = None;
        for (i, (key, text)) in scans.iter().enumerate() {
            let (r, clicked) = icon_text_button(ui, x, grid.min.y, ui.id().with(("scan", i)), key, text, true, false);
            if clicked {
                scan_clicked = Some(i);
            }
            x = r.max.x + 8.0;
        }
        // Row 2: the image buttons, right-aligned.
        let bottom = grid.max.y - 40.0;
        let has = self.preview.is_some();
        let actions = [(icon::Upload, s::QRCodeWindow_Upload()), (icon::Save, s::QRCodeWindow_Save()), (icon::Copy, s::QRCodeWindow_Copy_image())];
        let mut x = grid.max.x;
        let mut action = None;
        for (i, (key, text)) in actions.iter().enumerate() {
            let (r, clicked) = icon_text_button(ui, x, bottom, ui.id().with(("action", i)), key, text, has, true);
            if clicked {
                action = Some(i);
            }
            x = r.min.x - 8.0;
        }
        // Row 1: `ColumnDefinitions="430,*"`.
        let row = Rect::from_min_max(pos2(grid.min.x, grid.min.y + 40.0 + 12.0), pos2(grid.max.x, bottom - 12.0));
        let left = Rect::from_min_max(row.min, pos2(row.min.x + 430.0, row.max.y));
        sharex_ui::border::border(ui.painter(), left, 4.0, 1.0, Some(sharex_ui::border::Brush::Solid(p.background_panel)), Some(p.border));
        let inner = left.shrink(17.0);
        let heading = font(15.0, 600);
        let hh = line_height(ui, &heading);
        layout::text(ui, inner.min, s::QRCodeWindow_Content(), heading, p.text, inner.width());
        let size_row_h = 40.0;
        let text_rect = Rect::from_min_max(pos2(inner.min.x, inner.min.y + hh + 10.0), pos2(inner.max.x, inner.max.y - size_row_h - 10.0));
        let tb = TextBox {
            placeholder: s::QRCodeWindow_Enter_text_or_a_URL_to_encode(),
            multiline: true,
            wrap: true,
            links: Some(p.link),
            ..TextBox::default()
        };
        tb.show(ui, text_rect, ui.id().with("content"), &mut self.text);
        if self.text.len() > MAXIMUM_CONTENT_BYTES {
            // `MaxLength="2952"` (characters, as Avalonia counts them).
            self.text = self.text.chars().take(MAXIMUM_CONTENT_BYTES).collect();
        }
        let size_row = Rect::from_min_max(pos2(inner.min.x, inner.max.y - size_row_h), inner.max);
        let nf = font(13.0, 400);
        let label = s::QRCodeWindow_QR_code_size();
        let lw = layout::width(ui, label, nf.clone());
        layout::text(ui, pos2(size_row.min.x, center_y(size_row, line_height(ui, &nf))), label, nf.clone(), p.text_secondary, f32::INFINITY);
        let spin = Rect::from_min_size(pos2(size_row.min.x + lw + 8.0, size_row.min.y), vec2(120.0, size_row_h));
        DecimalUpDown { minimum: 0.0, maximum: 2048.0, increment: 64.0, decimals: 0, enabled: true }.show(ui, spin, ui.id().with("size"), &mut self.size);
        layout::text(ui, pos2(spin.max.x + 8.0, center_y(size_row, line_height(ui, &nf))), s::QRCodeWindow_px(), nf, p.text_secondary, f32::INFINITY);
        // The preview.
        let right = Rect::from_min_max(pos2(left.max.x + 12.0, row.min.y), row.max);
        sharex_ui::border::border(ui.painter(), right, 4.0, 1.0, Some(sharex_ui::border::Brush::Solid(p.background_panel)), Some(p.border));
        // `SetAvailableSize`: the preview's bounds less 48.
        self.available = ((right.width() - 48.0).min(right.height() - 48.0).floor().max(64.0)) as u32;
        self.regenerate();
        let area = right.shrink(17.0);
        let mut preview_clicked = false;
        if let Some((_, _, image)) = &self.preview {
            // A white `Border` (`CornerRadius="2"`, `Padding="8"`) filling
            // the preview, the image uniform in it, unsmoothed.
            let white = area;
            let fit = layout::uniform(area.shrink(8.0), image.dimensions());
            sharex_ui::border::border(ui.painter(), white, 2.0, 0.0, Some(sharex_ui::border::Brush::Solid(egui::Color32::WHITE)), None);
            let tex = self.texture.get_or_insert_with(|| {
                let color = egui::ColorImage::from_rgba_unmultiplied([image.width() as usize, image.height() as usize], image.as_raw());
                ui.ctx().load_texture("qr-preview", color, egui::TextureOptions::NEAREST)
            });
            ui.painter().image(tex.id(), fit, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), egui::Color32::WHITE);
            preview_clicked = ui.interact(right, ui.id().with("preview"), egui::Sense::click()).clicked();
        } else {
            let f = font(13.0, 400);
            let lh = line_height(ui, &f);
            let mut y = center_y(area, 48.0 + 8.0 + lh);
            icons::paint(ui.painter(), Rect::from_min_size(pos2((area.center().x - 24.0).round(), y), Vec2::splat(48.0)), icon::QrCode, p.text_secondary);
            y += 56.0;
            sharex_ui::controls::text_centered_at(ui, area.center().x, y, s::QRCodeWindow_Enter_content_to_generate_a_QR_code(), f, p.text_secondary);
        }
        if let Some(i) = scan_clicked {
            match i {
                // `ScanCaptureAsync`: the window hides while the screen is grabbed.
                0 => out.push(Effect::ScanScreen),
                1 => out.push(Effect::NotPorted("QR code region scan")),
                _ => {
                    let file = rfd::FileDialog::new()
                        .set_title(s::QRCodeWindow_Select_image_dialog())
                        .add_filter("Images", &["png", "jpg", "jpeg", "bmp", "gif", "webp", "tif", "tiff"])
                        .pick_file();
                    if let Some(path) = file {
                        let results = image::open(&path).ok().map(|i| scan(&i.to_rgba8()));
                        self.apply_scan(results);
                    }
                }
            }
        }
        if let Some(i) = action {
            let size = self.effective_size();
            match i {
                0 => out.extend(generate(&self.text, size).map(|im| Effect::Upload(Box::new(UploadRequest::QrCodeImage(im))))),
                1 => self.save(),
                _ => out.extend(generate(&self.text, size).map(Effect::CopyImage)),
            }
        }
        if preview_clicked {
            out.push(Effect::NotPorted("The image viewer"));
        }
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            out.push(Effect::Close);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes_as_zxing_chooses_them() {
        assert_eq!(choose_mode("0123"), Mode::Numeric);
        assert_eq!(choose_mode("HTTP://A.B"), Mode::Alphanumeric);
        assert_eq!(choose_mode("https://example.com"), Mode::Byte);
    }

    #[test]
    fn codes_scan_back() {
        let image = generate("https://example.com/qr-test", 300).unwrap();
        assert_eq!(image.dimensions(), (300, 300));
        assert_eq!(scan(&image), ["https://example.com/qr-test"]);
    }

    #[test]
    fn file_names_from_the_text() {
        assert_eq!(suggested_file_name("https://a.b/c?d"), "https___a.b_c_d.png");
        assert_eq!(suggested_file_name("  "), "qr-code.png");
    }
}
