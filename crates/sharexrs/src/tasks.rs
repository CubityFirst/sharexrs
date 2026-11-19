//! What runs behind the main window's task area: `ShareX/WorkerTask.cs`,
//! `TaskInfo.cs`, `TaskManager.cs`, `RecentTaskManager.cs` and
//! `UploadInfoStatus.cs`: image tasks from a capture (after capture tasks)
//! and upload tasks (`crate::upload`).
//!
//! A task's work runs on its own thread, as ShareX's `ThreadWorker`; what
//! it reports comes back to the event loop as a [`TaskEvent`].

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use sharex_helpers::file_helpers::{is_image_file, is_text_file, is_video_file};
use sharex_strings::sharex as s;

/// `ShareX.TaskStatus`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskStatus {
    InQueue,
    Preparing,
    Working,
    Stopping,
    Stopped,
    Failed,
    Completed,
    History,
}

/// `ShareX.TaskJob`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TaskJob {
    #[default]
    Job,
    /// `CreateDataUploaderTask` (the browser extension's image uploads).
    #[allow(dead_code)]
    DataUpload,
    FileUpload,
    TextUpload,
    ShortenURL,
    ShareURL,
    Download,
    DownloadUpload,
}

/// `ShareX.HelpersLib.EDataType`.
#[allow(clippy::upper_case_acronyms)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EDataType {
    #[default]
    Default,
    File,
    Image,
    Text,
    URL,
}

/// What `ProgressManager` says at a report (`TaskInfo.Progress`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct UploadProgress {
    pub position: u64,
    pub length: u64,
    /// Bytes per second.
    pub speed: f64,
    pub elapsed: Duration,
    pub remaining: Duration,
}

impl UploadProgress {
    /// `Percentage`.
    pub fn percentage(&self) -> f64 {
        self.position as f64 / self.length as f64 * 100.0
    }
}

/// `UploadResult`: what an upload returned (or what a recent task brings
/// back), with its errors' texts.
#[derive(Clone, Debug, Default)]
pub struct UploadResult {
    pub url: String,
    pub thumbnail_url: String,
    pub deletion_url: String,
    pub shortened_url: String,
    pub response: String,
    pub errors: Vec<String>,
    /// The first error's title (`UploaderErrorInfo.Title`: "<service> error").
    pub error_title: String,
    /// `IsURLExpected`: false for tasks that don't upload (ShareX sets it so
    /// in `ThreadDoWork`), so their errors fail them whatever their URL.
    pub url_expected: bool,
    /// `ResponseInfo` of the last request (the response window shows it).
    pub response_info: Option<Box<sharex_uploaders::http::ResponseInfo>>,
}

impl UploadResult {
    /// `IsError`.
    pub fn is_error(&self) -> bool {
        !self.errors.is_empty() && (!self.url_expected || self.url.is_empty())
    }

    /// `UploadResult.ToString`: the shortened URL, else the URL.
    pub fn text(&self) -> &str {
        if !self.shortened_url.is_empty() { &self.shortened_url } else { &self.url }
    }
}

/// `TaskInfo`.
#[derive(Clone, Debug, Default)]
pub struct TaskInfo {
    pub status: String,
    pub file_path: Option<PathBuf>,
    pub file_name: String,
    pub thumbnail_file_path: Option<PathBuf>,
    pub start_time: Option<SystemTime>,
    pub end_time: Option<SystemTime>,
    pub result: UploadResult,
    pub job: TaskJob,
    /// `Progress`: the last report of an upload.
    pub progress: Option<UploadProgress>,
    /// `IsUploadJob`, uploaded as a file (`UploadDestination`) to Amazon S3
    /// (`GetFileDestinationByDataType`): `CanDeleteAmazonS3Object`'s task half.
    pub amazon_s3_file: bool,
}

impl TaskInfo {
    /// `FilePath`'s setter also sets `FileName`.
    pub fn set_file_path(&mut self, path: Option<PathBuf>) {
        self.file_name = path
            .as_deref()
            .and_then(Path::file_name)
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.file_path = path;
    }

    pub fn file_path_str(&self) -> String {
        self.file_path.as_deref().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default()
    }

