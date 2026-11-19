//! ShareXrs: ShareX, ported to Rust and egui.
//!
//! `sharexrs` runs the app. `sharexrs --render <screen> --out <png>` draws
//! one screen offscreen instead, at the size ShareX's would be, for
//! comparing against a screenshot of ShareX (`scripts/compare.ps1`).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app_icon;
mod app_settings;
mod app_windows;
mod appearance;
mod auto_capture;
mod capture;
mod capture_job;
mod cli;
mod controls_gallery;
mod dialogs;
mod history;
mod history_ui;
mod hotkey_settings;
mod hotkeys;
mod integration;
mod main_menu;
mod menu_settings;
mod main_window;
mod message_box;
mod ocr;
mod image_editor;
mod ocr_window;
mod image_tools;
mod image_effects;
mod recording;
mod region_render;
mod remote_storage_browser;
mod region_session;
mod render;
mod screen_tools;
mod scrolling_capture;
mod settings_form;
mod shell;
#[cfg(windows)]
mod shortcut;
mod sound;
mod start_screen;
mod startup;
mod task_actions;
mod task_editors;
mod task_settings_window;
mod task_view;
mod tasks;
#[cfg(test)]
mod test_mock;
mod test_pattern;
mod print;
mod thumbnail;
mod toolbar_editor;
mod transparent_region;
mod tray;
mod tray_menu;
mod update;
mod upload;
mod watch_folder;
mod window;

use std::sync::Arc;
use std::time::{Duration, Instant};

use sharex_ui::gpu::Gpu;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, WindowEvent};
use sharex_settings::paths::is_command_present;
use sharex_settings::types::{Point, Size};
use sharex_settings::{AppPaths, HotkeyType, SettingManager, SystemOptions, TaskSettings};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Icon, WindowAttributes, WindowButtons, WindowId, WindowLevel};

use capture::{CaptureError, CaptureJob};
use hotkeys::HotkeyManager;
use main_menu::MenuCommand;
use main_window::{Action, MainWindow};
use region_session::RegionSession;
use sharex_capture::region::Outcome;
use transparent_region::TransparentRegion;
use tray::{TrayEvent, TrayMenuEntries};
use tray_menu::TrayMenu;
use message_box::{DialogResult, MessageBox, MessageBoxIcon};
use window::EguiWindow;

/// What other threads tell the event loop.
#[derive(Debug)]
pub enum UserEvent {
    /// A global hotkey was pressed (its `HotkeyInfo.ID`).
    Hotkey(u16),
    /// The tray icon was clicked, or its native menu used.
    Tray(TrayEvent),
    /// A task's thread or a thumbnail decoder finished.
    Task(tasks::TaskEvent),
    /// A task's thread asks for a window (the before upload window, ...).
    Ui(upload::UiRequest),
    /// The screen recording's worker or tray icon.
    Recording(recording::Event),
    /// The automatic update check found an update.
    Update(update::UpdateEvent),
    /// A watch folder's new file is ready (`FileWatcherTrigger`): the
    /// watcher's id and the file.
    WatchFolder(u64, std::path::PathBuf),
    /// A later launch's command line (`SingleInstanceManager.ArgumentsReceived`).
    Arguments(Vec<String>),
    /// The browser extension's image, downloaded
    /// (`ProcessImagesDuringExtensionUpload`).
    ExtensionImage(Box<cli::ExtensionImage>),
    /// A worker finished (the scrolling capture's loop): look again.
    Wake,
    /// The auto capture window's tray icon.
    AutoCaptureTray(TrayEvent),
}

fn main() {
    cli::START.get_or_init(Instant::now);
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--render") {
        if let Err(e) = render::run(&args[1..]) {
            eprintln!("sharexrs --render: {e}");
            std::process::exit(1);
        }
        return;
    }
    if args.first().map(String::as_str) == Some("--test-pattern") {
        if let Err(e) = test_pattern::run(&args[1..]) {
            eprintln!("sharexrs --test-pattern: {e}");
            std::process::exit(1);
        }
        return;
    }
    // `Program.Main`: where the profile is, then one running copy
    // (`SingleInstanceManager`): a later launch passes its command line to
    // it and leaves. `-multi` (`-m`) runs another copy anyway.
    let system = SystemOptions::read();
    let paths = AppPaths::initialize(&args, &system);
    let multi_instance = is_command_present(&args, &["multi", "m"]);
    let (mutex_name, pipe_name) = cli::instance_names(&paths);
    let (arguments_tx, arguments_rx) = std::sync::mpsc::channel::<Vec<String>>();
    let instance = sharex_helpers::single_instance::SingleInstanceManager::new(
        &mutex_name,
        &pipe_name,
        !multi_instance,
        &args,
        Arc::new(move |a| {
            let _ = arguments_tx.send(a);
        }),
    );
    if instance.is_single_instance && !instance.is_first_instance {
        return;
    }
    // What an earlier update left behind (the replaced app or files).
    sharex_update::install::init();
    // `Program.Run`: the settings (`SettingManager.LoadInitialSettings`),
    // before any window.
    if let Some(method) = &paths.detection_method {
        eprintln!("Personal path detection method: {method}");
    }
    let settings = SettingManager::load(paths, system);
    settings.paths.create_application_folders(Some(&settings.settings));
    // `FileTypeRegistration.RegisterMissingExtensions`: .sxcu and .sxie
    // open with this copy, unless it's portable.
    if !settings.paths.portable {
        integration::register_missing_extensions();
    }
    // `LoadApplicationConfig`'s `MigrateHistoryFile` and `HistoryConnect`.
    history::connect(&settings.paths);
    // `LanguageHelper.ChangeLanguage(Settings.Language)`.
    appearance::change_language(&settings.settings.Language);

    #[cfg(target_os = "macos")]
    sharex_capture::platform::macos::ask_for_screen_recording();
    #[allow(unused_mut)]
    let mut builder = EventLoop::<UserEvent>::with_user_event();
    // No Dock icon until the main window opens (docs/MACOS-LEARNINGS.md 3.5).
    #[cfg(target_os = "macos")]
    {
        use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
        builder.with_activation_policy(ActivationPolicy::Accessory);
    }
    let event_loop = builder.build().expect("event loop");
    let proxy = event_loop.create_proxy();
    // Later launches' command lines, to the event loop
    // (`SingleInstanceCommandRouter.ArgumentsReceived`).
    {
        let proxy = proxy.clone();
        let _ = std::thread::Builder::new().name("arguments".into()).spawn(move || {
            for a in arguments_rx {
                if proxy.send_event(UserEvent::Arguments(a)).is_err() {
                    break;
                }
            }
        });
    }
    let hotkey_proxy = proxy.clone();
    let host = hotkeys::platform_host(Arc::new(move |id| {
        let _ = hotkey_proxy.send_event(UserEvent::Hotkey(id));
    }));
    let uploads = upload::app::Uploads::load(&settings);
    upload::app::set_dialog_hooks(proxy.clone());
    let updates = update::Updates::new(proxy.clone(), settings.paths.portable);
    let mut app = App {
        gpu: None,
        main: None,
        state: MainWindow::new(),
        cursor: None,
        swallow_release: false,
        settings,
        uploads,
        hotkeys: HotkeyManager::new(host),
        dialog: None,
        started: false,
        // `-NoHotkeys`: `StartupOptions.IgnoreHotkeyWarning`.
        ignore_hotkey_warning: is_command_present(&args, &["NoHotkeys"]),
        region: None,
        pending_capture: None,
        ocr_region: None,
        last_regions: Vec::new(),
        proxy,
        tray: None,
        tray_menu: None,
        tray_entries: TrayMenuEntries::default(),
        tray_left_clicks: 0,
        tray_click_at: None,
        transparent: None,
        transparent_grab: None,
        tray_progress: -1,
        modifiers: Default::default(),
        start_window: None,
        settings_window: None,
        hotkey_window: None,
        task_window: None,
        dialogs: Vec::new(),
        pending_dialogs: Vec::new(),
        current_theme: egui::Theme::Dark,
        system_theme: None,
        // `StartupOptions.SilentRun` (`-silent`, `-s`): start in the tray.
        silent: is_command_present(&args, &["silent", "s"]),
        recording: None,
        ffmpeg_options: None,
        ffmpeg_download: None,
        updates,
        watch_folders: Default::default(),
        // `ApplicationCommandLine.Initialize`: run once startup is done.
        router: cli::CommandRouter::new(args.clone()),
        // `StartupOptions.AutoClose` (`-AutoClose`): exit once no task is busy.
        auto_close: is_command_present(&args, &["AutoClose"]),
        exit_requested: false,
        queued_dialogs: Vec::new(),
        screen_tools: Default::default(),
        scrolling: Default::default(),
        auto_capture: Default::default(),
        desktop_icons: None,
    };
    event_loop.run_app(&mut app).expect("run");
    drop(instance);
}

/// A message box in its own window (`MessageBoxWindow`).
struct Dialog {
    window: EguiWindow,
    message: MessageBox,
    /// What the answer is for.
    purpose: DialogPurpose,
}

