//! `ShareX.Tools/Tools/AnalyzeImage/AnalyzeImageWindow`
//! (`AnalyzeImageViewModel`) with `TaskHelpers.ShowAnalyzeImageWindow`'s
//! services: the image (a file, a dropped file, or a region captured with
//! the window minimised), the prompt with its presets, Analyze (the
//! provider asked off the UI thread by `sharex_tools::analyze_image::
//! analyze`, with the elapsed time), the result or the error, Copy all,
//! Options... (`AnalyzeImageOptionsWindow`), the auto-start region and
//! analysis, the auto-copy and the notification sound. The options, and
//! the prompt as it's typed, are written into the task's `AIOptions`.

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Instant;

use egui::{Id, Rect, Ui, Vec2, pos2, vec2};
use sharex_settings::TaskSettings;
use sharex_strings::tools as s;
use sharex_tools::analyze_image::{self as ai, AIOptions};
use sharex_ui::controls::{self, ComboBox, ComboItem, TextBox};
use sharex_ui::keys::app_icons as icon;
use sharex_ui::theme::{Thickness, palette};
use sharex_ui::widgets::{center_y, line_height, text_width};

use crate::dialogs::{Cx, Dialog, Effect, Spec};
use crate::image_tools::{self as tools, Picture};

pub const WIDTH: f32 = 1000.0;
pub const HEIGHT: f32 = 650.0;
/// The left column (`ColumnDefinitions="360,*"`).
const LEFT_W: f32 = 360.0;
/// The prompt box's row (`RowDefinitions="Auto,Auto,110"`).
const PROMPT_H: f32 = 110.0;

/// What asks the provider: the real request, or (tests, `--render`) a stand-in.
pub type Analyzer = Box<dyn Fn(&AIOptions, Option<&str>, Option<&[u8]>) -> Result<Option<String>, String> + Send + Sync>;

/// `AnalyzeImageService.AnalyzeAsync`.
pub fn real_analyzer() -> Analyzer {
    Box::new(|options, path, data| match (path, data) {
        (_, Some(d)) => ai::analyze(options, ai::AnalyzeInput::Data(d)),
        (Some(p), None) => ai::analyze(options, ai::AnalyzeInput::File(std::path::Path::new(p))),
        (None, None) => Ok(Some(String::new())),
    })
}

/// An analysis's answer and how long it took (ms).
type Analysis = (Result<Option<String>, String>, u128);

pub struct AnalyzeImageWindow {
    options: AIOptions,
    task: TaskSettings,
    image_path: Option<String>,
    image_data: Option<Vec<u8>>,
    preview: Option<Picture>,
    pub prompt: String,
    pub selected_preset: Option<usize>,
    pub result_text: String,
    pub elapsed_text: String,
    pub error_message: String,
    pub is_busy: bool,
    busy_since: Instant,
    analyzer: std::sync::Arc<Analyzer>,
    rx: Option<mpsc::Receiver<Analysis>>,
    /// Options the options window saved.
    options_rx: Option<mpsc::Receiver<AIOptions>>,
    /// `OnOpened` → `InitializeAsync` has run.
    initialized: bool,
    /// A region capture is out: analyse once it's in (`analyzeAfterCapture`),
    /// or (opening) when `AutoStartAnalyze`.
    capture_then: Option<bool>,
    options_changed: bool,
    play_sound: bool,
    pub sync: bool,
}

impl AnalyzeImageWindow {
    /// `new AnalyzeImageWindow(imagePath, options, captureRegion, sound)` over
    /// the task's `ToolsSettingsReference.AIOptions`.
    pub fn new(task: TaskSettings, defaults: &TaskSettings, image_path: Option<String>) -> Self {
        let options = AIOptions::from_json(&super::tools_settings(&task, defaults).AIOptions);
        let mut w = Self {
            prompt: options.input.clone().unwrap_or_default(),
            options,
            play_sound: task.GeneralSettings.as_ref().is_none_or(|g| g.PlaySoundAfterAction),
            task,
            image_path: None,
            image_data: None,
            preview: None,
            selected_preset: None,
            result_text: String::new(),
            elapsed_text: String::new(),
            error_message: String::new(),
            is_busy: false,
            busy_since: Instant::now(),
            analyzer: std::sync::Arc::new(real_analyzer()),
            rx: None,
            options_rx: None,
            initialized: false,
            capture_then: None,
            options_changed: false,
            sync: false,
        };
        if let Some(p) = image_path.filter(|p| !p.trim().is_empty()) {
            w.load_image(&p);
        }
        w
    }