    /// `TaskInfo.ToString`: the result's URL, else the file path.
    pub fn text(&self) -> String {
        let url = self.result.text();
        if url.is_empty() { self.file_path_str() } else { url.to_string() }
    }
}

/// `WorkerTask`.
#[derive(Clone, Debug)]
pub struct WorkerTask {
    pub id: u64,
    pub status: TaskStatus,
    pub info: TaskInfo,
    pub stop_requested: bool,
    /// Stops the task's thread (`WorkerTask.Stop` reaching its uploader).
    pub stop: Option<crate::upload::StopHandle>,
    /// The list view's transfer columns (progress, speed, elapsed,
    /// remaining), which keep their last values once a task finishes
    /// (`ThumbnailItemViewModel.RefreshListColumns`).
    pub transfer: [String; 4],
}

impl WorkerTask {
    /// `WorkerTask.IsWorking`.
    pub fn is_working(&self) -> bool {
        matches!(self.status, TaskStatus::Preparing | TaskStatus::Working | TaskStatus::Stopping)
    }

    /// `WorkerTask.IsBusy`.
    pub fn is_busy(&self) -> bool {
        self.status == TaskStatus::InQueue || self.is_working()
    }
}

/// `RecentTask`: what's kept of a finished task (`ApplicationConfig.RecentTasks`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RecentTask {
    pub file_path: String,
    pub url: String,
    pub thumbnail_url: String,
    pub deletion_url: String,
    pub shortened_url: String,
    /// `Time`, as Json.NET wrote it.
    pub time: String,
}

impl RecentTask {
    /// As ShareX writes it in `ApplicationConfig.json`.
    pub fn to_json(&self) -> serde_json::Value {
        let opt = |s: &str| if s.is_empty() { serde_json::Value::Null } else { serde_json::Value::String(s.to_string()) };
        serde_json::json!({
            "FilePath": opt(&self.file_path),
            "URL": opt(&self.url),
            "ThumbnailURL": opt(&self.thumbnail_url),
            "DeletionURL": opt(&self.deletion_url),
            "ShortenedURL": opt(&self.shortened_url),
            "Time": if self.time.is_empty() { now_json() } else { self.time.clone() },
        })
    }

    /// `ToString()`: the shortened URL, the URL or the file path.
    pub fn text(&self) -> &str {
        [&self.shortened_url, &self.url, &self.file_path].into_iter().find(|s| !s.is_empty()).map_or("", |s| s.as_str())
    }

    /// `TrayMenuText`: `[HH:mm:ss] ` and the text, its last 47 characters
    /// after "..." when longer than 50 (`Truncate(50, "...", false)`).
    pub fn tray_menu_text(&self) -> String {
        let text = self.text();
        let chars: Vec<char> = text.chars().collect();
        let text = if chars.len() > 50 { format!("...{}", chars[chars.len() - 47..].iter().collect::<String>()) } else { text.to_owned() };
        let time = chrono::DateTime::parse_from_rfc3339(&self.time)
            .map(|t| t.with_timezone(&chrono::Local).format("%H:%M:%S").to_string())
            .unwrap_or_else(|_| chrono::NaiveDateTime::parse_from_str(&self.time, "%Y-%m-%dT%H:%M:%S%.f").map(|t| t.format("%H:%M:%S").to_string()).unwrap_or_default());
        format!("[{time}] {text}")
    }

    /// `MainMenuBuilder.GetRecentTaskIcon`.
    pub fn icon(&self) -> &'static str {
        use sharex_helpers::file_helpers as f;
        use sharex_ui::keys::app_icons as icon;
        let path = self.file_path.as_str();
        if !self.shortened_url.is_empty() || !self.url.is_empty() {
            icon::Link
        } else if f::is_video_file(path) {
            icon::FileVideo
        } else if f::is_text_file(path) {
            icon::FileText
        } else if f::is_image_file(path) {
            icon::FileImage
        } else {
            icon::File
        }
    }

    pub fn from_json(v: &serde_json::Value) -> Self {
        let get = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or_default().to_string();
        Self {
            file_path: get("FilePath"),
            url: get("URL"),
            thumbnail_url: get("ThumbnailURL"),
            deletion_url: get("DeletionURL"),
            shortened_url: get("ShortenedURL"),
            time: get("Time"),
        }
    }
}

