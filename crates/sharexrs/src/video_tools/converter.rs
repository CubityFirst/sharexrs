//! `ShareX.Tools/Tools/VideoConverter` (`VideoConverterWindow.axaml`,
//! `VideoConverterViewModel`) and `TaskHelpers.RunVideoConversionAsync`:
//! the input file, the encoder and its CRF or bitrate, the output folder
//! and name, Start/Stop encoding with FFmpeg's progress, and the result
//! (`GetVideoConversionOutputInfo`). The options live in the task's
//! `ToolsSettingsReference.VideoConverterOptions`, saved as they change.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, mpsc};
use std::time::Duration;

use egui::{FontId, Id, Rect, Ui, pos2, vec2};
use sharex_screen_recording::cli::FFmpegCliManager;
use sharex_settings::TaskSettings;
use sharex_strings::tools as s;
use sharex_tools::video::converter::{Codec, ConverterWindow, OutputInfo, VideoConverterOptions};
use sharex_ui::controls::{CheckBox, ComboBox, ComboItem, DecimalUpDown, Slider, TextBox};
use sharex_ui::keys::app_icons as icon;
use sharex_ui::theme::{Thickness, palette};
use sharex_ui::widgets::{center_y, font, line_height, text_width};

use crate::dialogs::{Cx, Dialog, Effect, Spec};
use crate::image_tools as tools;

pub const WIDTH: f32 = 850.0;
pub const HEIGHT: f32 = 550.0;
pub const MIN_WIDTH: f32 = 700.0;
pub const MIN_HEIGHT: f32 = 550.0;

/// The input picker's types (`SelectInputFileAsync`).
const INPUT_PATTERNS: [&str; 11] = ["mp4", "mkv", "webm", "avi", "mov", "m4v", "wmv", "gif", "webp", "png", "apng"];

/// `VideoConversionResult`.
pub struct ConversionResult {
    pub succeeded: bool,
    pub was_cancelled: bool,
    pub error_message: Option<String>,
    pub output_info: Option<OutputInfo>,
}

struct Encoding {
    ffmpeg: FFmpegCliManager,
    progress: Arc<AtomicU32>,
    rx: mpsc::Receiver<ConversionResult>,
    output: String,
    auto_open: bool,
}

pub struct VideoConverterWindow {
    pub input_file_path: String,
    pub output_folder_path: String,
    pub output_file_name: String,
    pub codec: Codec,
    pub use_bitrate: bool,
    pub video_quality: f64,
    pub video_bitrate: f64,
    pub auto_open_folder: bool,
    pub status_text: String,
    pub output_info: Option<OutputInfo>,
    /// `Progress` (0 to 100).
    pub progress: f64,
    /// The options this window edits (`_options`), with the fields it
    /// doesn't show (custom arguments).
    options: VideoConverterOptions,
    /// FFmpeg's path (`CaptureSettings.FFmpegOptions.FFmpegPath`, checked).
    ffmpeg_path: Option<std::path::PathBuf>,
    task: TaskSettings,
    save_to_defaults: bool,
    options_changed: bool,
    encoding: Option<Encoding>,
    /// Shown encoding (`--render encoding`) without FFmpeg running.
    fake_encoding: bool,
}

impl VideoConverterWindow {
    /// `new VideoConverterWindow(options, handler, inputFilePath)` over the
    /// task's `ToolsSettingsReference.VideoConverterOptions`.
    pub fn new(task: TaskSettings, defaults: &TaskSettings, ffmpeg_path: Option<std::path::PathBuf>, input: Option<String>) -> Self {
        let save_to_defaults = task.UseDefaultToolsSettings;
        let tools = if save_to_defaults { defaults.ToolsSettings.as_ref() } else { task.ToolsSettings.as_ref() };
        let options = tools.map(|t| VideoConverterOptions::from_json(&t.VideoConverterOptions)).unwrap_or_default();
        let input_file_path = options.input_file_path.clone().unwrap_or_default();
        let status_text = if input_file_path.trim().is_empty() { s::VideoConverterViewModel_Choose_video_or_animation().to_string() } else { String::new() };
        let mut w = Self {
            input_file_path,
            output_folder_path: options.output_folder_path.clone().unwrap_or_default(),
            output_file_name: options.output_file_name.clone().unwrap_or_default(),
            codec: options.video_codec,
            use_bitrate: options.video_quality_use_bitrate,
            video_quality: options.video_quality as f64,
            video_bitrate: options.video_quality_bitrate as f64,
            auto_open_folder: options.auto_open_folder,
            status_text,
            output_info: None,
            progress: 0.0,
            options,
            ffmpeg_path,
            task,
            save_to_defaults,
            options_changed: false,
            encoding: None,
            fake_encoding: false,
        };
        if let Some(input) = input.filter(|i| !i.trim().is_empty()) {
            w.load_input(&input);
        }
        w
    }

