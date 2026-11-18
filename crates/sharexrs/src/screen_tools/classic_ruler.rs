//! Tools â€º Ruler: `ShareX.Tools/Tools/ClassicRuler/ClassicRulerWindow` and
//! `Controls/ClassicRulerOverlayControl`, the ShareX 21 ruler drawn with
//! the region capture's visuals. The screen is frozen; a drawn (or clicked
//! window) rectangle shows its position, size, area, perimeter, diagonal and
//! angle, with ticks every 10 and 100 pixels, the centre crosshair and
//! resize handles; the pointer has the magnifier and its position. Options
//! come from the task's region capture options (`OpenClassicRuler`).

use std::time::{Duration, Instant};

use egui::{Color32, CornerRadius, Rect, Stroke, StrokeKind, TextureOptions, pos2, vec2};
use image::RgbaImage;
use sharex_capture::region::cursors::CustomCursor;
use sharex_capture::region::paint::{self as hud, Mapper};
use sharex_capture::{RectF, RegionCaptureOptions};
use sharex_tools::ruler::Rect as R;
use sharex_tools::text::{Arg, format_dotnet};
use sharex_ui::theme::palette;
use sharex_ui::widgets;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, KeyCode, NamedKey, PhysicalKey};
use winit::window::WindowId;

use super::overlay::Overlay;

const DRAG_THRESHOLD: i32 = 4;
const HANDLE_SIZE: f64 = 15.0;
const INPUT_DELAY: Duration = Duration::from_millis(500);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Interaction {
    None,
    Pending,
    Creating,
    Moving,
    Resizing,
}

/// `Edges` (left 1, top 2, right 4, bottom 8).
const LEFT: u8 = 1;
const TOP: u8 = 2;
const RIGHT: u8 = 4;
const BOTTOM: u8 = 8;
const HANDLE_EDGES: [u8; 8] = [LEFT | TOP, TOP, RIGHT | TOP, RIGHT, RIGHT | BOTTOM, BOTTOM, LEFT | BOTTOM, LEFT];

/// `ClassicRulerOptions`, from the region capture options.
#[derive(Clone, Debug)]
pub struct ClassicRulerOptions {
    pub show_info: bool,
    pub use_custom_info_text: bool,
    pub custom_info_text: String,
    pub show_magnifier: bool,
    pub use_square_magnifier: bool,
    pub magnifier_size: i32,
    pub magnifier_pixel_count: i32,
    pub background_dim_strength: i32,
    pub dim_all_except_selection: bool,
    pub show_center_crosshair: bool,
    pub show_screen_crosshair: bool,
    pub detect_windows: bool,
    pub detect_controls: bool,
}

impl ClassicRulerOptions {
    /// `OpenClassicRuler`'s copy of `RegionCaptureOptions`.
    pub fn from_region(o: &RegionCaptureOptions) -> Self {
        Self {
            show_info: o.ShowInfo,
            use_custom_info_text: o.UseCustomInfoText,
            custom_info_text: o.CustomInfoText.clone(),
            show_magnifier: o.ShowMagnifier,
            use_square_magnifier: o.UseSquareMagnifier,
            magnifier_size: o.MagnifierSize,
            magnifier_pixel_count: o.MagnifierPixelCount,
            background_dim_strength: o.BackgroundDimStrength,
            dim_all_except_selection: o.DimAllExceptSelection,
            show_center_crosshair: o.ShowCenterCrosshair,
            show_screen_crosshair: o.ShowScreenCrosshair,
            detect_windows: o.DetectWindows,
            detect_controls: o.DetectControls,
        }
    }
}

pub enum Outcome {
    Continue,
    Close,
    Copy(String),
}

pub struct ClassicRuler {
    overlay: Overlay,
    screenshot: RgbaImage,
    /// The desktop's top-left (`_origin`).
    origin: (i32, i32),
    monitors: Vec<R>,
    windows: Vec<R>,
    options: ClassicRulerOptions,
    width: i32,
    height: i32,
    selection: Option<R>,
    hover: Option<R>,
    interaction: Interaction,
    resize_edges: u8,
    press: (i32, i32),
    last_creation: (i32, i32),
    start_selection: R,
    pointer: Option<(i32, i32)>,
    moving_during_creation: bool,
    control_held_during_creation: bool,
    suppress_right_release: bool,
    opened: Instant,
    shift: bool,
    control: bool,
    alt: bool,
    textures: Vec<Option<egui::TextureHandle>>,
    magnifier: Option<((i32, i32), RgbaImage)>,
}

fn contains(r: &R, p: (i32, i32)) -> bool {
    r.contains(p.0, p.1)
}

