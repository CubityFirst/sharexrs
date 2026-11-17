//! The upload half of `ShareX/WorkerTask.cs`, run on the task's own thread
//! (`ThreadDoWork`): the job before the upload (after capture tasks, saving
//! the text, downloading), `DoUploadJob` with its retries, `DoUpload` with
//! the uploader filters, `UploadData`, then `DoAfterUploadJobs`
//! (`super::after_upload`). What it reports goes to the event loop as
//! [`TaskEvent`]s; the task's end is a [`TaskReport`].

use std::io::Cursor;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use sharex_settings::{AfterCaptureTasks, AfterUploadTasks, ApplicationConfig, TaskSettings};
use sharex_strings::sharex as s;
use sharex_uploaders::factory::{Ported, TaskReferenceHelper};
use sharex_uploaders::http::{self, ProgressManager, SharedStream};
use sharex_uploaders::{FileDestination, ImageDestination, TextDestination, UploadResult, UploaderErrorManager};

use super::filters::{Service, find_filter_service};
use super::{EventSink, StopHandle, TaskUi, UploadersConfigStore};
use crate::capture_job::{self, ImageTask};
use crate::tasks::{self, EDataType, TaskEvent, TaskJob, TaskReport, TaskStatus, UploadProgress};

/// What every task's thread shares: the uploaders' settings, the desktop,
/// where to report, and the application settings it reads.
#[derive(Clone)]
pub struct UploadContext {
    /// `ApplicationState.UploadersConfig`.
    pub config: UploadersConfigStore,
    pub ui: Arc<dyn TaskUi>,
    pub events: EventSink,
    /// `ApplicationState.Settings`, as the task started.
    pub app: ApplicationConfig,
    /// `SystemOptions.DisableUpload`.
    pub system_disable_upload: bool,
    /// `AppPaths.ScreenshotsParentFolder`.
    pub screenshots_parent: PathBuf,
    /// The window that owns what an image task copies to the clipboard.
    pub owner: Option<isize>,
    /// The pause before a retry (`Thread.Sleep(1000)`).
    pub retry_delay: Duration,
    /// `Uploader.RequestTimeout`: ShareX's task leaves it infinite (`None`);
    /// tests set one to see a timeout fail a task.
    pub request_timeout: Option<Duration>,
}

/// What a task uploads, as `WorkerTask`'s constructors leave it.
pub enum Source {
    /// `CreateImageUploaderTask`: the capture, through the after capture tasks.
    Image(Box<ImageTask>),
    /// `CreateFileUploaderTask`: the file, opened (`LoadFileStream`).
    File(std::fs::File),
    /// `CreateTextUploaderTask`.
    Text(String),
    /// `CreateURLShortenerTask`, `CreateShareURLTask`, `CreateDownloadTask`:
    /// the URL is the result's.
    Url,
}

/// A task to start: `WorkerTask` as its constructors make it.
pub struct UploadTask {
    pub job: TaskJob,
    pub data_type: EDataType,
    pub file_name: String,
    pub file_path: Option<PathBuf>,
    /// `Info.Result.URL` at the start (URL tasks).
    pub url: String,
    pub task: TaskSettings,
    pub source: Source,
}

impl UploadTask {
    /// The `TaskInfo` the task list starts with.
    pub fn info(&self) -> tasks::TaskInfo {
        let mut info = tasks::TaskInfo { job: self.job, ..Default::default() };
        info.set_file_path(self.file_path.clone());
        info.file_name = self.file_name.clone();
        info.result.url = self.url.clone();
        info.result.url_expected = true;
        info
    }
}

fn is_upload_job(job: TaskJob, task: &TaskSettings) -> bool {
    match job {
        TaskJob::Job => task.AfterCaptureJob.contains(AfterCaptureTasks::UploadImageToHost),
        TaskJob::Download => false,
        _ => true,
    }
}

/// Runs `task` to its end on this thread (`ThreadDoWork`) and gives what
/// `OnTaskCompleted` needs.
pub fn run(task: UploadTask, id: u64, ctx: &UploadContext, stop: &StopHandle) -> TaskReport {
    let mut w = Worker::new(task, id, ctx, stop);
    w.thread_do_work();
    w.report()
}

