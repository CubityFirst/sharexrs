//! `DownloaderWindow` (`ShareX.HelpersLib/Presentation/UpdateChecker/`)
//! without its window: the download on a worker thread, the texts and
//! progress the window shows, its one action button, and installing.
//!
//! The UI thread owns it: it calls [`opened`](DownloaderWindow::opened),
//! [`action_click`](DownloaderWindow::action_click),
//! [`closing`](DownloaderWindow::closing) and, every frame,
//! [`poll`](DownloaderWindow::poll), and shows the invalid signature
//! question when [`signature_prompt`](DownloaderWindow::signature_prompt)
//! is set.
//!
//! Differences from ShareX, on purpose:
//! - After a stop, ShareX's `FileDownloader` still reports success, so
//!   ShareX goes on to `Install()` on the deleted file (and asks about its
//!   signature) after its window has closed. ShareXrs stops there.
//! - ShareXrs checks the download against the release's `SHA256SUMS`
//!   before installing (required to update in place on macOS and Linux).
//! - The macOS and Linux installs ([`InstallFlow`]) are additions.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::Duration;

use sharex_strings::helpers_lib as s;
use sharex_uploaders::HttpMethod;
use sharex_uploaders::http::{self, HttpRequest, RequestOptions};

use crate::checker::{GitHubUpdateChecker, InstallFlow};
use crate::downloader::{DownloadEvent, DownloadProgress, FileDownloader};
use crate::install::{self, UpdateHost};
use crate::message::DialogResult;
use crate::text::{format_fixed, to_size_string};

/// `DownloaderWindowStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloaderWindowStatus {
    Waiting,
    DownloadStarted,
    DownloadCompleted,
    InstallStarted,
}

/// `InstallType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallType {
    Default,
    Silent,
    VerySilent,
    Event,
}

enum WorkerMessage {
    Event(DownloadEvent),
    Finished(Result<bool, String>),
}

/// `DownloaderWindow`'s state.
pub struct DownloaderWindow {
    /// `URL`.
    pub url: String,
    pub file_name: String,
    pub download_location: PathBuf,
    pub accept_header: String,
    pub auto_start_download: bool,
    pub install_type: InstallType,
    pub auto_start_install: bool,
    pub status: DownloaderWindowStatus,
    pub run_installer_in_background: bool,
    pub verify_installer_signature: bool,
    /// How the update is put in place (ShareX: always the setup exe).
    pub flow: InstallFlow,
    /// The release's `SHA256SUMS` URL (ShareXrs).
    pub checksums_url: Option<String>,
    /// `Title`.
    pub title: String,
    pub heading: String,
    pub description: String,
    /// `FileNameText`.
    pub file_name_text: String,
    /// `StatusText`.
    pub status_text: String,
    /// `ProgressText` (empty until the size is known).
    pub progress_text: String,
    /// `DownloadProgress.Value`, 0–100.
    pub progress_value: f64,
    /// `ActionButton.Content`.
    pub action_text: String,
    /// `ActionButton.IsEnabled`.
    pub action_enabled: bool,
    /// The invalid signature question is showing; answer with
    /// [`confirm_signature`](Self::confirm_signature).
    pub signature_prompt: bool,
    /// The window should close (`Close()`); the UI closes it.
    pub close_requested: bool,
    /// `_result`.
    pub result: DialogResult,
    /// ShareXrs should exit after the window closes (the installer, or the
    /// replaced program, starts it again).
    pub shutdown_after: bool,
    closed: bool,
    host: Arc<dyn UpdateHost>,
    downloader: Option<FileDownloader>,
    worker: Option<Receiver<WorkerMessage>>,
}

