//! `ShareX.Tools/Tools/ImageConverter` (`ImageConverterWindow.axaml`,
//! `ImageConverterViewModel`, `ImageConverterService`): the image list (the
//! first file selected), the output format with each format's options (JPEG
//! quality and chroma subsampling, WebP lossless with quality or effort, PNG
//! bit depth, compression level and filter, GIF quality, BMP bit depth), the
//! background colour where the output can't keep alpha, the file name
//! template and folder, the preview of the selected file as the chosen
//! encoder writes it (100 ms after the last change), and Convert images.

use egui::{Color32, Id, Rect, Ui, pos2, vec2};
use sharex_strings::{helpers_lib as hs, tools as s};
use sharex_tools::converter::{ConvertOptions, OutputFormat};
use sharex_tools::imaging::{BmpBitDepth, GifQuality, JpegDownsample, PngBitDepth};
use sharex_ui::controls::{CheckBox, ComboBox, ComboItem, TextBox, wrap_lines};
use sharex_ui::keys::app_icons as icon;
use sharex_ui::theme::Thickness;
use sharex_ui::widgets::line_height;

use super::{FileList, Job, ListAction, Picture};
use crate::dialogs::{Cx, Dialog, Effect, Spec};

pub const WIDTH: f32 = 1180.0;
pub const HEIGHT: f32 = 650.0;
pub const MIN_WIDTH: f32 = 960.0;
pub const MIN_HEIGHT: f32 = 560.0;

const FORMATS: [OutputFormat; 5] = [OutputFormat::Png, OutputFormat::Jpeg, OutputFormat::Webp, OutputFormat::Gif, OutputFormat::Bmp];
const GIF_QUALITIES: [GifQuality; 5] = [GifQuality::Default, GifQuality::Bit8, GifQuality::Bit4, GifQuality::Grayscale, GifQuality::Adaptive];
const PNG_DEPTHS: [PngBitDepth; 3] = [PngBitDepth::Automatic, PngBitDepth::Bit32, PngBitDepth::Bit24];
const BMP_DEPTHS: [BmpBitDepth; 3] = [BmpBitDepth::Automatic, BmpBitDepth::Bit32, BmpBitDepth::Bit24];
const SUBSAMPLINGS: [JpegDownsample; 3] = [JpegDownsample::Downsample420, JpegDownsample::Downsample422, JpegDownsample::Downsample444];

type Preview = (Vec<u8>, u32, u32);

pub struct ImageConverterWindow {
    pub list: FileList,
    pub format: usize,
    pub quality: f32,
    pub webp_lossless: bool,
    pub webp_effort: f32,
    /// `PngCompressionLevel` and the filter only change the PNG's bytes.
    pub png_compression_level: usize,
    pub png_filter: usize,
    pub png_depth: usize,
    pub bmp_depth: usize,
    pub subsampling: usize,
    pub gif_quality: usize,
    pub background: Color32,
    pub output_folder_path: String,
    pub output_file_name: String,
    pub message: String,
    preview: Option<Picture>,
    preview_size: String,
    job: Job<Preview>,
    scroll_id: Id,
    busy: bool,
}

impl Default for ImageConverterWindow {
    fn default() -> Self {
        Self {
            list: FileList::semibold(),
            format: 0,
            quality: 90.0,
            webp_lossless: false,
            webp_effort: 75.0,
            png_compression_level: 1,
            png_filter: 0,
            png_depth: 0,
            bmp_depth: 2,
            subsampling: 0,
            gif_quality: 4,
            background: Color32::WHITE,
            output_folder_path: String::new(),
            output_file_name: sharex_tools::converter::DEFAULT_FILE_NAME.into(),
            message: String::new(),
            preview: None,
            preview_size: String::new(),
            job: Job::new(100),
            scroll_id: Id::new("converter-scroll"),
            busy: false,
        }
    }
}

impl ImageConverterWindow {
    pub fn set_sync(&mut self) {
        self.job.sync = true;
    }

    fn output_format(&self) -> OutputFormat {
        FORMATS[self.format.min(4)]
    }

    fn is(&self, f: OutputFormat) -> bool {
        self.output_format() == f
    }

    fn has_quality(&self) -> bool {
        self.is(OutputFormat::Jpeg) || self.is(OutputFormat::Webp) && !self.webp_lossless
    }

