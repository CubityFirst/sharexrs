//! The app's history (`ApplicationState.HistoryManager`): History.db opened
//! at startup after the JSON migration (`SettingManager.MigrateHistoryFile`,
//! `HistoryConnect`), closed on exit (`HistoryClose`), and a finished task
//! added to it as `TaskManager.Task_TaskCompleted` does
//! (`AppendHistoryItemAsync`). The model and the database are
//! `sharex-history` (docs/HISTORY.md).

use std::sync::{Arc, Mutex};

use sharex_history::{DateTime, HistoryItem, HistoryManagerSqlite, SystemZone, Tags};
use sharex_settings::{AppPaths, ApplicationConfig};

use crate::tasks::TaskInfo;

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

/// `TaskInfo.GetHistoryItem` for the image tasks ShareXrs runs: `Type` is
/// `Image` (`EDataType`), `Host` the uploader (none: nothing uploads yet),
/// the date the task's end, and the window title and process name as tags
/// (`GetTags`, null when there are neither).
pub fn history_item(info: &TaskInfo) -> HistoryItem {
    let opt = |s: &str| (!s.is_empty()).then(|| s.to_string());
    let mut tags = Tags::new();
    if let Some(t) = info.window_title.as_deref().filter(|t| !t.is_empty()) {
        tags.set("WindowTitle", Some(t.to_string()));
    }
    if let Some(p) = info.process_name.as_deref().filter(|p| !p.is_empty()) {
        tags.set("ProcessName", Some(p.to_string()));
    }
    HistoryItem {
        id: 0,
        // TaskInfo.FilePath's setter makes the name "" (not null) without a path.
        file_name: Some(info.file_name.clone()),
        file_path: info.file_path.as_ref().map(|p| p.to_string_lossy().into_owned()),
        date_time: info.end_time.map(|t| DateTime::from_system_time(t, &SystemZone)).unwrap_or_else(DateTime::now),
        r#type: Some("Image".into()),
        host: None,
        url: opt(&info.result.url),
        thumbnail_url: opt(&info.result.thumbnail_url),
        deletion_url: opt(&info.result.deletion_url),
        shortened_url: opt(&info.result.shortened_url),
        tags: (!tags.is_empty()).then_some(tags),
    }
}

/// `Task_TaskCompleted`: a task with a result (`info.ToString()` not empty)
/// goes into history when `HistorySaveTasks`, and with `HistoryCheckURL`
/// only if it has a URL or a shortened URL; added off the event loop.
/// (`HistorySaveOCRText` waits for OCR to be ported.)
pub fn task_completed(settings: &ApplicationConfig, info: &TaskInfo) {
    if info.text().is_empty()
        || !settings.HistorySaveTasks
        || (settings.HistoryCheckURL && info.result.url.is_empty() && info.result.shortened_url.is_empty())
    {
        return;
    }
    let mut item = history_item(info);
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
    use std::path::PathBuf;

    #[test]
    fn a_finished_capture_becomes_a_history_item() {
        let mut info = TaskInfo::default();
        info.set_file_path(Some(PathBuf::from(r"C:\Shots\a.png")));
        info.end_time = Some(std::time::SystemTime::now());
        info.process_name = Some("chrome".into());
        let item = history_item(&info);
        assert_eq!(item.file_name.as_deref(), Some("a.png"));
        assert_eq!((item.host.clone(), item.url.clone()), (None, None));
        assert_eq!(item.tags.as_ref().unwrap().to_json(), r#"{"ProcessName":"chrome"}"#);
        assert!(item.is_valid());
        assert!(history_item(&TaskInfo::default()).tags.is_none());
    }
}
