//! `ShareX/Presentation/BeforeUpload/BeforeUploadWindow.axaml(.cs)`: before
//! a task uploads, its destination can be changed; Upload goes on, Cancel
//! (or closing) uploads nothing.

use std::sync::Arc;
use std::sync::mpsc::Sender;

use egui::{Rect, Vec2, pos2, vec2};
use image::RgbaImage;
use sharex_settings::{FileDestination, ImageDestination, TaskSettings, TextDestination, UrlShortenerType};
use sharex_strings::sharex as s;
use sharex_ui::controls::{RadioButton, wrap_lines};
use sharex_ui::icons;
use sharex_ui::keys::app_icons as icon;
use sharex_ui::theme::palette;
use sharex_ui::widgets::{center_y, font, line_height};
use sharex_uploaders::{UploadersConfig, factory, validator};

use super::clipboard_upload::{paint_image, texture};
use super::layout::{self, Compact};
use super::{Cx, Dialog, Effect, Spec};
use crate::tasks::EDataType;
use crate::upload::BeforeUpload;

/// What choosing an option sets (`CreateOption`'s `select`).
#[derive(Clone, Debug, PartialEq)]
enum Choice {
    Image(ImageDestination),
    ImageFile(FileDestination),
    Text(TextDestination),
    TextFile(FileDestination),
    File(FileDestination),
    Url(UrlShortenerType),
}

/// `BeforeUploadDestinationOption`.
struct Option_ {
    label: String,
    choice: Choice,
}

pub struct BeforeUploadWindow {
    request: BeforeUpload,
    options: Vec<Option_>,
    selected: Option<usize>,
    reply: Option<Sender<Option<TaskSettings>>>,
    texture: Option<sharex_ui::tiled::TiledTexture>,
}

/// `GetDestinationLabel`: a custom uploader's name in "Custom [{0}]", else
/// the destination's localized description.
fn label(enum_name: &str, value: &str, custom_selected: Option<i32>, task: &TaskSettings, config: &UploadersConfig) -> String {
    if let Some(mut index) = custom_selected.filter(|&i| i >= 0) {
        let list = &config.CustomUploadersList.0;
        if task.OverrideCustomUploader {
            let max = list.len() as i32 - 1;
            index = if (0..=max).contains(&task.CustomUploaderIndex) { task.CustomUploaderIndex } else { 0 };
        }
        if let Some(item) = usize::try_from(index).ok().and_then(|i| list.get(i)) {
            return s::BeforeUploadWindow_CustomUploader().replace("{0}", &item.display_name());
        }
    }
    factory::service_name(enum_name, value)
}

