//! ShareX's command line in the app: `ShareXCLIManager.UseCommandLineArgs`
//! (what each argument does), `ApplicationCommandLine` (the first command
//! line and the ones later launches pass on), `SingleInstanceCommandRouter`
//! (those wait until startup is done, at most 5 seconds), the
//! `ApplicationInfo` mutex and pipe names, and the handlers it calls in
//! `TaskHelpers`: `ImportCustomUploader`, `ImportImageEffect`,
//! `HandleNativeMessagingInput` and `ExecuteJob(job, filePath)`.
//!
//! What a command line does, in order, for each argument:
//!
//! - `-CustomUploader <file.sxcu>`: imports the custom uploader, asking
//!   whether to add it or make it the active one;
//! - `-ImageEffect <file.sxie>`: unpacks the package into the ImageEffects
//!   folder and imports its preset into the Image effects window (Presets
//!   mode, the default task's presets), asking to turn image effects on;
//! - `-<HotkeyType> [file]`: runs that job with the default task settings,
//!   with the file for the jobs that take one (`-OCR`, `-ImageEditor`,
//!   `-FileUpload`, ...; a file that isn't there skips the job);
//! - `-workflow "<name>"`: runs the hotkey (workflow) with that name;
//! - `-NativeMessagingInput <file.json>`: what the browser extension sent;
//! - anything else is a file, folder or URL to upload, with the task
//!   settings of the hotkey named by `-task "<name>"`, if any.
//!
//! Startup options (`-silent`, `-portable`, `-sandbox`, `-multi`,
//! `-NoHotkeys`, `-AutoClose`) are read where startup needs them.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use image::RgbaImage;
use sharex_helpers::cli::CliManager;
use sharex_settings::{AppPaths, HotkeySettings, HotkeyType, TaskSettings};
use sharexrs_native_messaging::{NativeMessagingAction, NativeMessagingInput};
use winit::event_loop::ActiveEventLoop;

use crate::App;
use crate::message_box::{DialogResult, MessageBox, MessageBoxIcon};

/// ShareX's `ApplicationInfo.MutexName` is its installer's AppId; ShareXrs's
/// is its own installer's (packaging/windows/ShareXrs-setup.iss), which is
/// also the installer's `AppMutex`.
pub const MUTEX_NAME: &str = "F7492E77-AA16-4212-9FD6-53D97873DD92";

/// When the program started (`Program._startTimer`).
pub static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

/// `SingleInstanceCommandRouter.StartupTimeout`.
const STARTUP_TIMEOUT: Duration = Duration::from_secs(5);

/// The single instance mutex and pipe names: ShareX's
/// (`ApplicationInfo.MutexName`, `{MachineName}-{UserName}-{Name}`) with
/// ShareXrs's name. A portable copy adds a key of its personal folder, so
/// it never passes its command line to the installed copy or to another
/// portable one (ShareX's portable and installed copies share one name).
pub fn instance_names(paths: &AppPaths) -> (String, String) {
    let base = if cfg!(windows) {
        let var = |n: &str| std::env::var(n).unwrap_or_default();
        format!("{}-{}-ShareXrs", var("COMPUTERNAME"), var("USERNAME"))
    } else {
        // The socket is in the user's own folder already; the host name can
        // differ between a terminal and the desktop, so it's left out.
        format!("{}-ShareXrs", std::env::var("USER").unwrap_or_default())
    };
    if !paths.portable {
        return (MUTEX_NAME.to_owned(), base);
    }
    let key = folder_key(&paths.personal_folder);
    (format!("{MUTEX_NAME}-{key}"), format!("{base}-{key}"))
}

/// A short hash of the folder, the same however it's spelt (case on
/// Windows and macOS, links, `..`).
fn folder_key(folder: &std::path::Path) -> String {
    let path = std::fs::canonicalize(folder).unwrap_or_else(|_| folder.to_path_buf());
    let mut text = path.to_string_lossy().into_owned();
    if cfg!(any(windows, target_os = "macos")) {
        text = sharex_helpers::culture::ordinal_upper(&text);
    }
    let hash = text.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3));
    format!("{hash:016x}")
}

/// One thing a command line asks for.
#[derive(Debug, Clone, PartialEq)]
pub enum CliAction {
    /// `CheckCustomUploader`: the parameter, when it ends with `.sxcu`.
    ImportCustomUploader(PathBuf),
    /// `CheckImageEffect`: the parameter, when it ends with `.sxie`.
    ImportImageEffect(PathBuf),
    /// `CheckCLIHotkey`: `ExecuteJob(job, filePath)` with the default task
    /// settings; the file made absolute (against the program's folder, as
    /// `FileHelpers.GetAbsolutePath`) and known to exist.
    Job { job: HotkeyType, file: Option<PathBuf> },
    /// `CheckCLIWorkflow`: `ExecuteJob(hotkey.TaskSettings)`, the hotkey's index.
    Workflow(usize),
    /// `CheckNativeMessagingInput`: the parameter, when it ends with `.json`.
    NativeMessagingInput(PathBuf),
    /// `UploadManager.DownloadAndUploadFile(url, taskSettings)`.
    DownloadAndUpload(String),
    /// `UploadManager.UploadFile(path, taskSettings)`.
    UploadFile(PathBuf),
}

