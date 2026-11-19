//! `ShareX.Tools/Tools/BackgroundRemover` (`BackgroundRemoverWindow.axaml`,
//! `BackgroundRemoverViewModel` with its notifications,
//! `TaskHelpers.OpenBackgroundRemover`): the image row (path, Browse...),
//! the models row (the models folder's `*.onnx` files, Refresh, Open
//! models folder, the guide link highlighted while there's no model, the
//! device), Remove Background on a thread (`BackgroundRemovalService` on
//! ONNX Runtime), the `ImageComparisonSlider` between the image and the
//! result, Save / Save as..., and the toast. `BackgroundRemoverOptions`
//! (the model's file name, the device) are written into the task's
//! `ToolsSettingsReference` as they change. No model ships with ShareX or
//! ShareXrs: the user puts one in the models folder.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Instant;

use egui::{Color32, Id, Rect, Ui, pos2, vec2};
use image::RgbaImage;
use sharex_settings::TaskSettings;
use sharex_strings::tools as s;
use sharex_tools::background_remover::{self as engine, BackgroundRemovalService, BackgroundRemoverOptions, Device, Model};
use sharex_tools::imaging::{self, Bitmap, ImageFileFormat};
use sharex_ui::border::{self, Brush};
use sharex_ui::controls::{ComboBox, ComboItem};
use sharex_ui::icons;
use sharex_ui::keys::app_icons as icon;
use sharex_ui::theme::{Thickness, fluent, palette};
use sharex_ui::widgets::{center_y, font, line_height, text_width};

use super::Picture;
use super::slider::ComparisonSlider;
use crate::dialogs::{Cx, Dialog, Effect, Spec};
use crate::image_editor::notify::{Notifications, Placement};

pub const WIDTH: f32 = 1080.0;
pub const HEIGHT: f32 = 720.0;
pub const MIN_WIDTH: f32 = 760.0;
pub const MIN_HEIGHT: f32 = 520.0;
/// `BackgroundRemoverGuideUrl`.
pub const GUIDE_URL: &str = "https://getsharex.com/docs/background-remover";
/// The panels' `Padding="12"` and the rows' spacing.
const PADDING: f32 = 12.0;
/// `Button.guide-highlight`'s animation: opacity 0.4 → 1 → 0.4 over 1.2 s.
const GUIDE_PERIOD: f64 = 1.2;

/// What the removal thread hands back: the result and the elapsed time
/// (the view model's stopwatch: decoding the file again included).
type Outcome = Result<(Bitmap, u128), String>;

pub struct BackgroundRemoverWindow {
    /// `ModelsFolder` (`AppPaths.ModelsFolder`).
    models_folder: Option<PathBuf>,
    pub options: BackgroundRemoverOptions,
    task: TaskSettings,
    save_to_defaults: bool,
    options_changed: bool,
    pub models: Vec<Model>,
    pub selected_model: Option<usize>,
    pub device: Device,
    /// `ImagePath`.
    pub image_path: Option<String>,
    source: Option<Picture>,
    /// `_resultBitmap` and its preview.
    result: Option<(Bitmap, Picture)>,
    pub slider: ComparisonSlider,
    /// `IsGuideHighlighted`.
    guide_highlighted: bool,
    /// `IsProcessing`.
    pub processing: bool,
    service: Arc<Mutex<BackgroundRemovalService>>,
    job: Option<mpsc::Receiver<Outcome>>,
    pub notify: Notifications,
    /// Runs the removal on this thread (`--render`).
    pub sync: bool,
    /// The guide button's opacity, fixed (`--render`: the animation's
    /// phase in the shot); animated otherwise.
    pub guide_opacity: Option<f32>,
    /// A combo box to open on the next frame (`--render` states).
    pub open_combo: Option<&'static str>,
    /// The notification's elapsed time, fixed (`--render`).
    pub elapsed_ms: Option<u128>,
}

