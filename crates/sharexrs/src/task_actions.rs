//! The app's side of the main window's task area: running a capture's
//! image task on its own thread (`TaskManager.Start`), what happens when it
//! completes (`TaskManager.Task_TaskCompleted`), the view settings kept in
//! `ApplicationConfig`, and the task commands that need the system
//! (`UploadInfoManager`'s open and copy, `DeleteSelectedFiles`).

use std::path::{Path, PathBuf};

use sharex_settings::TaskSettings;
use sharex_strings::sharex as s;

use crate::message_box::{DialogResult, MessageBox};
use crate::task_view::{TaskAction, TaskCommand, ViewSettings};
use crate::tasks::{RecentTask, TaskEvent, TaskInfo, TaskStatus, UploadInfoStatus};
use crate::{App, DialogPurpose, UserEvent, capture_job, shell, sound};

impl App {
    /// The task area's settings, from `ApplicationConfig`.
    fn view_settings(&self) -> ViewSettings {
        let c = &self.settings.settings;
        let defaults = &c.DefaultTaskSettings;
        ViewSettings {
            mode: c.TaskViewMode.clone(),
            show_columns: c.ShowColumns,
            column_widths: c.TaskListViewColumnWidths.clone(),
            image_preview: c.ImagePreview.clone(),
            preview_location: c.ImagePreviewLocation.clone(),
            preview_width: c.ImagePreviewWidth,
            preview_height: c.ImagePreviewHeight,
            show_title: c.ShowThumbnailTitle,
            title_location: c.ThumbnailTitleLocation.clone(),
            thumbnail_size: (c.ThumbnailSize.width, c.ThumbnailSize.height),
            click_action: c.ThumbnailClickAction.clone(),
            uploads_disabled: self.state.uploads_disabled,
            clipboard_formats: c
                .ClipboardContentFormats
                .iter()
                .filter_map(|f| f.get("Description").and_then(|d| d.as_str()).map(str::to_string))
                .collect(),
            external_programs: defaults
                .ExternalPrograms
                .as_ref()
                .is_some_and(|p| p.iter().any(|x| x.get("Name").and_then(|n| n.as_str()).is_some_and(|n| !n.is_empty()))),
        }
    }

    /// At startup: the view settings, and `RecentTaskManager.InitItems`
    /// (the saved recent tasks, shown when the settings ask for it).
    pub(super) fn init_task_view(&mut self) {
        let settings = self.view_settings();
        self.state.tasks.set_settings(settings);
        let c = &self.settings.settings;
        let max = c.RecentTasksMaxCount.clamp(1, 100) as usize;
        let recent: Vec<RecentTask> =
            c.RecentTasks.iter().flatten().take(max).map(RecentTask::from_json).collect();
        let show = c.RecentTasksSave && c.RecentTasksShowInMainWindow;
        self.state.tasks.manager.recent = recent.clone();
        if show {
            for r in &recent {
                let id = self.state.tasks.manager.start_history_task(r);
                self.state.tasks.add_task(id);
            }
        }
    }

    /// Writes the task area's settings back to `ApplicationConfig`, and the
    /// recent tasks (`RecentTasks`, kept only with `RecentTasksSave`). A new
    /// view mode is saved at once (`SwitchTaskViewMode`).
    fn store_view_settings(&mut self) {
        let v = self.state.tasks.settings.clone();
        let c = &mut self.settings.settings;
        let mode_changed = c.TaskViewMode != v.mode;
        c.TaskViewMode = v.mode;
        c.TaskListViewColumnWidths = v.column_widths;
        c.ImagePreviewWidth = v.preview_width;
        c.ImagePreviewHeight = v.preview_height;
        self.store_recent_tasks();
        if mode_changed && let Err(e) = self.settings.save_application_config(false) {
            eprintln!("couldn't save the settings: {e}");
        }
    }

    fn store_recent_tasks(&mut self) {
        let c = &mut self.settings.settings;
        let recent = &self.state.tasks.manager.recent;
        c.RecentTasks = (c.RecentTasksSave && !recent.is_empty()).then(|| recent.iter().map(RecentTask::to_json).collect());
    }

    /// `UploadManager.RunImageTask`: the task shows in the main window at
    /// once (preparing), and its after capture tasks run on a thread.
    pub(super) fn run_image_task(&mut self, job: capture_job::ImageTask) {
        let play_sound = job.task.GeneralSettings.as_ref().is_none_or(|g| g.PlaySoundAfterUpload);
        let id = self.state.tasks.manager.start_image_task(job.file_name.clone());
        self.state.tasks.add_task(id);
        eprintln!("Task status: Preparing");
        let owner = self.main.as_ref().and_then(|w| sharex_capture::platform::window_handle(&w.window));
        let proxy = self.proxy.clone();
        std::thread::spawn(move || {
            let (file_path, error) = match job.run(owner) {
                Ok(outcome) => (outcome.file_path, None),
                Err(e) => {
                    eprintln!("after capture tasks: {e}");
                    (None, Some(e))
                }
            };
            let _ = proxy.send_event(UserEvent::Task(TaskEvent::Completed { id, file_path, error, play_sound }));
        });
        self.redraw_main();
    }