/// What `UseCommandLineArgs` does with a command line.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CliPlan {
    /// `FindCLITask`: the hotkey whose task settings uploads use.
    pub task: Option<usize>,
    pub actions: Vec<CliAction>,
    /// `"CommandLine: " + command`, as ShareX logs each argument.
    pub log: Vec<String>,
}

fn ends_with_ignore_case(s: &str, suffix: &str) -> bool {
    s.len() >= suffix.len()
        && s.is_char_boundary(s.len() - suffix.len())
        && sharex_helpers::culture::ordinal_ignore_case_eq(&s[s.len() - suffix.len()..], suffix)
}

/// `ShareXCLIManager.UseCommandLineArgs(commands)`, without running
/// anything. `display_names` are the hotkeys' `TaskSettings.ToString()`.
pub fn plan(cli: &CliManager, hotkeys: &[HotkeySettings], display_names: &[String]) -> CliPlan {
    let mut plan = CliPlan::default();
    let commands = &cli.commands;
    if commands.is_empty() {
        return plan;
    }
    // `FindCLITask`: the first "task" with a parameter, by the hotkey's name.
    plan.task = commands
        .iter()
        .find(|c| c.check_command("task") && c.parameter().is_some())
        .and_then(|c| display_names.iter().position(|n| Some(n.as_str()) == c.parameter()));
    for command in commands {
        plan.log.push(format!("CommandLine: {command}"));
        if command.is_command {
            if let Some(action) = command_action(command, hotkeys, display_names, &mut plan.log) {
                plan.actions.push(action);
            }
            continue;
        }
        if sharex_uploaders::url_helpers::is_valid_url(&command.command) {
            plan.actions.push(CliAction::DownloadAndUpload(command.command.clone()));
        } else {
            plan.actions.push(CliAction::UploadFile(PathBuf::from(&command.command)));
        }
    }
    plan
}

/// `CheckCustomUploader || CheckImageEffect || CheckCLIHotkey ||
/// CheckCLIWorkflow || CheckNativeMessagingInput`.
fn command_action(
    command: &sharex_helpers::cli::CliCommand,
    hotkeys: &[HotkeySettings],
    display_names: &[String],
    log: &mut Vec<String>,
) -> Option<CliAction> {
    let parameter = command.parameter();
    if command.check_command("CustomUploader") {
        return parameter.filter(|p| ends_with_ignore_case(p, ".sxcu")).map(|p| CliAction::ImportCustomUploader(p.into()));
    }
    if command.check_command("ImageEffect") {
        return parameter.filter(|p| ends_with_ignore_case(p, ".sxie")).map(|p| CliAction::ImportImageEffect(p.into()));
    }
    if let Some(job) = HotkeyType::ALL.iter().find(|j| command.check_command(j.name())) {
        // `CheckParameterForFilePath`: a file that isn't there throws, and
        // the job doesn't run.
        let file = match parameter {
            Some(p) => {
                let path = sharex_settings::paths::absolute_path(p);
                if !path.is_file() {
                    log.push(format!("System.IO.FileNotFoundException: Unable to find the specified file. ({})", path.display()));
                    return None;
                }
                Some(path)
            }
            None => None,
        };
        return Some(CliAction::Job { job: job.clone(), file });
    }
    if command.check_command("workflow")
        && let Some(p) = parameter
        && let Some(i) = hotkeys
            .iter()
            .zip(display_names)
            .position(|(h, n)| h.TaskSettings.Job != HotkeyType::None && n == p)
    {
        return Some(CliAction::Workflow(i));
    }
    if command.check_command("NativeMessagingInput") {
        return parameter.filter(|p| ends_with_ignore_case(p, ".json")).map(|p| CliAction::NativeMessagingInput(p.into()));
    }
    None
}

/// `WebHelpers.DataURLToImage`: `data:<image type>;base64,<data>` with a
/// PNG, JPEG, GIF or BMP type (`MimeTypes.IsImageMimeType`).
pub fn data_url_to_image(url: &str) -> Option<RgbaImage> {
    let lower = url.get(..5)?;
    if !lower.eq_ignore_ascii_case("data:") {
        return None;
    }
    // `^data:(?<mediaType>[\w\/]+);base64,(?<data>.+)$`, ignoring case: `.`
    // stops at a line feed, and `$` also matches before a final one.
    let rest = &url[5..];
    let semicolon = rest.find(';')?;
    let media_type = &rest[..semicolon];
    if media_type.is_empty() || !media_type.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '/') {
        return None;
    }
    let after = &rest[semicolon + 1..];
    if after.len() < 7 || !after[..7].eq_ignore_ascii_case("base64,") {
        return None;
    }
    let data = after[7..].strip_suffix('\n').unwrap_or(&after[7..]);
    if data.is_empty() || data.contains('\n') {
        return None;
    }
    let image_type = ["image/png", "image/jpeg", "image/gif", "image/bmp"].iter().any(|t| t.eq_ignore_ascii_case(media_type));
    if !image_type {
        return None;
    }
    // `Convert.FromBase64String` skips white space.
    let clean: String = data.chars().filter(|c| !matches!(c, ' ' | '\t' | '\r' | '\n')).collect();
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD.decode(clean).ok()?;
    image::load_from_memory(&bytes).ok().map(|i| i.to_rgba8())
}