impl BackgroundRemoverWindow {
    /// `new BackgroundRemoverWindow(modelsFolder, options)` over the task's
    /// `ToolsSettingsReference.BackgroundRemoverOptions`.
    pub fn new(models_folder: Option<PathBuf>, task: TaskSettings, defaults: &TaskSettings) -> Self {
        let save_to_defaults = task.UseDefaultToolsSettings;
        let tools = if save_to_defaults { defaults.ToolsSettings.as_ref() } else { task.ToolsSettings.as_ref() };
        let options = tools.map(|t| BackgroundRemoverOptions::from_json(&t.BackgroundRemoverOptions)).unwrap_or_default();
        let mut w = Self {
            models_folder,
            device: options.selected_device,
            options,
            task,
            save_to_defaults,
            // `_options.SelectedDevice = SelectedDevice` (an undefined value
            // becomes Auto).
            options_changed: true,
            models: Vec::new(),
            selected_model: None,
            image_path: None,
            source: None,
            result: None,
            slider: ComparisonSlider::default(),
            guide_highlighted: true,
            processing: false,
            service: Arc::new(Mutex::new(BackgroundRemovalService::new())),
            job: None,
            notify: Notifications::default(),
            sync: false,
            guide_opacity: None,
            open_combo: None,
            elapsed_ms: None,
        };
        w.refresh_models();
        w
    }

    /// `HasImage`.
    fn has_image(&self) -> bool {
        self.image_path.as_deref().is_some_and(|p| !p.is_empty())
    }

    /// `CanRemoveBackground`.
    pub fn can_remove_background(&self) -> bool {
        self.has_image() && self.selected_model.is_some() && !self.processing
    }

    /// `CanSaveImage` (`IsSaving` lasts only while the dialog is open, which
    /// blocks this window).
    pub fn can_save(&self) -> bool {
        self.result.is_some() && !self.processing
    }

    /// `RefreshModels`: the saved (or current) model kept when it's still
    /// there, else the first; the guide highlighted without models.
    pub fn refresh_models(&mut self) {
        let keep = self.selected_model.and_then(|i| self.models.get(i)).map(|m| m.file_name.clone()).unwrap_or_else(|| self.options.selected_model_file_name.clone());
        self.models.clear();
        self.selected_model = None;
        self.guide_highlighted = true;
        let Some(folder) = self.models_folder.as_ref().filter(|f| !f.as_os_str().is_empty()) else { return };
        self.models = engine::scan_models(folder);
        self.set_model(engine::select_model(&self.models, &keep));
        self.guide_highlighted = self.models.is_empty();
    }

    /// `SelectedModel` set (`OnSelectedModelChanged`: a model's name saved).
    fn set_model(&mut self, index: Option<usize>) {
        self.selected_model = index;
        if let Some(m) = index.and_then(|i| self.models.get(i)) {
            self.options.selected_model_file_name = m.file_name.clone();
            self.options_changed = true;
        }
    }

    /// `SelectedDevice` set.
    fn set_device(&mut self, device: Device) {
        self.device = device;
        self.options.selected_device = device;
        self.options_changed = true;
    }

    /// `LoadImage(filePath)`: `SKBitmap.Decode`, nothing when it fails.
    pub fn load_image(&mut self, path: &str) {
        if self.processing || path.trim().is_empty() {
            return;
        }
        match engine::load_image(Path::new(path)) {
            Some(bmp) => self.set_source_image(&bmp, path),
            None => eprintln!("BackgroundRemoverViewModel: Failed to load image '{path}'."),
        }
    }

    /// `SetSourceImage`: the result goes.
    fn set_source_image(&mut self, bmp: &Bitmap, path: &str) {
        self.source = Some(Picture::new(to_rgba(bmp)));
        self.result = None;
        self.image_path = Some(path.to_string());
    }

    /// `SetResultImage`: the divider moves to the left edge.
    fn set_result_image(&mut self, bmp: Bitmap) {
        let pic = Picture::new(to_rgba(&bmp));
        self.result = Some((bmp, pic));
        self.slider.position = 0.0;
    }

