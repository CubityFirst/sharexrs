//! `ShareX.Tools/Tools/ImageThumbnailer` (`ImageThumbnailerWindow.axaml`,
//! `ImageThumbnailerViewModel`): the image list, the thumbnail size and
//! JPEG quality, the file name template and output folder (the first
//! file's folder by default), and Generate thumbnails, which writes each
//! `<template>.jpg` (`CreateThumbnail` on white) and opens the folder with
//! the first one; the result in the message panel.

use egui::{Id, Rect, Ui, pos2, vec2};
use sharex_strings::tools as s;
use sharex_ui::controls::{NumericUpDown, TextBox};
use sharex_ui::keys::app_icons as icon;

use super::{FileList, ListAction};
use crate::dialogs::{Cx, Dialog, Effect, Spec};

pub const WIDTH: f32 = 900.0;
pub const HEIGHT: f32 = 590.0;
pub const MIN_WIDTH: f32 = 760.0;
pub const MIN_HEIGHT: f32 = 520.0;

pub struct ImageThumbnailerWindow {
    pub list: FileList,
    pub width: i64,
    pub height: i64,
    pub quality: f32,
    pub output_folder_path: String,
    pub output_file_name: String,
    pub message: String,
    busy: bool,
}

impl Default for ImageThumbnailerWindow {
    fn default() -> Self {
        Self {
            list: FileList::semibold(),
            width: sharex_tools::thumbnailer::DEFAULT_WIDTH as i64,
            height: sharex_tools::thumbnailer::DEFAULT_HEIGHT as i64,
            quality: sharex_tools::thumbnailer::DEFAULT_QUALITY as f32,
            output_folder_path: String::new(),
            output_file_name: sharex_tools::thumbnailer::DEFAULT_FILE_NAME.into(),
            message: String::new(),
            busy: false,
        }
    }
}

impl ImageThumbnailerWindow {
    /// `AddFiles`: the output folder is the first file's when unset.
    pub fn add_files(&mut self, files: Vec<String>) {
        for f in files.into_iter().filter(|f| super::file_exists(f)) {
            if self.output_folder_path.trim().is_empty() {
                self.output_folder_path = super::directory_name(&f);
            }
            self.list.items.push(f);
        }
        self.message.clear();
    }

    fn can_generate(&self) -> bool {
        !self.busy && !self.list.items.is_empty() && self.width > 0 && self.height > 0 && super::dir_exists(&self.output_folder_path) && !self.output_file_name.trim().is_empty()
    }

    /// `GenerateAsync`.
    fn generate(&mut self) -> Vec<Effect> {
        if !self.can_generate() {
            return Vec::new();
        }
        match sharex_tools::thumbnailer::generate(&self.list.items, self.width as i32, self.height as i32, self.quality.round() as i32, &self.output_folder_path, &self.output_file_name) {
            Ok(files) => {
                self.message = super::count_text(s::ImageThumbnailerViewModel_Generated_one(), s::ImageThumbnailerViewModel_Generated_many(), files.len());
                super::open_folder_with_first(&files)
            }
            Err(e) => {
                eprintln!("ImageThumbnailerViewModel: Failed to generate thumbnails. {e}");
                self.message = s::ImageThumbnailerViewModel_Generation_failed().replace("{0}", &e);
                Vec::new()
            }
        }
    }
}

impl Dialog for ImageThumbnailerWindow {
    fn spec(&self) -> Spec {
        Spec::sized(crate::rs_title(s::ImageThumbnailerWindow_ShareX_Image_Thumbnailer()), WIDTH, HEIGHT, MIN_WIDTH, MIN_HEIGHT)
    }

