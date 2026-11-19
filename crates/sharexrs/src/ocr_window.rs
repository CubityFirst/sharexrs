//! `ShareX.Tools/Tools/OCR/OCRWindow` (`.axaml`, code-behind, `OCRViewModel`,
//! `Controls/OCRPreview`): the image on a recessed canvas with each
//! recognized region's box over it (hover highlights, a click copies the
//! region's text), a scan line while recognizing, the Select region /
//! Select image toolbar; beside it the result sheet: the editable text (or
//! "No text found" / the error), the Single line, Separate paragraphs and
//! Copy automatically switches, the service picker with Open service and
//! Edit services (an overlay editor), and Copy all. Options are saved into
//! the task's `OCROptions` as they change; the result goes to `<file>.txt`
//! when the window was opened for a file (`TaskHelpers.OCRImage`).

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Instant;

use egui::{Color32, CornerRadius, Id, Pos2, Rect, Ui, Vec2, pos2, vec2};
use image::RgbaImage;
use serde_json::{Value, json};
use sharex_ocr::OcrResult;
use sharex_strings::tools as s;
use sharex_ui::border::{self, Brush};
use sharex_ui::controls::{Button, CheckBox, ComboBox, ComboItem, TextBox, ToggleSwitch, content_font};
use sharex_ui::icons;
use sharex_ui::keys::app_icons as icon;
use sharex_ui::theme::{Thickness, palette};
use sharex_ui::widgets::{center_y, font, galley, line_height, text_width};

use crate::dialogs::render::Options;
use crate::dialogs::{Cx, Dialog, Effect, Spec};

pub const WIDTH: f32 = 1100.0;
pub const HEIGHT: f32 = 680.0;
pub const MIN_WIDTH: f32 = 820.0;
pub const MIN_HEIGHT: f32 = 520.0;
/// `ContentGrid`'s third column (`ResultPanelWidth` 0 uses it).
const RESULT_PANEL_WIDTH: f32 = 420.0;
const MIN_RESULT_PANEL_WIDTH: f32 = 300.0;
/// `OCRPreview.MaxFitZoom`.
const MAX_FIT_ZOOM: f32 = 4.0;
/// `Colors.DodgerBlue`: `HighlightBrush` is ShareX's accent gradient, not a
/// solid colour, so `OCRPreview` falls back to it.
const HIGHLIGHT: Color32 = Color32::from_rgb(0x1E, 0x90, 0xFF);

/// Where the window's `OCROptions` live, to save its changes into.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OptionsTarget {
    /// `DefaultTaskSettings.CaptureSettings.OCROptions`.
    Default,
    /// A task's own capture settings (kept for the window's life).
    Task,
}

/// `OCROptions.DefaultServiceLinks`.
fn default_services() -> Vec<(String, String)> {
    vec![
        (s::OCROptions_Google_Translate().into(), "https://translate.google.com/?sl=auto&tl=en&text={0}&op=translate".into()),
        (s::OCROptions_Google_Search().into(), "https://www.google.com/search?q={0}".into()),
        (s::OCROptions_Google_Images().into(), "https://www.google.com/search?q={0}&tbm=isch".into()),
        (s::OCROptions_Bing().into(), "https://www.bing.com/search?q={0}".into()),
        (s::OCROptions_DuckDuckGo().into(), "https://duckduckgo.com/?q={0}".into()),
        (s::OCROptions_DeepL().into(), "https://www.deepl.com/translator#auto/en/{0}".into()),
    ]
}

/// `OCRServiceLinkOption.GenerateLink`: `{0}` replaced by the URL-encoded text.
pub fn generate_link(url: &str, input: &str) -> Option<String> {
    if input.trim().is_empty() || url.trim().is_empty() {
        return None;
    }
    Some(url.replace("{0}", &sharex_uploaders::url_helpers::url_encode(input, false)))
}

/// The service editor's state.
struct ServiceEditor {
    editing: Option<usize>,
}

type Recognition = (u64, Result<OcrResult, String>);

pub struct OcrWindow {
    image: RgbaImage,
    file_path: Option<PathBuf>,
    /// `OCROptions` as JSON (unknown properties kept).
    options: Value,
    target: OptionsTarget,
    single_line: bool,
    separate_paragraphs: bool,
    auto_copy: bool,
    close_after_open: bool,
    services: Vec<(String, String)>,
    selected_service: Option<usize>,
    editor: Option<ServiceEditor>,
    result: Option<OcrResult>,
    text: String,
    error: String,
    busy: bool,
    busy_since: Instant,
    version: u64,
    rx: Option<mpsc::Receiver<Recognition>>,
    texture: Option<((u32, u32), egui::TextureHandle)>,
    /// `OCRPreview._zoom` (relative to the fitted size) and `_pan` (from
    /// the centre of the content area).
    zoom: f32,
    pan: egui::Vec2,
    /// A drag of the zoomed image: where it started and the pan then.
    drag: Option<(Pos2, egui::Vec2)>,
    /// `ResultPanelColumn.Width` (the splitter moves it).
    panel_width: f32,
    hovered: Option<usize>,
    copied: Option<usize>,
    started: bool,
    options_changed: bool,
    effects: Vec<Effect>,
    sync: bool,
    /// A task waiting for the window to close (`WorkerTask.DoOCR`).
    pub reply: Option<mpsc::Sender<()>>,
}

impl OcrWindow {
    /// The window over a task's `CaptureSettingsReference.OCROptions`:
    /// saved into the default task settings when the task uses them.
    pub fn for_task(image: RgbaImage, file_path: Option<PathBuf>, task: &sharex_settings::TaskSettings) -> Self {
        let options = task.CaptureSettings.as_ref().map(|c| c.OCROptions.clone()).unwrap_or(Value::Null);
        let target = if task.UseDefaultCaptureSettings { OptionsTarget::Default } else { OptionsTarget::Task };
        Self::new(image, file_path, &options, target)
    }

