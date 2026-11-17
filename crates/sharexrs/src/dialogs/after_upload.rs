//! `ShareX/Presentation/AfterUpload/AfterUploadWindow.axaml(.cs)`: a
//! finished upload's links in every format (`LinkFormatEnum`, then the
//! custom clipboard formats for images), its image, and buttons to copy
//! and open them. Opens without taking focus; closes itself after 60 s with
//! `AutoCloseAfterUploadForm`.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::{Rect, Vec2, pos2, vec2};
use image::RgbaImage;
use sharex_strings::sharex as s;
use sharex_ui::border::{self, Brush};
use sharex_ui::controls::Button;
use sharex_ui::icons;
use sharex_ui::keys::app_icons as icon;
use sharex_ui::theme::{Thickness, palette};
use sharex_ui::widgets::{center_y, font, galley, line_height};
use sharex_uploaders::enums::LinkFormatEnum;

use super::before_upload::{section_header, size_badge};
use super::clipboard_upload::{paint_image, texture};
use super::layout::{self, Compact};
use super::{Cx, Dialog, Effect, Spec};
use crate::tasks::{EDataType, UploadResult};
use crate::upload::after_upload::{InfoFields, parse_upload_info};

/// What the window shows of a finished task (`TaskInfo`).
pub struct AfterUploadInfo {
    pub file_name: String,
    pub file_path: Option<PathBuf>,
    pub thumbnail_file_path: Option<PathBuf>,
    pub result: UploadResult,
    pub data_type: EDataType,
    pub image: Option<Arc<RgbaImage>>,
    pub upload_duration: Option<Duration>,
    pub auto_close: bool,
    /// `ApplicationState.Settings.ClipboardContentFormats`: (description, format).
    pub clipboard_formats: Vec<(String, String)>,
}

/// `AfterUploadFormatItem`.
#[derive(Clone, Debug, PartialEq)]
pub struct FormatItem {
    pub group: String,
    pub description: String,
    pub text: String,
}

/// `UploadInfoParser`'s link formats.
mod formats {
    pub const FORUM_IMAGE: &str = "[img]$url[/img]";
    pub const HTML_IMAGE: &str = "<img src=\"$url\">";
    pub const WIKI_IMAGE: &str = "[$url]";
    pub const FORUM_LINKED_IMAGE: &str = "[url=$url][img]$thumbnailurl[/img][/url]";
    pub const HTML_LINKED_IMAGE: &str = "<a href=\"$url\"><img src=\"$thumbnailurl\"></a>";
    pub const WIKI_LINKED_IMAGE: &str = "[$url $thumbnailurl]";
}

/// `new Uri(filePath).AbsoluteUri` of an existing file: `file:///` and the
/// path with forward slashes, escaped as `System.Uri` does.
pub fn file_uri(path: &Path) -> String {
    if !path.is_file() {
        return String::new();
    }
    let s = path.to_string_lossy().replace('\\', "/");
    // A UNC path keeps its host: `file://server/share/...`.
    let (prefix, rest) = match s.strip_prefix("//") {
        Some(unc) => ("file://", unc),
        None => ("file:///", s.as_str()),
    };
    let mut out = String::from(prefix);
    for c in rest.chars() {
        match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '.' | '_' | '~' | '/' | ':' | '!' | '$' | '&' | '\'' | '(' | ')' | '*' | '+' | ',' | ';' | '=' | '@' => out.push(c),
            c => {
                let mut buf = [0u8; 4];
                for b in c.encode_utf8(&mut buf).bytes() {
                    out.push_str(&format!("%{b:02X}"));
                }
            }
        }
    }
    out
}