impl ClassicRuler {
    pub fn open(
        event_loop: &ActiveEventLoop,
        gpu: &sharex_ui::gpu::Gpu,
        options: ClassicRulerOptions,
        theme: egui::Theme,
    ) -> Result<Self, String> {
        let screens = sharex_capture::screenshot::screens();
        let b = super::overlay::desktop_bounds(&screens);
        let screenshot = sharex_capture::platform::capture_rectangle(b).ok_or("couldn't capture the screen")?;
        let title = crate::rs_title(sharex_strings::tools::ClassicRulerWindow_Title());
        let mut overlay = Overlay::open(event_loop, gpu, &screens, &title, false, theme)?;
        overlay.set_cursor(event_loop, CustomCursor::Crosshair);
        let monitors = screens.iter().map(|s| R::new(s.bounds.x - b.x, s.bounds.y - b.y, s.bounds.w, s.bounds.h)).collect();
        let n = overlay.windows.len();
        let mut me = Self {
            overlay,
            screenshot,
            origin: (b.x, b.y),
            monitors,
            windows: Vec::new(),
            options,
            width: b.w.max(1),
            height: b.h.max(1),
            selection: None,
            hover: None,
            interaction: Interaction::None,
            resize_edges: 0,
            press: (0, 0),
            last_creation: (0, 0),
            start_selection: R::default(),
            pointer: None,
            moving_during_creation: false,
            control_held_during_creation: false,
            suppress_right_release: false,
            opened: Instant::now(),
            shift: false,
            control: false,
            alt: false,
            textures: vec![None; n],
            magnifier: None,
        };
        // `LoadWindowRectanglesAsync`: the windows under the ruler, its own skipped.
        if me.options.detect_windows {
            let own: Vec<isize> = me.overlay.windows.iter().filter_map(|w| sharex_capture::platform::window_handle(&w.window.window)).collect();
            me.windows = sharex_capture::platform::window_list(me.options.detect_controls, &own)
                .into_iter()
                .map(|w| R::new(w.rect.x - b.x, w.rect.y - b.y, w.rect.w, w.rect.h))
                .filter(|r| r.w > 0 && r.h > 0)
                .collect();
        }
        if let Some((x, y)) = sharex_capture::platform::cursor_position() {
            me.pointer = Some(me.clamp_point((x - b.x, y - b.y)));
            me.update_hover();
        }
        for i in 0..n {
            me.paint(i);
        }
        me.overlay.show(sharex_capture::platform::cursor_position());
        Ok(me)
    }

    pub fn owns(&self, id: WindowId) -> bool {
        self.overlay.owns(id)
    }

    pub fn window(&self) -> &winit::window::Window {
        &self.overlay.windows[0].window.window
    }

    pub fn wake_at(&mut self, now: Instant) -> Option<Instant> {
        self.overlay.due(now)
    }

    fn clamp_point(&self, (x, y): (i32, i32)) -> (i32, i32) {
        (x.clamp(0, self.width - 1), y.clamp(0, self.height - 1))
    }

    fn clamp_rect(&self, r: R) -> R {
        let l = r.x.clamp(0, self.width - 1);
        let t = r.y.clamp(0, self.height - 1);
        let rr = r.right().clamp(l + 1, self.width);
        let b = r.bottom().clamp(t + 1, self.height);
        R::from_ltrb(l, t, rr, b)
    }

    /// `CreateRectangle(start, end, square)`.
    fn create_rectangle(&self, s: (i32, i32), e: (i32, i32), square: bool) -> R {
        let (mut dx, mut dy) = (e.0 - s.0, e.1 - s.1);
        if square {
            let aw = if dx < 0 { s.0 } else { self.width - 1 - s.0 };
            let ah = if dy < 0 { s.1 } else { self.height - 1 - s.1 };
            let side = dx.abs().max(dy.abs()).min(aw.min(ah));
            dx = if dx < 0 { -side } else { side };
            dy = if dy < 0 { -side } else { side };
        }
        R::new(s.0.min(s.0 + dx), s.1.min(s.1 + dy), dx.abs() + 1, dy.abs() + 1)
    }

    /// `Move`.
    fn move_rect(&self, r: R, dx: i32, dy: i32) -> R {
        let w = r.w.min(self.width);
        let h = r.h.min(self.height);
        R::new((r.x + dx).clamp(0, self.width - w), (r.y + dy).clamp(0, self.height - h), w, h)
    }

    /// `Resize`.
    fn resize_rect(&self, s: R, edges: u8, p: (i32, i32)) -> R {
        let (mut l, mut t, mut r, mut b) = (s.x, s.y, s.right() - 1, s.bottom() - 1);
        if edges & LEFT != 0 {
            l = p.0;
        }
        if edges & RIGHT != 0 {
            r = p.0;
        }
        if edges & TOP != 0 {
            t = p.1;
        }
        if edges & BOTTOM != 0 {
            b = p.1;
        }
        self.clamp_rect(self.create_rectangle((l, t), (r, b), false))
    }