    /// `new OCRWindow(imageData, options, recognize, selectRegion)` over a
    /// task's `OCROptions` (`TaskHelpers.OCRImage`: services with
    /// ShareX's default names are re-localized).
    pub fn new(image: RgbaImage, file_path: Option<PathBuf>, options: &Value, target: OptionsTarget) -> Self {
        let flag = |k: &str, d: bool| options.get(k).and_then(Value::as_bool).unwrap_or(d);
        let mut services: Vec<(String, String)> = options
            .get("ServiceLinks")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .map(|l| (l.get("Name").and_then(Value::as_str).unwrap_or("").to_string(), l.get("Url").and_then(Value::as_str).unwrap_or("").to_string()))
                    .collect()
            })
            .unwrap_or_else(default_services);
        let defaults = default_services();
        if services.len() == defaults.len() && services.iter().zip(&defaults).all(|(a, b)| a.0 == b.0) {
            services = defaults;
        }
        let selected = options.get("SelectedServiceLink").and_then(Value::as_i64).unwrap_or(0);
        let selected_service = (!services.is_empty()).then(|| selected.clamp(0, services.len() as i64 - 1) as usize);
        Self {
            image,
            file_path,
            options: options.clone(),
            target,
            single_line: flag("SingleLine", false),
            separate_paragraphs: flag("SeparateParagraphs", true),
            auto_copy: flag("AutoCopy", false),
            close_after_open: flag("CloseWindowAfterOpeningServiceLink", false),
            services,
            selected_service,
            editor: None,
            result: None,
            text: String::new(),
            error: String::new(),
            busy: false,
            busy_since: Instant::now(),
            version: 0,
            rx: None,
            texture: None,
            zoom: 1.0,
            pan: egui::Vec2::ZERO,
            drag: None,
            // `ResultPanelWidth` (0: the column's 420), at least 300.
            panel_width: match options.get("ResultPanelWidth").and_then(Value::as_f64) {
                Some(w) if w > 0.0 => (w as f32).max(MIN_RESULT_PANEL_WIDTH),
                _ => RESULT_PANEL_WIDTH,
            },
            hovered: None,
            copied: None,
            started: false,
            options_changed: false,
            effects: Vec::new(),
            sync: false,
            reply: None,
        }
    }

    /// `OCRViewModel.Result`: the text trimmed (`<file>.txt`, the window's answer).
    pub fn result_text(&self) -> String {
        self.text.trim().to_string()
    }

    /// `SaveOptions`.
    fn save_options(&mut self) {
        if !self.options.is_object() {
            self.options = json!({});
        }
        let o = self.options.as_object_mut().unwrap();
        o.insert("SingleLine".into(), json!(self.single_line));
        o.insert("SeparateParagraphs".into(), json!(self.separate_paragraphs));
        o.insert("AutoCopy".into(), json!(self.auto_copy));
        o.insert("CloseWindowAfterOpeningServiceLink".into(), json!(self.close_after_open));
        o.insert("SelectedServiceLink".into(), json!(self.selected_service.unwrap_or(0)));
        o.insert("ServiceLinks".into(), Value::Array(self.services.iter().map(|(n, u)| json!({"Name": n, "Url": u})).collect()));
        self.options_changed = true;
    }

    /// `RecognizeAsync`: on a thread; a later run's result wins.
    fn recognize(&mut self, ctx: &egui::Context) {
        self.version += 1;
        let version = self.version;
        self.busy = true;
        self.busy_since = Instant::now();
        self.error.clear();
        self.text.clear();
        self.result = None;
        self.hovered = None;
        self.copied = None;
        let bitmap = sharex_ocr::bitmap_from_rgba(self.image.width(), self.image.height(), self.image.as_raw());
        if self.sync {
            let r = sharex_ocr::recognize(&bitmap).map_err(|e| e.to_string());
            self.finish((version, r));
            return;
        }
        let (tx, rx) = mpsc::channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let r = sharex_ocr::recognize(&bitmap).map_err(|e| e.to_string());
            let _ = tx.send((version, r));
            ctx.request_repaint();
        });
        self.rx = Some(rx);
    }

    fn finish(&mut self, (version, r): Recognition) {
        if version != self.version {
            return;
        }
        self.busy = false;
        match r {
            Ok(result) => {
                self.text = result.text(self.single_line, self.separate_paragraphs);
                self.result = Some(result);
                if self.auto_copy && !self.text.trim().is_empty() {
                    self.effects.push(Effect::CopyText(self.text.clone()));
                }
            }
            Err(e) => {
                eprintln!("OCRViewModel: OCR failed. {e}");
                self.error = e;
            }
        }
    }

    /// `ReformatResult`.
    fn reformat(&mut self) {
        if let Some(r) = &self.result
            && !self.busy
        {
            self.text = r.text(self.single_line, self.separate_paragraphs);
        }
        self.save_options();
    }

    /// `ReplaceSourceAsync`: a new image (a region or a file), recognized.
    pub fn replace_source(&mut self, image: RgbaImage, ctx: &egui::Context) {
        self.image = image;
        self.texture = None;
        // `OnPropertyChanged(Source)` -> `ResetZoom`.
        self.zoom = 1.0;
        self.pan = egui::Vec2::ZERO;
        self.drag = None;
        self.recognize(ctx);
    }

    fn can_use_result(&self) -> bool {
        !self.busy && !self.text.trim().is_empty()
    }
}

/// `OCRPreview.MaxZoomScale`: zooming in stops at this many units a pixel.
const MAX_ZOOM_SCALE: f32 = 32.0;
/// `WheelZoomFactor`.
const WHEEL_ZOOM_FACTOR: f32 = 1.25;

/// `OCRPreview.TryGetLayout`: the image's rect and scale in `content` at
/// `zoom` and `pan`, and the pan that gives that place. An image smaller
/// than the content is centred; a larger one keeps covering it.
fn image_layout(content: Rect, size: (u32, u32), zoom: f32, pan: egui::Vec2) -> Option<(Rect, f32, egui::Vec2)> {
    let (w, h) = (size.0 as f32, size.1 as f32);
    if w <= 0.0 || h <= 0.0 || content.width() <= 0.0 || content.height() <= 0.0 {
        return None;
    }
    let fit = (content.width() / w).min(content.height() / h).min(MAX_FIT_ZOOM);
    let scale = fit * zoom;
    let (sw, sh) = (w * scale, h * scale);
    let cx = content.min.x + (content.width() - sw) / 2.0;
    let cy = content.min.y + (content.height() - sh) / 2.0;
    // `ClampAxis`.
    let clamp = |pos: f32, len: f32, start: f32, end: f32| if len <= end - start { start + (end - start - len) / 2.0 } else { pos.clamp(end - len, start) };
    let x = clamp(cx + pan.x, sw, content.min.x, content.max.x);
    let y = clamp(cy + pan.y, sh, content.min.y, content.max.y);
    Some((Rect::from_min_size(pos2(x, y), vec2(sw, sh)), scale, vec2(x - cx, y - cy)))
}

