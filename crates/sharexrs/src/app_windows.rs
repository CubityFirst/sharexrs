//! The Start screen and the settings windows in the app: opening them
//! (`Program.StartApplicationAsync`, `ApplicationSettingsIntegration.Show`,
//! `HotkeySettingsIntegration.Show`, `TaskSettingsIntegration.Show`), their
//! events, and applying what they change (`ThemeManager`,
//! `ApplicationSettingsRuntime.Apply`, the hotkeys re-registered live).

use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Icon, WindowAttributes, WindowId};

use sharex_ui::gpu::Gpu;

use crate::app_settings::{ApplicationSettings, Effect};
use crate::hotkey_settings::{self, HotkeySettingsWindow};
use crate::task_settings_window::{self, TaskCtx, TaskSettingsWindow};
use crate::start_screen::StartScreen;
use crate::window::EguiWindow;
use crate::{App, app_icon, appearance, center, set_theme};

pub struct StartWindow {
    pub window: EguiWindow,
    pub state: StartScreen,
}

pub struct SettingsWindow {
    pub window: EguiWindow,
    pub state: ApplicationSettings,
}

pub struct HotkeyWindow {
    pub window: EguiWindow,
    pub state: HotkeySettingsWindow,
    /// The keyboard's modifiers, for recording a hotkey.
    modifiers: winit::keyboard::ModifiersState,
}

/// Whose task settings a Task settings window edits.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TaskTarget {
    /// `ApplicationState.DefaultTaskSettings`.
    Default,
    /// A hotkey's (`HotkeySettings.TaskSettings`), by its index.
    Hotkey(usize),
}

pub struct TaskWindow {
    pub window: EguiWindow,
    pub state: TaskSettingsWindow,
    pub target: TaskTarget,
}

/// A window with the system's frame (Avalonia's default chrome).
fn framed(title: &str, theme: egui::Theme) -> WindowAttributes {
    WindowAttributes::default()
        .with_title(title)
        .with_window_icon(Icon::from_rgba(app_icon::rgba64(), 64, 64).ok())
        .with_theme(Some(match theme {
            egui::Theme::Dark => winit::window::Theme::Dark,
            egui::Theme::Light => winit::window::Theme::Light,
        }))
        .with_visible(false)
}

impl App {
    /// `ThemeManager.ApplyConfiguredSettings`: the accent, icon pack and
    /// spacing app-wide, and the theme on every open window.
    pub(crate) fn apply_appearance(&mut self) {
        let a = appearance::apply(&self.settings.settings, self.system_theme);
        self.current_theme = a.theme;
        let windows = [
            self.main.as_ref(),
            self.dialog.as_ref().map(|d| &d.window),
            self.start_window.as_ref().map(|s| &s.window),
            self.settings_window.as_ref().map(|s| &s.window),
            self.hotkey_window.as_ref().map(|s| &s.window),
            self.task_window.as_ref().map(|s| &s.window),
        ];
        for w in windows.into_iter().flatten().chain(self.dialogs.iter().map(|d| &d.window)) {
            sharex_ui::theme::apply(w.ctx());
            set_theme(w, a.theme);
            w.window.request_redraw();
        }
    }

    /// `StartScreenWindow`: 700 wide, as tall as its content, capped to the
    /// screen's working area (`FitToWorkingArea`), centred.
    pub(crate) fn open_start_screen(&mut self, event_loop: &ActiveEventLoop) -> Result<(), String> {
        let gpu = match &self.gpu {
            Some(g) => g,
            None => self.gpu.insert(Gpu::new()?),
        };
        let attrs = framed(sharex_strings::sharex::StartScreen_Welcome(), self.current_theme)
            .with_inner_size(LogicalSize::new(crate::start_screen::WIDTH, 600.0))
            .with_resizable(false);
        let window = event_loop.create_window(attrs).map_err(|e| e.to_string())?;
        let mut w = EguiWindow::new(window, gpu, None)?;
        set_theme(&w, self.current_theme);
        let mut state = StartScreen::default();
        let settings = &self.settings.settings;
        let mut height = w.paint(|ui| state.content_height(ui, settings));
        let screens = sharex_capture::screenshot::screens();
        if let Some(screen) = screens.iter().find(|s| s.primary).or(screens.first()) {
            let scale = w.window.scale_factor() as f32;
            let frame = (w.window.outer_size().height as f32 - w.window.inner_size().height as f32) / scale;
            let available = screen.work_area.h as f32 / scale - frame;
            height = height.min(available.floor());
        }
        let _ = w.window.request_inner_size(LogicalSize::new(crate::start_screen::WIDTH, height));
        center(&w.window, event_loop);
        w.window.set_visible(true);
        w.window.focus_window();
        w.window.request_redraw();
        self.start_window = Some(StartWindow { window: w, state });
        Ok(())
    }