    fn redraw_main(&self) {
        if let Some(w) = &self.main {
            w.window.request_redraw();
        }
    }

    /// A task's thread or a picture decoder reported back.
    pub(super) fn task_event(&mut self, event: TaskEvent) {
        match event {
            TaskEvent::Completed { id, file_path, error, play_sound } => {
                let max = self.settings.settings.RecentTasksMaxCount.clamp(1, 100) as usize;
                self.state.tasks.manager.complete(id, file_path, error, max);
                self.state.tasks.task_changed(id);
                if let Some(task) = self.state.tasks.manager.get(id) {
                    let info = &task.info;
                    let duration = info
                        .end_time
                        .zip(info.start_time)
                        .and_then(|(e, s)| e.duration_since(s).ok())
                        .map_or(0, |d| d.as_millis());
                    eprintln!("Task status: {:?}", task.status);
                    if task.status == TaskStatus::Failed {
                        eprintln!("Task failed. File name: {}, Errors:\n{}", info.file_name, info.result.errors.join("\n"));
                        if play_sound {
                            sound::play_error();
                        }
                    } else {
                        eprintln!("Task completed. File name: {}, Duration: {duration} ms", info.file_name);
                        if play_sound && !info.text().is_empty() {
                            sound::play_task_completed();
                        }
                    }
                }
                self.store_recent_tasks();
                let busy = self.state.tasks.manager.tasks.iter().any(|t| t.is_busy());
                if self.settings.settings.SaveSettingsAfterTaskCompleted
                    && !busy
                    && let Err(e) = self.settings.save_all()
                {
                    eprintln!("couldn't save the settings: {e}");
                }
                self.redraw_main();
            }
            TaskEvent::Image { id, version, kind, image } => {
                if let Some(w) = &self.main {
                    self.state.tasks.image_ready(w.ctx(), id, version, kind, image);
                    w.window.request_redraw();
                }
            }
        }
    }

    /// Decodes the pictures the task area asked for, each on its own thread.
    pub(super) fn decode_task_images(&mut self) {
        for request in std::mem::take(&mut self.state.tasks.requests) {
            let proxy = self.proxy.clone();
            std::thread::spawn(move || {
                let image = request.run();
                let event = TaskEvent::Image { id: request.id, version: request.version, kind: request.kind, image };
                let _ = proxy.send_event(UserEvent::Task(event));
            });
        }
    }

    pub(super) fn task_action(&mut self, event_loop: &winit::event_loop::ActiveEventLoop, action: TaskAction) {
        match action {
            TaskAction::Command(command, ids) => self.task_command(event_loop, command, ids),
            TaskAction::OpenImageViewer(path) => eprintln!("image viewer: not ported yet ({})", path.display()),
            TaskAction::OpenFile(path) => {
                shell::open_file(&path);
            }
            TaskAction::SettingsChanged => self.store_view_settings(),
            TaskAction::CloseWindow => {}
        }
        self.redraw_main();
    }

    fn owner(&self) -> Option<isize> {
        self.main.as_ref().and_then(|w| sharex_capture::platform::window_handle(&w.window))
    }

    /// `PlayNotificationSoundAsync(ActionCompleted)` with the default task settings.
    fn action_completed(&self) {
        let task = TaskSettings::default_task_settings(&self.settings.settings.DefaultTaskSettings);
        if task.GeneralSettings.as_ref().is_none_or(|g| g.PlaySoundAfterAction) {
            sound::play_action_completed();
        }
    }

    /// `CopyTexts`: the lines joined with CRLF.
    fn copy_texts(&self, texts: Vec<String>) {
        let text = texts.join("\r\n");
        if text.is_empty() {
            return;
        }
        match sharex_capture::clipboard::copy_text(&text, self.owner()) {
            Ok(()) => self.action_completed(),
            Err(e) => eprintln!("Clipboard copy text failed: {e}"),
        }
    }

