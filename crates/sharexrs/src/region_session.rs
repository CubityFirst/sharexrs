//! The region capture overlay on screen: one borderless, topmost window
//! per monitor (snapr's approach; ShareX stretches one window over every
//! monitor, which Windows then gives a single DPI), all feeding one
//! [`RegionCapture`] session. Each window is painted before it's shown, so
//! it never flashes empty.

use std::collections::HashMap;
use std::time::Instant;

use sharex_capture::region::{
    Button, CursorKind, Key, Modifiers, Outcome, OverlayView, RegionCapture, Request, cursors::CustomCursor,
};
use sharex_capture::{PointF, Screen, platform};
use sharex_ui::gpu::Gpu;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{KeyCode, NamedKey, PhysicalKey};
use winit::window::{CursorIcon, WindowAttributes, WindowId, WindowLevel};

use crate::window::EguiWindow;

struct Overlay {
    window: EguiWindow,
    view: OverlayView,
    /// Its monitor's scaling.
    scale: f64,
    /// The pointer over it, physical pixels from its top-left corner.
    pointer: Option<PhysicalPosition<f64>>,
}

pub struct RegionSession {
    pub capture: RegionCapture,
    overlays: Vec<Overlay>,
    modifiers: Modifiers,
    buttons_down: u32,
    cursors: HashMap<(CustomCursor, u32), winit::window::CustomCursor>,
    cursor_shown: Option<(CursorKind, u32)>,
}

/// A borderless, topmost window covering `screen`, still hidden.
fn open_window(event_loop: &ActiveEventLoop, screen: &Screen) -> Result<winit::window::Window, String> {
    let b = screen.bounds;
    #[allow(unused_mut)]
    let mut attrs = WindowAttributes::default()
        .with_title(sharex_strings::screen_capture_lib::BaseRegionForm_InitializeComponent_Region_capture())
        .with_decorations(false)
        .with_resizable(false)
        .with_visible(false)
        .with_window_level(WindowLevel::AlwaysOnTop)
        .with_position(PhysicalPosition::new(b.x, b.y))
        .with_inner_size(PhysicalSize::new(b.w as u32, b.h as u32));
    #[cfg(windows)]
    {
        use winit::platform::windows::WindowAttributesExtWindows;
        attrs = attrs.with_skip_taskbar(true);
    }
    // winit turns a new window's physical place and size into points by the
    // main screen's scale; this monitor's own is the right one
    // (docs/MACOS-LEARNINGS.md 3.8).
    #[cfg(target_os = "macos")]
    {
        let monitor = event_loop
            .available_monitors()
            .min_by_key(|m| (m.position().x - b.x).abs() + (m.position().y - b.y).abs());
        if let Some(m) = monitor {
            let scale = m.scale_factor();
            attrs = attrs
                .with_position(m.position().to_logical::<f64>(scale))
                .with_inner_size(m.size().to_logical::<f64>(scale))
                .with_transparent(true);
        }
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let monitor = event_loop
            .available_monitors()
            .min_by_key(|m| (m.position().x - b.x).abs() + (m.position().y - b.y).abs());
        attrs = attrs.with_fullscreen(Some(winit::window::Fullscreen::Borderless(monitor)));
    }
    let window = event_loop.create_window(attrs).map_err(|e| e.to_string())?;
    platform::prepare_overlay(&window);
    Ok(window)
}

fn key(event: &winit::event::KeyEvent) -> Key {
    match event.physical_key {
        PhysicalKey::Code(KeyCode::Numpad0) => return Key::NumPad0,
        PhysicalKey::Code(KeyCode::Backquote) => return Key::Tilde,
        PhysicalKey::Code(KeyCode::KeyH) => return Key::H,
        _ => {}
    }
    match &event.logical_key {
        winit::keyboard::Key::Named(n) => match n {
            NamedKey::Escape => Key::Escape,
            NamedKey::Enter => Key::Enter,
            NamedKey::Space => Key::Space,
            NamedKey::Tab => Key::Tab,
            NamedKey::ArrowLeft => Key::Left,
            NamedKey::ArrowRight => Key::Right,
            NamedKey::ArrowUp => Key::Up,
            NamedKey::ArrowDown => Key::Down,
            NamedKey::Delete => Key::Delete,
            NamedKey::Shift => Key::Shift,
            NamedKey::Control => Key::Control,
            NamedKey::Alt => Key::Alt,
            NamedKey::Super => Key::Super,
            _ => Key::Other,
        },
        _ => Key::Other,
    }
}

impl RegionSession {
    /// Opens an overlay on every monitor for `request` and focuses the one
    /// under the pointer.
    pub fn open(event_loop: &ActiveEventLoop, gpu: &Gpu, request: Request, dark: bool) -> Result<Self, String> {
        let screens = request.screens.clone();
        let cursor = request.cursor;
        let capture = RegionCapture::new(request);
        let mut overlays = Vec::new();
        for screen in &screens {
            let window = open_window(event_loop, screen)?;
            let w = EguiWindow::new(window, gpu, None)?;
            w.ctx().set_theme(if dark { egui::Theme::Dark } else { egui::Theme::Light });
            overlays.push(Overlay { window: w, view: OverlayView::new(screen.bounds), scale: screen.scale, pointer: None });
        }
        let mut me = Self {
            capture,
            overlays,
            modifiers: Modifiers::default(),
            buttons_down: 0,
            cursors: HashMap::new(),
            cursor_shown: None,
        };
        let now = Instant::now();
        for o in &mut me.overlays {
            let capture = &me.capture;
            let view = &mut o.view;
            o.window.paint(|ui| capture.paint(ui, view, now));
            o.window.window.set_visible(true);
        }
        let focus = cursor
            .and_then(|(x, y)| me.overlays.iter().position(|o| o.view.monitor.contains(x, y)))
            .unwrap_or(0);
        if let Some(o) = me.overlays.get(focus) {
            o.window.window.focus_window();
        }
        for o in &me.overlays {
            platform::overlay_presented(&o.window.window);
        }
        me.update_cursor(event_loop);
        Ok(me)
    }