    /// The Start screen closed: it isn't shown again, nor the icon pack
    /// prompt (it has its own icon pack choice), and startup goes on.
    fn close_start_screen(&mut self, event_loop: &ActiveEventLoop) {
        self.start_window = None;
        let s = &mut self.settings.settings;
        s.ShowStartScreen = false;
        s.ShowIconPackPrompt = false;
        if let Err(e) = self.settings.save_application_config(false) {
            eprintln!("couldn't save the settings: {e}");
        }
        self.continue_start(event_loop);
    }

    /// `ApplicationSettingsIntegration.Show`: one window, brought to the
    /// front when it's already open.
    pub(crate) fn show_application_settings(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(s) = &self.settings_window {
            s.window.window.set_minimized(false);
            s.window.window.focus_window();
            return;
        }
        if let Err(e) = self.open_application_settings(event_loop) {
            eprintln!("couldn't open Application settings: {e}");
        }
    }

    fn open_application_settings(&mut self, event_loop: &ActiveEventLoop) -> Result<(), String> {
        let gpu = match &self.gpu {
            Some(g) => g,
            None => self.gpu.insert(Gpu::new()?),
        };
        let width = sharex_ui::theme::density(&egui::Context::default()).settings_window_width;
        let title = format!("ShareXrs - {}", sharex_strings::sharex::ApplicationSettingsWindow_WindowTitle().trim_start_matches("ShareX - "));
        let attrs = framed(&title, self.current_theme)
            .with_inner_size(LogicalSize::new(width, crate::app_settings::HEIGHT))
            .with_min_inner_size(LogicalSize::new(width, crate::app_settings::MIN_HEIGHT));
        let window = event_loop.create_window(attrs).map_err(|e| e.to_string())?;
        let w = EguiWindow::new(window, gpu, None)?;
        set_theme(&w, self.current_theme);
        center(&w.window, event_loop);
        w.window.set_visible(true);
        w.window.focus_window();
        w.window.request_redraw();
        let parent = self.settings.paths.screenshots_parent_folder(Some(&self.settings.settings));
        let state = ApplicationSettings::new(&self.settings.settings, self.settings.paths.portable, parent);
        self.settings_window = Some(SettingsWindow { window: w, state });
        Ok(())
    }

    /// The settings window closed (`ApplicationSettingsViewModel.Dispose`):
    /// the personal folder is stored, the settings applied and saved.
    fn close_application_settings(&mut self) {
        let Some(mut s) = self.settings_window.take() else { return };
        s.state.flush_personal_path();
        self.apply_settings();
        if let Err(e) = self.settings.save_application_config(false) {
            eprintln!("couldn't save the settings: {e}");
        }
    }

    /// `ApplicationSettingsRuntime.Apply`: what the app takes from the
    /// settings while it runs.
    pub(crate) fn apply_settings(&mut self) {
        let s = &self.settings.settings;
        self.hotkeys.repeat_limit = std::time::Duration::from_millis(s.hotkey_repeat_limit() as u64);
        if let Some(tray) = &mut self.tray {
            tray.set_visible(s.ShowTray);
        }
        self.init_task_view_settings();
        self.refresh_menus();
        self.apply_update_settings();
    }