    fn is_encoding(&self) -> bool {
        self.encoding.is_some() || self.fake_encoding
    }

    /// Shows the encoding state at `progress` without running FFmpeg (`--render`).
    pub fn set_encoding(&mut self, progress: f64) {
        self.fake_encoding = true;
        self.progress = progress;
        self.output_info = None;
        self.status_text = s::VideoConverterViewModel_Converting().into();
    }

    fn view(&self) -> ConverterWindow {
        ConverterWindow {
            input_file_path: self.input_file_path.clone(),
            output_folder_path: self.output_folder_path.clone(),
            output_file_name: self.output_file_name.clone(),
            codec: self.codec,
            use_bitrate: self.use_bitrate,
            video_quality: self.video_quality,
            video_bitrate: self.video_bitrate,
        }
    }

    /// `InputFileDisplay`.
    fn input_file_display(&self) -> &str {
        if self.input_file_path.trim().is_empty() { s::VideoConverterViewModel_No_file_selected() } else { &self.input_file_path }
    }

    fn shows_quality_slider(&self) -> bool {
        self.codec.can_choose_rate_control() && !self.use_bitrate
    }

    fn shows_bitrate(&self) -> bool {
        self.codec.shows_quality_controls() && (!self.codec.can_choose_rate_control() || self.use_bitrate)
    }

    /// `LoadInput`.
    pub fn load_input(&mut self, path: &str) {
        self.input_file_path = path.to_string();
        if self.output_folder_path.trim().is_empty() {
            self.output_folder_path = tools::directory_name(path);
        }
        if self.output_file_name.trim().is_empty() {
            self.output_file_name = format!("{}-output", sharex_tools::dotnet::file_name_without_extension(path));
        }
        self.output_info = None;
        self.status_text.clear();
        self.options_changed = true;
    }

    /// `PersistSettings`.
    fn persist(&mut self) {
        let o = &mut self.options;
        o.input_file_path = Some(self.input_file_path.clone());
        o.output_folder_path = Some(self.output_folder_path.clone());
        o.output_file_name = Some(self.output_file_name.clone());
        o.video_codec = self.codec;
        o.video_quality_use_bitrate = self.use_bitrate;
        o.video_quality = self.video_quality.round_ties_even() as i32;
        o.video_quality_bitrate = self.video_bitrate as i32;
        o.auto_open_folder = self.auto_open_folder;
    }

    fn save(&mut self, cx: &mut Cx) {
        self.persist();
        let json = self.options.to_json();
        if self.save_to_defaults {
            cx.settings.DefaultTaskSettings.ToolsSettings.get_or_insert_default().VideoConverterOptions = json.clone();
        }
        if let Some(t) = self.task.ToolsSettings.as_mut() {
            t.VideoConverterOptions = json;
        }
    }