/// `DateTime.Now` as Json.NET writes it (local time with its offset).
pub fn now_json() -> String {
    chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::AutoSi, false)
}

/// What a task's thread (or the thumbnail loader) tells the event loop.
#[derive(Debug)]
pub enum TaskEvent {
    /// `StatusChanged`: preparing, uploading, ...
    Status { id: u64, status: TaskStatus, text: String },
    /// The task's file changed (`Info.FileName`/`FilePath`: the image was
    /// encoded and saved, the text saved, the file downloaded).
    File { id: u64, file_name: String, file_path: Option<PathBuf> },
    /// `UploadStarted`.
    UploadStarted { id: u64 },
    /// `UploadProgressChanged`.
    Progress { id: u64, progress: UploadProgress },
    /// `TaskCompleted` of a task run by `crate::upload`.
    Finished { id: u64, report: Box<TaskReport> },
    /// `UploadersConfigWindowRequested`: a destination's settings are
    /// missing (its service name).
    DestinationSettingsRequested { service: String },
    /// "Don't show this message again" in the large file warning:
    /// `ShowLargeFileSizeWarning = 0`.
    LargeFileWarningOff,
    /// `ImageReady`: the task's image after its after capture tasks, for
    /// its thumbnail while it's still working (`RefreshFromOwnedImage`).
    ImageReady { id: u64, image: std::sync::Arc<image::RgbaImage> },
    /// A thumbnail or preview was decoded (`RefreshThumbnailAsync`).
    Image { id: u64, version: u64, kind: ImageKind, image: Option<image::RgbaImage> },
}

/// What a finished task hands back to its `WorkerTask` on the UI thread.
#[derive(Debug, Clone, Default)]
pub struct TaskReport {
    pub file_name: String,
    pub file_path: Option<PathBuf>,
    pub result: UploadResult,
    /// `StopRequested`.
    pub stop_requested: bool,
    /// `IsUploadJob`.
    pub is_upload_job: bool,
    /// [`TaskInfo::amazon_s3_file`].
    pub amazon_s3_file: bool,
    /// The task's `TaskSettings` as `Task_TaskCompleted` reads them.
    pub play_sound: bool,
    /// `AfterUploadJob` has `ShowAfterUploadWindow`.
    pub show_after_upload_window: bool,
    /// `Info.DataType`.
    pub data_type: EDataType,
    /// `Info.Metadata.Image`, kept for the after upload window.
    pub image: Option<std::sync::Arc<image::RgbaImage>>,
    /// `AdvancedSettings.AutoCloseAfterUploadForm`.
    pub auto_close_after_upload_form: bool,
    /// `ShowToastNotificationAfterTaskCompleted`.
    pub show_toast: bool,
    /// The task's general settings, for its toast.
    pub general: Option<Box<sharex_settings::TaskSettingsGeneral>>,
    /// `UploadInfoParser.Parse(info, BalloonTipContentFormat)` when the
    /// format isn't empty.
    pub balloon_text: Option<String>,
    /// `UploadDuration`.
    pub upload_duration: Option<Duration>,
    /// `TaskInfo.GetHistoryItem` at the task's end.
    pub history: Option<sharex_history::HistoryItem>,
    /// `TaskInfo.HistoryOCRTask`: the image's text for the history item's `OCR` tag.
    pub history_ocr: Option<crate::ocr::PendingText>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageKind {
    Thumbnail,
    Preview,
}

/// `TaskManager`, with `RecentManager`.
#[derive(Default)]
pub struct TaskManager {
    /// In the order they were started (`TaskManager.Tasks`).
    pub tasks: Vec<WorkerTask>,
    pub recent: Vec<RecentTask>,
    next_id: u64,
}

impl TaskManager {
    /// `WorkerTask.CreateImageUploaderTask` + `TaskManager.Start` +
    /// `Start`/`Prepare`: a new image task, already preparing. Its file
    /// name is the pattern's with a "bmp" extension until it's saved.
    pub fn start_image_task(&mut self, file_name: String) -> u64 {
        self.next_id += 1;
        let id = self.next_id;
        let info = TaskInfo {
            file_name,
            status: s::UploadTask_Prepare_Preparing().to_string(),
            start_time: Some(SystemTime::now()),
            ..Default::default()
        };
        self.tasks.push(WorkerTask { id, status: TaskStatus::Preparing, info, stop_requested: false, stop: None, transfer: Default::default() });
        id
    }