/// Why a message box is open.
enum DialogPurpose {
    /// The "Hotkey registration failed" warning at startup.
    HotkeysFailed,
    /// `TaskHelpers.ConfirmFileDelete` for these tasks' files.
    DeleteFiles(Vec<u64>),
    /// A message that only needs reading (`CaptureCustomWindow`'s window
    /// not found).
    Information,
    /// Custom uploader settings' "Clear all..." confirmation.
    ClearCustomUploaders,
    /// FFmpeg is missing (`CheckFFmpeg`): Yes downloads it.
    DownloadFFmpeg(std::path::PathBuf),
    /// The downloaded installer isn't signed: Yes runs it anyway.
    UpdateSignature,
    /// A question a command line asked (importing a custom uploader or an
    /// image effect).
    CommandLine(cli::CliQuestion),
}

struct App {
    gpu: Option<Gpu>,
    main: Option<EguiWindow>,
    state: MainWindow,
    /// The pointer in the main window, in points.
    cursor: Option<egui::Pos2>,
    /// A press closed a menu (light dismiss); its release goes nowhere either.
    swallow_release: bool,
    /// `ApplicationState.Settings` and `HotkeysConfig`.
    settings: SettingManager,
    /// `ApplicationState.UploadersConfig` and the queued upload tasks.
    uploads: upload::app::Uploads,
    /// `ApplicationState.HotkeyManager`.
    hotkeys: HotkeyManager,
    /// The open message box (the hotkey failure one, at startup).
    dialog: Option<Dialog>,
    started: bool,
    ignore_hotkey_warning: bool,
    /// The region capture overlay, while it's open.
    region: Option<Region>,
    /// A capture waiting for its screenshot delay or for the main window to
    /// hide.
    pending_capture: Option<PendingCapture>,
    /// Where the next region capture goes when it's for OCR.
    ocr_region: Option<ocr_window::OcrRegion>,
    /// `RegionCaptureIntegration.LastRegionRectangles`.
    last_regions: Vec<sharex_capture::Rect>,
    /// For tasks' threads and the tray icon to report back to the event loop.
    proxy: winit::event_loop::EventLoopProxy<UserEvent>,
    /// `MainForm.TrayIconService`, while `ShowTray`.
    tray: Option<tray::Tray>,
    /// The tray menu (Windows), while it's open.
    tray_menu: Option<TrayMenu>,
    /// The commands of the native tray menu (macOS, Linux), by index.
    tray_entries: TrayMenuEntries,
    /// `DesktopTrayIconService._leftClickCount` and its single click timer.
    tray_left_clicks: u32,
    tray_click_at: Option<Instant>,
    /// Capture › Region (Transparent), while it's open.
    transparent: Option<Transparent>,
    /// A transparent region chosen, captured once its windows are gone
    /// (`CaptureRegionTransparent.CaptureDelay`).
    transparent_grab: Option<(Instant, sharex_capture::Rect, TaskSettings, bool)>,
    /// `TaskManager.lastIconStatus`.
    tray_progress: i32,
    /// The keyboard's modifiers in the main window.
    modifiers: winit::keyboard::ModifiersState,
    /// The Start screen, on first run.
    start_window: Option<app_windows::StartWindow>,
    /// Application settings, while it's open.
    settings_window: Option<app_windows::SettingsWindow>,
    /// Hotkey settings, while it's open.
    hotkey_window: Option<app_windows::HotkeyWindow>,
    /// Task settings (the defaults' or a hotkey's), while it's open.
    task_window: Option<app_windows::TaskWindow>,
    /// The upload windows that are open (`dialogs`).
    dialogs: Vec<dialogs::DialogWindow>,
    /// Windows to open once the event loop is at hand (a finished task's
    /// after upload window).
    pending_dialogs: Vec<Box<dyn dialogs::Dialog>>,
    /// `ThemeManager.GetCurrentTheme()`.
    current_theme: egui::Theme,
    /// The OS's light or dark mode, for `UseSystemTheme`.
    system_theme: Option<winit::window::Theme>,
    silent: bool,
    /// `ScreenRecordManager` while `IsRecording`.
    recording: Option<recording::Recording>,
    /// The FFmpeg options window, while it's open.
    ffmpeg_options: Option<recording::ffmpeg_options::FFmpegOptionsHost>,
    /// FFmpeg being downloaded.
    ffmpeg_download: Option<recording::ffmpeg_options::Download>,
    /// `ApplicationState.UpdateManager`.
    updates: update::Updates,
    /// `ApplicationState.WatchFolderManager`.
    watch_folders: watch_folder::WatchFolderManager,
    /// `SingleInstanceCommandRouter` and the first command line.
    router: cli::CommandRouter,
    /// `StartupOptions.AutoClose`.
    auto_close: bool,
    /// Exit at the next chance (`-AutoClose` once the tasks are done).
    exit_requested: bool,
    /// Message boxes waiting for the open one to close.
    queued_dialogs: Vec<(MessageBox, DialogPurpose)>,
    /// The Screen tools' windows and overlays that are open.
    screen_tools: screen_tools::ScreenTools,
    /// Capture › Scrolling capture's window and capture.
    scrolling: scrolling_capture::ScrollingCapture,
    /// Capture › Auto capture's window and timer.
    auto_capture: auto_capture::AutoCapture,
    /// `CaptureAutoHideDesktopIcons`: the icons hidden for the capture
    /// running, shown again when it ends (or when this is dropped).
    desktop_icons: Option<sharex_capture::screenshot::DesktopIconsHidden>,
}

/// `CaptureBase.CaptureAsync` between the request and the capture: the
/// screenshot delay, then (from the main window's menu) hiding it for 250 ms.
struct PendingCapture {
    at: Instant,
    job: CaptureJob,
    task: TaskSettings,
    auto_hide: bool,
    /// The main window was hidden for it (`showMainForm`).
    hidden: bool,
    /// The desktop icons and the main window were hidden (if asked) and
    /// waited for.
    prepared: bool,
}

/// An open Capture › Region (Transparent) and the task it runs for.
struct Transparent {
    region: TransparentRegion,
    task: TaskSettings,
    hidden: bool,
}

/// An open region capture and the task it runs for.
struct Region {
    session: RegionSession,
    task: TaskSettings,
    hidden: bool,
}

impl App {
    fn open_main_window(&mut self, event_loop: &ActiveEventLoop) -> Result<(), String> {
        let gpu = match &self.gpu {
            Some(g) => g,
            None => self.gpu.insert(Gpu::new()?),
        };
        let icon = Icon::from_rgba(app_icon::rgba64(), 64, 64).ok();
        let attrs = main_window_attributes()
            .with_title(&self.state.title)
            .with_inner_size(LogicalSize::new(main_window::WIDTH, main_window::HEIGHT))
            .with_min_inner_size(LogicalSize::new(main_window::MIN_WIDTH, 200.0))
            .with_visible(false)
            .with_window_icon(icon);
        let window = event_loop.create_window(attrs).map_err(|e| e.to_string())?;
        // Capture › Monitor (`DesktopScreen.AllScreens`).
        self.state.menu_state.monitors = event_loop
            .available_monitors()
            .map(|m| (m.position().x, m.position().y, m.size().width, m.size().height))
            .collect();
        let w = EguiWindow::new(window, gpu, Some(5.0))?;
        // Open at the sidebar's height, as ShareX does when it has no
        // remembered size (`ConfigureWindowHeightFromNavigation`).
        let min_height = self.state.min_height(w.ctx());
        w.window.set_min_inner_size(Some(LogicalSize::new(main_window::MIN_WIDTH, min_height)));
        let height = self.state.opening_height(w.ctx());
        let _ = w.window.request_inner_size(LogicalSize::new(main_window::WIDTH, height));
        center(&w.window, event_loop);
        self.restore_window_bounds(&w.window, min_height);
        set_theme(&w, self.theme());
        w.window.set_visible(true);
        w.window.request_redraw();
        self.main = Some(w);
        #[cfg(target_os = "macos")]
        tray::set_dock_icon(true);
        Ok(())
    }