    /// `RemoveBackgroundAsync`: the file decoded again and shown, then the
    /// model run off the UI thread.
    pub fn remove_background(&mut self, ctx: &egui::Context) {
        if !self.can_remove_background() {
            return;
        }
        let (Some(path), Some(model)) = (self.image_path.clone(), self.selected_model.and_then(|i| self.models.get(i)).cloned()) else { return };
        let started = Instant::now();
        let Some(source) = engine::load_image(Path::new(&path)) else { return };
        self.processing = true;
        self.set_source_image(&source, &path);
        let device = self.device;
        let service = self.service.clone();
        let work = move || -> Outcome {
            let mut service = service.lock().map_err(|e| e.to_string())?;
            let r = service.remove_background(&source, &model, device)?;
            eprintln!(
                "Background removal (device={}, execution={}, model={}, {}): session={} ms, preprocess={} ms, inference={} ms, postprocess={} ms",
                device.name(),
                r.execution_device,
                model.file_name,
                if r.session_cached { "cached" } else { "not cached" },
                r.session_ms,
                r.preprocessing_ms,
                r.inference_ms,
                r.postprocessing_ms
            );
            Ok((r.image, started.elapsed().as_millis()))
        };
        if self.sync {
            let out = work();
            self.finish(out);
            return;
        }
        let (tx, rx) = mpsc::channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(work());
            ctx.request_repaint();
        });
        self.job = Some(rx);
    }

    fn finish(&mut self, out: Outcome) {
        self.processing = false;
        self.job = None;
        match out {
            Ok((bmp, ms)) => {
                self.set_result_image(bmp);
                let ms = self.elapsed_ms.unwrap_or(ms);
                let text = s::BackgroundRemoverViewModel_Background_removed_ms().replace("{0}", &ms.to_string());
                self.notify.show(true, text, Some(icon::Eraser));
            }
            // `ToolsDiagnostics.ReportWarning`: no message box.
            Err(e) => eprintln!("BackgroundRemoverViewModel: Background removal failed. {e}"),
        }
    }

    /// `SaveAsync` / `SaveAsAsync`: Save writes over a PNG or WebP source,
    /// otherwise (and for Save as...) asks where.
    fn save(&mut self, ask: bool) {
        if !self.can_save() {
            return;
        }
        let Some((bmp, _)) = &self.result else { return };
        let source = self.image_path.clone();
        let ext = source.as_deref().map(|p| sharex_tools::dotnet::extension(p).to_lowercase()).unwrap_or_default();
        let target = match source.as_deref() {
            Some(p) if !ask && !p.trim().is_empty() && (ext == ".png" || ext == ".webp") => Some(p.to_string()),
            _ => save_dialog(source.as_deref()),
        };
        let Some(target) = target else { return };
        match std::fs::write(&target, encode_result(bmp, &target)) {
            Ok(()) => {
                let text = s::BackgroundRemoverViewModel_Image_saved_file_path().replace("{0}", &target);
                self.notify.show(true, text, Some(if ask { icon::SaveAs } else { icon::Save }));
            }
            Err(e) => eprintln!("BackgroundRemoverViewModel: Failed to save background removal result. {e}"),
        }
    }

    /// `OpenModelsFolder`: created, opened, then the list refreshed.
    fn open_models_folder(&mut self) -> Vec<Effect> {
        let Some(folder) = self.models_folder.clone().filter(|f| !f.as_os_str().is_empty()) else { return Vec::new() };
        if let Err(e) = std::fs::create_dir_all(&folder) {
            eprintln!("BackgroundRemoverViewModel: Failed to open models folder. {e}");
            return Vec::new();
        }
        self.refresh_models();
        vec![Effect::OpenFile(folder)]
    }

    fn guide_opacity(&self, ui: &Ui) -> f32 {
        if let Some(o) = self.guide_opacity {
            return o;
        }
        ui.ctx().request_repaint();
        let t = (ui.input(|i| i.time) % GUIDE_PERIOD) / GUIDE_PERIOD;
        // 0% 0.4, 50% 1, 100% 0.4, linear between.
        let k = if t < 0.5 { t * 2.0 } else { (1.0 - t) * 2.0 };
        (0.4 + 0.6 * k) as f32
    }

    /// The top panel: the image row and the models row.
    fn top_panel(&mut self, ui: &Ui, rect: Rect) -> Vec<Effect> {
        let mut effects = Vec::new();
        super::panel(ui, rect);
        let inner = rect.shrink(1.0 + PADDING);
        let label_font = super::semibold_font();
        let row0 = Rect::from_min_size(inner.min, vec2(inner.width(), super::BUTTON_HEIGHT));
        let row1 = Rect::from_min_size(pos2(inner.min.x, row0.max.y + 8.0), vec2(inner.width(), super::BUTTON_HEIGHT));
        let p = palette(ui.ctx());

        // Row 0: `Image`, the path, Browse...
        let image_label = s::BackgroundRemoverWindow_Image();
        let lw = text_width(ui, image_label, label_font.clone());
        sharex_ui::controls::label(ui, Rect::from_min_size(row0.min, vec2(lw, row0.height())), image_label, label_font.clone(), p.text);
        let browse = s::BackgroundRemoverWindow_Browse();
        let bw = super::text_button_width(ui, browse);
        let br = Rect::from_min_size(pos2(row0.max.x - bw, row0.min.y), vec2(bw, row0.height()));
        // `TargetNullValue='No image selected'`.
        let path = self.image_path.clone().unwrap_or_else(|| "No image selected".into());
        super::path_text(ui, Rect::from_min_max(pos2(row0.min.x + lw + 12.0, row0.min.y), pos2(br.min.x - 12.0, row0.max.y)), &path);
        if super::text_button(ui, br, Id::new("bgremover-browse"), browse, !self.processing).clicked()
            && let Some(f) = super::pick_image(s::BackgroundRemoverViewModel_Select_image_dialog())
        {
            self.load_image(&f);
        }

        // Row 1: `Models`, [combo, refresh, folder, guide, Device, device combo], Remove Background.
        let models_label = s::BackgroundRemoverWindow_Models();
        let mw = text_width(ui, models_label, label_font.clone());
        sharex_ui::controls::label(ui, Rect::from_min_size(row1.min, vec2(mw, row1.height())), models_label, label_font.clone(), p.text);
        let remove = s::BackgroundRemoverWindow_Remove_Background();
        let rw = super::text_button_width(ui, remove);
        let rr = Rect::from_min_size(pos2(row1.max.x - rw, row1.min.y), vec2(rw, row1.height()));
        if super::text_button(ui, rr, Id::new("bgremover-remove"), remove, self.can_remove_background()).clicked() {
            self.remove_background(ui.ctx());
        }
        let mid = Rect::from_min_max(pos2(row1.min.x + mw + 12.0, row1.min.y), pos2(rr.min.x - 12.0, row1.max.y));
        // Right to left: the device combo (`MinWidth="88"`), `Device` with
        // its 8px margins, the three 40px buttons, 4px apart.
        let combo = ComboBox { padding: Thickness::hv(8.0, 6.0), enabled: !self.processing };
        let device_items: Vec<ComboItem> = Device::ALL.iter().map(|d| ComboItem::text(device_text(*d))).collect();
        let device_index = Device::ALL.iter().position(|d| *d == self.device);
        let dw = combo.auto_width(ui, &device_items, device_index).max(88.0);
        let dr = Rect::from_min_size(pos2(mid.max.x - dw, mid.min.y), vec2(dw, mid.height()));
        let device_label = s::BackgroundRemoverWindow_Device();
        let dlw = text_width(ui, device_label, label_font.clone());
        let dlr = Rect::from_min_size(pos2(dr.min.x - 4.0 - 8.0 - dlw, mid.min.y), vec2(dlw, mid.height()));
        sharex_ui::controls::label(ui, dlr, device_label, label_font, p.text);
        let b = super::BUTTON_HEIGHT;
        let guide = Rect::from_min_size(pos2(dlr.min.x - 8.0 - 4.0 - b, mid.min.y), vec2(b, b));
        let folder = guide.translate(vec2(-(b + 4.0), 0.0));
        let refresh = folder.translate(vec2(-(b + 4.0), 0.0));
        let model_rect = Rect::from_min_max(mid.min, pos2(refresh.min.x - 4.0, mid.max.y));

        let device_id = Id::new("bgremover-device");
        let model_id = Id::new("bgremover-model");
        match self.open_combo.take() {
            Some("model") => ComboBox::set_open(ui.ctx(), model_id, true),
            Some("device") => ComboBox::set_open(ui.ctx(), device_id, true),
            _ => {}
        }
        // `ToolTip.Tip`s (a disabled control gets no pointer events, so no tip).
        let can_open = !self.processing && self.models_folder.as_ref().is_some_and(|f| !f.as_os_str().is_empty());
        let tip = |rect: Rect, key: &str, text: &str, enabled: bool| {
            if enabled {
                sharex_ui::tooltip::show(ui, rect, Id::new(("bgremover-tip", key)), text);
            }
        };
        tip(model_rect, "model", s::BackgroundRemoverWindow_Background_removal_model(), !self.processing);
        tip(refresh, "refresh", s::BackgroundRemoverWindow_Refresh_models(), !self.processing);
        tip(folder, "folder", s::BackgroundRemoverWindow_Open_models_folder(), can_open);
        tip(guide, "guide", s::BackgroundRemoverWindow_Open_guide_page(), true);
        tip(dr, "device", s::BackgroundRemoverWindow_Processing_device(), !self.processing);
        let model_items: Vec<ComboItem> = self.models.iter().map(|m| ComboItem::text(m.display_name())).collect();
        if let Some(i) = combo.show(ui, model_rect, model_id, &model_items, self.selected_model) {
            self.set_model(Some(i));
        }
        if super::icon_button_sized(ui, refresh, Id::new("bgremover-refresh"), icon::Refresh, 17.0, !self.processing).clicked() {
            self.refresh_models();
        }
        if super::icon_button_sized(ui, folder, Id::new("bgremover-folder"), icon::FolderOpen, 17.0, can_open).clicked() {
            effects.extend(self.open_models_folder());
        }
        let r = if self.guide_highlighted { self.guide_button(ui, guide) } else { super::icon_button_sized(ui, guide, Id::new("bgremover-guide"), icon::Help, 17.0, true) };
        if r.clicked() {
            effects.push(Effect::OpenUrl(GUIDE_URL.into()));
        }
        if let Some(i) = combo.show(ui, dr, device_id, &device_items, device_index) {
            self.set_device(Device::ALL[i]);
        }
        effects
    }

    /// `Button.guide-highlight`: the accent fill and border, the icon in
    /// `Accent.Foreground`, the whole button pulsing.
    fn guide_button(&self, ui: &Ui, rect: Rect) -> egui::Response {
        let p = palette(ui.ctx());
        let r = ui.interact(rect, Id::new("bgremover-guide"), egui::Sense::click());
        let opacity = self.guide_opacity(ui);
        let behind = p.background_panel;
        let fill = Brush::accent(ui.ctx());
        let shell = border::Part::Border { rect, radius: 4.0, thickness: 1.0, background: Some(fill), border: Some(p.accent_border) };
        border::paint(ui.painter(), &[shell], opacity);
        let mid = Color32::from_rgb(
            ((p.accent_start.r() as u16 + p.accent_end.r() as u16) / 2) as u8,
            ((p.accent_start.g() as u16 + p.accent_end.g() as u16) / 2) as u8,
            ((p.accent_start.b() as u16 + p.accent_end.b() as u16) / 2) as u8,
        );
        let color = border::group_content(p.accent_foreground, mid, opacity, behind);
        let inner = rect.shrink(1.0);
        let b = Rect::from_min_size(pos2(inner.min.x + ((inner.width() - 17.0) / 2.0).round_ties_even(), center_y(inner, 17.0)), vec2(17.0, 17.0));
        icons::paint(ui.painter(), b, icon::Help, color);
        r
    }
}

