//! The Start screen and Application settings windows in the app: opening
//! them (`Program.StartApplicationAsync`, `ApplicationSettingsIntegration.Show`),
//! their events, and applying what they change (`ThemeManager`,
//! `ApplicationSettingsRuntime.Apply`).

use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Icon, WindowAttributes, WindowId};

use sharex_ui::gpu::Gpu;

use crate::app_settings::{ApplicationSettings, Effect};
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
        ];
        for w in windows.into_iter().flatten() {
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
        if let Some(tray) = &self.tray {
            tray.set_visible(s.ShowTray);
        }
        self.init_task_view_settings();
        self.refresh_menus();
    }

    fn settings_effects(&mut self, event_loop: &ActiveEventLoop, effects: Vec<Effect>) {
        for e in effects {
            match e {
                Effect::Appearance => {
                    self.apply_appearance();
                    // `Width`/`MinWidth` follow `ShareX.Density.Settings.WindowWidth`.
                    if let Some(sw) = &self.settings_window {
                        let width = sharex_ui::theme::density(sw.window.ctx()).settings_window_width;
                        let w = &sw.window.window;
                        let height = w.inner_size().to_logical::<f64>(w.scale_factor()).height;
                        w.set_min_inner_size(Some(LogicalSize::new(width, crate::app_settings::MIN_HEIGHT)));
                        let _ = w.request_inner_size(LogicalSize::new(width as f64, height));
                    }
                }
                Effect::ShowTray(v) => {
                    if let Some(tray) = &self.tray {
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
        false
    }

    /// Repaints the Start screen and settings window when they asked to be.
    pub(crate) fn app_windows_due(&mut self, now: std::time::Instant) -> Option<std::time::Instant> {
        let mut next = None;
        for w in [self.start_window.as_mut().map(|s| &mut s.window), self.settings_window.as_mut().map(|s| &mut s.window)]
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