    /// Opened as `--render` and the tests draw it: no auto start, no
    /// sound, `analyzer` instead of the provider.
    pub fn set_offline(&mut self, analyzer: Analyzer) {
        self.analyzer = std::sync::Arc::new(analyzer);
        self.initialized = true;
        self.play_sound = false;
        self.sync = true;
    }

    #[cfg(test)]
    pub fn options(&self) -> &AIOptions {
        &self.options
    }

    pub fn set_options(&mut self, options: AIOptions) {
        self.options = options;
        self.options_changed = true;
    }

    pub fn has_image(&self) -> bool {
        self.preview.is_some()
    }

    pub fn has_result(&self) -> bool {
        !self.result_text.trim().is_empty()
    }

    /// `CanAnalyze`.
    pub fn can_analyze(&self) -> bool {
        self.has_image() && self.options.has_api_key() && !self.is_busy
    }

    /// `ImageDescription`.
    pub fn image_description(&self) -> String {
        match &self.image_path {
            Some(p) if !p.trim().is_empty() => sharex_tools::dotnet::file_name(p).to_owned(),
            _ => s::AnalyzeImageViewModel_Captured_region().to_owned(),
        }
    }

    /// `LoadPreview`: the bytes decoded (`new Bitmap(stream)`).
    fn load_preview(&mut self, data: &[u8]) -> bool {
        match image::load_from_memory(data) {
            Ok(img) => {
                self.preview = Some(Picture::new(img.to_rgba8()));
                self.error_message.clear();
                true
            }
            Err(e) => {
                self.error_message = e.to_string();
                false
            }
        }
    }

    /// `LoadImage(filePath)`.
    pub fn load_image(&mut self, path: &str) -> bool {
        match std::fs::read(path) {
            Ok(data) => {
                if self.load_preview(&data) {
                    self.image_path = Some(path.to_owned());
                    self.image_data = None;
                    return true;
                }
            }
            Err(e) => self.error_message = e.to_string(),
        }
        false
    }

    /// `LoadImage(byte[])`: a captured region's PNG.
    pub fn load_image_data(&mut self, data: Vec<u8>) -> bool {
        if !self.load_preview(&data) {
            return false;
        }
        self.image_path = None;
        self.image_data = Some(data);
        true
    }

    /// `AnalyzeAsync`.
    pub fn analyze(&mut self, ctx: &egui::Context) {
        if !self.can_analyze() {
            if self.has_image() && !self.options.has_api_key() {
                self.error_message = s::AnalyzeImageViewModel_Configure_API_key().to_owned();
            }
            return;
        }
        self.is_busy = true;
        self.busy_since = Instant::now();
        self.result_text = s::AnalyzeImageViewModel_Thinking().to_owned();
        self.elapsed_text.clear();
        self.error_message.clear();
        let mut options = self.options.clone();
        options.input = Some(self.prompt.clone());
        let path = self.image_path.clone();
        let data = self.image_data.clone();
        let analyzer = self.analyzer.clone();
        let work = move || {
            let start = Instant::now();
            let r = analyzer(&options, path.as_deref(), data.as_deref());
            (r, start.elapsed().as_millis())
        };
        let (tx, rx) = mpsc::channel();
        if self.sync {
            let _ = tx.send(work());
        } else {
            let ctx = ctx.clone();
            std::thread::spawn(move || {
                let _ = tx.send(work());
                ctx.request_repaint();
            });
        }
        self.rx = Some(rx);
    }

    /// The analysis is in: the result (and the copy, the sound), or the error.
    fn poll(&mut self) -> Vec<Effect> {
        let mut out = Vec::new();
        if let Some(rx) = &self.options_rx
            && let Ok(options) = rx.try_recv()
        {
            self.options_rx = None;
            self.set_options(options);
        }
        let Some(rx) = &self.rx else { return out };
        let Ok((r, ms)) = rx.try_recv() else { return out };
        self.rx = None;
        self.is_busy = false;
        match r {
            Ok(result) => self.set_result(result.as_deref(), ms),
            Err(e) => {
                eprintln!("AnalyzeImageViewModel: Image analysis failed. {e}");
                self.result_text.clear();
                self.error_message = e;
                return out;
            }
        }
        if self.options.auto_copy_result && self.has_result() {
            out.push(Effect::CopyText(self.result_text.clone()));
        }
        if self.has_result() && self.play_sound {
            crate::sound::play_action_completed();
        }
        out
    }