/// What `DoThreadJob` decided.
enum Next {
    Continue,
    /// `return false`: the task stops.
    Stop,
    /// An after capture task failed (an error, the task fails).
    Fail,
}

pub(super) struct Worker<'a> {
    pub(super) id: u64,
    pub(super) ctx: &'a UploadContext,
    pub(super) stop: &'a StopHandle,
    pub(super) task: TaskSettings,
    pub(super) job: TaskJob,
    pub(super) data_type: EDataType,
    pub(super) file_name: String,
    pub(super) file_path: Option<PathBuf>,
    /// `Info.Result`.
    pub(super) result: UploadResult,
    /// `Info.UploadDuration`.
    pub(super) upload_duration: Option<Duration>,
    source: Option<Source>,
    /// `Data`.
    data: Option<SharedStream>,
    data_length: u64,
    /// `uploader.Errors` of the last uploader made.
    uploader_errors: Option<UploaderErrorManager>,
    /// `EarlyURLCopied`.
    early_url_copied: Arc<AtomicBool>,
    pub(super) task_ref: TaskReferenceHelper,
    /// `Info.Metadata`'s window title and process name (image tasks), the
    /// history item's tags.
    metadata: (Option<String>, Option<String>),
    /// `Info.Metadata.Image`, for the before and after upload windows.
    image: Option<Arc<image::RgbaImage>>,
}

impl<'a> Worker<'a> {
    fn new(t: UploadTask, id: u64, ctx: &'a UploadContext, stop: &'a StopHandle) -> Self {
        let result = UploadResult { url: (!t.url.is_empty()).then(|| t.url.clone()), ..Default::default() };
        let advanced = t.task.AdvancedSettings.clone().unwrap_or_default();
        let metadata = match &t.source {
            Source::Image(image) => (image.metadata.window_title.clone(), image.metadata.process_name.clone()),
            _ => (None, None),
        };
        // `CreateTaskReferenceHelper`.
        let task_ref = TaskReferenceHelper {
            stop_requested: false,
            override_ftp: t.task.OverrideFTP,
            ftp_index: t.task.FTPIndex,
            override_custom_uploader: t.task.OverrideCustomUploader,
            custom_uploader_index: t.task.CustomUploaderIndex,
            text_format: advanced.TextFormat,
        };
        Self {
            id,
            ctx,
            stop,
            task: t.task,
            job: t.job,
            data_type: t.data_type,
            file_name: t.file_name,
            file_path: t.file_path,
            result,
            upload_duration: None,
            source: Some(t.source),
            data: None,
            data_length: 0,
            uploader_errors: None,
            early_url_copied: Arc::default(),
            task_ref,
            metadata,
            image: None,
        }
    }

    pub(super) fn stop_requested(&self) -> bool {
        self.stop.is_requested()
    }

    pub(super) fn emit(&self, event: TaskEvent) {
        (self.ctx.events)(event);
    }

    fn status(&self, status: TaskStatus, text: &str) {
        self.emit(TaskEvent::Status { id: self.id, status, text: text.to_owned() });
    }

    fn file_changed(&self) {
        self.emit(TaskEvent::File { id: self.id, file_name: self.file_name.clone(), file_path: self.file_path.clone() });
    }

    fn is_upload_job(&self) -> bool {
        is_upload_job(self.job, &self.task)
    }

