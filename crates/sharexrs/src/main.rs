//! ShareXrs: ShareX, ported to Rust and egui.
//!
//! `sharexrs` runs the app. `sharexrs --render <screen> --out <png>` draws
//! one screen offscreen instead, at the size ShareX's would be, for
//! comparing against a screenshot of ShareX (`scripts/compare.ps1`).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app_icon;
mod main_menu;
mod main_window;
mod region_render;
mod render;
mod test_pattern;
mod window;

use std::time::Instant;

use sharex_ui::gpu::Gpu;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Icon, WindowAttributes, WindowId, WindowLevel};

use main_window::{Action, MainWindow};
use window::EguiWindow;

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
    let event_loop = EventLoop::new().expect("event loop");
    let mut app = App { gpu: None, main: None, state: MainWindow::new(), cursor: None, swallow_release: false };
    event_loop.run_app(&mut app).expect("run");
}

struct App {
    gpu: Option<Gpu>,
    main: Option<EguiWindow>,
    state: MainWindow,
    /// The pointer in the main window, in points.
    cursor: Option<egui::Pos2>,
    /// A press closed a menu (light dismiss); its release goes nowhere either.
    swallow_release: bool,
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
                let ctx = w.ctx();
                let next = match ctx.theme() {
                    egui::Theme::Dark => egui::Theme::Light,
                    egui::Theme::Light => egui::Theme::Dark,
                };
                ctx.set_theme(next);
                w.window.set_theme(Some(match next {
                    egui::Theme::Dark => winit::window::Theme::Dark,
                    egui::Theme::Light => winit::window::Theme::Light,
                }));
            }
            Action::Nav(i, _) => eprintln!("sidebar entry {i}: not ported yet"),
            Action::Menu(command) => eprintln!("menu: {command:?}: not ported yet"),
        }
        w.window.request_redraw();
    }
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

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.main.is_none()
            && let Err(e) = self.open_main_window(event_loop)
        {
            eprintln!("couldn't open the main window: {e}");
            event_loop.exit();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
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
