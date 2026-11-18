//! `ShareX.Tools/Tools/ImageSplitter` (`ImageSplitterWindow.axaml`,
//! `ImageSplitterViewModel`): the image and output folder pickers (the
//! folder defaults to the image's), the columns and rows with the grid size
//! between them, and Split image, which writes `<name><n>.png` into the
//! folder and opens it with the first file. A dropped file is the image.

use egui::{Id, Rect, Ui, pos2, vec2};
use sharex_strings::tools as s;
use sharex_ui::controls::NumericUpDown;
use sharex_ui::keys::app_icons as icon;
use sharex_ui::theme::palette;
use sharex_ui::widgets::{center_y, font, line_height, text_width};

use crate::dialogs::{Cx, Dialog, Effect, Spec};

pub const WIDTH: f32 = 680.0;
pub const MIN_WIDTH: f32 = 580.0;

pub struct ImageSplitterWindow {
    pub image_file_path: String,
    pub output_folder_path: String,
    pub row_count: i64,
    pub column_count: i64,
    busy: bool,
}

impl Default for ImageSplitterWindow {
    fn default() -> Self {
        Self { image_file_path: String::new(), output_folder_path: String::new(), row_count: 1, column_count: 1, busy: false }
    }
}

impl ImageSplitterWindow {
    /// `SetImageFile`.
    pub fn set_image_file(&mut self, path: &str) {
        if !super::file_exists(path) {
            return;
        }
        self.image_file_path = path.to_string();
        if self.output_folder_path.trim().is_empty() {
            self.output_folder_path = super::directory_name(path);
        }
    }

    /// `CanProcess`.
    fn can_process(&self) -> bool {
        !self.busy && super::file_exists(&self.image_file_path) && super::dir_exists(&self.output_folder_path) && (self.row_count > 1 || self.column_count > 1)
    }

    /// `SplitImageAsync`.
    fn split(&mut self) -> Vec<Effect> {
        if !self.can_process() {
            return Vec::new();
        }
        match sharex_tools::splitter::split_image_file(&self.image_file_path, self.row_count as i32, self.column_count as i32, &self.output_folder_path) {
            Ok(files) => super::open_folder_with_first(&files),
            Err(e) => {
                eprintln!("ImageSplitterViewModel: Failed to split image. {e}");
                Vec::new()
            }
        }
    }

    /// The panel heights: a picker row, the layout panel.
    fn picker_height() -> f32 {
        2.0 + 24.0 + super::BUTTON_HEIGHT
    }

    fn layout_height(ui: &Ui) -> f32 {
        2.0 + 32.0 + super::heading_height(ui) + 14.0 + line_height(ui, &super::normal_font()) + 6.0 + super::CONTROL_HEIGHT
    }

    fn total_height(ui: &Ui) -> f32 {
        super::MARGIN * 2.0 + Self::picker_height() * 2.0 + Self::layout_height(ui) + super::BUTTON_HEIGHT + 12.0 * 3.0
    }

    /// One picker row: the heading, the path and Browse...
    fn picker(&mut self, ui: &Ui, rect: Rect, k: usize, title: &str, path: &str) -> bool {
        super::panel(ui, rect);
        let c = rect.shrink(13.0);
        let hf = super::heading_font();
        let hw = text_width(ui, title, hf.clone());
        super::text_at(ui, pos2(c.min.x, center_y(c, line_height(ui, &hf))), title, hf, palette(ui.ctx()).text);
        let browse = s::ImageSplitterWindow_Browse();
        let bw = super::text_button_width(ui, browse);
        let br = Rect::from_min_size(pos2(c.max.x - bw, c.min.y), vec2(bw, super::BUTTON_HEIGHT));
        super::path_text(ui, Rect::from_min_max(pos2(c.min.x + hw + 12.0, c.min.y), pos2(br.min.x - 12.0, c.max.y)), path);
        super::text_button(ui, br, Id::new(("splitter-browse", k)), browse, true).clicked()
    }
}

