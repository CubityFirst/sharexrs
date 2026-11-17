//! What runs behind the main window's task area: `ShareX/WorkerTask.cs`,
//! `TaskInfo.cs`, `TaskManager.cs`, `RecentTaskManager.cs` and
//! `UploadInfoStatus.cs`, for the tasks ShareXrs runs so far (image tasks
//! from a capture: after capture tasks, no upload).
//!
//! A task's work runs on its own thread, as ShareX's `ThreadWorker`; what
//! it reports comes back to the event loop as a [`TaskEvent`].

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use sharex_helpers::file_helpers::{is_image_file, is_text_file, is_video_file};
use sharex_strings::sharex as s;

/// `ShareX.TaskStatus`. `Working` and `Stopping` come with uploads.
#[allow(dead_code)]
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

/// `UploadResult`: what an upload returned. Nothing uploads yet, so only
/// the URLs a recent task brings back and the errors.
#[derive(Clone, Debug, Default)]
pub struct UploadResult {
    pub url: String,
    pub thumbnail_url: String,
    pub deletion_url: String,
    pub shortened_url: String,
    pub response: String,
    pub errors: Vec<String>,
}

impl UploadResult {
    pub fn is_error(&self) -> bool {
        !self.errors.is_empty()
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
    /// `OnTaskCompleted`: the after capture tasks ran (or failed).
    /// `play_sound`: the task's `PlaySoundAfterUpload`.
    Completed { id: u64, file_path: Option<PathBuf>, error: Option<String>, play_sound: bool },
    /// A thumbnail or preview was decoded (`RefreshThumbnailAsync`).
    Image { id: u64, version: u64, kind: ImageKind, image: Option<image::RgbaImage> },
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
        self.tasks.push(WorkerTask { id, status: TaskStatus::Preparing, info, stop_requested: false });
        id
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
        self.tasks.push(WorkerTask { id, status: TaskStatus::History, info, stop_requested: false });
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
    fn file_urls_have_an_extension() {
        assert!(is_file_url("https://i.example.com/a/b.png"));
        assert!(!is_file_url("https://example.com/"));
    }
}