/// `LocalizedDisplayConverter` for a device.
pub fn device_text(d: Device) -> &'static str {
    match d {
        Device::Auto => s::LocalizedDisplayConverter_Automatic(),
        Device::Gpu => s::LocalizedDisplayConverter_GPU(),
        Device::Cpu => s::LocalizedDisplayConverter_CPU(),
    }
}

/// `BitmapConversionHelpers.ToAvaloniBitmap`.
fn to_rgba(bmp: &Bitmap) -> RgbaImage {
    RgbaImage::from_raw(bmp.width(), bmp.height(), bmp.to_straight()).unwrap_or_default()
}

/// `SaveImageToFile`: WebP for `.webp` (any case), PNG otherwise, through
/// `SKImage.Encode(format, 100)`.
pub fn encode_result(bmp: &Bitmap, path: &str) -> Vec<u8> {
    let webp = sharex_tools::dotnet::extension(path).eq_ignore_ascii_case(".webp");
    imaging::encode(bmp, if webp { ImageFileFormat::Webp } else { ImageFileFormat::Png }, 100)
}

/// `SaveImageAsAsync`'s suggested name: `<name>-output.png`, or
/// `image-output.png` without a source.
pub fn suggested_file_name(source: Option<&str>) -> String {
    match source.filter(|p| !p.trim().is_empty()) {
        Some(p) => format!("{}-output.png", Path::new(p).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()),
        None => "image-output.png".into(),
    }
}