/// `WebHelpers.DownloadImageAsync`: a successful response whose content
/// type is an image type, decoded.
fn download_image(url: &str) -> Option<RgbaImage> {
    use sharex_uploaders::http;
    let request = http::HttpRequest { method: sharex_uploaders::HttpMethod::GET, url: url.to_owned(), headers: Vec::new(), content: None };
    let response = http::send(request, http::RequestOptions::default()).ok()?;
    let media_type = response.header("Content-Type")?.split(';').next()?.trim().to_owned();
    if !response.is_success() || !["image/png", "image/jpeg", "image/gif", "image/bmp"].iter().any(|t| t.eq_ignore_ascii_case(&media_type)) {
        return None;
    }
    image::load_from_memory(&response.response_data?).ok().map(|i| i.to_rgba8())
}

/// `SingleInstanceCommandRouter`: command lines from later launches wait
/// until startup is done (`MarkReady`); one that waited more than 5
/// seconds is dropped, as ShareX's `WaitAsync(StartupTimeout)` drops it.
#[derive(Default)]
pub struct CommandRouter {
    ready: bool,
    pending: Vec<(Instant, Vec<String>)>,
    /// The first command line, run once startup is done
    /// (`ExecuteInitialAsync`).
    initial: Option<Vec<String>>,
}

impl CommandRouter {
    /// `ApplicationCommandLine.Initialize(args)`.
    pub fn new(initial: Vec<String>) -> Self {
        Self { ready: false, pending: Vec::new(), initial: Some(initial) }
    }
}

/// A message box the command line asked for, and what its answer is for.
pub enum CliQuestion {
    /// `ImportCustomUploader`'s confirmation: `activate` asks whether to make
    /// it the active one (Yes/No/Cancel), else whether to add it (Yes/No).
    CustomUploader { item: Box<sharex_uploaders::custom::CustomUploaderItem>, activate: bool },
    /// `ImportImageEffect`'s "Would you like to enable image effects?".
    EnableImageEffects,
}

/// The result of `ProcessImagesDuringExtensionUpload`'s download.
#[derive(Debug)]
pub struct ExtensionImage {
    pub url: String,
    pub image: Option<RgbaImage>,
    pub task: TaskSettings,
}

impl App {
    /// `SingleInstanceCommandRouter.ArgumentsReceived`.
    pub(crate) fn arguments_received(&mut self, event_loop: &ActiveEventLoop, arguments: Vec<String>) {
        eprintln!("Arguments received: \"{}\"", arguments.join(" "));
        sharex_capture::trace::begin("arguments received");
        if self.router.ready {
            self.execute_received(event_loop, arguments);
        } else {
            self.router.pending.push((Instant::now(), arguments));
        }
    }

    /// `MarkReady`, then `ApplicationCommandLine.ExecuteInitialAsync`:
    /// startup is done, so the first command line runs, and the ones that
    /// arrived meanwhile.
    pub(crate) fn startup_finished(&mut self, event_loop: &ActiveEventLoop) {
        if self.router.ready {
            return;
        }
        self.router.ready = true;
        eprintln!("Startup time: {} ms", START.get().map_or(0, |s| s.elapsed().as_millis()));
        // `DispatcherTimer.RunOnce(WarmUpRegionCapture, 2 s)` from the main
        // window's constructor.
        self.region_warm_up_at = Some(Instant::now() + std::time::Duration::from_secs(2));
        if let Some(args) = self.router.initial.take() {
            self.use_command_line_args(event_loop, &args);
        }
        // `if (ActionsToolbarRunAtStartup) TaskHelpers.OpenActionsToolbar()`.
        if self.settings.settings.ActionsToolbarRunAtStartup {
            self.open_actions_toolbar(event_loop);
        }
        for (at, args) in std::mem::take(&mut self.router.pending) {
            if at.elapsed() > STARTUP_TIMEOUT {
                eprintln!("Arguments were not processed because application startup did not complete within 5 seconds.");
                continue;
            }
            self.execute_received(event_loop, args);
        }
    }

    /// `ApplicationCommandLine.ExecuteReceivedAsync`: no arguments bring the
    /// main window up; with some, a visible main window is activated, as
    /// Avalonia activates (which the foreground lock may refuse, leaving
    /// the active window for an active-window job: `crate::activation`).
    fn execute_received(&mut self, event_loop: &ActiveEventLoop, arguments: Vec<String>) {
        let visible = self.main.as_ref().is_some_and(|w| w.window.is_visible() != Some(false));
        if crate::activation::activates_on_receive(arguments.is_empty(), visible) {
            // ShareX also hides and shows the tray icon first, for a Windows
            // startup bug; ShareXrs's icon doesn't have it.
            self.show_main_window(event_loop);
        }
        self.use_command_line_args(event_loop, &arguments);
    }

