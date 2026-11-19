//! `ShareX.Tools/Tools/ImageWatermark` (`ImageWatermarkWindow.axaml`,
//! `ImageWatermarkViewModel`, `ImageWatermarkService`): the image list (the
//! first file selected), a text watermark (text, size, colour) or an image
//! one (file, scale), its position, margin, opacity and rotation, the
//! output format (JPEG quality and background), the file name template and
//! folder, the preview 100 ms after the last change, and Apply watermark.

use egui::{Color32, Id, Rect, Ui, pos2, vec2};
use sharex_strings::tools as s;
use sharex_tools::watermark::{OutputFormat, Position, WatermarkOptions, WatermarkType};
use sharex_ui::controls::{ComboBox, ComboItem, NumericUpDown, TextBox};
use sharex_ui::keys::app_icons as icon;
use sharex_ui::theme::Thickness;
use sharex_ui::widgets::line_height;

use super::{FileList, Job, ListAction, Picture};
use crate::dialogs::{Cx, Dialog, Effect, Spec};

pub const WIDTH: f32 = 1220.0;
pub const HEIGHT: f32 = 700.0;
pub const MIN_WIDTH: f32 = 1000.0;
pub const MIN_HEIGHT: f32 = 600.0;

type Preview = (Vec<u8>, u32, u32);

pub struct ImageWatermarkWindow {
    pub list: FileList,
    pub kind: WatermarkType,
    pub text: String,
    pub image_path: String,
    pub text_size: i64,
    pub text_color: Color32,
    pub image_scale: f32,
    pub position: usize,
    pub margin: i64,
    pub opacity: f32,
    pub rotation: f32,
    pub format: OutputFormat,
    pub quality: f32,
    pub background: Color32,
    pub output_folder_path: String,
    pub output_file_name: String,
    pub message: String,
    preview: Option<Picture>,
    preview_size: String,
    job: Job<Preview>,
    busy: bool,
}

impl Default for ImageWatermarkWindow {
    fn default() -> Self {
        Self {
            list: FileList::semibold(),
            kind: WatermarkType::Text,
            text: s::ImageWatermarkWindow_Watermark().into(),
            image_path: String::new(),
            text_size: 48,
            text_color: Color32::WHITE,
            image_scale: 25.0,
            position: 8,
            margin: 24,
            opacity: 70.0,
            rotation: 0.0,
            format: OutputFormat::Png,
            quality: 90.0,
            background: Color32::WHITE,
            output_folder_path: String::new(),
            output_file_name: sharex_tools::watermark::DEFAULT_FILE_NAME.into(),
            message: String::new(),
            preview: None,
            preview_size: String::new(),
            job: Job::new(100),
            busy: false,
        }
    }
}

impl ImageWatermarkWindow {
    pub fn set_sync(&mut self) {
        self.job.sync = true;
    }

    /// `GetOptions`.
    pub fn options(&self) -> WatermarkOptions {
        WatermarkOptions {
            kind: self.kind,
            text: self.text.clone(),
            position: Position::ALL[self.position.min(8)],
            margin: self.margin as i32,
            opacity: self.opacity as i32,
            text_size: self.text_size as f32,
            text_color: super::rgba(self.text_color),
            image_scale: self.image_scale as i32,
            rotation: self.rotation as i32 as f32,
        }
    }

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

    fn options_changed(&mut self) {
        self.message.clear();
        if !super::file_exists(&self.preview_file()) {
            self.job.cancel();
            self.preview = None;
            self.preview_size.clear();
            return;
        }
        self.job.queue();
    }

    fn has_valid_watermark(&self) -> bool {
        match self.kind {
            WatermarkType::Text => !self.text.trim().is_empty(),
            WatermarkType::Image => super::file_exists(&self.image_path),
        }
    }

    fn can_apply(&self) -> bool {
        !self.busy && !self.list.items.is_empty() && self.has_valid_watermark() && super::dir_exists(&self.output_folder_path) && !self.output_file_name.trim().is_empty()
    }

    fn apply(&mut self) -> Vec<Effect> {
        if !self.can_apply() {
            return Vec::new();
        }
        self.message.clear();
        let r = sharex_tools::watermark::watermark_images(
            &self.list.items,
            &self.image_path,
            &self.options(),
            self.format,
            self.quality as i32,
            super::rgba(self.background),
            &self.output_folder_path,
            &self.output_file_name,
        );
        match r {
            Ok(files) => super::open_folder_with_first(&files),
            Err(e) => {
                eprintln!("ImageWatermarkViewModel: Failed to watermark images. {e}");
                self.message = e.to_string();
                Vec::new()
            }
        }
    }

