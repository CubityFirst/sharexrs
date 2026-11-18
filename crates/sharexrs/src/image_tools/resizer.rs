//! `ShareX.Tools/Tools/ImageResizer` (`ImageResizerWindow.axaml`,
//! `ImageResizerViewModel`, `ImageResizerService`): the image list (the
//! first file selected), width/height and the resize mode, the output
//! format (JPEG with its quality and background colour), the file name
//! template and folder, the preview of the selected file 100 ms after the
//! last change (`CreatePreview`, at most 1600 px), and Resize image.
//! Opened from Tools and from the task list's Resize image (with files).

use egui::{Color32, Id, Rect, Ui, pos2, vec2};
use sharex_strings::tools as s;
use sharex_tools::resizer::{OutputFormat, ResizeMode};
use sharex_ui::controls::{ComboBox, ComboItem, NumericUpDown, TextBox};
use sharex_ui::keys::app_icons as icon;
use sharex_ui::theme::Thickness;
use sharex_ui::widgets::line_height;

use super::{FileList, Job, ListAction, Picture};
use crate::dialogs::{Cx, Dialog, Effect, Spec};

pub const WIDTH: f32 = 1180.0;
pub const HEIGHT: f32 = 650.0;
pub const MIN_WIDTH: f32 = 960.0;
pub const MIN_HEIGHT: f32 = 560.0;

pub struct ImageResizerWindow {
    pub list: FileList,
    pub width: i64,
    pub height: i64,
    pub mode: ResizeMode,
    pub format: OutputFormat,
    pub quality: f32,
    pub background: Color32,
    pub output_folder_path: String,
    pub output_file_name: String,
    pub message: String,
    preview: Option<Picture>,
    job: Job<Result<Vec<u8>, String>>,
    busy: bool,
}

impl Default for ImageResizerWindow {
    fn default() -> Self {
        Self {
            list: FileList::semibold(),
            width: 1920,
            height: 1080,
            mode: ResizeMode::Fit,
            format: OutputFormat::Png,
            quality: 90.0,
            background: Color32::WHITE,
            output_folder_path: String::new(),
            output_file_name: "$filename_resized".into(),
            message: String::new(),
            preview: None,
            job: Job::new(100),
            busy: false,
        }
    }
}

impl ImageResizerWindow {
    pub fn new(files: Vec<String>) -> Self {
        let mut w = Self::default();
        w.add_files(files);
        w
    }

    pub fn set_sync(&mut self) {
        self.job.sync = true;
    }

    /// `AddFiles`: the first file becomes the selected one.
    pub fn add_files(&mut self, files: Vec<String>) {
        for f in files.into_iter().filter(|f| super::file_exists(f)) {
            if self.output_folder_path.trim().is_empty() {
                self.output_folder_path = super::directory_name(&f);
            }
            self.list.items.push(f);
        }
        if self.list.primary.is_none() && !self.list.items.is_empty() {
            self.list.select(Some(0));
        }
        self.options_changed();
    }

    fn preview_file(&self) -> String {
        self.list.selected_item().or(self.list.items.first()).cloned().unwrap_or_default()
    }

    /// `QueuePreviewRefresh` / `RefreshPreviewAsync`.
    fn options_changed(&mut self) {
        self.message.clear();
        let file = self.preview_file();
        if !super::file_exists(&file) || self.width < 1 || self.height < 1 {
            self.job.cancel();
            self.preview = None;
            return;
        }
        self.job.queue();
    }

    fn can_resize(&self) -> bool {
        !self.busy && !self.list.items.is_empty() && self.width > 0 && self.height > 0 && super::dir_exists(&self.output_folder_path) && !self.output_file_name.trim().is_empty()
    }

    /// `ResizeAsync`.
    fn resize(&mut self) -> Vec<Effect> {
        if !self.can_resize() {
            return Vec::new();
        }
        self.message.clear();
        let r = sharex_tools::resizer::resize_images(
            &self.list.items,
            self.width as i32,
            self.height as i32,
            self.mode,
            self.format,
            self.quality as i32,
            super::rgba(self.background),
            &self.output_folder_path,
            &self.output_file_name,
        );
        match r {
            Ok(files) => super::open_folder_with_first(&files),
            Err(e) => {
                eprintln!("ImageResizerViewModel: Failed to resize images. {e}");
                self.message = e;
                Vec::new()
            }
        }
    }

    fn poll_preview(&mut self, ctx: &egui::Context) {
        let file = self.preview_file();
        let (w, h, mode, format, quality, bg) = (self.width as i32, self.height as i32, self.mode, self.format, self.quality as i32, super::rgba(self.background));
        if let Some(r) = self.job.poll(ctx, move || Box::new(move || sharex_tools::resizer::create_preview(&file, w, h, mode, format, quality, bg))) {
            match r {
                Ok(bytes) => self.preview = Picture::decode(&bytes),
                Err(e) => {
                    self.preview = None;
                    eprintln!("ImageResizerViewModel: Failed to create resize preview. {e}");
                }
            }
        }
    }