    /// `StartEncodingAsync`.
    fn start(&mut self, cx: &mut Cx) {
        if self.is_encoding() {
            return;
        }
        if !tools::file_exists(&self.input_file_path) {
            self.status_text = s::VideoConverterViewModel_Select_existing_input().into();
            return;
        }
        if !tools::dir_exists(&self.output_folder_path) {
            self.status_text = s::VideoConverterViewModel_Select_existing_output().into();
            return;
        }
        if self.output_file_name.trim().is_empty() {
            self.status_text = s::VideoConverterViewModel_Enter_output_name().into();
            return;
        }
        self.save(cx);
        self.progress = 0.0;
        self.output_info = None;
        self.status_text = s::VideoConverterViewModel_Converting().into();
        let view = self.view();
        let args = view.build_arguments();
        let output = view.output_file_path();
        let ffmpeg = FFmpegCliManager::new(self.ffmpeg_path.clone().unwrap_or_default());
        ffmpeg.set_show_error(false);
        ffmpeg.set_track_encode_progress(true);
        let progress = Arc::new(AtomicU32::new(0));
        let p = progress.clone();
        ffmpeg.on_encode_progress(move |v| p.store(v.to_bits(), Ordering::Relaxed));
        let (tx, rx) = mpsc::channel();
        let (worker, out) = (ffmpeg.clone(), output.clone());
        std::thread::spawn(move || {
            let _ = tx.send(run_conversion(&worker, &args, &out));
        });
        self.encoding = Some(Encoding { ffmpeg, progress, rx, output, auto_open: self.auto_open_folder });
    }

    /// `StopEncoding`: FFmpeg asked to stop (`CancellationToken` → `Close`).
    fn stop(&mut self) {
        if let Some(e) = &self.encoding {
            self.status_text = s::VideoConverterViewModel_Stopping().into();
            e.ffmpeg.close();
        }
    }

    fn poll(&mut self, ctx: &egui::Context) -> Vec<Effect> {
        let Some(e) = &self.encoding else { return Vec::new() };
        self.progress = (f32::from_bits(e.progress.load(Ordering::Relaxed)) as f64).clamp(0.0, 100.0);
        let Ok(result) = e.rx.try_recv() else {
            ctx.request_repaint_after(Duration::from_millis(100));
            return Vec::new();
        };
        let e = self.encoding.take().expect("encoding");
        let mut effects = Vec::new();
        if result.succeeded && !result.was_cancelled {
            if e.auto_open {
                effects.push(Effect::OpenFolderWithFile(e.output.clone().into()));
            }
            self.progress = 100.0;
            self.output_info = result.output_info;
            self.status_text = s::VideoConverterViewModel_Conversion_complete().replace("{0}", &self.view().output_file_path());
        } else if result.was_cancelled {
            self.progress = 0.0;
            self.status_text = s::VideoConverterViewModel_Conversion_stopped().into();
        } else {
            self.progress = 0.0;
            self.status_text = match result.error_message.filter(|m| !m.trim().is_empty()) {
                Some(m) => s::VideoConverterViewModel_Conversion_failed_message().replace("{0}", &m),
                None => s::VideoConverterViewModel_Conversion_failed().into(),
            };
        }
        effects
    }

    /// The state a finished conversion leaves, for the output file already
    /// there (`--render result`): only probed, never written.
    pub fn show_result(&mut self) {
        let output = self.view().output_file_path();
        self.output_info = self.ffmpeg_path.as_deref().and_then(|f| output_info(f, &output));
        self.progress = 100.0;
        self.status_text = s::VideoConverterViewModel_Conversion_complete().replace("{0}", &output);
    }

    /// Runs a conversion to the end on this thread (tests).
    #[cfg(test)]
    pub fn convert_now(&mut self, cx: &mut Cx) -> Vec<Effect> {
        self.auto_open_folder = false;
        self.start(cx);
        let Some(e) = self.encoding.take() else { return Vec::new() };
        let result = e.rx.recv().unwrap_or(ConversionResult { succeeded: false, was_cancelled: false, error_message: None, output_info: None });
        let (tx, rx) = mpsc::channel();
        let _ = tx.send(result);
        self.encoding = Some(Encoding { rx, ..e });
        self.poll(&egui::Context::default())
    }

    fn field_label_font() -> FontId {
        tools::normal_font()
    }
}

