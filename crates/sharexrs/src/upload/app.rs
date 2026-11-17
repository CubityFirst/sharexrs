//! The app's side of upload tasks: `TaskManager.Start`/`StartTasks` (each
//! task on its own thread, `UploadLimit` at a time), the task events
//! (`Task_StatusChanged`, `Task_UploadProgressChanged`,
//! `Task_TaskCompleted`), `TaskManager.StopAllTasks`, the upload jobs of
//! hotkeys, the tray and the Upload menu (`TaskHelpers.ExecuteJob`,
//! `MainMenuBuilder.BuildUploadMenu`), and `UploadersConfig.json` saved
//! with the other settings.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use sharex_settings::{HotkeyType, TaskSettings};
use sharex_strings::sharex as s;
use sharex_uploaders::config::uploaders_config_file_path;

use super::manager::{self, NewTaskContext, dialogs};
use super::worker::{self, UploadContext, UploadTask};
use super::{StopHandle, SystemUi, UploadersConfigStore};
use crate::main_menu::MenuCommand;
use crate::tasks::{TaskEvent, TaskJob, TaskReport, TaskStatus};
use crate::{App, UserEvent, sound};

/// The uploads' state the app keeps.
#[derive(Default)]
pub struct Uploads {
    /// `ApplicationState.UploadersConfig`.
    pub config: UploadersConfigStore,
    /// Tasks waiting in the queue (`TaskStatus.InQueue`) for `UploadLimit`.
    queued: Vec<(u64, UploadTask)>,
}

impl Uploads {
    /// `SettingManager.LoadInitialSettings`' uploaders part: the config
    /// loads in the background; URL sharing services open pages in the
    /// browser.
    pub fn load(settings: &sharex_settings::SettingManager) -> Self {
        let path = uploaders_config_file_path(&settings.paths, Some(&settings.settings));
        let config = UploadersConfigStore::load_in_background(path, settings.paths.backup_folder());
        sharex_uploaders::destinations::sharing::set_url_opener(Some(Arc::new(|url: &str| super::open_url(url))));
        Self { config, queued: Vec::new() }
    }
}

impl App {
    fn upload_context(&self) -> UploadContext {
        let proxy = Mutex::new(self.proxy.clone());
        let owner = self.main.as_ref().and_then(|w| sharex_capture::platform::window_handle(&w.window));
        UploadContext {
            config: self.uploads.config.clone(),
            ui: Arc::new(SystemUi { owner }),
            events: Arc::new(move |e| {
                let _ = proxy.lock().expect("event proxy").send_event(UserEvent::Task(e));
            }),
            app: self.settings.settings.clone(),
            system_disable_upload: self.settings.system.disable_upload,
            screenshots_parent: self.settings.paths.screenshots_parent_folder(Some(&self.settings.settings)),
            owner,
            retry_delay: Duration::from_secs(1),
            request_timeout: None,
        }
    }

