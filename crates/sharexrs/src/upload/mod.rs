//! Uploads as tasks, as ShareX runs them: the upload half of
//! `ShareX/WorkerTask.cs` ([`worker`]), `UploadManager.cs` ([`manager`]),
//! the after upload tasks with `UploadInfoParser` ([`after_upload`]),
//! `UploaderFilter` ([`filters`]) and `SettingManager`'s handling of
//! `UploadersConfig.json` ([`config_store`]).
//!
//! The uploaders themselves are `sharex-uploaders`. What touches the
//! desktop (the clipboard, opening a URL, the QR code window) goes through
//! [`TaskUi`], so tests record it instead.

pub mod after_upload;
pub mod app;
pub mod config_store;
pub mod filters;
pub mod manager;
pub mod worker;

#[cfg(test)]
mod tests;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};

use sharex_settings::TaskSettings;

use sharex_uploaders::http::CancellationToken;

pub use config_store::UploadersConfigStore;

use crate::tasks::TaskEvent;

/// Where a task's thread reports to (the event loop, or a test).
pub type EventSink = Arc<dyn Fn(TaskEvent) + Send + Sync>;

/// What a task does on the desktop, as `ClipboardHelpers` and
/// `URLHelpers.OpenURL` do in ShareX.
pub trait TaskUi: Send + Sync {
    /// `ClipboardHelpers.CopyText`.
    fn copy_text(&self, text: &str);
    /// `ClipboardHelpers.ContainsText`.
    fn clipboard_contains_text(&self) -> bool;
    /// `ClipboardHelpers.Clear`.
    fn clear_clipboard(&self);
    /// `URLHelpers.OpenURL`.
    fn open_url(&self, url: &str);
    /// `TaskHelpers.OpenQRCode(text)`: the QR code window.
    fn show_qr_code(&self, text: &str);
    /// `ClipboardHelpers.CopyFile`: the file as a file drop list.
    fn copy_file(&self, path: &std::path::Path);
    /// `FileHelpers.OpenFolderWithFile`: its folder, with it selected.
    fn open_folder_with_file(&self, path: &std::path::Path);
    /// `TaskHelpers.OpenQRCodeScanFromImageFile`: the QR code window,
    /// scanning the image.
    fn scan_qr_code_file(&self, _path: &std::path::Path) {}
    /// `TaskHelpers.AnalyzeImage(filePath, taskSettings)`: the Analyze
    /// image window, which isn't ported yet.
    fn analyze_image(&self, path: &std::path::Path) {
        eprintln!("Analyze image window not ported yet: {}", path.display());
    }
    /// `LargeFileUploadWarningWindowIntegration.Show()`: (continue, don't
    /// show again). The task's thread waits for the answer.
    fn large_file_warning(&self) -> (bool, bool) {
        (true, false)
    }
    /// `BeforeUploadWindowIntegration.Show(info)`: the task settings with
    /// the destination the window left chosen, or `None` when cancelled.
    /// The task's thread waits for the answer.
    fn before_upload(&self, request: BeforeUpload) -> Option<TaskSettings> {
        Some(request.task)
    }
    /// `ToolsIntegration.ShowOCRWindowAsync` from a task (`WorkerTask.DoOCR`
    /// waits for the window to close).
    fn show_ocr_window(&self, image: Arc<image::RgbaImage>, task: TaskSettings) {
        let _ = (image, task);
    }
    /// `ImageEffectsIntegration.ShowDialog(bmp, presets, selected, Editor)`
    /// (`ShowImageEffectsWindowAfterCapture`): the presets as the window
    /// left them and the selected index. The task's thread waits.
    fn image_effects_window(&self, image: &image::RgbaImage, task: &TaskSettings) -> Option<(Vec<serde_json::Value>, i32)> {
        let _ = (image, task);
        None
    }
}

/// What the before upload window shows of a task (`TaskInfo`).
pub struct BeforeUpload {
    pub data_type: crate::tasks::EDataType,
    pub file_name: String,
    pub file_path: Option<std::path::PathBuf>,
    /// `Info.Metadata.Image`.
    pub image: Option<Arc<image::RgbaImage>>,
    pub task: TaskSettings,
}

/// What a task's thread asks the event loop to show.
pub enum UiRequest {
    LargeFileWarning(mpsc::Sender<(bool, bool)>),
    BeforeUpload(Box<BeforeUpload>, mpsc::Sender<Option<TaskSettings>>),
    QrCode(String),
    /// The OCR window on a task's image; answered when it closes.
    Ocr(Arc<image::RgbaImage>, Box<TaskSettings>, mpsc::Sender<()>),
    /// `OpenQRCodeScanFromImageFile`.
    QrCodeScanFile(std::path::PathBuf),
    /// The Image effects window (Editor mode) on a capture; answered with
    /// the presets and the selected index when it closes.
    ImageEffects(Box<image::RgbaImage>, Box<TaskSettings>, mpsc::Sender<(Vec<serde_json::Value>, i32)>),
}

