//! ShareXrs: ShareX, ported to Rust and egui.
//!
//! `sharexrs` runs the app. `sharexrs --render <screen> --out <png>` draws
//! one screen offscreen instead, at the size ShareX's would be, for
//! comparing against a screenshot of ShareX (`scripts/compare.ps1`).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app_icon;
mod hotkeys;
mod main_menu;
mod main_window;
mod message_box;
mod render;
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
use main_window::{Action, MainWindow};
use message_box::{MessageBox, MessageBoxIcon};
use window::EguiWindow;

/// What other threads tell the event loop.
#[derive(Debug)]
pub enum UserEvent {
    /// A global hotkey was pressed (its `HotkeyInfo.ID`).
    Hotkey(u16),
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
    // `Program.Run`: where the profile is, then the settings
    // (`SettingManager.LoadInitialSettings`), before any window.
    let system = SystemOptions::read();
    let paths = AppPaths::initialize(&args, &system);
    if let Some(method) = &paths.detection_method {
        eprintln!("Personal path detection method: {method}");
    }
    let settings = SettingManager::load(paths, system);
    settings.paths.create_application_folders(Some(&settings.settings));

    let event_loop = EventLoop::<UserEvent>::with_user_event().build().expect("event loop");
    let proxy = event_loop.create_proxy();
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
    };
    event_loop.run_app(&mut app).expect("run");
}

/// A message box in its own window (`MessageBoxWindow`).
struct Dialog {
    window: EguiWindow,
    message: MessageBox,
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
            Action::Menu(command) => eprintln!("menu: {command:?}: not ported yet"),
        }
        w.window.request_redraw();
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
        if let Some(failed) = failed.filter(|_| !self.ignore_hotkey_warning) {
            let message = MessageBox::ok(failed.caption, failed.text, MessageBoxIcon::Warning);
            match self.open_dialog(event_loop, message) {
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
    fn open_dialog(&mut self, event_loop: &ActiveEventLoop, message: MessageBox) -> Result<(), String> {
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
        self.dialog = Some(Dialog { window: w, message });
        Ok(())
    }

    fn close_dialog(&mut self, event_loop: &ActiveEventLoop) {
        self.dialog = None;
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
            // `GetCancelResult(OK)` is OK: closing the window answers it.
            WindowEvent::CloseRequested => self.close_dialog(event_loop),
            WindowEvent::RedrawRequested => {
                let message = &d.message;
                if d.window.paint(|ui| message.ui(ui)).is_some() {
                    self.close_dialog(event_loop);
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
            // Light dismiss: a press outside the open menu closes it, and
            // Avalonia doesn't pass that press on.
            WindowEvent::MouseInput { state: ElementState::Pressed, .. }
                if self.state.flyout.as_ref().is_some_and(|f| !self.cursor.is_some_and(|c| f.contains(c))) =>
            {
                self.state.flyout = None;
                self.swallow_release = true;
                w.window.request_redraw();
            }
            WindowEvent::MouseInput { state: ElementState::Released, .. } if self.swallow_release => {
                self.swallow_release = false;
            }
            // Popups close with their window's activation.
            WindowEvent::Focused(false) => {
                if self.state.flyout.take().is_some() {
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
        let next = self.main.as_ref().and_then(|w| w.repaint_at);
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