    /// `ThreadDoWork`.
    fn thread_do_work(&mut self) {
        let next = self.do_thread_job();
        if matches!(next, Next::Stop) {
            self.stop.stop();
        }
        // `OnImageReady`: the task list shows the saved file.
        self.file_changed();
        if !self.stop_requested() {
            if !matches!(next, Next::Fail) && self.is_upload_job() && self.is_upload_allowed() {
                self.do_upload_job();
            } else {
                self.result.is_url_expected = false;
            }
        }
        // finally: `Dispose()`, then the early copied URL goes if nothing came of it.
        self.data = None;
        self.source = None;
        if self.early_url_copied.load(Ordering::SeqCst)
            && (self.stop_requested() || self.result.url.as_deref().is_none_or(str::is_empty))
            && self.ctx.ui.clipboard_contains_text()
        {
            self.ctx.ui.clear_clipboard();
        }
        let advanced = self.task.AdvancedSettings.clone().unwrap_or_default();
        if (self.job == TaskJob::Job || (self.job == TaskJob::FileUpload && advanced.UseAfterCaptureTasksDuringFileUpload))
            && self.task.AfterCaptureJob.contains(AfterCaptureTasks::DeleteFile)
            && let Some(path) = self.file_path.as_ref().filter(|p| p.is_file())
            && let Err(e) = std::fs::remove_file(path)
        {
            eprintln!("Delete file failed: {}: {e}", path.display());
        }
        if !self.stop_requested() && self.result.is_url_expected && !self.result.is_error() {
            if self.result.url.as_deref().is_none_or(str::is_empty) {
                self.result.errors.add(s::UploadTask_ThreadDoWork_URL_is_empty_());
            } else {
                self.do_after_upload_jobs();
            }
        }
    }

    /// `TaskHelpers.IsUploadAllowed`. ShareX explains in a message box.
    fn is_upload_allowed(&self) -> bool {
        if self.ctx.system_disable_upload {
            eprintln!("{}", s::YourSystemAdminDisabledTheUploadFeature());
            return false;
        }
        if self.ctx.app.DisableUpload {
            eprintln!("{}", s::ThisFeatureWillNotWorkWhenDisableUploadOptionIsEnabled());
            return false;
        }
        true
    }

    /// `DoThreadJob`.
    fn do_thread_job(&mut self) -> Next {
        let advanced = self.task.AdvancedSettings.clone().unwrap_or_default();
        if self.is_upload_job() && advanced.AutoClearClipboard {
            self.ctx.ui.clear_clipboard();
        }
        if matches!(self.job, TaskJob::Download | TaskJob::DownloadUpload) {
            if !self.download_from_url(self.job == TaskJob::DownloadUpload) {
                return Next::Stop;
            } else if self.job == TaskJob::Download {
                return Next::Continue;
            }
        }
        match self.source.take() {
            Some(Source::Image(image)) if self.job == TaskJob::Job => {
                // `DoAfterCaptureJobs` (ported in `capture_job`).
                match image.run(self.ctx.owner) {
                    Ok(outcome) => {
                        if let Some(name) = outcome.file_name {
                            self.file_name = name;
                        }
                        if outcome.file_path.is_some() {
                            self.file_path = outcome.file_path;
                        }
                        if let Some(data) = outcome.data {
                            self.set_data(data);
                        }
                        self.image = outcome.image.map(Arc::new);
                    }
                    Err(e) => {
                        eprintln!("after capture tasks: {e}");
                        self.result.errors.add(e);
                        return Next::Fail;
                    }
                }
                // `DoFileJobs` (actions, copy file/path, show in Explorer) isn't ported yet.
            }
            Some(Source::Text(text)) => {
                if self.job == TaskJob::TextUpload && !text.is_empty() {
                    self.do_text_jobs(&text);
                }
            }
            Some(Source::File(file)) => {
                self.data_length = file.metadata().map(|m| m.len()).unwrap_or(0);
                self.data = Some(http::shared_stream(file));
            }
            other => self.source = other,
        }
        if self.task.AfterCaptureJob.contains(AfterCaptureTasks::DoOCR) {
            eprintln!("OCR isn't ported yet.");
        }
        Next::Continue
    }

    fn set_data(&mut self, data: Vec<u8>) {
        self.data_length = data.len() as u64;
        self.data = Some(http::shared_stream(Cursor::new(data)));
    }