    /// `new ShareXCLIManager(args)`, `ParseCommands()`, `UseCommandLineArgs()`.
    pub(crate) fn use_command_line_args(&mut self, event_loop: &ActiveEventLoop, arguments: &[String]) {
        let mut cli = CliManager::new(arguments);
        if !cli.parse_commands() {
            eprintln!("System.IndexOutOfRangeException: an empty command line argument");
        }
        let hotkeys = &self.settings.hotkeys.Hotkeys;
        let names: Vec<String> = hotkeys.iter().map(|h| h.TaskSettings.display_name()).collect();
        let plan = plan(&cli, hotkeys, &names);
        for line in &plan.log {
            eprintln!("{line}");
        }
        let defaults = &self.settings.settings.DefaultTaskSettings;
        // `FindCLITask`'s `GetSafeTaskSettings`; without one, uploads take
        // the defaults (`GetDefaultTaskSettings`).
        let upload_task = match plan.task {
            Some(i) => hotkeys[i].TaskSettings.safe_task_settings(defaults),
            None => TaskSettings::default_task_settings(defaults),
        };
        for action in plan.actions {
            self.run_cli_action(event_loop, action, &upload_task);
        }
    }

    fn run_cli_action(&mut self, event_loop: &ActiveEventLoop, action: CliAction, upload_task: &TaskSettings) {
        match action {
            CliAction::ImportCustomUploader(path) => self.import_custom_uploader(event_loop, &path),
            CliAction::ImportImageEffect(path) => self.import_image_effect(event_loop, &path),
            CliAction::Job { job, file } => {
                let defaults = &self.settings.settings.DefaultTaskSettings;
                let task = defaults.safe_task_settings(defaults);
                self.run_job_with_file(event_loop, job, task, file);
            }
            CliAction::Workflow(i) => {
                let Some(hotkey) = self.settings.hotkeys.Hotkeys.get(i) else { return };
                let task = hotkey.TaskSettings.safe_task_settings(&self.settings.settings.DefaultTaskSettings);
                self.run_job(event_loop, task.Job.clone(), task, Some(i));
            }
            CliAction::NativeMessagingInput(path) => self.handle_native_messaging_input(event_loop, &path),
            CliAction::DownloadAndUpload(url) => {
                let tasks = crate::upload::manager::download_and_upload_file(&url, upload_task, &mut self.new_task_context());
                self.start_upload_tasks(tasks.into_iter().collect());
            }
            CliAction::UploadFile(path) => {
                let mut ctx = self.new_task_context();
                let tasks = crate::upload::manager::upload_file(&path, upload_task, &mut ctx);
                let confirm = ctx.confirm_files.take();
                self.start_upload_tasks(tasks);
                self.confirm_files(event_loop, confirm, upload_task.clone());
            }
        }
    }

    /// `TaskHelpers.ExecuteJob(taskSettings, job, filePath)`: the jobs that
    /// take a file; the others run as without one.
    pub(crate) fn run_job_with_file(&mut self, event_loop: &ActiveEventLoop, job: HotkeyType, task: TaskSettings, file: Option<PathBuf>) {
        let Some(file) = file else {
            self.run_job(event_loop, job, task, None);
            return;
        };
        eprintln!("job: {} ({}) with {}", job.name(), task.display_name(), file.display());
        match job {
            // `UploadManager.UploadFile(filePath, taskSettings)`.
            HotkeyType::FileUpload => {
                let mut ctx = self.new_task_context();
                let tasks = crate::upload::manager::upload_file(&file, &task, &mut ctx);
                let confirm = ctx.confirm_files.take();
                self.start_upload_tasks(tasks);
                self.confirm_files(event_loop, confirm, task);
            }
            // `OCRImage(filePath, taskSettings)`.
            HotkeyType::OCR => self.ocr_file(event_loop, &file, task),
            // `OpenQRCodeScanFromImageFile(filePath)`.
            HotkeyType::QRCode => {
                self.open_dialog_window(event_loop, Box::new(crate::dialogs::qr_code::QrCodeWindow::scan_image_file(&file)));
            }
            // `PinToScreen(filePath)`.
            HotkeyType::PinToScreen | HotkeyType::PinToScreenFromFile => self.pin_file(event_loop, std::path::Path::new(&file), Some(task)),
            // `AnnotateImageFromFile(filePath, safeTaskSettings)`.
            HotkeyType::ImageEditor => self.annotate_file(event_loop, std::path::Path::new(&file), Some(task)),
            // `OpenVideoConverter(filePath, …)`, `OpenVideoTrimmer(…, filePath)`,
            // `OpenAnimatedGifTrimmer(filePath, …)`.
            HotkeyType::VideoConverter | HotkeyType::VideoTrimmer | HotkeyType::AnimatedGifTrimmer => {
                if let Some(key) = crate::video_tools::open::tool_for_hotkey(&job) {
                    self.open_video_tool(event_loop, key, Some(task), Some(file.to_string_lossy().into_owned()));
                }
            }
            // `TaskHelpers.OpenImageViewer(filePath)`.
            HotkeyType::ImageViewer => self.show_image_file(event_loop, std::path::Path::new(&file)),
            // `OpenImageBeautifier(filePath, safeTaskSettings)`.
            HotkeyType::ImageBeautifier => self.open_image_beautifier(event_loop, Some(file), Some(task)),
            // `OpenImageEffects(filePath, safeTaskSettings)`.
            HotkeyType::ImageEffects => self.open_image_effects_tool(event_loop, Some(file), task),
            // The image tools on a file: their windows aren't ported.
            HotkeyType::PhotoEditor
            | HotkeyType::PixelEditor => eprintln!("{} with a file: not ported yet", job.name()),
            // `OpenHashCheck(filePath, …)`, `OpenMetadataWindow(filePath)`,
            // `StripMetadata(filePath, …)`.
            HotkeyType::HashCheck | HotkeyType::Metadata | HotkeyType::StripMetadata => {
                self.run_data_job(event_loop, &job, &task, Some(file.to_string_lossy().into_owned()));
            }
            _ => self.run_job(event_loop, job, task, None),
        }
    }