    fn new_task_context(&mut self) -> NewTaskContext<'_> {
        let screenshots_parent = self.settings.paths.screenshots_parent_folder(Some(&self.settings.settings));
        let image_effects_folder = self.settings.paths.image_effects_folder();
        NewTaskContext { app: &mut self.settings.settings, screenshots_parent, image_effects_folder }
    }

    /// `TaskManager.Start` for each task: shown at once, started as
    /// `UploadLimit` allows.
    pub(crate) fn start_upload_tasks(&mut self, tasks: Vec<UploadTask>) {
        for task in tasks {
            let id = self.state.tasks.manager.add_task(task.info(), StopHandle::default());
            self.state.tasks.add_task(id);
            self.uploads.queued.push((id, task));
        }
        self.start_queued_tasks();
        self.update_progress_ui();
        if let Some(w) = &self.main {
            w.window.request_redraw();
        }
    }

    /// `TaskManager.StartTasks`.
    fn start_queued_tasks(&mut self) {
        let limit = self.settings.settings.UploadLimit;
        for id in self.state.tasks.manager.startable(limit) {
            let Some(i) = self.uploads.queued.iter().position(|(q, _)| *q == id) else { continue };
            let (_, task) = self.uploads.queued.remove(i);
            self.state.tasks.manager.prepare(id);
            eprintln!("Task status: Preparing");
            let stop = self.state.tasks.manager.get(id).and_then(|t| t.stop.clone()).unwrap_or_default();
            let ctx = self.upload_context();
            std::thread::Builder::new()
                .name(format!("task-{id}"))
                .spawn(move || {
                    let report = worker::run(task, id, &ctx, &stop);
                    (ctx.events)(TaskEvent::Finished { id, report: Box::new(report) });
                })
                .expect("task thread");
        }
        // Stopped while queued: nothing to run.
        let manager = &self.state.tasks.manager;
        self.uploads.queued.retain(|(id, _)| manager.get(*id).is_some_and(|t| t.status == TaskStatus::InQueue));
    }

    /// `WorkerTask.Stop` for these tasks (`Stop upload`, `StopAllTasks`).
    pub(crate) fn stop_tasks(&mut self, ids: &[u64]) {
        let max = self.settings.settings.RecentTasksMaxCount.clamp(1, 100) as usize;
        for &id in ids {
            if self.state.tasks.manager.stop(id, max) {
                self.task_completed(id, &TaskReport { stop_requested: true, ..Default::default() });
            }
            self.state.tasks.task_changed(id);
        }
        self.start_queued_tasks();
        self.update_progress_ui();
    }

    /// The events of tasks `crate::upload` runs. Returns false for others.
    pub(crate) fn upload_task_event(&mut self, event: &TaskEvent) -> bool {
        let max = self.settings.settings.RecentTasksMaxCount.clamp(1, 100) as usize;
        let binary_units = self.settings.settings.BinaryUnits;
        self.state.tasks.manager.apply_event(event, binary_units, max);
        match event {
            TaskEvent::Status { id, .. } | TaskEvent::File { id, .. } | TaskEvent::UploadStarted { id } => self.state.tasks.task_changed(*id),
            TaskEvent::Progress { .. } => {}
            TaskEvent::DestinationSettingsRequested { service } => {
                // `TaskHelpers.OpenUploadersConfigWindow(service)`.
                eprintln!("Destination settings window not ported yet ({service}).");
            }
            TaskEvent::Finished { id, report } => {
                self.state.tasks.task_changed(*id);
                self.task_completed(*id, report);
                self.start_queued_tasks();
            }
            TaskEvent::Image { .. } => return false,
        }
        if let Some(w) = &self.main {
            w.window.request_redraw();
        }
        true
    }

    /// `TaskManager.Task_TaskCompleted` after `OnTaskCompleted`.
    fn task_completed(&mut self, id: u64, report: &TaskReport) {
        if let Some(task) = self.state.tasks.manager.get(id) {
            let info = &task.info;
            crate::history::task_completed(&self.settings.settings, info, report.history.as_ref());
            let duration =
                info.end_time.zip(info.start_time).and_then(|(e, s)| e.duration_since(s).ok()).map_or(0, |d| d.as_millis());
            eprintln!("Task status: {:?}", task.status);
            let result = info.text();
            match task.status {
                TaskStatus::Stopped => eprintln!("Task stopped. File name: {}", info.file_name),
                TaskStatus::Failed => {
                    eprintln!("Task failed. File name: {}, Errors:\n{}", info.file_name, info.result.errors.join("\n\n"));
                    if report.play_sound {
                        sound::play_error();
                    }
                    if report.show_toast && !info.result.errors.is_empty() {
                        let title =
                            if info.result.error_title.is_empty() { s::TaskManager_task_UploadCompleted_Error() } else { &info.result.error_title };
                        eprintln!("Notification not ported yet: ShareXrs - {title}");
                    }
                }
                _ => {
                    eprintln!("Task completed. File name: {}, Duration: {duration} ms", info.file_name);
                    if !task.stop_requested && info.job != TaskJob::ShareURL && !result.is_empty() {
                        if report.play_sound {
                            sound::play_task_completed();
                        }
                        if report.show_toast {
                            // `NotificationWindow.Show`, then the after upload window with it.
                            eprintln!("Notification not ported yet: {result}");
                            if report.show_after_upload_window && report.is_upload_job {
                                eprintln!("After upload window not ported yet: {result}");
                            }
                        }
                    }
                }
            }
        }
        self.store_recent_tasks();
        let busy = self.state.tasks.manager.tasks.iter().any(|t| t.is_busy());
        if self.settings.settings.SaveSettingsAfterTaskCompleted && !busy {
            self.save_all_settings_async();
        }
    }

    /// `SettingManager.SaveUploadersConfigAsync` (or `Save` with `wait`).
    pub(crate) fn save_uploaders_config(&mut self, wait: bool) {
        let Some(json) = self.uploads.config.to_json() else { return };
        let Some(path) = uploaders_config_file_path(&self.settings.paths, Some(&self.settings.settings)) else { return };
        if let Err(e) = self.settings.save_file(json, path, wait) {
            eprintln!("couldn't save the uploaders config: {e}");
        }
    }

    /// `SettingManager.SaveAllSettingsAsync`.
    pub(crate) fn save_all_settings_async(&mut self) {
        self.settings.save_all_async();
        self.save_uploaders_config(false);
    }

    /// `TaskHelpers.ExecuteJob`'s upload jobs. Returns false for other jobs.
    pub(crate) fn run_upload_job(&mut self, job: &HotkeyType, task: &TaskSettings) -> bool {
        let tasks = match job {
            HotkeyType::FileUpload => manager::upload_file_with_picker(task, &mut self.new_task_context()),
            HotkeyType::FolderUpload => manager::upload_folder_with_picker(task, &mut self.new_task_context()),
            HotkeyType::ClipboardUpload => match dialogs::clipboard_content() {
                Some(content) => manager::clipboard_upload(content, task, &mut self.new_task_context()),
                None => Vec::new(),
            },
            HotkeyType::ClipboardUploadWithContentViewer => {
                dialogs::clipboard_upload_window();
                Vec::new()
            }
            HotkeyType::UploadText => match dialogs::text_upload_window() {
                Some(text) => manager::upload_text(&text, task, false, &mut self.new_task_context()).into_iter().collect(),
                None => Vec::new(),
            },
            HotkeyType::UploadURL => match dialogs::url_upload_window(None) {
                Some(url) => manager::download_and_upload_file(&url, task, &mut self.new_task_context()).into_iter().collect(),
                None => Vec::new(),
            },
            HotkeyType::DragDropUpload => {
                dialogs::drop_window();
                Vec::new()
            }
            HotkeyType::ShortenURL => dialogs::shorten_url_window(None).and_then(|url| manager::shorten_url(&url, task)).into_iter().collect(),
            HotkeyType::StopUploads => {
                let ids: Vec<u64> = self.state.tasks.manager.tasks.iter().map(|t| t.id).collect();
                self.stop_tasks(&ids);
                return true;
            }
            _ => return false,
        };
        self.start_upload_tasks(tasks);
        true
    }

    /// `BuildUploadMenu`'s entries and the Debug menu's test uploads
    /// (`MainWindowIntegration`), with the default task settings. Returns
    /// false for other commands.
    pub(crate) fn upload_menu_command(&mut self, command: &MenuCommand) -> bool {
        let task = TaskSettings::default_task_settings(&self.settings.settings.DefaultTaskSettings);
        let job = match command {
            MenuCommand::UploadFile => HotkeyType::FileUpload,
            MenuCommand::UploadFolder => HotkeyType::FolderUpload,
            MenuCommand::UploadClipboard => {
                let tasks = manager::clipboard_upload_main_window(&task, &mut self.new_task_context());
                self.start_upload_tasks(tasks);
                return true;
            }
            MenuCommand::UploadText => HotkeyType::UploadText,
            MenuCommand::UploadUrl => HotkeyType::UploadURL,
            MenuCommand::DragAndDropUpload => HotkeyType::DragDropUpload,
            MenuCommand::ShortenUrl => HotkeyType::ShortenURL,
            MenuCommand::TestImageUpload | MenuCommand::TestFileUpload => {
                // `UploadManager.UploadImage(ShareXResources.Logo[, FileUploader])`
                // with ShareXrs's logo.
                let mut task = task;
                task.UseDefaultAfterCaptureJob = false;
                task.AfterCaptureJob = sharex_settings::AfterCaptureTasks::UploadImageToHost;
                if *command == MenuCommand::TestFileUpload {
                    task.UseDefaultDestinations = false;
                    task.ImageDestination = sharex_settings::ImageDestination::FileUploader;
                    task.ImageFileDestination = self.settings.settings.DefaultTaskSettings.FileDestination.clone();
                }
                let logo = image::RgbaImage::from_raw(64, 64, crate::app_icon::rgba64().to_vec());
                if let Some(logo) = logo {
                    let t = manager::process_image_upload(logo, task, &mut self.new_task_context());
                    self.start_upload_tasks(vec![t]);
                }
                return true;
            }
            MenuCommand::TestTextUpload => {
                let text = s::MainForm_tsmiTestTextUpload_Click_Text_upload_test();
                let t = manager::upload_text(text, &task, false, &mut self.new_task_context());
                self.start_upload_tasks(t.into_iter().collect());
                return true;
            }
            MenuCommand::TestUrlShortener | MenuCommand::TestUrlSharing => {
                // `Links.Website`.
                let url = "https://getsharex.com";
                let t = if *command == MenuCommand::TestUrlShortener { manager::shorten_url(url, &task) } else { manager::share_url(url, &task) };
                self.start_upload_tasks(t.into_iter().collect());
                return true;
            }
            _ => return false,
        };
        self.run_upload_job(&job, &task)
    }
}