    /// `UpdateHover`: the topmost window under the pointer while there's no rectangle.
    fn update_hover(&mut self) {
        self.hover = None;
        let Some(p) = self.pointer else { return };
        if self.selection.is_some() || !matches!(self.interaction, Interaction::None | Interaction::Pending) {
            return;
        }
        if let Some(w) = self.windows.iter().find(|w| contains(w, p)) {
            let l = w.x.max(0);
            let t = w.y.max(0);
            let r = w.right().min(self.width);
            let b = w.bottom().min(self.height);
            self.hover = (r > l && b > t).then(|| R::from_ltrb(l, t, r, b));
        }
    }

    fn handle_center(s: &R, e: u8) -> (f64, f64) {
        let x = if e & LEFT != 0 {
            s.x as f64
        } else if e & RIGHT != 0 {
            s.right() as f64
        } else {
            s.x as f64 + s.w as f64 / 2.0
        };
        let y = if e & TOP != 0 {
            s.y as f64
        } else if e & BOTTOM != 0 {
            s.bottom() as f64
        } else {
            s.y as f64 + s.h as f64 / 2.0
        };
        (x, y)
    }

    fn scale(&self) -> f64 {
        self.overlay.windows.first().map_or(1.0, |w| w.scale)
    }

    /// `HitTestHandle`.
    fn hit_handle(&self, p: (i32, i32)) -> u8 {
        let Some(s) = self.selection else { return 0 };
        if matches!(self.interaction, Interaction::Creating | Interaction::Pending) {
            return 0;
        }
        let radius = HANDLE_SIZE * self.scale() / 2.0;
        for e in HANDLE_EDGES {
            let (cx, cy) = Self::handle_center(&s, e);
            let (dx, dy) = (p.0 as f64 + 0.5 - cx, p.1 as f64 + 0.5 - cy);
            if dx * dx + dy * dy <= radius * radius {
                return e;
            }
        }
        0
    }

    fn update_cursor(&mut self, event_loop: &ActiveEventLoop, p: (i32, i32)) {
        let c = if self.hit_handle(p) != 0 { CustomCursor::OpenHand } else { CustomCursor::Crosshair };
        self.overlay.set_cursor(event_loop, c);
    }

    /// `UpdateSelectionDuringCreation`: Shift squares, Ctrl moves.
    fn update_creation(&mut self, p: (i32, i32)) {
        if !self.control {
            self.moving_during_creation = false;
        } else if let Some(s) = self.selection
            && (self.moving_during_creation || !self.control_held_during_creation)
        {
            let moved = self.move_rect(s, p.0 - self.last_creation.0, p.1 - self.last_creation.1);
            self.press = self.clamp_point((self.press.0 + moved.x - s.x, self.press.1 + moved.y - s.y));
            self.selection = Some(moved);
            self.moving_during_creation = true;
            self.control_held_during_creation = true;
            self.last_creation = p;
            return;
        }
        self.selection = Some(self.create_rectangle(self.press, p, self.shift));
        self.last_creation = p;
        self.control_held_during_creation = self.control;
    }

    /// `GetSelectionText`.
    fn selection_text(&self, s: &R) -> String {
        use sharex_strings::tools as t;
        let (x, y) = (self.origin.0 + s.x, self.origin.1 + s.y);
        let (right, bottom) = (x + s.w - 1, y + s.h - 1);
        let (dx, dy) = ((right - x) as f64, (bottom - y) as f64);
        let distance = (dx * dx + dy * dy).sqrt();
        let angle = dy.atan2(dx) * 180.0 / std::f64::consts::PI;
        let area = s.w as i64 * s.h as i64;
        let perimeter = 2 * (s.w as i64 + s.h as i64);
        let nl = sharex_tools::color::NEW_LINE;
        format!(
            "{}{nl}{}{nl}{}",
            format_dotnet(t::ClassicRulerOverlayControl_Position(), &[x.into(), y.into(), right.into(), bottom.into()]),
            format_dotnet(t::ClassicRulerOverlayControl_Size(), &[s.w.into(), s.h.into(), area.into(), perimeter.into()]),
            format_dotnet(t::ClassicRulerOverlayControl_Distance(), &[Arg::F64(distance), Arg::F64(angle)]),
        )
    }

    /// `GetPointerText`.
    fn pointer_text(&self, p: (i32, i32)) -> String {
        let pos = (self.origin.0 + p.0, self.origin.1 + p.1);
        if self.options.use_custom_info_text {
            let px = self.screenshot.get_pixel(p.0 as u32, p.1 as u32);
            let c = sharex_tools::color::Color::rgb(px[0], px[1], px[2]);
            if !self.options.custom_info_text.is_empty() {
                return sharex_tools::color::pixel_info_parse(&self.options.custom_info_text, &c, pos);
            }
            let nl = sharex_tools::color::NEW_LINE;
            return format!("RGB: {}, {}, {}{nl}Hex: {}{nl}X: {} Y: {}", c.r, c.g, c.b, sharex_tools::color::color_to_hex(&c, sharex_tools::color::ColorFormat::Rgb), pos.0, pos.1);
        }
        format!("X: {} Y: {}", pos.0, pos.1)
    }