impl BeforeUploadWindow {
    /// `new BeforeUploadWindow(info)`: the destinations whose settings are
    /// complete (`UploadersConfigValidator`) for the task's data type.
    pub fn new(request: BeforeUpload, config: &UploadersConfig, reply: Sender<Option<TaskSettings>>) -> Self {
        let t = &request.task;
        let c = config;
        let mut list: Vec<(Option_, bool)> = Vec::new();
        let mut push = |label: String, choice: Choice, selected: bool| list.push((Option_ { label, choice }, selected));
        // The file destinations, as `wrap` chooses them.
        let file_options = |push: &mut dyn FnMut(String, Choice, bool), wrap: fn(FileDestination) -> Choice, selected: &dyn Fn(&FileDestination) -> bool| {
            for d in FileDestination::ALL.iter().filter(|d| validator::file(d, c)) {
                let custom = (*d == FileDestination::CustomFileUploader).then_some(c.CustomFileUploaderSelected);
                push(label("FileDestination", d.name(), custom, t, c), wrap(d.clone()), selected(d));
            }
        };
        match request.data_type {
            EDataType::Image => {
                for d in ImageDestination::ALL.iter().filter(|d| **d != ImageDestination::FileUploader && validator::image(d, c)) {
                    let custom = (*d == ImageDestination::CustomImageUploader).then_some(c.CustomImageUploaderSelected);
                    push(label("ImageDestination", d.name(), custom, t, c), Choice::Image(d.clone()), t.ImageDestination == *d);
                }
                let image_file = t.ImageDestination == ImageDestination::FileUploader;
                file_options(&mut push, Choice::ImageFile, &|d| image_file && t.ImageFileDestination == *d);
            }
            EDataType::Text => {
                for d in TextDestination::ALL.iter().filter(|d| **d != TextDestination::FileUploader && validator::text(d, c)) {
                    let custom = (*d == TextDestination::CustomTextUploader).then_some(c.CustomTextUploaderSelected);
                    push(label("TextDestination", d.name(), custom, t, c), Choice::Text(d.clone()), t.TextDestination == *d);
                }
                let text_file = t.TextDestination == TextDestination::FileUploader;
                file_options(&mut push, Choice::TextFile, &|d| text_file && t.TextFileDestination == *d);
            }
            EDataType::File => file_options(&mut push, Choice::File, &|d| t.FileDestination == *d),
            EDataType::URL => {
                for d in UrlShortenerType::ALL.iter().filter(|d| validator::url_shortener(d, c)) {
                    let custom = (*d == UrlShortenerType::CustomURLShortener).then_some(c.CustomURLShortenerSelected);
                    push(label("UrlShortenerType", d.name(), custom, t, c), Choice::Url(d.clone()), t.URLShortenerDestination == *d);
                }
            }
            EDataType::Default => {}
        }
        // The first option that's selected is the one checked (a radio
        // group holds one).
        let selected = list.iter().position(|(_, sel)| *sel);
        let options = list.into_iter().map(|(o, _)| o).collect();
        Self { request, options, selected, reply: Some(reply), texture: None }
    }

    /// `UpdatePrompt`.
    fn prompt(&self) -> String {
        match self.selected.and_then(|i| self.options.get(i)) {
            Some(o) => s::BeforeUploadWindow_UploadPrompt().replace("{0}", &self.request.file_name).replace("{1}", &o.label),
            None => s::BeforeUploadWindow_ChooseDestination().to_owned(),
        }
    }

    /// The option's `select`: the task settings follow.
    fn select(&mut self, i: usize) {
        self.selected = Some(i);
        let t = &mut self.request.task;
        match self.options[i].choice.clone() {
            Choice::Image(d) => t.ImageDestination = d,
            Choice::ImageFile(d) => {
                t.ImageDestination = ImageDestination::FileUploader;
                t.ImageFileDestination = d;
            }
            Choice::Text(d) => t.TextDestination = d,
            Choice::TextFile(d) => {
                t.TextDestination = TextDestination::FileUploader;
                t.TextFileDestination = d;
            }
            Choice::File(d) => {
                t.ImageDestination = ImageDestination::FileUploader;
                t.TextDestination = TextDestination::FileUploader;
                t.ImageFileDestination = d.clone();
                t.TextFileDestination = d.clone();
                t.FileDestination = d;
            }
            Choice::Url(d) => t.URLShortenerDestination = d,
        }
    }

    /// The preview: the task's image, else an image file it uploads.
    fn source_image(&mut self) -> Option<Arc<RgbaImage>> {
        if self.request.image.is_none()
            && let Some(path) = self.request.file_path.as_ref().filter(|p| p.is_file())
            && sharex_helpers::file_helpers::is_image_file(&path.to_string_lossy())
        {
            self.request.image = image::open(path).ok().map(|i| Arc::new(i.to_rgba8()));
        }
        self.request.image.clone()
    }

    fn answer(&mut self, accepted: bool) {
        if let Some(r) = self.reply.take() {
            let _ = r.send(accepted.then(|| self.request.task.clone()));
        }
    }
}