    fn poll_preview(&mut self, ctx: &egui::Context) {
        let file = self.preview_file();
        let image = self.image_path.clone();
        let (o, format, quality, bg) = (self.options(), self.format, self.quality as i32, super::rgba(self.background));
        if let Some((bytes, w, h)) = self.job.poll(ctx, move || Box::new(move || sharex_tools::watermark::create_preview(&file, &image, &o, format, quality, bg))) {
            self.preview = if bytes.is_empty() { None } else { Picture::decode(&bytes) };
            self.preview_size = if w > 0 && h > 0 { format!("{w} × {h}") } else { String::new() };
        }
    }

    fn combo(ui: &Ui, rect: Rect, id: &str, items: &[&str], selected: usize) -> Option<usize> {
        let items: Vec<ComboItem> = items.iter().map(|t| ComboItem::text(*t)).collect();
        let combo = ComboBox { padding: Thickness::hv(8.0, 6.0), enabled: true };
        let cw = combo.auto_width(ui, &items, Some(selected)).min(rect.width());
        combo.show(ui, Rect::from_min_size(rect.min, vec2(cw, 40.0)), Id::new(id), &items, Some(selected))
    }

    fn watermark_panel_rows(&self, ui: &Ui) -> Vec<f32> {
        let hh = super::heading_height(ui);
        let group = match self.kind {
            WatermarkType::Text => super::stack_height(&[40.0, 40.0, 32.0], 10.0),
            WatermarkType::Image => super::stack_height(&[40.0, super::SLIDER_HEIGHT], 10.0),
        };
        vec![hh, 40.0, group, 40.0, 40.0, super::SLIDER_HEIGHT, super::SLIDER_HEIGHT]
    }

    fn output_panel_rows(&self, ui: &Ui) -> Vec<f32> {
        let hh = super::heading_height(ui);
        let nl = line_height(ui, &super::normal_font());
        let jpeg = self.format == OutputFormat::Jpeg;
        let mut rows = vec![hh, 40.0];
        if jpeg {
            rows.push(super::SLIDER_HEIGHT);
            rows.push(32.0);
        }
        rows.extend([nl, 40.0, 40.0]);
        rows
    }