    fn settings_effects(&mut self, event_loop: &ActiveEventLoop, effects: Vec<Effect>) {
        for e in effects {
            match e {
                Effect::Appearance => {
                    self.apply_appearance();
                    // `Width`/`MinWidth` follow `ShareX.Density.Settings.WindowWidth`.
                    let windows = [
                        self.settings_window.as_ref().map(|s| (&s.window, crate::app_settings::MIN_HEIGHT)),
                        self.task_window.as_ref().map(|s| (&s.window, task_settings_window::MIN_HEIGHT)),
                    ];
                    for (sw, min_height) in windows.into_iter().flatten() {
                        let width = sharex_ui::theme::density(sw.ctx()).settings_window_width;
                        let w = &sw.window;
                        let height = w.inner_size().to_logical::<f64>(w.scale_factor()).height;
                        w.set_min_inner_size(Some(LogicalSize::new(width, min_height)));
                        let _ = w.request_inner_size(LogicalSize::new(width as f64, height));
                    }
                }
                Effect::ShowTray(v) => {
                    if let Some(tray) = &mut self.tray {
                        tray.set_visible(v);
                    }
                }
                Effect::TrayIcon => {
                    self.tray_progress = -2;
                    self.update_tray_icon(-1);
                }
                Effect::Restart => self.restart(event_loop),
                Effect::OpenFolder(path) => open_path(&path),
                Effect::OpenUrl(url) => open_path(std::path::Path::new(url)),
                Effect::NotPorted(what) => eprintln!("{what}: not ported yet"),
            }
        }
    }

    /// `ApplicationLifecycle.Restart`: a new instance with the same
    /// arguments, then this one exits (saving its settings).
    pub(crate) fn restart(&mut self, event_loop: &ActiveEventLoop) {
        self.close_application_settings();
        if let Ok(exe) = std::env::current_exe() {
            let args: Vec<String> = std::env::args().skip(1).collect();
            if let Err(e) = std::process::Command::new(exe).args(args).spawn() {
                eprintln!("couldn't restart: {e}");
                return;
            }
        }
        self.exit(event_loop);
    }

    /// Routes an event to the Start screen or the settings window; false
    /// when it's neither's.
    pub(crate) fn app_window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: &WindowEvent) -> bool {
        if let Some(sw) = self.start_window.as_mut().filter(|s| s.window.window.id() == id) {
            match event {
                // Closing it is "Get started" too (`OnClosed`).
                WindowEvent::CloseRequested => self.close_start_screen(event_loop),
                WindowEvent::RedrawRequested => {
                    let settings = &mut self.settings.settings;
                    let state = &mut sw.state;
                    let out = sw.window.paint(|ui| state.ui(ui, settings));
                    if out.appearance {
                        self.apply_appearance();
                    }
                    if out.close {
                        self.close_start_screen(event_loop);
                    }
                }
                other => sw.window.on_event(other),
            }
            return true;
        }
        if let Some(sw) = self.settings_window.as_mut().filter(|s| s.window.window.id() == id) {
            match event {
                WindowEvent::CloseRequested => self.close_application_settings(),
                WindowEvent::RedrawRequested => {
                    let settings = &mut self.settings.settings;
                    let state = &mut sw.state;
                    let effects = sw.window.paint(|ui| state.ui(ui, settings));
                    self.settings_effects(event_loop, effects);
                }
                other => sw.window.on_event(other),
            }
            return true;
        }
        if self.hotkey_window.as_ref().is_some_and(|h| h.window.window.id() == id) {
            self.hotkey_window_event(event_loop, event);
            return true;
        }
        if self.task_window.as_ref().is_some_and(|t| t.window.window.id() == id) {
            self.task_window_event(event_loop, event);
            return true;
        }
        false
    }

    /// Repaints the Start screen and settings window when they asked to be.
    pub(crate) fn app_windows_due(&mut self, now: std::time::Instant) -> Option<std::time::Instant> {
        let mut next = None;
        for w in [
            self.start_window.as_mut().map(|s| &mut s.window),
            self.settings_window.as_mut().map(|s| &mut s.window),
            self.hotkey_window.as_mut().map(|s| &mut s.window),
            self.task_window.as_mut().map(|s| &mut s.window),
        ]
        .into_iter()
        .flatten()
        {
            match w.repaint_at {
                Some(at) if at <= now => {
                    w.window.request_redraw();
                    w.repaint_at = None;
                }
                Some(at) => next = Some(next.map_or(at, |n: std::time::Instant| n.min(at))),
                None => {}
            }
        }
        next
    }
}