/// The save dialog: PNG or WebP, `png` by default.
fn save_dialog(source: Option<&str>) -> Option<String> {
    let mut d = rfd::FileDialog::new()
        .set_title(s::BackgroundRemoverWindow_Save_image_as_dialog())
        .set_file_name(suggested_file_name(source))
        .add_filter("PNG", &["png"])
        .add_filter("WebP", &["webp"]);
    if let Some(dir) = source.map(Path::new).and_then(Path::parent).filter(|d| d.is_dir()) {
        d = d.set_directory(dir);
    }
    let path = d.save_file()?;
    // `DefaultExtension = "png"`.
    let path = if path.extension().is_none() { path.with_extension("png") } else { path };
    Some(path.to_string_lossy().into_owned())
}

impl Dialog for BackgroundRemoverWindow {
    fn spec(&self) -> Spec {
        Spec::sized(crate::rs_title(s::BackgroundRemoverWindow_ShareX_Background_Remover()), WIDTH, HEIGHT, MIN_WIDTH, MIN_HEIGHT)
    }

    fn ui(&mut self, ui: &mut Ui, cx: &mut Cx) -> Vec<Effect> {
        if let Some(rx) = &self.job
            && let Ok(out) = rx.try_recv()
        {
            self.finish(out);
        }
        if self.processing {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Wait);
        }
        let p = palette(ui.ctx());
        let full = ui.max_rect();
        let c = super::background(ui);
        let top_h = 2.0 + PADDING * 2.0 + super::BUTTON_HEIGHT * 2.0 + 8.0;
        let top = Rect::from_min_size(c.min, vec2(c.width(), top_h));
        let bottom = Rect::from_min_max(pos2(c.min.x, c.max.y - super::BUTTON_HEIGHT), c.max);
        let body = Rect::from_min_max(pos2(c.min.x, top.max.y + 12.0), pos2(c.max.x, bottom.min.y - 12.0));

