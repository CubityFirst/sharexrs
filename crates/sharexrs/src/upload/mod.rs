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
use std::sync::{Arc, Mutex};

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
}

/// The desktop's clipboard (owned by the main window, which Windows needs)
/// and browser.
pub struct SystemUi {
    pub owner: Option<isize>,
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
        // `QRCodeWindow` (ShareX.Tools) isn't ported yet.
        eprintln!("QR code window not ported yet: {text}");
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