    /// `TaskManager.Start` of a new task: in the queue until
    /// [`Self::prepare`] starts it.
    pub fn add_task(&mut self, info: TaskInfo, stop: crate::upload::StopHandle) -> u64 {
        self.next_id += 1;
        let id = self.next_id;
        self.tasks.push(WorkerTask {
            id,
            status: TaskStatus::InQueue,
            info,
            stop_requested: false,
            stop: Some(stop),
            transfer: Default::default(),
        });
        id
    }

    /// `TaskManager.StartTasks`: the queued tasks that may start now, oldest
    /// first (all of them with no `UploadLimit`).
    pub fn startable(&self, upload_limit: i32) -> Vec<u64> {
        let queued = self.tasks.iter().filter(|t| t.status == TaskStatus::InQueue && !t.stop_requested).map(|t| t.id);
        if upload_limit <= 0 {
            return queued.collect();
        }
        let working = self.tasks.iter().filter(|t| t.is_working()).count() as i32;
        queued.take((upload_limit - working).max(0) as usize).collect()
    }

    /// `WorkerTask.Start` + `Prepare`: preparing (images and text) or
    /// starting (the rest).
    pub fn prepare(&mut self, id: u64) {
        let Some(task) = self.tasks.iter_mut().find(|t| t.id == id) else { return };
        task.status = TaskStatus::Preparing;
        task.info.start_time = Some(SystemTime::now());
        task.info.status = match task.info.job {
            TaskJob::Job | TaskJob::TextUpload => s::UploadTask_Prepare_Preparing(),
            _ => s::UploadTask_Prepare_Starting(),
        }
        .to_string();
    }

    /// `StatusChanged` from the task's thread. A stop asked for meanwhile
    /// keeps "Stopping...".
    pub fn set_status(&mut self, id: u64, status: TaskStatus, text: String) {
        let Some(task) = self.tasks.iter_mut().find(|t| t.id == id) else { return };
        if task.stop_requested || !task.is_working() {
            return;
        }
        task.status = status;
        task.info.status = text;
    }

    /// The task's file changed on its thread.
    pub fn set_file(&mut self, id: u64, file_name: String, file_path: Option<PathBuf>) {
        let Some(task) = self.tasks.iter_mut().find(|t| t.id == id) else { return };
        if file_path.is_some() {
            task.info.set_file_path(file_path);
        }
        if !file_name.is_empty() {
            task.info.file_name = file_name;
        }
    }

    /// `uploader_ProgressChanged` (`Info.Progress`) and, while working, the
    /// list view's transfer columns (`RefreshListColumns`).
    pub fn set_progress(&mut self, id: u64, progress: UploadProgress, binary_units: bool) {
        let Some(task) = self.tasks.iter_mut().find(|t| t.id == id) else { return };
        task.info.progress = Some(progress);
        if task.status == TaskStatus::Working && progress.length > 0 {
            task.transfer[0] =
                format!("{} / {}", size_string(progress.position, binary_units), size_string(progress.length, binary_units));
            if progress.speed > 0.0 {
                task.transfer[1] = format!("{}/s", size_string(progress.speed as u64, binary_units));
            }
            task.transfer[2] = proper_time_span(progress.elapsed);
            task.transfer[3] = proper_time_span(progress.remaining);
        }
    }