/// `ZoomAt`: the zoom times `factor` (1 to `MaxZoomScale`), the image point
/// under `point` kept where it is. The new zoom and pan, if it changed.
fn zoom_at(content: Rect, size: (u32, u32), zoom: f32, pan: egui::Vec2, point: Pos2, factor: f32) -> Option<(f32, egui::Vec2)> {
    let (rect, scale, _) = image_layout(content, size, zoom, pan)?;
    let fit = scale / zoom;
    let new_zoom = (zoom * factor).clamp(1.0, (MAX_ZOOM_SCALE / fit).max(1.0));
    if new_zoom == zoom {
        return None;
    }
    let new_scale = fit * new_zoom;
    let ip = ((point.x - rect.min.x) / scale, (point.y - rect.min.y) / scale);
    let top_left = pos2(point.x - ip.0 * new_scale, point.y - ip.1 * new_scale);
    let (sw, sh) = (size.0 as f32 * new_scale, size.1 as f32 * new_scale);
    let centered = pos2(content.min.x + (content.width() - sw) / 2.0, content.min.y + (content.height() - sh) / 2.0);
    let (_, _, clamped) = image_layout(content, size, new_zoom, top_left - centered)?;
    Some((new_zoom, clamped))
}

impl OcrWindow {
    /// The canvas: the image, its regions, the scan line and the toolbar.
    fn canvas(&mut self, ui: &mut Ui, rect: Rect, live: bool) {
        let p = palette(ui.ctx());
        let dark = ui.ctx().theme() == egui::Theme::Dark;
        let canvas_color = if dark { Color32::from_rgb(0x1C, 0x1C, 0x1C) } else { Color32::from_rgb(0xE9, 0xE9, 0xEC) };
        border::border(ui.painter(), rect, 10.0, 0.0, Some(Brush::Solid(canvas_color)), None);
        // `Padding="28,28,28,84"`: clear of the toolbar.
        let content = Rect::from_min_max(pos2(rect.min.x + 28.0, rect.min.y + 28.0), pos2(rect.max.x - 28.0, rect.max.y - 84.0));
        let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
        let size_px = self.image.dimensions();
        if live {
            // `OnPointerWheelChanged` (`OCRPreview`).
            let hover = ui.input(|i| i.pointer.hover_pos()).filter(|p| rect.contains(*p));
            if let Some(pt) = hover {
                let wheel: f32 = ui.input(|i| {
                    i.events
                        .iter()
                        .filter_map(|e| if let egui::Event::MouseWheel { delta, unit, .. } = e { Some(if *unit == egui::MouseWheelUnit::Point { delta.y / 50.0 } else { delta.y }) } else { None })
                        .sum()
                });
                if wheel != 0.0
                    && let Some((z, p)) = zoom_at(content, size_px, self.zoom, self.pan, pt, WHEEL_ZOOM_FACTOR.powf(wheel))
                {
                    self.zoom = z;
                    self.pan = p;
                }
            }
        }
        if let Some((img_rect, scale, clamped)) = image_layout(content, size_px, self.zoom, self.pan) {
            self.pan = clamped;
            // The soft shadow under the image (`Blur=24, OffsetY=6`, black 90).
            let shadow = egui::epaint::Shadow { offset: [0, 6], blur: 24, spread: 0, color: Color32::from_black_alpha(90) };
            painter.add(shadow.as_shape(img_rect, CornerRadius::same(6)));
            // Enlarged pixels stay sharp (the picture at its own size, drawn
            // unsmoothed); shrinking is smoothed.
            let size = if scale > 1.0 { size_px } else { (img_rect.width().round().max(1.0) as u32, img_rect.height().round().max(1.0) as u32) };
            if self.texture.as_ref().is_none_or(|(s, _)| *s != size) {
                let img = if scale > 1.0 { self.image.clone() } else { crate::thumbnail::resize(&self.image, size.0, size.1) };
                let rounded = round_corners(img, if scale > 1.0 { 6.0 / scale } else { 6.0 });
                let filter = if scale > 1.0 { egui::TextureOptions::NEAREST } else { egui::TextureOptions::LINEAR };
                let tex = ui.ctx().load_texture("ocr-image", crate::thumbnail::to_color_image(&rounded), filter);
                self.texture = Some((size, tex));
            }
            if let Some((_, tex)) = &self.texture {
                painter.image(tex.id(), img_rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            }
            if self.busy {
                // The scan line sweeping (1.4 s each way, sine eased).
                let t = self.busy_since.elapsed().as_secs_f32() / 1.4;
                let phase = t.fract();
                let forward = (t as u32).is_multiple_of(2);
                let eased = 0.5 - 0.5 * (std::f32::consts::PI * phase).cos();
                let pos = if forward { eased } else { 1.0 - eased };
                let y = img_rect.min.y + pos * img_rect.height();
                let glow = (img_rect.height() / 4.0).clamp(8.0, 56.0);
                let clip = painter.with_clip_rect(img_rect.intersect(painter.clip_rect()));
                clip.rect_filled(img_rect, CornerRadius::same(6), Color32::from_black_alpha(51));
                let steps = 16;
                for k in 0..steps {
                    let a = k as f32 / steps as f32;
                    let alpha = (110.0 * (1.0 - (a * 2.0 - 1.0).abs())) as u8;
                    let band = Rect::from_min_max(pos2(img_rect.min.x, y - glow + a * glow * 2.0), pos2(img_rect.max.x, y - glow + (a + 1.0 / steps as f32) * glow * 2.0));
                    clip.rect_filled(band, 0.0, Color32::from_rgba_unmultiplied(HIGHLIGHT.r(), HIGHLIGHT.g(), HIGHLIGHT.b(), alpha));
                }
                clip.line_segment([pos2(img_rect.min.x, y), pos2(img_rect.max.x, y)], egui::Stroke::new(2.0, HIGHLIGHT));
                ui.ctx().request_repaint();
            }
            if let Some(result) = &self.result {
                let pointer = ui.input(|i| i.pointer.hover_pos()).filter(|p| img_rect.contains(*p));
                let hit = pointer.and_then(|pt| {
                    let ix = (pt.x - img_rect.min.x) / scale;
                    let iy = (pt.y - img_rect.min.y) / scale;
                    result.regions.iter().position(|r| ix >= r.bounds.left && ix < r.bounds.right && iy >= r.bounds.top && iy < r.bounds.bottom)
                });
                if live && hit != self.hovered {
                    self.hovered = hit;
                    self.copied = None;
                }
                for (i, region) in result.regions.iter().enumerate() {
                    let pts: Vec<Pos2> = region.points.iter().map(|q| pos2(img_rect.min.x + q.x * scale, img_rect.min.y + q.y * scale)).collect();
                    let alpha = if Some(i) == self.copied {
                        0.55
                    } else if Some(i) == self.hovered {
                        0.32
                    } else {
                        0.14
                    };
                    let fill = Color32::from_rgba_unmultiplied(HIGHLIGHT.r(), HIGHLIGHT.g(), HIGHLIGHT.b(), (alpha * 255.0f32).round() as u8);
                    painter.add(egui::Shape::convex_polygon(pts, fill, egui::Stroke::new(1.5, HIGHLIGHT)));
                }
                if live {
                    let r = ui.interact(img_rect.intersect(rect), Id::new("ocr-preview"), egui::Sense::click_and_drag());
                    if r.double_clicked() {
                        // `ResetZoom`.
                        self.zoom = 1.0;
                        self.pan = egui::Vec2::ZERO;
                        self.drag = None;
                    }
                    if self.zoom > 1.0 {
                        if r.drag_started() {
                            self.drag = r.interact_pointer_pos().map(|p| (p, self.pan));
                        }
                        if let (Some((start, start_pan)), Some(now)) = (self.drag, r.interact_pointer_pos())
                            && r.dragged()
                            && let Some((_, _, p)) = image_layout(content, size_px, self.zoom, start_pan + (now - start))
                        {
                            self.pan = p;
                        }
                        if r.drag_stopped() {
                            self.drag = None;
                        }
                        ui.ctx().set_cursor_icon(if self.drag.is_some() { egui::CursorIcon::Grabbing } else { egui::CursorIcon::PointingHand });
                    }
                    if let Some(h) = hit {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        let tip = if Some(h) == self.copied { s::OCRWindow_Copied_to_clipboard().to_string() } else { result.regions[h].text.clone() };
                        r.clone().on_hover_text(tip);
                    }
                    if r.clicked()
                        && let Some(i) = hit
                    {
                        self.copied = Some(i);
                        self.effects.push(Effect::CopyText(result.regions[i].text.clone()));
                    }
                }
            }
        }
        // The floating toolbar (`Border.ocr-toolbar`).
        let body = font(13.0, 400);
        let labels = [(icon::Crop, s::OCRWindow_Select_region()), (icon::Image, s::OCRWindow_Select_image())];
        let widths: Vec<f32> = labels.iter().map(|(_, t)| 14.0 + 13.0 + 8.0 + text_width(ui, t, body.clone()) + 14.0).collect();
        let tw = widths.iter().sum::<f32>() + 2.0 + 8.0 + 2.0;
        let th = 36.0 + 8.0 + 2.0;
        let bar = Rect::from_min_size(pos2(rect.min.x + ((rect.width() - tw) / 2.0).round_ties_even(), rect.max.y - 20.0 - th), vec2(tw, th));
        // `BoxShadow="0 10 28 -8 #60000000"` (the spread as a smaller box).
        let shadow = egui::epaint::Shadow { offset: [0, 10], blur: 28, spread: 0, color: Color32::from_black_alpha(0x60) };
        painter.add(shadow.as_shape(bar.shrink(8.0), CornerRadius::same(14)));
        border::border(&painter, bar, 22.0, 1.0, Some(Brush::Solid(p.background_popup)), Some(p.border));
        let mut x = bar.min.x + 5.0;
        for (k, (key, text)) in labels.into_iter().enumerate() {
            let br = Rect::from_min_size(pos2(x, bar.min.y + 5.0), vec2(widths[k], 36.0));
            let enabled = !self.busy;
            let r = ui.interact(br, Id::new(("ocr-toolbar", k)), if enabled && live { egui::Sense::click() } else { egui::Sense::hover() });
            if enabled && (r.hovered() || r.is_pointer_button_down_on()) {
                let fill = if r.is_pointer_button_down_on() { p.control_background } else { p.control_background_hover };
                painter.rect_filled(br, CornerRadius::same(18), fill);
            }
            let opacity = if enabled { 1.0 } else { 0.35 };
            let lh = line_height(ui, &body);
            let top = center_y(br, lh);
            icons::paint(&painter, Rect::from_min_size(pos2(br.min.x + 14.0, top + (lh - 13.0) / 2.0), Vec2::splat(13.0)), key, p.text.gamma_multiply(opacity));
            let c = p.text_secondary.gamma_multiply(opacity);
            painter.galley(pos2(br.min.x + 14.0 + 21.0, top), galley(ui, text, body.clone(), c, f32::INFINITY), c);
            if r.clicked() && enabled && live {
                if k == 0 {
                    self.effects.push(Effect::OcrSelectRegion);
                } else if let Some(path) = rfd::FileDialog::new()
                    .set_title(s::OCRWindow_Select_image_dialog())
                    .add_filter("Images", &["png", "jpg", "jpeg", "jpe", "jfif", "gif", "bmp", "tif", "tiff", "webp", "ico"])
                    .pick_file()
                    && let Some(img) = crate::thumbnail::load(&path)
                {
                    self.replace_source(img, ui.ctx());
                }
            }
            x += widths[k] + 2.0;
        }
    }

    /// The result sheet.
    fn sheet(&mut self, ui: &mut Ui, rect: Rect, live: bool) {
        let p = palette(ui.ctx());
        border::border(ui.painter(), rect, 10.0, 1.0, Some(Brush::Solid(p.background_panel)), Some(p.border));
        let inner = rect.shrink(1.0);
        // The header: the icon and "Result" (`Margin="18,16,18,8"`).
        let heading = font(15.0, 600);
        let hl = line_height(ui, &heading);
        let header_h = hl.max(18.0);
        let header = Rect::from_min_size(pos2(inner.min.x + 18.0, inner.min.y + 16.0), vec2(inner.width() - 36.0, header_h));
        icons::paint(ui.painter(), Rect::from_min_size(pos2(header.min.x, center_y(header, 18.0)), Vec2::splat(18.0)), icon::TextRecognition, p.accent_icon);
        ui.painter().galley(pos2(header.min.x + 28.0, center_y(header, hl)), galley(ui, s::OCRWindow_Result(), heading, p.text, f32::INFINITY), p.text);
        let bottom_h = 6.0 + 40.0 + 16.0 + 42.0 + 16.0;
        let options_h = 1.0 + 10.0 + 34.0 * 3.0 + 2.0 * 2.0 + 6.0;
        let text_area = Rect::from_min_max(pos2(inner.min.x, header.max.y + 8.0), pos2(inner.max.x, inner.max.y - bottom_h - options_h));
        // `TextBox.ocr-text`: no border or background, `Padding="18,4,18,12"`, 14px.
        let before = self.text.clone();
        TextBox { padding: Thickness::new(18.0, 4.0, 18.0, 12.0), multiline: true, wrap: true, plain: true, enabled: !self.busy, font: Some(content_font(14.0, 400)), ..TextBox::default() }
            .show(ui, text_area, Id::new("ocr-text"), &mut self.text);
        if self.text != before {
            self.error.clear();
        }
        let normal = font(13.0, 400);
        let lh = line_height(ui, &normal);
        let no_text = self.result.as_ref().is_some_and(|r| r.regions.is_empty());
        if no_text {
            let tw = text_width(ui, s::OCRWindow_No_text_found(), normal.clone());
            let h = 34.0 + 10.0 + lh;
            let top = text_area.min.y + ((text_area.height() - h) / 2.0).round_ties_even();
            icons::paint(ui.painter(), Rect::from_min_size(pos2(text_area.min.x + ((text_area.width() - 34.0) / 2.0).round_ties_even(), top), Vec2::splat(34.0)), icon::TextRecognition, p.text_secondary);
            ui.painter().galley(pos2(text_area.min.x + ((text_area.width() - tw) / 2.0).round_ties_even(), top + 44.0), galley(ui, s::OCRWindow_No_text_found(), normal.clone(), p.text_secondary, f32::INFINITY), p.text_secondary);
        }
        if !self.error.trim().is_empty() {
            let w = text_area.width() - 48.0;
            let lines = sharex_ui::controls::wrap_lines(ui, &self.error, &normal, w);
            let h = 34.0 + 10.0 + lh * lines.len() as f32;
            let top = text_area.min.y + ((text_area.height() - h) / 2.0).round_ties_even();
            icons::paint(ui.painter(), Rect::from_min_size(pos2(text_area.min.x + ((text_area.width() - 34.0) / 2.0).round_ties_even(), top), Vec2::splat(34.0)), icon::Warning, p.cancel_border);
            for (k, line) in lines.iter().enumerate() {
                let lw = text_width(ui, line, normal.clone());
                ui.painter().galley(pos2(text_area.min.x + ((text_area.width() - lw) / 2.0).round_ties_even(), top + 44.0 + lh * k as f32), galley(ui, line, normal.clone(), p.text_secondary, f32::INFINITY), p.text_secondary);
            }
        }
        // The options (`Padding="18,10,14,6"`, a separator above).
        let opts = Rect::from_min_max(pos2(inner.min.x, text_area.max.y), pos2(inner.max.x, text_area.max.y + options_h));
        ui.painter().rect_filled(Rect::from_min_size(opts.min, vec2(opts.width(), 1.0)), 0.0, p.separator);
        let mut y = opts.min.y + 1.0 + 10.0;
        let rows: [(&str, u8, bool); 3] = [
            (s::OCRWindow_Single_line(), 0, true),
            (s::OCRWindow_Separate_paragraphs(), 1, !self.single_line),
            (s::OCRWindow_Copy_recognized_text_automatically(), 2, true),
        ];
        for (label, k, enabled) in rows {
            let row = Rect::from_min_max(pos2(opts.min.x + 18.0, y), pos2(opts.max.x - 14.0, y + 34.0));
            let color = if enabled { p.text_secondary } else { p.text_secondary.gamma_multiply(0.5) };
            ui.painter().galley(pos2(row.min.x, center_y(row, lh)), galley(ui, label, normal.clone(), color, row.width() - 60.0), color);
            let value = match k {
                0 => &mut self.single_line,
                1 => &mut self.separate_paragraphs,
                _ => &mut self.auto_copy,
            };
            let before = *value;
            ToggleSwitch { enabled: enabled && live }.show(ui, pos2(row.max.x - ToggleSwitch::SIZE.x, center_y(row, ToggleSwitch::SIZE.y)), Id::new(("ocr-opt", k)), value);
            if *value != before {
                if k < 2 { self.reformat() } else { self.save_options() }
            }
            y += 34.0 + 2.0;
        }
        // The services and Copy all (`Margin="16,6,16,16" Spacing="16"`).
        let bottom = Rect::from_min_max(pos2(inner.min.x + 16.0, opts.max.y + 6.0), pos2(inner.max.x - 16.0, inner.max.y - 16.0));
        let row = Rect::from_min_size(bottom.min, vec2(bottom.width(), 40.0));
        let settings_r = Rect::from_min_size(pos2(row.max.x - 40.0, row.min.y), Vec2::splat(40.0));
        let open_r = Rect::from_min_size(pos2(settings_r.min.x - 6.0 - 40.0, row.min.y), Vec2::splat(40.0));
        let combo_r = Rect::from_min_max(row.min, pos2(open_r.min.x - 6.0, row.max.y));
        let items: Vec<ComboItem> = self.services.iter().map(|(n, _)| ComboItem::text(n.clone())).collect();
        let combo = ComboBox { padding: Thickness::hv(8.0, 6.0), enabled: !self.services.is_empty() && live };
        if let Some(i) = combo.show(ui, combo_r, Id::new("ocr-service"), &items, self.selected_service) {
            self.selected_service = Some(i);
            self.save_options();
        }
        if icon_only_button(ui, open_r, Id::new("ocr-open-service"), icon::ExternalLink, self.can_use_result() && live).on_hover_text(s::OCRWindow_Open_service()).clicked()
            && let Some(url) = self.selected_service.and_then(|i| self.services.get(i)).and_then(|(_, u)| generate_link(u, &self.result_text()))
        {
            self.effects.push(Effect::OpenUrl(url));
            if self.close_after_open {
                self.effects.push(Effect::Close);
            }
        }
        if icon_only_button(ui, settings_r, Id::new("ocr-edit-services"), icon::Settings, live).on_hover_text(s::OCRWindow_Edit_services()).clicked() {
            self.editor = Some(ServiceEditor { editing: self.selected_service.or((!self.services.is_empty()).then_some(0)) });
        }
        let copy_r = Rect::from_min_size(pos2(bottom.min.x, row.max.y + 16.0), vec2(bottom.width(), 42.0));
        if primary_icon_button(ui, copy_r, Id::new("ocr-copy-all"), icon::Copy, s::OCRWindow_Copy_all(), self.can_use_result() && live).clicked() {
            self.effects.push(Effect::CopyText(self.text.clone()));
        }
    }

    /// The service editor overlay (620 wide, `Padding="20"`).
    fn services_ui(&mut self, ui: &mut Ui, full: Rect) {
        let layer = egui::LayerId::new(egui::Order::Foreground, Id::new("ocr-services"));
        let mut ui = ui.new_child(egui::UiBuilder::new().layer_id(layer).max_rect(full));
        let _ = ui.interact(full, Id::new("ocr-services-block"), egui::Sense::click());
        let p = palette(ui.ctx());
        ui.painter().rect_filled(full, 0.0, p.overlay_modal);
        let h = 2.0 + 40.0 + 40.0 + 14.0 + 40.0 + 14.0 + 90.0 + 14.0 + 40.0;
        let w = 620.0;
        let card = Rect::from_min_size(pos2(full.min.x + ((full.width() - w) / 2.0).round_ties_even(), full.min.y + ((full.height() - h) / 2.0).round_ties_even()), vec2(w, h));
        let shadow = egui::epaint::Shadow { offset: [0, 20], blur: 48, spread: 0, color: Color32::from_black_alpha(0x70) };
        ui.painter().add(shadow.as_shape(card.shrink(12.0), CornerRadius::same(10)));
        border::border(ui.painter(), card, 10.0, 1.0, Some(Brush::Solid(p.background_panel)), Some(p.border));
        let c = card.shrink(21.0);
        let heading = font(15.0, 600);
        let header = Rect::from_min_size(c.min, vec2(c.width(), 40.0));
        ui.painter().galley(pos2(c.min.x, center_y(header, line_height(&ui, &heading))), galley(&ui, s::OCRWindow_Edit_services(), heading, p.text, f32::INFINITY), p.text);
        let has = self.editor.as_ref().is_some_and(|e| e.editing.is_some());
        let buttons = [(icon::Plus, s::OCRWindow_New(), true), (icon::Trash, s::OCRWindow_Remove(), has), (icon::Reset, s::OCRWindow_Reset(), true)];
        let widths: Vec<f32> = buttons.iter().map(|(_, t, _)| crate::history_ui::common::icon_button_width(&ui, t)).collect();
        let mut x = header.max.x - widths.iter().sum::<f32>() - 16.0;
        let mut clicked = None;
        for (k, (key, text, enabled)) in buttons.into_iter().enumerate() {
            let r = Rect::from_min_size(pos2(x, header.min.y), vec2(widths[k], 40.0));
            if crate::history_ui::common::icon_button(&ui, r, Id::new(("ocr-svc-btn", k)), key, text, false, enabled).clicked() {
                clicked = Some(k);
            }
            x += widths[k] + 8.0;
        }
        match clicked {
            // `NewService`.
            Some(0) => {
                self.services.push((s::OCRViewModel_New_service_name().into(), "https://example.com/search?q={0}".into()));
                let i = self.services.len() - 1;
                self.editor = Some(ServiceEditor { editing: Some(i) });
                self.selected_service = Some(i);
                self.save_options();
            }
            // `RemoveService`.
            Some(1) => {
                if let Some(i) = self.editor.as_ref().and_then(|e| e.editing) {
                    self.services.remove(i);
                    let next = (!self.services.is_empty()).then(|| (i as i64 - 1).clamp(0, self.services.len() as i64 - 1) as usize);
                    self.editor = Some(ServiceEditor { editing: next });
                    self.selected_service = next;
                    self.save_options();
                }
            }
            // `ResetServices`.
            Some(2) => {
                self.services = default_services();
                self.editor = Some(ServiceEditor { editing: Some(0) });
                self.selected_service = Some(0);
                self.save_options();
            }
            _ => {}
        }
        let mut y = header.max.y + 14.0;
        let items: Vec<ComboItem> = self.services.iter().map(|(n, _)| ComboItem::text(n.clone())).collect();
        let combo_r = Rect::from_min_size(pos2(c.min.x, y), vec2(c.width(), 40.0));
        let editing = self.editor.as_ref().and_then(|e| e.editing);
        if let Some(i) = (ComboBox { padding: Thickness::hv(8.0, 6.0), enabled: true }).show(&ui, combo_r, Id::new("ocr-svc-combo"), &items, editing) {
            self.editor = Some(ServiceEditor { editing: Some(i) });
        }
        if editing.is_none() {
            let f = sharex_ui::theme::fluent(ui.ctx());
            let pf = ComboBox::item_font();
            ui.painter().galley(pos2(combo_r.min.x + 8.0, center_y(combo_r, line_height(&ui, &pf))), galley(&ui, s::OCRWindow_Select_a_service(), pf, f.base_medium, f32::INFINITY), f.base_medium);
        }
        y += 40.0 + 14.0;
        let normal = font(13.0, 400);
        for (k, label) in [s::OCRWindow_Name(), s::OCRWindow_URL()].into_iter().enumerate() {
            let lr = Rect::from_min_size(pos2(c.min.x, y), vec2(75.0, 40.0));
            sharex_ui::controls::label(&ui, lr, label, normal.clone(), p.text_secondary);
            let tr = Rect::from_min_max(pos2(c.min.x + 85.0, y), pos2(c.max.x, y + 40.0));
            let placeholder = if k == 1 { s::OCRWindow_Use_0_where_the_recognized_text_belongs() } else { "" };
            let mut text = editing.and_then(|i| self.services.get(i)).map(|(n, u)| if k == 0 { n.clone() } else { u.clone() }).unwrap_or_default();
            let before = text.clone();
            TextBox { enabled: editing.is_some(), placeholder, ..TextBox::default() }.show(&mut ui, tr, Id::new(("ocr-svc-field", k)), &mut text);
            if text != before
                && let Some(i) = editing
            {
                if k == 0 {
                    self.services[i].0 = text;
                } else {
                    self.services[i].1 = text;
                }
                self.save_options();
            }
            y += 50.0;
        }
        y += 4.0;
        let cb = CheckBox { text: s::OCRWindow_Close_after_opening(), text_font: content_font(13.0, 400), text_color: None, min_height: 32.0, enabled: true };
        let before = self.close_after_open;
        cb.show(&ui, Rect::from_min_size(pos2(c.min.x, center_y(Rect::from_min_size(pos2(c.min.x, y), vec2(1.0, 40.0)), 32.0)), vec2(cb.width(&ui), 32.0)), Id::new("ocr-svc-close-after"), &mut self.close_after_open);
        if before != self.close_after_open {
            self.save_options();
        }
        let close = s::OCRWindow_Close();
        let b = Button { primary: true, ..Button::default() };
        let bw = b.width(&ui, close).max(96.0);
        let br = Rect::from_min_size(pos2(c.max.x - bw, y), vec2(bw, 40.0));
        if b.show(&ui, br, Id::new("ocr-svc-close"), close, true).clicked() || sharex_ui::popup::escape_pressed(ui.ctx()) {
            // `CloseServiceEditor`.
            self.editor = None;
            self.save_options();
        }
    }
}

/// A 40x40 `Button` with only an icon (`Padding="0"`).
fn icon_only_button(ui: &Ui, rect: Rect, id: Id, key: &str, enabled: bool) -> egui::Response {
    let p = palette(ui.ctx());
    let r = Button::compact(Thickness::hv(0.0, 0.0)).show(ui, rect, id, "", enabled);
    let opacity = if enabled { 1.0 } else { 0.35 };
    let inner = rect.shrink(1.0);
    let b = Rect::from_min_size(pos2(inner.min.x + ((inner.width() - 13.0) / 2.0).round_ties_even(), center_y(inner, 13.0)), Vec2::splat(13.0));
    icons::paint(ui.painter(), b, key, p.text.gamma_multiply(opacity));
    r
}

/// `Button.primary` with an icon and a `TextBlock`, centred.
fn primary_icon_button(ui: &Ui, rect: Rect, id: Id, key: &str, text: &str, enabled: bool) -> egui::Response {
    let p = palette(ui.ctx());
    let r = Button { primary: true, ..Button::default() }.show(ui, rect, id, "", enabled);
    let opacity = if enabled { 1.0 } else { 0.35 };
    let body = font(13.0, 400);
    let tw = text_width(ui, text, body.clone());
    let content_w = 13.0 + 8.0 + tw;
    let inner = rect.shrink(1.0);
    let x = inner.min.x + ((inner.width() - content_w) / 2.0).round_ties_even();
    let lh = line_height(ui, &body);
    let top = center_y(inner, lh);
    let c = p.accent_foreground.gamma_multiply(opacity);
    icons::paint(ui.painter(), Rect::from_min_size(pos2(x, top + (lh - 13.0) / 2.0), Vec2::splat(13.0)), key, c);
    ui.painter().galley(pos2(x + 21.0, top), galley(ui, text, body, c, f32::INFINITY), c);
    r
}

/// The image clipped to `radius` corners (`PushClip(roundedImageRect)`).
fn round_corners(mut img: RgbaImage, radius: f32) -> RgbaImage {
    let (w, h) = img.dimensions();
    for y in 0..h {
        for x in 0..w {
            let c = crate::thumbnail::corner_coverage(x, y, w, h, radius);
            if c < 1.0 {
                let px = img.get_pixel_mut(x, y);
                px[3] = (px[3] as f32 * c).round() as u8;
            }
        }
    }
    img
}

impl Dialog for OcrWindow {
    fn spec(&self) -> Spec {
        Spec::sized(crate::rs_title(s::OCRWindow_ShareX_Optical_character_recognition()), WIDTH, HEIGHT, MIN_WIDTH, MIN_HEIGHT)
    }

