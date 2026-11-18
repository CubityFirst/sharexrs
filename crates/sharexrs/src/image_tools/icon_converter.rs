//! `ShareX.Tools/Tools/IconConverter` (`IconConverterWindow.axaml`,
//! `IconConverterViewModel`, `IconConverterService`): an image (Browse or
//! dropped), the icon sizes (16 to 256, all on) and the bit depth (32-bit by
//! default), the preview with the image's size, and Convert..., which asks
//! where to save the `.ico` (`<name>.ico`) and says where it went.

use egui::{Color32, Id, Rect, Ui, pos2, vec2};
use image::RgbaImage;
use sharex_strings::tools as s;
use sharex_tools::icon::IconBitDepth;
use sharex_ui::controls::{CheckBox, RadioButton};
use sharex_ui::keys::app_icons as icon;
use sharex_ui::theme::palette;
use sharex_ui::widgets::{center_y, font, line_height, text_width};

use super::Picture;
use crate::dialogs::{Cx, Dialog, Effect, Spec};

pub const WIDTH: f32 = 860.0;
pub const HEIGHT: f32 = 570.0;
pub const MIN_WIDTH: f32 = 720.0;
pub const MIN_HEIGHT: f32 = 500.0;

pub struct IconConverterWindow {
    pub image_path: Option<String>,
    source: Option<sharex_tools::imaging::Bitmap>,
    preview: Option<Picture>,
    pub image_details: String,
    pub status: String,
    pub sizes: [bool; 6],
    pub depth: IconBitDepth,
    /// Where Convert... saves without asking (tests).
    pub save_to: Option<String>,
}

impl Default for IconConverterWindow {
    fn default() -> Self {
        Self {
            image_path: None,
            source: None,
            preview: None,
            image_details: s::IconConverterViewModel_Drop_or_choose().into(),
            status: String::new(),
            sizes: [true; 6],
            depth: IconBitDepth::TrueColor32,
            save_to: None,
        }
    }
}

impl IconConverterWindow {
    /// `LoadImage`.
    pub fn load_image(&mut self, path: &str) {
        match sharex_tools::icon::load_image(path) {
            Some(bmp) => {
                let rgba = RgbaImage::from_raw(bmp.width(), bmp.height(), bmp.to_straight()).unwrap_or_default();
                self.preview = Some(Picture::new(rgba));
                self.image_details = s::IconConverterViewModel_Image_dimensions().replace("{0}", &bmp.width().to_string()).replace("{1}", &bmp.height().to_string());
                self.source = Some(bmp);
                self.image_path = Some(path.to_string());
                self.status.clear();
            }
            None => self.status = s::IconConverterViewModel_Image_load_failed().into(),
        }
    }

    fn can_convert(&self) -> bool {
        self.source.is_some() && self.sizes.iter().any(|s| *s)
    }

    /// `ConvertAsync` and `SaveIconAsync`.
    fn convert(&mut self) {
        let Some(source) = &self.source else { return };
        self.status = s::IconConverterViewModel_Creating_icon().into();
        let sizes: Vec<i32> = sharex_tools::icon::SUPPORTED_SIZES.iter().zip(self.sizes).filter(|(_, on)| *on).map(|(s, _)| *s).collect();
        let bytes = match sharex_tools::icon::convert(source, &sizes, self.depth) {
            Ok(b) => b,
            Err(e) => {
                self.status = s::IconConverterViewModel_Failed_create_icon().replace("{0}", &e);
                return;
            }
        };
        let suggested = match self.image_path.as_deref().filter(|p| !p.trim().is_empty()) {
            Some(p) => format!("{}.ico", sharex_tools::dotnet::file_name_without_extension(p)),
            None => "icon.ico".into(),
        };
        let path = self.save_to.clone().or_else(|| {
            rfd::FileDialog::new()
                .set_title(s::IconConverterWindow_Save_icon_as_dialog())
                .set_file_name(&suggested)
                .add_filter(s::IconConverterWindow_Icon_file(), &["ico"])
                .save_file()
                .map(|p| p.to_string_lossy().into_owned())
        });
        self.status = match path {
            Some(p) => match std::fs::write(&p, &bytes) {
                Ok(()) => s::IconConverterViewModel_Icon_saved().replace("{0}", &p),
                Err(e) => s::IconConverterViewModel_Failed_create_icon().replace("{0}", &e.to_string()),
            },
            None => String::new(),
        };
    }
}

impl Dialog for IconConverterWindow {
    fn spec(&self) -> Spec {
        Spec::sized(crate::rs_title(s::IconConverterWindow_ShareX_Icon_Converter()), WIDTH, HEIGHT, MIN_WIDTH, MIN_HEIGHT)
    }