/// `RunVideoConversionAsync`'s work: FFmpeg run with the arguments, then
/// the output probed (`GetVideoConversionOutputInfo`).
fn run_conversion(ffmpeg: &FFmpegCliManager, args: &str, output: &str) -> ConversionResult {
    let succeeded = ffmpeg.run(args);
    let was_cancelled = ffmpeg.stop_requested();
    let error_message = (!succeeded && !was_cancelled).then(|| ffmpeg.output().split(['\r', '\n']).rfind(|l| !l.is_empty()).unwrap_or_default().to_string());
    let output_info = (succeeded && !was_cancelled).then(|| output_info(ffmpeg.ffmpeg_path(), output)).flatten();
    ConversionResult { succeeded, was_cancelled, error_message, output_info }
}

/// `GetVideoConversionOutputInfo`.
fn output_info(ffmpeg: &std::path::Path, output: &str) -> Option<OutputInfo> {
    let size = std::fs::metadata(output).ok().filter(|m| m.is_file())?.len();
    let probe = FFmpegCliManager::new(ffmpeg);
    probe.set_show_error(false);
    probe.run(&sharex_tools::video::converter::probe_arguments(output));
    Some(sharex_tools::video::converter::output_info(&probe.output(), size))
}

/// `OutputBitrateText`: `N0` in the current culture, then `kbps`.
fn bitrate_text(info: Option<&OutputInfo>) -> String {
    match info.and_then(|i| i.average_bitrate_kbps) {
        Some(b) => format!("{} {}", sharex_tools::text::format_number(b, "N0", true), s::VideoConverterWindow_kbps()),
        None => "-".into(),
    }
}

impl Dialog for VideoConverterWindow {
    fn spec(&self) -> Spec {
        Spec::sized(crate::rs_title(s::VideoConverterWindow_ShareX_Video_Converter()), WIDTH, HEIGHT, MIN_WIDTH, MIN_HEIGHT)
    }