    fn act(&mut self, event_loop: &ActiveEventLoop, action: Action) {
        if let Action::Task(action) = action {
            self.task_action(event_loop, action);
            return;
        }
        let Some(w) = &self.main else { return };
        match action {
            Action::DragWindow => {
                let _ = w.window.drag_window();
            }
            Action::ToggleMaximized => {
                w.window.set_maximized(!w.window.is_maximized());
            }
            Action::Minimize => w.window.set_minimized(true),
            Action::Close => {
                self.close_main_window(event_loop);
                return;
            }
            Action::ToggleTopmost => {
                self.state.topmost = !self.state.topmost;
                w.window.set_window_level(if self.state.topmost {
                    WindowLevel::AlwaysOnTop
                } else {
                    WindowLevel::Normal
                });
            }
            Action::ToggleTheme => {
                // `OnThemeToggleClick`.
                let next = match w.ctx().theme() {
                    egui::Theme::Dark => egui::Theme::Light,
                    egui::Theme::Light => egui::Theme::Dark,
                };
                let options = self.settings.settings.theme_options();
                options.Theme = if next == egui::Theme::Dark { "Dark" } else { "Light" }.into();
                options.UseSystemTheme = false;
                let _ = self.settings.save_application_config(false);
                self.apply_appearance();
                return;
            }
            // `MainMenuBuilder_ApplicationSettings`.
            Action::Nav(7, _) => {
                self.show_application_settings(event_loop);
                return;
            }
            // `MainMenuBuilder_TaskSettings`, `MainMenuBuilder_HotkeySettings`.
            Action::Nav(8, _) => {
                self.show_task_settings(event_loop, app_windows::TaskTarget::Default);
                return;
            }
            Action::Nav(9, _) => {
                self.show_hotkey_settings(event_loop);
                return;
            }
            // `MainMenuBuilder_DestinationSettings`.
            Action::Nav(10, _) => {
                self.show_destination_settings(event_loop, None);
                return;
            }
            // `MainMenuBuilder_CustomUploaderSettings`.
            Action::Nav(11, _) => {
                self.show_custom_uploader_settings(event_loop);
                return;
            }
            // `MainMenuBuilder_History`, `_ImageHistory`, `_Stats`.
            Action::Nav(13, _) => {
                self.show_history(event_loop);
                return;
            }
            Action::Nav(14, _) => {
                self.show_image_history(event_loop);
                return;
            }
            Action::Nav(15, _) => {
                self.show_stats(event_loop);
                return;
            }
            Action::Nav(i, _) => eprintln!("sidebar entry {i}: not ported yet"),
            Action::Menu(command) => {
                self.menu_command(event_loop, command, false);
                return;
            }
            Action::Task(_) => {}
        }
        w.window.request_redraw();
    }

    /// What a menu entry does (`MainMenuBuilder`'s lambdas): the main
    /// window's menus hide it while capturing (`autoHide`), the tray menu's
    /// don't (`MainMenuBuilder(trayMenu: true)`).
    fn menu_command(&mut self, event_loop: &ActiveEventLoop, command: MenuCommand, tray_menu: bool) {
        let auto_hide = !tray_menu;
        // `TaskHelpers.OCRImage(autoHideMainWindow: !_trayMenu)`.
        if command == MenuCommand::Tool("MainMenuBuilder_OCR") {
            let task = TaskSettings::default_task_settings(&self.settings.settings.DefaultTaskSettings);
            self.ocr_capture(task, auto_hide);
            return;
        }
        if command == MenuCommand::Tool("MainMenuBuilder_RemoteStorageBrowser") {
            self.open_remote_storage_browser(event_loop);
            return;
        }
        if let MenuCommand::Tool(key) = command
            && (self.open_image_tool(event_loop, key, None) || self.open_screen_tool(event_loop, key))
        {
            return;
        }
        // `new Capture…().Capture(autoHide)`: the default task settings.
        let task = || TaskSettings::default_task_settings(&self.settings.settings.DefaultTaskSettings);
        let job = match &command {
            MenuCommand::CaptureFullscreen => Some(CaptureJob::Fullscreen),
            MenuCommand::CaptureWindow(handle) => Some(CaptureJob::Window(*handle)),
            &MenuCommand::CaptureMonitor(x, y, w, h) => {
                Some(CaptureJob::Monitor(sharex_capture::Rect::new(x, y, w as i32, h as i32)))
            }
            MenuCommand::CaptureRegion => Some(CaptureJob::Region),
            MenuCommand::CaptureRegionTransparent => Some(CaptureJob::RegionTransparent),
            MenuCommand::CaptureLastRegion => Some(CaptureJob::LastRegion),
            _ => None,
        };
        if let Some(job) = job {
            let task = task();
            self.capture(job, task, auto_hide);
            return;
        }
        // `TaskHelpers.OpenScrollingCapture()`, `OpenAutoCapture()`: the
        // default task settings.
        if let MenuCommand::ScrollingCapture | MenuCommand::AutoCapture = command {
            let task = task();
            if command == MenuCommand::ScrollingCapture {
                self.open_scrolling_capture(event_loop, task);
            } else {
                self.open_auto_capture(event_loop, task);
            }
            return;
        }
        // `TaskHelpers.StartScreenRecording(output, Region)`: the default
        // task settings, and no hiding of the main window.
        if let MenuCommand::ScreenRecording | MenuCommand::ScreenRecordingGif = command {
            let job = if command == MenuCommand::ScreenRecordingGif { HotkeyType::ScreenRecorderGIF } else { HotkeyType::ScreenRecorder };
            let task = task();
            self.run_recording_job(event_loop, &job, &task, None);
            return;
        }
        if self.upload_menu_command(event_loop, &command) {
            return;
        }
        if menu_settings::apply(&mut self.settings.settings.DefaultTaskSettings, &command) {
            // `staysOpenOnClick` entries: the menu shows the change at once.
            self.refresh_menus();
            return;
        }
        let capture = self.settings.settings.DefaultTaskSettings.CaptureSettings.get_or_insert_default();
        match command {
            MenuCommand::ToggleShowCursor => capture.ShowCursor = !capture.ShowCursor,
            // `MainWindowIntegration.SetScreenshotDelay`.
            MenuCommand::SetScreenshotDelay(delay) => capture.ScreenshotDelay = delay as f64,
            MenuCommand::ToggleScreenshotDelayJustOnce => {
                let s = &mut self.settings.settings;
                s.ScreenshotDelayJustOnce = !s.ScreenshotDelayJustOnce;
            }
            MenuCommand::ToggleHotkeys => self.toggle_hotkeys(),
            MenuCommand::ShowMainWindow => self.show_main_window(event_loop),
            MenuCommand::ApplicationSettings => self.show_application_settings(event_loop),
            MenuCommand::TaskSettings => self.show_task_settings(event_loop, app_windows::TaskTarget::Default),
            MenuCommand::HotkeySettings => self.show_hotkey_settings(event_loop),
            MenuCommand::DestinationSettings => self.show_destination_settings(event_loop, None),
            MenuCommand::CustomUploaderSettings => self.show_custom_uploader_settings(event_loop),
            MenuCommand::History => self.show_history(event_loop),
            MenuCommand::ImageHistory => self.show_image_history(event_loop),
            MenuCommand::Exit => self.exit(event_loop),
            other => eprintln!("menu: {other:?}: not ported yet"),
        }
        self.refresh_menus();
    }

    /// `MainWindow.RefreshMenus`: the menus show the settings as they are.
    fn refresh_menus(&mut self) {
        let s = &self.settings.settings;
        let capture = s.DefaultTaskSettings.CaptureSettings.clone().unwrap_or_default();
        let m = &mut self.state.menu_state;
        m.show_cursor = capture.ShowCursor;
        m.screenshot_delay = capture.ScreenshotDelay as f32;
        m.screenshot_delay_just_once = s.ScreenshotDelayJustOnce;
        m.hotkeys_disabled = s.DisableHotkeys;
        menu_settings::fill(m, &s.DefaultTaskSettings);
        m.list_windows = true;
        // `DesktopScreen.AllScreens`.
        m.monitors = sharex_capture::screenshot::screens()
            .iter()
            .map(|s| (s.bounds.x, s.bounds.y, s.bounds.w as u32, s.bounds.h as u32))
            .collect();
        if cfg!(not(windows))
            && let Some(tray) = &mut self.tray
        {
            // The native menu is built ahead (Avalonia builds it as it opens).
            let mut state = self.state.menu_state.clone();
            state.list_windows = false;
            state.windows = main_menu::WindowItem::list();
            self.tray_entries = TrayMenuEntries::new(&main_menu::tray_menu(&state));
            tray.set_menu(self.tray_entries.native.clone());
        }
        if let Some(w) = &self.main {
            w.window.request_redraw();
        }
    }
}

impl App {
    /// `CaptureBase.Capture`: waits for the screenshot delay, hides the
    /// main window when asked (`autoHideForm`, from the main window's
    /// menus), then captures.
    fn capture(&mut self, job: CaptureJob, task: TaskSettings, auto_hide: bool) {
        if self.region.is_some() || self.transparent.is_some() || self.pending_capture.is_some() {
            return;
        }
        let delay = task.CaptureSettings.as_ref().map_or(0.0, |c| c.ScreenshotDelay).max(0.0);
        // `ScreenshotDelayJustOnce`: the default delay goes back to 0.
        if delay > 0.0 && task.UseDefaultCaptureSettings && self.settings.settings.ScreenshotDelayJustOnce {
            self.settings.settings.DefaultTaskSettings.CaptureSettings.get_or_insert_default().ScreenshotDelay = 0.0;
            self.refresh_menus();
        }
        let at = Instant::now() + Duration::from_secs_f64(delay);
        self.pending_capture = Some(PendingCapture { at, job, task, auto_hide, hidden: false, prepared: false });
    }