    pub fn owns(&self, id: WindowId) -> bool {
        self.overlays.iter().any(|o| o.window.window.id() == id)
    }

    /// When the session wants to be woken (the input delay, animations).
    pub fn wake_at(&mut self) -> Option<Instant> {
        let at = self.capture.tick(Instant::now());
        if at.is_some() {
            self.redraw();
        }
        at
    }

    fn redraw(&self) {
        for o in &self.overlays {
            o.window.window.request_redraw();
        }
    }

    /// The image point under a position in overlay `i`.
    fn image_point(&self, i: usize, p: PhysicalPosition<f64>) -> PointF {
        let sb = self.capture.screen_bounds();
        let m = self.overlays[i].view.monitor;
        PointF::new((m.x - sb.x) as f64 + p.x, (m.y - sb.y) as f64 + p.y)
    }

    fn last_point(&self, i: usize) -> Option<PointF> {
        self.overlays[i].pointer.map(|p| self.image_point(i, p))
    }

    fn update_cursor(&mut self, event_loop: &ActiveEventLoop) {
        let kind = self.capture.cursor();
        for o in &self.overlays {
            let scale = (o.scale * 100.0).round() as u32;
            if self.cursor_shown == Some((kind, scale)) {
                continue;
            }
            match kind {
                CursorKind::Arrow => o.window.window.set_cursor(CursorIcon::Default),
                CursorKind::Move => o.window.window.set_cursor(CursorIcon::Move),
                CursorKind::Custom(c) => {
                    let cursor = self.cursors.entry((c, scale)).or_insert_with(|| {
                        let b = sharex_capture::region::cursors::cursor(c, o.scale).expect("ShareX's cursors decode");
                        let source = winit::window::CustomCursor::from_rgba(b.rgba, b.width, b.height, b.hotspot.0, b.hotspot.1)
                            .expect("a valid cursor");
                        event_loop.create_custom_cursor(source)
                    });
                    o.window.window.set_cursor(cursor.clone());
                }
            }
        }
        if let Some(o) = self.overlays.first() {
            self.cursor_shown = Some((kind, (o.scale * 100.0).round() as u32));
        }
    }

    /// Handles an event of one of the overlay windows.
    pub fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) -> Outcome {
        let Some(i) = self.overlays.iter().position(|o| o.window.window.id() == id) else { return Outcome::Continue };
        let m = self.modifiers;
        let outcome = match event {
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let capture = &self.capture;
                let o = &mut self.overlays[i];
                let view = &mut o.view;
                o.window.paint(|ui| capture.paint(ui, view, now));
                return Outcome::Continue;
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.overlays[i].pointer = Some(position);
                if self.buttons_down == 0 && !self.capture.dragging_toolbar() {
                    let o = &self.overlays[i];
                    let ppp = o.window.window.scale_factor() as f32;
                    let part = o.view.toolbar_part_at(egui::pos2(position.x as f32 / ppp, position.y as f32 / ppp));
                    self.capture.set_toolbar_hover(part);
                }
                let p = self.image_point(i, position);
                self.capture.pointer_moved(p, m)
            }
            WindowEvent::CursorLeft { .. } => {
                if self.buttons_down == 0 {
                    self.capture.pointer_left();
                }
                Outcome::Continue
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let button = match button {
                    MouseButton::Left => Button::Left,
                    MouseButton::Right => Button::Right,
                    MouseButton::Middle => Button::Middle,
                    MouseButton::Back => Button::X1,
                    MouseButton::Forward => Button::X2,
                    MouseButton::Other(_) => return Outcome::Continue,
                };
                let Some(p) = self.last_point(i) else { return Outcome::Continue };
                let scale = self.overlays[i].scale;
                match state {
                    ElementState::Pressed => {
                        self.buttons_down += 1;
                        self.capture.pointer_pressed(p, button, m, scale)
                    }
                    ElementState::Released => {
                        self.buttons_down = self.buttons_down.saturating_sub(1);
                        self.capture.pointer_released(p, button, m)
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let y = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y as f64,
                    MouseScrollDelta::PixelDelta(p) => p.y,
                };
                self.capture.wheel(y, m)
            }
            WindowEvent::ModifiersChanged(mods) => {
                let s = mods.state();
                self.modifiers = Modifiers { shift: s.shift_key(), ctrl: s.control_key(), alt: s.alt_key() };
                Outcome::Continue
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let k = key(&event);
                match event.state {
                    ElementState::Pressed => self.capture.key_down(k, m, &mut platform::set_cursor_position),
                    ElementState::Released => self.capture.key_up(k, m),
                }
            }
            WindowEvent::CloseRequested => self.capture.cancel(),
            other => {
                self.overlays[i].window.on_event(&other);
                return Outcome::Continue;
            }
        };
        self.update_cursor(event_loop);
        self.redraw();
        outcome
    }
}