impl App {
    /// `HotkeySettingsIntegration.Show`: one window, brought to the front
    /// when it's already open.
    pub(crate) fn show_hotkey_settings(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(h) = &self.hotkey_window {
            h.window.window.set_minimized(false);
            h.window.window.focus_window();
            return;
        }
        if let Err(e) = self.open_hotkey_settings(event_loop) {
            eprintln!("couldn't open Hotkey settings: {e}");
        }
    }

    fn open_hotkey_settings(&mut self, event_loop: &ActiveEventLoop) -> Result<(), String> {
        let gpu = match &self.gpu {
            Some(g) => g,
            None => self.gpu.insert(Gpu::new()?),
        };
        let title = format!("ShareXrs - {}", sharex_strings::sharex::HotkeySettingsWindow_WindowTitle().trim_start_matches("ShareX - "));
        let attrs = framed(&title, self.current_theme)
            .with_inner_size(LogicalSize::new(hotkey_settings::WIDTH, hotkey_settings::HEIGHT))
            .with_min_inner_size(LogicalSize::new(hotkey_settings::MIN_WIDTH, hotkey_settings::MIN_HEIGHT));
        let window = event_loop.create_window(attrs).map_err(|e| e.to_string())?;
        let w = EguiWindow::new(window, gpu, None)?;
        set_theme(&w, self.current_theme);
        center(&w.window, event_loop);
        w.window.set_visible(true);
        w.window.focus_window();
        w.window.request_redraw();
        let mut svc = hotkey_settings::Service {
            hotkeys: &mut self.settings.hotkeys.Hotkeys,
            manager: Some(&mut self.hotkeys),
            settings: &mut self.settings.settings,
        };
        let state = HotkeySettingsWindow::open(&mut svc);
        self.hotkey_window = Some(HotkeyWindow { window: w, state, modifiers: Default::default() });
        self.refresh_hotkey_tips();
        Ok(())
    }

    /// The window closed (`HotkeySettingsViewModel.Dispose`): presses count
    /// again, the menus follow and `HotkeysConfig.json` is saved.
    fn close_hotkey_settings(&mut self) {
        let Some(mut h) = self.hotkey_window.take() else { return };
        let mut svc = hotkey_settings::Service {
            hotkeys: &mut self.settings.hotkeys.Hotkeys,
            manager: Some(&mut self.hotkeys),
            settings: &mut self.settings.settings,
        };
        h.state.close(&mut svc);
        self.refresh_hotkey_tips();
        self.refresh_menus();
        if let Err(e) = self.settings.save_hotkeys_config(false) {
            eprintln!("couldn't save the hotkeys: {e}");
        }
    }