    /// Moves a pending capture on when it's due.
    fn advance_pending_capture(&mut self, event_loop: &ActiveEventLoop) {
        if let Some((at, rect, ..)) = &self.transparent_grab
            && *at <= Instant::now()
        {
            let rect = *rect;
            let Some((_, _, task, hidden)) = self.transparent_grab.take() else { return };
            self.grab_transparent_region(event_loop, rect, task, hidden);
        }
        let Some(pending) = self.pending_capture.as_ref().filter(|p| p.at <= Instant::now()) else { return };
        let main_handle = self.main.as_ref().and_then(|w| sharex_capture::platform::window_handle(&w.window));
        if !pending.prepared {
            // `CaptureInternalAsync`: the desktop icons, then the main window,
            // hidden, and 250 ms for either.
            let mut wait = false;
            if pending.task.CaptureSettings.as_ref().is_some_and(|c| c.CaptureAutoHideDesktopIcons)
                && let Some(icons) = sharex_capture::screenshot::DesktopIconsHidden::hide(&sharex_capture::screenshot::screens())
            {
                self.desktop_icons = Some(icons);
                wait = true;
            }
            let hide_main = pending.auto_hide
                && pending.job.allow_auto_hide(main_handle)
                && self.main.as_ref().is_some_and(|w| w.window.is_visible() != Some(false));
            if hide_main {
                // `MainWindowIntegration.Hide`.
                self.save_window_bounds();
                if let Some(w) = &self.main {
                    w.window.set_visible(false);
                }
                wait = true;
            }
            if let Some(pending) = self.pending_capture.as_mut() {
                pending.prepared = true;
                pending.hidden = hide_main;
                if wait {
                    pending.at = Instant::now() + Duration::from_millis(250);
                    return;
                }
            }
        }
        let Some(PendingCapture { job, task, hidden, .. }) = self.pending_capture.take() else { return };
        match job {
            CaptureJob::Region => self.start_region_capture(event_loop, task, hidden),
            // With no last region, Last region is a region capture.
            CaptureJob::LastRegion if self.last_regions.is_empty() => {
                self.start_region_capture(event_loop, task, hidden)
            }
            CaptureJob::RegionTransparent => self.start_transparent_capture(event_loop, task, hidden),
            job => {
                let result = capture::execute(&job, &task, &self.last_regions);
                // `finally`: the desktop icons come back, then the main window.
                self.desktop_icons = None;
                if hidden {
                    self.show_main_window(event_loop);
                }
                match result {
                    Ok(Some(metadata)) => self.after_capture(metadata, task, matches!(job, CaptureJob::LastRegion)),
                    Ok(None) => {}
                    Err(CaptureError::WindowNotFound) => {
                        let message = MessageBox::ok(
                            "ShareX",
                            sharex_strings::sharex::UnableToFindAWindowWithSpecifiedWindowTitle(),
                            MessageBoxIcon::Information,
                        );
                        if let Err(e) = self.open_dialog(event_loop, message, DialogPurpose::Information) {
                            eprintln!("couldn't open the message box: {e}");
                        }
                    }
                    Err(CaptureError::Failed(e)) => eprintln!("capture: {e}"),
                }
            }
        }
    }

    /// `CaptureRegionTransparent.ExecuteAsync`: a rectangle is drawn over
    /// the live screen (the active monitor in active monitor mode).
    fn start_transparent_capture(&mut self, event_loop: &ActiveEventLoop, task: TaskSettings, hidden: bool) {
        let options = task.CaptureSettings.clone().unwrap_or_default().RegionCaptureOptions;
        let screens = sharex_capture::screenshot::screens();
        let cursor = sharex_capture::screenshot::cursor_position().unwrap_or_default();
        let bounds = if options.ActiveMonitorMode {
            sharex_capture::screenshot::active_screen_bounds(&screens, cursor)
        } else {
            sharex_capture::screenshot::screen_bounds(&screens)
        };
        let opened = match &self.gpu {
            Some(g) => Ok(g),
            None => Gpu::new().map(|g| &*self.gpu.insert(g)),
        }
        .and_then(|gpu| TransparentRegion::open(event_loop, gpu, &screens, bounds, options.ActiveMonitorMode));
        match opened {
            Ok(region) => self.transparent = Some(Transparent { region, task, hidden }),
            Err(e) => {
                eprintln!("transparent region capture: {e}");
                self.desktop_icons = None;
                if hidden {
                    self.show_main_window(event_loop);
                }
            }
        }
    }

    /// The transparent region's windows closed: the region is remembered
    /// (`SetLastRegion`) and captured 50 ms later, once they're off screen.
    fn finish_transparent_capture(&mut self, event_loop: &ActiveEventLoop, selection: Option<sharex_capture::Rect>) {
        let Some(Transparent { task, hidden, .. }) = self.transparent.take() else { return };
        match selection {
            Some(rect) => {
                self.last_regions = vec![rect];
                self.transparent_grab = Some((Instant::now() + Duration::from_millis(50), rect, task, hidden));
            }
            None => {
                self.desktop_icons = None;
                if hidden {
                    self.show_main_window(event_loop);
                }
            }
        }
    }

    fn grab_transparent_region(
        &mut self,
        event_loop: &ActiveEventLoop,
        rect: sharex_capture::Rect,
        task: TaskSettings,
        hidden: bool,
    ) {
        let screens = sharex_capture::screenshot::screens();
        // `TaskHelpers.GetScreenshot(taskSettings).CaptureRectangle(rect)`.
        let image = capture::task_screenshot(&task).capture_rectangle(&screens, rect);
        self.desktop_icons = None;
        if hidden {
            self.show_main_window(event_loop);
        }
        match image {
            Some(image) => {
                let metadata = capture::create_metadata(image, Some(rect), Some("explorer"));
                self.after_capture(metadata, task, true);
            }
            None => eprintln!("transparent region capture: couldn't capture the screen"),
        }
    }

    /// `CaptureRegion.ExecuteRegionCaptureAvaloniaAsync`: freezes the
    /// screen and opens the overlay.
    fn start_region_capture(&mut self, event_loop: &ActiveEventLoop, task: TaskSettings, hidden: bool) {
        let dark = self.theme() == egui::Theme::Dark;
        let opened = capture_job::prepare(&task, self.last_regions.clone()).and_then(|request| {
            let gpu = match &self.gpu {
                Some(g) => g,
                None => self.gpu.insert(Gpu::new()?),
            };
            RegionSession::open(event_loop, gpu, request, dark)
        });
        match opened {
            Ok(session) => self.region = Some(Region { session, task, hidden }),
            Err(e) => {
                eprintln!("region capture: {e}");
                self.desktop_icons = None;
                if hidden {
                    self.show_main_window(event_loop);
                }
            }
        }
    }

    /// `MainWindowIntegration.Activate` (`MainWindow.ShowAndActivate`):
    /// the tray menu closes first; a hidden window comes back (centred if
    /// no monitor shows where it was), a minimized one is restored, and
    /// it's brought to the front.
    fn show_main_window(&mut self, event_loop: &ActiveEventLoop) {
        self.tray_menu = None;
        let Some(w) = &self.main else {
            if let Err(e) = self.open_main_window(event_loop) {
                eprintln!("couldn't open the main window: {e}");
            }
            return;
        };
        if w.window.is_visible() == Some(false) {
            let on_screen = w.window.outer_position().is_ok_and(|p| {
                event_loop.available_monitors().any(|m| {
                    let (mp, ms) = (m.position(), m.size());
                    p.x >= mp.x && p.y >= mp.y && p.x < mp.x + ms.width as i32 && p.y < mp.y + ms.height as i32
                })
            });
            if !on_screen {
                center(&w.window, event_loop);
            }
            w.window.set_visible(true);
        }
        if w.window.is_minimized() == Some(true) {
            w.window.set_minimized(false);
        }
        w.window.focus_window();
        w.window.request_redraw();
        #[cfg(target_os = "macos")]
        tray::set_dock_icon(true);
    }

    /// `MainWindow.HideToTray`.
    fn hide_to_tray(&mut self) {
        self.save_window_bounds();
        if let Some(w) = &self.main {
            w.window.set_visible(false);
        }
        #[cfg(target_os = "macos")]
        tray::set_dock_icon(false);
    }

    /// `MainWindow.OnClosing`: with the tray icon, closing hides the window
    /// to the tray and saves the settings (the first time, ShareX says so
    /// in a notification); without it, ShareX exits.
    fn close_main_window(&mut self, event_loop: &ActiveEventLoop) {
        if !self.settings.settings.ShowTray || self.tray.is_none() {
            self.exit(event_loop);
            return;
        }
        self.hide_to_tray();
        if self.settings.settings.FirstTimeMinimizeToTray {
            // `ShowNotificationTip`: notifications aren't ported yet.
            eprintln!("{}", sharex_strings::sharex::ShareXIsMinimizedToTheSystemTray());
            self.settings.settings.FirstTimeMinimizeToTray = false;
        }
        if let Err(e) = self.settings.save_all() {
            eprintln!("couldn't save the settings: {e}");
        }
    }

