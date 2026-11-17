//! The app's history (`ApplicationState.HistoryManager`): History.db opened
//! at startup after the JSON migration (`SettingManager.MigrateHistoryFile`,
//! `HistoryConnect`), closed on exit (`HistoryClose`), and finished tasks
//! added to it: the upload worker makes the task's `TaskInfo.GetHistoryItem`
//! when it ends (`upload::worker`), and `TaskManager.Task_TaskCompleted`
//! (`upload::app`) adds it under the history settings
//! (`AppendHistoryItemAsync`). The model and the database are
//! `sharex-history` (docs/HISTORY.md).

use std::path::Path;
use std::sync::{Arc, Mutex};

use sharex_history::{DateTime, HistoryItem, HistoryManagerSqlite, SystemZone, Tags};
use sharex_settings::{AppPaths, ApplicationConfig, TaskSettings};
use sharex_uploaders::{ImageDestination, TextDestination, UploadResult};

use crate::tasks::{EDataType, TaskInfo, TaskJob};

static HISTORY: Mutex<Option<HistoryManagerSqlite>> = Mutex::new(None);

/// `MigrateHistoryFile` then `HistoryConnect`. Nothing in sandbox mode.
/// ShareX shows an error box when either fails; ShareXrs logs it.
pub fn connect(paths: &AppPaths) {
    let db = paths.history_file();
    let old = (!paths.sandbox).then(|| paths.personal_folder.join(sharex_history::HISTORY_FILE_NAME_OLD));
    if let Err(e) = sharex_history::migrate_history_file(db.as_deref(), old.as_deref(), &paths.backup_folder(), Arc::new(SystemZone)) {
        eprintln!("history: migrating History.json failed: {e}");
    }
    let manager = db.and_then(|db| match HistoryManagerSqlite::open(&db) {
        Ok(m) => Some(m),
        Err(e) => {
            eprintln!("history: couldn't open {}: {e}", db.display());
            None
        }
    });
    *HISTORY.lock().unwrap() = manager;
}

/// `HistoryClose`.
pub fn close() {
    HISTORY.lock().unwrap().take();
}

/// `EnumExtensions.GetLocalizedDescription()` of a destination: the
/// `<Type>_<Value>` string of ShareX.HelpersLib's resources, else its
/// `[Description]`.
fn localized_description(type_name: &str, value: &str) -> String {
    sharex_strings::helpers_lib::lookup(&format!("{type_name}_{value}"))
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| sharex_uploaders::factory::enum_description(type_name, value))
}

/// `TaskInfo.UploaderHost`: the destination of an upload job, by its
/// `UploadDestination` (an image or text going to the file uploader counts as
/// a file); null for jobs that don't upload.
pub fn uploader_host(job: TaskJob, data_type: EDataType, task: &TaskSettings, is_upload_job: bool) -> Option<String> {
    if !is_upload_job {
        return None;
    }
    let destination = match data_type {
        EDataType::Image if task.ImageDestination == ImageDestination::FileUploader => EDataType::File,
        EDataType::Text if task.TextDestination == TextDestination::FileUploader => EDataType::File,
        d => d,
    };
    Some(match destination {
        EDataType::Image => localized_description("ImageDestination", task.ImageDestination.name()),
        EDataType::Text => localized_description("TextDestination", task.TextDestination.name()),
        EDataType::File => {
            let d = match data_type {
                EDataType::Image => &task.ImageFileDestination,
                EDataType::Text => &task.TextFileDestination,
                _ => &task.FileDestination,
            };
            localized_description("FileDestination", d.name())
        }
        EDataType::URL if job == TaskJob::ShareURL => {
            localized_description("URLSharingServices", task.URLSharingServiceDestination.name())
        }
        EDataType::URL => localized_description("UrlShortenerType", task.URLShortenerDestination.name()),
        EDataType::Default => return None,
    })
}

/// What `TaskInfo.GetHistoryItem` reads, as the worker has it at the end.
pub struct TaskEnd<'a> {
    pub file_name: &'a str,
    pub file_path: Option<&'a Path>,
    pub data_type: EDataType,
    pub host: Option<String>,
    pub result: &'a UploadResult,
    /// `Metadata.WindowTitle` / `ProcessName` (image tasks).
    pub window_title: Option<&'a str>,
    pub process_name: Option<&'a str>,
}