    fn hotkey_window_event(&mut self, event_loop: &ActiveEventLoop, event: &WindowEvent) {
        use winit::event::ElementState;
        use winit::keyboard::{KeyCode, PhysicalKey};
        let Some(h) = self.hotkey_window.as_mut() else { return };
        match event {
            WindowEvent::CloseRequested => self.close_hotkey_settings(),
            WindowEvent::RedrawRequested => {
                let mut svc = hotkey_settings::Service {
                    hotkeys: &mut self.settings.hotkeys.Hotkeys,
                    manager: Some(&mut self.hotkeys),
                    settings: &mut self.settings.settings,
                };
                let state = &mut h.state;
                let effects = h.window.paint(|ui| state.ui(ui, &mut svc));
                self.hotkey_effects(event_loop, effects);
            }
            WindowEvent::ModifiersChanged(m) => {
                h.modifiers = m.state();
                h.window.on_event(event);
            }
            // Recording a hotkey takes every key (`OnHotkeyKeyDown` tunnels,
            // so Enter and Space don't click), and Print Screen on its
            // release, which is all Windows sends for it.
            WindowEvent::KeyboardInput { event: key, .. } if h.state.capturing.is_some() => {
                let PhysicalKey::Code(code) = key.physical_key else { return };
                let print = code == KeyCode::PrintScreen;
                if (key.state == ElementState::Pressed) == print {
                    return;
                }
                let win = matches!(code, KeyCode::SuperLeft | KeyCode::SuperRight);
                let Some(k) = hotkey_settings::input_key(code) else { return };
                let m = h.modifiers;
                let gesture = hotkey_settings::Gesture {
                    key: k,
                    control: m.control_key() || matches!(code, KeyCode::ControlLeft | KeyCode::ControlRight),
                    shift: m.shift_key() || matches!(code, KeyCode::ShiftLeft | KeyCode::ShiftRight),
                    alt: m.alt_key() || matches!(code, KeyCode::AltLeft | KeyCode::AltRight),
                    win: win || m.super_key(),
                };
                let mut svc = hotkey_settings::Service {
                    hotkeys: &mut self.settings.hotkeys.Hotkeys,
                    manager: Some(&mut self.hotkeys),
                    settings: &mut self.settings.settings,
                };
                h.state.key_down(&mut svc, gesture, code == KeyCode::Escape);
                // What it changed comes back from the next frame.
                h.window.window.request_redraw();
            }
            other => h.window.on_event(other),
        }
    }

    fn hotkey_effects(&mut self, event_loop: &ActiveEventLoop, effects: Vec<hotkey_settings::Effect>) {
        use hotkey_settings::Effect as E;
        for e in effects {
            match e {
                E::EditTask(i) => self.show_task_settings(event_loop, TaskTarget::Hotkey(i)),
                E::EnableHotkeys => {
                    if self.settings.settings.DisableHotkeys {
                        self.toggle_hotkeys();
                    }
                }
                E::Changed => self.refresh_hotkey_tips(),
                E::Removed(i) => {
                    // A Task settings window on that hotkey has nothing left
                    // to edit; one on a later hotkey follows its index.
                    match self.task_window.as_ref().map(|t| t.target) {
                        Some(TaskTarget::Hotkey(j)) if j == i => self.task_window = None,
                        Some(TaskTarget::Hotkey(j)) if j > i => {
                            if let Some(t) = self.task_window.as_mut() {
                                t.target = TaskTarget::Hotkey(j - 1);
                            }
                        }
                        _ => {}
                    }
                }
                E::Moved(from, to) => {
                    if let Some(t) = self.task_window.as_mut()
                        && let TaskTarget::Hotkey(j) = t.target
                    {
                        t.target = TaskTarget::Hotkey(hotkey_settings::moved_index(j, from, to));
                    }
                }
            }
        }
    }

    /// `TaskSettingsIntegration.Show`: one Task settings window at a time;
    /// asking again brings it to the front.
    pub(crate) fn show_task_settings(&mut self, event_loop: &ActiveEventLoop, target: TaskTarget) {
        if let Some(t) = &self.task_window {
            t.window.window.set_minimized(false);
            t.window.window.focus_window();
            return;
        }
        if let Err(e) = self.open_task_settings(event_loop, target) {
            eprintln!("couldn't open Task settings: {e}");
        }
    }