/// `CreateFormats`: the link formats that have a value (image formats only
/// for an image URL), then the custom formats for an image URL.
pub fn create_formats(info: &AfterUploadInfo) -> Vec<FormatItem> {
    let r = &info.result;
    let some = |s: &str| (!s.is_empty()).then(|| s.to_owned());
    let result = sharex_uploaders::UploadResult {
        url: some(&r.url),
        thumbnail_url: some(&r.thumbnail_url),
        deletion_url: some(&r.deletion_url),
        shortened_url: some(&r.shortened_url),
        ..Default::default()
    };
    let fields = InfoFields {
        result: &result,
        file_name: &info.file_name,
        file_path: info.file_path.as_deref(),
        thumbnail_file_path: info.thumbnail_file_path.as_deref(),
        upload_duration: info.upload_duration,
    };
    let parse = |pattern: &str| parse_upload_info(&fields, pattern);
    let is_image_url = sharex_helpers::file_helpers::is_image_file(&r.url);
    let mut out = Vec::new();
    for t in LinkFormatEnum::ALL {
        use LinkFormatEnum as L;
        let image_format = matches!(t, L::HTMLImage | L::HTMLLinkedImage | L::ForumImage | L::ForumLinkedImage | L::WikiImage | L::WikiLinkedImage);
        if !is_image_url && image_format {
            continue;
        }
        let group = match t {
            L::ForumImage | L::ForumLinkedImage => s::AfterUploadWindow_GroupForums().to_owned(),
            L::HTMLImage | L::HTMLLinkedImage => "HTML".to_owned(),
            L::WikiImage | L::WikiLinkedImage => "Wiki".to_owned(),
            L::LocalFilePath | L::LocalFilePathUri => s::AfterUploadWindow_GroupLocal().to_owned(),
            _ => s::AfterUploadWindow_GroupLinks().to_owned(),
        };
        let path = info.file_path.as_deref();
        let text = match t {
            L::URL => r.url.clone(),
            L::ShortenedURL => r.shortened_url.clone(),
            L::ForumImage => parse(formats::FORUM_IMAGE),
            L::HTMLImage => parse(formats::HTML_IMAGE),
            L::WikiImage => parse(formats::WIKI_IMAGE),
            L::ForumLinkedImage => parse(formats::FORUM_LINKED_IMAGE),
            L::HTMLLinkedImage => parse(formats::HTML_LINKED_IMAGE),
            L::WikiLinkedImage => parse(formats::WIKI_LINKED_IMAGE),
            L::ThumbnailURL => r.thumbnail_url.clone(),
            L::LocalFilePath => path.map(|p| p.to_string_lossy().into_owned()).unwrap_or_default(),
            L::LocalFilePathUri => path.map(file_uri).unwrap_or_default(),
            L::Unknown(_) => String::new(),
        };
        if !text.is_empty() {
            let description = sharex_uploaders::factory::service_name("LinkFormatEnum", t.name());
            out.push(FormatItem { group, description, text });
        }
    }
    if is_image_url {
        for (description, format) in &info.clipboard_formats {
            let text = parse(format);
            if !text.is_empty() {
                out.push(FormatItem { group: s::AfterUploadWindow_GroupCustom().to_owned(), description: description.clone(), text });
            }
        }
    }
    out
}

pub struct AfterUploadWindow {
    info: AfterUploadInfo,
    formats: Vec<FormatItem>,
    selected: Option<usize>,
    texture: Option<egui::TextureHandle>,
    close_at: Option<Instant>,
    /// The remote image being downloaded (`LoadRemotePreviewAsync`).
    remote: Option<std::sync::mpsc::Receiver<Option<RgbaImage>>>,
    opened: bool,
}

impl AfterUploadWindow {
    pub fn new(mut info: AfterUploadInfo) -> Self {
        let formats = create_formats(&info);
        // `LoadInitialPreview`: an image task's file, else its image.
        if info.data_type == EDataType::Image
            && let Some(path) = info.file_path.as_ref().filter(|p| p.is_file() && sharex_helpers::file_helpers::is_image_file(&p.to_string_lossy()))
            && let Ok(image) = image::open(path)
        {
            info.image = Some(Arc::new(image.to_rgba8()));
        }
        if info.data_type != EDataType::Image {
            info.image = None;
        }
        Self { info, formats, selected: Some(0), texture: None, close_at: None, remote: None, opened: false }
    }