    /// `DoTextJobs`: saved as a file first (`TextTaskSaveAsFile`), as UTF-8
    /// with a byte order mark (`File.WriteAllText(…, Encoding.UTF8)`); the
    /// upload is the text's UTF-8 bytes.
    fn do_text_jobs(&mut self, text: &str) {
        let advanced = self.task.AdvancedSettings.clone().unwrap_or_default();
        if advanced.TextTaskSaveAsFile {
            let folder = capture_job::screenshots_folder(&self.task, &self.ctx.app, &self.ctx.screenshots_parent, &no_metadata());
            if let Some(path) = capture_job::handle_exists_file(&folder.join(&self.file_name), &self.task) {
                let mut bytes = vec![0xEF, 0xBB, 0xBF];
                bytes.extend_from_slice(text.as_bytes());
                let written = path.parent().map_or(Ok(()), std::fs::create_dir_all).and_then(|()| std::fs::write(&path, bytes));
                match written {
                    Ok(()) => {
                        eprintln!("Text saved to file: {}", path.display());
                        self.file_path = Some(path);
                    }
                    Err(e) => eprintln!("Text save failed: {}: {e}", path.display()),
                }
            }
        }
        self.set_data(text.as_bytes().to_vec());
    }

    /// `DownloadFromURL`: the file name from the server (unless the name
    /// pattern is used), the screenshots folder, then the download.
    fn download_from_url(&mut self, upload: bool) -> bool {
        let url = self.result.url.take().unwrap_or_default().trim().to_owned();
        self.result.url = Some(String::new());
        let upload_settings = self.task.UploadSettings.clone().unwrap_or_default();
        if !upload_settings.FileUploadUseNamePattern
            && let Some(name) = file_name_from_web_server(&url)
        {
            self.file_name = sharex_helpers::file_helpers::sanitize_file_name(&name);
        }
        let folder = capture_job::screenshots_folder(&self.task, &self.ctx.app, &self.ctx.screenshots_parent, &no_metadata());
        let Some(path) = capture_job::handle_exists_file(&folder.join(&self.file_name), &self.task) else { return false };
        self.file_path = Some(path.clone());
        self.status(TaskStatus::Preparing, s::UploadTask_DownloadAndUpload_Downloading());
        self.file_changed();
        match download_file(&url, &path) {
            Ok(()) => {
                if upload {
                    // `LoadFileStream`.
                    match std::fs::File::open(&path) {
                        Ok(file) => {
                            self.data_length = file.metadata().map(|m| m.len()).unwrap_or(0);
                            self.data = Some(http::shared_stream(file));
                        }
                        Err(e) => eprintln!("{}: {e}", path.display()),
                    }
                }
                true
            }
            Err(e) => {
                // ShareX shows this in a message box.
                eprintln!("{}", s::UploadManager_DownloadAndUploadFile_Download_failed().replace("{0}", &e));
                false
            }
        }
    }

    /// `DoUploadJob`.
    fn do_upload_job(&mut self) {
        let app = &self.ctx.app;
        if app.ShowLargeFileSizeWarning > 0 {
            let n = app.ShowLargeFileSizeWarning as u64;
            let size = if app.BinaryUnits { n * 1024 * 1024 } else { n * 1000 * 1000 };
            if self.data.is_some() && self.data_length > size {
                let (go_on, dont_show_again) = self.ctx.ui.large_file_warning();
                if dont_show_again {
                    self.emit(TaskEvent::LargeFileWarningOff);
                }
                if !go_on {
                    self.stop.stop();
                }
            }
        }
        if self.stop_requested() {
            return;
        }
        // `SettingManager.WaitUploadersConfig()`.
        drop(self.ctx.config.wait());
        self.status(TaskStatus::Working, s::UploadTask_DoUploadJob_Uploading());
        if self.task.AfterCaptureJob.contains(AfterCaptureTasks::ShowBeforeUploadWindow) {
            let request = super::BeforeUpload {
                data_type: self.data_type,
                file_name: self.file_name.clone(),
                file_path: self.file_path.clone(),
                image: self.image.clone(),
                task: self.task.clone(),
            };
            match self.ctx.ui.before_upload(request) {
                Some(task) => self.task = task,
                // `cancelUpload`: nothing is uploaded, the task goes on.
                None => return,
            }
        }
        let mut status = format!("Upload started. File name: {}", self.file_name);
        if let Some(p) = &self.file_path {
            status += &format!(", File path: {}", p.display());
        }
        eprintln!("{status}");
        self.emit(TaskEvent::UploadStarted { id: self.id });
        let mut is_error = self.do_upload(0);
        let retries = self.ctx.app.MaxUploadFailRetry;
        let mut retry = 1;
        while is_error && retries > 0 && !self.stop_requested() && retry <= retries {
            eprintln!("Upload failed. Retrying upload.");
            is_error = self.do_upload(retry);
            retry += 1;
        }
        if !is_error && !self.result.is_error() {
            let url = self.result.to_string();
            if !url.is_empty() {
                let mut text = s::TaskManager_UploadCompletedURL().replace("{0}", &url);
                if let Some(d) = self.upload_duration {
                    text += &format!(", Duration: {} ms", d.as_millis());
                }
                eprintln!("{text}");
            }
        }
    }