    fn has_effort(&self) -> bool {
        self.is(OutputFormat::Webp) && self.webp_lossless
    }

    fn has_background(&self) -> bool {
        self.is(OutputFormat::Jpeg) || self.is(OutputFormat::Png) && PNG_DEPTHS[self.png_depth] != PngBitDepth::Bit32 || self.is(OutputFormat::Bmp) && BMP_DEPTHS[self.bmp_depth] != BmpBitDepth::Bit32
    }

    /// The encoder settings (`GetEncoderQuality` etc.).
    pub fn options(&self) -> ConvertOptions {
        ConvertOptions {
            format: self.output_format(),
            quality: if self.has_effort() { self.webp_effort as i32 } else { self.quality as i32 },
            background: super::rgba(self.background),
            jpeg_subsampling: SUBSAMPLINGS[self.subsampling.min(2)],
            gif_quality: GIF_QUALITIES[self.gif_quality.min(4)],
            png_bit_depth: PNG_DEPTHS[self.png_depth.min(2)],
            bmp_bit_depth: BMP_DEPTHS[self.bmp_depth.min(2)],
            webp_lossless: self.webp_lossless,
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

    fn can_convert(&self) -> bool {
        !self.busy && !self.list.items.is_empty() && super::dir_exists(&self.output_folder_path) && !self.output_file_name.trim().is_empty()
    }

    fn convert(&mut self) -> Vec<Effect> {
        if !self.can_convert() {
            return Vec::new();
        }
        self.message.clear();
        match sharex_tools::converter::convert_images(&self.list.items, &self.options(), &self.output_folder_path, &self.output_file_name) {
            Ok(files) => super::open_folder_with_first(&files),
            Err(e) => {
                eprintln!("ImageConverterViewModel: Failed to convert images. {e}");
                self.message = e.to_string();
                Vec::new()
            }
        }
    }

    fn poll_preview(&mut self, ctx: &egui::Context) {
        let file = self.preview_file();
        let o = self.options();
        if let Some((bytes, w, h)) = self.job.poll(ctx, move || Box::new(move || sharex_tools::converter::create_preview(&file, &o))) {
            self.preview = if bytes.is_empty() { None } else { Picture::decode(&bytes) };
            self.preview_size = if w > 0 && h > 0 { format!("{w} × {h}") } else { String::new() };
        }
    }

    /// A `TextBlock.row-label` with `TextWrapping="Wrap"`: its height.
    fn wrapped_label_height(ui: &Ui, text: &str, width: f32) -> f32 {
        let f = super::normal_font();
        line_height(ui, &f) * wrap_lines(ui, text, &f, width).len().max(1) as f32
    }

    fn wrapped_label(ui: &Ui, rect: Rect, text: &str) {
        let f = super::normal_font();
        let lh = line_height(ui, &f);
        let lines = wrap_lines(ui, text, &f, rect.width());
        let top = sharex_ui::widgets::center_y(rect, lh * lines.len() as f32);
        let color = sharex_ui::theme::palette(ui.ctx()).text_secondary;
        for (k, l) in lines.iter().enumerate() {
            super::text_at(ui, pos2(rect.min.x, top + lh * k as f32), l, f.clone(), color);
        }
    }

    /// The heights of row 2 (the format's options) and row 3 (JPEG subsampling).
    fn format_rows(&self, ui: &Ui, width: f32) -> (f32, f32) {
        let nl = line_height(ui, &super::normal_font());
        let quality_row = super::SLIDER_HEIGHT.max(Self::wrapped_label_height(ui, s::ImageConverterWindow_Quality(), 110.0));
        let effort_row = super::SLIDER_HEIGHT.max(Self::wrapped_label_height(ui, s::ImageConverterWindow_WebPCompressionEffort(), 110.0));
        let first = super::stack_height(
            &[if self.is(OutputFormat::Webp) { 32.0 } else { 0.0 }, if self.has_quality() { quality_row } else { 0.0 }, if self.has_effort() { effort_row } else { 0.0 }],
            10.0,
        );
        let png = if self.is(OutputFormat::Png) {
            super::stack_height(&[nl, 40.0, Self::wrapped_label_height(ui, s::ImageConverterWindow_PNGCompressionLevel(), width), 40.0, nl, 40.0], 8.0)
        } else {
            0.0
        };
        let one = if self.is(OutputFormat::Bmp) || self.is(OutputFormat::Gif) { super::stack_height(&[nl, 40.0], 8.0) } else { 0.0 };
        let row3 = if self.is(OutputFormat::Jpeg) { super::stack_height(&[Self::wrapped_label_height(ui, s::ImageConverterWindow_JPEGSubsampling(), width), 40.0], 8.0) } else { 0.0 };
        (first.max(png).max(one), row3)
    }

    fn output_panel_height(&self, ui: &Ui, width: f32) -> f32 {
        let hh = super::heading_height(ui);
        let nl = line_height(ui, &super::normal_font());
        let (r2, r3) = self.format_rows(ui, width);
        2.0 + 32.0 + super::grid_height(&[hh, 40.0, r2, r3, if self.has_background() { 32.0 } else { 0.0 }, nl, 40.0], 10.0)
    }

    /// An auto-width combo box (`HorizontalAlignment="Left"`): the new index.
    fn combo(ui: &Ui, rect: Rect, id: &str, items: &[String], selected: usize) -> Option<usize> {
        let items: Vec<ComboItem> = items.iter().map(|t| ComboItem::text(t.clone())).collect();
        let combo = ComboBox { padding: Thickness::hv(8.0, 6.0), enabled: true };
        let cw = combo.auto_width(ui, &items, Some(selected)).min(rect.width());
        combo.show(ui, Rect::from_min_size(rect.min, vec2(cw, 40.0)), Id::new(id), &items, Some(selected))
    }

    fn options_column(&mut self, ui: &mut Ui, col: Rect) {
        let hh = super::heading_height(ui);
        let nl = line_height(ui, &super::normal_font());
        let inner_w = col.width() - 34.0;
        let h1 = self.output_panel_height(ui, inner_w);
        let h2 = 2.0 + 32.0 + 40.0;
        let message_h = if self.message.trim().is_empty() { 0.0 } else { super::message_panel(ui, col.min, col.width(), &self.message, false) };
        let content_h = super::stack_height(&[h1, h2, message_h], 12.0);
        let offset = crate::dialogs::layout::scroll_offset(ui, self.scroll_id, col, content_h);
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(col));
        child.set_clip_rect(col.intersect(ui.clip_rect()));
        let ui = &mut child;
        let before = self.options();
        let before_png = (self.png_compression_level, self.png_filter);
        let top = col.min.y - offset;
        let panel = Rect::from_min_size(pos2(col.min.x, top), vec2(col.width(), h1));
        super::panel(ui, panel);
        let p = panel.shrink(17.0);
        super::heading(ui, Rect::from_min_size(p.min, vec2(p.width(), hh)), s::ImageThumbnailerWindow_Output());
        let mut y = p.min.y + hh + 10.0;
        let control = super::label_row(ui, Rect::from_min_size(pos2(p.min.x, y), vec2(p.width(), 40.0)), 110.0, s::ImageConverterWindow_OutputFormat());
        let formats: Vec<String> = ["PNG", "JPEG", "WebP", "GIF", "BMP"].iter().map(|s| s.to_string()).collect();
        if let Some(i) = Self::combo(ui, control, "converter-format", &formats, self.format) {
            self.format = i;
        }
        y += 50.0;
        let (r2, r3) = self.format_rows(ui, p.width());
        // Row 2: WebP lossless, quality, effort; or the PNG/BMP/GIF panels.
        let mut sy = y;
        if self.is(OutputFormat::Webp) {
            let cb = CheckBox::new(s::ImageConverterWindow_WebPLossless());
            cb.show(ui, Rect::from_min_size(pos2(p.min.x, sy), vec2(cb.width(ui), 32.0)), Id::new("converter-lossless"), &mut self.webp_lossless);
            sy += 32.0 + 10.0;
        }
        let row_label_h = |text: &str| super::SLIDER_HEIGHT.max(Self::wrapped_label_height(ui, text, 110.0));
        if self.has_quality() {
            let h = row_label_h(s::ImageConverterWindow_Quality());
            let r = Rect::from_min_size(pos2(p.min.x, sy), vec2(p.width(), h));
            Self::wrapped_label(ui, Rect::from_min_size(r.min, vec2(110.0, h)), s::ImageConverterWindow_Quality());
            let control = Rect::from_min_max(pos2(r.min.x + 120.0, r.min.y), r.max);
            super::slider_row(ui, control, 0.0, "", Id::new("converter-quality"), &mut self.quality, (0.0, 100.0), super::semibold_font(), &|v| format!("{}%", super::format_0(v)), true);
            sy += h + 10.0;
        }
        if self.has_effort() {
            let h = row_label_h(s::ImageConverterWindow_WebPCompressionEffort());
            let r = Rect::from_min_size(pos2(p.min.x, sy), vec2(p.width(), h));
            Self::wrapped_label(ui, Rect::from_min_size(r.min, vec2(110.0, h)), s::ImageConverterWindow_WebPCompressionEffort());
            let control = Rect::from_min_max(pos2(r.min.x + 120.0, r.min.y), r.max);
            // `ToolTip.Tip` on the label and the slider (the value's column
            // is `MinWidth="40"` after `ColumnSpacing="10"`).
            let hint = s::ImageConverterWindow_WebPCompressionEffortHint();
            sharex_ui::tooltip::show(ui, Rect::from_min_size(r.min, vec2(110.0, h)), Id::new("converter-effort-label-tip"), hint);
            sharex_ui::tooltip::show(ui, Rect::from_min_max(control.min, pos2(control.max.x - 50.0, control.max.y)), Id::new("converter-effort-tip"), hint);
            super::slider_row(ui, control, 0.0, "", Id::new("converter-effort"), &mut self.webp_effort, (0.0, 100.0), super::semibold_font(), &super::format_0, true);
        }
        // A label over its combo box; `tip` is both's `ToolTip.Tip` ("" none).
        let stack = |ui: &mut Ui, label: &str, wrap: bool, key: &str, items: Vec<String>, sel: &mut usize, sy: &mut f32, tip: &str| {
            let h = if wrap { Self::wrapped_label_height(ui, label, p.width()) } else { nl };
            sharex_ui::tooltip::show(ui, Rect::from_min_size(pos2(p.min.x, *sy), vec2(p.width(), h)), Id::new((key, "label-tip")), tip);
            sharex_ui::tooltip::show(ui, Rect::from_min_size(pos2(p.min.x, *sy + h + 8.0), vec2(p.width(), 40.0)), Id::new((key, "tip")), tip);
            if wrap {
                Self::wrapped_label(ui, Rect::from_min_size(pos2(p.min.x, *sy), vec2(p.width(), h)), label);
            } else {
                super::row_label(ui, Rect::from_min_size(pos2(p.min.x, *sy), vec2(p.width(), h)), label);
            }
            *sy += h + 8.0;
            if let Some(i) = Self::combo(ui, Rect::from_min_size(pos2(p.min.x, *sy), vec2(p.width(), 40.0)), key, &items, *sel) {
                *sel = i;
            }
            *sy += 40.0 + 8.0;
        };
        let mut sy = y;
        if self.is(OutputFormat::Png) {
            let depths = vec![hs::PNGBitDepth_Automatic().to_string(), hs::PNGBitDepth_Bit32().to_string(), hs::PNGBitDepth_Bit24().to_string()];
            stack(ui, s::ImageConverterWindow_PNGBitDepth(), false, "converter-png-depth", depths, &mut self.png_depth, &mut sy, "");
            let levels: Vec<String> = (0..10).map(|l| l.to_string()).collect();
            stack(ui, s::ImageConverterWindow_PNGCompressionLevel(), true, "converter-png-level", levels, &mut self.png_compression_level, &mut sy, s::ImageConverterWindow_PNGCompressionHint());
            let filters = vec![
                s::ImageConverterWindow_PNGFilterAutomatic().to_string(),
                s::ImageConverterWindow_PNGFilterNone().to_string(),
                s::ImageConverterWindow_PNGFilterSub().to_string(),
                s::ImageConverterWindow_PNGFilterUp().to_string(),
                s::ImageConverterWindow_PNGFilterAverage().to_string(),
                s::ImageConverterWindow_PNGFilterPaeth().to_string(),
            ];
            stack(ui, s::ImageConverterWindow_PNGFilter(), false, "converter-png-filter", filters, &mut self.png_filter, &mut sy, "");
        }
        if self.is(OutputFormat::Bmp) {
            let depths = vec![hs::BMPBitDepth_Automatic().to_string(), hs::BMPBitDepth_Bit32().to_string(), hs::BMPBitDepth_Bit24().to_string()];
            stack(ui, s::ImageConverterWindow_BMPBitDepth(), false, "converter-bmp-depth", depths, &mut self.bmp_depth, &mut sy, "");
        }
        if self.is(OutputFormat::Gif) {
            let qualities = vec![
                hs::GIFQuality_Default().to_string(),
                hs::GIFQuality_Bit8().to_string(),
                hs::GIFQuality_Bit4().to_string(),
                hs::GIFQuality_Grayscale().to_string(),
                hs::GIFQuality_Adaptive().to_string(),
            ];
            stack(ui, s::ImageConverterWindow_Quality(), false, "converter-gif-quality", qualities, &mut self.gif_quality, &mut sy, "");
        }
        y += r2 + 10.0;
        if self.is(OutputFormat::Jpeg) {
            let mut sy = y;
            let items = vec![
                s::ImageConverterWindow_JPEGSubsampling420().to_string(),
                s::ImageConverterWindow_JPEGSubsampling422().to_string(),
                s::ImageConverterWindow_JPEGSubsampling444().to_string(),
            ];
            stack(ui, s::ImageConverterWindow_JPEGSubsampling(), true, "converter-subsampling", items, &mut self.subsampling, &mut sy, s::ImageConverterWindow_JPEGSubsamplingHint());
        }
        y += r3 + 10.0;
        if self.has_background() {
            let control = super::label_row(ui, Rect::from_min_size(pos2(p.min.x, y), vec2(p.width(), 32.0)), 110.0, s::ImageConverterWindow_BackgroundColor());
            super::color_picker(ui, control, Id::new("converter-background"), &mut self.background, true);
            y += 32.0;
        }
        y += 10.0;
        super::row_label(ui, Rect::from_min_size(pos2(p.min.x, y), vec2(p.width(), nl)), s::ImageConverterWindow_FileNameTemplate());
        y += nl + 10.0;
        let name_before = self.output_file_name.clone();
        TextBox::default().show(ui, Rect::from_min_size(pos2(p.min.x, y), vec2(p.width(), 40.0)), Id::new("converter-name"), &mut self.output_file_name);
        // The folder panel.
        let panel = Rect::from_min_size(pos2(col.min.x, panel.max.y + 12.0), vec2(col.width(), h2));
        super::panel(ui, panel);
        let p = panel.shrink(17.0);
        let browse = s::ImageThumbnailerWindow_Browse();
        let bw = super::text_button_width(ui, browse);
        let br = Rect::from_min_size(pos2(p.max.x - bw, p.min.y), vec2(bw, 40.0));
        super::path_text(ui, Rect::from_min_max(p.min, pos2(br.min.x - 8.0, p.max.y)), &self.output_folder_path);
        if super::text_button(ui, br, Id::new("converter-browse"), browse, true).clicked()
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
        if before != self.options() || before_png != (self.png_compression_level, self.png_filter) {
            self.options_changed();
        }
        crate::dialogs::layout::scroll_bar(ui, col, content_h, offset, 16.0);
    }
}

impl Dialog for ImageConverterWindow {
    fn spec(&self) -> Spec {
        Spec::sized(crate::rs_title(s::ImageConverterWindow_Image_converter()), WIDTH, HEIGHT, MIN_WIDTH, MIN_HEIGHT)
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
            "converter",
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
        let viewer = super::preview_pane(ui, right, &path, &size, self.preview.as_mut(), loading, !self.list.items.is_empty());
        let text = s::ImageConverterWindow_Convert_images();
        let bw = super::icon_text_button_width(ui, text);
        let br = Rect::from_min_size(pos2(bottom.max.x - bw, bottom.min.y), vec2(bw, super::BUTTON_HEIGHT));
        if super::icon_text_button(ui, br, Id::new("converter-convert"), icon::Refresh, text, self.can_convert()).clicked() {
            return self.convert();
        }
        viewer.into_iter().collect()
    }

    fn dropped(&mut self, files: Vec<std::path::PathBuf>, _cx: &mut Cx) -> Vec<Effect> {
        self.add_files(files.into_iter().map(|f| f.to_string_lossy().into_owned()).collect());
        Vec::new()
    }
}
