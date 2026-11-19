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

use super::manager::{self, NewTaskContext};
use super::worker::{self, UploadContext, UploadTask};
use super::{StopHandle, SystemUi, UploadersConfigStore};
use crate::main_menu::MenuCommand;
use crate::tasks::{TaskEvent, TaskJob, TaskReport, TaskStatus};
use crate::dialogs::{self, UploadRequest};
use crate::{App, UserEvent, sound};
use winit::event_loop::ActiveEventLoop;

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
        let ui_proxy = Mutex::new(self.proxy.clone());
        let owner = self.main.as_ref().and_then(|w| sharex_capture::platform::window_handle(&w.window));
        let requests: super::RequestSink = Arc::new(move |r| {
            let _ = ui_proxy.lock().expect("event proxy").send_event(UserEvent::Ui(r));
        });
        UploadContext {
            config: self.uploads.config.clone(),
            ui: Arc::new(SystemUi { owner, requests: Some(requests) }),
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

    pub(crate) fn new_task_context(&mut self) -> NewTaskContext<'_> {
        let screenshots_parent = self.settings.paths.screenshots_parent_folder(Some(&self.settings.settings));
        let image_effects_folder = self.settings.paths.image_effects_folder();
        NewTaskContext::new(&mut self.settings.settings, screenshots_parent, image_effects_folder)
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
            TaskEvent::LargeFileWarningOff => self.settings.settings.ShowLargeFileSizeWarning = 0,
            TaskEvent::DestinationSettingsRequested { service } => {
                // `TaskHelpers.OpenUploadersConfigWindow(service)`.
                self.pending_dialogs.push(Box::new(crate::dialogs::destination_settings::DestinationSettingsWindow::new(Some(service))));
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

    /// A task's thread asks for a window: the large file warning, the
    /// before upload window or the QR code window.
    pub(crate) fn ui_request(&mut self, event_loop: &ActiveEventLoop, request: super::UiRequest) {
        use dialogs::confirm::{Confirm, ConfirmWindow};
        let dialog: Box<dyn dialogs::Dialog> = match request {
            super::UiRequest::LargeFileWarning(reply) => Box::new(ConfirmWindow::new(Confirm::LargeFile { reply: Some(reply) })),
            super::UiRequest::BeforeUpload(request, reply) => {
                let w = self.uploads.config.with(|c| dialogs::before_upload::BeforeUploadWindow::new(*request, c, reply));
                Box::new(w)
            }
            super::UiRequest::QrCode(text) => Box::new(dialogs::qr_code::QrCodeWindow::new(Some(text))),
            super::UiRequest::Ocr(image, task, reply) => {
                let mut w = crate::ocr_window::OcrWindow::for_task((*image).clone(), None, &task);
                w.reply = Some(reply);
                Box::new(w)
            }
            super::UiRequest::QrCodeScanFile(path) => Box::new(dialogs::qr_code::QrCodeWindow::scan_image_file(&path)),
            super::UiRequest::FileExists(path, reply) => Box::new(dialogs::file_exist::FileExistWindow::new(&path, Some(reply))),
            super::UiRequest::PinToScreen(image, task) => {
                self.pin_image(event_loop, *image, None, &task);
                return;
            }
            super::UiRequest::Annotate(image, task, reply) => {
                self.open_image_editor(event_loop, Some(*image), None, *task, Some(reply));
                return;
            }
            super::UiRequest::ImageEffects(image, task, reply) => {
                self.open_image_effects_editor(event_loop, *image, *task, reply);
                return;
            }
        };
        self.open_dialog_window(event_loop, dialog);
    }

    /// `AfterUploadWindowIntegration.Show(info)`.
    fn show_after_upload_window(&mut self, id: u64, report: &TaskReport) {
        let Some(task) = self.state.tasks.manager.get(id) else { return };
        let info = &task.info;
        let formats = self
            .settings
            .settings
            .ClipboardContentFormats
            .iter()
            .map(|f| {
                let get = |k: &str| f.get(k).and_then(serde_json::Value::as_str).unwrap_or_default().to_owned();
                (get("Description"), get("Format"))
            })
            .collect();
        let w = dialogs::after_upload::AfterUploadWindow::new(dialogs::after_upload::AfterUploadInfo {
            file_name: info.file_name.clone(),
            file_path: info.file_path.clone(),
            thumbnail_file_path: info.thumbnail_file_path.clone(),
            result: info.result.clone(),
            data_type: report.data_type,
            image: report.image.clone(),
            upload_duration: report.upload_duration,
            auto_close: report.auto_close_after_upload_form,
            clipboard_formats: formats,
        });
        self.pending_dialogs.push(Box::new(w));
    }

    /// `TaskManager.Task_TaskCompleted` after `OnTaskCompleted`.
    fn task_completed(&mut self, id: u64, report: &TaskReport) {
        if let Some(task) = self.state.tasks.manager.get(id) {
            let info = &task.info;
            crate::history::task_completed(&self.settings.settings, info, report.history.as_ref(), report.history_ocr.clone());
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
                                self.show_after_upload_window(id, report);
                            }
                        }
                    }
                }
            }
        }
        self.store_recent_tasks();
        // The native tray menus (macOS, Linux) are built ahead: Recent items.
        if cfg!(not(windows)) {
            self.refresh_menus();
        }
        let busy = self.state.tasks.manager.tasks.iter().any(|t| t.is_busy());
        // `TaskManager`'s `if (!IsBusy && StartupOptions.AutoClose) ApplicationLifecycle.Exit()`.
        if !busy && self.auto_close {
            self.exit_requested = true;
            return;
        }
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
    pub(crate) fn run_upload_job(&mut self, event_loop: &ActiveEventLoop, job: &HotkeyType, task: &TaskSettings) -> bool {
        let task = task.clone();
        match job {
            HotkeyType::FileUpload | HotkeyType::FolderUpload => {
                let mut ctx = self.new_task_context();
                let tasks = if *job == HotkeyType::FileUpload {
                    manager::upload_file_with_picker(&task, &mut ctx)
                } else {
                    manager::upload_folder_with_picker(&task, &mut ctx)
                };
                let confirm = ctx.confirm_files.take();
                self.start_upload_tasks(tasks);
                self.confirm_files(event_loop, confirm, task);
            }
            // `UploadManager.ClipboardUpload`.
            HotkeyType::ClipboardUpload => {
                if let Some(content) = dialogs::clipboard_upload::read_clipboard() {
                    self.upload_request(event_loop, UploadRequest::clipboard(content, task));
                }
            }
            // `ClipboardUploadWithContentViewer`.
            HotkeyType::ClipboardUploadWithContentViewer => {
                self.open_dialog_window(event_loop, Box::new(dialogs::clipboard_upload::ClipboardUploadWindow::new(task, false)));
            }
            // `ShowTextUploadDialog`.
            HotkeyType::UploadText => {
                self.open_dialog_window(event_loop, Box::new(dialogs::text_upload::TextUploadWindow::new(None, task)));
            }
            // `UploadURL`, `ShowShortenURLDialog`: a URL on the clipboard
            // fills the box.
            HotkeyType::UploadURL | HotkeyType::ShortenURL => {
                use dialogs::url_input::{UrlKind, UrlWindow};
                let kind = if *job == HotkeyType::UploadURL { UrlKind::Upload } else { UrlKind::Shorten };
                let text = sharex_capture::clipboard::get_text().filter(|t| sharex_uploaders::url_helpers::is_valid_url(t));
                self.open_dialog_window(event_loop, Box::new(UrlWindow::new(kind, text, task)));
            }
            // `TaskHelpers.OpenDropWindow`: one window; asking again gives
            // it these task settings.
            HotkeyType::DragDropUpload => {
                if let Some(d) = self.dialogs.iter_mut().find(|d| d.dialog.single() == Some("drop-window")) {
                    d.dialog.set_task(task);
                    d.window.window.focus_window();
                } else {
                    let w = dialogs::drop_window::DropWindow::new(&self.settings.settings, task);
                    self.open_dialog_window(event_loop, Box::new(w));
                }
            }
            HotkeyType::StopUploads => {
                let ids: Vec<u64> = self.state.tasks.manager.tasks.iter().map(|t| t.id).collect();
                self.stop_tasks(&ids);
            }
            _ => return false,
        }
        true
    }

    /// `MultiUploadConfirmationWindowIntegration.Show(count)` for files
    /// `UploadManager.UploadFile` held back.
    pub(crate) fn confirm_files(&mut self, event_loop: &ActiveEventLoop, files: Option<Vec<std::path::PathBuf>>, task: TaskSettings) {
        if let Some(files) = files {
            let kind = dialogs::confirm::Confirm::MultiUpload { files, task: Box::new(task) };
            self.open_dialog_window(event_loop, Box::new(dialogs::confirm::ConfirmWindow::new(kind)));
        }
    }

    /// What an upload window asked `UploadManager` to start.
    pub(crate) fn upload_request(&mut self, event_loop: &ActiveEventLoop, request: UploadRequest) {
        let defaults = TaskSettings::default_task_settings(&self.settings.settings.DefaultTaskSettings);
        let mut ctx = self.new_task_context();
        let (tasks, task) = match request {
            UploadRequest::Text { text, task, allow_custom_text } => {
                (manager::upload_text(&text, &task, allow_custom_text, &mut ctx).into_iter().collect(), task)
            }
            UploadRequest::ProcessText { text, task } => (manager::process_text_upload(&text, &task, &mut ctx), task),
            UploadRequest::DownloadAndUpload { url, task } => {
                (manager::download_and_upload_file(&url, &task, &mut ctx).into_iter().collect(), task)
            }
            UploadRequest::ShortenUrl { url, task } => (manager::shorten_url(&url, &task).into_iter().collect(), task),
            UploadRequest::ProcessImage { image, task } => (vec![manager::process_image_upload(image, task.clone(), &mut ctx)], task),
            // `UploadManager.UploadImage(image)`: the default task settings.
            UploadRequest::QrCodeImage(image) => (vec![manager::process_image_upload(image, defaults.clone(), &mut ctx)], defaults),
            UploadRequest::Files { files, task, confirmed } => {
                ctx.files_confirmed = confirmed;
                (manager::upload_files(&files, &task, &mut ctx), task)
            }
        };
        let confirm = ctx.confirm_files.take();
        self.start_upload_tasks(tasks);
        self.confirm_files(event_loop, confirm, task);
    }

    /// `BuildUploadMenu`'s entries and the Debug menu's test uploads
    /// (`MainWindowIntegration`), with the default task settings. Returns
    /// false for other commands.
    pub(crate) fn upload_menu_command(&mut self, event_loop: &ActiveEventLoop, command: &MenuCommand) -> bool {
        let task = TaskSettings::default_task_settings(&self.settings.settings.DefaultTaskSettings);
        let job = match command {
            MenuCommand::UploadFile => HotkeyType::FileUpload,
            MenuCommand::UploadFolder => HotkeyType::FolderUpload,
            // `UploadManager.ClipboardUploadMainWindow`: the content viewer
            // first, with "Don't show this window again", while
            // `ShowClipboardContentViewer`.
            MenuCommand::UploadClipboard => {
                if self.settings.settings.ShowClipboardContentViewer {
                    let w = dialogs::clipboard_upload::ClipboardUploadWindow::new(task, true);
                    self.open_dialog_window(event_loop, Box::new(w));
                    return true;
                }
                return self.run_upload_job(event_loop, &HotkeyType::ClipboardUpload, &task);
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
        self.run_upload_job(event_loop, &job, &task)
    }
}