impl DownloaderWindow {
    /// `new DownloaderWindow(url, fileName)`: the download goes to
    /// `<temp>/ShareXrs/<fileName>` (ShareX: `<temp>/ShareX`).
    pub fn new(url: &str, file_name: &str, flow: InstallFlow, host: Arc<dyn UpdateHost>) -> Self {
        Self {
            url: url.to_owned(),
            file_name: file_name.to_owned(),
            download_location: PathBuf::new(),
            accept_header: String::new(),
            auto_start_download: true,
            install_type: InstallType::Silent,
            auto_start_install: true,
            status: DownloaderWindowStatus::Waiting,
            run_installer_in_background: true,
            verify_installer_signature: false,
            flow,
            checksums_url: None,
            title: s::DownloaderWindow_Title().to_owned(),
            heading: s::DownloaderWindow_Heading().to_owned(),
            description: s::DownloaderWindow_Description().to_owned(),
            file_name_text: s::DownloaderForm_DownloaderForm_Filename___0_().replace("{0}", file_name),
            status_text: status(s::DownloaderForm_DownloaderForm_Waiting_()),
            progress_text: String::new(),
            progress_value: 0.0,
            action_text: s::DownloaderWindow_Download().to_owned(),
            action_enabled: true,
            signature_prompt: false,
            close_requested: false,
            result: DialogResult::None,
            shutdown_after: false,
            closed: false,
            host,
            downloader: None,
            worker: None,
        }
    }

    /// `ShowAsync(UpdateChecker)`: GitHub's asset API needs
    /// `Accept: application/octet-stream`, and the installer's signature is checked.
    pub fn for_update(checker: &GitHubUpdateChecker, host: Arc<dyn UpdateHost>) -> Self {
        let mut window = Self::new(checker.download_url.as_deref().unwrap_or(""), &checker.file_name(), checker.flow.clone(), host);
        window.accept_header = "application/octet-stream".to_owned();
        window.verify_installer_signature = true;
        window.checksums_url = checker.checksums_url();
        window
    }

    /// The window has closed.
    pub fn is_closed(&self) -> bool {
        self.closed
    }

    /// `OnOpened`.
    pub fn opened(&mut self) {
        if self.auto_start_download {
            self.start_download();
        }
    }

    /// `OnActionClick`.
    pub fn action_click(&mut self) {
        match self.status {
            DownloaderWindowStatus::Waiting => self.start_download(),
            DownloaderWindowStatus::DownloadCompleted => self.install(),
            _ => {
                self.result = DialogResult::Cancel;
                self.close();
            }
        }
    }

    /// `OnClosing`: a running download stops.
    pub fn closing(&mut self) {
        if self.status == DownloaderWindowStatus::DownloadStarted
            && let Some(d) = &self.downloader
        {
            d.stop_download();
        }
        self.closed = true;
    }

    fn close(&mut self) {
        self.close_requested = true;
        self.closing();
    }

    /// The progress so far, for the window's progress bar.
    pub fn progress(&self) -> DownloadProgress {
        self.downloader.as_ref().map(FileDownloader::progress).unwrap_or_default()
    }

    /// `StartDownloadAsync`.
    fn start_download(&mut self) {
        if self.url.is_empty() || self.status != DownloaderWindowStatus::Waiting {
            return;
        }
        self.status = DownloaderWindowStatus::DownloadStarted;
        self.action_text = s::DownloaderForm_StartDownload_Cancel().to_owned();
        self.progress_value = 0.0;
        let folder = std::env::temp_dir().join("ShareXrs");
        let _ = std::fs::create_dir_all(&folder);
        self.download_location = folder.join(&self.file_name);
        eprintln!("Downloading: \"{}\" -> \"{}\"", self.url, self.download_location.display());

        let mut downloader = FileDownloader::new(&self.url, &self.download_location);
        downloader.accept_header = self.accept_header.clone();
        self.change_status(s::DownloaderForm_StartDownload_Getting_file_size_());

        let (tx, rx) = mpsc::channel();
        let worker = downloader.clone();
        let checksums = self.checksums_url.clone().map(|url| (url, self.accept_header.clone()));
        let flow = self.flow.clone();
        let file_name = self.file_name.clone();
        std::thread::Builder::new()
            .name("update-download".into())
            .spawn(move || {
                let events_tx = tx.clone();
                let events = move |e: DownloadEvent| {
                    let _ = events_tx.send(WorkerMessage::Event(e));
                };
                let mut result = worker.start_download(&events);
                if matches!(result, Ok(true))
                    && !worker.is_canceled()
                    && let Err(e) = verify_checksum(&worker.download_location, &file_name, checksums, &flow)
                {
                    let _ = std::fs::remove_file(&worker.download_location);
                    result = Err(e);
                }
                let _ = tx.send(WorkerMessage::Finished(result));
            })
            .expect("download thread");
        self.downloader = Some(downloader);
        self.worker = Some(rx);
    }