impl Dialog for ImageSplitterWindow {
    fn spec(&self) -> Spec {
        let mut spec = Spec::fit_height(crate::rs_title(s::ImageSplitterWindow_ShareX_Image_Splitter()), WIDTH, MIN_WIDTH);
        // `SizeToContent="Height"` keeps `CanResize` (the width).
        spec.resizable = true;
        spec
    }

    fn content_height(&mut self, ui: &mut Ui) -> f32 {
        Self::total_height(ui)
    }

    fn ui(&mut self, ui: &mut Ui, _cx: &mut Cx) -> Vec<Effect> {
        let p = palette(ui.ctx());
        let c = super::background(ui);
        let mut y = c.min.y;
        let ph = Self::picker_height();
        let image = self.image_file_path.clone();
        if self.picker(ui, Rect::from_min_size(pos2(c.min.x, y), vec2(c.width(), ph)), 0, s::ImageSplitterWindow_Image(), &image)
            && let Some(f) = super::pick_image(s::ImageSplitterWindow_Choose_image_dialog())
        {
            self.set_image_file(&f);
        }
        y += ph + 12.0;
        let folder = self.output_folder_path.clone();
        if self.picker(ui, Rect::from_min_size(pos2(c.min.x, y), vec2(c.width(), ph)), 1, s::ImageSplitterWindow_Output_folder(), &folder)
            && let Some(f) = super::pick_folder(s::ImageSplitterWindow_Choose_output_folder_dialog(), &self.output_folder_path)
        {
            self.output_folder_path = f;
        }
        y += ph + 12.0;
        let lh = Self::layout_height(ui);
        let panel = Rect::from_min_size(pos2(c.min.x, y), vec2(c.width(), lh));
        super::panel(ui, panel);
        let inner = panel.shrink(17.0);
        let hh = super::heading_height(ui);
        super::heading(ui, Rect::from_min_size(inner.min, vec2(inner.width(), hh)), s::ImageSplitterWindow_Split_layout());
        // `ColumnDefinitions="*,Auto,*" ColumnSpacing="24"`, the grid size text 28px SemiBold in the accent text colour.
        let gf = font(28.0, 600);
        let grid = s::ImageSplitterViewModel_Grid_size().replace("{0}", &self.column_count.to_string()).replace("{1}", &self.row_count.to_string());
        let gw = text_width(ui, &grid, gf.clone());
        let row_top = inner.min.y + hh + 14.0;
        let row = Rect::from_min_max(pos2(inner.min.x, row_top), pos2(inner.max.x, inner.max.y));
        let star = ((row.width() - gw - 48.0) / 2.0).max(0.0);
        let nl = line_height(ui, &super::normal_font());
        for (k, (label, x)) in [(s::ImageSplitterWindow_Columns(), row.min.x), (s::ImageSplitterWindow_Rows(), row.max.x - star)].into_iter().enumerate() {
            super::row_label(ui, Rect::from_min_size(pos2(x, row.min.y), vec2(star, nl)), label);
            let value = if k == 0 { &mut self.column_count } else { &mut self.row_count };
            NumericUpDown { minimum: 1, maximum: 100, enabled: true }.show(ui, Rect::from_min_size(pos2(x, row.min.y + nl + 6.0), vec2(star, super::CONTROL_HEIGHT)), Id::new(("splitter-count", k)), value);
        }
        let gx = row.min.x + star + 24.0;
        super::text_at(ui, pos2(gx, center_y(row, line_height(ui, &gf))), &grid, gf, p.accent_text);
        y += lh + 12.0;
        let text = s::ImageSplitterWindow_Split_image();
        let bw = super::icon_text_button_width(ui, text);
        let br = Rect::from_min_size(pos2(c.max.x - bw, y), vec2(bw, super::BUTTON_HEIGHT));
        if super::icon_text_button(ui, br, Id::new("splitter-split"), icon::Split, text, self.can_process()).clicked() {
            return self.split();
        }
        Vec::new()
    }

    fn dropped(&mut self, files: Vec<std::path::PathBuf>, _cx: &mut Cx) -> Vec<Effect> {
        if let Some(f) = files.first() {
            self.set_image_file(&f.to_string_lossy());
        }
        Vec::new()
    }
}
