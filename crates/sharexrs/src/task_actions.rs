//! The app's side of the main window's task area: running a capture's
//! image task on its own thread (`TaskManager.Start`), what happens when it
//! completes (`TaskManager.Task_TaskCompleted`), the view settings kept in
//! `ApplicationConfig`, and the task commands that need the system
//! (`UploadInfoManager`'s open and copy, `DeleteSelectedFiles`).

use std::path::{Path, PathBuf};

use sharex_settings::TaskSettings;
use sharex_strings::sharex as s;

use crate::message_box::{DialogResult, MessageBox};
use crate::task_view::{FormatFilter, TaskAction, TaskCommand, UrlField, ViewSettings};
use crate::tasks::{RecentTask, TaskEvent, TaskInfo, UploadInfoStatus};
use crate::{App, DialogPurpose, UserEvent, capture_job, shell, sound};

/// A key press egui-winit would turn into a clipboard event (Ctrl+C,
/// Ctrl+V, Shift+Delete), as the task area's shortcut with its modifiers.
pub(super) fn clipboard_key(
    key: &winit::keyboard::Key,
    m: winit::keyboard::ModifiersState,
) -> Option<(egui::Key, egui::Modifiers)> {
    use winit::keyboard::{Key, NamedKey};
    let modifiers = egui::Modifiers { alt: m.alt_key(), ctrl: m.control_key(), shift: m.shift_key(), mac_cmd: false, command: m.control_key() };
    let k = match key {
        Key::Named(NamedKey::Delete) if m.shift_key() && !m.control_key() => egui::Key::Delete,
        Key::Character(c) if m.control_key() && c.eq_ignore_ascii_case("c") => egui::Key::C,
        Key::Character(c) if m.control_key() && c.eq_ignore_ascii_case("v") => egui::Key::V,
        _ => return None,
    };
    Some((k, modifiers))
}

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
                .clone()
                .unwrap_or_default(),
        }
    }

    /// At startup: the view settings, and `RecentTaskManager.InitItems`
    /// (the saved recent tasks, shown when the settings ask for it).
    /// The task area's settings from `ApplicationConfig` again (after
    /// Application settings changed them).
    pub(super) fn init_task_view_settings(&mut self) {
        let settings = self.view_settings();
        self.state.tasks.set_settings(settings);
        if let Some(w) = &self.main {
            w.window.request_redraw();
        }
    }

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

    pub(crate) fn store_recent_tasks(&mut self) {
        let c = &mut self.settings.settings;
        let recent = &self.state.tasks.manager.recent;
        c.RecentTasks = (c.RecentTasksSave && !recent.is_empty()).then(|| recent.iter().map(RecentTask::to_json).collect());
    }

    /// `UploadManager.RunImageTask`: the task shows in the main window at
    /// once (preparing), and its after capture tasks (and upload) run on a
    /// thread (`crate::upload`).
    /// `UploadManager.RunImageTask`: the after capture window first, when
    /// the task asks for it (`TaskHelpers.ShowAfterCaptureWindow`).
    pub(super) fn run_image_task(&mut self, job: capture_job::ImageTask) {
        // `QuickTaskMenu.ShowMenu()` first, with `ShowQuickTaskMenu`.
        if job.task.AfterCaptureJob.contains(sharex_settings::AfterCaptureTasks::ShowQuickTaskMenu) {
            self.pending_quick_task = Some(job);
            return;
        }
        self.run_image_task_skipping_quick_menu(job);
    }

    /// `RunImageTask(metadata, taskSettings, skipQuickTaskMenu: true)`.
    pub(crate) fn run_image_task_after_quick_menu(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop, job: capture_job::ImageTask) {
        self.run_image_task_skipping_quick_menu(job);
    }

    fn run_image_task_skipping_quick_menu(&mut self, job: capture_job::ImageTask) {
        if job.task.AfterCaptureJob.contains(sharex_settings::AfterCaptureTasks::ShowAfterCaptureWindow) {
            self.pending_dialogs.push(self.after_capture_window(job));
            return;
        }
        self.start_image_task(job);
    }

    /// `StartImageTask`: `WorkerTask.CreateImageUploaderTask` and `TaskManager.Start`.
    pub(crate) fn start_image_task(&mut self, job: capture_job::ImageTask) {
        let task = crate::upload::manager::create_image_uploader_task(job);
        self.start_upload_tasks(vec![task]);
    }

    /// `AfterCaptureWindowIntegration.Show(taskSettings, metadata, null)`.
    fn after_capture_window(&self, job: capture_job::ImageTask) -> Box<dyn crate::dialogs::Dialog> {
        use crate::dialogs::after_capture::{AfterCaptureWindow, Target};
        let task = job.task.clone();
        let image = job.metadata.image.clone();
        // `TaskHelpers.GetFileName(taskSettings, null, metadata)`: the name
        // the task would use, without its extension.
        let name = std::path::Path::new(&job.file_name).file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let can_upload = !self.settings.system.disable_upload && !self.settings.settings.DisableUpload;
        let window = self.uploads.config.with(|c| AfterCaptureWindow::new(Target::Image(Box::new(job)), task, Some(image), None, name, can_upload, c));
        Box::new(window)
    }

    /// The after capture window's answer: the task starts (`StartImageTask`
    /// with `result.FileName`), or a recording becomes a file task
    /// (`FinishRecording`).
    pub(crate) fn after_capture_accepted(&mut self, accepted: crate::dialogs::after_capture::Accepted) {
        use crate::dialogs::after_capture::Target;
        match accepted.target {
            Target::Image(mut job) => {
                job.task = accepted.task;
                if !accepted.file_name.is_empty() {
                    // `FileHelpers.AppendExtension(customFileName, "bmp")`.
                    job.file_name = format!("{}.bmp", accepted.file_name);
                }
                self.start_image_task(*job);
            }
            Target::Recording(mut path) => {
                let name = accepted.file_name;
                let current = path.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                if !name.is_empty() && !current.eq_ignore_ascii_case(&name) {
                    // `FileHelpers.RenameFile(path, customFileName + ext)`.
                    let ext = path.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
                    let to = path.with_file_name(format!("{name}{ext}"));
                    match std::fs::rename(&path, &to) {
                        Ok(()) => path = to,
                        Err(e) => eprintln!("couldn't rename {}: {e}", path.display()),
                    }
                }
                let mut ctx = self.new_task_context();
                if let Some(t) = crate::upload::manager::create_file_job_task(&path, accepted.task, &mut ctx) {
                    self.start_upload_tasks(vec![t]);
                }
            }
            Target::Preview => {}
        }
        self.update_progress_ui();
    }

    fn redraw_main(&self) {
        if let Some(w) = &self.main {
            w.window.request_redraw();
        }
    }

    /// A task's thread or a picture decoder reported back.
    pub(super) fn task_event(&mut self, event: TaskEvent) {
        match event {
            TaskEvent::ImageReady { id, image } => {
                self.state.tasks.owned_image(id, image);
                self.redraw_main();
                self.decode_task_images();
            }
            TaskEvent::Image { id, version, kind, image } => {
                if let Some(w) = &self.main {
                    self.state.tasks.image_ready(w.ctx(), id, version, kind, image);
                    w.window.request_redraw();
                }
            }
            event => {
                self.upload_task_event(&event);
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
            TaskAction::OpenImageViewer(path) => {
                // `ShowImagesInViewer`: the task list's image files, from this one.
                let images: Vec<std::path::PathBuf> = self
                    .state
                    .tasks
                    .manager
                    .tasks
                    .iter()
                    .rev()
                    .filter_map(|t| t.info.file_path.clone())
                    .filter(|p| p.is_file() && sharex_helpers::file_helpers::is_image_file(&p.to_string_lossy()))
                    .collect();
                let index = images.iter().position(|p| *p == path).unwrap_or(0);
                self.show_image_files(event_loop, &images, index);
            }
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
            TaskCommand::ResizeImages => {
                let files = paths(|s| s.is_image_file).iter().map(|p| p.to_string_lossy().into_owned()).collect();
                self.open_image_resizer(event_loop, files);
            }
            TaskCommand::CombineImages(horizontal) => {
                let files = paths(|s| s.is_image_file).iter().map(|p| p.to_string_lossy().into_owned()).collect();
                let o = if horizontal { sharex_tools::combiner::Orientation::Horizontal } else { sharex_tools::combiner::Orientation::Vertical };
                self.combine_images(files, o);
            }
            // `TaskHelpers.OpenVideoTrimmer(taskSettings, filePath)` /
            // `OpenAnimatedGifTrimmer(filePath, taskSettings)`, with the
            // default task settings (tasks here don't keep theirs).
            TaskCommand::TrimVideo if sel.is_video_file => {
                self.open_video_tool(event_loop, "MainMenuBuilder_VideoTrimmer", None, Some(path.to_string_lossy().into_owned()));
            }
            TaskCommand::TrimAnimatedGif if sel.is_file_exist && path.extension().is_some_and(|e| e.eq_ignore_ascii_case("gif")) => {
                self.open_video_tool(event_loop, "MainMenuBuilder_AnimatedGifTrimmer", None, Some(path.to_string_lossy().into_owned()));
            }
            TaskCommand::AddImageEffects if sel.is_image_file => {
                let task = TaskSettings::default_task_settings(&self.settings.settings.DefaultTaskSettings);
                self.open_image_effects_tool(event_loop, Some(path.clone()), task);
            }
            // `TaskHelpers.OpenImageBeautifier(filePath)`: default task settings.
            TaskCommand::BeautifyImage if sel.is_image_file => self.open_image_beautifier(event_loop, Some(path.clone()), None),
            TaskCommand::OcrImage if sel.is_image_file => {
                let task = TaskSettings::default_task_settings(&self.settings.settings.DefaultTaskSettings);
                self.ocr_file(event_loop, &path, task);
            }
            TaskCommand::OpenFile if sel.is_file_exist => {
                shell::open_file(&path);
            }
            TaskCommand::OpenFolder if sel.is_file_exist => {
                shell::open_folder_with_file(&path);
            }
            TaskCommand::TryOpen => {
                if sel.is_shortened_url_exist {
                    crate::upload::open_url(&first.result.shortened_url);
                } else if sel.is_url_exist {
                    crate::upload::open_url(&first.result.url);
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
            TaskCommand::PinToScreen if sel.is_image_file => self.pin_file(event_loop, &path, None),
            TaskCommand::EditImage if sel.is_image_file => self.annotate_file(event_loop, &path, None),
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
            TaskCommand::OpenUrl(field) => {
                let url = url_of(first, field);
                if !url.is_empty() {
                    crate::upload::open_url(&url);
                }
            }
            TaskCommand::CopyUrl(field) => self.copy_texts(infos.iter().map(|i| url_of(i, field)).filter(|u| !u.is_empty()).collect()),
            TaskCommand::CopyFormat(format, filter) => {
                let ok = |s: &UploadInfoStatus| match filter {
                    FormatFilter::Url => s.is_url_exist,
                    FormatFilter::Image => s.is_image_url,
                    FormatFilter::LinkedImage => s.is_image_url && s.is_thumbnail_url_exist,
                };
                let texts = infos.iter().zip(&statuses).filter(|(_, s)| ok(s)).map(|(i, _)| parse_info(i, format)).collect();
                self.copy_texts(texts);
            }
            TaskCommand::CopyCustomFormat(index) => {
                let format = self
                    .settings
                    .settings
                    .ClipboardContentFormats
                    .get(index)
                    .and_then(|f| f.get("Format"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_owned();
                if !format.is_empty() {
                    let texts = infos.iter().zip(&statuses).filter(|(_, s)| s.is_url_exist).map(|(i, _)| parse_info(i, &format)).collect();
                    self.copy_texts(texts);
                }
            }
            TaskCommand::OpenThumbnailFile if sel.is_thumbnail_file_exist => {
                if let Some(p) = &first.thumbnail_file_path {
                    shell::open_file(p);
                }
            }
            TaskCommand::CopyThumbnailFile if sel.is_thumbnail_file_exist => {
                if let Some(p) = first.thumbnail_file_path.clone() {
                    match sharex_capture::clipboard::copy_files(&[p], self.owner()) {
                        Ok(()) => self.action_completed(),
                        Err(e) => eprintln!("Clipboard copy file failed: {e}"),
                    }
                }
            }
            TaskCommand::CopyThumbnailImage if sel.is_thumbnail_file_exist => {
                let copied = first.thumbnail_file_path.as_deref().and_then(crate::thumbnail::load).map(|img| sharex_capture::clipboard::copy_image(&img, self.owner()));
                match copied {
                    Some(Ok(())) => self.action_completed(),
                    Some(Err(e)) => eprintln!("Clipboard copy image from file failed: {e}"),
                    None => {}
                }
            }
            // `UploadManager.UploadFile(path)` with the default task settings.
            TaskCommand::Upload if sel.is_file_exist => {
                let task = sharex_settings::TaskSettings::default_task_settings(&self.settings.settings.DefaultTaskSettings);
                self.upload_request(event_loop, crate::dialogs::UploadRequest::Files { files: vec![path], task, confirmed: false });
            }
            // `UploadManager.DownloadFile(url)`.
            TaskCommand::Download if sel.is_file_url => {
                let task = sharex_settings::TaskSettings::default_task_settings(&self.settings.settings.DefaultTaskSettings);
                let mut ctx = crate::upload::manager::NewTaskContext::new(
                    &mut self.settings.settings,
                    self.settings.paths.screenshots_parent_folder(None),
                    self.settings.paths.image_effects_folder(),
                );
                let t = crate::upload::manager::create_download_task(&first.result.url, false, task, &mut ctx);
                self.start_upload_tasks(t.into_iter().collect());
            }
            // `UploadManager.ShortenURL(url, urlShortener)` / `ShareURL`:
            // the result's text with the default task settings.
            TaskCommand::ShortenUrl(service) if sel.is_url_exist => {
                let mut task = sharex_settings::TaskSettings::default_task_settings(&self.settings.settings.DefaultTaskSettings);
                task.URLShortenerDestination = service;
                let t = crate::upload::manager::shorten_url(&first.text(), &task);
                self.start_upload_tasks(t.into_iter().collect());
            }
            TaskCommand::ShareUrl(service) if sel.is_url_exist => {
                let mut task = sharex_settings::TaskSettings::default_task_settings(&self.settings.settings.DefaultTaskSettings);
                task.URLSharingServiceDestination = service;
                let t = crate::upload::manager::share_url(&first.text(), &task);
                self.start_upload_tasks(t.into_iter().collect());
            }
            // `SearchImageUsingGoogleLensAsync` / `SearchImageUsingBingAsync`:
            // the sharing service's page with the URL.
            TaskCommand::SearchImage(service) if sel.is_url_exist => {
                let mut config = sharex_uploaders::UploadersConfig::default();
                if let Some(mut sharer) = sharex_uploaders::factory::create_url_sharing(&service, &mut config, &Default::default()) {
                    let _ = sharex_uploaders::uploader::share_url(sharer.as_mut(), &first.result.url);
                }
            }
            TaskCommand::ShowQrCode if sel.is_url_exist => {
                let w = crate::dialogs::qr_code::QrCodeWindow::new(Some(first.result.url.clone()));
                self.open_dialog_window(event_loop, Box::new(w));
            }
            // `ResponseWindow.ShowInstance(result)`: the open one shows it.
            TaskCommand::ShowResponse => {
                let data = response_data(first);
                if let Some(d) = self.dialogs.iter_mut().find(|d| d.dialog.single() == Some("response")) {
                    d.dialog.show_response(data);
                    d.window.window.set_minimized(false);
                    d.window.window.focus_window();
                    d.window.window.request_redraw();
                } else {
                    self.open_dialog_window(event_loop, Box::new(crate::dialogs::response::ResponseWindow::new(data)));
                }
            }
            // `WorkerTask.ShowErrorWindow`.
            TaskCommand::ShowErrors if first.result.is_error() => {
                let errors = first.result.errors.join("\r\n\r\n");
                if !errors.is_empty() {
                    // `AppPaths.LogsFilePath` (shown when it exists).
                    let log = Some(self.settings.paths.logs_folder().join(format!("ShareX-Log-{}.txt", chrono::Local::now().format("%Y-%m"))));
                    let w = crate::dialogs::response::ErrorWindow::new(s::UploadInfoManager_ShowErrors_Upload_errors().to_owned(), errors, log);
                    self.open_dialog_window(event_loop, Box::new(w));
                }
            }
            TaskCommand::DeleteSelectedFiles => self.delete_selected_files(event_loop, ids),
            TaskCommand::StopUpload => self.stop_tasks(&ids),
            TaskCommand::NotPorted(name) => eprintln!("{name}: not ported yet"),
            // `action.RunAsync(filePath)`, not awaited by anything.
            TaskCommand::RunAction(i) => {
                if let Some(action) = self.state.tasks.settings.external_programs.get(i).cloned() {
                    let path = path.to_string_lossy().into_owned();
                    std::thread::spawn(move || action.run(&path));
                }
            }
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

    /// `TaskHelpers.ConfirmFileDelete`, then `FileHelpers.DeleteFile(path,
    /// true)` (to the recycle bin).
    pub(crate) fn confirm_delete_path(&mut self, event_loop: &winit::event_loop::ActiveEventLoop, path: std::path::PathBuf) {
        if !self.settings.settings.ShowFileDeleteConfirmation {
            shell::delete_file_to_recycle_bin(&path);
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
        if let Err(e) = self.open_dialog(event_loop, message, DialogPurpose::DeletePath(path)) {
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

/// One of an upload's URLs.
fn url_of(info: &TaskInfo, field: UrlField) -> String {
    let r = &info.result;
    match field {
        UrlField::Url => r.url.clone(),
        UrlField::Shortened => r.shortened_url.clone(),
        UrlField::Thumbnail => r.thumbnail_url.clone(),
        UrlField::Deletion => r.deletion_url.clone(),
    }
}

/// `UploadInfoParser.Parse(info, pattern)` of a task.
fn parse_info(info: &TaskInfo, pattern: &str) -> String {
    let some = |s: &str| (!s.is_empty()).then(|| s.to_owned());
    let r = &info.result;
    let result = sharex_uploaders::UploadResult {
        url: some(&r.url),
        thumbnail_url: some(&r.thumbnail_url),
        deletion_url: some(&r.deletion_url),
        shortened_url: some(&r.shortened_url),
        ..Default::default()
    };
    let fields = crate::upload::after_upload::InfoFields {
        result: &result,
        file_name: &info.file_name,
        file_path: info.file_path.as_deref(),
        thumbnail_file_path: info.thumbnail_file_path.as_deref(),
        upload_duration: None,
    };
    crate::upload::after_upload::parse_upload_info(&fields, pattern)
}

/// What the response window shows of a task.
fn response_data(info: &TaskInfo) -> crate::dialogs::response::ResponseData {
    let r = &info.result;
    crate::dialogs::response::ResponseData {
        url: r.url.clone(),
        shortened_url: r.shortened_url.clone(),
        thumbnail_url: r.thumbnail_url.clone(),
        deletion_url: r.deletion_url.clone(),
        is_error: r.is_error(),
        errors: r.errors.join("\r\n\r\n"),
        response: r.response.clone(),
        response_info: r.response_info.as_deref().cloned(),
    }
}
