//! `ShareX.Tools/Tools/VideoThumbnailer` (`VideoThumbnailerWindow.axaml`,
//! `VideoThumbnailerViewModel`) and `TaskHelpers.OpenVideoThumbnailer`:
//! the video, the thumbnails' count, format, width and suffix, where they
//! go, the contact sheet's options, Create thumbnails with FFmpeg's
//! progress, and the thumbnails uploaded afterwards when `UploadThumbnails`.
//! Every option is written into the task's `VideoThumbnailOptions` as it
//! changes, as the view model does.

use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

use egui::{Id, Rect, Ui, pos2, vec2};
use sharex_settings::TaskSettings;
use sharex_strings::tools as s;
use sharex_tools::video::thumbnailer::{ImageFormat, OutputLocation, VideoThumbnailInfo, VideoThumbnailOptions};
use sharex_ui::controls::{ComboBox, ComboItem, NumericUpDown, TextBox, ToggleSwitch};
use sharex_ui::keys::app_icons as icon;
use sharex_ui::theme::{Thickness, palette};
use sharex_ui::widgets::{center_y, line_height, text_width};

use crate::dialogs::{Cx, Dialog, Effect, Spec, UploadRequest};
use crate::image_tools as tools;

pub const WIDTH: f32 = 900.0;
pub const HEIGHT: f32 = 650.0;
pub const MIN_WIDTH: f32 = 760.0;
pub const MIN_HEIGHT: f32 = 580.0;

/// The video picker's types (`SelectVideoAsync`).
const VIDEO_PATTERNS: [&str; 10] = ["mp4", "mkv", "webm", "avi", "mov", "wmv", "m4v", "mpg", "mpeg", "flv"];

const FORMATS: [(ImageFormat, &str); 5] =
    [(ImageFormat::Png, "PNG"), (ImageFormat::Jpeg, "JPEG"), (ImageFormat::Webp, "WebP"), (ImageFormat::Gif, "GIF"), (ImageFormat::Bmp, "BMP")];
const LOCATIONS: [OutputLocation; 3] = [OutputLocation::DefaultFolder, OutputLocation::ParentFolder, OutputLocation::CustomFolder];

type Outcome = Result<Option<Vec<VideoThumbnailInfo>>, String>;

struct Running {
    rx: mpsc::Receiver<Outcome>,
    progress: Arc<Mutex<(i32, i32)>>,
}

pub struct VideoThumbnailerWindow {
    pub options: VideoThumbnailOptions,
    pub video_path: String,
    pub error_message: String,
    /// `ProgressValue` / `ProgressMaximum`.
    pub progress: (i32, i32),
    ffmpeg_path: std::path::PathBuf,
    task: TaskSettings,
    save_to_defaults: bool,
    running: Option<Running>,
    /// Shown busy (`--render progress`) without FFmpeg running.
    fake_busy: bool,
}

impl VideoThumbnailerWindow {
    /// `new VideoThumbnailerWindow(ffmpegPath, options, thumbnailsTaken)`
    /// over the task's `ToolsSettingsReference.VideoThumbnailOptions`, its
    /// `DefaultOutputDirectory` the task's screenshots folder.
    pub fn new(task: TaskSettings, defaults: &TaskSettings, ffmpeg_path: std::path::PathBuf, default_output_directory: String) -> Self {
        let save_to_defaults = task.UseDefaultToolsSettings;
        let tools = if save_to_defaults { defaults.ToolsSettings.as_ref() } else { task.ToolsSettings.as_ref() };
        let mut options = tools.map(|t| VideoThumbnailOptions::from_json(&t.VideoThumbnailOptions)).unwrap_or_default();
        options.default_output_directory = Some(default_output_directory);
        // The view model's clamps.
        options.thumbnail_count = options.thumbnail_count.max(1);
        options.max_thumbnail_width = options.max_thumbnail_width.max(0);
        options.column_count = options.column_count.max(1);
        options.padding = options.padding.max(0);
        options.spacing = options.spacing.max(0);
        let video_path = options.last_video_path.clone().unwrap_or_default();
        Self { options, video_path, error_message: String::new(), progress: (0, 1), ffmpeg_path, task, save_to_defaults, running: None, fake_busy: false }
    }

    fn is_busy(&self) -> bool {
        self.running.is_some() || self.fake_busy
    }

    /// Shows the busy state at `current` of the count (`--render progress`).
    pub fn set_busy(&mut self, current: i32) {
        self.fake_busy = true;
        self.progress = (current, self.options.thumbnail_count.max(1));
    }