    /// `WorkerTask.Stop`: a queued task stops at once (`OnTaskCompleted`);
    /// a preparing or working one is told to stop and shows "Stopping...".
    /// Returns whether a queued task completed.
    pub fn stop(&mut self, id: u64, max_recent: usize) -> bool {
        let Some(task) = self.tasks.iter_mut().find(|t| t.id == id) else { return false };
        task.stop_requested = true;
        if let Some(stop) = &task.stop {
            stop.stop();
        }
        match task.status {
            TaskStatus::InQueue => {
                self.complete(id, None, None, max_recent);
                true
            }
            TaskStatus::Preparing | TaskStatus::Working => {
                task.status = TaskStatus::Stopping;
                task.info.status = s::UploadTask_Stop_Stopping().to_string();
                false
            }
            _ => false,
        }
    }

    /// A task thread's event on the task it's about (`Task_StatusChanged`,
    /// `Task_UploadProgressChanged`, `OnTaskCompleted`, ...). Returns that
    /// task's id, or `None` for events about no task.
    pub fn apply_event(&mut self, event: &TaskEvent, binary_units: bool, max_recent: usize) -> Option<u64> {
        match event {
            TaskEvent::Status { id, status, text } => {
                self.set_status(*id, *status, text.clone());
                Some(*id)
            }
            TaskEvent::File { id, file_name, file_path } => {
                self.set_file(*id, file_name.clone(), file_path.clone());
                Some(*id)
            }
            TaskEvent::UploadStarted { id } => Some(*id),
            TaskEvent::Progress { id, progress } => {
                self.set_progress(*id, *progress, binary_units);
                Some(*id)
            }
            TaskEvent::Finished { id, report } => {
                self.finish(*id, report, max_recent);
                Some(*id)
            }
            TaskEvent::DestinationSettingsRequested { .. } | TaskEvent::LargeFileWarningOff | TaskEvent::Image { .. } | TaskEvent::ImageReady { .. } => None,
        }
    }

    /// `OnTaskCompleted` of a task `crate::upload` ran: its file, result and
    /// stop, then the bookkeeping of [`Self::complete`].
    pub fn finish(&mut self, id: u64, report: &TaskReport, max_recent: usize) -> bool {
        let Some(task) = self.tasks.iter_mut().find(|t| t.id == id) else { return false };
        task.info.file_name = report.file_name.clone();
        task.info.file_path = report.file_path.clone();
        task.info.result = report.result.clone();
        task.info.amazon_s3_file = report.amazon_s3_file;
        task.stop_requested |= report.stop_requested;
        self.complete(id, None, None, max_recent)
    }

    /// `WorkerTask.CreateHistoryTask`, started (`AddRecentTasksToMainWindow`).
    pub fn start_history_task(&mut self, recent: &RecentTask) -> u64 {
        self.next_id += 1;
        let id = self.next_id;
        let mut info = TaskInfo::default();
        info.set_file_path((!recent.file_path.is_empty()).then(|| PathBuf::from(&recent.file_path)));
        info.result = UploadResult {
            url: recent.url.clone(),
            thumbnail_url: recent.thumbnail_url.clone(),
            deletion_url: recent.deletion_url.clone(),
            shortened_url: recent.shortened_url.clone(),
            ..Default::default()
        };
        self.tasks.push(WorkerTask { id, status: TaskStatus::History, info, stop_requested: false, stop: None, transfer: Default::default() });
        id
    }

    pub fn get(&self, id: u64) -> Option<&WorkerTask> {
        self.tasks.iter().find(|t| t.id == id)
    }

    /// `OnTaskCompleted` (`Done`, `Error` or `Stopped`), then
    /// `TaskManager.Task_TaskCompleted`'s bookkeeping: a task with a result
    /// joins the recent tasks. Returns whether it completed without error.
    pub fn complete(&mut self, id: u64, file_path: Option<PathBuf>, error: Option<String>, max_recent: usize) -> bool {
        let Some(task) = self.tasks.iter_mut().find(|t| t.id == id) else { return false };
        if file_path.is_some() {
            task.info.set_file_path(file_path);
        }
        if let Some(e) = error {
            task.info.result.errors.push(e);
        }
        task.info.end_time = Some(SystemTime::now());
        let (status, text) = if task.stop_requested {
            (TaskStatus::Stopped, s::UploadTask_OnUploadCompleted_Stopped())
        } else if task.info.result.is_error() {
            (TaskStatus::Failed, s::TaskManager_task_UploadCompleted_Error())
        } else {
            (TaskStatus::Completed, s::UploadTask_OnUploadCompleted_Done())
        };
        task.status = status;
        task.info.status = text.to_string();
        let ok = status == TaskStatus::Completed;
        if !task.info.text().is_empty() {
            let recent = RecentTask {
                file_path: task.info.file_path_str(),
                url: task.info.result.url.clone(),
                thumbnail_url: task.info.result.thumbnail_url.clone(),
                deletion_url: task.info.result.deletion_url.clone(),
                shortened_url: task.info.result.shortened_url.clone(),
                time: now_json(),
            };
            // `RecentTaskManager.Add`: the oldest goes past `MaxCount`.
            while self.recent.len() >= max_recent.clamp(1, 100) {
                self.recent.remove(0);
            }
            self.recent.push(recent);
        }
        ok
    }