/// `ImageSizeBadge`: "W × H" in white 12px on `#CC202020`, `Padding="7,3"`,
/// 4px corners, 8px in from the bottom right.
pub fn size_badge(ui: &egui::Ui, area: Rect, size: (u32, u32)) {
    let text = format!("{} × {}", size.0, size.1);
    let f = font(12.0, 600);
    let w = layout::width(ui, &text, f.clone()) + 14.0;
    let h = line_height(ui, &f) + 6.0;
    let badge = Rect::from_min_size(pos2(area.max.x - 8.0 - w, area.max.y - 8.0 - h), vec2(w, h));
    let bg = egui::Color32::from_rgba_unmultiplied(0x20, 0x20, 0x20, 0xCC);
    sharex_ui::border::border(ui.painter(), badge, 4.0, 0.0, Some(sharex_ui::border::Brush::Solid(bg)), None);
    layout::text(ui, pos2(badge.min.x + 7.0, badge.min.y + 3.0), &text, f, egui::Color32::WHITE, f32::INFINITY);
}

/// The section header in a panel: the toolbar background, 6px corners,
/// `Padding="10,7"`, a 16px accent icon and the title 7px after it.
pub fn section_header(ui: &egui::Ui, rect_top: egui::Pos2, width: f32, icon_key: &str, title: &str) -> f32 {
    let p = palette(ui.ctx());
    let f = layout::text_font();
    let h = 7.0 + line_height(ui, &f).max(16.0) + 7.0;
    let band = Rect::from_min_size(rect_top, vec2(width, h));
    sharex_ui::border::border(ui.painter(), band, 6.0, 0.0, Some(sharex_ui::border::Brush::Solid(p.background_toolbar)), None);
    let row = band.shrink2(vec2(10.0, 7.0));
    icons::paint(ui.painter(), Rect::from_min_size(pos2(row.min.x, center_y(row, 16.0)), Vec2::splat(16.0)), icon_key, p.accent_icon);
    let y = center_y(row, line_height(ui, &f));
    layout::text(ui, pos2(row.min.x + 16.0 + 7.0, y), title, f, p.text_secondary, row.width());
    h
}