    fn has_video(&self) -> bool {
        tools::file_exists(&self.video_path)
    }

    fn can_start(&self) -> bool {
        self.has_video() && self.ffmpeg_path.is_file() && !self.is_busy()
    }

    /// `LoadVideo` / `VideoPath = …` (`OnVideoPathChanged`).
    pub fn load_video(&mut self, path: &str) {
        if !self.is_busy() && !path.trim().is_empty() {
            self.video_path = path.to_string();
            self.options.last_video_path = Some(path.to_string());
            self.error_message.clear();
        }
    }

    fn save(&self, cx: &mut Cx) {
        let json = self.options.to_json();
        if self.save_to_defaults {
            cx.settings.DefaultTaskSettings.ToolsSettings.get_or_insert_default().VideoThumbnailOptions = json;
        }
    }

    /// `StartAsync`.
    fn start(&mut self) {
        if !self.can_start() {
            return;
        }
        self.error_message.clear();
        self.progress = (0, self.options.thumbnail_count.max(1));
        let progress = Arc::new(Mutex::new(self.progress));
        let (tx, rx) = mpsc::channel();
        let (ffmpeg, video, options, p) = (self.ffmpeg_path.clone(), self.video_path.clone(), self.options.clone(), progress.clone());
        std::thread::spawn(move || {
            let mut random = sharex_image_effects::RandomFast::new();
            let r = sharex_tools::video::thumbnailer::take_thumbnails(
                &ffmpeg,
                &video,
                &options,
                || random.next(0, i32::MAX) as f64 / i32::MAX as f64,
                |current, length| {
                    if let Ok(mut g) = p.lock() {
                        *g = (current, length.max(1));
                    }
                },
            );
            let _ = tx.send(r);
        });
        self.running = Some(Running { rx, progress });
    }

    /// Runs to the end on this thread (`--render results`).
    pub fn run_now(&mut self) -> Vec<Effect> {
        self.start();
        let Some(r) = self.running.take() else { return Vec::new() };
        let outcome = r.rx.recv().unwrap_or(Ok(None));
        let (tx, rx) = mpsc::channel();
        let _ = tx.send(outcome);
        self.running = Some(Running { rx, ..r });
        self.poll(&egui::Context::default())
    }

    fn poll(&mut self, ctx: &egui::Context) -> Vec<Effect> {
        let Some(r) = &self.running else { return Vec::new() };
        if let Ok(g) = r.progress.lock() {
            self.progress = *g;
        }
        let Ok(outcome) = r.rx.try_recv() else {
            ctx.request_repaint_after(Duration::from_millis(100));
            return Vec::new();
        };
        self.running = None;
        let mut effects = Vec::new();
        match outcome {
            Ok(Some(thumbnails)) if !thumbnails.is_empty() => {
                // `VideoThumbnailer.TakeThumbnails`: `OpenDirectory`.
                if self.options.open_directory {
                    effects.push(Effect::OpenFolderWithFile(thumbnails[0].file_path.clone().into()));
                }
                // `thumbnailsTaken`: `UploadManager.UploadFile` for each.
                if self.options.upload_thumbnails {
                    for t in &thumbnails {
                        effects.push(Effect::Upload(Box::new(UploadRequest::Files {
                            files: vec![t.file_path.clone().into()],
                            task: self.task.clone(),
                            confirmed: true,
                        })));
                    }
                }
            }
            Ok(_) => self.error_message = s::VideoThumbnailerViewModel_No_thumbnails().into(),
            Err(e) => {
                eprintln!("VideoThumbnailerViewModel: Failed to create video thumbnails. {e}");
                self.error_message = e;
            }
        }
        effects
    }

    /// A `Grid ColumnDefinitions="*,Auto" MinHeight="30"` row: the
    /// `TextBlock` (Normal) and the `ToggleSwitch` at the right.
    fn toggle_row(ui: &Ui, rect: Rect, id: Id, text: &str, value: &mut bool, enabled: bool) -> bool {
        let p = palette(ui.ctx());
        let f = tools::normal_font();
        tools::text_at(ui, pos2(rect.min.x, center_y(rect, line_height(ui, &f))), text, f, p.text);
        let size = ToggleSwitch::SIZE;
        let before = *value;
        ToggleSwitch { enabled }.show(ui, pos2(rect.max.x - size.x, center_y(rect, size.y)), id, value);
        before != *value
    }
}