    fn ui(&mut self, ui: &mut Ui, cx: &mut Cx) -> Vec<Effect> {
        if !self.started {
            // `OnOpened` → `StartAsync`: the options saved, then recognition.
            self.started = true;
            self.save_options();
            self.recognize(ui.ctx());
        }
        if let Some(rx) = &self.rx
            && let Ok(r) = rx.try_recv()
        {
            self.rx = None;
            self.finish(r);
        }
        let live = self.editor.is_none();
        if live && sharex_ui::popup::escape_pressed(ui.ctx()) && !ComboBox::is_open(ui.ctx(), Id::new("ocr-service")) {
            self.effects.push(Effect::Close);
        }
        let p = palette(ui.ctx());
        let full = ui.max_rect();
        ui.painter().rect_filled(full, 0.0, p.background_main);
        let content = full.shrink(12.0);
        let panel_w = self.panel_width.max(MIN_RESULT_PANEL_WIDTH).min((content.width() - 12.0).max(MIN_RESULT_PANEL_WIDTH));
        let sheet = Rect::from_min_max(pos2(content.max.x - panel_w, content.min.y), content.max);
        let canvas = Rect::from_min_max(content.min, pos2(sheet.min.x - 12.0, content.max.y));
        // The `GridSplitter` in the 12px column between them.
        let split = Rect::from_min_max(pos2(canvas.max.x, content.min.y), pos2(sheet.min.x, content.max.y));
        let sr = ui.interact(split, Id::new("ocr-splitter"), egui::Sense::drag()).on_hover_cursor(egui::CursorIcon::ResizeColumn);
        if live && sr.dragged() {
            let dx = sr.drag_delta().x;
            self.panel_width = (panel_w - dx).clamp(MIN_RESULT_PANEL_WIDTH, (content.width() - 12.0).max(MIN_RESULT_PANEL_WIDTH));
        } else {
            self.panel_width = panel_w;
        }
        self.canvas(ui, canvas, live);
        // Under the service editor the sheet keeps its look; the overlay
        // takes the pointer.
        self.sheet(ui, sheet, true);
        if self.editor.is_some() {
            self.services_ui(ui, full);
        }
        if self.options_changed {
            self.options_changed = false;
            if self.target == OptionsTarget::Default {
                cx.settings.DefaultTaskSettings.CaptureSettings.get_or_insert_default().OCROptions = self.options.clone();
            }
        }
        std::mem::take(&mut self.effects)
    }