    /// `DoUpload`: true when it failed.
    fn do_upload(&mut self, retry: i32) -> bool {
        let mut is_error = false;
        if retry > 0 {
            std::thread::sleep(self.ctx.retry_delay);
        }
        let outcome = match self.check_upload_filters() {
            Some(outcome) => Some(outcome),
            None => self.upload_destination_service().map(|service| self.upload_data(&service)),
        };
        match outcome {
            Some(Ok(result)) => self.result = result.unwrap_or_default(),
            // An exception, unless the task was stopped.
            Some(Err(e)) if !self.stop_requested() => {
                eprintln!("Upload exception: {e}");
                is_error = true;
                self.result.errors.add(e);
            }
            Some(Err(_)) | None => {}
        }
        if let Some(errors) = &self.uploader_errors {
            self.result.errors.extend(errors);
        }
        is_error | self.result.is_error()
    }

    /// `Info.UploadDestination` and the service `UploadImage`/`UploadText`/
    /// `UploadFile` pick for it (none for URL tasks).
    fn upload_destination_service(&self) -> Option<Service> {
        let t = &self.task;
        let destination = match self.data_type {
            EDataType::Image if t.ImageDestination == ImageDestination::FileUploader => EDataType::File,
            EDataType::Text if t.TextDestination == TextDestination::FileUploader => EDataType::File,
            d => d,
        };
        match destination {
            EDataType::Image => Some(Service::Image(t.ImageDestination.clone())),
            EDataType::Text => Some(Service::Text(t.TextDestination.clone())),
            EDataType::File => Some(Service::File(self.file_destination_by_data_type())),
            _ => None,
        }
    }

    /// `TaskSettings.GetFileDestinationByDataType`.
    fn file_destination_by_data_type(&self) -> FileDestination {
        match self.data_type {
            EDataType::Image => self.task.ImageFileDestination.clone(),
            EDataType::Text => self.task.TextFileDestination.clone(),
            _ => self.task.FileDestination.clone(),
        }
    }

    /// `CheckUploadFilters`: the first filter matching the file name uploads
    /// it instead, when its service exists.
    fn check_upload_filters(&mut self) -> Option<Result<Option<UploadResult>, String>> {
        self.data.as_ref()?;
        let filters = self.task.UploadSettings.as_ref().map(|u| u.UploaderFilters.clone()).unwrap_or_default();
        let service = find_filter_service(&filters, &self.file_name)??;
        Some(self.upload_data(&service))
    }