    /// `GetMeasurementText`: the rectangle's, else the pointer's.
    fn measurement_text(&self) -> String {
        if let Some(r) = self.selection.or(self.hover) {
            return self.selection_text(&r);
        }
        self.pointer.map(|p| self.pointer_text(p)).unwrap_or_default()
    }

    /// `HandleKeyDown`.
    fn key_down(&mut self, code: Option<KeyCode>, named: Option<NamedKey>) -> bool {
        let speed = if self.shift { 10 } else { 1 };
        let (dx, dy) = match named {
            Some(NamedKey::ArrowLeft) => (-speed, 0),
            Some(NamedKey::ArrowRight) => (speed, 0),
            Some(NamedKey::ArrowUp) => (0, -speed),
            Some(NamedKey::ArrowDown) => (0, speed),
            _ => (0, 0),
        };
        if dx != 0 || dy != 0 {
            match self.selection {
                Some(s) if self.interaction == Interaction::None => {
                    if self.control {
                        self.selection = Some(self.clamp_rect(R::new(s.x, s.y, (s.w + dx).max(1), (s.h + dy).max(1))));
                    } else if self.alt {
                        let l = (s.x + dx).min(s.right() - 1);
                        let t = (s.y + dy).min(s.bottom() - 1);
                        self.selection = Some(self.clamp_rect(R::from_ltrb(l, t, s.right(), s.bottom())));
                    } else {
                        self.selection = Some(self.move_rect(s, dx, dy));
                    }
                }
                _ => {
                    if let Some((x, y)) = sharex_capture::platform::cursor_position() {
                        sharex_capture::platform::set_cursor_position(x + dx, y + dy);
                    }
                }
            }
            return true;
        }
        if code == Some(KeyCode::Delete) && self.interaction == Interaction::None && self.selection.is_some() {
            self.selection = None;
            self.update_hover();
            return true;
        }
        false
    }