    /// `ResultText`: the URL, else the file name.
    fn result_text(&self) -> &str {
        if self.info.result.url.is_empty() { &self.info.file_name } else { &self.info.result.url }
    }

    fn file_exists(&self) -> bool {
        self.info.file_path.as_ref().is_some_and(|p| p.is_file())
    }

    /// `CopySelectedFormat`: the selected format, else the first.
    fn copy_selected(&self) -> Vec<Effect> {
        let item = self.selected.and_then(|i| self.formats.get(i)).or(self.formats.first());
        item.filter(|i| !i.text.is_empty()).map(|i| vec![Effect::CopyText(i.text.clone())]).unwrap_or_default()
    }

    /// `OnOpened`: the auto close timer, and the remote image when there's
    /// no local one.
    fn on_opened(&mut self) {
        if self.info.auto_close {
            self.close_at = Some(Instant::now() + Duration::from_secs(60));
        }
        if self.info.image.is_none() && self.info.data_type == EDataType::Image && !self.info.result.url.is_empty() {
            let url = self.info.result.url.clone();
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let _ = tx.send(download_image(&url));
            });
            self.remote = Some(rx);
        }
    }
}

/// `WebHelpers.DownloadImageAsync`.
fn download_image(url: &str) -> Option<RgbaImage> {
    use sharex_uploaders::http;
    let request = http::HttpRequest { method: sharex_uploaders::HttpMethod::GET, url: url.to_owned(), headers: Vec::new(), content: None };
    let response = http::send(request, http::RequestOptions::default()).ok()?;
    if !response.is_success() {
        return None;
    }
    image::load_from_memory(&response.response_data?).ok().map(|i| i.to_rgba8())
}

/// An icon-only `Button.compact.icon-button`: 40x40, a 15px icon.
fn icon_button(ui: &egui::Ui, rect: Rect, id: egui::Id, key: &str, enabled: bool) -> egui::Response {
    let p = palette(ui.ctx());
    let b = Button { padding: Thickness::all(0.0), ..Button::default() };
    let r = b.show(ui, rect, id, "", enabled);
    let color = if enabled {
        if r.hovered() { sharex_ui::theme::fluent(ui.ctx()).base_high } else { p.text }
    } else {
        // The disabled presenter at 35%.
        sharex_ui::border::group_content(sharex_ui::theme::fluent(ui.ctx()).base_medium_low, p.control_background, 0.35, p.background_panel)
    };
    icons::paint(ui.painter(), Rect::from_center_size(rect.center(), Vec2::splat(15.0)), key, color);
    r
}

