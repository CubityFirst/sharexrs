//! `ShareX/Presentation/ClipboardUpload/ClipboardUploadWindow.axaml(.cs)`:
//! the clipboard content viewer. What the clipboard holds (an image, text
//! or files) is shown before it's uploaded.

use sharex_ui::tiled::TiledTexture;
use egui::{Rect, pos2, vec2};
use image::RgbaImage;
use sharex_settings::TaskSettings;
use sharex_strings::sharex as s;
use sharex_ui::controls::{CheckBox, TextBox, content_font, wrap_lines};
use sharex_ui::keys::app_icons as icon;
use sharex_ui::theme::{Thickness, palette};
use sharex_ui::widgets::{center_y, line_height};

use super::layout::{self, Compact};
use super::{Cx, Dialog, Effect, Spec, UploadRequest};
use crate::upload::manager::ClipboardContent;

/// `ClipboardUploadWindowIntegration`'s answer: `ShowClipboardContentViewer`
/// follows "Don't show this window again" (from the main window only).
pub struct ClipboardUploadWindow {
    content: Option<ClipboardContent>,
    task: TaskSettings,
    show_dont_show_again: bool,
    dont_show_again: bool,
    texture: Option<TiledTexture>,
}

impl ClipboardUploadWindow {
    /// `new ClipboardUploadWindow(taskSettings, showDontShowAgain)`, reading
    /// the clipboard (`LoadClipboardContent`).
    pub fn new(task: TaskSettings, show_dont_show_again: bool) -> Self {
        Self::with_content(read_clipboard(), task, show_dont_show_again)
    }

    pub fn with_content(content: Option<ClipboardContent>, task: TaskSettings, show_dont_show_again: bool) -> Self {
        Self { content, task, show_dont_show_again, dont_show_again: false, texture: None }
    }

    fn summary(&self) -> String {
        match &self.content {
            Some(ClipboardContent::Image(image)) => {
                s::ClipboardUploadWindow_ImageSummary().replace("{0}", &image.width().to_string()).replace("{1}", &image.height().to_string())
            }
            Some(ClipboardContent::Text(text)) => s::ClipboardUploadWindow_TextSummary().replace("{0}", &text.encode_utf16().count().to_string()),
            Some(ClipboardContent::Files(files)) => s::ClipboardUploadWindow_FileSummary().replace("{0}", &files.len().to_string()),
            None => s::ClipboardUploadWindow_Empty().to_owned(),
        }
    }
}

/// `LoadClipboardContent`: an image first, else text, else files; empty
/// ones count as nothing.
pub fn read_clipboard() -> Option<ClipboardContent> {
    use sharex_capture::clipboard as c;
    if c::contains_image() {
        return c::get_image().map(ClipboardContent::Image);
    }
    if c::contains_text() {
        return c::get_text().filter(|t| !t.is_empty()).map(ClipboardContent::Text);
    }
    if c::contains_file_drop_list() {
        return c::get_file_drop_list().filter(|f| !f.is_empty()).map(ClipboardContent::Files);
    }
    None
}

/// The image's texture (tiles past the GPU's largest side), made once.
pub fn texture<'a>(ctx: &egui::Context, slot: &'a mut Option<TiledTexture>, name: &str, image: &RgbaImage) -> &'a TiledTexture {
    slot.get_or_insert_with(|| {
        let size = [image.width() as usize, image.height() as usize];
        let color = egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw());
        TiledTexture::new(ctx, name, color, egui::TextureOptions::LINEAR)
    })
}

/// Draws `image` uniformly scaled into `rect` (an `Image` with
/// `Stretch="Uniform"`).
pub fn paint_image(ui: &egui::Ui, rect: Rect, tex: &TiledTexture, size: (u32, u32)) {
    let r = layout::uniform(rect, size);
    tex.paint(ui.painter(), r, egui::Color32::WHITE);
}

impl Dialog for ClipboardUploadWindow {
    fn spec(&self) -> Spec {
        Spec::sized(crate::rs_title(&format!("ShareX - {}", s::ClipboardUploadWindow_ClipboardUpload())), 620.0, 500.0, 480.0, 380.0)
    }