    pub fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) -> Outcome {
        let Some(i) = self.overlay.index(id) else { return Outcome::Continue };
        let b = self.overlay.windows[i].bounds;
        let to_pixel = |p: winit::dpi::PhysicalPosition<f64>| ((b.x - self.origin.0) + p.x.round() as i32, (b.y - self.origin.1) + p.y.round() as i32);
        match event {
            WindowEvent::RedrawRequested => {
                self.paint(i);
                return Outcome::Continue;
            }
            WindowEvent::CloseRequested => return Outcome::Close,
            WindowEvent::ModifiersChanged(m) => {
                let s = m.state();
                self.shift = s.shift_key();
                self.control = s.control_key();
                self.alt = s.alt_key();
                return Outcome::Continue;
            }
            WindowEvent::CursorMoved { position, .. } => {
                let p = self.clamp_point(to_pixel(position));
                if self.pointer == Some(p) && self.interaction == Interaction::None {
                    return Outcome::Continue;
                }
                self.pointer = Some(p);
                match self.interaction {
                    Interaction::Pending => {
                        if (p.0 - self.press.0).abs() >= DRAG_THRESHOLD || (p.1 - self.press.1).abs() >= DRAG_THRESHOLD {
                            self.interaction = Interaction::Creating;
                            self.hover = None;
                            self.update_creation(p);
                        }
                    }
                    Interaction::Creating => self.update_creation(p),
                    Interaction::Moving => self.selection = Some(self.move_rect(self.start_selection, p.0 - self.press.0, p.1 - self.press.1)),
                    Interaction::Resizing => self.selection = Some(self.resize_rect(self.start_selection, self.resize_edges, p)),
                    Interaction::None => {
                        self.update_hover();
                        self.update_cursor(event_loop, p);
                    }
                }
            }
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Right, .. } => {
                if matches!(self.interaction, Interaction::Pending | Interaction::Creating) {
                    // A right click while drawing cancels the new rectangle.
                    self.selection = None;
                    self.interaction = Interaction::None;
                    self.moving_during_creation = false;
                    self.overlay.set_cursor(event_loop, CustomCursor::Crosshair);
                    self.suppress_right_release = true;
                }
            }
            WindowEvent::MouseInput { state: ElementState::Released, button: MouseButton::Right, .. } => {
                let p = self.pointer.unwrap_or_default();
                if self.suppress_right_release {
                    self.suppress_right_release = false;
                } else if self.selection.is_some_and(|s| contains(&s, p)) {
                    self.selection = None;
                    self.update_hover();
                    self.update_cursor(event_loop, p);
                } else {
                    return Outcome::Close;
                }
            }
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. } => {
                let Some(p) = self.pointer else { return Outcome::Continue };
                let edges = self.hit_handle(p);
                if let (Some(s), true) = (self.selection, edges != 0) {
                    self.interaction = Interaction::Resizing;
                    self.resize_edges = edges;
                    self.start_selection = s;
                } else if let Some(s) = self.selection.filter(|s| contains(s, p)) {
                    self.interaction = Interaction::Moving;
                    self.press = p;
                    self.start_selection = s;
                } else {
                    self.interaction = Interaction::Pending;
                    self.selection = None;
                    self.press = p;
                    self.last_creation = p;
                    self.moving_during_creation = false;
                    self.control_held_during_creation = self.control;
                }
                let c = if matches!(self.interaction, Interaction::Moving | Interaction::Resizing) { CustomCursor::ClosedHand } else { CustomCursor::Crosshair };
                self.overlay.set_cursor(event_loop, c);
            }
            WindowEvent::MouseInput { state: ElementState::Released, button: MouseButton::Left, .. } if self.interaction != Interaction::None => {
                if self.interaction == Interaction::Pending {
                    // A click picks the hovered window.
                    self.selection = self.hover;
                }
                self.interaction = Interaction::None;
                self.moving_during_creation = false;
                self.update_hover();
                let p = self.pointer.unwrap_or_default();
                self.overlay.set_cursor(event_loop, CustomCursor::Crosshair);
                self.update_cursor(event_loop, p);
            }
            WindowEvent::KeyboardInput { event: key, .. } => {
                // The key up of the hotkey that opened it isn't taken.
                if self.opened.elapsed() < INPUT_DELAY {
                    return Outcome::Continue;
                }
                let code = match key.physical_key {
                    PhysicalKey::Code(c) => Some(c),
                    _ => None,
                };
                let named = match &key.logical_key {
                    Key::Named(n) => Some(*n),
                    _ => None,
                };
                match key.state {
                    ElementState::Pressed => {
                        if code == Some(KeyCode::KeyC) && self.control && !self.shift && !self.alt {
                            return Outcome::Copy(self.measurement_text());
                        }
                        if !self.key_down(code, named) {
                            return Outcome::Continue;
                        }
                    }
                    // `OnKeyUp`: Escape closes on its release.
                    ElementState::Released if named == Some(NamedKey::Escape) && !self.shift && !self.control && !self.alt => return Outcome::Close,
                    ElementState::Released => return Outcome::Continue,
                }
            }
            other => {
                self.overlay.windows[i].window.on_event(&other);
                return Outcome::Continue;
            }
        }
        self.overlay.redraw();
        Outcome::Continue
    }

    /// `GetMonitorArea`.
    fn monitor_area(&self, p: (i32, i32)) -> R {
        self.monitors.iter().copied().find(|m| contains(m, p)).unwrap_or(R::new(0, 0, self.width, self.height))
    }

    fn magnifier_pixel_count(&self) -> i32 {
        let c = self.options.magnifier_pixel_count.clamp(3, 35);
        if c & 1 == 0 { c - 1 } else { c }
    }

    fn paint(&mut self, i: usize) {
        let w = &self.overlay.windows[i];
        let ppp = w.scale as f32;
        let m = Mapper { origin: ((w.bounds.x - self.origin.0) as f64, (w.bounds.y - self.origin.1) as f64), ppp };
        let monitor = R::new(w.bounds.x - self.origin.0, w.bounds.y - self.origin.1, w.bounds.w, w.bounds.h);
        if self.textures[i].is_none() {
            let crop = image::imageops::crop_imm(&self.screenshot, monitor.x as u32, monitor.y as u32, monitor.w as u32, monitor.h as u32).to_image();
            let ci = egui::ColorImage::from_rgba_unmultiplied([crop.width() as usize, crop.height() as usize], crop.as_raw());
            self.textures[i] = Some(w.window.ctx().load_texture("classic ruler screen", ci, TextureOptions::NEAREST));
        }
        // The magnifier picture for the pointer, made once per position.
        let hud_scale = w.scale;
        if let Some(p) = self.pointer
            && self.options.show_magnifier
            && self.magnifier.as_ref().is_none_or(|(at, _)| *at != p)
        {
            let count = self.magnifier_pixel_count();
            let cell = (((self.options.magnifier_size.clamp(50, 500) as f64 * hud_scale).round() as i32) / count).max(1);
            let accent = palette(w.window.ctx()).accent_start;
            let img = render_magnifier(&self.screenshot, p, count, cell, (hud_scale.round().max(1.0)) as i32, self.options.use_square_magnifier, accent);
            self.magnifier = Some((p, img));
        }
        let tex = self.textures[i].clone();
        let selection = self.selection.filter(|s| s.w > 0 && s.h > 0);
        let active = selection.or(self.hover);
        let dim_clear = selection.or(if self.options.dim_all_except_selection { None } else { self.hover });
        let dim = (self.options.background_dim_strength.clamp(0, 100) as f64 / 100.0 * 255.0).round() as u8;
        let creating = matches!(self.interaction, Interaction::Creating | Interaction::Pending);
        let selection_card = if self.options.show_info { active.map(|r| (r, self.selection_text(&r))) } else { None };
        let pointer = self.pointer;
        let pointer_text = pointer.filter(|_| self.options.show_info).map(|p| self.pointer_text(p));
        let options = self.options.clone();
        let magnifier = self.magnifier.clone();
        let count = self.magnifier_pixel_count();
        let (width, height) = (self.width, self.height);
        let area_of = |p: (i32, i32)| self.monitor_area(p);
        let sel_area = selection_card.as_ref().map(|(r, _)| (area_of((r.x, r.y.max(r.bottom() - 1))), area_of((r.x, r.y))));
        let ptr_area = pointer.map(area_of);
        self.overlay.windows[i].window.paint(|ui| {
            let painter = ui.painter().clone();
            let p = palette(ui.ctx());
            let accent = p.accent_start;
            let halo = hud::halo(accent);
            if let Some(t) = &tex {
                let r = m.rect(monitor.x as f64, monitor.y as f64, monitor.w as f64, monitor.h as f64);
                painter.image(t.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            }
            // `DrawDim`.
            if dim > 0 {
                let c = Color32::from_rgba_unmultiplied(0, 0, 0, dim);
                let (w, h) = (width as f64, height as f64);
                match dim_clear {
                    None => {
                        painter.rect_filled(m.rect(0.0, 0.0, w, h), 0.0, c);
                    }
                    Some(s) => {
                        let (l, t, r, b) = (s.x as f64, s.y as f64, s.right() as f64, s.bottom() as f64);
                        for (x, y, rw, rh) in [(0.0, 0.0, w, t), (0.0, b, w, h - b), (0.0, t, l, b - t), (r, t, w - r, b - t)] {
                            if rw > 0.0 && rh > 0.0 {
                                painter.rect_filled(m.rect(x, y, rw, rh), 0.0, c);
                            }
                        }
                    }
                }
            }
            if let Some(r) = active {
                let rf = RectF::new(r.x as f64, r.y as f64, r.w as f64, r.h as f64);
                hud::ant_rectangle(&painter, &m, rf, accent, halo);
                if let Some(s) = selection {
                    ticks(&painter, &m, &s, 5, 10, accent);
                    ticks(&painter, &m, &s, 15, 100, accent);
                    if options.show_center_crosshair {
                        let (cx, cy) = ((s.x + s.w / 2) as f64, (s.y + s.h / 2) as f64);
                        hud::pixel_cross(&painter, &m, halo, cx - 1.0, cy - 1.0);
                        hud::pixel_cross(&painter, &m, accent, cx, cy);
                    }
                    if !creating {
                        for e in HANDLE_EDGES {
                            hud::paint_node(&painter, &m, ClassicRuler::handle_center(&s, e));
                        }
                    }
                }
            }
            if let (Some((r, text)), Some((below_area, above_area))) = (&selection_card, sel_area) {
                // `DrawSelectionCard`: below the rectangle, else above it.
                let size = card_size(ui, text);
                let mut area = below_area;
                let mut y = r.bottom() as f32 + 8.0;
                if y + size.y > area.bottom() as f32 {
                    area = above_area;
                    y = r.y as f32 - size.y;
                }
                let x = (r.x as f32).clamp(area.x as f32, (area.right() as f32 - size.x).max(area.x as f32));
                let y = y.clamp(area.y as f32, (area.bottom() as f32 - size.y).max(area.y as f32));
                draw_card(ui, &m, text, (x.round() as f64, y.round() as f64));
            }
            if let (Some(pt), Some(area)) = (pointer, ptr_area) {
                if options.show_screen_crosshair {
                    screen_crosshair(&painter, &m, pt, width, height, accent, halo);
                }
                if options.show_magnifier || options.show_info {
                    // `DrawPointerPanel`.
                    let cell = (((options.magnifier_size.clamp(50, 500) as f64 * hud_scale).round() as i32) / count).max(1);
                    let ring = hud_scale.round().max(1.0);
                    let outer = if options.show_magnifier { (cell * count) as f64 + 2.0 * ring } else { 0.0 };
                    let info = pointer_text.as_ref().map(|t| (t, card_size(ui, t)));
                    let gap = if outer > 0.0 && info.is_some() { 10.0 * hud_scale } else { 0.0 };
                    let iw = info.as_ref().map_or(0.0, |(_, s)| s.x as f64);
                    let ih = info.as_ref().map_or(0.0, |(_, s)| s.y as f64);
                    let width_p = outer.max(iw);
                    let height_p = outer + gap + ih;
                    let off = 18.0 * hud_scale;
                    let mut x = pt.0 as f64 + off;
                    let mut y = pt.1 as f64 + off;
                    if x + width_p > area.right() as f64 {
                        x = pt.0 as f64 - width_p - off;
                    }
                    if y + height_p > area.bottom() as f64 {
                        y = pt.1 as f64 - height_p - off;
                    }
                    let x = x.clamp(area.x as f64, (area.right() as f64 - width_p).max(area.x as f64)).round();
                    let y = y.clamp(area.y as f64, (area.bottom() as f64 - height_p).max(area.y as f64)).round();
                    if outer > 0.0
                        && let Some((_, img)) = &magnifier
                    {
                        let lx = x + ((width_p - outer) / 2.0).round();
                        let ci = egui::ColorImage::from_rgba_unmultiplied([img.width() as usize, img.height() as usize], img.as_raw());
                        let t = ui.ctx().load_texture("classic ruler magnifier", ci, TextureOptions::NEAREST);
                        painter.image(t.id(), m.rect(lx, y, img.width() as f64, img.height() as f64), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                    }
                    if let Some((t, s)) = &info {
                        let cx = (x + (width_p - s.x as f64) / 2.0).round();
                        draw_card(ui, &m, t, (cx, (y + outer + gap).round()));
                    }
                }
            }
        });
    }
}

/// `DrawTicks`: `size`-long marks every `step` pixels inside each edge.
fn ticks(painter: &egui::Painter, m: &Mapper, s: &R, size: i32, step: i32, accent: Color32) {
    if s.w < size || s.h < size {
        return;
    }
    let fill = |x: i32, y: i32, w: i32, h: i32| {
        let mut mesh = egui::Mesh::default();
        mesh.add_colored_rect(m.rect(x as f64, y as f64, w as f64, h as f64), accent);
        painter.add(mesh);
    };
    let mut i = 1;
    while i * step < s.w {
        let x = s.x + i * step;
        fill(x, s.y, 1, size);
        fill(x, s.bottom() - size, 1, size);
        i += 1;
    }
    let mut i = 1;
    while i * step < s.h {
        let y = s.y + i * step;
        fill(s.x, y, size, 1);
        fill(s.right() - size, y, size, 1);
        i += 1;
    }
}

/// `DrawScreenCrosshair`: dashed lines through the pointer, 5px clear around it.
fn screen_crosshair(painter: &egui::Painter, m: &Mapper, p: (i32, i32), w: i32, h: i32, accent: Color32, halo: Color32) {
    let (x, y) = (p.0 as f32 + 0.5, p.1 as f32 + 0.5);
    let pt = |a: f32, b: f32| m.pos(a as f64, b as f64);
    let halo = Color32::from_rgba_unmultiplied(halo.r(), halo.g(), halo.b(), 230);
    for (a, b) in [((0.0, y), (x - 5.0, y)), ((x + 5.0, y), (w as f32, y)), ((x, 0.0), (x, y - 5.0)), ((x, y + 5.0), (x, h as f32))] {
        if b.0 <= a.0 && b.1 <= a.1 {
            continue;
        }
        painter.line_segment([pt(a.0, a.1), pt(b.0, b.1)], Stroke::new(1.0, accent));
        painter.add(egui::Shape::dashed_line(&[pt(a.0, a.1), pt(b.0, b.1)], Stroke::new(1.0, halo), 5.0, 5.0));
    }
}

/// The cards' font: 12px SemiBold (`ShareX.FontFamily.Default`).
fn card_font() -> egui::FontId {
    widgets::font(12.0, 600)
}

/// `GetCardSize`: the text plus a 1px border and 7x4 padding, rounded up.
fn card_size(ui: &egui::Ui, text: &str) -> egui::Vec2 {
    let f = card_font();
    let lines: Vec<&str> = text.lines().collect();
    let w = lines.iter().map(|l| ui.fonts_mut(|fo| fo.layout_no_wrap(l.to_string(), f.clone(), Color32::WHITE).size().x)).fold(0.0, f32::max);
    let h = ui.fonts_mut(|fo| fo.row_height(&f)) * lines.len().max(1) as f32;
    vec2((w + 16.0).ceil(), (h + 10.0).ceil())
}

/// `DrawCard`: main background, border, 5px corners, the text in its colour.
fn draw_card(ui: &egui::Ui, m: &Mapper, text: &str, at: (f64, f64)) {
    let p = palette(ui.ctx());
    let size = card_size(ui, text);
    let r = Rect::from_min_size(m.pos(at.0, at.1), size);
    ui.painter().rect_filled(r, CornerRadius::same(5), p.background_main);
    ui.painter().rect_stroke(r, CornerRadius::same(5), Stroke::new(1.0, p.border), StrokeKind::Inside);
    let f = card_font();
    let lh = ui.fonts_mut(|fo| fo.row_height(&f));
    for (k, line) in text.lines().enumerate() {
        let g = ui.painter().layout_no_wrap(line.to_string(), f.clone(), p.text);
        ui.painter().galley(r.min + vec2(8.0, 5.0 + k as f32 * lh), g, p.text);
    }
}

/// `DrawMagnifier` as one picture: the white ring, the pixels around `p`
/// at `cell` each, the accent-tinted centre row and column, the grid, the
/// centre outline and the black inner ring; a circle unless `square`.
fn render_magnifier(shot: &RgbaImage, p: (i32, i32), count: i32, cell: i32, ring: i32, square: bool, accent: Color32) -> RgbaImage {
    let size = count * cell;
    let outer = size + 2 * ring;
    let radius = count / 2;
    let (sx, sy) = (p.0 - radius, p.1 - radius);
    let (iw, ih) = (shot.width() as i32, shot.height() as i32);
    let center = radius * cell;
    let far = center + cell;
    let mut out = RgbaImage::new(outer as u32, outer as u32);
    let blend = |dst: [f32; 3], c: [u8; 3], a: f32| -> [f32; 3] { [0, 1, 2].map(|k| c[k] as f32 / 255.0 * a + dst[k] * (1.0 - a)) };
    let coverage = |x: i32, y: i32, cx: f64, r: f64| -> f32 {
        let mut n = 0;
        for j in 0..4 {
            for i in 0..4 {
                let dx = x as f64 + (i as f64 + 0.5) / 4.0 - cx;
                let dy = y as f64 + (j as f64 + 0.5) / 4.0 - cx;
                if dx * dx + dy * dy <= r * r {
                    n += 1;
                }
            }
        }
        n as f32 / 16.0
    };
    let c = outer as f64 / 2.0;
    for y in 0..outer {
        for x in 0..outer {
            let (bx, by) = (x - ring, y - ring);
            let in_content = bx >= 0 && by >= 0 && bx < size && by < size;
            // The content (black where off screen) with its overlays.
            let mut px = [0.0f32; 3];
            if in_content {
                let (ix, iy) = (sx + bx / cell, sy + by / cell);
                if ix >= 0 && iy >= 0 && ix < iw && iy < ih {
                    let s = shot.get_pixel(ix as u32, iy as u32);
                    px = [s[0] as f32 / 255.0, s[1] as f32 / 255.0, s[2] as f32 / 255.0];
                }
                let acc = [accent.r(), accent.g(), accent.b()];
                let in_rect = |x0: i32, y0: i32, w: i32, h: i32| bx >= x0 && by >= y0 && bx < x0 + w && by < y0 + h;
                if in_rect(0, center, center, cell - 1) || in_rect(far, center, size - far, cell - 1) || in_rect(center, 0, cell - 1, center) || in_rect(center, far, cell - 1, size - far) {
                    px = blend(px, acc, 75.0 / 255.0);
                }
                for k in 1..count {
                    let line = k * cell - 1;
                    if bx == line {
                        px = blend(px, [0, 0, 0], 75.0 / 255.0);
                    }
                    if by == line {
                        px = blend(px, [0, 0, 0], 75.0 / 255.0);
                    }
                }
                let outline = |x0: i32, y0: i32, w: i32, h: i32| w >= 2 && h >= 2 && in_rect(x0, y0, w, h) && !(bx > x0 && bx < x0 + w - 1 && by > y0 && by < y0 + h - 1);
                if outline(center - 1, center - 1, cell + 1, cell + 1) {
                    px = [0.0; 3];
                }
                if outline(center, center, cell - 1, cell - 1) {
                    px = [1.0; 3];
                }
            }
            let (rgb, a) = if square {
                let mut v = [1.0f32; 3];
                if in_content {
                    v = px;
                    let inner = bx >= ring && by >= ring && bx < size - ring && by < size - ring;
                    if !inner {
                        v = [0.0; 3];
                    }
                }
                (v, 1.0)
            } else {
                let white = coverage(x, y, c, c);
                let content = coverage(x, y, c, size as f64 / 2.0);
                let ring_in = coverage(x, y, c, size as f64 / 2.0 - ring as f64);
                let mut v = [1.0f32; 3];
                v = [0, 1, 2].map(|k| px[k] * content + v[k] * (1.0 - content));
                let black = (content - ring_in).max(0.0);
                v = [0, 1, 2].map(|k| v[k] * (1.0 - black));
                (v, white)
            };
            let to = |v: f32| (v * 255.0).round().clamp(0.0, 255.0) as u8;
            out.put_pixel(x as u32, y as u32, image::Rgba([to(rgb[0]), to(rgb[1]), to(rgb[2]), (a * 255.0).round() as u8]));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_text_matches_sharex() {
        // ShareX's Ctrl+C after drawing 200,150 to 500,400.
        let s = R::new(200, 150, 301, 251);
        let (x, y) = (s.x, s.y);
        let (right, bottom) = (x + s.w - 1, y + s.h - 1);
        assert_eq!((right, bottom), (500, 400));
        let d = (((right - x) as f64).powi(2) + ((bottom - y) as f64).powi(2)).sqrt();
        assert_eq!(sharex_tools::text::format_number(d, "0.00", false), "390.51");
        let a = ((bottom - y) as f64).atan2((right - x) as f64).to_degrees();
        assert_eq!(sharex_tools::text::format_number(a, "0.00", false), "39.81");
    }
}