impl Dialog for AfterUploadWindow {
    fn spec(&self) -> Spec {
        let shown = if self.file_exists() { self.info.file_path.as_ref().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default() } else { self.info.file_name.clone() };
        let title = format!("{} - {shown}", crate::rs_title(s::AfterUploadWindow_Title()));
        Spec { activate: false, ..Spec::sized(title, 940.0, 580.0, 760.0, 460.0) }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _cx: &mut Cx) -> Vec<Effect> {
        if !self.opened {
            self.opened = true;
            self.on_opened();
        }
        if let Some(rx) = &self.remote {
            match rx.try_recv() {
                Ok(image) => {
                    self.remote = None;
                    if let Some(image) = image {
                        self.info.image = Some(Arc::new(image));
                        self.texture = None;
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => ui.ctx().request_repaint_after(Duration::from_millis(100)),
                Err(_) => self.remote = None,
            }
        }
        if let Some(at) = self.close_at {
            if Instant::now() >= at {
                return vec![Effect::Close];
            }
            ui.ctx().request_repaint_after(at - Instant::now());
        }
        let p = palette(ui.ctx());
        let full = layout::background(ui);
        let result_text = self.result_text().to_owned();
        let header = layout::header(ui, full, icon::CircleCheck, s::AfterUploadWindow_UploadComplete(), Some(&result_text), false);
        let grid = Rect::from_min_max(pos2(full.min.x + 18.0, full.min.y + header + 14.0), pos2(full.max.x - 18.0, full.max.y - 18.0));
        let buttons_top = grid.max.y - Compact::HEIGHT;
        let row = Rect::from_min_max(grid.min, pos2(grid.max.x, buttons_top - 10.0));
        // `ColumnDefinitions="*,380"`.
        let left = Rect::from_min_max(row.min, pos2(row.max.x - 380.0 - 14.0, row.max.y));
        layout::panel(ui, left, 6.0);
        let inner = left.shrink(9.0);
        let band = section_header(ui, inner.min, inner.width(), icon::Link, s::AfterUploadWindow_ClipboardFormats());
        let view = Rect::from_min_max(pos2(inner.min.x, inner.min.y + band + 6.0), inner.max);
        // Items: `Padding="8,6"`, `Margin="0,1"`; the group (11px at 80%)
        // over the description, the text in the 130px column's right.
        let group_font = font(11.0, 600);
        let desc_font = font(13.0, 600);
        let text_font = egui::FontId::new(12.0, sharex_ui::fonts::mono_semibold());
        let stack = line_height(ui, &group_font) + 1.0 + line_height(ui, &desc_font);
        let item_h = 6.0 + stack + 6.0;
        let pitch = item_h + 2.0;
        let content = pitch * self.formats.len() as f32;
        let offset = layout::scroll_offset(ui, ui.id().with("formats-scroll"), view, content);
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(view));
        child.set_clip_rect(view.intersect(ui.clip_rect()));
        let mut copy = false;
        for (i, item) in self.formats.iter().enumerate() {
            let r = Rect::from_min_size(pos2(view.min.x, view.min.y + 1.0 + pitch * i as f32 - offset), vec2(view.width(), item_h));
            if !r.intersects(view) {
                continue;
            }
            let resp = child.interact(r.intersect(view), child.id().with(("format", i)), egui::Sense::click());
            let selected = self.selected == Some(i);
            let fg = if selected {
                border::border(child.painter(), r, 4.0, 0.0, Some(Brush::accent(ui.ctx())), None);
                p.accent_foreground
            } else {
                if resp.hovered() {
                    border::border(child.painter(), r, 4.0, 0.0, Some(Brush::Solid(p.control_background_hover)), None);
                }
                p.text_secondary
            };
            if resp.clicked() {
                self.selected = Some(i);
            }
            if resp.double_clicked() {
                copy = true;
            }
            let cell = r.shrink2(vec2(8.0, 6.0));
            let y = center_y(cell, stack);
            let left_w = 130.0;
            let g = galley(&child, &item.group, group_font.clone(), fg.gamma_multiply(0.8), left_w);
            child.painter().galley(pos2(cell.min.x, y), g, fg);
            let g = galley(&child, &item.description, desc_font.clone(), fg, left_w);
            child.painter().galley(pos2(cell.min.x, y + line_height(ui, &group_font) + 1.0), g, fg);
            let tx = cell.min.x + left_w + 10.0;
            let ty = center_y(cell, line_height(ui, &text_font));
            let g = galley(&child, &item.text, text_font.clone(), fg, cell.max.x - tx);
            child.painter().galley(pos2(tx, ty), g, fg);
        }
        layout::scroll_bar(ui, view, content, offset, 16.0);
        // The preview.
        let right = Rect::from_min_max(pos2(left.max.x + 14.0, row.min.y), row.max);
        layout::panel(ui, right, 6.0);
        let mut preview_clicked = false;
        if let Some(image) = self.info.image.clone() {
            let tex = texture(ui.ctx(), &mut self.texture, "after-upload-preview", &image);
            let area = right.shrink(1.0);
            paint_image(ui, area.shrink(10.0), tex, image.dimensions());
            size_badge(ui, area, image.dimensions());
            let r = ui.interact(right, ui.id().with("preview"), egui::Sense::click());
            preview_clicked = r.clicked();
            r.on_hover_cursor(egui::CursorIcon::PointingHand);
        } else {
            let f = layout::text_font();
            let lh = line_height(ui, &f);
            let mut y = center_y(right, 48.0 + 8.0 + lh);
            icons::paint(ui.painter(), Rect::from_min_size(pos2((right.center().x - 24.0).round(), y), Vec2::splat(48.0)), icon::Link, p.text.gamma_multiply(0.45));
            y += 56.0;
            sharex_ui::controls::text_centered_at(ui, right.center().x, y, s::AfterUploadWindow_NoPreview(), f, p.text_secondary);
        }
        // The buttons: five 40px icon buttons 6px apart, Close on the right.
        let exists = self.file_exists();
        let has_image = self.info.image.is_some();
        let buttons = [
            (icon::Image, has_image),
            (icon::Copy, !self.formats.is_empty()),
            (icon::ExternalLink, !self.info.result.url.is_empty()),
            (icon::File, exists),
            (icon::Folder, exists),
        ];
        let mut out = Vec::new();
        for (i, (key, enabled)) in buttons.iter().enumerate() {
            let r = Rect::from_min_size(pos2(grid.min.x + 46.0 * i as f32, buttons_top), Vec2::splat(40.0));
            if icon_button(ui, r, ui.id().with(("action", i)), key, *enabled).clicked() {
                match i {
                    0 => out.extend(self.info.image.as_ref().map(|im| Effect::CopyImage((**im).clone()))),
                    1 => copy = true,
                    2 => out.push(Effect::OpenUrl(self.info.result.url.clone())),
                    3 => out.extend(self.info.file_path.clone().map(Effect::OpenFile)),
                    _ => out.extend(self.info.file_path.clone().map(Effect::OpenFolderWithFile)),
                }
            }
        }
        let close = Compact { min_width: 96.0 };
        let close_text = s::AfterUploadWindow_Close();
        let w = Button { padding: Thickness::hv(12.0, 5.0), ..Button::default() }.width(ui, close_text).max(96.0);
        let close_rect = Rect::from_min_size(pos2(grid.max.x - w, buttons_top), vec2(w, Compact::HEIGHT));
        let closed = Button { padding: Thickness::hv(12.0, 5.0), ..Button::default() }.show(ui, close_rect, ui.id().with("close"), close_text, true).clicked();
        let _ = close;
        if copy {
            out.extend(self.copy_selected());
        }
        if preview_clicked {
            out.push(Effect::NotPorted("The image viewer"));
        }
        if closed || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            out.push(Effect::Close);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(url: &str, file: Option<&str>) -> AfterUploadInfo {
        AfterUploadInfo {
            file_name: "a.png".into(),
            file_path: file.map(PathBuf::from),
            thumbnail_file_path: None,
            result: UploadResult { url: url.into(), thumbnail_url: "https://t.example.com/a.png".into(), ..Default::default() },
            data_type: EDataType::Image,
            image: None,
            upload_duration: None,
            auto_close: false,
            clipboard_formats: vec![("Markdown".into(), "![]($url)".into())],
        }
    }

    #[test]
    fn image_urls_get_every_format_in_sharex_order() {
        let f = create_formats(&info("https://i.example.com/a.png", None));
        let names: Vec<_> = f.iter().map(|i| (i.group.as_str(), i.description.as_str())).collect();
        assert_eq!(
            names,
            [
                ("Links", "Full URL"),
                ("Forums", "Full Image for Forums"),
                ("HTML", "Full Image as HTML"),
                ("Wiki", "Full Image for Wiki"),
                ("Forums", "Linked Thumbnail for Forums"),
                ("HTML", "Linked Thumbnail as HTML"),
                ("Wiki", "Linked Thumbnail for Wiki"),
                ("Links", "Thumbnail"),
                ("Custom", "Markdown"),
            ]
        );
        assert_eq!(f[2].text, "<img src=\"https://i.example.com/a.png\">");
        assert_eq!(f[8].text, "![](https://i.example.com/a.png)");
    }

    #[test]
    fn other_urls_get_the_links_only() {
        let f = create_formats(&info("https://example.com/a.zip", None));
        assert_eq!(f.iter().map(|i| i.description.as_str()).collect::<Vec<_>>(), ["Full URL", "Thumbnail"]);
    }
}