    /// Takes in what the download thread reported; `true` when something
    /// changed (the UI repaints).
    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        loop {
            let message = match self.worker.as_ref().map(Receiver::try_recv) {
                Some(Ok(m)) => m,
                Some(Err(TryRecvError::Disconnected)) => {
                    self.worker = None;
                    break;
                }
                _ => break,
            };
            changed = true;
            match message {
                WorkerMessage::Event(DownloadEvent::FileSizeReceived) => {
                    self.change_status(s::DownloaderForm_StartDownload_Downloading_());
                    self.update_progress();
                }
                WorkerMessage::Event(DownloadEvent::ProgressChanged) => self.update_progress(),
                WorkerMessage::Finished(result) => {
                    self.worker = None;
                    self.download_finished(result);
                    break;
                }
            }
        }
        changed
    }

    /// The rest of `StartDownloadAsync`, once `StartDownload` returns.
    fn download_finished(&mut self, result: Result<bool, String>) {
        match result {
            Ok(false) => {}
            // ShareX carries on into `Install()` after its window closed.
            Ok(true) if self.closed => {}
            Ok(true) => {
                self.change_status(s::DownloaderForm_fileDownloader_DownloadCompleted_Download_completed_());
                self.status = DownloaderWindowStatus::DownloadCompleted;
                self.action_text = s::DownloaderForm_fileDownloader_DownloadCompleted_Install().to_owned();
                if self.auto_start_install {
                    self.install();
                }
            }
            Err(e) => self.change_status(&e),
        }
    }

    /// `UpdateProgress`.
    fn update_progress(&mut self) {
        let Some(downloader) = &self.downloader else { return };
        let p = downloader.progress();
        self.progress_value = p.percentage();
        self.progress_text = progress_text(&p);
    }

    /// `ChangeStatus`.
    fn change_status(&mut self, text: &str) {
        self.status_text = status(text);
    }

    /// `Install`.
    pub fn install(&mut self) {
        if self.status != DownloaderWindowStatus::DownloadCompleted {
            return;
        }
        if matches!(self.flow, InstallFlow::WindowsSetup)
            && self.verify_installer_signature
            && !install::is_trusted(&self.download_location)
        {
            // `MessageBox.Show(..., YesNo, Warning, Button2)`.
            self.signature_prompt = true;
            return;
        }
        self.install_trusted();
    }

    /// The answer to the invalid signature question.
    pub fn confirm_signature(&mut self, yes: bool) {
        self.signature_prompt = false;
        if yes {
            self.install_trusted();
        } else {
            self.result = DialogResult::Cancel;
            self.close();
        }
    }

    fn install_trusted(&mut self) {
        let path = self.download_location.clone();
        let outcome = match &self.flow {
            InstallFlow::MacApp { app } => install::replace_app(app, &path).map(|()| Some(app.join("Contents/MacOS/sharexrs"))),
            InstallFlow::LinuxTarball { folder } => {
                install::replace_tarball(folder, &path).map(|()| Some(folder.join("sharexrs")))
            }
            InstallFlow::LinuxDeb => self.host.open_file(&path).map(|()| None),
            _ => Ok(None),
        };
        let restart = match outcome {
            Ok(restart) => restart,
            Err(e) => {
                // The download is gone (or unusable): start again from Download.
                self.change_status(&e);
                self.status = DownloaderWindowStatus::Waiting;
                self.action_text = s::DownloaderWindow_Download().to_owned();
                return;
            }
        };
        self.status = DownloaderWindowStatus::InstallStarted;
        self.result = DialogResult::OK;
        self.action_enabled = false;
        self.shutdown_after = !matches!(self.flow, InstallFlow::LinuxDeb);
        match &self.flow {
            InstallFlow::WindowsSetup | InstallFlow::WindowsPortable => self.run_installer_with_delay(1000),
            _ => {
                if let Some(exe) = restart {
                    let host = self.host.clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(Duration::from_millis(1000));
                        if let Err(e) = host.restart(&exe) {
                            eprintln!("update: {e}");
                        }
                    });
                }
            }
        }
        self.close();
    }

    /// `RunInstallerWithDelay`: on a foreground thread, so ShareXrs's exit
    /// doesn't abandon it.
    fn run_installer_with_delay(&mut self, delay_ms: u64) {
        let (host, path, arguments) = (self.host.clone(), self.download_location.clone(), self.installer_arguments());
        let elevate = install::is_default_install_dir() && !install::is_member_of_administrators_group();
        if self.install_type == InstallType::Event {
            return;
        }
        if self.run_installer_in_background {
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(delay_ms));
                // `RunInstaller` swallows errors.
                let _ = host.run_installer(&path, &arguments, elevate);
            });
        } else {
            let _ = host.run_installer(&path, &arguments, elevate);
        }
    }

    /// `RunInstaller`'s arguments: `/UPDATE`, then `/SILENT` or `/VERYSILENT`.
    pub fn installer_arguments(&self) -> String {
        match self.install_type {
            InstallType::Silent => "/UPDATE /SILENT".to_owned(),
            InstallType::VerySilent => "/UPDATE /VERYSILENT".to_owned(),
            _ => "/UPDATE".to_owned(),
        }
    }

    /// The question asked about an unsigned installer.
    pub fn signature_question() -> (&'static str, &'static str) {
        (s::DownloaderWindow_InvalidSignatureWarning(), s::DownloaderWindow_InvalidSignatureTitle())
    }
}