    fn options_column(&mut self, ui: &mut Ui, col: Rect) {
        let hh = super::heading_height(ui);
        let nl = line_height(ui, &super::normal_font());
        let h1 = 2.0 + 32.0 + super::stack_height(&self.watermark_panel_rows(ui), 10.0);
        let h2 = 2.0 + 32.0 + super::stack_height(&self.output_panel_rows(ui), 10.0);
        let message_h = if self.message.trim().is_empty() { 0.0 } else { super::message_panel(ui, col.min, col.width(), &self.message, false) };
        let content_h = super::stack_height(&[h1, h2, message_h], 12.0);
        let scroll_id = Id::new("watermark-scroll");
        let offset = crate::dialogs::layout::scroll_offset(ui, scroll_id, col, content_h);
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(col));
        child.set_clip_rect(col.intersect(ui.clip_rect()));
        let ui = &mut child;
        let before = (self.options(), self.format, self.quality as i32, self.background, self.image_path.clone());
        let panel = Rect::from_min_size(pos2(col.min.x, col.min.y - offset), vec2(col.width(), h1));
        super::panel(ui, panel);
        let p = panel.shrink(17.0);
        super::heading(ui, Rect::from_min_size(p.min, vec2(p.width(), hh)), s::ImageWatermarkWindow_Watermark());
        let mut y = p.min.y + hh + 10.0;
        let row = |y: f32, h: f32| Rect::from_min_size(pos2(p.min.x, y), vec2(p.width(), h));
        let control = super::label_row(ui, row(y, 40.0), 110.0, s::ImageWatermarkWindow_Watermark_type());
        let kinds = [s::DirectoryIndexerViewModel_Text(), s::AnalyzeImageWindow_Image()];
        if let Some(i) = Self::combo(ui, control, "watermark-type", &kinds, usize::from(self.kind == WatermarkType::Image)) {
            self.kind = if i == 1 { WatermarkType::Image } else { WatermarkType::Text };
        }
        y += 50.0;
        match self.kind {
            WatermarkType::Text => {
                let control = super::label_row(ui, row(y, 40.0), 110.0, s::DirectoryIndexerViewModel_Text());
                TextBox::default().show(ui, control, Id::new("watermark-text"), &mut self.text);
                y += 50.0;
                let control = super::label_row(ui, row(y, 40.0), 110.0, s::ImageWatermarkWindow_Font_size());
                NumericUpDown { minimum: 1, maximum: 500, enabled: true }.show(ui, control, Id::new("watermark-size"), &mut self.text_size);
                y += 50.0;
                let control = super::label_row(ui, row(y, 32.0), 110.0, s::MonitorTestWindow_Color());
                super::color_picker(ui, control, Id::new("watermark-color"), &mut self.text_color, true);
                y += 42.0;
            }
            WatermarkType::Image => {
                // `ColumnDefinitions="110,*,Auto" ColumnSpacing="8"`.
                let r = row(y, 40.0);
                super::row_label(ui, Rect::from_min_size(r.min, vec2(110.0, 40.0)), s::AnalyzeImageWindow_Image());
                let browse = s::ImageThumbnailerWindow_Browse();
                let bw = super::text_button_width(ui, browse);
                let br = Rect::from_min_size(pos2(r.max.x - bw, r.min.y), vec2(bw, 40.0));
                super::path_text(ui, Rect::from_min_max(pos2(r.min.x + 118.0, r.min.y), pos2(br.min.x - 8.0, r.max.y)), &self.image_path);
                if super::text_button(ui, br, Id::new("watermark-image-browse"), browse, true).clicked()
                    && let Some(f) = super::pick_image(s::IconConverterViewModel_Select_image_dialog())
                {
                    self.image_path = f;
                }
                y += 50.0;
                super::slider_row(ui, row(y, super::SLIDER_HEIGHT), 110.0, s::ImageWatermarkWindow_Scale(), Id::new("watermark-scale"), &mut self.image_scale, (1.0, 100.0), super::semibold_font(), &|v| format!("{}%", super::format_0(v)), true);
                y += super::SLIDER_HEIGHT + 10.0;
            }
        }
        let control = super::label_row(ui, row(y, 40.0), 110.0, s::ImageWatermarkWindow_Position());
        let positions = [
            s::PinToScreenOptionsViewModel_Top_left(),
            s::PinToScreenOptionsViewModel_Top_center(),
            s::PinToScreenOptionsViewModel_Top_right(),
            s::PinToScreenOptionsViewModel_Middle_left(),
            s::PinToScreenOptionsViewModel_Center(),
            s::PinToScreenOptionsViewModel_Middle_right(),
            s::PinToScreenOptionsViewModel_Bottom_left(),
            s::PinToScreenOptionsViewModel_Bottom_center(),
            s::PinToScreenOptionsViewModel_Bottom_right(),
        ];
        if let Some(i) = Self::combo(ui, control, "watermark-position", &positions, self.position) {
            self.position = i;
        }
        y += 50.0;
        let control = super::label_row(ui, row(y, 40.0), 110.0, s::ImageWatermarkWindow_Margin());
        NumericUpDown { minimum: 0, maximum: 10000, enabled: true }.show(ui, control, Id::new("watermark-margin"), &mut self.margin);
        y += 50.0;
        super::slider_row(ui, row(y, super::SLIDER_HEIGHT), 110.0, s::InspectWindowWindow_Opacity(), Id::new("watermark-opacity"), &mut self.opacity, (0.0, 100.0), super::semibold_font(), &|v| format!("{}%", super::format_0(v)), true);
        y += super::SLIDER_HEIGHT + 10.0;
        super::slider_row(ui, row(y, super::SLIDER_HEIGHT), 110.0, s::ImageWatermarkWindow_Rotation(), Id::new("watermark-rotation"), &mut self.rotation, (-180.0, 180.0), super::semibold_font(), &|v| format!("{}°", super::format_0(v)), true);
        // The output panel.
        let panel = Rect::from_min_size(pos2(col.min.x, panel.max.y + 12.0), vec2(col.width(), h2));
        super::panel(ui, panel);
        let p = panel.shrink(17.0);
        let row = |y: f32, h: f32| Rect::from_min_size(pos2(p.min.x, y), vec2(p.width(), h));
        super::heading(ui, row(p.min.y, hh), s::ImageThumbnailerWindow_Output());
        let mut y = p.min.y + hh + 10.0;
        let control = super::label_row(ui, row(y, 40.0), 110.0, s::DirectoryIndexerWindow_Output_format());
        if let Some(i) = Self::combo(ui, control, "watermark-format", &["PNG", "JPEG"], usize::from(self.format == OutputFormat::Jpeg)) {
            self.format = if i == 1 { OutputFormat::Jpeg } else { OutputFormat::Png };
        }
        y += 50.0;
        if self.format == OutputFormat::Jpeg {
            super::slider_row(ui, row(y, super::SLIDER_HEIGHT), 110.0, s::ImageThumbnailerWindow_JPEG_quality(), Id::new("watermark-quality"), &mut self.quality, (0.0, 100.0), super::semibold_font(), &|v| format!("{}%", super::format_0(v)), true);
            y += super::SLIDER_HEIGHT + 10.0;
            let control = super::label_row(ui, row(y, 32.0), 110.0, s::PinToScreenOptionsWindow_Background_color());
            super::color_picker(ui, control, Id::new("watermark-background"), &mut self.background, true);
            y += 42.0;
        }
        super::row_label(ui, row(y, nl), s::ImageThumbnailerWindow_File_name_template());
        y += nl + 10.0;
        let name_before = self.output_file_name.clone();
        TextBox::default().show(ui, row(y, 40.0), Id::new("watermark-name"), &mut self.output_file_name);
        y += 50.0;
        let browse = s::ImageThumbnailerWindow_Browse();
        let bw = super::text_button_width(ui, browse);
        let br = Rect::from_min_size(pos2(p.max.x - bw, y), vec2(bw, 40.0));
        super::path_text(ui, Rect::from_min_max(pos2(p.min.x, y), pos2(br.min.x - 8.0, y + 40.0)), &self.output_folder_path);
        if super::text_button(ui, br, Id::new("watermark-browse"), browse, true).clicked()
            && let Some(f) = super::pick_folder(s::ImageThumbnailerWindow_Choose_output_folder_dialog(), &self.output_folder_path)
        {
            self.output_folder_path = f;
            self.message.clear();
        }
        if message_h > 0.0 {
            super::message_panel(ui, pos2(col.min.x, panel.max.y + 12.0), col.width(), &self.message, true);
        }
        if name_before != self.output_file_name {
            self.message.clear();
        }
        if before != (self.options(), self.format, self.quality as i32, self.background, self.image_path.clone()) {
            self.options_changed();
        }
        crate::dialogs::layout::scroll_bar(ui, col, content_h, offset, 16.0);
    }
}