    /// `TaskHelpers.ImportCustomUploader`: once the uploaders config is
    /// there, the file read and the question asked (`custom_uploader_answered`).
    fn import_custom_uploader(&mut self, event_loop: &ActiveEventLoop, path: &std::path::Path) {
        use sharex_uploaders::custom::CustomUploaderItem;
        // `JsonHelpers.DeserializeFromFile`: no file, or an empty one, is
        // `null`, and nothing happens.
        if std::fs::metadata(path).map_or(true, |m| !m.is_file() || m.len() == 0) {
            return;
        }
        let item = std::fs::read_to_string(path)
            .map_err(|e| e.to_string())
            .and_then(|text| CustomUploaderItem::from_json(text.trim_start_matches('\u{feff}')));
        let item = match item {
            Ok(Some(item)) => item,
            Ok(None) => return,
            Err(e) => {
                eprintln!("ImportCustomUploader({}): {e}", path.display());
                self.show_error(event_loop, &e);
                return;
            }
        };
        let (message, activate) = custom_uploader_question(&item.display_name(), item.destination_type);
        self.ask(event_loop, message, CliQuestion::CustomUploader { item: Box::new(item), activate });
    }

    /// The rest of `ImportCustomUploader`, with the answer.
    fn custom_uploader_answered(&mut self, item: sharex_uploaders::custom::CustomUploaderItem, asked_activate: bool, result: DialogResult) {
        use sharex_settings::{FileDestination, ImageDestination, TextDestination, URLSharingServices, UrlShortenerType};
        use sharex_uploaders::enums::CustomUploaderDestinationType as D;
        // Yes/No: only No stops it (closing the box adds it, as in ShareX).
        // Yes/No/Cancel: Cancel (or closing) stops it; Yes makes it active.
        if (!asked_activate && result == DialogResult::No) || (asked_activate && result == DialogResult::Cancel) {
            return;
        }
        let activate = asked_activate && result == DialogResult::Yes;
        let mut item = item;
        if let Err(e) = item.check_backward_compatibility() {
            eprintln!("ImportCustomUploader: {e}");
            return;
        }
        let t = item.destination_type;
        let index = self.uploads.config.with(|c| {
            c.CustomUploadersList.0.push(item);
            let index = c.CustomUploadersList.0.len() as i32 - 1;
            if activate {
                if t.contains(D::ImageUploader) {
                    c.CustomImageUploaderSelected = index;
                }
                if t.contains(D::TextUploader) {
                    c.CustomTextUploaderSelected = index;
                }
                if t.contains(D::FileUploader) {
                    c.CustomFileUploaderSelected = index;
                }
                if t.contains(D::URLShortener) {
                    c.CustomURLShortenerSelected = index;
                }
                if t.contains(D::URLSharingService) {
                    c.CustomURLSharingServiceSelected = index;
                }
            }
            index
        });
        if activate {
            let d = &mut self.settings.settings.DefaultTaskSettings;
            if t.contains(D::ImageUploader) {
                d.ImageDestination = ImageDestination::CustomImageUploader;
            }
            if t.contains(D::TextUploader) {
                d.TextDestination = TextDestination::CustomTextUploader;
            }
            if t.contains(D::FileUploader) {
                d.FileDestination = FileDestination::CustomFileUploader;
            }
            if t.contains(D::URLShortener) {
                d.URLShortenerDestination = UrlShortenerType::CustomURLShortener;
            }
            if t.contains(D::URLSharingService) {
                d.URLSharingServiceDestination = URLSharingServices::CustomURLSharingService;
            }
            self.refresh_menus();
        }
        eprintln!("Custom uploader imported ({index}).");
        // `CustomUploaderSettingsIntegration.Refresh(true)`.
        if let Some(d) = self.dialogs.iter().find(|d| d.dialog.single() == Some("custom-uploader-settings")) {
            d.window.window.request_redraw();
        }
    }