    fn open_task_settings(&mut self, event_loop: &ActiveEventLoop, target: TaskTarget) -> Result<(), String> {
        let gpu = match &self.gpu {
            Some(g) => g,
            None => self.gpu.insert(Gpu::new()?),
        };
        let state = match target {
            TaskTarget::Default => TaskSettingsWindow::new(&mut self.settings.settings.DefaultTaskSettings, None),
            TaskTarget::Hotkey(i) => {
                let defaults = &self.settings.settings.DefaultTaskSettings;
                let task = &mut self.settings.hotkeys.Hotkeys.get_mut(i).ok_or("no such hotkey")?.TaskSettings;
                TaskSettingsWindow::new(task, Some(defaults))
            }
        };
        let title = match target {
            TaskTarget::Default => state.title(&self.settings.settings.DefaultTaskSettings),
            TaskTarget::Hotkey(i) => state.title(&self.settings.hotkeys.Hotkeys[i].TaskSettings),
        };
        let width = sharex_ui::theme::density(&egui::Context::default()).settings_window_width;
        let attrs = framed(&title, self.current_theme)
            .with_inner_size(LogicalSize::new(width, task_settings_window::HEIGHT))
            .with_min_inner_size(LogicalSize::new(width, task_settings_window::MIN_HEIGHT));
        let window = event_loop.create_window(attrs).map_err(|e| e.to_string())?;
        let w = EguiWindow::new(window, gpu, None)?;
        set_theme(&w, self.current_theme);
        center(&w.window, event_loop);
        w.window.set_visible(true);
        w.window.focus_window();
        w.window.request_redraw();
        self.task_window = Some(TaskWindow { window: w, state, target });
        Ok(())
    }

    /// The window closed: the default task settings refresh the menus and
    /// save `ApplicationConfig.json`; a hotkey's save `HotkeysConfig.json`
    /// (`MainWindowIntegration`, `HotkeySettingsAvaloniaService.EditTask`).
    fn close_task_settings(&mut self) {
        let Some(t) = self.task_window.take() else { return };
        self.refresh_menus();
        self.refresh_hotkey_tips();
        let saved = match t.target {
            TaskTarget::Default => self.settings.save_application_config(false),
            TaskTarget::Hotkey(_) => self.settings.save_hotkeys_config(false),
        };
        if let Err(e) = saved {
            eprintln!("couldn't save the settings: {e}");
        }
    }

    fn task_window_event(&mut self, event_loop: &ActiveEventLoop, event: &WindowEvent) {
        let Some(t) = self.task_window.as_mut() else { return };
        match event {
            WindowEvent::CloseRequested => self.close_task_settings(),
            WindowEvent::RedrawRequested => {
                let s = &mut self.settings;
                let mut ctx = match t.target {
                    TaskTarget::Default => TaskCtx { task: &mut s.settings.DefaultTaskSettings, auto_increment: &mut s.settings.NameParserAutoIncrementNumber },
                    TaskTarget::Hotkey(i) => match s.hotkeys.Hotkeys.get_mut(i) {
                        Some(h) => TaskCtx { task: &mut h.TaskSettings, auto_increment: &mut s.settings.NameParserAutoIncrementNumber },
                        None => return,
                    },
                };
                let state = &mut t.state;
                let effects = t.window.paint(|ui| state.ui(ui, &mut ctx));
                let target = t.target;
                let mut ffmpeg = false;
                for e in effects {
                    match e {
                        task_settings_window::Effect::NotPorted(what) => eprintln!("{what}: not ported yet"),
                        task_settings_window::Effect::FFmpegOptions => ffmpeg = true,
                    }
                }
                if ffmpeg {
                    self.show_ffmpeg_options(event_loop, target);
                    return;
                }
                // The hotkey list shows the task and its "custom settings" mark.
                if matches!(t.target, TaskTarget::Hotkey(_))
                    && let Some(h) = &self.hotkey_window
                {
                    h.window.window.request_redraw();
                }
            }
            other => t.window.on_event(other),
        }
    }
}

/// Opens a folder or URL with the system (`FileHelpers.OpenFolder`,
/// `URLHelpers.OpenURL`).
pub fn open_path(path: &std::path::Path) {
    #[cfg(windows)]
    let r = std::process::Command::new("explorer").arg(path).spawn();
    #[cfg(target_os = "macos")]
    let r = std::process::Command::new("open").arg(path).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let r = std::process::Command::new("xdg-open").arg(path).spawn();
    if let Err(e) = r {
        eprintln!("couldn't open {}: {e}", path.display());
    }
}