    fn ui(&mut self, ui: &mut Ui, _cx: &mut Cx) -> Vec<Effect> {
        let c = super::background(ui);
        let bottom = Rect::from_min_max(pos2(c.min.x, c.max.y - super::BUTTON_HEIGHT), c.max);
        let main = Rect::from_min_max(c.min, pos2(c.max.x, bottom.min.y - 12.0));
        let right = Rect::from_min_max(pos2(main.max.x - 360.0, main.min.y), main.max);
        let left = Rect::from_min_max(main.min, pos2(right.min.x - 12.0, main.max.y));
        let count = super::count_text(s::ImageThumbnailerViewModel_One_image(), s::ImageThumbnailerViewModel_Image_count(), self.list.items.len());
        let (action, _) = super::list_panel(
            ui,
            left,
            "thumbnailer",
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
                self.message.clear();
            }
            _ => {}
        }
        let hh = super::heading_height(ui);
        // The size panel.
        let size_h = 2.0 + 32.0 + hh + 10.0 + 40.0 + 10.0 + 40.0 + 10.0 + super::SLIDER_HEIGHT;
        let panel = Rect::from_min_size(right.min, vec2(right.width(), size_h));
        super::panel(ui, panel);
        let p = panel.shrink(17.0);
        super::heading(ui, Rect::from_min_size(p.min, vec2(p.width(), hh)), s::ImageThumbnailerWindow_Thumbnail_size());
        let mut y = p.min.y + hh + 10.0;
        let before = (self.width, self.height, self.quality);
        for (k, label) in [s::ImageThumbnailerWindow_Width(), s::ImageThumbnailerWindow_Height()].into_iter().enumerate() {
            let control = super::label_row(ui, Rect::from_min_size(pos2(p.min.x, y), vec2(p.width(), 40.0)), 90.0, label);
            let v = if k == 0 { &mut self.width } else { &mut self.height };
            NumericUpDown { minimum: 1, maximum: 10000, enabled: true }.show(ui, control, Id::new(("thumbnailer-size", k)), v);
            y += 50.0;
        }
        super::slider_row(
            ui,
            Rect::from_min_size(pos2(p.min.x, y), vec2(p.width(), super::SLIDER_HEIGHT)),
            90.0,
            s::ImageThumbnailerWindow_JPEG_quality(),
            Id::new("thumbnailer-quality"),
            &mut self.quality,
            (0.0, 100.0),
            super::normal_font(),
            &|v| format!("{}%", super::format_0(v)),
            true,
        );
        // The output panel.
        let nl = egui_line(ui);
        let out_h = 2.0 + 32.0 + hh + 10.0 + nl + 10.0 + 40.0 + 10.0 + 40.0;
        let panel = Rect::from_min_size(pos2(right.min.x, panel.max.y + 12.0), vec2(right.width(), out_h));
        super::panel(ui, panel);
        let p = panel.shrink(17.0);
        super::heading(ui, Rect::from_min_size(p.min, vec2(p.width(), hh)), s::ImageThumbnailerWindow_Output());
        let mut y = p.min.y + hh + 10.0;
        super::row_label(ui, Rect::from_min_size(pos2(p.min.x, y), vec2(p.width(), nl)), s::ImageThumbnailerWindow_File_name_template());
        y += nl + 10.0;
        let name_before = self.output_file_name.clone();
        TextBox { placeholder: s::ImageThumbnailerWindow_filename_th(), ..TextBox::default() }.show(ui, Rect::from_min_size(pos2(p.min.x, y), vec2(p.width(), 40.0)), Id::new("thumbnailer-name"), &mut self.output_file_name);
        y += 50.0;
        let browse = s::ImageThumbnailerWindow_Browse();
        let bw = super::text_button_width(ui, browse);
        let br = Rect::from_min_size(pos2(p.max.x - bw, y), vec2(bw, 40.0));
        super::path_text(ui, Rect::from_min_max(pos2(p.min.x, y), pos2(br.min.x - 8.0, y + 40.0)), &self.output_folder_path);
        if super::text_button(ui, br, Id::new("thumbnailer-browse"), browse, true).clicked()
            && let Some(f) = super::pick_folder(s::ImageThumbnailerWindow_Choose_output_folder_dialog(), &self.output_folder_path)
        {
            self.output_folder_path = f;
            self.message.clear();
        }
        if before != (self.width, self.height, self.quality) || name_before != self.output_file_name {
            self.message.clear();
        }
        if !self.message.trim().is_empty() {
            super::message_panel(ui, pos2(right.min.x, panel.max.y + 12.0), right.width(), &self.message, true);
        }
        let text = s::ImageThumbnailerWindow_Generate_thumbnails();
        let bw = super::icon_text_button_width(ui, text);
        let br = Rect::from_min_size(pos2(bottom.max.x - bw, bottom.min.y), vec2(bw, super::BUTTON_HEIGHT));
        if super::icon_text_button(ui, br, Id::new("thumbnailer-generate"), icon::Images, text, self.can_generate()).clicked() {
            return self.generate();
        }
        Vec::new()
    }

    fn dropped(&mut self, files: Vec<std::path::PathBuf>, _cx: &mut Cx) -> Vec<Effect> {
        self.add_files(files.into_iter().map(|f| f.to_string_lossy().into_owned()).collect());
        Vec::new()
    }
}

/// A row-label's line height.
fn egui_line(ui: &Ui) -> f32 {
    sharex_ui::widgets::line_height(ui, &super::normal_font())
}
