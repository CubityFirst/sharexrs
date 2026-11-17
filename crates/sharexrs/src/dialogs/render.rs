//! `sharexrs --render <screen>` for the upload windows, with content as
//! the reference shots of ShareX have it:
//!
//! - `text-upload` (`--text` prefilled and selected),
//! - `url-upload`, `shorten-url` (`--text` typed in: an invalid one shows
//!   the validation line),
//! - `clipboard-upload` (`--content text|image|files|empty`, `--text`,
//!   `--image <png>`, `--files a,b`; `--viewer-only` without "Don't show
//!   this window again"),
//! - `drop-window` (on black, as `PrintWindow` gets it at its opacity),
//! - `multi-upload` (`--count N`), `large-file`,
//! - `before-upload`, `after-upload`, `qr-code` (see their modules).

use std::path::PathBuf;

use sharex_settings::{ApplicationConfig, TaskSettings};
use sharex_ui::offscreen::Offscreen;

use super::{Cx, Dialog};
use crate::upload::UploadersConfigStore;
use crate::upload::manager::ClipboardContent;

/// What a dialog screen is drawn with.
#[derive(Default)]
pub struct Options {
    pub text: Option<String>,
    pub content: Option<String>,
    pub image: Option<PathBuf>,
    pub files: Vec<PathBuf>,
    pub count: usize,
    pub viewer_only: bool,
    pub hover: Option<egui::Pos2>,
    pub clicks: Vec<egui::Pos2>,
    pub dark: bool,
    pub scale: f32,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

fn load_image(path: &Option<PathBuf>) -> Result<image::RgbaImage, String> {
    let path = path.as_ref().ok_or("--image <png> is required")?;
    Ok(image::open(path).map_err(|e| format!("{}: {e}", path.display()))?.to_rgba8())
}

/// An `UploadersConfig` with the mock custom uploader (`.sxcu` in
/// `G:/scripts/sharexrs-ref/mock`) chosen for every kind, as the reference
/// ShareX has it.
pub fn mock_config() -> sharex_uploaders::UploadersConfig {
    let sxcu = serde_json::json!({
        "Version": "18.0.1",
        "Name": "Mock uploader",
        "DestinationType": "ImageUploader, TextUploader, FileUploader, URLShortener",
        "RequestMethod": "POST",
        "RequestURL": "http://127.0.0.1:18765/upload",
        "Headers": {"X-Mock": "1"},
        "Body": "MultipartFormData",
        "Arguments": {"url": "{input}"},
        "FileFormName": "file",
        "URL": "{json:url}",
        "ThumbnailURL": "{json:thumb}",
        "DeletionURL": "{json:delete}"
    });
    let mut config = sharex_uploaders::UploadersConfig::default();
    if let Ok(Some(item)) = sharex_uploaders::custom::CustomUploaderItem::from_json(&sxcu.to_string()) {
        config.CustomUploadersList.0.push(item);
    }
    config
}

/// The dialog a screen name stands for.
fn dialog(name: &str, o: &Options) -> Result<Box<dyn Dialog>, String> {
    let task = TaskSettings::default();
    Ok(match name {
        "text-upload" => Box::new(super::text_upload::TextUploadWindow::new(Some(o.text.clone().unwrap_or_default()), task)),
        "url-upload" | "shorten-url" => {
            use super::url_input::{UrlKind, UrlWindow};
            let kind = if name == "url-upload" { UrlKind::Upload } else { UrlKind::Shorten };
            let mut w = UrlWindow::new(kind, None, task);
            w.url = o.text.clone().unwrap_or_default();
            Box::new(w)
        }
        "clipboard-upload" => {
            let content = match o.content.as_deref().unwrap_or("text") {
                "text" => Some(ClipboardContent::Text(o.text.clone().unwrap_or_default())),
                "image" => Some(ClipboardContent::Image(load_image(&o.image)?)),
                "files" => Some(ClipboardContent::Files(o.files.clone())),
                "empty" => None,
                other => return Err(format!("no clipboard content {other}")),
            };
            Box::new(super::clipboard_upload::ClipboardUploadWindow::with_content(content, task, !o.viewer_only))
        }
        "drop-window" => Box::new(super::drop_window::DropWindow::new(&ApplicationConfig::default(), task)),
        "multi-upload" => {
            let files = (0..o.count.max(11)).map(|i| PathBuf::from(format!("{i}.png"))).collect();
            Box::new(super::confirm::ConfirmWindow::new(super::confirm::Confirm::MultiUpload { files, task: Box::new(task) }))
        }
        "large-file" => Box::new(super::confirm::ConfirmWindow::new(super::confirm::Confirm::LargeFile { reply: None })),
        "before-upload" => {
            // As ShareX shows a text (`--content text`, the default) or an
            // image file (`--image`) going to the mock custom uploader.
            let mut task = task;
            task.ImageDestination = sharex_settings::ImageDestination::CustomImageUploader;
            task.TextDestination = sharex_settings::TextDestination::CustomTextUploader;
            task.FileDestination = sharex_settings::FileDestination::CustomFileUploader;
            let image = o.content.as_deref() == Some("image");
            let request = crate::upload::BeforeUpload {
                data_type: if image { crate::tasks::EDataType::Image } else { crate::tasks::EDataType::Text },
                file_name: o.text.clone().unwrap_or_else(|| "fDwwlb1mSj.txt".into()),
                file_path: if image { o.image.clone() } else { None },
                image: None,
                task,
            };
            let (tx, _rx) = std::sync::mpsc::channel();
            Box::new(super::before_upload::BeforeUploadWindow::new(request, &mock_config(), tx))
        }
        "after-upload" => {
            let base = "http://127.0.0.1:18765";
            let url = o.text.clone().unwrap_or_else(|| format!("{base}/u/mock0.png"));
            let image = o.image.as_ref().map(|_| load_image(&o.image)).transpose()?.map(std::sync::Arc::new);
            let file = o.files.first().cloned();
            let info = super::after_upload::AfterUploadInfo {
                file_name: file.as_ref().and_then(|f| f.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "O1G4EPP6RK.png".into()),
                file_path: file.clone(),
                thumbnail_file_path: None,
                result: crate::tasks::UploadResult {
                    url: if file.is_some() { String::new() } else { url.clone() },
                    thumbnail_url: if file.is_some() { String::new() } else { url.replace("/u/", "/t/") },
                    deletion_url: if file.is_some() { String::new() } else { url.replace("/u/", "/d/").trim_end_matches(".png").to_owned() },
                    ..Default::default()
                },
                data_type: crate::tasks::EDataType::Image,
                image,
                upload_duration: None,
                auto_close: false,
                clipboard_formats: Vec::new(),
            };
            Box::new(super::after_upload::AfterUploadWindow::new(info))
        }
        "qr-code" => {
            let mut w = super::qr_code::QrCodeWindow::new(Some(o.text.clone().unwrap_or_default()));
            w.size = o.count as f64;
            Box::new(w)
        }
        other => return Err(format!("no dialog named {other}")),
    })
}

/// Whether `screen` is one of these.
pub fn is_dialog_screen(screen: &str) -> bool {
    matches!(
        screen,
        "text-upload"
            | "url-upload"
            | "shorten-url"
            | "clipboard-upload"
            | "drop-window"
            | "multi-upload"
            | "large-file"
            | "before-upload"
            | "after-upload"
            | "qr-code"
    )
}

/// Draws the dialog `name` at its window's size.
pub fn render(name: &str, o: &Options) -> Result<image::RgbaImage, String> {
    let mut d = dialog(name, o)?;
    let spec = d.spec();
    let mut settings = ApplicationConfig::default();
    let store = UploadersConfigStore::loaded(sharex_uploaders::UploadersConfig::default());
    let mut height = spec.height;
    if spec.size_to_content {
        let mut probe = Offscreen::new([spec.width as u32, 400], o.scale, o.dark)?;
        probe.run(Vec::new(), &mut |ui: &mut egui::Ui| height = d.content_height(ui));
    }
    let w = o.width.unwrap_or((spec.width * o.scale).round() as u32);
    let h = o.height.unwrap_or((height * o.scale).round() as u32);
    let mut screen = Offscreen::new([w, h], o.scale, o.dark)?;
    if spec.borderless {
        screen.transparent = true;
    }
    let opacity = d.opacity();
    let mut image = crate::render::render_frames(&mut screen, o.hover, &o.clicks, &mut |ui: &mut egui::Ui| {
        let mut cx = Cx { settings: &mut settings, uploaders: &store };
        ui.multiply_opacity(opacity);
        let _ = d.ui(ui, &mut cx);
    });
    if spec.borderless {
        // On black, as `PrintWindow` gets a translucent window.
        for p in image.pixels_mut() {
            let a = p[3] as u32;
            let c = |v: u8| ((v as u32 * a + 127) / 255) as u8;
            *p = image::Rgba([c(p[0]), c(p[1]), c(p[2]), 255]);
        }
    }
    Ok(image)
}