        // The preview panel (`ClipToBounds`).
        super::panel(ui, body);
        let area = body.shrink(1.0);
        match (&mut self.source, &mut self.result) {
            (Some(src), Some((_, res))) => self.slider.show(ui, area.shrink(PADDING), Id::new("bgremover-slider"), src, res, true),
            (Some(src), None) => ComparisonSlider::draw_single(ui, area.shrink(PADDING), src, super::slider::Stretch::Both),
            _ => {
                let hint = s::BackgroundRemoverWindow_Select_an_image_to_preview_it_here();
                let hf = font(16.0, 400);
                let hw = text_width(ui, hint, hf.clone());
                let lh = line_height(ui, &hf);
                let pos = pos2(area.min.x + ((area.width() - hw) / 2.0).round_ties_even(), area.min.y + ((area.height() - lh) / 2.0).round_ties_even());
                super::text_at(ui, pos, hint, hf, p.text);
            }
        }

        // The buttons, right-aligned, 8 apart; drawn before the top panel
        // so its dropdowns go over them.
        let can_save = self.can_save();
        let save_as = s::BackgroundRemoverWindow_Save_as();
        let saw = super::icon_text_button_width(ui, save_as);
        let sar = Rect::from_min_size(pos2(bottom.max.x - saw, bottom.min.y), vec2(saw, super::BUTTON_HEIGHT));
        let save = s::BackgroundRemoverWindow_Save();
        let sw = super::icon_text_button_width(ui, save);
        let sr = Rect::from_min_size(pos2(sar.min.x - 8.0 - sw, bottom.min.y), vec2(sw, super::BUTTON_HEIGHT));
        if super::icon_text_button(ui, sr, Id::new("bgremover-save"), icon::Save, save, can_save).clicked() {
            self.save(false);
        }
        if super::icon_text_button(ui, sar, Id::new("bgremover-saveas"), icon::SaveAs, save_as, can_save).clicked() {
            self.save(true);
        }

