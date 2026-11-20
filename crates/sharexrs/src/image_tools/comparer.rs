//! `ShareX.Tools/Tools/ImageComparer` (`ImageComparerWindow.axaml`,
//! `ImageComparerViewModel`, `ImageComparisonService`): two image pickers,
//! then the comparison: the Slider mode (`ImageComparisonSlider` over both
//! previews, at most 2048 px) or the Diff view (`ComparisonResult.diff`), and
//! the similarity, green when the images are identical.

use egui::{Color32, Id, Rect, Ui, pos2, vec2};
use image::RgbaImage;
use sharex_strings::tools as s;
use sharex_tools::comparer::ComparisonResult;
use sharex_ui::controls::{RadioButton, content_font};
use sharex_ui::theme::{fluent, palette};
use sharex_ui::widgets::{center_y, font, line_height, text_width};

use super::Picture;
use super::slider::ComparisonSlider;
use crate::dialogs::{Cx, Dialog, Effect, Spec};

pub const WIDTH: f32 = 1180.0;
pub const HEIGHT: f32 = 760.0;
pub const MIN_WIDTH: f32 = 900.0;
pub const MIN_HEIGHT: f32 = 600.0;
/// `MaxPreviewDimension`.
const MAX_PREVIEW: u32 = 2048;
/// The identical colour, `Color.FromRgb(16, 185, 129)`.
const IDENTICAL: Color32 = Color32::from_rgb(16, 185, 129);

struct Side {
    path: String,
    bitmap: sharex_tools::imaging::Bitmap,
    preview: Picture,
}

pub struct ImageComparerWindow {
    sides: [Option<Side>; 2],
    result: Option<ComparisonResult>,
    diff: Option<Picture>,
    /// `SelectedMode`: Diff view.
    pub diff_mode: bool,
    slider: ComparisonSlider,
    pub status: String,
}

impl Default for ImageComparerWindow {
    fn default() -> Self {
        Self { sides: [None, None], result: None, diff: None, diff_mode: false, slider: ComparisonSlider::default(), status: s::ImageComparerViewModel_Select_two_images().into() }
    }
}

/// `BitmapConversionHelpers.ToAvaloniBitmap` of an engine bitmap.
fn to_rgba(bmp: &sharex_tools::imaging::Bitmap) -> RgbaImage {
    RgbaImage::from_raw(bmp.width(), bmp.height(), bmp.to_straight()).unwrap_or_default()
}

/// `CreatePreviewBitmap`: the image, or it scaled to 2048 on its longer side.
fn preview(bmp: &sharex_tools::imaging::Bitmap) -> Picture {
    let largest = bmp.width().max(bmp.height());
    let img = to_rgba(bmp);
    if largest <= MAX_PREVIEW {
        return Picture::new(img);
    }
    let scale = MAX_PREVIEW as f64 / largest as f64;
    let w = ((bmp.width() as f64 * scale).round() as u32).max(1);
    let h = ((bmp.height() as f64 * scale).round() as u32).max(1);
    Picture::new(crate::thumbnail::resize(&img, w, h))
}

impl ImageComparerWindow {
    /// `SelectImageAsync` with the chosen file.
    pub fn set_image(&mut self, k: usize, path: &str) {
        match sharex_tools::comparer::load_image(path) {
            Some(bmp) => {
                let p = preview(&bmp);
                self.sides[k] = Some(Side { path: path.to_string(), bitmap: bmp, preview: p });
                self.update_comparison();
            }
            None => self.status = s::ImageComparerViewModel_Image_load_failed().into(),
        }
    }

    /// `UpdateComparison`.
    fn update_comparison(&mut self) {
        self.result = None;
        self.diff = None;
        let (Some(a), Some(b)) = (&self.sides[0], &self.sides[1]) else {
            self.status = s::ImageComparerViewModel_Select_two_images().into();
            return;
        };
        let r = sharex_tools::comparer::compare(&a.bitmap, &b.bitmap, MAX_PREVIEW as i32);
        self.diff = Some(Picture::new(to_rgba(&r.diff)));
        self.status = sharex_tools::comparer::texts(&r).1;
        self.result = Some(r);
    }

    fn similarity(&self) -> (String, Color32) {
        match &self.result {
            Some(r) => (sharex_tools::comparer::texts(r).0, if r.is_identical() { IDENTICAL } else { Color32::WHITE }),
            None => (s::ImageComparerViewModel_Similarity_empty().into(), Color32::WHITE),
        }
    }