    /// A finished analysis: the result text and `Time_ms`.
    pub fn set_result(&mut self, result: Option<&str>, ms: u128) {
        self.is_busy = false;
        self.result_text = ai::result_text(result);
        self.elapsed_text = super::format(s::AnalyzeImageViewModel_Time_ms(), &[&ms.to_string()]);
    }

    /// An analysis that failed with `message` (`ex.Message`).
    pub fn set_error(&mut self, message: &str) {
        self.is_busy = false;
        self.result_text.clear();
        self.error_message = message.to_owned();
    }

    /// An analysis under way (`--render`).
    pub fn set_busy(&mut self) {
        self.is_busy = true;
        self.result_text = s::AnalyzeImageViewModel_Thinking().to_owned();
        self.elapsed_text.clear();
        self.error_message.clear();
    }

    pub fn preset_items() -> Vec<ComboItem> {
        ai::preset_prompts().iter().map(|p| ComboItem::text(*p)).collect()
    }

    /// The left column's image panel.
    fn image_panel(&mut self, ui: &mut Ui, rect: Rect, out: &mut Vec<Effect>, ctx: &egui::Context) {
        let p = palette(ui.ctx());
        tools::panel(ui, rect);
        let inner = rect.shrink(1.0);
        let hf = tools::heading_font();
        let hh = line_height(ui, &hf);
        let head = Rect::from_min_size(pos2(inner.min.x + 12.0, inner.min.y + 12.0), vec2(inner.width() - 24.0, hh));
        let title = s::AnalyzeImageWindow_Image();
        let tw = text_width(ui, title, hf.clone());
        tools::heading(ui, Rect::from_min_size(head.min, vec2(tw, hh)), title);
        if self.has_image() {
            tools::path_text(ui, Rect::from_min_max(pos2(head.min.x + tw + 8.0, head.min.y), head.max), &self.image_description());
        }
        let by = head.max.y + 8.0;
        let region = s::AnalyzeImageWindow_Select_region();
        let rw = tools::icon_text_button_width(ui, region);
        let rr = Rect::from_min_size(pos2(head.min.x, by), vec2(rw, tools::BUTTON_HEIGHT));
        let pick = s::AnalyzeImageWindow_Select_image();
        let iw = tools::icon_text_button_width(ui, pick);
        let ir = Rect::from_min_size(pos2(rr.max.x + 8.0, by), vec2(iw, tools::BUTTON_HEIGHT));
        let idle = !self.is_busy;
        if tools::icon_text_button(ui, rr, Id::new("ai-select-region"), icon::Crop, region, idle).clicked() && idle {
            self.capture_then = Some(true);
            out.push(Effect::OcrSelectRegion);
        }
        if tools::icon_text_button(ui, ir, Id::new("ai-select-image"), icon::Image, pick, idle).clicked()
            && idle
            && let Some(f) = rfd::FileDialog::new().set_title(s::AnalyzeImageWindow_Select_image_dialog()).add_filter("Images", &tools::IMAGE_EXTENSIONS).pick_file()
            && self.load_image(&f.to_string_lossy())
        {
            self.analyze(ctx);
        }
        // `PreviewHost`: the control background, a 1px border, 4px corners.
        let host = Rect::from_min_max(pos2(inner.min.x + 12.0, rr.max.y + 12.0), pos2(inner.max.x - 12.0, inner.max.y - 12.0));
        sharex_ui::border::border(ui.painter(), host, 4.0, 1.0, Some(sharex_ui::border::Brush::Solid(p.control_background)), Some(p.border));
        let area = host.shrink(1.0 + 8.0);
        if let Some(pic) = self.preview.as_mut() {
            // `Stretch="Uniform"`: fitted either way.
            let dest = crate::image_tools::slider::image_bounds(area, pic.size(), crate::image_tools::slider::Stretch::Both);
            let size = (dest.width().round().max(1.0) as u32, dest.height().round().max(1.0) as u32);
            pic.texture(ui.ctx(), size).paint(&ui.painter().with_clip_rect(host.shrink(1.0)), dest, egui::Color32::WHITE);
            // `OnPreviewPointerPressed`: the image viewer.
            if ui.interact(host, Id::new("ai-preview"), egui::Sense::click()).clicked() {
                out.push(Effect::ShowImageNamed(pic.image.clone(), self.image_description()));
            }
        } else {
            tools::empty_state(ui, host.shrink(1.0), icon::ImageUpload, 42.0, 10.0, &[(s::AnalyzeImageWindow_Select_or_drop_an_image(), tools::normal_font(), p.text_secondary)]);
        }
    }

