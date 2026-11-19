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
//! - `before-upload`, `after-upload`, `qr-code` (see their modules),
//! - `destination-<page>`: Destination settings on a page (`imgur`, `ftp`...),
//!   `--state` a state of its cards from made-up data
//!   ([`super::destination_settings::render_config`] and
//!   `seed_render_state`: `connected-display`, `code-entered`, `listed`...),
//!   `--scroll` its page scrolled,
//! - `oauth-listener` (waiting for the browser), `input-box` (MEGA's
//!   two-factor question, `--text` typed in), `email-window` (`--state
//!   empty|prefilled|edited`), `youtube-options` (`--state
//!   default|edited`; `--clicks` on the combo box opens it),
//!   `remote-storage-not-configured` (the message box), `remote-storage`
//!   (see [`crate::remote_storage_browser::render_dialog`]),
//! - `update-message` (`--content installed|portable|prerelease|dev`: the
//!   setup flow, the portable one, a pre-release, a dev build without the
//!   changelog link), `update-downloader` (`--content
//!   waiting|getting-size|downloading|completed|error`), from
//!   [`update_checker`]: ShareXrs 22.4.0.0 and a 22.5.0 release.

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
    /// `--db`: the History.db the history windows show.
    pub db: Option<PathBuf>,
    /// `--state`: which state of a window (a dialog open, a menu...).
    pub state: Option<String>,
    /// `--select`: indexes of selected items.
    pub select: Vec<usize>,
    /// `--context X,Y`: a right click there.
    pub context: Option<egui::Pos2>,
    /// `--open`: the submenus opened along this path.
    pub open: Vec<usize>,
    /// `--scroll`: a scroll offset.
    pub scroll: f32,
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
        // `--image`: the image to print (a flat colour by default).
        "print-window" | "print-preview" => {
            let img = match &o.image {
                Some(p) => image::open(p).map_err(|e| e.to_string())?.to_rgba8(),
                None => image::RgbaImage::from_pixel(640, 400, image::Rgba([40, 120, 200, 255])),
            };
            if name == "print-preview" {
                Box::new(super::print_window::PrintPreview::new(img, crate::print::PrintSettings::default()))
            } else {
                Box::new(super::print_window::PrintWindow::new(img, false))
            }
        }
        "multi-upload" => {
            let files = (0..o.count.max(11)).map(|i| PathBuf::from(format!("{i}.png"))).collect();
            Box::new(super::confirm::ConfirmWindow::new(super::confirm::Confirm::MultiUpload { files, task: Box::new(task) }))
        }
        // `--text`: the existing file's path; `--state empty` clears the
        // new name, `--state name=<text>` types one.
        "file-exists" => {
            let path = o.text.clone().unwrap_or_else(|| r"C:\ShareX\Screenshots\2026-11\ShareX_test.png".into());
            let mut w = super::file_exist::FileExistWindow::new(std::path::Path::new(&path), None);
            w.assume_exists();
            match o.state.as_deref() {
                Some("empty") => w.new_name.clear(),
                Some(s) if s.starts_with("name=") => {
                    w.new_name = s["name=".len()..].to_owned();
                    w.select_on_open = false;
                }
                _ => {}
            }
            Box::new(w)
        }
        // The selected pack comes from `--state legacy` (see `render`).
        "icon-pack-prompt" => Box::new(super::icon_pack_prompt::IconPackPromptWindow),
        // `--state checking|up-to-date|available|failed|disabled`.
        "about" => {
            use sharex_update::checker::UpdateStatus;
            let state = o.state.as_deref().unwrap_or("checking");
            // `--text`/`--content`: the product name and brand (ShareX's, to
            // compare with its shots).
            let product = o.text.clone().unwrap_or_else(|| "ShareXrs 22.4".into());
            let mut w = super::about::AboutWindow::new(product, state == "disabled", None);
            if let Some(brand) = &o.content {
                w.brand = brand.clone();
            }
            let status = match state {
                "up-to-date" => Some(UpdateStatus::UpToDate),
                "available" => Some(UpdateStatus::UpdateAvailable),
                "failed" => Some(UpdateStatus::UpdateCheckFailed),
                _ => None,
            };
            if let Some(status) = status {
                let mut c = update_checker("installed");
                c.status = status;
                w.checked(c);
            }
            // The clicker: `clicker` after three clicks on the logo,
            // `clicker-store` with the store open and a few things bought.
            if state.starts_with("clicker") {
                w.clicker.activated = true;
                let g = &mut w.clicker.game;
                for _ in 0..3 {
                    g.click();
                }
                if state == "clicker-store" {
                    g.state.logos = 1200.0;
                    g.state.lifetime_logos = 1300.0;
                    g.state.store_discovered = true;
                    g.buy_building(&crate::clicker::BUILDINGS[0], 3);
                    g.buy_upgrade(&crate::clicker::UPGRADES[0]);
                    w.clicker.store_visible = true;
                }
            }
            Box::new(w)
        }
        // A generated log; `--state upload-confirm` shows the confirmation.
        "debug-log" => {
            let lines = [
                "ShareXrs starting.",
                "Version: 22.4.0",
                "Operating system: Windows 11 Pro",
                r"Personal path: C:\ShareXrs",
                "Startup time: 812 ms",
                "Task status: Preparing",
                r"Image saved to file: C:\ShareXrs\Screenshots\2026-11\ShareX_test.png",
                "URL opened: https://example.invalid/",
            ];
            // `--files <log.txt>`: that log instead.
            let text: String = match o.files.first() {
                Some(f) => std::fs::read_to_string(f).map_err(|e| format!("{}: {e}", f.display()))?,
                None => lines.iter().enumerate().map(|(i, l)| format!("2026-11-19 09:00:{:02}.{:03} - {l}\r\n", i * 3, i * 37)).collect(),
            };
            // `--text`: the startup path.
            let startup = o.text.clone().unwrap_or_else(|| r"C:\ShareXrs\".into());
            let mut w = super::debug_log::DebugLogWindow::with_text(text, Some(r"C:\ShareXrs\Logs\ShareX-Log-2026-11.txt".into()), startup);
            w.confirm_upload = o.state.as_deref() == Some("upload-confirm");
            Box::new(w)
        }
        // The toast: `--state tray-minimized` (the tip), else a finished
        // task's with `--image` (or none) and `--text` (its file path);
        // `--content hover` or `hover:<button>` draws the pointer over the
        // card or a button (`tip:<button>` with its tooltip).
        "notification" => {
            use super::notification::{NotificationConfig, NotificationWindow};
            let general = sharex_settings::TaskSettingsGeneral::default();
            let config = if o.state.as_deref() == Some("tray-minimized") {
                NotificationConfig::tip(&general, sharex_strings::sharex::ShareXIsMinimizedToTheSystemTray(), "ShareX", 8000)
            } else {
                let mut c = NotificationConfig::new(&general).with_clicks(&general);
                c.title = format!("ShareX - {}", sharex_strings::sharex::TaskManager_task_UploadCompleted_ShareX___Task_completed());
                c.text = o.text.clone().unwrap_or_default();
                c.file_path = c.text.clone();
                c.image = o.image.as_ref().map(|_| load_image(&o.image)).transpose()?.map(std::sync::Arc::new);
                c
            };
            let mut w = NotificationWindow::new(config).ok_or("nothing to show")?;
            w.force_hover = match o.content.as_deref() {
                Some("hover") => Some(None),
                Some(h) if h.starts_with("hover:") => Some(h["hover:".len()..].parse().ok()),
                Some(h) if h.starts_with("tip:") => Some(h["tip:".len()..].parse().ok()),
                _ => None,
            };
            w.force_tooltip = o.content.as_deref().is_some_and(|c| c.starts_with("tip:"));
            Box::new(w)
        }
        // `--image`: the capture (none: a recording, `--files` its path);
        // `--text`: the file name; `--state goal:<n>`: a goal picked;
        // `--content <AfterCaptureJob>`: the task's after capture tasks.
        "after-capture" => {
            use super::after_capture::{AfterCaptureWindow, Target};
            let mut task = TaskSettings::default();
            if let Some(job) = o.content.as_deref() {
                task.AfterCaptureJob = sharex_settings::AfterCaptureTasks::parse(job).ok_or("--content: AfterCaptureTasks")?;
            }
            let image = o.image.as_ref().map(|_| load_image(&o.image)).transpose()?;
            let file = o.files.first().cloned();
            let mut w = AfterCaptureWindow::new(Target::Preview, task, image, file, o.text.clone().unwrap_or_else(|| "AkM5MGPNSL".into()), true, &mock_config());
            if let Some(g) = o.state.as_deref().and_then(|s| s.strip_prefix("goal:")).and_then(|g| g.parse::<usize>().ok()) {
                let goals = super::after_capture::available_goals(w.has_image(), true);
                if let Some(goal) = goals.get(g) {
                    w.goal = *goal;
                }
            }
            Box::new(w)
        }
        // `--image` (or `--files a,b` with `--count` the one shown);
        // `--width`/`--height`: the screen.
        "image-viewer" => {
            use super::image_viewer::{ImageViewerWindow, Images};
            let mut images = Images::default();
            if o.files.is_empty() {
                let path = o.image.clone().ok_or("--image or --files")?;
                images.load_image(load_image(&o.image)?, path.file_name().map(|n| n.to_string_lossy().into_owned()));
            } else if !images.load_files(&o.files, o.count) {
                return Err("no image among --files".into());
            }
            let (w, h) = (o.width.unwrap_or(1024) as i32, o.height.unwrap_or(720) as i32);
            Box::new(ImageViewerWindow::new(images).with_size(w, h))
        }
        // `--state add-overlay|edit-overlay|reset-confirm`.
        "quick-task-editor" => {
            let mut w = super::quick_task_editor::QuickTaskMenuEditorWindow::new();
            match o.state.as_deref() {
                Some("add-overlay") => w.open_editor(None),
                Some("edit-overlay") => w.open_editor(Some(0)),
                Some("reset-confirm") => w.confirm_reset = true,
                _ => {}
            }
            Box::new(w)
        }
        // `--text`: the title (ShareX's to compare); `--content hover:<n>`.
        "actions-toolbar" => {
            let mut w = super::actions_toolbar::ActionsToolbarWindow::new(&ApplicationConfig::default());
            if let Some(t) = &o.text {
                w.title = t.clone();
            }
            w.hover = o.content.as_deref().and_then(|c| c.strip_prefix("hover:")).and_then(|n| n.parse().ok());
            Box::new(w)
        }
        // `--state add-menu`.
        "actions-toolbar-editor" => {
            let mut w = super::actions_toolbar::ActionsToolbarEditorWindow::new();
            w.open_add_menu = o.state.as_deref() == Some("add-menu");
            Box::new(w)
        }
        // A failed FFmpeg run's output (`--files <log>` for its text).
        "output-box" => {
            let text = match o.files.first() {
                Some(f) => std::fs::read_to_string(f).map_err(|e| format!("{}: {e}", f.display()))?,
                None => ["ffmpeg version 7.1 Copyright (c) 2000-2024 the FFmpeg developers", "[gdigrab @ 0000020b] Capturing whole desktop", "Unknown encoder 'libx265x'"].join("\r\n"),
            };
            Box::new(super::output_box::OutputBoxWindow::new(&text, sharex_strings::helpers_lib::FFmpegCLIManager_Error_title(), true))
        }
        // A report of two synthetic monitors (the second 150%, above the
        // first's right half), the pointer on the first.
        "display-diagnostics" => {
            use sharex_capture::{Rect as R, Screen};
            let screens = [
                Screen { bounds: R::new(0, 0, 1920, 1080), work_area: R::new(0, 0, 1920, 1032), scale: 1.0, primary: true },
                Screen { bounds: R::new(960, -1440, 2560, 1440), work_area: R::new(960, -1440, 2560, 1440), scale: 1.5, primary: false },
            ];
            let at = chrono::Local::now();
            let native = super::display_diagnostics::sample_snapshot(&screens);
            let config = ApplicationConfig::default();
            let data = super::display_diagnostics::collect(&super::display_diagnostics::Inputs {
                screens: &screens,
                window: Some((R::new(510, 170, 900, 740), 1.0)),
                settings: &config,
                now: at,
                native: Some(&native),
                geometry: &[],
            });
            Box::new(super::display_diagnostics::DisplayDiagnosticsWindow::with_data(data))
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
        "response" => {
            // The mock's answer to a custom uploader test (`--content
            // result|info|text` picks the section).
            let base = "http://127.0.0.1:18765";
            let text = format!("{{\"url\": \"{base}/u/mock0.png\", \"thumb\": \"{base}/t/mock0.png\", \"delete\": \"{base}/d/mock0\"}}");
            let data = super::response::ResponseData {
                url: format!("{base}/u/mock0.png"),
                thumbnail_url: format!("{base}/t/mock0.png"),
                deletion_url: format!("{base}/d/mock0"),
                response: text.clone(),
                response_info: Some(sharex_uploaders::http::ResponseInfo {
                    status_code: 200,
                    status_description: "OK".into(),
                    response_url: Some(format!("{base}/upload")),
                    headers: vec![("Content-Type".into(), "application/json".into()), ("Content-Length".into(), text.len().to_string())],
                    response_text: Some(text),
                    response_data: None,
                }),
                ..Default::default()
            };
            let mut w = super::response::ResponseWindow::new(data);
            w.section = match o.content.as_deref() {
                Some("info") => 1,
                Some("text") => 2,
                _ => 0,
            };
            Box::new(w)
        }
        "custom-uploader" => {
            let config = mock_config();
            let mut w = super::custom_uploaders::CustomUploaderSettingsWindow::new(&config);
            w.section = match o.content.as_deref() {
                Some("request") => 1,
                Some("body") => 2,
                Some("response") => 3,
                Some("test") => 4,
                _ => 0,
            };
            Box::new(w)
        }
        "qr-code" => {
            let mut w = super::qr_code::QrCodeWindow::new(Some(o.text.clone().unwrap_or_default()));
            w.size = o.count as f64;
            Box::new(w)
        }
        "update-message" => {
            let checker = update_checker(o.content.as_deref().unwrap_or("installed"));
            Box::new(super::update::UpdateMessageWindow::new(Box::new(checker), true, super::update::UpdateOrigin::User))
        }
        "update-downloader" => {
            use sharex_update::downloader::DownloadProgress;
            use sharex_update::downloader_window::{DownloaderWindowStatus, progress_text};
            use sharex_strings::helpers_lib as hs;
            let checker = update_checker("installed");
            let mut d = super::update::DownloaderDialog::new(&checker, std::sync::Arc::new(crate::update::AppHost), super::update::UpdateOrigin::User);
            let w = &mut d.window;
            // Drawn as it is, never started.
            w.auto_start_download = false;
            let status = |t: &str| hs::DownloaderForm_ChangeStatus_Status___0_().replace("{0}", t);
            let progress = DownloadProgress { file_size: 52_428_800, downloaded_size: 13_107_200, download_speed: 3_276_800.0 };
            match o.content.as_deref().unwrap_or("downloading") {
                "waiting" => {}
                "getting-size" => {
                    w.status = DownloaderWindowStatus::DownloadStarted;
                    w.action_text = hs::DownloaderForm_StartDownload_Cancel().to_owned();
                    w.status_text = status(hs::DownloaderForm_StartDownload_Getting_file_size_());
                }
                "downloading" => {
                    w.status = DownloaderWindowStatus::DownloadStarted;
                    w.action_text = hs::DownloaderForm_StartDownload_Cancel().to_owned();
                    w.status_text = status(hs::DownloaderForm_StartDownload_Downloading_());
                    w.progress_text = progress_text(&progress);
                    w.progress_value = progress.percentage();
                }
                "completed" => {
                    let done = DownloadProgress { downloaded_size: progress.file_size, ..progress };
                    w.status = DownloaderWindowStatus::DownloadCompleted;
                    w.action_text = hs::DownloaderForm_fileDownloader_DownloadCompleted_Install().to_owned();
                    w.status_text = status(hs::DownloaderForm_fileDownloader_DownloadCompleted_Download_completed_());
                    w.progress_text = progress_text(&done);
                    w.progress_value = 100.0;
                }
                "error" => {
                    w.status = DownloaderWindowStatus::DownloadStarted;
                    w.action_text = hs::DownloaderForm_StartDownload_Cancel().to_owned();
                    w.status_text = status("Response status code does not indicate success: 404 (Not Found).");
                }
                other => return Err(format!("no downloader state {other}")),
            }
            Box::new(d)
        }
        "oauth-listener" => {
            let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            Box::new(super::oauth_listener::OAuthListenerWindow::new(sharex_uploaders::http::CancellationToken::new(), done))
        }
        "input-box" => {
            let title = sharex_strings::uploaders_lib::DestinationSettings_Enter_current_two_factor_authentication_code();
            Box::new(super::input_box::InputBoxWindow::new(title, o.text.as_deref(), None, None, None))
        }
        "email-window" => {
            // As an email upload opens it: `EmailLastTo` (remembered), the
            // default subject and body; "edited": typed over.
            let d = sharex_uploaders::UploadersConfig::default();
            let (subject, body) = (d.EmailDefaultSubject.unwrap_or_default(), d.EmailDefaultBody.unwrap_or_default());
            match o.state.as_deref().unwrap_or("prefilled") {
                "empty" => Box::new(super::email_window::EmailWindow::new("", "", "", None)),
                "prefilled" => Box::new(super::email_window::EmailWindow::new("", &subject, &body, None)),
                "edited" => Box::new(super::email_window::EmailWindow::new(
                    "someone@example.com",
                    "Screenshot of the build",
                    "Here is the screenshot.
It shows the dialog.",
                    None,
                )),
                other => return Err(format!("no email window state {other}")),
            }
        }
        "youtube-options" => {
            use sharex_uploaders::enums::YouTubeVideoPrivacy;
            // The file name and the account's privacy, as the upload opens it.
            match o.state.as_deref().unwrap_or("default") {
                "default" => Box::new(super::youtube_options::YouTubeOptionsWindow::new("Recording_01.mp4", "", &YouTubeVideoPrivacy::Private, None)),
                "edited" => Box::new(super::youtube_options::YouTubeOptionsWindow::new(
                    "A screen recording",
                    "Recorded with ShareXrs.",
                    &YouTubeVideoPrivacy::Unlisted,
                    None,
                )),
                other => return Err(format!("no YouTube options state {other}")),
            }
        }
        "remote-storage-not-configured" => {
            use crate::message_box::{MessageBox, MessageBoxIcon};
            // `TaskHelpers.OpenRemoteStorageBrowser` without a provider.
            let title = sharex_strings::sharex::MainMenuBuilder_RemoteStorageBrowser().trim_end_matches(['.', '…']);
            let text = sharex_strings::sharex::TaskHelpers_RemoteStorageNotConfigured();
            Box::new(super::message_dialog::MessageDialog::new(MessageBox::ok(format!("ShareXrs - {title}"), text, MessageBoxIcon::Information), None))
        }
        s if crate::remote_storage_browser::is_render_screen(s) => crate::remote_storage_browser::render_dialog(s, o)?,
        page if page.starts_with("destination-") => {
            // `destination-<page id>`: Destination settings on that page.
            let id = page.trim_start_matches("destination-").trim_start_matches("settings-");
            let mut w = super::destination_settings::DestinationSettingsWindow::new(None);
            if !w.select_page(id) {
                return Err(format!("no destination settings page {id}"));
            }
            w.seed_render_state(id, o.state.as_deref().unwrap_or(""))?;
            w.set_scroll(o.scroll);
            Box::new(w)
        }
        s if crate::history_ui::is_render_screen(s) => crate::history_ui::render_dialog(s, o)?,
        s if crate::ocr_window::is_render_screen(s) => crate::ocr_window::render_dialog(s, o)?,
        s if crate::image_editor::is_render_screen(s) => crate::image_editor::render_dialog(s, o)?,
        s if crate::image_tools::render::is_render_screen(s) => crate::image_tools::render::render_dialog(s, o)?,
        s if crate::video_tools::render::is_render_screen(s) => crate::video_tools::render::render_dialog(s, o)?,
        s if crate::image_effects::render::is_render_screen(s) => crate::image_effects::render::render_dialog(s, o)?,
        s if crate::screen_tools::render::is_render_screen(s) => crate::screen_tools::render::render_dialog(s, o)?,
        s if crate::scrolling_capture::render::is_render_screen(s) => crate::scrolling_capture::render::render_dialog(s, o)?,
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
            | "print-window"
            | "print-preview"
            | "multi-upload"
            | "large-file"
            | "file-exists"
            | "icon-pack-prompt"
            | "about"
            | "debug-log"
            | "notification"
            | "after-capture"
            | "image-viewer"
            | "quick-task-editor"
            | "actions-toolbar"
            | "actions-toolbar-editor"
            | "output-box"
            | "display-diagnostics"
            | "before-upload"
            | "after-upload"
            | "qr-code"
            | "response"
            | "custom-uploader"
            | "update-message"
            | "update-downloader"
            | "oauth-listener"
            | "input-box"
            | "email-window"
            | "youtube-options"
            | "remote-storage-not-configured"
    ) || screen.starts_with("destination-")
        || crate::remote_storage_browser::is_render_screen(screen)
        || crate::history_ui::is_render_screen(screen)
        || crate::ocr_window::is_render_screen(screen)
        || crate::image_editor::is_render_screen(screen)
        || crate::image_tools::render::is_render_screen(screen)
        || crate::video_tools::render::is_render_screen(screen)
        || crate::image_effects::render::is_render_screen(screen)
        || crate::screen_tools::render::is_render_screen(screen)
        || crate::scrolling_capture::render::is_render_screen(screen)
        || crate::auto_capture::render::is_render_screen(screen)
}