        let effects = self.top_panel(ui, top);

        // The toast: centred, `Margin="16,16,16,68"`, `MaxWidth="640"`.
        let f = fluent(ui.ctx());
        self.notify.paint_at(ui, &p, full, Placement { bottom: 60.0, max_width: Some(640.0), icon_color: f.base_high });

        if std::mem::take(&mut self.options_changed) {
            let json = self.options.to_json();
            if self.save_to_defaults {
                cx.settings.DefaultTaskSettings.ToolsSettings.get_or_insert_default().BackgroundRemoverOptions = json.clone();
            }
            if let Some(t) = self.task.ToolsSettings.as_mut() {
                t.BackgroundRemoverOptions = json;
            }
        }
        effects
    }

    /// `OnDrop`: the first file, unless it's busy.
    fn dropped(&mut self, files: Vec<PathBuf>, _cx: &mut Cx) -> Vec<Effect> {
        if !self.processing
            && let Some(f) = files.into_iter().find(|f| f.is_file())
        {
            self.load_image(&f.to_string_lossy());
        }
        Vec::new()
    }
}

impl crate::App {
    /// `TaskHelpers.OpenBackgroundRemover(taskSettings)`: the models folder
    /// and the task's options.
    pub(crate) fn open_background_remover(&mut self, event_loop: &winit::event_loop::ActiveEventLoop, task: TaskSettings) {
        let folder = self.settings.paths.models_folder();
        let w = BackgroundRemoverWindow::new(Some(folder), task, &self.settings.settings.DefaultTaskSettings);
        self.open_dialog_window(event_loop, Box::new(w));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../sharex-tools/tests/fixtures").join(name)
    }