    /// The left column's prompt panel.
    fn prompt_panel(&mut self, ui: &mut Ui, rect: Rect) {
        tools::panel(ui, rect);
        let inner = rect.shrink(13.0);
        let hh = tools::heading_height(ui);
        tools::heading(ui, Rect::from_min_size(inner.min, vec2(inner.width(), hh)), s::AnalyzeImageWindow_Prompt());
        let items = Self::preset_items();
        let combo = ComboBox { padding: Thickness::hv(8.0, 6.0), enabled: true };
        let placeholder = s::AnalyzeImageWindow_Choose_a_preset_prompt();
        let cw = match self.selected_preset {
            Some(_) => combo.auto_width(ui, &items, self.selected_preset),
            None => combo.placeholder_width(ui, placeholder),
        }
        .min(inner.width());
        let cr = Rect::from_min_size(pos2(inner.min.x, inner.min.y + hh + 9.0), vec2(cw, 40.0));
        if let Some(i) = combo.show_with_placeholder(ui, cr, Id::new("ai-preset"), &items, self.selected_preset, placeholder) {
            // `OnSelectedPresetChanged`: the prompt takes it.
            self.selected_preset = Some(i);
            self.prompt = ai::preset_prompts()[i].to_owned();
            self.options.input = Some(self.prompt.clone());
            self.options_changed = true;
        }
        let pr = Rect::from_min_size(pos2(inner.min.x, cr.max.y + 9.0), vec2(inner.width(), PROMPT_H));
        let before = self.prompt.clone();
        TextBox { multiline: true, wrap: true, ..TextBox::default() }.show(ui, pr, Id::new("ai-prompt"), &mut self.prompt);
        if before != self.prompt {
            // `OnPromptChanged`: `_options.Input = value`.
            self.options.input = Some(self.prompt.clone());
            self.options_changed = true;
        }
    }

    fn prompt_panel_height(ui: &Ui) -> f32 {
        26.0 + tools::grid_height(&[tools::heading_height(ui), 40.0, PROMPT_H], 9.0)
    }