    fn ui(&mut self, ui: &mut Ui, cx: &mut Cx) -> Vec<Effect> {
        let mut effects = self.poll(ui.ctx());
        let p = palette(ui.ctx());
        let c = tools::background(ui);
        let idle = !self.is_encoding();
        let hh = tools::heading_height(ui);
        let nf = tools::normal_font();
        let sf = tools::semibold_font();
        let nl = line_height(ui, &nf);
        let before = (self.input_file_path.clone(), self.output_folder_path.clone(), self.output_file_name.clone(), self.codec, self.use_bitrate, self.video_quality, self.video_bitrate, self.auto_open_folder);

        // Row 3: progress, status and Start/Stop.
        let (button_text, button_icon) =
            if idle { (s::VideoConverterWindow_Start_encoding(), icon::Play) } else { (s::VideoConverterWindow_Stop_encoding(), icon::Stop) };
        let bw = tools::icon_text_button_width(ui, button_text);
        let status_w = c.width() - bw - 12.0;
        let status_h = sharex_ui::controls::wrapped(ui, pos2(0.0, 0.0), status_w, &self.status_text, nf.clone(), p.text, false);
        let row3_h = if idle { 0.0 } else { 6.0 + 8.0 } + status_h.max(tools::BUTTON_HEIGHT);
        let row3 = Rect::from_min_max(pos2(c.min.x, c.max.y - row3_h), c.max);
        // Row 2: the result.
        let rv = font(15.0, 600);
        let row2_h = 26.0 + nl + 5.0 + line_height(ui, &rv);
        let row2 = Rect::from_min_size(pos2(c.min.x, row3.min.y - 12.0 - row2_h), vec2(c.width(), row2_h));
        // Row 0: the input file.
        let row0 = Rect::from_min_size(c.min, vec2(c.width(), 26.0 + tools::BUTTON_HEIGHT));
        let row1 = Rect::from_min_max(pos2(c.min.x, row0.max.y + 12.0), pos2(c.max.x, row2.min.y - 12.0));

        tools::panel(ui, row0);
        let q = row0.shrink(13.0);
        let heading = s::VideoConverterWindow_Input_file();
        let hw = text_width(ui, heading, tools::heading_font());
        tools::heading(ui, Rect::from_min_size(q.min, vec2(hw, q.height())), heading);
        let browse = s::VideoConverterWindow_Browse();
        let bbw = tools::text_button_width(ui, browse);
        let br = Rect::from_min_size(pos2(q.max.x - bbw, q.min.y), vec2(bbw, tools::BUTTON_HEIGHT));
        tools::path_text(ui, Rect::from_min_max(pos2(q.min.x + hw + 12.0, q.min.y), pos2(br.min.x - 12.0, q.max.y)), self.input_file_display());
        if tools::text_button(ui, br, Id::new("converter-browse"), browse, idle).clicked() {
            let folder = tools::directory_name(&self.input_file_path);
            let mut d = rfd::FileDialog::new()
                .set_title(s::VideoConverterViewModel_Select_input_dialog())
                .add_filter(s::VideoConverterWindow_Video_animation_files(), &INPUT_PATTERNS)
                .add_filter("All", &["*"]);
            if tools::dir_exists(&folder) {
                d = d.set_directory(&folder);
            }
            if let Some(f) = d.pick_file() {
                self.load_input(&f.to_string_lossy());
            }
        }

        // Row 1, left: encoding and quality (a `ScrollViewer`).
        let left = Rect::from_min_size(row1.min, vec2(360.0, row1.height()));
        let right = Rect::from_min_max(pos2(left.max.x + 12.0, row1.min.y), row1.max);
        tools::panel(ui, left);
        let view = left.shrink(17.0);
        let quality_h = if !self.codec.shows_quality_controls() {
            0.0
        } else {
            let check = if self.codec.can_choose_rate_control() { 32.0 + 10.0 } else { 0.0 };
            let slider = if self.shows_quality_slider() {
                line_height(ui, &nf).max(line_height(ui, &sf)) + tools::SLIDER_HEIGHT + line_height(ui, &font(12.0, 400))
            } else {
                0.0
            };
            let bitrate = if self.shows_bitrate() { nl + 5.0 + 40.0 } else { 0.0 };
            hh + 10.0 + check + slider + bitrate
        };
        let encoding_h = hh + 10.0 + nl + 5.0 + 40.0;
        let content_h = tools::stack_height(&[encoding_h, quality_h], 18.0);
        let offset = crate::dialogs::layout::scroll_offset(ui, Id::new("converter-scroll"), view, content_h);
        {
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(view));
            child.set_clip_rect(view.intersect(ui.clip_rect()));
            let ui = &mut child;
            let x = view.min.x;
            let w = view.width();
            let mut y = view.min.y - offset;
            tools::heading(ui, Rect::from_min_size(pos2(x, y), vec2(w, hh)), s::VideoConverterWindow_Encoding());
            y += hh + 10.0;
            tools::text_at(ui, pos2(x, y), s::VideoConverterWindow_Video_encoder(), Self::field_label_font(), p.text_secondary);
            y += nl + 5.0;
            let items: Vec<ComboItem> = Codec::ALL.iter().map(|c| ComboItem::text(c.display_name())).collect();
            let selected = Codec::ALL.iter().position(|c| *c == self.codec);
            // ShareX's `ComboBox` style: `Padding="8,6"`.
            let combo = ComboBox { padding: Thickness::hv(8.0, 6.0), enabled: idle };
            if let Some(i) = combo.show(ui, Rect::from_min_size(pos2(x, y), vec2(w, 40.0)), Id::new("converter-codec"), &items, selected) {
                self.codec = Codec::ALL[i];
                // `OnSelectedCodecChanged`: the quality clamped to the codec's range.
                let (min, max) = self.codec.quality_range();
                self.video_quality = self.video_quality.clamp(min, max);
            }
            y += 40.0 + 18.0;
            if self.codec.shows_quality_controls() {
                tools::heading(ui, Rect::from_min_size(pos2(x, y), vec2(w, hh)), s::VideoConverterWindow_Quality());
                y += hh + 10.0;
                if self.codec.can_choose_rate_control() {
                    let text = s::VideoConverterWindow_Use_a_target_bitrate();
                    let cb = CheckBox { text, text_font: font(13.0, 400), text_color: None, min_height: 32.0, enabled: idle };
                    cb.show(ui, Rect::from_min_size(pos2(x, y), vec2(cb.width(ui), 32.0)), Id::new("converter-use-bitrate"), &mut self.use_bitrate);
                    y += 42.0;
                }
                if self.shows_quality_slider() {
                    let lh = line_height(ui, &nf).max(line_height(ui, &sf));
                    let row = Rect::from_min_size(pos2(x, y), vec2(w, lh));
                    tools::text_at(ui, pos2(x, center_y(row, nl)), s::VideoConverterWindow_Constant_quality(), nf.clone(), p.text_secondary);
                    let value = tools::format_0(self.video_quality as f32);
                    let vw = text_width(ui, &value, sf.clone());
                    tools::text_at(ui, pos2(row.max.x - vw, center_y(row, line_height(ui, &sf))), &value, sf.clone(), p.text);
                    y += lh;
                    let (min, max) = self.codec.quality_range();
                    let mut v = self.video_quality as f32;
                    let slider = Slider { minimum: min as f32, maximum: max as f32, step: 1.0, enabled: idle };
                    if slider.show(ui, Rect::from_min_size(pos2(x, y), vec2(w, tools::SLIDER_HEIGHT)), Id::new("converter-quality"), &mut v) {
                        self.video_quality = v as f64;
                    }
                    y += tools::SLIDER_HEIGHT;
                    let small = font(12.0, 400);
                    tools::text_at(ui, pos2(x, y), s::VideoConverterWindow_Higher_quality(), small.clone(), p.text);
                    let sw = text_width(ui, s::VideoConverterWindow_Smaller_file(), small.clone());
                    tools::text_at(ui, pos2(x + w - sw, y), s::VideoConverterWindow_Smaller_file(), small.clone(), p.text);
                    y += line_height(ui, &small);
                }
                if self.shows_bitrate() {
                    tools::text_at(ui, pos2(x, y), s::VideoConverterWindow_Target_bitrate(), Self::field_label_font(), p.text_secondary);
                    y += nl + 5.0;
                    let kbps = s::VideoConverterWindow_kbps();
                    let kw = text_width(ui, kbps, nf.clone());
                    let row = Rect::from_min_size(pos2(x, y), vec2(w, 40.0));
                    DecimalUpDown { minimum: 100.0, maximum: 100000.0, increment: 100.0, decimals: 0, enabled: idle }.show(
                        ui,
                        Rect::from_min_max(row.min, pos2(row.max.x - kw - 8.0, row.max.y)),
                        Id::new("converter-bitrate"),
                        &mut self.video_bitrate,
                    );
                    tools::text_at(ui, pos2(row.max.x - kw, center_y(row, nl)), kbps, nf.clone(), p.text);
                }
            }
        }
        crate::dialogs::layout::scroll_bar(ui, view, content_h, offset, 0.0);

