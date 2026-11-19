//! `ShareX.Tools/Tools/QRCode` (`QRCodeWindow`, `QRCodeViewModel`) with
//! ShareX's services (`TaskHelpers.ShowQRCodeWindow`): the text, its QR
//! code as ZXing makes it (`TaskHelpers.GenerateQRCode`), copy, save,
//! upload, and scanning the screen or an image file for codes.

use egui::{Rect, Vec2, pos2, vec2};
use image::RgbaImage;
use sharex_strings::tools as s;
use sharex_ui::controls::{Button, DecimalUpDown, TextBox};
use sharex_ui::icons;
use sharex_ui::keys::app_icons as icon;
use sharex_ui::theme::palette;
use sharex_ui::widgets::{center_y, font, line_height};

use super::layout;
use super::{Cx, Dialog, Effect, Spec, UploadRequest};

/// `QRCodeViewModel.MaximumContentBytes` (`CheckQRCodeContent`).
pub const MAXIMUM_CONTENT_BYTES: usize = sharex_tools::qr::MAXIMUM_CONTENT_BYTES;

/// `TaskHelpers.CheckQRCodeContent`.
pub fn check_content(text: &str) -> bool {
    sharex_tools::qr::check_content(text)
}

/// `TaskHelpers.GenerateQRCode(text, size)` (`sharex_tools::qr`: ZXing's
/// writer, a one-module quiet zone, black on white).
pub fn generate(text: &str, size: u32) -> Option<RgbaImage> {
    let bmp = sharex_tools::qr::generate(text, size.min(i32::MAX as u32) as i32)?;
    RgbaImage::from_raw(bmp.width(), bmp.height(), bmp.to_straight())
}

/// `BarcodeScan(bitmap)`: the codes' texts in an image (any format, as
/// `ScanQRCodeAsync` asks), through `sharex_tools::qr` (ZXing's readers).
pub fn scan(image: &RgbaImage) -> Vec<String> {
    let (w, h) = image.dimensions();
    if w == 0 || h == 0 {
        return Vec::new();
    }
    let bmp = sharex_tools::imaging::Bitmap::from_straight(w, h, image.as_raw());
    sharex_tools::qr::scan(&bmp, false).unwrap_or_default()
}

/// `SkiaImageHelpers.LoadImage(filePath)` then `BarcodeScan`; `None` when
/// the file isn't an image (`ScanAsync` gets null).
pub fn scan_file(path: &std::path::Path) -> Option<Vec<String>> {
    let bmp = sharex_tools::imaging::load_image(path)?;
    Some(sharex_tools::qr::scan(&bmp, false).unwrap_or_default())
}

/// `QRCodeWindowOptions.InitialScanMode`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScanMode {
    Screen,
    Region,
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
    /// `InitialScanMode`, run when the window opens.
    pub initial_scan: Option<ScanMode>,
    /// The scans' notification sound (off for `--render`).
    pub play_sound: bool,
}

impl QrCodeWindow {
    /// `TaskHelpers.OpenQRCode(text)`: the clipboard's text when there's none.
    pub fn new(text: Option<String>) -> Self {
        let text = text.or_else(|| sharex_capture::clipboard::get_text().filter(|t| check_content(t))).unwrap_or_default();
        Self::with_text(text)
    }

    /// The window with `text` as it is (no clipboard read).
    pub fn with_text(text: String) -> Self {
        Self { text, size: 0.0, available: 400, preview: None, texture: None, status: String::new(), initial_scan: None, play_sound: true }
    }

    /// `OpenQRCodeScanScreen` / `OpenQRCodeScanRegion`.
    pub fn scan_mode(mode: ScanMode) -> Self {
        Self { initial_scan: Some(mode), ..Self::with_text(String::new()) }
    }

    /// `QRCodeWindowOptions { InitialImageFilePath }`: no text, then the
    /// image's codes (`ScanAsync(QRCodeScanMode.ImageFile, path)`).
    pub fn scan_image_file(path: &std::path::Path) -> Self {
        let mut w = Self::with_text(String::new());
        w.apply_scan(scan_file(path));
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
    pub fn apply_scan(&mut self, results: Option<Vec<String>>) {
        let results = results.unwrap_or_default();
        self.text = sharex_tools::qr::join_results(&results);
        self.status = if results.is_empty() {
            s::QRCodeViewModel_No_QR_code().to_owned()
        } else if results.len() == 1 {
            s::QRCodeViewModel_Decoded_one().replace("{0}", "1")
        } else {
            s::QRCodeViewModel_Decoded_many().replace("{0}", &results.len().to_string())
        };
        if self.play_sound {
            crate::sound::play_action_completed();
        }
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
            sharex_tools::qr::generate_svg_file(&self.text, size as i32).map(|svg| std::fs::write(&path, svg).map_err(|e| e.to_string()))
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

    /// `ScanCaptureAsync(Region)`: the region captured with the window
    /// minimised, scanned.
    fn replace_image(&mut self, image: RgbaImage, _ctx: &egui::Context) {
        self.apply_scan(Some(scan(&image)));
    }

    fn spec(&self) -> Spec {
        Spec::sized(crate::rs_title(s::QRCodeWindow_ShareX_QR_Code()), 940.0, 610.0, 760.0, 520.0)
    }

    fn ui(&mut self, ui: &mut egui::Ui, _cx: &mut Cx) -> Vec<Effect> {
        let p = palette(ui.ctx());
        let full = layout::background(ui);
        let grid = full.shrink(16.0);
        let mut out = Vec::new();
        // `InitializeAsync`: `InitialScanMode` (`ScanCaptureAsync`).
        match self.initial_scan.take() {
            Some(ScanMode::Screen) => out.push(Effect::ScanScreen),
            Some(ScanMode::Region) => out.push(Effect::OcrSelectRegion),
            None => {}
        }
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
        let before = self.text.chars().count();
        tb.show(ui, text_rect, ui.id().with("content"), &mut self.text);
        if self.text.chars().count() > MAXIMUM_CONTENT_BYTES.max(before) {
            // `MaxLength="2952"` (characters, as Avalonia counts them) holds
            // typing back; text set otherwise (a scan, the clipboard) stays.
            self.text = self.text.chars().take(MAXIMUM_CONTENT_BYTES.max(before)).collect();
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
                1 => out.push(Effect::OcrSelectRegion),
                _ => {
                    let file = rfd::FileDialog::new()
                        .set_title(s::QRCodeWindow_Select_image_dialog())
                        .add_filter("Images", &["png", "jpg", "jpeg", "bmp", "gif", "webp", "tif", "tiff"])
                        .pick_file();
                    if let Some(path) = file {
                        self.apply_scan(scan_file(&path));
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
        if preview_clicked && let Some((_, _, image)) = &self.preview {
            // `ImageViewerWindowIntegration.ShowImage(stream.ToArray(), Image_viewer_title)`.
            out.push(Effect::ShowImageNamed(image.clone(), sharex_strings::tools::QRCodeWindow_Image_viewer_title().to_owned()));
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