    /// The result panel.
    fn result_panel(&mut self, ui: &mut Ui, rect: Rect, out: &mut Vec<Effect>, ctx: &egui::Context) {
        let p = palette(ui.ctx());
        tools::panel(ui, rect);
        let inner = rect.shrink(1.0);
        let head = Rect::from_min_size(pos2(inner.min.x + 12.0, inner.min.y + 12.0), vec2(inner.width() - 24.0, tools::BUTTON_HEIGHT));
        tools::heading(ui, head, s::AnalyzeImageWindow_Result());
        let copy = s::AnalyzeImageWindow_Copy_all();
        let cw = tools::icon_text_button_width(ui, copy);
        let cr = Rect::from_min_size(pos2(head.max.x - cw, head.min.y), vec2(cw, tools::BUTTON_HEIGHT));
        let opts = s::AnalyzeImageWindow_Options();
        let ow = tools::icon_text_button_width(ui, opts);
        let or = Rect::from_min_size(pos2(cr.min.x - 8.0 - ow, head.min.y), vec2(ow, tools::BUTTON_HEIGHT));
        if tools::icon_text_button(ui, or, Id::new("ai-options"), icon::Settings, opts, !self.is_busy).clicked() {
            // `EditOptionsAsync`: the options window, its Save handed back.
            let (tx, rx) = mpsc::channel();
            self.options_rx = Some(rx);
            out.push(Effect::OpenDialog(Box::new(super::analyze_options::AnalyzeImageOptionsWindow::new(self.options.clone(), Some(tx)))));
        }
        if tools::icon_text_button(ui, cr, Id::new("ai-copy"), icon::Copy, copy, self.has_result()).clicked() && self.has_result() {
            out.push(Effect::CopyText(self.result_text.clone()));
            if self.play_sound {
                crate::sound::play_action_completed();
            }
        }
        let mut top = head.max.y + 12.0;
        if self.is_busy {
            let bar = Rect::from_min_size(pos2(inner.min.x, top), vec2(inner.width(), 4.0));
            controls::indeterminate_progress(ui.painter(), bar, self.busy_since.elapsed().as_secs_f64());
            ui.ctx().request_repaint();
            top += 4.0;
        }
        // The bottom row: the elapsed time and Analyze image.
        let analyze = s::AnalyzeImageWindow_Analyze_image();
        let aw = tools::icon_text_button_width(ui, analyze);
        let bottom = Rect::from_min_max(pos2(inner.min.x + 12.0, inner.max.y - 12.0 - tools::BUTTON_HEIGHT), pos2(inner.max.x - 12.0, inner.max.y - 12.0));
        let ar = Rect::from_min_size(pos2(bottom.max.x - aw, bottom.min.y), vec2(aw, tools::BUTTON_HEIGHT));
        if !self.elapsed_text.is_empty() {
            let f = tools::normal_font();
            tools::text_at(ui, pos2(bottom.min.x, center_y(bottom, line_height(ui, &f))), &self.elapsed_text, f, p.text_secondary);
        }
        if tools::icon_text_button(ui, ar, Id::new("ai-analyze"), icon::Sparkles, analyze, self.can_analyze()).clicked() {
            self.analyze(ctx);
        }
        let body = Rect::from_min_max(pos2(inner.min.x + 12.0, top), pos2(inner.max.x - 12.0, bottom.min.y - 12.0));
        TextBox { read_only: true, multiline: true, wrap: true, font: Some(controls::content_font(15.0, 400)), ..TextBox::default() }.show(
            ui,
            body,
            Id::new("ai-result"),
            &mut self.result_text,
        );
        if !self.error_message.trim().is_empty() {
            // `HasError`: the warning and the message over the result box.
            let f = tools::normal_font();
            let width = 430.0f32.min(body.width());
            let lines = controls::wrap_lines(ui, &self.error_message, &f, width);
            let block_w = lines.iter().map(|l| text_width(ui, l, f.clone())).fold(0.0, f32::max);
            let text_h = crate::video_tools::wrapped_text(ui, pos2(0.0, 0.0), width, &self.error_message, f.clone(), p.text, false);
            let icon_size = 38.0;
            let stack_w = block_w.max(icon_size);
            let total = icon_size + 10.0 + text_h;
            let x0 = body.min.x + ((body.width() - stack_w) / 2.0).round_ties_even();
            let y0 = body.min.y + ((body.height() - total) / 2.0).round_ties_even();
            let ir = Rect::from_min_size(pos2(x0 + ((stack_w - icon_size) / 2.0).round_ties_even(), y0), Vec2::splat(icon_size));
            sharex_ui::icons::paint(ui.painter(), ir, icon::Warning, p.cancel_border);
            let exact = ui.fonts_mut(|fo| fo.row_height(&f));
            let bx = x0 + ((stack_w - block_w) / 2.0).round_ties_even();
            for (i, line) in lines.iter().enumerate() {
                let w = text_width(ui, line, f.clone());
                tools::text_at(ui, pos2(bx + ((block_w - w) / 2.0).round_ties_even(), y0 + icon_size + 10.0 + (exact * i as f32).round()), line, f.clone(), p.text);
            }
        }
    }
}

impl Dialog for AnalyzeImageWindow {
    fn spec(&self) -> Spec {
        Spec::sized(crate::rs_title(s::AnalyzeImageWindow_ShareX_Analyze_image()), WIDTH, HEIGHT, 820.0, 560.0)
    }