    fn is_jpeg(&self) -> bool {
        self.format == OutputFormat::Jpeg
    }

    /// The options column (`StackPanel Spacing="12"`).
    fn options(&mut self, ui: &mut Ui, col: Rect) {
        let hh = super::heading_height(ui);
        let nl = line_height(ui, &super::normal_font());
        let combo = ComboBox { padding: Thickness::hv(8.0, 6.0), enabled: true };
        // The resize panel.
        let h1 = 2.0 + 32.0 + super::grid_height(&[hh, 40.0, 40.0, 40.0], 10.0);
        let panel = Rect::from_min_size(col.min, vec2(col.width(), h1));
        super::panel(ui, panel);
        let p = panel.shrink(17.0);
        super::heading(ui, Rect::from_min_size(p.min, vec2(p.width(), hh)), s::ImageResizerWindow_Resize_image());
        let mut y = p.min.y + hh + 10.0;
        let before = (self.width, self.height, self.mode, self.format, self.quality, self.background);
        for (k, label) in [s::ImageThumbnailerWindow_Width(), s::ImageThumbnailerWindow_Height()].into_iter().enumerate() {
            let control = super::label_row(ui, Rect::from_min_size(pos2(p.min.x, y), vec2(p.width(), 40.0)), 90.0, label);
            let v = if k == 0 { &mut self.width } else { &mut self.height };
            NumericUpDown { minimum: 1, maximum: 10000, enabled: true }.show(ui, control, Id::new(("resizer-size", k)), v);
            y += 50.0;
        }
        let control = super::label_row(ui, Rect::from_min_size(pos2(p.min.x, y), vec2(p.width(), 40.0)), 90.0, s::ImageResizerWindow_Resize_mode());
        let modes = [s::ImageResizerWindow_Fill(), s::ImageResizerWindow_Fit(), s::ImageResizerWindow_Stretch()];
        let items: Vec<ComboItem> = modes.iter().map(|m| ComboItem::text(*m)).collect();
        let sel = match self.mode {
            ResizeMode::Fill => 0,
            ResizeMode::Fit => 1,
            ResizeMode::Stretch => 2,
        };
        let cw = combo.auto_width(ui, &items, Some(sel)).min(control.width());
        if let Some(i) = combo.show(ui, Rect::from_min_size(control.min, vec2(cw, 40.0)), Id::new("resizer-mode"), &items, Some(sel)) {
            self.mode = [ResizeMode::Fill, ResizeMode::Fit, ResizeMode::Stretch][i];
        }
        // The output panel: seven rows, the JPEG ones hidden for PNG.
        let jpeg = self.is_jpeg();
        let rows = [hh, 40.0, if jpeg { super::SLIDER_HEIGHT } else { 0.0 }, if jpeg { 32.0 } else { 0.0 }, nl, 40.0, 40.0];
        let h2 = 2.0 + 32.0 + super::grid_height(&rows, 10.0);
        let panel = Rect::from_min_size(pos2(col.min.x, panel.max.y + 12.0), vec2(col.width(), h2));
        super::panel(ui, panel);
        let p = panel.shrink(17.0);
        super::heading(ui, Rect::from_min_size(p.min, vec2(p.width(), hh)), s::ImageThumbnailerWindow_Output());
        let mut y = p.min.y + hh + 10.0;
        let control = super::label_row(ui, Rect::from_min_size(pos2(p.min.x, y), vec2(p.width(), 40.0)), 110.0, s::DirectoryIndexerWindow_Output_format());
        let items = [ComboItem::text("PNG"), ComboItem::text("JPEG")];
        let sel = usize::from(jpeg);
        let cw = combo.auto_width(ui, &items, Some(sel)).min(control.width());
        if let Some(i) = combo.show(ui, Rect::from_min_size(control.min, vec2(cw, 40.0)), Id::new("resizer-format"), &items, Some(sel)) {
            self.format = if i == 1 { OutputFormat::Jpeg } else { OutputFormat::Png };
        }
        y += 40.0 + 10.0;
        if jpeg {
            super::slider_row(
                ui,
                Rect::from_min_size(pos2(p.min.x, y), vec2(p.width(), super::SLIDER_HEIGHT)),
                110.0,
                s::ImageThumbnailerWindow_JPEG_quality(),
                Id::new("resizer-quality"),
                &mut self.quality,
                (0.0, 100.0),
                super::semibold_font(),
                &|v| format!("{}%", super::format_0(v)),
                true,
            );
            y += super::SLIDER_HEIGHT;
        }
        y += 10.0;
        if jpeg {
            let control = super::label_row(ui, Rect::from_min_size(pos2(p.min.x, y), vec2(p.width(), 32.0)), 110.0, s::PinToScreenOptionsWindow_Background_color());
            super::color_picker(ui, control, Id::new("resizer-background"), &mut self.background, true);
            y += 32.0;
        }
        y += 10.0;
        super::row_label(ui, Rect::from_min_size(pos2(p.min.x, y), vec2(p.width(), nl)), s::ImageThumbnailerWindow_File_name_template());
        y += nl + 10.0;
        let name_before = self.output_file_name.clone();
        TextBox::default().show(ui, Rect::from_min_size(pos2(p.min.x, y), vec2(p.width(), 40.0)), Id::new("resizer-name"), &mut self.output_file_name);
        y += 50.0;
        let browse = s::ImageThumbnailerWindow_Browse();
        let bw = super::text_button_width(ui, browse);
        let br = Rect::from_min_size(pos2(p.max.x - bw, y), vec2(bw, 40.0));
        super::path_text(ui, Rect::from_min_max(pos2(p.min.x, y), pos2(br.min.x - 8.0, y + 40.0)), &self.output_folder_path);
        if super::text_button(ui, br, Id::new("resizer-browse"), browse, true).clicked()
            && let Some(f) = super::pick_folder(s::ImageThumbnailerWindow_Choose_output_folder_dialog(), &self.output_folder_path)
        {
            self.output_folder_path = f;
            self.message.clear();
        }
        if name_before != self.output_file_name {
            self.message.clear();
        }
        if before != (self.width, self.height, self.mode, self.format, self.quality, self.background) {
            self.options_changed();
        }
        if !self.message.trim().is_empty() {
            super::message_panel(ui, pos2(col.min.x, panel.max.y + 12.0), col.width(), &self.message, true);
        }
    }
}