    fn picker(&mut self, ui: &Ui, rect: Rect, k: usize) {
        let p = palette(ui.ctx());
        super::panel(ui, rect);
        let c = rect.shrink(13.0);
        let title = if k == 0 { s::ImageComparerWindow_Image_1() } else { s::ImageComparerWindow_Image_2() };
        let tf = font(16.0, 600);
        let tw = text_width(ui, title, tf.clone());
        super::text_at(ui, pos2(c.min.x, center_y(c, line_height(ui, &tf))), title, tf, p.text);
        let choose = s::ImageComparerWindow_Choose();
        let bw = super::text_button_width(ui, choose);
        let br = Rect::from_min_size(pos2(c.max.x - bw, c.min.y), vec2(bw, super::BUTTON_HEIGHT));
        // `TargetNullValue='No image selected'`.
        let path = self.sides[k].as_ref().map(|s| s.path.clone()).unwrap_or_else(|| "No image selected".into());
        super::path_text(ui, Rect::from_min_max(pos2(c.min.x + tw + 12.0, c.min.y), pos2(br.min.x - 12.0, c.max.y)), &path);
        if super::text_button(ui, br, Id::new(("comparer-choose", k)), choose, true).clicked() {
            let title = s::ImageComparerViewModel_Select_image_dialog().replace("{0}", &(k + 1).to_string());
            if let Some(f) = super::pick_image(&title) {
                self.set_image(k, &f);
            }
        }
    }
}

impl Dialog for ImageComparerWindow {
    fn spec(&self) -> Spec {
        Spec::sized(crate::rs_title(s::ImageComparerWindow_ShareX_Image_Comparer()), WIDTH, HEIGHT, MIN_WIDTH, MIN_HEIGHT)
    }

    fn ui(&mut self, ui: &mut Ui, _cx: &mut Cx) -> Vec<Effect> {
        let p = palette(ui.ctx());
        let f = fluent(ui.ctx());
        let c = super::background(ui);
        let top_h = 2.0 + 24.0 + super::BUTTON_HEIGHT;
        let half = ((c.width() - 12.0) / 2.0).floor();
        self.picker(ui, Rect::from_min_size(c.min, vec2(half, top_h)), 0);
        self.picker(ui, Rect::from_min_max(pos2(c.min.x + half + 12.0, c.min.y), pos2(c.max.x, c.min.y + top_h)), 1);
        let body = Rect::from_min_max(pos2(c.min.x, c.min.y + top_h + 12.0), c.max);
        super::panel(ui, body);
        let inner = body.shrink(1.0);
        // The toolbar (`Padding="16,12"`, a 1px bottom border).
        let header_h = 12.0 + 32.0 + 12.0 + 1.0;
        let header = Rect::from_min_size(inner.min, vec2(inner.width(), header_h));
        let radius = egui::CornerRadius { nw: 3, ne: 3, ..Default::default() };
        ui.painter().rect_filled(header, radius, p.background_toolbar);
        ui.painter().rect_filled(Rect::from_min_size(pos2(header.min.x, header.max.y - 1.0), vec2(header.width(), 1.0)), 0.0, p.border);
        let row = Rect::from_min_max(pos2(header.min.x + 16.0, header.min.y + 12.0), pos2(header.max.x - 16.0, header.max.y - 1.0 - 12.0));
        let rf = content_font(14.0, 400);
        let mut x = row.min.x;
        for (k, text) in [s::ImageComparerWindow_Slider(), s::ImageComparerWindow_Diff_View()].into_iter().enumerate() {
            let checked = self.diff_mode == (k == 1);
            let w = 20.0 + 8.0 + text_width(ui, text, rf.clone());
            let rb = RadioButton { text, text_font: rf.clone(), text_color: Some(f.base_high), padding_left: 8.0, enabled: true };
            if rb.show(ui, Rect::from_min_size(pos2(x, row.min.y), vec2(w, 32.0)), Id::new(("comparer-mode", k)), checked).clicked() {
                self.diff_mode = k == 1;
            }
            x += w + 16.0;
        }
        let (sim, color) = self.similarity();
        let sf = font(16.0, 600);
        let sw = text_width(ui, &sim, sf.clone());
        super::text_at(ui, pos2(row.max.x - sw, center_y(row, line_height(ui, &sf))), &sim, sf, color);
        let area = Rect::from_min_max(pos2(inner.min.x, header.max.y), inner.max);
        ui.painter().rect_filled(area, egui::CornerRadius { sw: 3, se: 3, ..Default::default() }, p.background_panel);
        match (&mut self.sides, self.result.is_some()) {
            ([Some(a), Some(b)], true) if !self.diff_mode => {
                self.slider.show(ui, area.shrink(12.0), Id::new("comparer-slider"), &mut a.preview, &mut b.preview, true);
            }
            (_, true) => {
                if let Some(d) = &mut self.diff {
                    // `Image Stretch="Uniform"`.
                    let bounds = super::slider::image_bounds(area.shrink(12.0), d.size(), super::slider::Stretch::Both);
                    d.texture(ui.ctx(), (bounds.width() as u32, bounds.height() as u32)).paint(ui.painter(), bounds, Color32::WHITE);
                }
            }
            _ => {
                let hint = s::ImageComparerWindow_Select_Image_1_and_Image_2_to_show_the_comparison_preview();
                let hf = font(16.0, 400);
                let hw = text_width(ui, hint, hf.clone());
                let lh = line_height(ui, &hf);
                let pos = pos2(area.min.x + ((area.width() - hw) / 2.0).round_ties_even(), area.min.y + ((area.height() - lh) / 2.0).round_ties_even());
                super::text_at(ui, pos, hint, hf, p.text_secondary);
            }
        }
        Vec::new()
    }
}