    /// `TaskHelpers.OCRImage`: the result into `<file>.txt` (UTF-8 with a
    /// BOM, `Encoding.UTF8`) when the window was opened for a file.
    fn closed(&mut self, cx: &mut Cx) -> Vec<Effect> {
        // `SaveWindowState`: the result panel's width.
        if let Some(o) = self.options.as_object_mut() {
            o.insert("ResultPanelWidth".into(), json!(self.panel_width.round() as f64));
            if self.target == OptionsTarget::Default {
                cx.settings.DefaultTaskSettings.CaptureSettings.get_or_insert_default().OCROptions = self.options.clone();
            }
        }
        let result = self.result_text();
        if let Some(path) = &self.file_path
            && !result.is_empty()
        {
            let txt = path.with_extension("txt");
            let mut bytes = vec![0xEF, 0xBB, 0xBF];
            bytes.extend_from_slice(result.as_bytes());
            if let Err(e) = std::fs::write(&txt, bytes) {
                eprintln!("couldn't write {}: {e}", txt.display());
            }
        }
        if let Some(reply) = self.reply.take() {
            let _ = reply.send(());
        }
        Vec::new()
    }

    fn replace_image(&mut self, image: RgbaImage, ctx: &egui::Context) {
        self.replace_source(image, ctx);
    }
}

/// The `--render` screens of the OCR window.
pub fn is_render_screen(name: &str) -> bool {
    name.starts_with("ocr")
}

/// `--render ocr --image <png>`: the window on the image, recognized
/// (`--state services` with the service editor open, `--state busy` while
/// recognizing; `--hover` over a region).
pub fn render_dialog(_name: &str, o: &Options) -> Result<Box<dyn Dialog>, String> {
    let path = o.image.clone().ok_or("--image <png> is required")?;
    let image = image::open(&path).map_err(|e| format!("{}: {e}", path.display()))?.to_rgba8();
    let mut w = OcrWindow::new(image, Some(path), &json!({}), OptionsTarget::Task);
    w.sync = true;
    w.started = true;
    match o.state.as_deref() {
        Some("busy") => {
            w.busy = true;
        }
        _ => w.recognize(&egui::Context::default()),
    }
    match o.state.as_deref() {
        Some("services") => w.editor = Some(ServiceEditor { editing: w.selected_service }),
        // `--count`: the zoom in percent (the preview's centre kept), and
        // with `pan` the image dragged to its top left corner.
        Some("zoom") | Some("pan") => {
            w.zoom = (o.count.max(100) as f32) / 100.0;
            if o.state.as_deref() == Some("pan") {
                w.pan = vec2(1.0e6, 1.0e6);
            }
        }
        // The splitter dragged: the result panel `--count` px wide.
        Some("splitter") => w.panel_width = o.count.max(300) as f32,
        _ => {}
    }
    // A render doesn't write the result beside the image.
    w.file_path = None;
    Ok(Box::new(w))
}

/// What a region capture for OCR goes to.
pub enum OcrRegion {
    /// Tools › OCR / the OCR hotkey: a new window (or the silent path).
    New(Box<sharex_settings::TaskSettings>),
    /// An OCR window's Select region...
    Window(winit::window::WindowId),
    /// The image editor's New from screenshot (hidden, not minimized).
    Shown(winit::window::WindowId),
}

impl crate::App {
    /// `TaskHelpers.OCRImage(taskSettings, autoHideMainWindow)`: a region
    /// capture, then [`Self::ocr_image`].
    pub(crate) fn ocr_capture(&mut self, task: sharex_settings::TaskSettings, auto_hide: bool) {
        if self.region.is_some() || self.transparent.is_some() || self.pending_capture.is_some() {
            return;
        }
        self.ocr_region = Some(OcrRegion::New(Box::new(task.clone())));
        self.capture(crate::capture::CaptureJob::Region, task, auto_hide);
    }