    /// `ApplicationLifecycle.ForceClose`.
    fn exit(&mut self, event_loop: &ActiveEventLoop) {
        self.tray_menu = None;
        event_loop.exit();
    }

    /// The overlay closed: the main window comes back if it was hidden, and
    /// a capture goes through the after capture tasks
    /// (`CaptureBase.AfterCapture`, `UploadManager.RunImageTask`).
    fn finish_region_capture(&mut self, event_loop: &ActiveEventLoop, mut outcome: Outcome) {
        let Some(Region { session, task, hidden }) = self.region.take() else { return };
        // The overlay changes its options as ShareX's does (magnifier size,
        // toolbar place), on the settings the task takes them from.
        // The annotation tools' options too (`ToolsSettingsReference`).
        if let Some(o) = session.capture.editor_options()
            && task.UseDefaultToolsSettings
            && let Some(t) = self.settings.settings.DefaultTaskSettings.ToolsSettings.as_mut()
            && o != sharex_image_editor::ImageEditorOptions::from_value(&t.ImageEditorOptions)
        {
            t.ImageEditorOptions = o.to_value();
        }
        let options = session.capture.options().clone();
        if task.UseDefaultCaptureSettings
            && let Some(c) = self.settings.settings.DefaultTaskSettings.CaptureSettings.as_mut()
        {
            c.RegionCaptureOptions = options;
        }
        // `CaptureRegion.ExecuteRegionCaptureAvaloniaAsync`: a window picked
        // whole gets its transparency (`CaptureTransparent`), the overlay
        // gone and before the main window comes back.
        if self.screen_tools.pin_region.is_none()
            && self.ocr_region.is_none()
            && let Outcome::Captured(result) = &mut outcome
        {
            drop(session);
            if let Some(image) = capture::region_window_transparent(&task, result.window_handle, &result.image, result.screen_rectangle) {
                result.image = image;
            }
        }
        // `CaptureBase`'s `finally`: the desktop icons come back.
        self.desktop_icons = None;
        if hidden {
            self.show_main_window(event_loop);
        }
        if let Some(target) = self.screen_tools.pin_region.take() {
            let captured = match &outcome {
                Outcome::Captured(result) => {
                    self.last_regions = result.regions.clone();
                    Some((result.image.clone(), (result.screen_rectangle.x, result.screen_rectangle.y)))
                }
                _ => None,
            };
            self.pin_region_captured(event_loop, target, captured);
            return;
        }
        if let Some(target) = self.ocr_region.take() {
            let image = match &outcome {
                Outcome::Captured(result) => {
                    self.last_regions = result.regions.clone();
                    Some(result.image.clone())
                }
                _ => None,
            };
            self.ocr_region_captured(event_loop, target, image);
            return;
        }
        let Outcome::Captured(result) = outcome else { return };
        self.last_regions = result.regions.clone();
        let metadata = capture_job::Metadata::from_result(&result);
        self.after_capture(metadata, task, true);
    }

    /// `CaptureBase.AfterCapture`: the capture sound, then the image task
    /// (`UploadManager.RunImageTask`) off the event loop. `region`: a region,
    /// last region or transparent region capture (`ImageEffectOnlyRegionCapture`).
    fn after_capture(&mut self, metadata: capture_job::Metadata, mut task: TaskSettings, region: bool) {
        if task.GeneralSettings.as_ref().is_none_or(|g| g.PlaySoundAfterCapture) {
            sound::play_capture();
        }
        if !region && task.ImageSettings.as_ref().is_some_and(|s| s.ImageEffectOnlyRegionCapture) {
            task.AfterCaptureJob.remove(sharex_settings::AfterCaptureTasks::AddImageEffects);
        }
        let parent = self.settings.paths.screenshots_parent_folder(Some(&self.settings.settings));
        let effects = self.settings.paths.image_effects_folder();
        let job = capture_job::ImageTask::new(metadata, task, &mut self.settings.settings, &parent, effects);
        self.run_image_task(job);
        self.update_progress_ui();
    }

    /// `UploadManager.RunImageTask(bmp, taskSettings)`: an image made by a
    /// tool (the combiner) through the task's after capture tasks.
    pub(crate) fn run_image_task_for(&mut self, image: image::RgbaImage, task: TaskSettings) {
        let metadata = capture_job::Metadata { image, window_title: None, process_name: None };
        let parent = self.settings.paths.screenshots_parent_folder(Some(&self.settings.settings));
        let effects = self.settings.paths.image_effects_folder();
        let job = capture_job::ImageTask::new(metadata, task, &mut self.settings.settings, &parent, effects);
        self.run_image_task(job);
        self.update_progress_ui();
    }

    /// `TaskManager.UpdateProgressUI`, for the tray: while tasks upload,
    /// the tray icon shows their average progress and its tooltip the
    /// window's title with it.
    fn update_progress_ui(&mut self) {
        let tasks = &self.state.tasks.manager.tasks;
        let working: Vec<_> = tasks.iter().filter(|t| t.status == tasks::TaskStatus::Working).collect();
        let with_progress: Vec<f64> = working.iter().filter_map(|t| t.info.progress.map(|p| p.percentage())).collect();
        let average = if with_progress.is_empty() { 0.0 } else { with_progress.iter().sum::<f64>() / with_progress.len() as f64 };
        let progress = if working.is_empty() { -1 } else { average as i32 };
        // `MainWindowIntegration.SetTitle`: "<title> - 12.3%" while tasks work.
        let base = self.state.base_title.clone();
        let title = if progress >= 0 { format!("{base} - {average:.1}%") } else { base };
        if title != self.state.title {
            self.state.title = title.clone();
            if let Some(w) = &self.main {
                w.window.set_title(&title);
                w.window.request_redraw();
            }
        }
        if let Some(tray) = &mut self.tray {
            tray.set_tooltip(&title);
        }
        self.update_tray_icon(progress);
    }

    /// `TaskManager.UpdateTrayIcon`: the progress icon at `progress` %, or
    /// the app's icon at -1.
    fn update_tray_icon(&mut self, progress: i32) {
        let s = &self.settings.settings;
        if !s.TrayIconProgressEnabled || !s.ShowTray || self.tray_progress == progress {
            return;
        }
        if let Some(tray) = &mut self.tray {
            let size = tray::Tray::icon_size();
            let image = if progress >= 0 { tray::progress_icon(size, progress) } else { tray::app_icon(size) };
            tray.set_icon(&image);
        }
        self.tray_progress = progress;
    }
}

impl App {
    /// `StartApplicationAsync`, once the event loop runs: the hotkeys are
    /// registered (`UpdateHotkeysAsync`, after `ApplyCore` sets the repeat
    /// limit) and the main window opens, after the "Hotkey registration
    /// failed" message box if any failed (`ShowFailedHotkeys` holds startup
    /// until it's closed).
    fn start(&mut self, event_loop: &ActiveEventLoop) {
        self.system_theme = event_loop.system_theme();
        self.apply_appearance();
        // The Start screen comes first, on a fresh profile.
        if self.settings.settings.ShowStartScreen && !self.settings.paths.sandbox {
            eprintln!("Start screen opening.");
            match self.open_start_screen(event_loop) {
                Ok(()) => return,
                Err(e) => eprintln!("couldn't open the Start screen: {e}"),
            }
        }
        self.continue_start(event_loop);
    }

    /// The rest of `StartApplicationAsync`, after the Start screen.
    fn continue_start(&mut self, event_loop: &ActiveEventLoop) {
        // `new DesktopTrayIconService(…, ApplicationInfo.TitleShort, ShowTray)`.
        let proxy = self.proxy.clone();
        let callback: tray::Callback = Arc::new(move |e| {
            let _ = proxy.send_event(UserEvent::Tray(e));
        });
        match tray::Tray::new("ShareXrs", self.settings.settings.ShowTray, callback) {
            Ok(t) => self.tray = Some(t),
            Err(e) => eprintln!("couldn't add the tray icon: {e}"),
        }
        self.refresh_menus();
        // `ApplicationSettingsRuntime.InitializeAsync`: the update manager too.
        self.apply_update_settings();
        self.hotkeys.repeat_limit = Duration::from_millis(self.settings.settings.hotkey_repeat_limit() as u64);
        let disabled = self.settings.settings.DisableHotkeys;
        let failed = self.hotkeys.update_hotkeys(&mut self.settings.hotkeys.Hotkeys, disabled);
        eprintln!("HotkeyManager started.");
        self.refresh_hotkey_tips();
        self.init_task_view();
        // `ReloadCoreAsync`: `WatchFolderManager.UpdateWatchFolders()`.
        self.update_watch_folders();
        self.highlighter_activate_on_startup(event_loop);
        eprintln!("WatchFolderManager started.");
        if let Some(failed) = failed.filter(|_| !self.ignore_hotkey_warning) {
            let message = MessageBox::ok(failed.caption, failed.text, MessageBoxIcon::Warning);
            match self.open_dialog(event_loop, message, DialogPurpose::HotkeysFailed) {
                Ok(()) => return,
                Err(e) => eprintln!("couldn't open the message box: {e}"),
            }
        }
        // `showMainWindow`: not when starting silently to the tray.
        let s = &self.settings.settings;
        let show = !(self.silent || s.SilentRun) || !s.ShowTray || self.tray.is_none();
        if show && let Err(e) = self.open_main_window(event_loop) {
            eprintln!("couldn't open the main window: {e}");
            event_loop.exit();
            return;
        }
        self.startup_finished(event_loop);
    }

