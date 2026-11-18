//! `FileDownloader` (`ShareX.HelpersLib/FileDownloader.cs`): a GET written
//! to a file as it arrives, with the size from `Content-Length`, the speed
//! every half second and progress events every 100 ms; stopping deletes the
//! partial file.

use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use sharex_uploaders::HttpMethod;
use sharex_uploaders::http::{self, HttpError, HttpRequest, RequestOptions};

/// `FileDownloader`'s progress members.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DownloadProgress {
    /// `FileSize`: -1 until the response says.
    pub file_size: i64,
    pub downloaded_size: i64,
    /// `DownloadSpeed`, bytes per second.
    pub download_speed: f64,
}

impl Default for DownloadProgress {
    fn default() -> Self {
        Self { file_size: -1, downloaded_size: 0, download_speed: 0.0 }
    }
}

impl DownloadProgress {
    /// `DownloadPercentage`.
    pub fn percentage(&self) -> f64 {
        if self.file_size > 0 { self.downloaded_size as f64 / self.file_size as f64 * 100.0 } else { 0.0 }
    }
}

/// `FileSizeReceived` and `ProgressChanged`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadEvent {
    FileSizeReceived,
    ProgressChanged,
}

/// `bufferSize`.
const BUFFER_SIZE: usize = 32768;

/// `FileDownloader`.
#[derive(Clone)]
pub struct FileDownloader {
    pub url: String,
    pub download_location: PathBuf,
    /// `AcceptHeader` (`application/octet-stream` for GitHub's asset API).
    pub accept_header: String,
    progress: Arc<Mutex<DownloadProgress>>,
    is_downloading: Arc<AtomicBool>,
    is_canceled: Arc<AtomicBool>,
}

impl FileDownloader {
    pub fn new(url: &str, download_location: &Path) -> Self {
        Self {
            url: url.to_owned(),
            download_location: download_location.to_path_buf(),
            accept_header: String::new(),
            progress: Arc::default(),
            is_downloading: Arc::default(),
            is_canceled: Arc::default(),
        }
    }

    pub fn progress(&self) -> DownloadProgress {
        *self.progress.lock().expect("progress")
    }

    pub fn is_downloading(&self) -> bool {
        self.is_downloading.load(Ordering::SeqCst)
    }

    pub fn is_canceled(&self) -> bool {
        self.is_canceled.load(Ordering::SeqCst)
    }

    /// `StopDownload`: the loop stops before its next read.
    pub fn stop_download(&self) {
        self.is_canceled.store(true, Ordering::SeqCst);
    }

    /// `StartDownload`: `Ok(true)` once the loop ends (also when it was
    /// stopped, as ShareX's does), `Ok(false)` when it didn't start or the
    /// response has no length, `Err` for an exception (not after a stop).
    pub fn start_download(&self, events: &(dyn Fn(DownloadEvent) + Send + Sync)) -> Result<bool, String> {
        if self.is_downloading() || self.url.is_empty() {
            return Ok(false);
        }
        self.is_downloading.store(true, Ordering::SeqCst);
        self.is_canceled.store(false, Ordering::SeqCst);
        *self.progress.lock().expect("progress") = DownloadProgress::default();
        let result = self.do_work(events);
        let result = match result {
            Err(e) if self.is_canceled() => {
                let _ = e;
                Ok(false)
            }
            other => other,
        };
        if self.is_canceled() && self.download_location.exists() {
            let _ = std::fs::remove_file(&self.download_location);
        }
        self.is_downloading.store(false, Ordering::SeqCst);
        result
    }

    fn do_work(&self, events: &(dyn Fn(DownloadEvent) + Send + Sync)) -> Result<bool, String> {
        let mut headers = Vec::new();
        if !self.accept_header.is_empty() {
            headers.push(("Accept".to_owned(), self.accept_header.clone()));
        }
        let request = HttpRequest { method: HttpMethod::GET, url: self.url.clone(), headers, content: None };
        // ShareX's request has no cancellation token: a stop is only seen
        // between reads. No overall timeout: with `ResponseHeadersRead`,
        // `HttpClient.Timeout` doesn't cover reading the body.
        let options = RequestOptions { read_body: false, ..Default::default() };
        http::send_streaming(request, options, |response, body| {
            // `EnsureSuccessStatusCode`.
            if !response.is_success() {
                return Err(HttpError::Failed(format!(
                    "Response status code does not indicate success: {} ({}).",
                    response.status_code, response.status_description
                )));
            }
            let file_size: i64 = response.header("Content-Length").and_then(|v| v.trim().parse().ok()).unwrap_or(-1);
            self.progress.lock().expect("progress").file_size = file_size;
            events(DownloadEvent::FileSizeReceived);
            if file_size <= 0 {
                return Ok(false);
            }
            self.read_to_file(body, file_size, events).map(|()| true).map_err(HttpError::Failed)
        })
        .map_err(|e| e.to_string())
    }

    fn read_to_file(&self, body: &mut dyn Read, file_size: i64, events: &(dyn Fn(DownloadEvent) + Send + Sync)) -> Result<(), String> {
        let mut buffer = vec![0u8; BUFFER_SIZE.min(file_size as usize)];
        let mut file = File::create(&self.download_location).map_err(|e| e.to_string())?;
        // Stopwatches: `timer` for the speed, `progress_timer` for events.
        let mut timer: Option<Instant> = None;
        let mut progress_timer: Option<Instant> = None;
        let mut speed_test: i64 = 0;
        loop {
            let downloaded = self.progress().downloaded_size;
            if downloaded >= file_size || self.is_canceled() {
                break;
            }
            let timer_start = *timer.get_or_insert_with(Instant::now);
            let progress_start = *progress_timer.get_or_insert_with(Instant::now);
            let bytes_read = body.read(&mut buffer).map_err(|e| e.to_string())?;
            if bytes_read == 0 {
                // .NET's content stream throws when the body ends before
                // `Content-Length`.
                return Err("The response ended prematurely. (ResponseEnded)".to_owned());
            }
            file.write_all(&buffer[..bytes_read]).map_err(|e| e.to_string())?;
            speed_test += bytes_read as i64;
            {
                let mut p = self.progress.lock().expect("progress");
                p.downloaded_size += bytes_read as i64;
                let elapsed = timer_start.elapsed().as_millis() as i64;
                if elapsed > 500 {
                    p.download_speed = speed_test as f64 / elapsed as f64 * 1000.0;
                    speed_test = 0;
                    timer = None;
                }
            }
            if progress_start.elapsed().as_millis() > 100 {
                events(DownloadEvent::ProgressChanged);
                progress_timer = None;
            }
        }
        events(DownloadEvent::ProgressChanged);
        file.flush().map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentage() {
        let p = DownloadProgress { file_size: 200, downloaded_size: 50, download_speed: 0.0 };
        assert_eq!(p.percentage(), 25.0);
        assert_eq!(DownloadProgress::default().percentage(), 0.0);
    }
}