        // Row 1, right: the output.
        tools::panel(ui, right);
        let q = right.shrink(17.0);
        let (x, w) = (q.min.x, q.width());
        let mut y = q.min.y;
        tools::heading(ui, Rect::from_min_size(pos2(x, y), vec2(w, hh)), s::VideoConverterWindow_Output());
        y += hh + 10.0;
        tools::text_at(ui, pos2(x, y), s::VideoConverterWindow_Folder(), nf.clone(), p.text_secondary);
        y += nl + 5.0;
        let fb = Rect::from_min_size(pos2(q.max.x - tools::ICON_BUTTON_WIDTH, y), vec2(tools::ICON_BUTTON_WIDTH, 40.0));
        TextBox { placeholder: s::VideoConverterWindow_Choose_an_output_folder(), enabled: idle, ..TextBox::default() }.show(
            ui,
            Rect::from_min_max(pos2(x, y), pos2(fb.min.x - 8.0, y + 40.0)),
            Id::new("converter-folder"),
            &mut self.output_folder_path,
        );
        if tools::icon_button(ui, fb, Id::new("converter-folder-browse"), icon::FolderOpen, idle).clicked()
            && let Some(f) = tools::pick_folder(s::VideoConverterViewModel_Select_output_dialog(), &self.output_folder_path)
        {
            self.output_folder_path = f;
        }
        y += 40.0 + 12.0;
        tools::text_at(ui, pos2(x, y), s::VideoConverterWindow_File_name(), nf.clone(), p.text_secondary);
        y += nl + 5.0;
        TextBox { placeholder: s::VideoConverterWindow_video_output(), enabled: idle, ..TextBox::default() }.show(
            ui,
            Rect::from_min_size(pos2(x, y), vec2(w, 40.0)),
            Id::new("converter-name"),
            &mut self.output_file_name,
        );
        y += 40.0 + 8.0;
        tools::path_text(ui, Rect::from_min_size(pos2(x, y), vec2(w, nl)), &self.view().output_file_path());
        y += nl + 12.0;
        // The check box's `TextBlock` wraps (`TextWrapping="Wrap"`).
        let text = s::VideoConverterWindow_Open_the_output_folder_when_finished();
        let cb = CheckBox { text, text_font: font(13.0, 400), text_color: None, min_height: 32.0, enabled: idle };
        cb.show(ui, Rect::from_min_size(pos2(x, y), vec2(cb.width(ui).min(w), 32.0)), Id::new("converter-auto-open"), &mut self.auto_open_folder);