    /// `MainWindow.RefreshHotkeyTips`.
    fn refresh_hotkey_tips(&mut self) {
        self.state.hotkey_tips = main_window::hotkey_tips(&self.settings.hotkeys.Hotkeys);
        // The Hotkey settings window shows the statuses too.
        if let Some(h) = &self.hotkey_window {
            h.window.window.request_redraw();
        }
        if let Some(w) = &self.main {
            w.window.request_redraw();
        }
    }

    /// The theme the settings ask for (`ThemeManager.GetCurrentTheme`).
    fn theme(&self) -> egui::Theme {
        self.current_theme
    }

    /// `MessageBox.Show` with no owner: centred on the screen, topmost.
    fn open_dialog(&mut self, event_loop: &ActiveEventLoop, message: MessageBox, purpose: DialogPurpose) -> Result<(), String> {
        let theme = self.theme();
        let gpu = match &self.gpu {
            Some(g) => g,
            None => self.gpu.insert(Gpu::new()?),
        };
        let icon = Icon::from_rgba(app_icon::rgba64(), 64, 64).ok();
        let attrs = dialog_attributes()
            .with_title(&message.caption)
            .with_inner_size(LogicalSize::new(message_box::WIDTH, 240.0))
            .with_resizable(false)
            .with_enabled_buttons(WindowButtons::CLOSE)
            .with_window_level(WindowLevel::AlwaysOnTop)
            .with_theme(Some(match theme {
                egui::Theme::Dark => winit::window::Theme::Dark,
                egui::Theme::Light => winit::window::Theme::Light,
            }))
            .with_visible(false)
            .with_window_icon(icon);
        let window = event_loop.create_window(attrs).map_err(|e| e.to_string())?;
        let mut w = EguiWindow::new(window, gpu, None)?;
        set_theme(&w, theme);
        // `SizeToContent="Height"`: lay the text out once to know it.
        let height = w.paint(|ui| message.height(ui));
        let _ = w.window.request_inner_size(LogicalSize::new(message_box::WIDTH, height));
        center(&w.window, event_loop);
        w.window.set_visible(true);
        w.window.focus_window();
        w.window.request_redraw();
        self.dialog = Some(Dialog { window: w, message, purpose });
        Ok(())
    }

    fn close_dialog(&mut self, event_loop: &ActiveEventLoop, result: DialogResult) {
        let Some(dialog) = self.dialog.take() else { return };
        // Startup waits for the hotkey warning, then opens the main window;
        // other message boxes leave it as it is (in the tray).
        let startup = matches!(dialog.purpose, DialogPurpose::HotkeysFailed);
        if let DialogPurpose::DeleteFiles(ids) = dialog.purpose {
            self.file_delete_answered(ids, result, dialog.message.checked);
        } else if let DialogPurpose::DownloadFFmpeg(path) = &dialog.purpose {
            if result == DialogResult::Yes {
                self.download_ffmpeg(path.clone());
            }
        } else if matches!(dialog.purpose, DialogPurpose::UpdateSignature) {
            if let Some(d) = self.dialogs.iter_mut().find(|d| d.dialog.single() == Some("update-downloader")) {
                d.dialog.answered(result == DialogResult::Yes);
                d.window.window.request_redraw();
            }
        } else if let DialogPurpose::CommandLine(question) = dialog.purpose {
            self.cli_answered(question, result);
        } else if matches!(dialog.purpose, DialogPurpose::ClearCustomUploaders)
            && result == DialogResult::Yes
            && let Some(d) = self.dialogs.iter_mut().find(|d| d.dialog.single() == Some("custom-uploader-settings"))
        {
            d.dialog.confirmed();
            d.window.window.request_redraw();
        }
        if startup {
            if self.main.is_none()
                && let Err(e) = self.open_main_window(event_loop)
            {
                eprintln!("couldn't open the main window: {e}");
                event_loop.exit();
                return;
            }
            self.startup_finished(event_loop);
        }
        if self.dialog.is_none() && !self.queued_dialogs.is_empty() {
            let (message, purpose) = self.queued_dialogs.remove(0);
            if let Err(e) = self.open_dialog(event_loop, message, purpose) {
                eprintln!("couldn't open the message box: {e}");
            }
        }
    }

    /// A message box after the open one, if there's one.
    fn queue_dialog(&mut self, event_loop: &ActiveEventLoop, message: MessageBox, purpose: DialogPurpose) {
        if self.dialog.is_some() {
            self.queued_dialogs.push((message, purpose));
        } else if let Err(e) = self.open_dialog(event_loop, message, purpose) {
            eprintln!("couldn't open the message box: {e}");
        }
    }

    fn dialog_event(&mut self, event_loop: &ActiveEventLoop, event: WindowEvent) {
        let Some(d) = self.dialog.as_mut() else { return };
        match event {
            // Closing the window answers `GetCancelResult`.
            WindowEvent::CloseRequested => {
                let result = d.message.cancel_result();
                self.close_dialog(event_loop, result);
            }
            WindowEvent::RedrawRequested => {
                let message = &mut d.message;
                if let Some(result) = d.window.paint(|ui| message.ui(ui)) {
                    self.close_dialog(event_loop, result);
                }
            }
            other => d.window.on_event(&other),
        }
    }

    /// `MainWindow.RestoreWindowBounds`: the remembered size and position,
    /// when the settings ask for them.
    fn restore_window_bounds(&self, window: &winit::window::Window, min_height: f32) {
        let s = &self.settings.settings;
        if s.RememberMainFormSize && s.MainFormSize != Size::default() {
            let _ = window.request_inner_size(LogicalSize::new(
                (s.MainFormSize.width as f32).max(main_window::MIN_WIDTH),
                (s.MainFormSize.height as f32).max(min_height),
            ));
        }
        let p = s.MainFormPosition;
        if s.RememberMainFormPosition && p != Point::default() {
            let size = window.outer_size();
            let on_screen = window.available_monitors().any(|m| {
                let (mp, ms) = (m.position(), m.size());
                p.x < mp.x + ms.width as i32
                    && p.x + size.width as i32 > mp.x
                    && p.y < mp.y + ms.height as i32
                    && p.y + size.height as i32 > mp.y
            });
            if on_screen {
                window.set_outer_position(winit::dpi::PhysicalPosition::new(p.x, p.y));
            }
        }
    }

    /// `MainWindow.SaveWindowBounds`: where the window is, while it's
    /// neither maximized nor minimized (saved with the settings).
    fn save_window_bounds(&mut self) {
        let Some(w) = &self.main else { return };
        if w.window.is_maximized() || w.window.is_minimized() == Some(true) {
            return;
        }
        let size = w.window.inner_size().to_logical::<f64>(w.window.scale_factor());
        if size.width < 1.0 || size.height < 1.0 {
            return;
        }
        if let Ok(pos) = w.window.outer_position() {
            self.settings.settings.MainFormPosition = Point::new(pos.x, pos.y);
        }
        self.settings.settings.MainFormSize = Size::new(size.width.round() as i32, size.height.round() as i32);
    }

    /// `TaskHelpers.ExecuteJob`: what a hotkey runs, with its task's
    /// settings filled in from the defaults (`GetSafeTaskSettings`).
    /// Captures and tools hook in here as they're ported; until then the
    /// job is logged.
    fn run_job(&mut self, event_loop: &ActiveEventLoop, job: HotkeyType, task: TaskSettings, origin: Option<usize>) {
        if job == HotkeyType::None {
            return;
        }
        eprintln!("job: {} ({})", job.name(), task.display_name());
        if let Some(capture) = CaptureJob::from_hotkey(&job) {
            self.capture(capture, task, false);
            return;
        }
        if self.run_recording_job(event_loop, &job, &task, origin) {
            return;
        }
        if self.run_upload_job(event_loop, &job, &task) {
            return;
        }
        if self.run_screen_job(event_loop, &job, &task, None) {
            return;
        }
        if let Some(key) = image_tools::open::tool_for_hotkey(&job) {
            self.open_image_tool(event_loop, key, Some(task));
            return;
        }
        match job {
            HotkeyType::DisableHotkeys => self.toggle_hotkeys(),
            HotkeyType::OpenMainWindow => self.show_main_window(event_loop),
            HotkeyType::ToggleTrayMenu => self.toggle_tray_menu(event_loop),
            HotkeyType::ExitShareX => self.exit(event_loop),
            HotkeyType::OCR => self.ocr_capture(task, false),
            // `OpenScrollingCapture`, `OpenAutoCapture`, `StartAutoCapture`, `StopAutoCapture`.
            HotkeyType::ScrollingCapture => self.open_scrolling_capture(event_loop, task),
            HotkeyType::AutoCapture => self.open_auto_capture(event_loop, task),
            HotkeyType::StartAutoCapture => self.start_auto_capture(event_loop, task),
            HotkeyType::StopAutoCapture => self.stop_auto_capture(event_loop),
            HotkeyType::OpenHistory => self.show_history(event_loop),
            HotkeyType::OpenImageHistory => self.show_image_history(event_loop),
            _ => eprintln!("{}: not ported yet", job.name()),
        }
    }