    fn ui(&mut self, ui: &mut Ui, cx: &mut Cx) -> Vec<Effect> {
        let ctx = ui.ctx().clone();
        let mut out = self.poll();
        if !self.initialized {
            // `OnOpened` → `InitializeAsync`.
            self.initialized = true;
            if !self.has_image() && self.options.auto_start_region {
                self.capture_then = Some(false);
                out.push(Effect::OcrSelectRegion);
            } else if self.has_image() && self.options.auto_start_analyze {
                self.analyze(&ctx);
            }
        }
        let c = tools::background(ui);
        let left = Rect::from_min_size(c.min, vec2(LEFT_W, c.height()));
        let ph = Self::prompt_panel_height(ui);
        let prompt = Rect::from_min_max(pos2(left.min.x, left.max.y - ph), left.max);
        let image = Rect::from_min_max(left.min, pos2(left.max.x, prompt.min.y - 12.0));
        self.image_panel(ui, image, &mut out, &ctx);
        self.prompt_panel(ui, prompt);
        let right = Rect::from_min_max(pos2(left.max.x + 12.0, c.min.y), c.max);
        self.result_panel(ui, right, &mut out, &ctx);
        if std::mem::take(&mut self.options_changed) {
            let json = self.options.to_json();
            super::save_tools_setting(cx, &mut self.task, |t| t.AIOptions = json.clone());
        }
        out.extend(self.poll());
        out
    }

    fn replace_image(&mut self, image: image::RgbaImage, ctx: &egui::Context) {
        // The captured region as PNG bytes (`region.Save(stream, Png)`).
        let then = self.capture_then.take();
        let mut png = Vec::new();
        if image::DynamicImage::ImageRgba8(image).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).is_err() {
            return;
        }
        if self.load_image_data(png) {
            match then {
                Some(true) => self.analyze(ctx),
                Some(false) if self.options.auto_start_analyze => self.analyze(ctx),
                _ => {}
            }
        }
    }

    fn dropped(&mut self, files: Vec<PathBuf>, _cx: &mut Cx) -> Vec<Effect> {
        // `OnDrop`: loaded, not analysed.
        if !self.is_busy
            && let Some(f) = files.iter().find(|f| f.is_file())
        {
            self.load_image(&f.to_string_lossy());
        }
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sharexrs-analyze-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("a.png");
        image::RgbaImage::from_pixel(8, 6, image::Rgba([10, 20, 30, 255])).save(&p).unwrap();
        p
    }

    #[test]
    fn analyzes_with_a_stand_in_provider() {
        let defaults = TaskSettings::default();
        let task = TaskSettings::default_task_settings(&defaults);
        let path = png();
        let mut w = AnalyzeImageWindow::new(task, &defaults, Some(path.to_string_lossy().into_owned()));
        w.set_offline(Box::new(|o, path, data| {
            assert!(path.is_some() && data.is_none());
            Ok(Some(format!("seen: {}\nline", o.input.clone().unwrap_or_default())))
        }));
        assert!(w.has_image());
        assert_eq!(w.image_description(), "a.png");
        // No key: the error, no request.
        let ctx = egui::Context::default();
        w.analyze(&ctx);
        assert_eq!(w.error_message, s::AnalyzeImageViewModel_Configure_API_key());
        let mut o = w.options().clone();
        o.openai_api_key.set(Some("test-key".into()));
        w.set_options(o);
        assert!(w.can_analyze());
        w.analyze(&ctx);
        let _ = w.poll();
        assert!(w.error_message.is_empty());
        assert_eq!(w.result_text, format!("seen: {}\r\nline", s::AIOptions_What_is_in_this_image()));
        assert!(w.elapsed_text.ends_with("ms"), "{}", w.elapsed_text);
        // An API error.
        w.set_offline(Box::new(|_, _, _| Err("Response status code does not indicate success: 401 (Unauthorized).".into())));
        w.analyze(&ctx);
        let _ = w.poll();
        assert!(w.result_text.is_empty());
        assert!(w.error_message.contains("401"));
    }

    #[test]
    fn a_captured_region_is_png_data() {
        let defaults = TaskSettings::default();
        let task = TaskSettings::default_task_settings(&defaults);
        let mut w = AnalyzeImageWindow::new(task, &defaults, None);
        w.set_offline(Box::new(|_, path, data| {
            assert!(path.is_none() && data.is_some_and(|d| d.starts_with(b"\x89PNG")));
            Ok(Some("ok".into()))
        }));
        w.capture_then = Some(true);
        w.replace_image(image::RgbaImage::from_pixel(3, 3, image::Rgba([1, 1, 1, 255])), &egui::Context::default());
        assert!(w.has_image());
        assert_eq!(w.image_description(), s::AnalyzeImageViewModel_Captured_region());
    }
}