impl Dialog for ImageWatermarkWindow {
    fn spec(&self) -> Spec {
        Spec::sized(crate::rs_title(s::ImageWatermarkWindow_Image_watermark()), WIDTH, HEIGHT, MIN_WIDTH, MIN_HEIGHT)
    }

    fn ui(&mut self, ui: &mut Ui, _cx: &mut Cx) -> Vec<Effect> {
        self.poll_preview(ui.ctx());
        let c = super::background(ui);
        let bottom = Rect::from_min_max(pos2(c.min.x, c.max.y - super::BUTTON_HEIGHT), c.max);
        let main = Rect::from_min_max(c.min, pos2(c.max.x, bottom.min.y - 12.0));
        let left = Rect::from_min_size(main.min, vec2(340.0, main.height()));
        let middle = Rect::from_min_size(pos2(left.max.x + 12.0, main.min.y), vec2(360.0, main.height()));
        let right = Rect::from_min_max(pos2(middle.max.x + 12.0, main.min.y), main.max);
        let count = super::count_text(s::ImageThumbnailerViewModel_One_image(), s::ImageThumbnailerViewModel_Image_count(), self.list.items.len());
        let (action, changed) = super::list_panel(
            ui,
            left,
            "watermark",
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
                self.list.select((!self.list.items.is_empty()).then_some(0));
                self.options_changed();
            }
            _ => {
                if changed {
                    self.options_changed();
                }
            }
        }
        self.options_column(ui, middle);
        let path = self.preview_file();
        let loading = self.job.busy.then(|| self.job.busy_since.elapsed().as_secs_f64());
        let size = self.preview_size.clone();
        let pressed = super::preview_pane(ui, right, &path, &size, self.preview.as_mut(), loading, !self.list.items.is_empty());
        let text = s::ImageWatermarkWindow_Apply_watermark();
        let bw = super::icon_text_button_width(ui, text);
        let br = Rect::from_min_size(pos2(bottom.max.x - bw, bottom.min.y), vec2(bw, super::BUTTON_HEIGHT));
        if super::icon_text_button(ui, br, Id::new("watermark-apply"), icon::Stamp, text, self.can_apply()).clicked() {
            return self.apply();
        }
        if pressed {
            return super::show_preview(self.preview.as_ref(), &path);
        }
        Vec::new()
    }

    fn dropped(&mut self, files: Vec<std::path::PathBuf>, _cx: &mut Cx) -> Vec<Effect> {
        self.add_files(files.into_iter().map(|f| f.to_string_lossy().into_owned()).collect());
        Vec::new()
    }
}