    /// `TaskHelpers.ImportImageEffect`: the package's images go to the
    /// ImageEffects folder, then its preset to the Image effects window
    /// (`OpenImageEffectsSingleton(DefaultTaskSettings, configJson)`: the
    /// one Presets window, which imports and selects it,
    /// `ImageEffectsViewModel.ImportPreset`), then the question whether to
    /// turn on Add image effects.
    fn import_image_effect(&mut self, event_loop: &ActiveEventLoop, path: &std::path::Path) {
        let folder = self.settings.paths.image_effects_folder();
        let config = match sharex_image_effects::package::extract(path, &folder) {
            Ok(c) => c,
            Err(e) => {
                // `ex.ShowError(false)`.
                self.show_error(event_loop, &e.to_string());
                return;
            }
        };
        let Some(config) = config.filter(|c| !c.is_empty()) else { return };
        self.open_image_effects_presets(event_loop, None, Some(config));
        let job = &self.settings.settings.DefaultTaskSettings.AfterCaptureJob;
        if !job.contains(sharex_settings::AfterCaptureTasks::AddImageEffects) {
            let text = sharex_strings::sharex::WouldYouLikeToEnableImageEffects();
            self.ask(event_loop, MessageBox::yes_no("ShareXrs", text, MessageBoxIcon::Question), CliQuestion::EnableImageEffects);
        }
    }

    /// A command line's message box was answered.
    pub(crate) fn cli_answered(&mut self, question: CliQuestion, result: DialogResult) {
        match question {
            CliQuestion::CustomUploader { item, activate } => self.custom_uploader_answered(*item, activate, result),
            CliQuestion::EnableImageEffects => {
                if result == DialogResult::Yes {
                    self.settings.settings.DefaultTaskSettings.AfterCaptureJob.insert(sharex_settings::AfterCaptureTasks::AddImageEffects);
                    self.refresh_menus();
                }
            }
        }
    }

    fn ask(&mut self, event_loop: &ActiveEventLoop, message: MessageBox, question: CliQuestion) {
        self.queue_dialog(event_loop, message, crate::DialogPurpose::CommandLine(question));
    }

    /// `ex.ShowError(false)`: the message, in an error box.
    pub(crate) fn show_error(&mut self, event_loop: &ActiveEventLoop, text: &str) {
        let caption = format!("ShareXrs - {}", sharex_strings::helpers_lib::Error());
        self.queue_dialog(event_loop, MessageBox::ok(caption, text, MessageBoxIcon::Error), crate::DialogPurpose::Information);
    }

    /// `TaskHelpers.HandleNativeMessagingInput(filePath)`.
    fn handle_native_messaging_input(&mut self, event_loop: &ActiveEventLoop, path: &std::path::Path) {
        if !path.is_file() {
            return;
        }
        // An empty file is `default`; read errors are logged; the file goes
        // either way.
        let input = match std::fs::read(path) {
            Ok(bytes) => NativeMessagingInput::parse(&String::from_utf8_lossy(&bytes)),
            Err(e) => {
                eprintln!("HandleNativeMessagingInput: {e}");
                None
            }
        };
        let _ = std::fs::remove_file(path);
        let Some(input) = input else { return };
        let task = TaskSettings::default_task_settings(&self.settings.settings.DefaultTaskSettings);
        // `PlayNotificationSoundAsync(NotificationSound.ActionCompleted, taskSettings)`.
        if task.GeneralSettings.as_ref().is_none_or(|g| g.PlaySoundAfterAction) {
            crate::sound::play_action_completed();
        }
        let url = input.url.filter(|u| !u.is_empty());
        let text = input.text.filter(|t| !t.is_empty());
        match input.action {
            NativeMessagingAction::UploadImage => {
                let Some(url) = url else { return };
                if let Some(image) = data_url_to_image(&url) {
                    self.run_extension_image(image, task);
                } else if task.AdvancedSettings.as_ref().is_some_and(|a| a.ProcessImagesDuringExtensionUpload) {
                    // `await WebHelpers.DownloadImageAsync(url)`, off the event loop.
                    let proxy = self.proxy.clone();
                    std::thread::spawn(move || {
                        let image = download_image(&url);
                        let _ = proxy.send_event(crate::UserEvent::ExtensionImage(Box::new(ExtensionImage { url, image, task })));
                    });
                } else {
                    self.cli_download_and_upload(&url, &task);
                }
            }
            NativeMessagingAction::UploadVideo | NativeMessagingAction::UploadAudio => {
                if let Some(url) = url {
                    self.cli_download_and_upload(&url, &task);
                }
            }
            NativeMessagingAction::UploadText => {
                if let Some(text) = text {
                    self.cli_upload_text(&text, &task);
                }
            }
            NativeMessagingAction::ShortenURL => {
                if let Some(url) = url {
                    self.start_upload_tasks(crate::upload::manager::shorten_url(&url, &task).into_iter().collect());
                }
            }
            // "TEMP: For backward compatibility".
            NativeMessagingAction::None | NativeMessagingAction::Other(_) => {
                if let Some(url) = url {
                    self.cli_download_and_upload(&url, &task);
                } else if let Some(text) = text {
                    self.cli_upload_text(&text, &task);
                }
            }
        }
        let _ = event_loop;
    }

