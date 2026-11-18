//! Watch folders: `ShareX/WatchFolder.cs` (a `FileSystemWatcher` on the
//! folder; a new file is taken once it's unlocked and its size has held
//! still) and `ShareX/WatchFolderManager.cs` (a watcher per watch folder of
//! the default task settings and of every hotkey's, each new file uploaded
//! with its task settings, moved to the screenshots folder first when the
//! folder asks).
//!
//! The file system events come from `notify` (ReadDirectoryChangesW,
//! FSEvents, inotify), as `FileSystemWatcher` gets them from the OS. Only
//! files are taken: ShareX's wait throws on a new folder (it can't open it),
//! so a folder is never uploaded there either.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use notify::{EventKind, RecursiveMode, Watcher};
use sharex_settings::{ApplicationConfig, HotkeySettings, TaskSettings, WatchFolderSettings};

/// `WatchFolder`'s waits.
#[derive(Clone, Copy, Debug)]
pub struct Timing {
    /// `WaitWhileAsync`'s `waitStart`: before the first check.
    pub wait_start: Duration,
    /// Between checks.
    pub interval: Duration,
    /// How long the checks may go on before the file is given up.
    pub timeout: Duration,
    /// `WatchFolderDuplicateEventTimer.expireTime`: another event for the
    /// same path within this is the same file.
    pub duplicate_expire: Duration,
}

impl Default for Timing {
    /// `Helpers.WaitWhileAsync(…, 250, 5000, …, 1000)`; duplicates 1000 ms.
    fn default() -> Self {
        Self {
            wait_start: Duration::from_millis(1000),
            interval: Duration::from_millis(250),
            timeout: Duration::from_millis(5000),
            duplicate_expire: Duration::from_millis(1000),
        }
    }
}

/// What a watcher calls with a file that's ready (`FileWatcherTrigger`),
/// from its own thread; the app posts it to the event loop
/// (`context.Post`).
pub type Trigger = Arc<dyn Fn(PathBuf) + Send + Sync>;

/// `WatchFolder` while enabled: dropping it stops watching (`Dispose`).
pub struct WatchFolder {
    _watcher: notify::RecommendedWatcher,
}

impl WatchFolder {
    /// `Enable()`: watches the folder (its variables expanded) when it
    /// exists, with the filter and subfolders as set. `None` when there's
    /// nothing to watch.
    pub fn enable(settings: &WatchFolderSettings, timing: Timing, trigger: Trigger) -> Option<Self> {
        let folder = sharex_settings::paths::expand_folder_variables(settings.FolderPath.as_deref().unwrap_or_default());
        if folder.is_empty() || !Path::new(&folder).is_dir() {
            return None;
        }
        let filter = settings.clone();
        let timers: Arc<Mutex<Vec<(PathBuf, Instant)>>> = Arc::default();
        let handler = move |res: notify::Result<notify::Event>| {
            let Ok(event) = res else { return };
            // `fileWatcher.Created`.
            if !matches!(event.kind, EventKind::Create(_)) {
                return;
            }
            for path in event.paths {
                let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                if !filter.matches(&name) || path.is_dir() {
                    continue;
                }
                created(path, &timers, timing, trigger.clone());
            }
        };
        let mut watcher = match notify::recommended_watcher(handler) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("Watch folder: {folder}: {e}");
                return None;
            }
        };
        let mode = if settings.IncludeSubdirectories { RecursiveMode::Recursive } else { RecursiveMode::NonRecursive };
        if let Err(e) = watcher.watch(Path::new(&folder), mode) {
            eprintln!("Watch folder: {folder}: {e}");
            return None;
        }
        Some(Self { _watcher: watcher })
    }
}

/// `fileWatcher_Created`: drops an event for a path seen within the
/// duplicate time (restarting its timer), then waits on its own thread for
/// the file to be ready and triggers.
fn created(path: PathBuf, timers: &Mutex<Vec<(PathBuf, Instant)>>, timing: Timing, trigger: Trigger) {
    {
        let mut timers = timers.lock().expect("watch folder timers");
        // `CleanElapsedTimers`.
        timers.retain(|(_, t)| t.elapsed() < timing.duplicate_expire);
        // `IsDuplicateEvent`.
        if let Some(t) = timers.iter_mut().find(|(p, _)| *p == path) {
            t.1 = Instant::now();
            return;
        }
        timers.push((path.clone(), Instant::now()));
    }
    std::thread::Builder::new()
        .name("watch-folder".into())
        .spawn(move || {
            if wait_until_ready(&path, timing) {
                trigger(path);
            }
        })
        .expect("watch folder thread");
}