impl Dialog for BeforeUploadWindow {
    fn spec(&self) -> Spec {
        Spec { topmost: true, ..Spec::sized(crate::rs_title(s::BeforeUploadWindow_Title()), 900.0, 560.0, 720.0, 460.0) }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _cx: &mut Cx) -> Vec<Effect> {
        let p = palette(ui.ctx());
        let full = layout::background(ui);
        let prompt = self.prompt();
        let header = layout::header(ui, full, icon::Upload, s::BeforeUploadWindow_Heading(), Some(&prompt), true);
        let grid = Rect::from_min_max(pos2(full.min.x + 18.0, full.min.y + header + 14.0), pos2(full.max.x - 18.0, full.max.y - 18.0));
        let buttons_top = grid.max.y - Compact::HEIGHT;
        let row = Rect::from_min_max(grid.min, pos2(grid.max.x, buttons_top - 10.0));
        // The destination list: `ColumnDefinitions="360,*"`.
        let left = Rect::from_min_max(row.min, pos2(row.min.x + 360.0, row.max.y));
        layout::panel(ui, left, 6.0);
        let inner = left.shrink(9.0);
        let band = section_header(ui, inner.min, inner.width(), icon::Upload, s::BeforeUploadWindow_Destination());
        // The `ListBox` (`Padding="4,2,4,8"`), rows of 32px radio buttons.
        let view = Rect::from_min_max(pos2(inner.min.x, inner.min.y + band + 6.0), inner.max);
        const ROW: f32 = 32.0;
        let content = 2.0 + ROW * self.options.len() as f32 + 8.0;
        let offset = layout::scroll_offset(ui, ui.id().with("list-scroll"), view, content);
        let clip = ui.clip_rect();
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(view));
        child.set_clip_rect(view.intersect(clip));
        let mut chosen = None;
        for (i, o) in self.options.iter().enumerate() {
            let r = Rect::from_min_size(pos2(view.min.x + 4.0 + 2.0, view.min.y + 2.0 + ROW * i as f32 - offset), vec2(view.width() - 12.0, ROW));
            if !r.intersects(view) {
                continue;
            }
            // String content: Inter at Fluent's 14px, `RadioButtonForeground`
            // (base high).
            let rb = RadioButton {
                text: &o.label,
                text_font: sharex_ui::controls::content_font(14.0, 400),
                text_color: Some(sharex_ui::theme::fluent(ui.ctx()).base_high),
                padding_left: 7.0,
                enabled: true,
            };
            if rb.show(&child, r, child.id().with(("dest", i)), self.selected == Some(i)).clicked() {
                chosen = Some(i);
            }
        }
        layout::scroll_bar(ui, view, content, offset, 16.0);
        if let Some(i) = chosen {
            self.select(i);
        }
        // The preview: `Grid.Column="1"`, 14px after the list.
        let right = Rect::from_min_max(pos2(left.max.x + 14.0, row.min.y), row.max);
        layout::panel(ui, right, 6.0);
        let mut preview_clicked = false;
        match self.source_image() {
            Some(image) => {
                let tex = texture(ui.ctx(), &mut self.texture, "before-upload-preview", &image);
                let area = right.shrink(1.0);
                paint_image(ui, area.shrink(10.0), tex, image.dimensions());
                size_badge(ui, area, image.dimensions());
                let r = ui.interact(right, ui.id().with("preview"), egui::Sense::click());
                preview_clicked = r.clicked();
                r.on_hover_cursor(egui::CursorIcon::PointingHand);
            }
            None => {
                // `EmptyPreview`: a 48px file icon at 45%, the file name
                // under it (`MaxWidth="320"`, wrapped), 8px apart.
                let text = if self.request.file_name.is_empty() { s::BeforeUploadWindow_NoPreview() } else { &self.request.file_name };
                let f = layout::text_font();
                let lines = wrap_lines(ui, text, &f, 320.0);
                let lh = line_height(ui, &f);
                let stack = 48.0 + 8.0 + lh * lines.len() as f32;
                let mut y = center_y(right, stack);
                let x = right.center().x - 24.0;
                icons::paint(ui.painter(), Rect::from_min_size(pos2(x.round(), y), Vec2::splat(48.0)), icon::FileText, p.text.gamma_multiply(0.45));
                y += 48.0 + 8.0;
                for line in &lines {
                    sharex_ui::controls::text_centered_at(ui, right.center().x, y, line, f.clone(), p.text_secondary);
                    y += lh;
                }
            }
        }
        let compact = Compact { min_width: 96.0 };
        let labels = [s::BeforeUploadWindow_Cancel(), s::BeforeUploadWindow_Upload()];
        let rects = compact.row(ui, grid.max.x, buttons_top, 7.0, &labels);
        let cancel = compact.show(ui, rects[0], ui.id().with("cancel"), labels[0], false, true).clicked();
        let upload = compact.show(ui, rects[1], ui.id().with("upload"), labels[1], true, true).clicked();
        let (enter, escape) = ui.input(|i| (i.key_pressed(egui::Key::Enter), i.key_pressed(egui::Key::Escape)));
        let escape = escape && !sharex_ui::popup::was_open(ui.ctx());
        if preview_clicked && let Some(image) = self.source_image() {
            // `ImageViewerWindowIntegration.ShowImage(_sourceImage)`.
            return vec![Effect::ShowImage((*image).clone())];
        }
        if upload || enter {
            self.answer(true);
            return vec![Effect::Close];
        }
        if cancel || escape {
            self.answer(false);
            return vec![Effect::Close];
        }
        Vec::new()
    }

    fn closed(&mut self, _cx: &mut Cx) -> Vec<Effect> {
        self.answer(false);
        Vec::new()
    }
}