    /// `TaskHelpers.ExecuteJob(HotkeyType)`: a job with the default task
    /// settings, as the tray icon's click actions run them.
    fn run_default_job(&mut self, event_loop: &ActiveEventLoop, job: HotkeyType) {
        let defaults = &self.settings.settings.DefaultTaskSettings;
        let task = defaults.safe_task_settings(defaults);
        self.run_job(event_loop, job, task, None);
    }

    /// `DesktopTrayIconService.OnMouseUp` / `OnMouseDown` and the main
    /// window's `OnTrayIconRightButtonDown` / `Up`.
    fn tray_event(&mut self, event_loop: &ActiveEventLoop, event: TrayEvent) {
        use tray::Button;
        match event {
            TrayEvent::Down(Button::Right) => self.tray_menu = None,
            TrayEvent::Down(_) => {}
            TrayEvent::Up(Button::Right) => self.toggle_tray_menu(event_loop),
            TrayEvent::Up(Button::Middle) => {
                let job = self.settings.settings.TrayMiddleClickAction.clone();
                self.run_default_job(event_loop, job);
            }
            TrayEvent::Up(Button::Left) => {
                let s = &self.settings.settings;
                if s.TrayLeftDoubleClickAction == HotkeyType::None {
                    let job = s.TrayLeftClickAction.clone();
                    self.run_default_job(event_loop, job);
                } else {
                    self.tray_left_clicks += 1;
                    if self.tray_left_clicks == 1 {
                        self.tray_click_at = Some(Instant::now() + tray_double_click_time());
                    } else {
                        self.tray_left_clicks = 0;
                        self.tray_click_at = None;
                        let job = s.TrayLeftDoubleClickAction.clone();
                        self.run_default_job(event_loop, job);
                    }
                }
            }
            TrayEvent::Command(i) => {
                if let Some(command) = self.tray_entries.commands.get(i).cloned() {
                    self.menu_command(event_loop, command, true);
                }
            }
        }
    }

    /// `OnSingleClickTimerTick`: one click and no second within the
    /// double click time.
    fn tray_click_timer(&mut self, event_loop: &ActiveEventLoop) {
        if self.tray_click_at.is_some_and(|at| at <= Instant::now()) {
            self.tray_click_at = None;
            if self.tray_left_clicks == 1 {
                self.tray_left_clicks = 0;
                let job = self.settings.settings.TrayLeftClickAction.clone();
                self.run_default_job(event_loop, job);
            }
        }
    }

    /// `MainWindow.ShowTrayMenu`: closes the menu when it's open, else opens
    /// it at the pointer.
    fn toggle_tray_menu(&mut self, event_loop: &ActiveEventLoop) {
        if self.tray_menu.take().is_some() {
            return;
        }
        if cfg!(not(windows)) {
            // The native menu opens from the icon itself.
            return;
        }
        self.refresh_menus();
        self.state.menu_state.uploads_enabled = !self.state.uploads_disabled;
        let entries = main_menu::tray_menu(&self.state.menu_state);
        let screens = sharex_capture::screenshot::screens();
        let cursor = sharex_capture::screenshot::cursor_position().unwrap_or_default();
        let open: &[usize] = if self.settings.settings.TrayAutoExpandCaptureMenu { &[0] } else { &[] };
        let theme = self.theme();
        let menu = match &self.gpu {
            Some(g) => Ok(g),
            None => Gpu::new().map(|g| &*self.gpu.insert(g)),
        }
        .and_then(|gpu| TrayMenu::open(event_loop, gpu, &screens, cursor, entries, theme, open));
        match menu {
            Ok(m) => self.tray_menu = Some(m),
            Err(e) => eprintln!("couldn't open the tray menu: {e}"),
        }
    }

    /// `OnTrayMenuClosed`: the settings are saved.
    fn close_tray_menu(&mut self) {
        self.tray_menu = None;
        if let Err(e) = self.settings.save_all() {
            eprintln!("couldn't save the settings: {e}");
        }
        self.save_uploaders_config(true);
    }

    /// `TaskHelpers.ToggleHotkeys()`: turns every hotkey but "Disable
    /// hotkeys" off, or all back on (`ApplicationConfig.DisableHotkeys`).
    fn toggle_hotkeys(&mut self) {
        let disabled = !self.settings.settings.DisableHotkeys;
        self.settings.settings.DisableHotkeys = disabled;
        self.hotkeys.toggle_hotkeys(&mut self.settings.hotkeys.Hotkeys, disabled);
        self.refresh_hotkey_tips();
        self.refresh_menus();
        // ShareX shows this as a notification, which isn't ported yet.
        eprintln!(
            "{}",
            if disabled {
                sharex_strings::sharex::TaskHelpers_ToggleHotkeys_Hotkeys_disabled_()
            } else {
                sharex_strings::sharex::TaskHelpers_ToggleHotkeys_Hotkeys_enabled_()
            }
        );
    }
}

/// The double click time the tray's left click waits for a second click.
fn tray_double_click_time() -> Duration {
    #[cfg(windows)]
    return tray::double_click_time();
    // Avalonia's `GetDoubleTapTime(Mouse)` default.
    #[cfg(not(windows))]
    Duration::from_millis(500)
}

/// A ShareX window title with ShareXrs's name: "ShareX - Text upload"
/// becomes "ShareXrs - Text upload".
fn rs_title(title: &str) -> String {
    match title.strip_prefix("ShareX") {
        Some(rest) if rest.is_empty() || rest.starts_with(' ') => format!("ShareXrs{rest}"),
        _ => title.to_owned(),
    }
}

fn set_theme(w: &EguiWindow, theme: egui::Theme) {
    w.ctx().set_theme(theme);
    w.window.set_theme(Some(match theme {
        egui::Theme::Dark => winit::window::Theme::Dark,
        egui::Theme::Light => winit::window::Theme::Light,
    }));
}

/// A message box keeps the system's frame (Avalonia's default chrome) and
/// stays out of the taskbar (`ShowInTaskbar="False"`).
fn dialog_attributes() -> WindowAttributes {
    let attrs = WindowAttributes::default();
    #[cfg(windows)]
    {
        use winit::platform::windows::WindowAttributesExtWindows;
        attrs.with_skip_taskbar(true)
    }
    #[cfg(not(windows))]
    attrs
}