/// `Helpers.WaitWhileAsync` with `WatchFolder`'s check: true once the file
/// is unlocked with the same size above 0 at four checks after the first.
pub fn wait_until_ready(path: &Path, timing: Timing) -> bool {
    std::thread::sleep(timing.wait_start);
    let mut success = 0;
    let mut previous: i64 = -1;
    let started = Instant::now();
    loop {
        // The check (`return successCount < 4` / `return true`).
        let go_on = match is_file_locked(path) {
            Err(e) => {
                // An `UnauthorizedAccessException` escapes ShareX's wait.
                eprintln!("Watch folder: {}: {e}", path.display());
                return false;
            }
            Ok(false) => {
                let size = std::fs::metadata(path).map(|m| m.len() as i64).unwrap_or(-1);
                if size > 0 && size == previous {
                    success += 1;
                }
                previous = size;
                success < 4
            }
            Ok(true) => {
                previous = -1;
                true
            }
        };
        if !go_on {
            return true;
        }
        if started.elapsed() >= timing.timeout {
            return false;
        }
        std::thread::sleep(timing.interval);
    }
}

/// `FileHelpers.IsFileLocked`: whether the file can't be opened for
/// reading with no sharing (`FileShare.None`). Any I/O error counts as
/// locked (a missing file too: `FileNotFoundException` is an
/// `IOException`); access denied is ShareX's `UnauthorizedAccessException`,
/// which it doesn't catch.
pub fn is_file_locked(path: &Path) -> std::io::Result<bool> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(0);
    }
    match options.open(path) {
        Ok(_file) => Ok(false),
        #[cfg(windows)]
        Err(e) if e.raw_os_error() == Some(5) => Err(e),
        #[cfg(not(windows))]
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => Err(e),
        Err(_) => Ok(true),
    }
}

struct Entry {
    id: u64,
    settings: WatchFolderSettings,
    /// `watchFolder.TaskSettings`: the task settings as of the last update.
    task: TaskSettings,
    enabled: bool,
    /// Watching while it's held (`Dispose` when dropped).
    _watcher: Option<WatchFolder>,
}

/// `WatchFolderManager`.
#[derive(Default)]
pub struct WatchFolderManager {
    entries: Vec<Entry>,
    next_id: u64,
    pub timing: Timing,
}

impl WatchFolderManager {
    /// `UpdateWatchFolders`: a watch folder per entry of the default task
    /// settings' `WatchFolderList`, then of each hotkey's, enabled when its
    /// task settings' `WatchFolderEnabled` is on. A folder that's unchanged
    /// keeps its watcher (`AddWatchFolder` only adds what isn't there); a
    /// changed one is enabled again (`UpdateWatchFolderState`), a removed one
    /// stops (`RemoveWatchFolder`). `trigger` makes each watcher's trigger
    /// from its entry id.
    pub fn update(&mut self, defaults: &TaskSettings, hotkeys: &[HotkeySettings], trigger: &dyn Fn(u64) -> Trigger) {
        let mut wanted: Vec<(&WatchFolderSettings, &TaskSettings)> = defaults.WatchFolderList.iter().map(|w| (w, defaults)).collect();
        for h in hotkeys {
            wanted.extend(h.TaskSettings.WatchFolderList.iter().map(|w| (w, &h.TaskSettings)));
        }
        let mut old = std::mem::take(&mut self.entries);
        for (settings, task) in wanted {
            let enabled = task.WatchFolderEnabled;
            let entry = match old.iter().position(|e| e.settings == *settings && e.enabled == enabled) {
                Some(i) => {
                    let mut e = old.remove(i);
                    e.task = task.clone();
                    e
                }
                None => {
                    self.next_id += 1;
                    let id = self.next_id;
                    let watcher = if enabled { WatchFolder::enable(settings, self.timing, trigger(id)) } else { None };
                    Entry { id, settings: settings.clone(), task: task.clone(), enabled, _watcher: watcher }
                }
            };
            self.entries.push(entry);
        }
        // `UnregisterAllWatchFolders` for the rest.
        drop(old);
    }