impl Dialog for VideoThumbnailerWindow {
    fn spec(&self) -> Spec {
        Spec::sized(crate::rs_title(s::VideoThumbnailerWindow_ShareX_Video_Thumbnailer()), WIDTH, HEIGHT, MIN_WIDTH, MIN_HEIGHT)
    }

    fn ui(&mut self, ui: &mut Ui, cx: &mut Cx) -> Vec<Effect> {
        let effects = self.poll(ui.ctx());
        let p = palette(ui.ctx());
        let c = tools::background(ui);
        let idle = !self.is_busy();
        let hh = tools::heading_height(ui);
        let nf = tools::normal_font();
        let before = self.options.clone();

        // Row 2: progress, error and Create thumbnails (`RowSpacing="8"`
        // stays when the bar is hidden).
        let text = s::VideoThumbnailerWindow_Create_thumbnails();
        let bw = tools::icon_text_button_width(ui, text);
        let err_w = c.width() - bw - 12.0;
        let err_h = if self.error_message.trim().is_empty() {
            0.0
        } else {
            sharex_ui::controls::wrapped(ui, pos2(0.0, 0.0), err_w, &self.error_message, nf.clone(), p.cancel_border, false)
        };
        let bar_h = if idle { 0.0 } else { 5.0 };
        let row2_h = bar_h + 8.0 + err_h.max(tools::BUTTON_HEIGHT);
        let row2 = Rect::from_min_max(pos2(c.min.x, c.max.y - row2_h), c.max);
        let row0 = Rect::from_min_size(c.min, vec2(c.width(), 26.0 + tools::BUTTON_HEIGHT));
        let row1 = Rect::from_min_max(pos2(c.min.x, row0.max.y + 12.0), pos2(c.max.x, row2.min.y - 12.0));

        // Row 0: the video.
        tools::panel(ui, row0);
        let q = row0.shrink(13.0);
        let heading = s::VideoThumbnailerWindow_Video();
        let hw = text_width(ui, heading, tools::heading_font());
        tools::heading(ui, Rect::from_min_size(q.min, vec2(hw, q.height())), heading);
        let browse = s::VideoThumbnailerWindow_Browse();
        let bbw = tools::text_button_width(ui, browse);
        let br = Rect::from_min_size(pos2(q.max.x - bbw, q.min.y), vec2(bbw, tools::BUTTON_HEIGHT));
        // `TargetNullValue` only covers null: an empty path shows nothing.
        tools::path_text(ui, Rect::from_min_max(pos2(q.min.x + hw + 12.0, q.min.y), pos2(br.min.x - 12.0, q.max.y)), &self.video_path);
        if tools::text_button(ui, br, Id::new("vthumb-browse"), browse, idle).clicked()
            && let Some(f) = rfd::FileDialog::new()
                .set_title(s::VideoThumbnailerWindow_Select_video_dialog())
                .add_filter(s::VideoThumbnailerWindow_Video_files(), &VIDEO_PATTERNS)
                .add_filter("All", &["*"])
                .pick_file()
        {
            self.load_video(&f.to_string_lossy());
        }

        let col_w = ((row1.width() - 12.0) / 2.0).floor();
        let left = Rect::from_min_size(row1.min, vec2(col_w, row1.height()));
        let right = Rect::from_min_max(pos2(left.max.x + 12.0, row1.min.y), row1.max);

        // Left, top: the thumbnails.
        let thumbs_h = 26.0 + tools::grid_height(&[hh, 40.0, 40.0, 40.0, 40.0, 32.0], 10.0);
        let panel = Rect::from_min_size(left.min, vec2(left.width(), thumbs_h));
        tools::panel(ui, panel);
        let q = panel.shrink(13.0);
        tools::heading(ui, Rect::from_min_size(q.min, vec2(q.width(), hh)), s::VideoThumbnailerWindow_Thumbnails());
        let mut y = q.min.y + hh + 10.0;
        let row = |y: f32| Rect::from_min_size(pos2(q.min.x, y), vec2(q.width(), 40.0));
        let control = tools::label_row(ui, row(y), 125.0, s::VideoThumbnailerWindow_Count());
        let mut count = self.options.thumbnail_count as i64;
        if (NumericUpDown { minimum: 1, maximum: 100, enabled: true }).show(ui, control, Id::new("vthumb-count"), &mut count) {
            self.options.thumbnail_count = (count as i32).max(1);
        }
        y += 50.0;
        let control = tools::label_row(ui, row(y), 125.0, s::VideoThumbnailerWindow_Image_format());
        let items: Vec<ComboItem> = FORMATS.iter().map(|(_, n)| ComboItem::normal(*n)).collect();
        let selected = FORMATS.iter().position(|(f, _)| *f == self.options.image_format).unwrap_or(0);
        if let Some(i) = (ComboBox { padding: Thickness::hv(8.0, 6.0), enabled: true }).show(ui, control, Id::new("vthumb-format"), &items, Some(selected)) {
            self.options.image_format = FORMATS[i].0;
        }
        y += 50.0;
        let control = tools::label_row(ui, row(y), 125.0, s::VideoThumbnailerWindow_Maximum_width());
        let mut width = self.options.max_thumbnail_width as i64;
        if (NumericUpDown { minimum: 0, maximum: 10000, enabled: true }).show(ui, control, Id::new("vthumb-width"), &mut width) {
            self.options.max_thumbnail_width = (width as i32).max(0);
        }
        y += 50.0;
        let control = tools::label_row(ui, row(y), 125.0, s::VideoThumbnailerWindow_Filename_suffix());
        TextBox::default().show(ui, control, Id::new("vthumb-suffix"), &mut self.options.filename_suffix);
        y += 50.0;
        Self::toggle_row(
            ui,
            Rect::from_min_size(pos2(q.min.x, y), vec2(q.width(), 32.0)),
            Id::new("vthumb-random"),
            s::VideoThumbnailerWindow_Choose_a_random_frame_for_each_thumbnail(),
            &mut self.options.random_frame,
            true,
        );

        // Left, bottom: the output.
        let panel = Rect::from_min_max(pos2(left.min.x, panel.max.y + 12.0), left.max);
        tools::panel(ui, panel);
        let q = panel.shrink(13.0);
        tools::heading(ui, Rect::from_min_size(q.min, vec2(q.width(), hh)), s::VideoThumbnailerWindow_Output());
        let mut y = q.min.y + hh + 10.0;
        let control = tools::label_row(ui, Rect::from_min_size(pos2(q.min.x, y), vec2(q.width(), 40.0)), 125.0, s::VideoThumbnailerWindow_Save_to());
        let names = [s::VideoThumbnailerViewModel_Default_screenshots_folder(), s::VideoThumbnailerViewModel_Same_folder_as_video(), s::VideoThumbnailerViewModel_Custom_folder()];
        let items: Vec<ComboItem> = names.iter().map(|n| ComboItem::normal(*n)).collect();
        let selected = LOCATIONS.iter().position(|l| *l == self.options.output_location).unwrap_or(0);
        if let Some(i) = (ComboBox { padding: Thickness::hv(8.0, 6.0), enabled: true }).show(ui, control, Id::new("vthumb-location"), &items, Some(selected)) {
            self.options.output_location = LOCATIONS[i];
        }
        y += 40.0;
        if self.options.output_location == OutputLocation::CustomFolder {
            y += 10.0;
            let bbw = tools::text_button_width(ui, browse);
            let br = Rect::from_min_size(pos2(q.max.x - bbw, y), vec2(bbw, 40.0));
            TextBox { placeholder: s::VideoThumbnailerWindow_Choose_an_output_folder(), ..TextBox::default() }.show(
                ui,
                Rect::from_min_max(pos2(q.min.x, y), pos2(br.min.x - 8.0, y + 40.0)),
                Id::new("vthumb-custom"),
                &mut self.options.custom_output_directory,
            );
            if tools::text_button(ui, br, Id::new("vthumb-custom-browse"), browse, idle).clicked()
                && let Some(f) = tools::pick_folder(s::VideoThumbnailerWindow_Select_output_folder_dialog(), &self.options.custom_output_directory)
            {
                self.options.custom_output_directory = f;
            }
            y += 40.0;
        }
        y += 10.0;
        let r = Rect::from_min_size(pos2(q.min.x, y), vec2(q.width(), 32.0));
        Self::toggle_row(ui, r, Id::new("vthumb-upload"), s::VideoThumbnailerWindow_Upload_created_thumbnails(), &mut self.options.upload_thumbnails, true);
        let r = r.translate(vec2(0.0, 40.0));
        Self::toggle_row(ui, r, Id::new("vthumb-open"), s::VideoThumbnailerWindow_Open_output_folder_when_finished(), &mut self.options.open_directory, true);

        // Right: the contact sheet.
        tools::panel(ui, right);
        let q = right.shrink(13.0);
        let header = Rect::from_min_size(q.min, vec2(q.width(), 32.0));
        tools::heading(ui, header, s::VideoThumbnailerWindow_Combined_contact_sheet());
        let size = ToggleSwitch::SIZE;
        ToggleSwitch { enabled: idle }.show(ui, pos2(header.max.x - size.x, header.min.y), Id::new("vthumb-combine"), &mut self.options.combine_screenshots);
        let enabled = self.options.combine_screenshots && idle;
        let mut y = header.max.y + 12.0;
        let o = &mut self.options;
        for (k, (label, value, max)) in [
            (s::VideoThumbnailerWindow_Columns(), &mut o.column_count, 20),
            (s::VideoThumbnailerWindow_Outer_padding(), &mut o.padding, 500),
            (s::VideoThumbnailerWindow_Thumbnail_spacing(), &mut o.spacing, 500),
        ]
        .into_iter()
        .enumerate()
        {
            let control = tools::label_row(ui, Rect::from_min_size(pos2(q.min.x, y), vec2(q.width(), 40.0)), 150.0, label);
            let min = if k == 0 { 1 } else { 0 };
            let mut v = *value as i64;
            if (NumericUpDown { minimum: min, maximum: max, enabled }).show(ui, control, Id::new(("vthumb-sheet", k)), &mut v) {
                *value = (v as i32).max(min as i32);
            }
            y += 50.0;
        }
        for (k, (label, value)) in [
            (s::VideoThumbnailerWindow_Add_video_information(), &mut o.add_video_info),
            (s::VideoThumbnailerWindow_Show_timestamps(), &mut o.add_timestamp),
            (s::VideoThumbnailerWindow_Draw_thumbnail_shadows(), &mut o.draw_shadow),
            (s::VideoThumbnailerWindow_Draw_thumbnail_borders(), &mut o.draw_border),
            (s::VideoThumbnailerWindow_Keep_individual_screenshots(), &mut o.keep_screenshots),
        ]
        .into_iter()
        .enumerate()
        {
            Self::toggle_row(ui, Rect::from_min_size(pos2(q.min.x, y), vec2(q.width(), 32.0)), Id::new(("vthumb-toggle", k)), label, value, enabled);
            y += 42.0;
        }

        // Row 2.
        let mut y = row2.min.y;
        if !idle {
            let (v, max) = self.progress;
            sharex_ui::controls::determinate_progress(ui.painter(), Rect::from_min_size(row2.min, vec2(row2.width(), 5.0)), v as f64 / max.max(1) as f64);
            y += 5.0;
        }
        y += 8.0;
        let row = Rect::from_min_max(pos2(row2.min.x, y), row2.max);
        if err_h > 0.0 {
            sharex_ui::controls::wrapped(ui, pos2(row.min.x, center_y(row, err_h)), err_w, &self.error_message, nf.clone(), p.cancel_border, true);
        }
        let br = Rect::from_min_size(pos2(row.max.x - bw, center_y(row, tools::BUTTON_HEIGHT)), vec2(bw, tools::BUTTON_HEIGHT));
        if tools::icon_text_button(ui, br, Id::new("vthumb-start"), icon::Images, text, self.can_start()).clicked() {
            self.start();
        }
        if before != self.options {
            self.save(cx);
        }
        effects
    }

    fn dropped(&mut self, files: Vec<std::path::PathBuf>, _cx: &mut Cx) -> Vec<Effect> {
        if let Some(f) = files.first() {
            self.load_video(&f.to_string_lossy());
        }
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_start_from_the_task_and_clamp() {
        let defaults = TaskSettings::default();
        let w = VideoThumbnailerWindow::new(TaskSettings::default_task_settings(&defaults), &defaults, "ffmpeg".into(), "shots".into());
        assert_eq!(w.options.thumbnail_count, 9);
        assert!(w.options.upload_thumbnails);
        assert_eq!(w.options.default_output_directory.as_deref(), Some("shots"));
        assert!(w.video_path.is_empty());
        assert!(!w.can_start());
    }

    #[test]
    fn loading_a_video_clears_the_error() {
        let defaults = TaskSettings::default();
        let mut w = VideoThumbnailerWindow::new(TaskSettings::default_task_settings(&defaults), &defaults, "ffmpeg".into(), String::new());
        w.error_message = "x".into();
        w.load_video("clip.mp4");
        assert_eq!(w.video_path, "clip.mp4");
        assert_eq!(w.options.last_video_path.as_deref(), Some("clip.mp4"));
        assert!(w.error_message.is_empty());
    }
}
