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
    /// The emoji picker over the overlay (`ShowEmojiPickerDialog` in the
    /// region capture's `EditorView`), on the overlay it opened on.
    emoji: Option<(usize, Box<crate::image_editor::emoji_picker::EmojiPicker>)>,
    /// Active monitor mode keeps the pointer on the overlay while it's
    /// active (freed when the session ends).
    cursor_lock: Option<sharex_capture::CursorLock>,
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
    use KeyCode as K;
    match event.physical_key {
        PhysicalKey::Code(K::Numpad0) => return Key::NumPad0,
        PhysicalKey::Code(K::Backquote) => return Key::Tilde,
        PhysicalKey::Code(K::KeyH) => return Key::H,
        PhysicalKey::Code(code) => {
            let numpad = [K::Numpad1, K::Numpad2, K::Numpad3, K::Numpad4, K::Numpad5, K::Numpad6, K::Numpad7, K::Numpad8, K::Numpad9];
            if let Some(i) = numpad.iter().position(|c| *c == code) {
                return Key::NumPad(i as u8 + 1);
            }
            let digits = [K::Digit0, K::Digit1, K::Digit2, K::Digit3, K::Digit4, K::Digit5, K::Digit6, K::Digit7, K::Digit8, K::Digit9];
            if let Some(i) = digits.iter().position(|c| *c == code) {
                return Key::Digit(i as u8);
            }
            // Letters by their place, as Avalonia's `Key` (the virtual key).
            let name = format!("{code:?}");
            if let Some(l) = name.strip_prefix("Key")
                && l.len() == 1
            {
                // The layout's letter, when it is one.
                if let winit::keyboard::Key::Character(c) = &event.logical_key
                    && let Some(ch) = c.chars().next().filter(|ch| ch.is_ascii_alphabetic())
                {
                    return Key::Letter(ch.to_ascii_uppercase());
                }
                return Key::Letter(l.chars().next().unwrap_or('?'));
            }
        }
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
            NamedKey::Backspace => Key::Backspace,
            NamedKey::Home => Key::Home,
            NamedKey::End => Key::End,
            NamedKey::PageUp => Key::PageUp,
            NamedKey::PageDown => Key::PageDown,
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
        let screen_bounds = request.screen_bounds;
        let active_monitor = request.options.ActiveMonitorMode;
        let capture = RegionCapture::new(request);
        let mut overlays = Vec::new();
        // The monitors the screenshot covers (only the active one in
        // active monitor mode, as ShareX's window spans `ScreenBounds`).
        for screen in screens.iter().filter(|s| s.bounds.intersect(&screen_bounds).is_valid()) {
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
            emoji: None,
            cursor_lock: None,
        };
        // `Helpers.LockCursorToWindow` in active monitor mode.
        if active_monitor {
            me.cursor_lock = me.overlays.first().map(|o| sharex_capture::CursorLock::new(platform::window_handle(&o.window.window)));
        }
        let now = Instant::now();
        for o in &mut me.overlays {
            let capture = &mut me.capture;
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
            // `WindowGeometryLog.Record(this, "Region capture", ScreenBounds,
            // imageSize)`: one overlay per monitor here, each its part.
            let m = o.view.monitor;
            crate::window_geometry_log::record(&o.window.window, "Region capture", m, (m.w.max(0) as u32, m.h.max(0) as u32));
        }
        me.update_cursor(event_loop);
        Ok(me)
    }

    /// The workspace's emoji picker requests: the picker opens on the
    /// overlay under the pointer.
    pub fn handle_editor_requests(&mut self) {
        for r in self.capture.take_editor_requests() {
            let replace = match r {
                sharex_image_editor::editor::Request::EmojiPicker => None,
                sharex_image_editor::editor::Request::ReplaceEmoji { id } => Some(id),
                _ => continue,
            };
            if self.emoji.is_some() {
                continue;
            }
            let at = sharex_capture::screenshot::cursor_position();
            let i = at.and_then(|(x, y)| self.overlays.iter().position(|o| o.view.monitor.contains(x, y))).unwrap_or(0);
            self.emoji = Some((i, Box::new(crate::image_editor::emoji_picker::EmojiPicker::new(replace))));
            self.redraw();
        }
    }

    /// `InsertEmojiAnnotation`: centred on the monitor of overlay `i`.
    fn insert_emoji(&mut self, i: usize, unicode: &str, name: &str) {
        let m = self.overlays[i].view.monitor;
        let (ox, oy) = self.capture.origin();
        let size = sharex_image_editor::editor::EMOJI_SIZE;
        let (cx, cy) = ((m.x + m.w / 2 - ox) as f64, (m.y + m.h / 2 - oy) as f64);
        self.capture.insert_emoji(unicode, name, (cx - size / 2.0, cy - size / 2.0));
    }

    /// A colour popup's screen colour picker request.
    pub fn take_pick_request(&mut self) -> Option<sharex_capture::region::options_row::OptionItem> {
        self.capture.take_pick_request()
    }

    /// The colour picked for `item` (none: cancelled).
    pub fn picked_color(&mut self, item: sharex_capture::region::options_row::OptionItem, color: Option<egui::Color32>) {
        if let Some(c) = color {
            self.capture.set_picked_color(item, c);
        }
        if let Some(o) = self.overlays.first() {
            o.window.window.focus_window();
        }
        self.redraw();
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
                CursorKind::Text => o.window.window.set_cursor(CursorIcon::Text),
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

    /// The pointer moved over overlay `i`: the toolbar part under it (when
    /// no button is down), a dragged slider, then the overlay.
    fn cursor_moved(&mut self, i: usize, position: PhysicalPosition<f64>) -> Outcome {
        let m = self.modifiers;
        self.overlays[i].pointer = Some(position);
        let o = &self.overlays[i];
        let ppp = o.window.window.scale_factor() as f32;
        let at = egui::pos2(position.x as f32 / ppp, position.y as f32 / ppp);
        if self.buttons_down == 0 && !self.capture.dragging_toolbar() {
            let part = o.view.toolbar_part_at(at);
            self.capture.set_toolbar_hover(part);
        }
        if let Some(item) = self.capture.slider_dragging()
            && let Some(t) = o.view.slider_fraction(item, at)
        {
            self.capture.set_slider_fraction(item, t);
        }
        let p = self.image_point(i, position);
        self.capture.pointer_moved(p, m)
    }

    /// Handles an event of one of the overlay windows.
    pub fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) -> Outcome {
        let Some(i) = self.overlays.iter().position(|o| o.window.window.id() == id) else { return Outcome::Continue };
        let m = self.modifiers;
        // An open popup's widgets (palette, eyedropper, lists, sliders) are
        // egui's: they see the pointer too (the overlay ignores presses on
        // the popup itself).
        if self.capture.popup_open()
            && matches!(event, WindowEvent::CursorMoved { .. } | WindowEvent::MouseInput { .. } | WindowEvent::MouseWheel { .. } | WindowEvent::CursorLeft { .. })
        {
            self.overlays[i].window.on_event(&event);
        }
        let outcome = match event {
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let capture = &mut self.capture;
                let o = &mut self.overlays[i];
                let view = &mut o.view;
                let emoji = &mut self.emoji;
                let mut picked = None;
                o.window.paint(|ui| {
                    capture.paint(ui, view, now);
                    if let Some((on, d)) = emoji.as_mut()
                        && *on == i
                    {
                        picked = Some(show_emoji(ui, d));
                    }
                });
                match picked {
                    Some(Some(EmojiOutcome::Pick(unicode, name))) => {
                        let replace = self.emoji.take().and_then(|(_, d)| d.replace.clone());
                        match replace {
                            Some(id) => self.capture.replace_emoji(&id, &unicode, &name),
                            None => self.insert_emoji(i, &unicode, &name),
                        }
                        self.redraw();
                    }
                    Some(Some(EmojiOutcome::Cancel)) => {
                        self.emoji = None;
                        self.redraw();
                    }
                    Some(None) => self.overlays[i].window.window.request_redraw(),
                    None => {}
                }
                return Outcome::Continue;
            }
            // The emoji picker has the input while it's open; a press that
            // began before it opened (the Emoji tool's button) still ends
            // in the overlay, or the toolbar would stay pressed on Emoji
            // and the next press would open the picker again.
            other if self.emoji.is_some() => {
                if let WindowEvent::MouseInput { state: ElementState::Released, button, .. } = &other
                    && self.buttons_down > 0
                {
                    self.buttons_down -= 1;
                    let b = match button {
                        MouseButton::Left => Some(Button::Left),
                        MouseButton::Right => Some(Button::Right),
                        MouseButton::Middle => Some(Button::Middle),
                        _ => None,
                    };
                    if let (Some(b), Some(p)) = (b, self.last_point(i)) {
                        let _ = self.capture.pointer_released(p, b, m);
                    }
                }
                self.overlays[i].window.on_event(&other);
                self.overlays[i].window.window.request_redraw();
                return Outcome::Continue;
            }
            WindowEvent::CursorMoved { position, .. } => self.cursor_moved(i, position),
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
                // A button message has its own position (Avalonia's pointer
                // events do): a move we weren't told of comes first.
                if state == ElementState::Pressed
                    && let Some((x, y)) = sharex_capture::screenshot::cursor_position()
                {
                    let mon = self.overlays[i].view.monitor;
                    let pos = PhysicalPosition::new((x - mon.x) as f64, (y - mon.y) as f64);
                    if mon.contains(x, y) && self.overlays[i].pointer != Some(pos) {
                        self.cursor_moved(i, pos);
                    }
                }
                let Some(p) = self.last_point(i) else { return Outcome::Continue };
                let scale = self.overlays[i].scale;
                if std::env::var_os("SHAREXRS_LOG_INPUT").is_some() {
                    eprintln!(
                        "region input: {state:?} {button:?} at {:.0},{:.0}, {} down, toolbar {:?}",
                        p.x,
                        p.y,
                        self.buttons_down,
                        self.capture.toolbar_hover()
                    );
                }
                match state {
                    ElementState::Pressed => {
                        self.buttons_down += 1;
                        let outcome = self.capture.pointer_pressed(p, button, m, scale);
                        // A slider of the tool options jumps to the press.
                        if let (Some(item), Some(pos)) = (self.capture.slider_dragging(), self.overlays[i].pointer) {
                            let ppp = self.overlays[i].window.window.scale_factor() as f32;
                            let at = egui::pos2(pos.x as f32 / ppp, pos.y as f32 / ppp);
                            if let Some(t) = self.overlays[i].view.slider_fraction(item, at) {
                                self.capture.set_slider_fraction(item, t);
                            }
                        }
                        outcome
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
                    ElementState::Pressed => {
                        // Typed text goes to the text box being edited.
                        if self.capture.wants_text()
                            && let Some(text) = event.text.as_ref().filter(|t| t.chars().any(|c| !c.is_control()))
                            && !m.ctrl
                        {
                            self.capture.text_input(text);
                            Outcome::Continue
                        } else {
                            self.capture.key_down(k, m, &mut platform::set_cursor_position)
                        }
                    }
                    ElementState::Released => self.capture.key_up(k, m),
                }
            }

            WindowEvent::CloseRequested => self.capture.cancel(),
            WindowEvent::Focused(focused) => {
                self.overlays[i].window.on_event(&WindowEvent::Focused(focused));
                if let Some(lock) = self.cursor_lock.as_mut() {
                    lock.focused(focused);
                }
                Outcome::Continue
            }
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

enum EmojiOutcome {
    Pick(String, String),
    Cancel,
}

/// The emoji picker centred on the overlay, over a scrim (`modal-scrim`).
fn show_emoji(ui: &mut egui::Ui, d: &mut crate::image_editor::emoji_picker::EmojiPicker) -> Option<EmojiOutcome> {
    use crate::image_editor::{emoji_picker, modal};
    let p = sharex_ui::palette(ui.ctx());
    let full = ui.max_rect();
    let size = d.size(full);
    let rect = egui::Rect::from_min_size(egui::pos2((full.center().x - size.x / 2.0).round(), (full.center().y - size.y / 2.0).round()), size);
    let combo_open = sharex_ui::controls::ComboBox::is_open(ui.ctx(), egui::Id::new("emoji-group"));
    let outside = modal::scrim(ui, &p, full, rect) && !combo_open;
    let escape = ui.input(|i| i.key_released(egui::Key::Escape));
    match d.show(ui, &p, rect) {
        Some(emoji_picker::Outcome::Pick(e)) => Some(EmojiOutcome::Pick(e.unicode.clone(), e.display_name().to_string())),
        Some(emoji_picker::Outcome::Cancel) => Some(EmojiOutcome::Cancel),
        None if outside || escape => Some(EmojiOutcome::Cancel),
        None => None,
    }
}
