//! ShareXrs: ShareX, ported to Rust and egui.
//!
//! `sharexrs` runs the app. `sharexrs --render <screen> --out <png>` draws
//! one screen offscreen instead, at the size ShareX's would be, for
//! comparing against a screenshot of ShareX (`scripts/compare.ps1`).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app_icon;
mod capture_job;
mod hotkeys;
mod main_menu;
mod main_window;
mod message_box;
mod region_render;
mod region_session;
mod render;
mod shell;
mod sound;
mod task_actions;
mod task_view;
mod tasks;
mod test_pattern;
mod thumbnail;
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

use hotkeys::HotkeyManager;
use main_menu::MenuCommand;
use main_window::{Action, MainWindow};
use region_session::RegionSession;
use sharex_capture::region::Outcome;
use message_box::{DialogResult, MessageBox, MessageBoxIcon};
use window::EguiWindow;

/// What other threads tell the event loop.
#[derive(Debug)]
pub enum UserEvent {
    /// A global hotkey was pressed (its `HotkeyInfo.ID`).
    Hotkey(u16),
    /// A task's thread or a thumbnail decoder finished.
    Task(tasks::TaskEvent),
}

fn main() {
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
    // `Program.Run`: where the profile is, then the settings
    // (`SettingManager.LoadInitialSettings`), before any window.
    let system = SystemOptions::read();
    let paths = AppPaths::initialize(&args, &system);
    if let Some(method) = &paths.detection_method {
        eprintln!("Personal path detection method: {method}");
    }
    let settings = SettingManager::load(paths, system);
    settings.paths.create_application_folders(Some(&settings.settings));

    #[cfg(target_os = "macos")]
    sharex_capture::platform::macos::ask_for_screen_recording();
    let event_loop = EventLoop::<UserEvent>::with_user_event().build().expect("event loop");
    let proxy = event_loop.create_proxy();
    let task_proxy = proxy.clone();
    let host = hotkeys::platform_host(Arc::new(move |id| {
        let _ = proxy.send_event(UserEvent::Hotkey(id));
    }));
    let mut app = App {
        gpu: None,
        main: None,
        state: MainWindow::new(),
        cursor: None,
        swallow_release: false,
        settings,
        hotkeys: HotkeyManager::new(host),
        dialog: None,
        started: false,
        // `-NoHotkeys`: `StartupOptions.IgnoreHotkeyWarning`.
        ignore_hotkey_warning: is_command_present(&args, &["NoHotkeys"]),
        region: None,
        pending_capture: None,
        last_regions: Vec::new(),
        proxy: task_proxy,
        modifiers: Default::default(),
    };
    event_loop.run_app(&mut app).expect("run");
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
    /// `RegionCaptureIntegration.LastRegionRectangles`.
    last_regions: Vec<sharex_capture::Rect>,
    /// For tasks' threads to report back to the event loop.
    proxy: winit::event_loop::EventLoopProxy<UserEvent>,
    /// The keyboard's modifiers in the main window.
    modifiers: winit::keyboard::ModifiersState,
}

/// `CaptureBase.CaptureAsync` between the request and the capture: the
/// screenshot delay, then (from the main window's menu) hiding it for 250 ms.
struct PendingCapture {
    at: Instant,
    task: TaskSettings,
    auto_hide: bool,
    /// The main window was hidden for it (`showMainForm`).
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
            Action::Close => event_loop.exit(),
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
                set_theme(w, next);
                let options = self.settings.settings.theme_options();
                options.Theme = if next == egui::Theme::Dark { "Dark" } else { "Light" }.into();
                options.UseSystemTheme = false;
                let _ = self.settings.save_application_config(false);
            }
            Action::Nav(i, _) => eprintln!("sidebar entry {i}: not ported yet"),
            Action::Menu(MenuCommand::CaptureRegion) => {
                // `new CaptureRegion().Capture(autoHide)` with the default
                // task settings.
                let task = TaskSettings::default_task_settings(&self.settings.settings.DefaultTaskSettings);
                self.capture_region(task, true);
                return;
            }
            Action::Menu(command) => eprintln!("menu: {command:?}: not ported yet"),
            Action::Task(_) => {}
        }
        w.window.request_redraw();
    }
}