/// ShareX draws its own title bar (`ExtendClientAreaToDecorationsHint`).
/// On macOS the content extends under a transparent title bar instead, so
/// the traffic lights stay.
fn main_window_attributes() -> WindowAttributes {
    let attrs = WindowAttributes::default();
    #[cfg(target_os = "macos")]
    {
        use winit::platform::macos::WindowAttributesExtMacOS;
        attrs
            .with_titlebar_transparent(true)
            .with_fullsize_content_view(true)
            .with_title_hidden(true)
    }
    #[cfg(windows)]
    {
        use winit::platform::windows::WindowAttributesExtWindows;
        attrs.with_decorations(false).with_undecorated_shadow(true)
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    attrs.with_decorations(false)
}

/// `WindowStartupLocation.CenterScreen`: centred in the working area of the
/// window's screen (Avalonia's `Screen.WorkingArea`).
fn center(window: &winit::window::Window, event_loop: &ActiveEventLoop) {
    let Some(monitor) = window.current_monitor().or_else(|| event_loop.primary_monitor()) else { return };
    let (m, size) = (monitor.position(), monitor.size());
    let (mut ax, mut ay, mut aw, mut ah) = (m.x, m.y, size.width as i32, size.height as i32);
    if let Some(s) = sharex_capture::screenshot::screens().into_iter().find(|s| s.bounds.contains(m.x, m.y)) {
        (ax, ay, aw, ah) = (s.work_area.x, s.work_area.y, s.work_area.w, s.work_area.h);
    }
    let outer = window.outer_size();
    window.set_outer_position(winit::dpi::PhysicalPosition::new(
        ax + (aw - outer.width as i32) / 2,
        ay + (ah - outer.height as i32) / 2,
    ));
}

impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if !self.started {
            self.started = true;
            self.start(event_loop);
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::Hotkey(id) => {
                let s = &self.settings.settings;
                let (fullscreen, defaults) = (s.DisableHotkeysOnFullscreen, &s.DefaultTaskSettings);
                let Some(i) = self.hotkeys.on_hotkey_pressed(id, &self.settings.hotkeys.Hotkeys, fullscreen) else {
                    return;
                };
                let hotkey = &self.settings.hotkeys.Hotkeys[i];
                eprintln!("Hotkey triggered. {hotkey}");
                let task = hotkey.TaskSettings.safe_task_settings(defaults);
                self.run_job(event_loop, task.Job.clone(), task, Some(i));
            }
            UserEvent::Tray(e) => self.tray_event(event_loop, e),
            UserEvent::Task(event) => {
                self.task_event(event);
                self.update_progress_ui();
                for d in std::mem::take(&mut self.pending_dialogs) {
                    self.open_dialog_window(event_loop, d);
                }
            }
            UserEvent::Ui(request) => self.ui_request(event_loop, request),
            UserEvent::Recording(event) => self.recording_event(event_loop, event),
            UserEvent::Update(event) => self.update_event(event_loop, event),
            UserEvent::WatchFolder(id, path) => self.watch_folder_trigger(event_loop, id, &path),
            UserEvent::Arguments(a) => self.arguments_received(event_loop, a),
            UserEvent::ExtensionImage(result) => self.extension_image(*result),
            UserEvent::Wake => {}
            UserEvent::AutoCaptureTray(e) => self.auto_capture_tray_event(e),
        }
    }

    /// `ApplicationLifecycle`: the hotkey host closes (unregistering every
    /// hotkey), then `CloseSequence` saves all settings.
    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        // `DisposeWindowResources`: the tray icon goes.
        self.tray_menu = None;
        self.watch_folders.clear();
        self.recording = None;
        self.ffmpeg_options = None;
        // `MouseHighlighterManager.Shutdown`.
        self.screen_tools.highlighter.shutdown();
        if let Some(mut s) = self.settings_window.take() {
            s.state.flush_personal_path();
        }
        self.hotkey_window = None;
        self.task_window = None;
        self.dialogs.clear();
        self.tray = None;
        self.save_window_bounds();
        self.hotkeys.unregister_all(&mut self.settings.hotkeys.Hotkeys, false);
        eprintln!("ShareX closing.");
        history::close();
        if let Err(e) = self.settings.save_all() {
            eprintln!("couldn't save the settings: {e}");
        }
        self.save_uploaders_config(true);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        // The OS switched light/dark mode: `PlatformSettings.ColorValuesChanged`.
        if let WindowEvent::ThemeChanged(t) = event
            && self.system_theme != Some(t)
        {
            self.system_theme = Some(t);
            let o = self.settings.settings.ThemeOptions.clone().unwrap_or_default();
            if o.UseSystemTheme || o.UseSystemAccentColor {
                self.apply_appearance();
            }
        }
        if self.app_window_event(event_loop, id, &event) {
            return;
        }
        if self.dialog_window_event(event_loop, id, &event) {
            return;
        }
        if self.recording_window_event(event_loop, id, &event) {
            return;
        }
        if self.ffmpeg_options_window_event(event_loop, id, &event) {
            return;
        }
        if self.screen_tools_window_event(event_loop, id, &event) {
            return;
        }
        if self.scrolling_window_event(event_loop, id, &event) {
            return;
        }
        if self.auto_capture_window_event(event_loop, id, &event) {
            return;
        }
        if let Some(region) = self.region.as_mut().filter(|r| r.session.owns(id)) {
            let outcome = region.session.window_event(event_loop, id, event);
            region.session.handle_editor_requests();
            let pick = region.session.take_pick_request();
            if !matches!(outcome, Outcome::Continue) {
                self.finish_region_capture(event_loop, outcome);
            } else if let Some(item) = pick {
                // `ScreenColorPickerWindow.PickAsync(owner)` over the overlay.
                self.open_screen_color_picker(event_loop, crate::screen_tools::PickPurpose::Region(item));
            }
            return;
        }
        if let Some(t) = self.transparent.as_mut().filter(|t| t.region.owns(id)) {
            if let transparent_region::Outcome::Done(selection) = t.region.window_event(id, event) {
                self.finish_transparent_capture(event_loop, selection);
            }
            return;
        }
        if let Some(menu) = self.tray_menu.as_mut().filter(|m| m.owns(id)) {
            let out = menu.window_event(event);
            if out.closed {
                self.close_tray_menu();
            }
            // Entries that stay open (`StaysOpenOnClick`) run with the menu
            // still up.
            if let Some(command) = out.command {
                self.menu_command(event_loop, command, true);
            }
            return;
        }
        if self.dialog.as_ref().is_some_and(|d| d.window.window.id() == id) {
            self.dialog_event(event_loop, event);
            return;
        }
        if matches!(event, WindowEvent::Moved(_) | WindowEvent::Resized(_)) {
            self.save_window_bounds();
        }
        if matches!(event, WindowEvent::CloseRequested) && self.main.as_ref().is_some_and(|w| w.window.id() == id) {
            self.close_main_window(event_loop);
            return;
        }
        let Some(w) = self.main.as_mut().filter(|w| w.window.id() == id) else { return };
        match event {
            WindowEvent::HoveredFile(_) | WindowEvent::HoveredFileCancelled | WindowEvent::DroppedFile(_) => {
                // Uploading what's dropped comes with the upload tools.
                self.state.drop_overlay = matches!(event, WindowEvent::HoveredFile(_));
                w.window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                self.state.maximized = w.window.is_maximized();
                let state = &mut self.state;
                let actions = w.paint(|ui| state.ui(ui));
                for a in actions {
                    self.act(event_loop, a);
                }
                self.decode_task_images();
            }
            WindowEvent::CursorMoved { position, .. } => {
                let scale = w.window.scale_factor();
                self.cursor = Some(egui::pos2((position.x / scale) as f32, (position.y / scale) as f32));
                w.on_event(&event);
            }
            WindowEvent::CursorLeft { .. } => {
                self.cursor = None;
                w.on_event(&event);
            }
            WindowEvent::ModifiersChanged(m) => {
                self.modifiers = m.state();
                w.on_event(&event);
            }
            WindowEvent::KeyboardInput { event: ref key, .. } => {
                if key.state == ElementState::Pressed
                    && let Some(k) = task_actions::clipboard_key(&key.logical_key, self.modifiers)
                {
                    self.state.tasks.clipboard_keys.push(k);
                }
                w.on_event(&event);
            }
            // Light dismiss: a press outside the open menu closes it, and
            // Avalonia doesn't pass that press on.
            WindowEvent::MouseInput { state: ElementState::Pressed, .. } if self.state.menu_at(self.cursor) == Some(false) => {
                self.state.close_menus();
                self.swallow_release = true;
                w.window.request_redraw();
            }
            WindowEvent::MouseInput { state: ElementState::Released, .. } if self.swallow_release => {
                self.swallow_release = false;
            }
            // Popups close with their window's activation.
            WindowEvent::Focused(false) => {
                if self.state.close_menus() {
                    w.window.request_redraw();
                }
                w.on_event(&event);
            }
            other => w.on_event(&other),
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // `-AutoClose`: `ApplicationLifecycle.Exit()` once the last task is done.
        if self.exit_requested {
            self.exit_requested = false;
            self.exit(event_loop);
            return;
        }
        if let Some(d) = &self.dialog
            && d.window.repaint_at.is_some_and(|at| at <= Instant::now())
        {
            d.window.window.request_redraw();
        }
        self.advance_pending_capture(event_loop);
        self.tray_click_timer(event_loop);
        // A due repaint is asked for once: a hidden window gets no redraw,
        // and waiting on it would stall every timer below.
        let now = Instant::now();
        if let Some(w) = self.main.as_mut().filter(|w| w.repaint_at.is_some_and(|at| at <= now)) {
            w.window.request_redraw();
            w.repaint_at = None;
        }
        if let Some(m) = &mut self.tray_menu {
            m.redraw_if_due(now);
        }
        let recording = self.recording_wake_at();
        let ffmpeg_options = self.ffmpeg_options_wake_at(now);
        let app_windows = self.app_windows_due(now);
        let dialogs = self.dialogs_due(now);
        let region = self.region.as_mut().and_then(|r| r.session.wake_at());
        let pending = self.pending_capture.as_ref().map(|p| p.at);
        let screen_tools = self.screen_tools_wake_at(now);
        let scrolling = self.scrolling_tick(event_loop);
        let auto_capture = self.auto_capture_tick(event_loop);
        let next = [
            screen_tools,
            scrolling,
            auto_capture,
            self.main.as_ref().and_then(|w| w.repaint_at),
            self.tray_menu.as_ref().and_then(|m| m.repaint_at()),
            region,
            pending,
            self.transparent_grab.as_ref().map(|g| g.0),
            self.tray_click_at,
            app_windows,
            dialogs,
            recording,
            ffmpeg_options,
        ]
        .into_iter()
        .flatten()
        .min();
        match next {
            Some(at) => event_loop.set_control_flow(ControlFlow::WaitUntil(at)),
            None => event_loop.set_control_flow(ControlFlow::Wait),
        }
    }
}