    /// `UploadData`: `CheckConfig`, `CreateUploader`, the upload, and the
    /// settings the uploader changed written back (a refreshed token).
    fn upload_data(&mut self, service: &Service) -> Result<Option<UploadResult>, String> {
        let name = service.name();
        let mut uploader = {
            let mut guard = self.ctx.config.wait();
            let config = guard.as_mut().expect("loaded");
            match service.check(config) {
                Ported::Yes(true) => {}
                Ported::Yes(false) => return Ok(Some(self.invalid_config_result(&name))),
                Ported::No => return Ok(Some(not_ported_result(&name))),
            }
            match service.create(config, &self.task_ref) {
                Some(u) => u,
                None => return Ok(None),
            }
        };
        let base = uploader.uploader();
        base.errors.default_title = Some(s::WorkerTask_ErrorTitle().replace("{0}", &name));
        base.buffer_size = 2usize.pow(self.ctx.app.BufferSizePower.clamp(0, 20) as u32) * 1024;
        if self.ctx.request_timeout.is_some() {
            base.request_timeout = self.ctx.request_timeout;
        }
        let token = base.stop_handle();
        {
            let (events, id, stop, token) = (self.ctx.events.clone(), self.id, self.stop.clone(), token.clone());
            base.on_progress(Arc::new(move |p: &ProgressManager| {
                // A stop that came before the uploader reset its token.
                if stop.is_requested() {
                    token.cancel();
                }
                events(TaskEvent::Progress { id, progress: progress_of(p) });
            }));
        }
        let advanced = self.task.AdvancedSettings.clone().unwrap_or_default();
        if self.task.AfterUploadJob.contains(AfterUploadTasks::CopyURLToClipboard) && advanced.EarlyCopyURL {
            let (ui, copied) = (self.ctx.ui.clone(), self.early_url_copied.clone());
            base.on_early_url_copy(Arc::new(move |url| {
                ui.copy_text(url);
                copied.store(true, Ordering::SeqCst);
            }));
        }
        let mut file_name = remove_bidi_control_characters(&self.file_name);
        if self.task.UploadSettings.as_ref().is_some_and(|u| u.FileUploadReplaceProblematicCharacters) {
            file_name = replace_reserved_characters(&file_name, "_");
        }
        self.stop.set_uploader(Some(token));
        let started = Instant::now();
        let result = match &self.data {
            Some(data) => uploader.upload(data, &file_name),
            None => Err("Value cannot be null. (Parameter 'stream')".to_owned()),
        };
        self.upload_duration = Some(started.elapsed());
        self.stop.set_uploader(None);
        self.ctx.config.with(|config| uploader.write_back(config));
        self.uploader_errors = Some(uploader.uploader().errors.clone());
        result
    }

    /// `GetInvalidConfigResult`: the error, and Destination settings asked
    /// to open on the service.
    pub(super) fn invalid_config_result(&self, service_name: &str) -> UploadResult {
        let mut ur = UploadResult::default();
        let message = s::WorkerTask_GetInvalidConfigResult__0__configuration_is_invalid_or_missing__Please_check__Destination_settings__window_to_configure_it_()
            .replace("{0}", service_name);
        eprintln!("{message}");
        ur.errors.add(message);
        self.emit(TaskEvent::DestinationSettingsRequested { service: service_name.to_owned() });
        ur
    }

    /// The task's end as `OnTaskCompleted` reads it.
    fn report(self) -> TaskReport {
        let r = &self.result;
        let general = self.task.GeneralSettings.clone().unwrap_or_default();
        // `TaskInfo.GetHistoryItem`, for `Task_TaskCompleted` to add to history.
        let history = crate::history::history_item(&crate::history::TaskEnd {
            file_name: &self.file_name,
            file_path: self.file_path.as_deref(),
            data_type: self.data_type,
            host: crate::history::uploader_host(self.job, self.data_type, &self.task, self.is_upload_job()),
            result: r,
            window_title: self.metadata.0.as_deref(),
            process_name: self.metadata.1.as_deref(),
        });
        TaskReport {
            file_name: self.file_name.clone(),
            file_path: self.file_path.clone(),
            result: tasks::UploadResult {
                url: r.url.clone().unwrap_or_default(),
                thumbnail_url: r.thumbnail_url.clone().unwrap_or_default(),
                deletion_url: r.deletion_url.clone().unwrap_or_default(),
                shortened_url: r.shortened_url.clone().unwrap_or_default(),
                response: r.response.clone().unwrap_or_default(),
                errors: r.errors.errors.iter().map(|e| e.text.clone()).collect(),
                error_title: r.errors.errors.first().map(|e| e.title.clone()).unwrap_or_default(),
                url_expected: r.is_url_expected,
            },
            stop_requested: self.stop_requested(),
            is_upload_job: self.is_upload_job(),
            play_sound: general.PlaySoundAfterUpload,
            show_after_upload_window: self.task.AfterUploadJob.contains(AfterUploadTasks::ShowAfterUploadWindow),
            data_type: self.data_type,
            image: self.image.clone().filter(|_| self.task.AfterUploadJob.contains(AfterUploadTasks::ShowAfterUploadWindow)),
            auto_close_after_upload_form: self.task.AdvancedSettings.as_ref().is_some_and(|a| a.AutoCloseAfterUploadForm),
            show_toast: general.ShowToastNotificationAfterTaskCompleted,
            upload_duration: self.upload_duration,
            history: Some(history),
        }
    }
}