impl std::fmt::Debug for UiRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LargeFileWarning(_) => f.write_str("LargeFileWarning"),
            Self::BeforeUpload(r, _) => write!(f, "BeforeUpload({})", r.file_name),
            Self::QrCode(t) => write!(f, "QrCode({t})"),
            Self::Ocr(..) => f.write_str("Ocr"),
            Self::QrCodeScanFile(p) => write!(f, "QrCodeScanFile({})", p.display()),
            Self::ImageEffects(..) => f.write_str("ImageEffects"),
        }
    }
}

/// Where a task's thread sends what it asks the UI.
pub type RequestSink = Arc<dyn Fn(UiRequest) + Send + Sync>;

/// The desktop's clipboard (owned by the main window, which Windows needs)
/// and browser.
pub struct SystemUi {
    pub owner: Option<isize>,
    /// The event loop, for the windows a task opens.
    pub requests: Option<RequestSink>,
}

impl TaskUi for SystemUi {
    fn copy_text(&self, text: &str) {
        if let Err(e) = sharex_capture::clipboard::copy_text(text, self.owner) {
            eprintln!("Clipboard copy text failed: {e}");
        }
    }

    fn clipboard_contains_text(&self) -> bool {
        sharex_capture::clipboard::contains_text()
    }

    fn clear_clipboard(&self) {
        if let Err(e) = sharex_capture::clipboard::clear(self.owner) {
            eprintln!("Clipboard clear failed: {e}");
        }
    }

    fn open_url(&self, url: &str) {
        open_url(url);
    }

    fn show_qr_code(&self, text: &str) {
        if let Some(r) = &self.requests {
            r(UiRequest::QrCode(text.to_owned()));
        }
    }

    fn copy_file(&self, path: &std::path::Path) {
        if let Err(e) = sharex_capture::clipboard::copy_files(&[path.to_path_buf()], self.owner) {
            eprintln!("Clipboard copy file failed: {e}");
        }
    }

    fn open_folder_with_file(&self, path: &std::path::Path) {
        crate::shell::open_folder_with_file(path);
    }

    fn scan_qr_code_file(&self, path: &std::path::Path) {
        if let Some(r) = &self.requests {
            r(UiRequest::QrCodeScanFile(path.to_path_buf()));
        }
    }

    fn large_file_warning(&self) -> (bool, bool) {
        let Some(r) = &self.requests else { return (true, false) };
        let (tx, rx) = mpsc::channel();
        r(UiRequest::LargeFileWarning(tx));
        // A window closed without answering is Cancel.
        rx.recv().unwrap_or((false, false))
    }

    fn show_ocr_window(&self, image: Arc<image::RgbaImage>, task: TaskSettings) {
        let Some(r) = &self.requests else { return };
        let (tx, rx) = mpsc::channel();
        r(UiRequest::Ocr(image, Box::new(task), tx));
        let _ = rx.recv();
    }

    fn image_effects_window(&self, image: &image::RgbaImage, task: &TaskSettings) -> Option<(Vec<serde_json::Value>, i32)> {
        let r = self.requests.as_ref()?;
        let (tx, rx) = mpsc::channel();
        r(UiRequest::ImageEffects(Box::new(image.clone()), Box::new(task.clone()), tx));
        rx.recv().ok()
    }

    fn before_upload(&self, request: BeforeUpload) -> Option<TaskSettings> {
        let Some(r) = &self.requests else { return Some(request.task) };
        let (tx, rx) = mpsc::channel();
        r(UiRequest::BeforeUpload(Box::new(request), tx));
        rx.recv().ok().flatten()
    }
}

/// `URLHelpers.OpenURL`: a valid URL in the browser; anything else is
/// "Invalid URL" (ShareX says so in a message box).
pub fn open_url(url: &str) {
    if url.is_empty() {
        return;
    }
    if !sharex_uploaders::url_helpers::is_valid_url(url) {
        eprintln!("{} {url}", sharex_strings::helpers_lib::URLHelpers_Invalid_URL());
        return;
    }
    if crate::shell::open_url(url) {
        eprintln!("URL opened: {url}");
    }
}

/// `WorkerTask.Stop` as it reaches a task's thread: the request (read as
/// `StopRequested`) and the running uploader's `StopUpload`.
#[derive(Clone, Default)]
pub struct StopHandle(Arc<StopInner>);

#[derive(Default)]
struct StopInner {
    requested: AtomicBool,
    uploader: Mutex<Option<CancellationToken>>,
}

impl std::fmt::Debug for StopHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("StopHandle").field(&self.is_requested()).finish()
    }
}

impl StopHandle {
    /// `Stop()`: `StopRequested = true`, and `uploader.StopUpload()`.
    pub fn stop(&self) {
        self.0.requested.store(true, Ordering::SeqCst);
        if let Some(token) = self.0.uploader.lock().expect("stop handle").as_ref() {
            token.cancel();
        }
    }

    /// `StopRequested`.
    pub fn is_requested(&self) -> bool {
        self.0.requested.load(Ordering::SeqCst)
    }

    /// The uploader that's running now (`WorkerTask.uploader`).
    fn set_uploader(&self, token: Option<CancellationToken>) {
        let mut slot = self.0.uploader.lock().expect("stop handle");
        *slot = token;
    }
}