    /// The downloaded extension image, or the URL uploaded as a file when
    /// it wasn't one.
    pub(crate) fn extension_image(&mut self, result: ExtensionImage) {
        match result.image {
            Some(image) => self.run_extension_image(image, result.task),
            None => self.cli_download_and_upload(&result.url, &result.task),
        }
    }

    /// `UploadManager.RunImageTask(bmp, taskSettings)`.
    fn run_extension_image(&mut self, image: RgbaImage, task: TaskSettings) {
        let screenshots_parent = self.settings.paths.screenshots_parent_folder(Some(&self.settings.settings));
        let effects = self.settings.paths.image_effects_folder();
        let metadata = crate::capture_job::Metadata { image, window_title: None, process_name: None };
        let job = crate::capture_job::ImageTask::new(metadata, task, &mut self.settings.settings, &screenshots_parent, effects);
        self.run_image_task(job);
    }

    fn cli_download_and_upload(&mut self, url: &str, task: &TaskSettings) {
        let tasks = crate::upload::manager::download_and_upload_file(url, task, &mut self.new_task_context());
        self.start_upload_tasks(tasks.into_iter().collect());
    }

    /// `UploadManager.UploadText(text, taskSettings)` (no custom text).
    fn cli_upload_text(&mut self, text: &str, task: &TaskSettings) {
        let tasks = crate::upload::manager::upload_text(text, task, false, &mut self.new_task_context());
        self.start_upload_tasks(tasks.into_iter().collect());
    }
}