impl Dialog for ImageResizerWindow {
    fn spec(&self) -> Spec {
        Spec::sized(crate::rs_title(s::ImageResizerWindow_Image_resizer()), WIDTH, HEIGHT, MIN_WIDTH, MIN_HEIGHT)
    }

    fn ui(&mut self, ui: &mut Ui, _cx: &mut Cx) -> Vec<Effect> {
        self.poll_preview(ui.ctx());
        let c = super::background(ui);
        let bottom = Rect::from_min_max(pos2(c.min.x, c.max.y - super::BUTTON_HEIGHT), c.max);
        let main = Rect::from_min_max(c.min, pos2(c.max.x, bottom.min.y - 12.0));
        let left = Rect::from_min_size(main.min, vec2(360.0, main.height()));
        let middle = Rect::from_min_size(pos2(left.max.x + 12.0, main.min.y), vec2(340.0, main.height()));
        let right = Rect::from_min_max(pos2(middle.max.x + 12.0, main.min.y), main.max);
        let count = super::count_text(s::ImageThumbnailerViewModel_One_image(), s::ImageThumbnailerViewModel_Image_count(), self.list.items.len());
        let (action, changed) = super::list_panel(
            ui,
            left,
            "resizer",
            &mut self.list,
            s::ImageThumbnailerWindow_Add(),
            &count,
            super::semibold_font(),
            s::ImageThumbnailerWindow_Drop_images_here_or_use_the_Add_button(),
            super::semibold_font(),
            false,
            true,
        );
        match action {
            ListAction::Add => {
                let files = super::pick_images(s::ImageThumbnailerWindow_Add_images_dialog());
                self.add_files(files);
            }
            ListAction::Remove => {
                self.list.remove_selected();
                // `SelectedImage = Images.FirstOrDefault()`.
                self.list.select((!self.list.items.is_empty()).then_some(0));
                self.options_changed();
            }
            _ => {
                if changed {
                    self.options_changed();
                }
            }
        }
        self.options(ui, middle);
        let size_text = format!("{} × {}", self.width, self.height);
        let path = self.preview_file();
        let loading = self.job.busy.then(|| self.job.busy_since.elapsed().as_secs_f64());
        super::preview_pane(ui, right, &path, &size_text, self.preview.as_mut(), loading, !self.list.items.is_empty());
        let text = s::ImageResizerWindow_Resize_image();
        let bw = super::icon_text_button_width(ui, text);
        let br = Rect::from_min_size(pos2(bottom.max.x - bw, bottom.min.y), vec2(bw, super::BUTTON_HEIGHT));
        if super::icon_text_button(ui, br, Id::new("resizer-resize"), icon::Resize, text, self.can_resize()).clicked() {
            return self.resize();
        }
        Vec::new()
    }

    fn dropped(&mut self, files: Vec<std::path::PathBuf>, _cx: &mut Cx) -> Vec<Effect> {
        self.add_files(files.into_iter().map(|f| f.to_string_lossy().into_owned()).collect());
        Vec::new()
    }
}