    fn ui(&mut self, ui: &mut Ui, _cx: &mut Cx) -> Vec<Effect> {
        let p = palette(ui.ctx());
        let c = super::background(ui);
        // The image row.
        let top = Rect::from_min_size(c.min, vec2(c.width(), 2.0 + 24.0 + super::BUTTON_HEIGHT));
        super::panel(ui, top);
        let t = top.shrink(13.0);
        let hf = super::heading_font();
        let title = s::IconConverterWindow_Image();
        let tw = text_width(ui, title, hf.clone());
        super::text_at(ui, pos2(t.min.x, center_y(t, line_height(ui, &hf))), title, hf.clone(), p.text);
        let browse = s::IconConverterWindow_Browse();
        let bw = super::text_button_width(ui, browse);
        let br = Rect::from_min_size(pos2(t.max.x - bw, t.min.y), vec2(bw, super::BUTTON_HEIGHT));
        let path = self.image_path.clone().unwrap_or_else(|| "No image selected".into());
        super::path_text(ui, Rect::from_min_max(pos2(t.min.x + tw + 12.0, t.min.y), pos2(br.min.x - 12.0, t.max.y)), &path);
        if super::text_button(ui, br, Id::new("icon-browse"), browse, true).clicked()
            && let Some(f) = super::pick_image(s::IconConverterViewModel_Select_image_dialog())
        {
            self.load_image(&f);
        }
        // The bottom row: the status and Convert...
        let convert = s::IconConverterWindow_Convert();
        let cw = super::icon_text_button_width(ui, convert);
        let bottom = Rect::from_min_max(pos2(c.min.x, c.max.y - super::BUTTON_HEIGHT), c.max);
        let cr = Rect::from_min_size(pos2(bottom.max.x - cw, bottom.min.y), vec2(cw, super::BUTTON_HEIGHT));
        if !self.status.is_empty() {
            let nf = super::normal_font();
            let lines = sharex_ui::controls::wrap_lines(ui, &self.status, &nf, cr.min.x - 12.0 - bottom.min.x);
            let lh = line_height(ui, &nf);
            let y = center_y(bottom, lh * lines.len() as f32);
            for (k, l) in lines.iter().enumerate() {
                super::text_at(ui, pos2(bottom.min.x, y + lh * k as f32), l, nf.clone(), p.text_secondary);
            }
        }
        if super::icon_text_button(ui, cr, Id::new("icon-convert"), icon::FileDownload, convert, self.can_convert()).clicked() {
            self.convert();
        }
        let mid = Rect::from_min_max(pos2(c.min.x, top.max.y + 12.0), pos2(c.max.x, bottom.min.y - 12.0));
        let left = Rect::from_min_size(mid.min, vec2(340.0, mid.height()));
        let right = Rect::from_min_max(pos2(left.max.x + 12.0, mid.min.y), mid.max);
        // Sizes and bit depth (`StackPanel Spacing="20"`, headings with `Margin="0,0,0,8"`).
        super::panel(ui, left);
        let l = left.shrink(17.0);
        let hh = line_height(ui, &hf);
        super::text_at(ui, l.min, s::IconConverterWindow_Sizes(), hf.clone(), p.text);
        let nf = font(13.0, 400);
        let labels = [
            s::IconConverterWindow_16_pixels(),
            s::IconConverterWindow_32_pixels(),
            s::IconConverterWindow_48_pixels(),
            s::IconConverterWindow_64_pixels(),
            s::IconConverterWindow_128_pixels(),
            s::IconConverterWindow_256_pixels(),
        ];
        let col_w = ((l.width() - 12.0) / 2.0).floor();
        let grid_top = l.min.y + hh + 8.0;
        for (k, label) in labels.into_iter().enumerate() {
            let (r, cidx) = (k / 2, k % 2);
            let pos = pos2(l.min.x + cidx as f32 * (col_w + 12.0), grid_top + r as f32 * 42.0);
            let cb = CheckBox { text: label, text_font: nf.clone(), text_color: Some(p.text_secondary), min_height: 32.0, enabled: true };
            cb.show(ui, Rect::from_min_size(pos, vec2(cb.width(ui), 32.0)), Id::new(("icon-size", k)), &mut self.sizes[k]);
        }
        let depth_top = grid_top + 3.0 * 32.0 + 2.0 * 10.0 + 20.0;
        super::text_at(ui, pos2(l.min.x, depth_top), s::IconConverterWindow_Bit_depth(), hf, p.text);
        let mut y = depth_top + hh + 8.0;
        for (k, (label, depth)) in [(s::IconConverterWindow_8_bits_256_colors(), IconBitDepth::Palette8), (s::IconConverterWindow_32_bits_16_7M_colors(), IconBitDepth::TrueColor32)].into_iter().enumerate() {
            let rb = RadioButton { text: label, text_font: nf.clone(), text_color: None, padding_left: 8.0, enabled: true };
            let w = 20.0 + 8.0 + text_width(ui, label, nf.clone());
            if rb.show(ui, Rect::from_min_size(pos2(l.min.x, y), vec2(w, 32.0)), Id::new(("icon-depth", k)), self.depth == depth).clicked() {
                self.depth = depth;
            }
            y += 32.0 + 8.0;
        }
        // The preview.
        super::panel(ui, right);
        let r = right.shrink(17.0);
        match &mut self.preview {
            Some(pic) => {
                let lh = line_height(ui, &nf);
                let img_area = Rect::from_min_max(r.min, pos2(r.max.x, r.max.y - lh - 12.0)).shrink(8.0);
                let dest = crate::dialogs::layout::uniform(img_area, pic.size());
                let tex = pic.texture(ui.ctx(), (dest.width().round() as u32, dest.height().round() as u32));
                ui.painter().image(tex, dest, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                let dwid = text_width(ui, &self.image_details, nf.clone());
                super::text_at(ui, pos2(r.min.x + ((r.width() - dwid) / 2.0).round_ties_even(), r.max.y - lh), &self.image_details, nf, p.text_secondary);
            }
            None => {
                super::empty_state(
                    ui,
                    r,
                    icon::FileImage,
                    46.0,
                    10.0,
                    &[(s::IconConverterWindow_Drop_an_image_here(), font(16.0, 600), p.text), (s::IconConverterWindow_or_use_Browse_to_choose_a_file(), nf, p.text_secondary)],
                );
            }
        }
        Vec::new()
    }

    fn dropped(&mut self, files: Vec<std::path::PathBuf>, _cx: &mut Cx) -> Vec<Effect> {
        if let Some(f) = files.first() {
            self.load_image(&f.to_string_lossy());
        }
        Vec::new()
    }
}