    /// How many folders are being watched.
    #[cfg(test)]
    pub fn watching(&self) -> usize {
        self.entries.iter().filter(|e| e._watcher.is_some()).count()
    }

    /// The folder settings and task settings of a watcher.
    pub fn get(&self, id: u64) -> Option<(&WatchFolderSettings, &TaskSettings)> {
        self.entries.iter().find(|e| e.id == id).map(|e| (&e.settings, &e.task))
    }

    /// `Dispose()`.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

/// The trigger's first half (`watchFolder.FileWatcherTrigger` in
/// `AddWatchFolder`): with `MoveFilesToScreenshotsFolder` the file moves
/// to the screenshots folder (a unique name if taken, as the task's
/// `FileExistAction` says) before upload. Returns the path to upload, or
/// `None` when the move failed (ShareX's `File.Move` throws).
pub fn prepare_file(orig: &Path, folder: &WatchFolderSettings, task: &TaskSettings, app: &ApplicationConfig, screenshots_parent: &Path) -> Option<PathBuf> {
    if !folder.MoveFilesToScreenshotsFolder {
        return Some(orig.to_path_buf());
    }
    let metadata = crate::capture_job::Metadata { image: image::RgbaImage::new(0, 0), window_title: None, process_name: None };
    let screenshots = crate::capture_job::screenshots_folder(task, app, screenshots_parent, &metadata);
    let name = orig.file_name()?;
    let dest = crate::capture_job::handle_exists_file(&screenshots.join(name), task)?;
    if let Some(parent) = dest.parent()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        eprintln!("Watch folder: {}: {e}", parent.display());
        return None;
    }
    match move_file(orig, &dest) {
        Ok(()) => Some(dest),
        Err(e) => {
            eprintln!("Watch folder: moving {} to {}: {e}", orig.display(), dest.display());
            None
        }
    }
}

/// `File.Move`: a rename, or a copy and delete across volumes.
fn move_file(from: &Path, to: &Path) -> std::io::Result<()> {
    if to.exists() {
        return Err(std::io::Error::new(std::io::ErrorKind::AlreadyExists, "the destination file exists"));
    }
    match std::fs::rename(from, to) {
        Ok(()) => Ok(()),
        Err(_) => {
            std::fs::copy(from, to)?;
            std::fs::remove_file(from)
        }
    }
}

impl crate::App {
    /// `WatchFolderManager.UpdateWatchFolders()` over the current settings;
    /// each watcher's trigger goes to the event loop.
    pub(crate) fn update_watch_folders(&mut self) {
        let proxy = Mutex::new(self.proxy.clone());
        let proxy = Arc::new(proxy);
        let trigger = |id: u64| -> Trigger {
            let proxy = proxy.clone();
            Arc::new(move |path| {
                let _ = proxy.lock().expect("event proxy").send_event(crate::UserEvent::WatchFolder(id, path));
            })
        };
        let s = &self.settings;
        self.watch_folders.update(&s.settings.DefaultTaskSettings, &s.hotkeys.Hotkeys, &trigger);
    }

    /// The trigger's handler on the UI thread: `GetSafeTaskSettings`, the
    /// move to the screenshots folder, then `UploadManager.UploadFile`.
    pub(crate) fn watch_folder_trigger(&mut self, event_loop: &winit::event_loop::ActiveEventLoop, id: u64, path: &Path) {
        let Some((folder, task)) = self.watch_folders.get(id) else { return };
        let (folder, task) = (folder.clone(), task.safe_task_settings(&self.settings.settings.DefaultTaskSettings));
        let parent = self.settings.paths.screenshots_parent_folder(Some(&self.settings.settings));
        let Some(dest) = prepare_file(path, &folder, &task, &self.settings.settings, &parent) else { return };
        let mut ctx = self.new_task_context();
        let tasks = crate::upload::manager::upload_file(&dest, &task, &mut ctx);
        let confirm = ctx.confirm_files.take();
        self.start_upload_tasks(tasks);
        self.confirm_files(event_loop, confirm, task);
    }
}

#[cfg(test)]
mod tests;