    /// `OCRWindow.SelectRegionAsync`: the window minimized, 250 ms, a region.
    pub(crate) fn ocr_select_region(&mut self, id: winit::window::WindowId) {
        if self.region.is_some() || self.transparent.is_some() || self.pending_capture.is_some() {
            return;
        }
        if let Some(d) = self.dialogs.iter().find(|d| d.window.window.id() == id) {
            d.window.window.set_minimized(true);
        }
        self.ocr_region = Some(OcrRegion::Window(id));
        let task = sharex_settings::TaskSettings::default_task_settings(&self.settings.settings.DefaultTaskSettings);
        self.capture(crate::capture::CaptureJob::Region, task, false);
        if let Some(p) = self.pending_capture.as_mut() {
            p.at += std::time::Duration::from_millis(250);
        }
    }

    /// The image editor's New from screenshot: hidden, captured, shown.
    pub(crate) fn editor_capture(&mut self, id: winit::window::WindowId, fullscreen: bool, task: sharex_settings::TaskSettings) {
        if self.region.is_some() || self.transparent.is_some() || self.pending_capture.is_some() {
            return;
        }
        let Some(d) = self.dialogs.iter_mut().find(|d| d.window.window.id() == id) else { return };
        d.window.window.set_visible(false);
        if fullscreen {
            // `GetScreenshot(taskSettings).CaptureFullscreen()` after the
            // 250 ms the window takes to disappear.
            std::thread::sleep(std::time::Duration::from_millis(250));
            let screens = sharex_capture::screenshot::screens();
            let grab = crate::capture::task_screenshot(&task).capture_fullscreen(&screens);
            d.window.window.set_visible(true);
            d.window.window.focus_window();
            if let Some(image) = grab {
                let ctx = d.window.ctx().clone();
                d.dialog.replace_image(image, &ctx);
            }
            d.window.window.request_redraw();
            return;
        }
        self.ocr_region = Some(OcrRegion::Shown(id));
        self.capture(crate::capture::CaptureJob::Region, task, false);
        if let Some(p) = self.pending_capture.as_mut() {
            p.at += std::time::Duration::from_millis(250);
        }
    }