/// `"Status: {0}"`.
fn status(text: &str) -> String {
    s::DownloaderForm_ChangeStatus_Status___0_().replace("{0}", text)
}

/// `UpdateProgress`'s text (a verbatim string with CRLF line breaks, as
/// ShareX's source is checked out on Windows).
pub fn progress_text(p: &DownloadProgress) -> String {
    format!(
        "{}: {}%\r\n{}: {}/s\r\n{}: {} / {}",
        s::DownloaderForm_FileDownloader_ProgressChanged_Progress(),
        format_fixed(p.percentage(), 1),
        s::DownloaderForm_FileDownloader_ProgressChanged_DownloadSpeed(),
        to_size_string(p.download_speed as i64),
        s::DownloaderForm_FileDownloader_ProgressChanged_FileSize(),
        to_size_string(p.downloaded_size),
        to_size_string(p.file_size),
    )
}

/// Checks a download against the release's `SHA256SUMS` (ShareXrs).
/// Updating in place (macOS app, Linux tarball) needs a listed checksum;
/// the Windows installer and the .deb are checked when one is listed.
fn verify_checksum(
    path: &Path,
    file_name: &str,
    checksums: Option<(String, String)>,
    flow: &InstallFlow,
) -> Result<(), String> {
    let required = needs_checksums(flow);
    let Some((url, accept)) = checksums else {
        return if required { Err("the release has no SHA256SUMS to check the download with".to_owned()) } else { Ok(()) };
    };
    let mut headers = Vec::new();
    if !accept.is_empty() {
        headers.push(("Accept".to_owned(), accept));
    }
    let request = HttpRequest { method: HttpMethod::GET, url, headers, content: None };
    let response = http::send(request, RequestOptions::default()).map_err(|e| e.to_string())?;
    if !response.is_success() {
        return Err(format!(
            "Response status code does not indicate success: {} ({}).",
            response.status_code, response.status_description
        ));
    }
    let sums = response.response_text.unwrap_or_default();
    match install::checksum_for(&sums, file_name) {
        None if required => Err(format!("SHA256SUMS doesn't list {file_name}")),
        None => Ok(()),
        Some(expected) => {
            let actual = install::sha256_file(path).map_err(|e| e.to_string())?;
            if actual == expected { Ok(()) } else { Err(format!("{file_name} doesn't match its checksum")) }
        }
    }
}

/// Whether an in-place update can go ahead without `SHA256SUMS`.
pub fn needs_checksums(flow: &InstallFlow) -> bool {
    matches!(flow, InstallFlow::MacApp { .. } | InstallFlow::LinuxTarball { .. })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_text_as_sharex() {
        let p = DownloadProgress { file_size: 52_428_800, downloaded_size: 13_107_200, download_speed: 1_536_000.7 };
        assert_eq!(
            progress_text(&p),
            "Progress: 25.0%\r\nDownload speed: 1.54 MB/s\r\nFile size: 13.11 MB / 52.43 MB"
        );
    }
}