/// `TaskInfo.GetHistoryItem`, dated now (`TaskEndTime`, set as the task
/// completes): `Type` is `DataType` by name, the URLs the result's (null when
/// unset), the window title and process name as tags (`GetTags`, null when
/// there are neither).
pub fn history_item(end: &TaskEnd<'_>) -> HistoryItem {
    let mut tags = Tags::new();
    if let Some(t) = end.window_title.filter(|t| !t.is_empty()) {
        tags.set("WindowTitle", Some(t.to_string()));
    }
    if let Some(p) = end.process_name.filter(|p| !p.is_empty()) {
        tags.set("ProcessName", Some(p.to_string()));
    }
    let r = end.result;
    HistoryItem {
        id: 0,
        file_name: Some(end.file_name.to_string()),
        file_path: end.file_path.map(|p| p.to_string_lossy().into_owned()),
        date_time: DateTime::now(),
        r#type: Some(format!("{:?}", end.data_type)),
        host: end.host.clone(),
        url: r.url.clone(),
        thumbnail_url: r.thumbnail_url.clone(),
        deletion_url: r.deletion_url.clone(),
        shortened_url: r.shortened_url.clone(),
        tags: (!tags.is_empty()).then_some(tags),
    }
}

/// `Task_TaskCompleted`: a task with a result (`info.ToString()` not empty)
/// goes into history when `HistorySaveTasks`, and with `HistoryCheckURL`
/// only if it has a URL or a shortened URL; added off the event loop.
/// (`HistorySaveOCRText` waits for OCR to be ported.)
pub fn task_completed(settings: &ApplicationConfig, info: &TaskInfo, item: Option<&HistoryItem>) {
    let Some(item) = item else { return };
    if info.text().is_empty()
        || !settings.HistorySaveTasks
        || (settings.HistoryCheckURL && info.result.url.is_empty() && info.result.shortened_url.is_empty())
    {
        return;
    }
    let mut item = item.clone();
    std::thread::spawn(move || {
        if let Some(m) = HISTORY.lock().unwrap().as_mut()
            && !m.append_history_item(&mut item)
        {
            eprintln!("history: couldn't add {}", item.file_name.as_deref().unwrap_or(""));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_finished_task_becomes_a_history_item() {
        let result = UploadResult { url: Some("https://i.imgur.com/a.png".into()), ..Default::default() };
        let end = TaskEnd {
            file_name: "a.png",
            file_path: Some(Path::new(r"C:\Shots\a.png")),
            data_type: EDataType::Image,
            host: Some("Imgur".into()),
            result: &result,
            window_title: None,
            process_name: Some("chrome"),
        };
        let item = history_item(&end);
        assert_eq!((item.r#type.as_deref(), item.host.as_deref()), (Some("Image"), Some("Imgur")));
        assert_eq!((item.url.as_deref(), item.deletion_url.clone()), (Some("https://i.imgur.com/a.png"), None));
        assert_eq!(item.tags.as_ref().unwrap().to_json(), r#"{"ProcessName":"chrome"}"#);
        assert!(item.is_valid());
    }

    #[test]
    fn uploader_host_follows_the_destination() {
        let mut t = TaskSettings::default();
        assert_eq!(uploader_host(TaskJob::Job, EDataType::Image, &t, false), None);
        assert_eq!(uploader_host(TaskJob::Job, EDataType::Image, &t, true).as_deref(), Some("Imgur"));
        t.ImageDestination = ImageDestination::CustomImageUploader;
        assert_eq!(
            uploader_host(TaskJob::Job, EDataType::Image, &t, true).as_deref(),
            sharex_strings::helpers_lib::lookup("ImageDestination_CustomImageUploader")
        );
        t.ImageDestination = ImageDestination::FileUploader;
        assert_eq!(uploader_host(TaskJob::Job, EDataType::Image, &t, true).as_deref(), Some("Dropbox"));
        assert_eq!(uploader_host(TaskJob::Download, EDataType::Default, &t, true), None);
    }
}