impl App {
    /// `CaptureBase.Capture`: waits for the screenshot delay, hides the
    /// main window when asked (`autoHideForm`, from the main window's
    /// menus), then captures.
    fn capture_region(&mut self, task: TaskSettings, auto_hide: bool) {
        if self.region.is_some() || self.pending_capture.is_some() {
            return;
        }
        let delay = task.CaptureSettings.as_ref().map_or(0.0, |c| c.ScreenshotDelay).max(0.0);
        let at = Instant::now() + Duration::from_secs_f64(delay);
        self.pending_capture = Some(PendingCapture { at, task, auto_hide, hidden: false });
    }

    /// Moves a pending capture on when it's due.
    fn advance_pending_capture(&mut self, event_loop: &ActiveEventLoop) {
        let Some(pending) = self.pending_capture.as_mut().filter(|p| p.at <= Instant::now()) else { return };
        if pending.auto_hide
            && !pending.hidden
            && let Some(w) = self.main.as_ref().filter(|w| w.window.is_visible() != Some(false))
        {
            w.window.set_visible(false);
            pending.hidden = true;
            pending.at = Instant::now() + Duration::from_millis(250);
            return;
        }
        let Some(pending) = self.pending_capture.take() else { return };
        self.start_region_capture(event_loop, pending.task, pending.hidden);
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
                if hidden {
                    self.show_main_window();
                }
            }
        }
    }

    /// `MainWindowIntegration.Activate`.
    fn show_main_window(&self) {
        if let Some(w) = &self.main {
            w.window.set_visible(true);
            w.window.focus_window();
            w.window.request_redraw();
        }
    }

    /// The overlay closed: the main window comes back if it was hidden, and
    /// a capture goes through the after capture tasks
    /// (`CaptureBase.AfterCapture`, `UploadManager.RunImageTask`).
    fn finish_region_capture(&mut self, outcome: Outcome) {
        let Some(Region { session, task, hidden }) = self.region.take() else { return };
        // The overlay changes its options as ShareX's does (magnifier size,
        // toolbar place), on the settings the task takes them from.
        let options = session.capture.into_options();
        if task.UseDefaultCaptureSettings
            && let Some(c) = self.settings.settings.DefaultTaskSettings.CaptureSettings.as_mut()
        {
            c.RegionCaptureOptions = options;
        }
        if hidden {
            self.show_main_window();
        }
        let Outcome::Captured(result) = outcome else { return };
        self.last_regions = result.regions.clone();
        if task.GeneralSettings.as_ref().is_none_or(|g| g.PlaySoundAfterCapture) {
            sound::play_capture();
        }
        let metadata = capture_job::Metadata::from_result(&result);
        let parent = self.settings.paths.screenshots_parent_folder(Some(&self.settings.settings));
        let job = capture_job::ImageTask::new(metadata, task, &mut self.settings.settings, &parent);
        self.run_image_task(job);
    }
}

impl App {
    /// `StartApplicationAsync`, once the event loop runs: the hotkeys are
    /// registered (`UpdateHotkeysAsync`, after `ApplyCore` sets the repeat
    /// limit) and the main window opens, after the "Hotkey registration
    /// failed" message box if any failed (`ShowFailedHotkeys` holds startup
    /// until it's closed).
    fn start(&mut self, event_loop: &ActiveEventLoop) {
        self.hotkeys.repeat_limit = Duration::from_millis(self.settings.settings.hotkey_repeat_limit() as u64);
        let disabled = self.settings.settings.DisableHotkeys;
        let failed = self.hotkeys.update_hotkeys(&mut self.settings.hotkeys.Hotkeys, disabled);
        eprintln!("HotkeyManager started.");
        self.refresh_hotkey_tips();
        self.init_task_view();
        if let Some(failed) = failed.filter(|_| !self.ignore_hotkey_warning) {
            let message = MessageBox::ok(failed.caption, failed.text, MessageBoxIcon::Warning);
            match self.open_dialog(event_loop, message, DialogPurpose::HotkeysFailed) {
                Ok(()) => return,
                Err(e) => eprintln!("couldn't open the message box: {e}"),
            }
        }
        if let Err(e) = self.open_main_window(event_loop) {
            eprintln!("couldn't open the main window: {e}");
            event_loop.exit();
        }
    }

    /// `MainWindow.RefreshHotkeyTips`.
    fn refresh_hotkey_tips(&mut self) {
        self.state.hotkey_tips = main_window::hotkey_tips(&self.settings.hotkeys.Hotkeys);
        if let Some(w) = &self.main {
            w.window.request_redraw();
        }
    }