    fn temp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("sharexrs-bgremover-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn models_saved_choice_and_guide() {
        let dir = temp("models");
        let defaults = TaskSettings::default();
        let task = TaskSettings::default_task_settings(&defaults);
        let w = BackgroundRemoverWindow::new(Some(dir.clone()), task.clone(), &defaults);
        assert!(w.models.is_empty() && w.selected_model.is_none() && w.guide_highlighted);
        assert!(!w.can_remove_background());
        for n in ["b.onnx", "a.onnx", "c.txt"] {
            std::fs::write(dir.join(n), b"x").unwrap();
        }
        let mut w = BackgroundRemoverWindow::new(Some(dir.clone()), task.clone(), &defaults);
        assert_eq!(w.models.iter().map(|m| m.file_name.as_str()).collect::<Vec<_>>(), ["a.onnx", "b.onnx"]);
        assert_eq!(w.selected_model, Some(0));
        assert!(!w.guide_highlighted);
        assert_eq!(w.options.selected_model_file_name, "a.onnx");
        // The saved name is kept across a refresh, ignoring case.
        w.options.selected_model_file_name = "B.ONNX".into();
        w.selected_model = None;
        w.refresh_models();
        assert_eq!(w.selected_model, Some(1));
        // The current model wins over the saved name.
        w.set_model(Some(0));
        w.refresh_models();
        assert_eq!(w.selected_model, Some(0));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn options_read_and_device_saved() {
        let mut defaults = TaskSettings::default();
        defaults.ToolsSettings.get_or_insert_default().BackgroundRemoverOptions = serde_json::json!({"SelectedModelFileName": "m.onnx", "SelectedDevice": "CPU"});
        let task = TaskSettings::default_task_settings(&defaults);
        let mut w = BackgroundRemoverWindow::new(None, task, &defaults);
        assert_eq!(w.device, Device::Cpu);
        assert_eq!(w.options.selected_model_file_name, "m.onnx");
        w.set_device(Device::Gpu);
        assert_eq!(w.options.to_json()["SelectedDevice"], "GPU");
    }

    #[test]
    fn loads_an_image_and_names_the_output() {
        let defaults = TaskSettings::default();
        let mut w = BackgroundRemoverWindow::new(None, TaskSettings::default_task_settings(&defaults), &defaults);
        let png = fixture("in-alpha_64x48.png");
        w.load_image(&png.to_string_lossy());
        assert!(w.has_image());
        assert_eq!(w.source.as_ref().map(|p| p.size()), Some((64, 48)));
        // A file that doesn't decode leaves the image as it was.
        w.load_image(&fixture("in-bgmodels-a-nchw-sigmoid.onnx").to_string_lossy());
        assert_eq!(w.image_path.as_deref(), Some(png.to_string_lossy().as_ref()));
        // A path in this platform's form (a backslash isn't a separator off
        // Windows, for .NET's Path either).
        let photo = std::path::Path::new("x").join("photo.jpg");
        assert_eq!(suggested_file_name(Some(&photo.to_string_lossy())), "photo-output.png");
        assert_eq!(suggested_file_name(None), "image-output.png");
        assert!(!w.can_save());
    }

    #[test]
    fn result_saved_as_png_or_webp() {
        let bmp = Bitmap::from_premultiplied(2, 1, vec![128, 0, 0, 128, 0, 0, 0, 0]);
        let png = encode_result(&bmp, "x/out.PNG");
        assert_eq!(&png[1..4], b"PNG");
        let webp = encode_result(&bmp, "x/out.WebP");
        assert_eq!(&webp[8..12], b"WEBP");
    }

    #[test]
    fn gpu_device_reports_its_error() {
        let dir = temp("gpu");
        std::fs::copy(fixture("in-bgmodels-a-nchw-sigmoid.onnx"), dir.join("a.onnx")).unwrap();
        let defaults = TaskSettings::default();
        let mut w = BackgroundRemoverWindow::new(Some(dir.clone()), TaskSettings::default_task_settings(&defaults), &defaults);
        w.sync = true;
        w.load_image(&fixture("in-alpha_64x48.png").to_string_lossy());
        w.set_device(Device::Gpu);
        w.remove_background(&egui::Context::default());
        // The failure is only logged (`ReportWarning`): no result, not busy.
        assert!(!w.processing && w.result.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