    /// `TaskManager.Remove` (with `RecentManager.Remove`).
    pub fn remove(&mut self, id: u64) {
        if let Some(i) = self.tasks.iter().position(|t| t.id == id) {
            let task = self.tasks.remove(i);
            // `task.Stop()` first: a removed task's upload doesn't go on.
            if let Some(stop) = &task.stop {
                stop.stop();
            }
            let (path, url) = (task.info.file_path_str(), task.info.result.url.clone());
            self.recent.retain(|r| r.file_path != path || r.url != url);
        }
    }

    /// `ClearTasks`: every task, and the recent tasks.
    pub fn clear(&mut self) {
        self.tasks.clear();
        self.recent.clear();
    }
}

/// `UploadInfoStatus`: what a selected task has, checked against the disk.
#[derive(Clone, Debug, Default)]
pub struct UploadInfoStatus {
    pub is_url_exist: bool,
    pub is_shortened_url_exist: bool,
    pub is_thumbnail_url_exist: bool,
    pub is_deletion_url_exist: bool,
    pub is_file_url: bool,
    pub is_image_url: bool,
    pub is_file_path_valid: bool,
    pub is_file_exist: bool,
    pub is_thumbnail_file_exist: bool,
    pub is_image_file: bool,
    pub is_video_file: bool,
    pub is_text_file: bool,
}

impl UploadInfoStatus {
    pub fn new(info: &TaskInfo) -> Self {
        let r = &info.result;
        let url = !r.url.is_empty();
        let is_file_url = url && is_file_url(&r.url);
        let path = info.file_path_str();
        let valid = !path.is_empty() && Path::new(&path).extension().is_some();
        let exists = valid && Path::new(&path).is_file();
        let thumb = info.thumbnail_file_path.as_deref().is_some_and(|p| p.extension().is_some() && p.is_file());
        Self {
            is_url_exist: url,
            is_shortened_url_exist: !r.shortened_url.is_empty(),
            is_thumbnail_url_exist: !r.thumbnail_url.is_empty(),
            is_deletion_url_exist: !r.deletion_url.is_empty(),
            is_file_url,
            is_image_url: is_file_url && is_image_file(&r.url),
            is_file_path_valid: valid,
            is_file_exist: exists,
            is_thumbnail_file_exist: thumb,
            is_image_file: exists && is_image_file(&path),
            is_video_file: exists && is_video_file(&path),
            is_text_file: exists && is_text_file(&path),
        }
    }
}

/// `NumberExtensions.ToSizeString(size, binary)` (2 decimal places).
pub fn size_string(size: u64, binary: bool) -> String {
    let base: f64 = if binary { 1024.0 } else { 1000.0 };
    let suffixes = if binary { ["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB"] } else { ["B", "KB", "MB", "GB", "TB", "PB", "EB"] };
    if (size as f64) < base {
        return format!("{size} B");
    }
    let place = ((size as f64).ln() / base.ln()).floor() as i32;
    let num = size as f64 / base.powi(place);
    // `ToString("0.00")` rounds half away from zero.
    let rounded = (num * 100.0).round() / 100.0;
    format!("{rounded:.2} {}", suffixes[place.clamp(0, 6) as usize])
}

/// `Helpers.ProperTimeSpan`: `mm:ss`, with the hours in front when there are any.
pub fn proper_time_span(ts: Duration) -> String {
    let total = ts.as_secs();
    let time = format!("{:02}:{:02}", (total / 60) % 60, total % 60);
    let hours = total / 3600;
    if hours > 0 { format!("{hours}:{time}") } else { time }
}

/// `URLHelpers.IsFileURL`: a URL whose last path segment has an extension.
fn is_file_url(url: &str) -> bool {
    let path = url.split(['?', '#']).next().unwrap_or_default();
    let last = path.rsplit('/').next().unwrap_or_default();
    path.contains("://") && path.matches('/').count() > 2 && last.contains('.')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_task_goes_from_preparing_to_done() {
        let mut m = TaskManager::default();
        let id = m.start_image_task("shot.bmp".into());
        assert_eq!(m.get(id).unwrap().status, TaskStatus::Preparing);
        assert_eq!(m.get(id).unwrap().info.status, s::UploadTask_Prepare_Preparing());
        assert!(m.complete(id, Some(PathBuf::from("dir/shot.png")), None, 10));
        let t = m.get(id).unwrap();
        assert_eq!((t.status, t.info.file_name.as_str()), (TaskStatus::Completed, "shot.png"));
        assert_eq!(t.info.status, s::UploadTask_OnUploadCompleted_Done());
        assert_eq!(t.info.text(), PathBuf::from("dir/shot.png").to_string_lossy());
        assert_eq!(m.recent.len(), 1);
        m.remove(id);
        assert!(m.tasks.is_empty() && m.recent.is_empty());
    }

    #[test]
    fn errors_fail_a_task() {
        let mut m = TaskManager::default();
        let id = m.start_image_task("a.bmp".into());
        assert!(!m.complete(id, None, Some("disk full".into()), 10));
        assert_eq!(m.get(id).unwrap().status, TaskStatus::Failed);
    }

    #[test]
    fn recent_tasks_keep_the_newest() {
        let mut m = TaskManager::default();
        for i in 0..4 {
            let id = m.start_image_task(format!("{i}.bmp"));
            m.complete(id, Some(PathBuf::from(format!("{i}.png"))), None, 2);
        }
        assert_eq!(m.recent.iter().map(|r| r.file_path.as_str()).collect::<Vec<_>>(), ["2.png", "3.png"]);
    }

    #[test]
    fn sizes_and_times_as_dotnet_formats_them() {
        assert_eq!(size_string(999, false), "999 B");
        assert_eq!(size_string(1500, false), "1.50 KB");
        assert_eq!(size_string(1536, true), "1.50 KiB");
        assert_eq!(size_string(5_000_000, false), "5.00 MB");
        assert_eq!(proper_time_span(Duration::from_secs(65)), "01:05");
        assert_eq!(proper_time_span(Duration::from_secs(3 * 3600 + 7)), "3:00:07");
    }

    #[test]
    fn upload_errors_fail_only_without_a_url() {
        let mut r = UploadResult {
            errors: vec!["shortener failed".into()],
            url: "https://x/a".into(),
            url_expected: true,
            ..Default::default()
        };
        assert!(!r.is_error());
        r.url.clear();
        assert!(r.is_error());
    }

    #[test]
    fn stopping_a_queued_task_completes_it() {
        let mut m = TaskManager::default();
        let id = m.add_task(TaskInfo { job: TaskJob::TextUpload, ..Default::default() }, crate::upload::StopHandle::default());
        assert!(m.stop(id, 10));
        assert_eq!(m.get(id).unwrap().status, TaskStatus::Stopped);
    }

    #[test]
    fn the_upload_limit_queues_tasks() {
        let mut m = TaskManager::default();
        let ids: Vec<u64> = (0..3).map(|_| m.add_task(TaskInfo::default(), crate::upload::StopHandle::default())).collect();
        assert_eq!(m.startable(0), ids);
        assert_eq!(m.startable(2), &ids[..2]);
        m.prepare(ids[0]);
        assert_eq!(m.startable(2), &ids[1..2]);
    }

    #[test]
    fn file_urls_have_an_extension() {
        assert!(is_file_url("https://i.example.com/a/b.png"));
        assert!(!is_file_url("https://example.com/"));
    }
}