    /// A region capture for OCR ended (`None`: cancelled).
    pub(crate) fn ocr_region_captured(&mut self, event_loop: &winit::event_loop::ActiveEventLoop, target: OcrRegion, image: Option<RgbaImage>) {
        match target {
            OcrRegion::New(task) => {
                if let Some(image) = image {
                    self.ocr_image(event_loop, image, None, *task);
                }
            }
            OcrRegion::Window(id) | OcrRegion::Shown(id) => {
                let Some(d) = self.dialogs.iter_mut().find(|d| d.window.window.id() == id) else { return };
                d.window.window.set_minimized(false);
                d.window.window.set_visible(true);
                d.window.window.focus_window();
                if let Some(image) = image {
                    let ctx = d.window.ctx().clone();
                    d.dialog.replace_image(image, &ctx);
                }
                d.window.window.request_redraw();
            }
        }
    }

    /// `TaskHelpers.OCRImage(filePath)`: the file loaded, then [`Self::ocr_image`].
    pub(crate) fn ocr_file(&mut self, event_loop: &winit::event_loop::ActiveEventLoop, path: &std::path::Path, task: sharex_settings::TaskSettings) {
        match crate::thumbnail::load(path) {
            Some(image) => self.ocr_image(event_loop, image, Some(path.to_path_buf()), task),
            None => eprintln!("OCR: couldn't load {}", path.display()),
        }
    }