/// Draws the dialog `name` at its window's size.
pub fn render(name: &str, o: &Options) -> Result<image::RgbaImage, String> {
    if let Some(db) = &o.db {
        crate::history::use_database(db)?;
    }
    // The auto capture window reads and writes the settings it's drawn with.
    let (mut d, mut settings) = if crate::auto_capture::render::is_render_screen(name) {
        let mut settings = crate::auto_capture::render::settings(o);
        let d = crate::auto_capture::render::render_dialog(name, o, &mut settings)?;
        (d, settings)
    } else {
        let mut settings = ApplicationConfig::default();
        if name == "icon-pack-prompt" && o.state.as_deref() == Some("legacy") {
            settings.theme_options().IconPack = sharex_settings::IconPack::Legacy;
        }
        (dialog(name, o)?, settings)
    };
    let spec = d.spec();
    let store = UploadersConfigStore::loaded(match name.strip_prefix("destination-") {
        Some(page) => super::destination_settings::render_config(page, o.state.as_deref()),
        None => mock_config(),
    });
    let mut height = spec.height;
    let mut width = spec.width;
    if spec.size_to_content {
        let mut probe = Offscreen::new([spec.width.max(1.0) as u32, 400], o.scale, o.dark)?;
        probe.run(Vec::new(), &mut |ui: &mut egui::Ui| {
            height = d.content_height(ui);
            if let Some(w) = d.content_width(ui) {
                width = w;
            }
        });
    }
    let w = o.width.unwrap_or((width * o.scale).round() as u32);
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

/// A finished check as the update windows' screens show it: ShareXrs
/// 22.4.0.0, ShareXrs's 22.5.0 release with its setup asset.
pub fn update_checker(kind: &str) -> sharex_update::GitHubUpdateChecker {
    use sharex_update::checker::{InstallFlow, UpdateStatus};
    use sharex_update::version::AppVersion;
    let flow = if kind == "portable" { InstallFlow::WindowsPortable } else { InstallFlow::WindowsSetup };
    let (owner, repo) = sharex_update::checker::repository();
    let mut c = sharex_update::GitHubUpdateChecker::new(owner, repo, flow);
    c.status = UpdateStatus::UpdateAvailable;
    c.current_version = AppVersion::parse("22.4.0.0").ok();
    c.latest_version = AppVersion::parse(if kind == "dev" { "22.4.0.1" } else { "22.5.0" }).ok();
    c.is_dev = kind == "dev";
    c.is_pre_release = kind == "prerelease";
    let name = if kind == "portable" { "ShareXrs-22.5.0-portable-x64.zip" } else { "ShareXrs-22.5.0-setup-x64.exe" };
    c.download_url = Some(format!("https://github.com/{owner}/{repo}/releases/download/v22.5.0/{name}"));
    c.set_file_name(Some(name.to_owned()));
    c
}