        // Row 2: the result.
        tools::panel(ui, row2);
        let q = Rect::from_min_max(pos2(row2.min.x + 17.0, row2.min.y + 13.0), pos2(row2.max.x - 17.0, row2.max.y - 13.0));
        let heading = s::VideoConverterWindow_Result();
        let hw = text_width(ui, heading, tools::heading_font());
        tools::heading(ui, Rect::from_min_size(q.min, vec2(hw, q.height())), heading);
        let col0 = hw + 12.0;
        let star = ((q.width() - col0 - 3.0 * 24.0) / 3.0).max(0.0);
        let info = self.output_info.as_ref();
        let values = [
            (s::VideoConverterWindow_Average_bitrate(), bitrate_text(info)),
            (s::VideoConverterWindow_File_size(), info.map(|i| crate::tasks::size_string(i.file_size, false)).unwrap_or_else(|| "-".into())),
            (
                s::VideoConverterWindow_Resolution(),
                info.and_then(|i| Some(format!("{} × {}", i.width?, i.height?))).unwrap_or_else(|| "-".into()),
            ),
        ];
        let value_color = if info.is_some() { p.text } else { p.text_secondary };
        for (k, (label, value)) in values.iter().enumerate() {
            let x = q.min.x + col0 + 24.0 + k as f32 * (star + 24.0);
            tools::text_at(ui, pos2(x, q.min.y), label, nf.clone(), p.text_secondary);
            let v = tools::ellipsize(ui, value, &rv, f32::INFINITY);
            tools::text_at(ui, pos2(x, q.min.y + nl + 5.0), &v, rv.clone(), value_color);
        }

        // Row 3.
        let mut y = row3.min.y;
        if !idle {
            sharex_ui::controls::determinate_progress(ui.painter(), Rect::from_min_size(row3.min, vec2(row3.width(), 6.0)), self.progress / 100.0);
            y += 14.0;
        }
        let row = Rect::from_min_max(pos2(row3.min.x, y), row3.max);
        sharex_ui::controls::wrapped(ui, pos2(row.min.x, center_y(row, status_h)), status_w, &self.status_text, nf.clone(), p.text, true);
        let br = Rect::from_min_size(pos2(row.max.x - bw, center_y(row, tools::BUTTON_HEIGHT)), vec2(bw, tools::BUTTON_HEIGHT));
        if tools::icon_text_button(ui, br, Id::new("converter-start"), button_icon, button_text, true).clicked() {
            if idle {
                self.start(cx);
            } else {
                self.stop();
            }
        }