/// `ImportCustomUploader`'s question for an uploader named `name`: add it
/// (no destination type), or add it and make it active for its kinds of
/// destination (Yes, No, Cancel). Returns the message and whether it asks
/// to activate.
pub(crate) fn custom_uploader_question(name: &str, t: sharex_uploaders::enums::CustomUploaderDestinationType) -> (MessageBox, bool) {
    use sharex_uploaders::enums::CustomUploaderDestinationType as D;
    let title = crate::rs_title(sharex_strings::sharex::TaskHelpers_CustomUploaderConfirmationTitle());
    if t == D::None {
        let text = sharex_strings::sharex::TaskHelpers_AddCustomUploaderConfirmation().replace("{0}", name);
        return (MessageBox::yes_no(title, text, MessageBoxIcon::Question), false);
    }
    let mut destinations = Vec::new();
    if t.contains(D::ImageUploader) {
        destinations.push("images");
    }
    if t.contains(D::TextUploader) {
        destinations.push("texts");
    }
    if t.contains(D::FileUploader) {
        destinations.push("files");
    }
    if t.contains(D::URLShortener) || t.contains(D::URLSharingService) {
        destinations.push("urls");
    }
    let text = sharex_strings::sharex::TaskHelpers_SetActiveCustomUploaderConfirmation().replace("{0}", name).replace("{1}", &destinations.join("/"));
    (MessageBox::yes_no_cancel(title, text, MessageBoxIcon::Question), true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    fn hotkey(job: HotkeyType, description: &str) -> HotkeySettings {
        let mut h = HotkeySettings::default();
        h.TaskSettings.Job = job;
        h.TaskSettings.Description = description.into();
        h
    }

    fn plan_of(a: &[&str], hotkeys: &[HotkeySettings]) -> CliPlan {
        let names: Vec<String> = hotkeys.iter().map(|h| h.TaskSettings.display_name()).collect();
        plan(&CliManager::parse(&args(a)), hotkeys, &names)
    }

    #[test]
    fn uploads_files_folders_and_urls() {
        let p = plan_of(&[r"C:\a b.png", "https://example.com/x.png", "/tmp/folder"], &[]);
        assert_eq!(
            p.actions,
            vec![
                CliAction::UploadFile(r"C:\a b.png".into()),
                CliAction::DownloadAndUpload("https://example.com/x.png".into()),
                CliAction::UploadFile("/tmp/folder".into()),
            ]
        );
        assert_eq!(p.log, vec![r"CommandLine: C:\a b.png", "CommandLine: https://example.com/x.png", "CommandLine: /tmp/folder"]);
        assert_eq!(p.task, None);
    }

    #[test]
    fn a_file_after_a_startup_option_is_its_parameter() {
        // ShareX's quirk: "-silent a.png" gives -silent a parameter.
        let p = plan_of(&["-silent", "a.png", "b.png"], &[]);
        assert_eq!(p.actions, vec![CliAction::UploadFile("b.png".into())]);
    }

    #[test]
    fn imports() {
        let p = plan_of(&["-CustomUploader", "x.SXCU", "-customuploader", "x.json", "-ImageEffect", "e.sxie", "-NativeMessagingInput", "m.JSON"], &[]);
        assert_eq!(
            p.actions,
            vec![
                CliAction::ImportCustomUploader("x.SXCU".into()),
                CliAction::ImportImageEffect("e.sxie".into()),
                CliAction::NativeMessagingInput("m.JSON".into()),
            ]
        );
        // "/CustomUploader" isn't a command: a file to upload.
        assert_eq!(plan_of(&["/CustomUploader"], &[]).actions, vec![CliAction::UploadFile("/CustomUploader".into())]);
    }

    #[test]
    fn jobs_by_hotkey_type_name() {
        let p = plan_of(&["-exitsharex", "-RectangleRegion", "-None"], &[]);
        assert_eq!(
            p.actions,
            vec![
                CliAction::Job { job: HotkeyType::ExitShareX, file: None },
                CliAction::Job { job: HotkeyType::RectangleRegion, file: None },
                CliAction::Job { job: HotkeyType::None, file: None },
            ]
        );
        // A file that isn't there skips the job.
        let p = plan_of(&["-OCR", "no-such-file-here.png"], &[]);
        assert!(p.actions.is_empty());
        assert!(p.log.iter().any(|l| l.contains("FileNotFoundException")));
        // An existing one is made absolute.
        let dir = std::env::temp_dir().join(format!("sharexrs-cli-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("i.png");
        std::fs::write(&file, b"x").unwrap();
        let p = plan_of(&["-ImageEditor", file.to_str().unwrap()], &[]);
        assert_eq!(p.actions, vec![CliAction::Job { job: HotkeyType::ImageEditor, file: Some(file.clone()) }]);
        // A folder isn't a file.
        assert!(plan_of(&["-OCR", dir.to_str().unwrap()], &[]).actions.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn workflows_and_task() {
        let hotkeys =
            [hotkey(HotkeyType::RectangleRegion, "Region to Imgur"), hotkey(HotkeyType::None, "Nothing"), hotkey(HotkeyType::PrintScreen, "")];
        let p = plan_of(&["-workflow", "Region to Imgur"], &hotkeys);
        assert_eq!(p.actions, vec![CliAction::Workflow(0)]);
        // Exact case; a hotkey with no job never matches.
        assert!(plan_of(&["-workflow", "region to imgur"], &hotkeys).actions.is_empty());
        assert!(plan_of(&["-workflow", "Nothing"], &hotkeys).actions.is_empty());
        // No description: the job's name (`TaskSettings.ToString`).
        let name = hotkeys[2].TaskSettings.display_name();
        assert_eq!(plan_of(&["-workflow", &name], &hotkeys).actions, vec![CliAction::Workflow(2)]);
        // -task picks the uploads' settings, anywhere on the line.
        let p = plan_of(&["a.png", "-task", "Region to Imgur"], &hotkeys);
        assert_eq!(p.task, Some(0));
        assert_eq!(p.actions, vec![CliAction::UploadFile("a.png".into())]);
        assert_eq!(plan_of(&["-task", "Nope", "a.png"], &hotkeys).task, None);
        // Even a hotkey with no job counts for -task.
        assert_eq!(plan_of(&["-task", "Nothing"], &hotkeys).task, Some(1));
    }

    #[test]
    fn startup_options_do_nothing_here() {
        let p = plan_of(&["-silent", "-portable", "-p", "-s", "-sandbox", "-multi", "-m", "-NoHotkeys", "-AutoClose", "-uninstall"], &[]);
        assert!(p.actions.is_empty());
        assert_eq!(p.log.len(), 10);
    }

    #[test]
    fn data_urls() {
        let mut png = Vec::new();
        image::RgbaImage::from_pixel(2, 1, image::Rgba([1, 2, 3, 255]))
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        use base64::Engine;
        let b64 = base64::engine::general_purpose::STANDARD.encode(&png);
        let img = data_url_to_image(&format!("DATA:Image/PNG;BASE64,{b64}")).unwrap();
        assert_eq!(img.dimensions(), (2, 1));
        assert!(data_url_to_image(&format!("data:image/png;base64,{b64}\n")).is_some());
        assert!(data_url_to_image(&format!("data:image/webp;base64,{b64}")).is_none());
        assert!(data_url_to_image(&format!("data:image/png;charset=x;base64,{b64}")).is_none());
        assert!(data_url_to_image("data:image/png;base64,").is_none());
        assert!(data_url_to_image("data:image/png;base64,!!!!").is_none());
        assert!(data_url_to_image("https://x.y/a.png").is_none());
    }

    #[test]
    fn portable_copies_have_their_own_names() {
        let installed = AppPaths::in_folder("/x/ShareXrs");
        let (m, p) = instance_names(&installed);
        assert_eq!(m, MUTEX_NAME);
        assert!(p.ends_with("-ShareXrs"));
        let mut a = AppPaths::in_folder(std::env::temp_dir().join("sharexrs-portable-a"));
        a.portable = true;
        let mut b = AppPaths::in_folder(std::env::temp_dir().join("sharexrs-portable-b"));
        b.portable = true;
        let (ma, pa) = instance_names(&a);
        let (mb, pb) = instance_names(&b);
        assert!(ma.starts_with(MUTEX_NAME) && ma != MUTEX_NAME);
        assert_ne!(ma, mb);
        assert_ne!(pa, pb);
        assert_eq!(instance_names(&a), (ma, pa));
    }
}