    /// `TaskHelpers.OCRImage(bmp, filePath, taskSettings)`: silently (the
    /// text to the clipboard and `<file>.txt`) or in the OCR window.
    pub(crate) fn ocr_image(&mut self, event_loop: &winit::event_loop::ActiveEventLoop, image: RgbaImage, file_path: Option<PathBuf>, task: sharex_settings::TaskSettings) {
        let options = crate::ocr::OcrOptions::from_task(&task);
        if !options.silent {
            let w = OcrWindow::for_task(image, file_path, &task);
            self.open_dialog_window(event_loop, Box::new(w));
            return;
        }
        // `AsyncOCRImage`.
        let owner = self.clipboard_owner();
        std::thread::spawn(move || {
            let bitmap = sharex_ocr::bitmap_from_rgba(image.width(), image.height(), image.as_raw());
            match sharex_ocr::ocr(&bitmap, options.single_line, options.separate_paragraphs) {
                Ok(text) if !text.is_empty() => {
                    let _ = sharex_capture::clipboard::copy_text(&text, owner);
                    if let Some(path) = file_path {
                        let mut bytes = vec![0xEF, 0xBB, 0xBF];
                        bytes.extend_from_slice(text.as_bytes());
                        let _ = std::fs::write(path.with_extension("txt"), bytes);
                    }
                }
                Ok(_) => {
                    let _ = sharex_capture::clipboard::clear(owner);
                }
                Err(e) => {
                    eprintln!("OCR failed: {e}");
                    return;
                }
            }
            if task.GeneralSettings.as_ref().is_none_or(|g| g.PlaySoundAfterAction) {
                crate::sound::play_action_completed();
            }
        });
    }
}