    fn single(&self) -> Option<&'static str> {
        Some("clipboard-upload")
    }

    fn ui(&mut self, ui: &mut egui::Ui, _cx: &mut Cx) -> Vec<Effect> {
        let p = palette(ui.ctx());
        let full = layout::background(ui);
        let summary = self.summary();
        let header = layout::header(ui, full, icon::Clipboard, s::ClipboardUploadWindow_ClipboardUpload(), Some(&summary), false);
        let bottom_top = full.max.y - 18.0 - Compact::HEIGHT;
        let card = Rect::from_min_max(pos2(full.min.x + 18.0, full.min.y + header + 14.0), pos2(full.max.x - 18.0, bottom_top - 14.0));
        layout::panel(ui, card, 6.0);
        let inner = card.shrink(1.0);
        let mut preview_clicked = false;
        match &self.content {
            Some(ClipboardContent::Image(image)) => {
                // `ImagePreviewContainer`: the checkerboard (`CornerRadius="5"`),
                // the image 10px in, a hand cursor; a click opens the viewer.
                layout::checkered(ui, inner, 5.0);
                let tex = texture(ui.ctx(), &mut self.texture, "clipboard-preview", image);
                paint_image(ui, inner.shrink(10.0), tex, image.dimensions());
                let r = ui.interact(inner, ui.id().with("preview"), egui::Sense::click());
                preview_clicked = r.clicked();
                r.on_hover_cursor(egui::CursorIcon::PointingHand);
            }
            Some(ClipboardContent::Text(text)) => {
                let mut t = text.clone();
                let tb = TextBox { padding: Thickness::all(12.0), multiline: true, plain: true, read_only: true, ..TextBox::default() };
                tb.show(ui, inner, ui.id().with("text"), &mut t);
            }
            Some(ClipboardContent::Files(files)) => {
                // `FilePreview`: a `ListBox` (`Padding="6"`), Fluent's
                // `ListBoxItem` `Padding="12,9,12,12"`.
                let f = layout::text_font();
                let lh = line_height(ui, &f);
                let clip = ui.painter().with_clip_rect(inner);
                let mut y = inner.min.y + 6.0;
                for file in files {
                    let x = inner.min.x + 6.0 + 12.0;
                    let g = sharex_ui::widgets::galley(ui, &file.to_string_lossy(), f.clone(), p.text_secondary, inner.max.x - 18.0 - x);
                    clip.galley(pos2(x, y + 9.0), g, p.text_secondary);
                    y += 9.0 + lh + 12.0;
                }
            }
            None => {
                // `EmptyPreview`: centred, `Margin="24"`, wrapped.
                let f = layout::text_font();
                let area = inner.shrink(24.0);
                let lines = wrap_lines(ui, &summary, &f, area.width());
                let lh = line_height(ui, &f);
                let mut y = center_y(area, lh * lines.len() as f32);
                for line in &lines {
                    sharex_ui::controls::text_centered_at(ui, area.center().x, y, line, f.clone(), p.text_secondary);
                    y += lh;
                }
            }
        }
        let row = Rect::from_min_max(pos2(full.min.x + 18.0, bottom_top), pos2(full.max.x - 18.0, full.max.y - 18.0));
        if self.show_dont_show_again {
            let cb = CheckBox { text_font: content_font(13.0, 400), ..CheckBox::new(s::ClipboardUploadWindow_DontShowAgain()) };
            let r = Rect::from_min_size(pos2(row.min.x, center_y(row, 32.0)), vec2(cb.width(ui), 32.0));
            cb.show(ui, r, ui.id().with("dont-show-again"), &mut self.dont_show_again);
        }
        let compact = Compact { min_width: 92.0 };
        let labels = [s::ClipboardUploadWindow_Cancel(), s::ClipboardUploadWindow_Upload()];
        let rects = compact.row(ui, row.max.x, row.min.y, 7.0, &labels);
        let enabled = self.content.is_some();
        let cancel = compact.show(ui, rects[0], ui.id().with("cancel"), labels[0], false, true).clicked();
        let upload = compact.show(ui, rects[1], ui.id().with("upload"), labels[1], true, enabled).clicked();
        let (enter, escape) = ui.input(|i| (i.key_pressed(egui::Key::Enter), i.key_pressed(egui::Key::Escape)));
        let escape = escape && !sharex_ui::popup::was_open(ui.ctx());
        if preview_clicked && let Some(ClipboardContent::Image(image)) = &self.content {
            // `ImageViewerWindowIntegration.ShowImage(image)`.
            return vec![Effect::ShowImage(image.clone())];
        }
        if cancel || escape {
            return vec![Effect::Close];
        }
        if (upload || enter) && enabled {
            let content = self.content.take().expect("enabled");
            return vec![Effect::Upload(Box::new(UploadRequest::clipboard(content, self.task.clone()))), Effect::Close];
        }
        Vec::new()
    }

    fn closed(&mut self, cx: &mut Cx) -> Vec<Effect> {
        if self.show_dont_show_again {
            cx.settings.ShowClipboardContentViewer = !self.dont_show_again;
        }
        Vec::new()
    }
}