    /// The theme the settings ask for (`ThemeManager.GetCurrentTheme`).
    fn theme(&self) -> egui::Theme {
        let options = self.settings.settings.ThemeOptions.clone().unwrap_or_default();
        let name = if options.UseSystemTheme {
            match self.main.as_ref().and_then(|w| w.window.theme()) {
                Some(winit::window::Theme::Light) => "Light",
                _ => "Dark",
            }
        } else {
            options.Theme.as_str()
        };
        if name.eq_ignore_ascii_case("Light") { egui::Theme::Light } else { egui::Theme::Dark }
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
        if let DialogPurpose::DeleteFiles(ids) = dialog.purpose {
            self.file_delete_answered(ids, result, dialog.message.checked);
        }
        if self.main.is_none()
            && let Err(e) = self.open_main_window(event_loop)
        {
            eprintln!("couldn't open the main window: {e}");
            event_loop.exit();
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
    fn run_job(&mut self, event_loop: &ActiveEventLoop, job: HotkeyType, task: TaskSettings) {
        eprintln!("job: {} ({})", job.name(), task.display_name());
        match job {
            HotkeyType::RectangleRegion => self.capture_region(task, false),
            HotkeyType::DisableHotkeys => self.toggle_hotkeys(),
            HotkeyType::OpenMainWindow => {
                if let Some(w) = &self.main {
                    w.window.set_minimized(false);
                    w.window.focus_window();
                }
            }
            HotkeyType::ExitShareX => event_loop.exit(),
            _ => {}
        }
    }

    /// `TaskHelpers.ToggleHotkeys()`: turns every hotkey but "Disable
    /// hotkeys" off, or all back on (`ApplicationConfig.DisableHotkeys`).
    fn toggle_hotkeys(&mut self) {
        let disabled = !self.settings.settings.DisableHotkeys;
        self.settings.settings.DisableHotkeys = disabled;
        self.hotkeys.toggle_hotkeys(&mut self.settings.hotkeys.Hotkeys, disabled);
        self.refresh_hotkey_tips();
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

fn center(window: &winit::window::Window, event_loop: &ActiveEventLoop) {
    let Some(monitor) = window.current_monitor().or_else(|| event_loop.primary_monitor()) else { return };
    let (m, size) = (monitor.position(), monitor.size());
    let outer = window.outer_size();
    window.set_outer_position(winit::dpi::PhysicalPosition::new(
        m.x + (size.width as i32 - outer.width as i32) / 2,
        m.y + (size.height as i32 - outer.height as i32) / 2,
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
                self.run_job(event_loop, task.Job.clone(), task);
            }
            UserEvent::Task(event) => self.task_event(event),
        }
    }

    /// `ApplicationLifecycle`: the hotkey host closes (unregistering every
    /// hotkey), then `CloseSequence` saves all settings.
    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        self.save_window_bounds();
        self.hotkeys.unregister_all(&mut self.settings.hotkeys.Hotkeys, false);
        eprintln!("ShareX closing.");
        if let Err(e) = self.settings.save_all() {
            eprintln!("couldn't save the settings: {e}");
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if let Some(region) = self.region.as_mut().filter(|r| r.session.owns(id)) {
            let outcome = region.session.window_event(event_loop, id, event);
            if !matches!(outcome, Outcome::Continue) {
                self.finish_region_capture(outcome);
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
        let Some(w) = self.main.as_mut().filter(|w| w.window.id() == id) else { return };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
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
        if let Some(d) = &self.dialog
            && d.window.repaint_at.is_some_and(|at| at <= Instant::now())
        {
            d.window.window.request_redraw();
        }
        self.advance_pending_capture(event_loop);
        let region = self.region.as_mut().and_then(|r| r.session.wake_at());
        let pending = self.pending_capture.as_ref().map(|p| p.at);
        let next = [self.main.as_ref().and_then(|w| w.repaint_at), region, pending].into_iter().flatten().min();
        match next {
            Some(at) if at <= Instant::now() => {
                if let Some(w) = &self.main {
                    w.window.request_redraw();
                }
                event_loop.set_control_flow(ControlFlow::Wait);
            }
            Some(at) => event_loop.set_control_flow(ControlFlow::WaitUntil(at)),
            None => event_loop.set_control_flow(ControlFlow::Wait),
        }
    }
}