        let after = (self.input_file_path.clone(), self.output_folder_path.clone(), self.output_file_name.clone(), self.codec, self.use_bitrate, self.video_quality, self.video_bitrate, self.auto_open_folder);
        if before != after || std::mem::take(&mut self.options_changed) {
            self.save(cx);
        }
        effects.append(&mut Vec::new());
        effects
    }

    fn dropped(&mut self, files: Vec<std::path::PathBuf>, _cx: &mut Cx) -> Vec<Effect> {
        if !self.is_encoding()
            && let Some(f) = files.first()
        {
            self.load_input(&f.to_string_lossy());
        }
        Vec::new()
    }

    fn closed(&mut self, _cx: &mut Cx) -> Vec<Effect> {
        // `Dispose`: the conversion cancelled.
        if let Some(e) = &self.encoding {
            e.ffmpeg.close();
        }
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window() -> VideoConverterWindow {
        let defaults = TaskSettings::default();
        VideoConverterWindow::new(TaskSettings::default_task_settings(&defaults), &defaults, None, None)
    }

    #[test]
    fn load_input_names_the_output() {
        let mut w = window();
        assert_eq!(w.status_text, s::VideoConverterViewModel_Choose_video_or_animation());
        let input = if cfg!(windows) { r"C:\v\clip.mov" } else { "/v/clip.mov" };
        w.load_input(input);
        assert_eq!(w.output_file_name, "clip-output");
        assert_eq!(w.output_folder_path, tools::directory_name(input));
        assert!(w.status_text.is_empty());
        assert!(w.view().output_file_path().ends_with("clip-output.mp4"));
        w.codec = Codec::Vp9;
        assert!(w.view().output_file_path().ends_with("clip-output.webm"));
    }

    #[test]
    fn quality_controls_follow_the_codec() {
        let mut w = window();
        assert!(w.shows_quality_slider() && !w.shows_bitrate());
        w.use_bitrate = true;
        assert!(!w.shows_quality_slider() && w.shows_bitrate());
        w.codec = Codec::H264Nvenc;
        assert!(!w.shows_quality_slider() && w.shows_bitrate());
        w.codec = Codec::Gif;
        assert!(!w.shows_quality_slider() && !w.shows_bitrate());
    }

    #[test]
    fn start_checks_the_input_first() {
        let mut w = window();
        let mut settings = sharex_settings::ApplicationConfig::default();
        let store = crate::upload::UploadersConfigStore::default();
        let mut cx = Cx { settings: &mut settings, uploaders: &store };
        w.start(&mut cx);
        assert_eq!(w.status_text, s::VideoConverterViewModel_Select_existing_input());
        assert!(!w.is_encoding());
    }

    /// A real conversion with a local FFmpeg (skipped without one): a
    /// generated 1 s clip to H.264, then the output probed.
    #[test]
    fn converts_with_a_local_ffmpeg() {
        let Some(ffmpeg) = sharex_screen_recording::discovery::fallback_locations(&std::env::var_os("PATH").unwrap_or_default()).into_iter().find(|p| p.is_file()) else {
            eprintln!("no FFmpeg: skipped");
            return;
        };
        let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("sharexrs-converter-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let input = dir.join("in.mp4");
        let made = std::process::Command::new(&ffmpeg)
            .args(["-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i", "testsrc2=size=160x90:rate=10:duration=1", "-pix_fmt", "yuv420p"])
            .arg(&input)
            .status()
            .is_ok_and(|s| s.success());
        if made {
            let defaults = TaskSettings::default();
            let mut w = VideoConverterWindow::new(TaskSettings::default_task_settings(&defaults), &defaults, Some(ffmpeg), Some(input.to_string_lossy().into_owned()));
            let mut settings = sharex_settings::ApplicationConfig::default();
            let store = crate::upload::UploadersConfigStore::default();
            let mut cx = Cx { settings: &mut settings, uploaders: &store };
            let effects = w.convert_now(&mut cx);
            assert!(effects.is_empty(), "the folder doesn't open in tests");
            let info = w.output_info.clone().expect("probed");
            assert_eq!((info.width, info.height), (Some(160), Some(90)));
            assert!(dir.join("in-output.mp4").is_file());
            assert_eq!(w.status_text, s::VideoConverterViewModel_Conversion_complete().replace("{0}", &dir.join("in-output.mp4").to_string_lossy()));
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn bitrate_text_is_n0() {
        let i = OutputInfo { file_size: 1, average_bitrate_kbps: Some(1234.5), width: None, height: None };
        assert_eq!(bitrate_text(Some(&i)), format!("1,235 {}", s::VideoConverterWindow_kbps()));
        assert_eq!(bitrate_text(None), "-");
    }
}