    /// `UploadInfoManager`'s open and copy commands, and file deletion.
    fn task_command(&mut self, event_loop: &winit::event_loop::ActiveEventLoop, command: TaskCommand, ids: Vec<u64>) {
        let infos: Vec<TaskInfo> = ids.iter().filter_map(|id| self.state.tasks.manager.get(*id)).map(|t| t.info.clone()).collect();
        let statuses: Vec<UploadInfoStatus> = infos.iter().map(UploadInfoStatus::new).collect();
        let Some((first, sel)) = infos.first().zip(statuses.first()) else {
            if let TaskCommand::NotPorted(name) = command {
                eprintln!("{name}: not ported yet");
            }
            return;
        };
        let path = first.file_path.clone().unwrap_or_default();
        // The selected items' paths that pass `ok`.
        let paths = |ok: fn(&UploadInfoStatus) -> bool| -> Vec<PathBuf> {
            infos.iter().zip(&statuses).filter(|(_, s)| ok(s)).filter_map(|(i, _)| i.file_path.clone()).collect()
        };
        match command {
            TaskCommand::OpenFile if sel.is_file_exist => {
                shell::open_file(&path);
            }
            TaskCommand::OpenFolder if sel.is_file_exist => {
                shell::open_folder_with_file(&path);
            }
            TaskCommand::TryOpen => {
                if sel.is_shortened_url_exist || sel.is_url_exist {
                    eprintln!("Open URL: not ported yet");
                } else if sel.is_file_path_valid {
                    shell::open_file(&path);
                }
            }
            TaskCommand::CopyFile if sel.is_file_exist => match sharex_capture::clipboard::copy_files(&[path], self.owner()) {
                Ok(()) => self.action_completed(),
                Err(e) => eprintln!("Clipboard copy file failed: {e}"),
            },
            TaskCommand::CopyImage if sel.is_image_file => {
                let copied = crate::thumbnail::load(&path).map(|img| sharex_capture::clipboard::copy_image(&img, self.owner()));
                match copied {
                    Some(Ok(())) => self.action_completed(),
                    Some(Err(e)) => eprintln!("Clipboard copy image from file failed: {e}"),
                    None => {}
                }
            }
            TaskCommand::CopyImageDimensions if sel.is_image_file => {
                if let Ok((w, h)) = image::image_dimensions(&path) {
                    self.copy_texts(vec![format!("{w} x {h}")]);
                }
            }
            TaskCommand::CopyText if sel.is_text_file => {
                if let Ok(text) = std::fs::read_to_string(&path) {
                    self.copy_texts(vec![text]);
                }
            }
            TaskCommand::CopyFilePath => {
                self.copy_texts(paths(|s| s.is_file_path_valid).iter().map(|p| p.to_string_lossy().into_owned()).collect());
            }
            TaskCommand::CopyFileName => self.copy_texts(
                paths(|s| s.is_file_path_valid)
                    .iter()
                    .map(|p| p.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default())
                    .collect(),
            ),
            TaskCommand::CopyFileNameWithExtension => self.copy_texts(
                paths(|s| s.is_file_path_valid)
                    .iter()
                    .map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default())
                    .collect(),
            ),
            TaskCommand::CopyFolder => self.copy_texts(
                paths(|s| s.is_file_path_valid)
                    .iter()
                    .map(|p| p.parent().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default())
                    .collect(),
            ),
            TaskCommand::TryCopy => {
                if sel.is_shortened_url_exist {
                    self.copy_texts(infos.iter().map(|i| i.result.shortened_url.clone()).filter(|u| !u.is_empty()).collect());
                } else if sel.is_url_exist {
                    self.copy_texts(infos.iter().map(|i| i.result.url.clone()).filter(|u| !u.is_empty()).collect());
                } else if sel.is_file_path_valid {
                    self.copy_texts(paths(|s| s.is_file_path_valid).iter().map(|p| p.to_string_lossy().into_owned()).collect());
                }
            }
            TaskCommand::DeleteSelectedFiles => self.delete_selected_files(event_loop, ids),
            TaskCommand::NotPorted(name) => eprintln!("{name}: not ported yet"),
            // Carried out by the task area itself, or not applicable.
            _ => {}
        }
    }

    /// `DeleteSelectedFiles`: after `TaskHelpers.ConfirmFileDelete`.
    fn delete_selected_files(&mut self, event_loop: &winit::event_loop::ActiveEventLoop, ids: Vec<u64>) {
        if !self.settings.settings.ShowFileDeleteConfirmation {
            self.delete_files(&ids);
            return;
        }
        if self.dialog.is_some() {
            return;
        }
        let message = MessageBox::yes_no_with_check_box(
            format!("ShareXrs - {}", s::MainForm_tsmiDeleteSelectedFile_Click_File_delete_confirmation()),
            s::MainForm_tsmiDeleteSelectedFile_Click_Do_you_really_want_to_delete_this_file_(),
            s::FileDeleteConfirmation_DontRemindAgain(),
        );
        if let Err(e) = self.open_dialog(event_loop, message, DialogPurpose::DeleteFiles(ids)) {
            eprintln!("couldn't open the message box: {e}");
        }
    }

    /// The answer to `ConfirmFileDelete`.
    pub(super) fn file_delete_answered(&mut self, ids: Vec<u64>, result: DialogResult, dont_remind: bool) {
        if result != DialogResult::Yes {
            return;
        }
        if dont_remind {
            self.settings.settings.ShowFileDeleteConfirmation = false;
        }
        self.delete_files(&ids);
    }

    /// `UploadInfoManager.DeleteFiles` (to the recycle bin), then
    /// `RemoveSelectedTasks`.
    fn delete_files(&mut self, ids: &[u64]) {
        for id in ids {
            if let Some(path) = self.state.tasks.manager.get(*id).and_then(|t| t.info.file_path.clone()) {
                shell::delete_file_to_recycle_bin(Path::new(&path));
            }
        }
        self.state.tasks.remove_tasks(ids);
        self.store_recent_tasks();
        self.redraw_main();
    }
}