/// A destination `sharex-uploaders` doesn't have yet: ShareX would upload;
/// ShareXrs fails the task saying so.
pub(super) fn not_ported_result(service_name: &str) -> UploadResult {
    let mut ur = UploadResult::default();
    ur.errors.add(format!("{service_name} isn't ported to ShareXrs yet."));
    ur
}

pub(super) fn progress_of(p: &ProgressManager) -> UploadProgress {
    UploadProgress { position: p.position, length: p.length, speed: p.speed, elapsed: p.elapsed(), remaining: p.remaining() }
}

/// `new TaskMetadata()`: no image, no window.
fn no_metadata() -> capture_job::Metadata {
    capture_job::Metadata { image: image::RgbaImage::new(0, 0), window_title: None, process_name: None }
}

/// `URLHelpers.RemoveBidiControlCharacters`.
pub fn remove_bidi_control_characters(text: &str) -> String {
    const BIDI: [char; 7] = ['\u{200E}', '\u{200F}', '\u{202A}', '\u{202B}', '\u{202C}', '\u{202D}', '\u{202E}'];
    text.chars().filter(|c| !BIDI.contains(c)).collect()
}

/// `URLHelpers.ReplaceReservedCharacters`: anything but `URLCharacters`
/// becomes `replace`, runs of them once.
pub fn replace_reserved_characters(text: &str, replace: &str) -> String {
    let mut sb = String::new();
    let mut last: Option<String> = None;
    for c in text.chars() {
        let s = if c.is_ascii_alphanumeric() || "-._~".contains(c) {
            c.to_string()
        } else if last.as_deref() != Some(replace) {
            replace.to_owned()
        } else {
            continue;
        };
        sb.push_str(&s);
        last = Some(s);
    }
    sb
}

/// `WebHelpers.GetFileNameFromWebServerAsync`: the `filename="…"` of a HEAD
/// request's `Content-Disposition`.
fn file_name_from_web_server(url: &str) -> Option<String> {
    // ShareX's `HttpMethod` has no HEAD; .NET's does.
    let method = sharex_uploaders::HttpMethod::Unknown("HEAD".into());
    let request = http::HttpRequest { method, url: url.to_owned(), headers: Vec::new(), content: None };
    let response = http::send(request, http::RequestOptions { read_body: false, ..Default::default() }).ok()?;
    let disposition = response.header("Content-Disposition")?;
    let marker = "filename=\"";
    let begin = disposition.to_ascii_lowercase().find(marker)? + marker.len();
    let rest = &disposition[begin..];
    Some(rest[..rest.find('"')?].to_owned())
}

/// `WebHelpers.DownloadFileAsync`: a GET written to `path` when it
/// succeeds (nothing is written otherwise, as ShareX).
fn download_file(url: &str, path: &std::path::Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let request = http::HttpRequest { method: sharex_uploaders::HttpMethod::GET, url: url.to_owned(), headers: Vec::new(), content: None };
    let response = http::send(request, http::RequestOptions::default()).map_err(|e| e.to_string())?;
    if response.is_success() {
        std::fs::write(path, response.response_data.unwrap_or_default()).map_err(|e| e.to_string())?;
    }
    Ok(())
}
