//! Tools › Measure: `ShareX.Tools/Tools/Ruler/RulerWindow` and
//! `Controls/RulerOverlayControl`. A transparent overlay over every monitor
//! (not topmost, as ShareX's); the screen is captured once when it opens.
//! Hovering measures the run of similar colour through the pointer
//! (horizontal or vertical: H, V, Space or the middle button), a click keeps
//! it, a drag keeps the content bounds of the dragged area; the wheel sets
//! the colour tolerance, arrows nudge the last measurement, Delete clears,
//! Ctrl+C copies the last one, a right click removes the one under the
//! pointer (or closes), Escape closes.

use std::time::{Duration, Instant};

use egui::{Color32, Pos2, Rect, pos2, vec2};
use sharex_tools::ruler::{self as engine, Rect as PxRect, ScreenPixelBuffer};
use sharex_ui::theme::palette;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, KeyCode, NamedKey, PhysicalKey};
use winit::window::{WindowId, WindowLevel};

use super::overlay::Overlay;

const DRAG_THRESHOLD: i32 = 4;
const HIT_PADDING: f32 = 6.0;
const MINIMUM_LINE_LENGTH: i32 = 2;
const END_CAP_SIZE: f32 = 10.0;
const RECTANGLE_CROSS_SIZE: f32 = 20.0;
const LABEL_FONT_SIZE: f32 = 13.0;
const LABEL_PAD_X: f32 = 10.0;
const LABEL_PAD_Y: f32 = 5.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Axis {
    Horizontal,
    Vertical,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Line,
    Rectangle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Measurement {
    kind: Kind,
    bounds: PxRect,
    axis: Axis,
    sample: (i32, i32),
    source: PxRect,
}

impl Measurement {
    /// `GetMeasurementText`.
    fn text(&self) -> String {
        match (self.kind, self.axis) {
            (Kind::Rectangle, _) => format!("{} × {} px", self.bounds.w, self.bounds.h),
            (_, Axis::Horizontal) => format!("{} px", self.bounds.w),
            _ => format!("{} px", self.bounds.h),
        }
    }
}

pub enum Outcome {
    Continue,
    Close,
    /// Ctrl+C: the last measurement's text.
    Copy(String),
}

pub struct Ruler {
    overlay: Overlay,
    screen: ScreenPixelBuffer,
    measurements: Vec<Measurement>,
    axis: Axis,
    tolerance: i32,
    pointer: Option<(i32, i32)>,
    press: (i32, i32),
    drag: PxRect,
    hover: Option<Measurement>,
    left_pressed: bool,
    dragging: bool,
    status_until: Option<Instant>,
    shift: bool,
    control: bool,
}

impl Ruler {
    /// `ConfigureOverlayAndCaptureScreen`: the desktop captured, then the overlay.
    pub fn open(event_loop: &ActiveEventLoop, gpu: &sharex_ui::gpu::Gpu, theme: egui::Theme) -> Result<Self, String> {
        let screens = sharex_capture::screenshot::screens();
        let b = super::overlay::desktop_bounds(&screens);
        let shot = sharex_capture::platform::capture_rectangle(b).ok_or("couldn't capture the screen")?;
        let screen = ScreenPixelBuffer::from_rgba(PxRect::new(b.x, b.y, b.w, b.h), shot.as_raw());
        let title = crate::rs_title(sharex_strings::tools::RulerWindow_ShareX_Ruler());
        let mut overlay = Overlay::open(event_loop, gpu, &screens, &title, true, theme)?;
        // Not `Topmost` (ShareX's `RulerWindow` isn't).
        for w in &overlay.windows {
            w.window.window.set_window_level(WindowLevel::Normal);
        }
        // `new Cursor(StandardCursorType.Cross)`.
        overlay.set_cursor_icon(winit::window::CursorIcon::Crosshair);
        let mut me = Self {
            overlay,
            screen,
            measurements: Vec::new(),
            axis: Axis::Horizontal,
            tolerance: 0,
            pointer: None,
            press: (0, 0),
            drag: PxRect::default(),
            hover: None,
            left_pressed: false,
            dragging: false,
            status_until: None,
            shift: false,
            control: false,
        };
        for i in 0..me.overlay.windows.len() {
            me.paint(i);
        }
        me.overlay.show(sharex_capture::platform::cursor_position());
        Ok(me)
    }

    pub fn owns(&self, id: WindowId) -> bool {
        self.overlay.owns(id)
    }

    /// A window of the overlay (to own the clipboard).
    pub fn window(&self) -> &winit::window::Window {
        &self.overlay.windows[0].window.window
    }

    pub fn wake_at(&mut self, now: Instant) -> Option<Instant> {
        if let Some(t) = self.status_until {
            if t <= now {
                self.status_until = None;
                self.overlay.redraw();
            } else {
                return Some(t);
            }
        }
        self.overlay.due(now)
    }

    fn color_tolerance(&self) -> i32 {
        engine::color_tolerance(self.tolerance)
    }

    /// `CreateLineMeasurement`.
    fn line(&self, p: (i32, i32), axis: Option<Axis>) -> Option<Measurement> {
        if !self.screen.bounds.contains(p.0, p.1) {
            return None;
        }
        let axis = axis.unwrap_or(self.axis);
        let bounds = self.screen.find_color_run(p, axis == Axis::Horizontal, self.color_tolerance());
        let len = if axis == Axis::Horizontal { bounds.w } else { bounds.h };
        (len >= MINIMUM_LINE_LENGTH).then_some(Measurement { kind: Kind::Line, bounds, axis, sample: p, source: PxRect::default() })
    }

    fn set_axis(&mut self, axis: Axis, at: Option<(i32, i32)>) {
        self.axis = axis;
        if let Some(p) = at.or(self.pointer) {
            self.hover = self.line(p, None);
        }
    }

    fn toggle_axis(&mut self, at: Option<(i32, i32)>) {
        let a = if self.axis == Axis::Horizontal { Axis::Vertical } else { Axis::Horizontal };
        self.set_axis(a, at);
    }

    /// `Nudge`.
    fn nudge(&mut self, dx: i32, dy: i32) {
        let tol = self.color_tolerance();
        let Some(m) = self.measurements.last().copied() else { return };
        let n = self.measurements.len() - 1;
        if m.kind == Kind::Line {
            let p = self.screen.clamp_point((m.sample.0 + dx, m.sample.1 + dy));
            if let Some(l) = self.line(p, Some(m.axis)) {
                self.measurements[n] = l;
            }
        } else {
            let mut src = m.source;
            src.offset(dx, dy);
            let src = self.screen.clamp_preserving_size(src);
            let snapped = self.screen.find_content_bounds(src, tol);
            self.measurements[n] = Measurement { bounds: snapped, source: src, ..m };
        }
    }

    pub fn window_event(&mut self, id: WindowId, event: WindowEvent) -> Outcome {
        let Some(i) = self.overlay.index(id) else { return Outcome::Continue };
        let mut changed = true;
        match event {
            WindowEvent::RedrawRequested => {
                self.paint(i);
                return Outcome::Continue;
            }
            WindowEvent::CloseRequested => return Outcome::Close,
            WindowEvent::ModifiersChanged(m) => {
                self.shift = m.state().shift_key();
                self.control = m.state().control_key();
                changed = false;
            }
            WindowEvent::CursorMoved { position, .. } => {
                let p = self.overlay.desktop_point(i, position);
                let moved = self.pointer != Some(p);
                self.pointer = Some(p);
                if self.left_pressed {
                    let (dx, dy) = ((p.0 - self.press.0).abs(), (p.1 - self.press.1).abs());
                    self.dragging |= dx >= DRAG_THRESHOLD || dy >= DRAG_THRESHOLD;
                    if self.dragging {
                        self.drag = self.screen.clamp(engine::create_rectangle(self.press, p));
                    }
                } else if moved {
                    self.hover = self.line(p, None);
                } else {
                    changed = false;
                }
            }
            WindowEvent::MouseInput { state: ElementState::Pressed, button, .. } => {
                let Some(p) = self.pointer else { return Outcome::Continue };
                match button {
                    MouseButton::Middle => self.toggle_axis(Some(p)),
                    MouseButton::Left => {
                        self.left_pressed = true;
                        self.dragging = false;
                        self.press = p;
                        self.drag = PxRect::default();
                    }
                    _ => changed = false,
                }
            }
            WindowEvent::MouseInput { state: ElementState::Released, button: MouseButton::Right, .. } => {
                // A measurement under the pointer goes; otherwise the window closes.
                let hit = self.pointer.and_then(|p| self.hit_test(i, p));
                match hit {
                    Some(k) => {
                        self.measurements.remove(k);
                    }
                    None => return Outcome::Close,
                }
            }
            WindowEvent::MouseInput { state: ElementState::Released, button: MouseButton::Left, .. } if self.left_pressed => {
                let p = self.pointer.unwrap_or(self.press);
                if self.dragging {
                    let src = self.screen.clamp(engine::create_rectangle(self.press, p));
                    let snapped = self.screen.find_content_bounds(src, self.color_tolerance());
                    if !snapped.is_empty() {
                        self.measurements.push(Measurement { kind: Kind::Rectangle, bounds: snapped, axis: self.axis, sample: p, source: src });
                    }
                } else if let Some(l) = self.line(p, None) {
                    self.measurements.push(l);
                }
                self.hover = None;
                self.drag = PxRect::default();
                self.left_pressed = false;
                self.dragging = false;
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let y = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y as f64,
                    MouseScrollDelta::PixelDelta(p) => p.y,
                };
                if y == 0.0 {
                    return Outcome::Continue;
                }
                if let Some(p) = self.pointer {
                    self.hover = self.line(p, None);
                }
                self.tolerance = (self.tolerance + y.signum() as i32).clamp(0, 100);
                // `RecalculateHoverMeasurement`.
                if let Some(h) = self.hover {
                    self.hover = self.line(h.sample, None);
                } else if let Some(p) = self.pointer {
                    self.hover = self.line(p, None);
                }
                self.status_until = Some(Instant::now() + Duration::from_millis(1400));
            }
            WindowEvent::KeyboardInput { event: key, .. } if key.state == ElementState::Pressed => {
                let amount = if self.shift { 10 } else { 1 };
                match (&key.logical_key, key.physical_key) {
                    (Key::Named(NamedKey::ArrowLeft), _) => self.nudge(-amount, 0),
                    (Key::Named(NamedKey::ArrowRight), _) => self.nudge(amount, 0),
                    (Key::Named(NamedKey::ArrowUp), _) => self.nudge(0, -amount),
                    (Key::Named(NamedKey::ArrowDown), _) => self.nudge(0, amount),
                    (Key::Named(NamedKey::Space), _) => self.toggle_axis(None),
                    (Key::Named(NamedKey::Delete), _) => {
                        self.measurements.clear();
                        if let Some(p) = self.pointer {
                            self.hover = self.line(p, None);
                        }
                    }
                    (Key::Named(NamedKey::Escape), _) => return Outcome::Close,
                    (_, PhysicalKey::Code(KeyCode::KeyH)) => self.set_axis(Axis::Horizontal, None),
                    (_, PhysicalKey::Code(KeyCode::KeyV)) => self.set_axis(Axis::Vertical, None),
                    (_, PhysicalKey::Code(KeyCode::KeyC)) if self.control => {
                        // `MeasurementText`: the last measurement's.
                        if let Some(m) = self.measurements.last() {
                            return Outcome::Copy(m.text());
                        }
                        changed = false;
                    }
                    _ => changed = false,
                }
            }
            other => {
                self.overlay.windows[i].window.on_event(&other);
                changed = false;
            }
        }
        if changed {
            self.overlay.redraw();
        }
        Outcome::Continue
    }

    /// Converts desktop pixels to window `i`'s points.
    fn mapper(&self, i: usize) -> impl Fn(f32, f32) -> Pos2 + use<> {
        let w = &self.overlay.windows[i];
        let (ox, oy, s) = (w.bounds.x as f32, w.bounds.y as f32, w.scale as f32);
        move |x, y| pos2((x - ox) / s, (y - oy) / s)
    }

    /// `HitTestMeasurement`: the last measurement whose shape and label
    /// (6px around) contain `p`.
    fn hit_test(&self, i: usize, p: (i32, i32)) -> Option<usize> {
        let ctx = self.overlay.windows[i].window.ctx().clone();
        let m = self.mapper(i);
        let pt = m(p.0 as f32, p.1 as f32);
        let area = self.area(i);
        (0..self.measurements.len()).rev().find(|&k| {
            let me = &self.measurements[k];
            let shape = match me.kind {
                Kind::Rectangle => rect_of(&m, me.bounds),
                Kind::Line => {
                    let (a, b) = endpoints(&m, me);
                    Rect::from_two_pos(a, b)
                }
            };
            let label = label_bounds(&ctx, &m, me, area);
            shape.union(label).expand(HIT_PADDING).contains(pt)
        })
    }

    /// The overlay's area (`Bounds`) in window `i`'s points.
    fn area(&self, i: usize) -> Rect {
        let w = &self.overlay.windows[i];
        Rect::from_min_size(Pos2::ZERO, vec2(w.bounds.w as f32, w.bounds.h as f32) / w.scale as f32)
    }

    fn paint(&mut self, i: usize) {
        let m = self.mapper(i);
        let area = self.area(i);
        let measurements = self.measurements.clone();
        let hover = if self.dragging && !self.drag.is_empty() { None } else { self.hover };
        let drag = (self.dragging && !self.drag.is_empty()).then_some(self.drag);
        let status = self.status_until.map(|_| sharex_strings::tools::RulerOverlayControl_Tolerance().replace("{0}", &self.tolerance.to_string()));
        let pointer = self.pointer;
        self.overlay.windows[i].window.paint(|ui| {
            let painter = ui.painter().clone();
            // `HitTestBrush`: black at alpha 1, so the window takes the pointer.
            painter.rect_filled(area, 0.0, Color32::from_black_alpha(1));
            let p = palette(ui.ctx());
            let accent = p.accent_start;
            let shadow = {
                let h = super::halo(accent);
                Color32::from_rgba_unmultiplied(h.r(), h.g(), h.b(), 125)
            };
            for me in &measurements {
                draw_measurement(ui, &m, me, accent, shadow, area);
            }
            if let Some(d) = drag {
                let r = rect_of(&m, d);
                painter.rect_filled(r, 0.0, Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 22));
                pen_rect(&painter, r, 4.0, shadow);
                dashed_rect(&painter, r, accent);
            } else if let Some(h) = &hover {
                draw_measurement(ui, &m, h, accent, shadow, area);
            }
            if let (Some(text), Some(pt)) = (status, pointer) {
                // `DrawStatus`: beside the pointer, flipped near the edges.
                let pointer = m(pt.0 as f32, pt.1 as f32);
                let size = label_size(ui.ctx(), &text);
                let mut x = pointer.x + 16.0 + size.x / 2.0;
                let mut y = pointer.y + 16.0 + size.y / 2.0;
                if x + size.x / 2.0 > area.width() - 6.0 {
                    x = pointer.x - 16.0 - size.x / 2.0;
                }
                if y + size.y / 2.0 > area.height() - 6.0 {
                    y = pointer.y - 16.0 - size.y / 2.0;
                }
                draw_label(ui, &text, pos2(x, y), accent, area);
            }
        });
    }
}

fn rect_of(m: &impl Fn(f32, f32) -> Pos2, r: PxRect) -> Rect {
    Rect::from_min_max(m(r.x as f32, r.y as f32), m(r.right() as f32, r.bottom() as f32))
}

/// `GetLineEndpoints`.
fn endpoints(m: &impl Fn(f32, f32) -> Pos2, me: &Measurement) -> (Pos2, Pos2) {
    let b = me.bounds;
    if me.axis == Axis::Horizontal {
        (m(b.x as f32, me.sample.1 as f32), m(b.right() as f32, me.sample.1 as f32))
    } else {
        (m(me.sample.0 as f32, b.y as f32), m(me.sample.0 as f32, b.bottom() as f32))
    }
}

/// The label's font: SemiBold, 13px (Segoe UI, as ShareX shows it).
fn label_font() -> egui::FontId {
    sharex_ui::widgets::font(LABEL_FONT_SIZE, 600)
}

/// `MeasureLabel`: the text plus 10x5 padding (unrounded, as `FormattedText`).
fn label_size(ctx: &egui::Context, text: &str) -> egui::Vec2 {
    let (w, h) = ctx.fonts_mut(|f| {
        let g = f.layout_no_wrap(text.to_string(), label_font(), Color32::WHITE);
        (g.size().x, f.row_height(&label_font()))
    });
    vec2(w + LABEL_PAD_X * 2.0, h + LABEL_PAD_Y * 2.0)
}

/// `GetLabelBounds`: centred on `center`, kept 6px inside the area.
fn label_rect(ctx: &egui::Context, text: &str, center: Pos2, area: Rect) -> Rect {
    let s = label_size(ctx, text);
    let r = Rect::from_center_size(center, s);
    let x = r.min.x.clamp(area.min.x + 6.0, (area.max.x - 6.0 - s.x).max(area.min.x + 6.0));
    let y = r.min.y.clamp(area.min.y + 6.0, (area.max.y - 6.0 - s.y).max(area.min.y + 6.0));
    Rect::from_min_size(pos2(x, y), s)
}

/// `GetLineLabelCenter`: the middle, or beside the line when it's shorter
/// than the label.
fn line_label_center(ctx: &egui::Context, text: &str, a: Pos2, b: Pos2, axis: Axis, area: Rect) -> Pos2 {
    let mut c = pos2((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
    let s = label_size(ctx, text);
    let (len, covered) = if axis == Axis::Horizontal { ((b.x - a.x).abs(), s.x) } else { ((b.y - a.y).abs(), s.y) };
    if len >= covered {
        return c;
    }
    let gap = 8.0;
    if axis == Axis::Horizontal {
        let off = END_CAP_SIZE / 2.0 + gap + s.y / 2.0;
        let (below, above) = (c.y + off, c.y - off);
        c.y = if below + s.y / 2.0 <= area.height() - 6.0 { below } else { above };
    } else {
        let off = gap + s.y / 2.0;
        let (below, above) = (a.y.max(b.y) + off, a.y.min(b.y) - off);
        c.y = if below + s.y / 2.0 <= area.height() - 6.0 { below } else { above };
    }
    c
}

fn rect_label_center(ctx: &egui::Context, r: Rect, text: &str, area: Rect) -> Pos2 {
    let h = label_size(ctx, text).y;
    let y = if r.max.y + 10.0 + h <= area.height() { r.max.y + 10.0 + h / 2.0 } else { r.min.y - 10.0 - h / 2.0 };
    pos2(r.center().x, y)
}

fn label_bounds(ctx: &egui::Context, m: &impl Fn(f32, f32) -> Pos2, me: &Measurement, area: Rect) -> Rect {
    let text = me.text();
    let center = match me.kind {
        Kind::Line => {
            let (a, b) = endpoints(m, me);
            line_label_center(ctx, &text, a, b, me.axis, area)
        }
        Kind::Rectangle => rect_label_center(ctx, rect_of(m, me.bounds), &text, area),
    };
    label_rect(ctx, &text, center, area)
}

/// `DrawLabel`: an accent capsule, the text in the colour that reads on it.
fn draw_label(ui: &egui::Ui, text: &str, center: Pos2, accent: Color32, area: Rect) {
    let r = label_rect(ui.ctx(), text, center, area);
    sharex_ui::border::border(ui.painter(), r, r.height() / 2.0, 0.0, Some(accent.into()), None);
    let fg = sharex_ui::accent::text_color_on(accent);
    let g = ui.painter().layout_no_wrap(text.to_string(), label_font(), fg);
    let th = ui.fonts_mut(|f| f.row_height(&label_font()));
    let at = pos2(r.min.x + (r.width() - g.size().x) / 2.0, r.min.y + (r.height() - th) / 2.0);
    ui.painter().galley(at, g, fg);
}

/// A centred `width` pen along a segment (`DrawLine`, flat caps), which
/// here is always horizontal or vertical.
fn pen_line(painter: &egui::Painter, a: Pos2, b: Pos2, width: f32, color: Color32) {
    let h = width / 2.0;
    let r = if (a.y - b.y).abs() < 0.01 {
        Rect::from_min_max(pos2(a.x.min(b.x), a.y - h), pos2(a.x.max(b.x), a.y + h))
    } else {
        Rect::from_min_max(pos2(a.x - h, a.y.min(b.y)), pos2(a.x + h, a.y.max(b.y)))
    };
    let mut mesh = egui::Mesh::default();
    mesh.add_colored_rect(r, color);
    painter.add(mesh);
}

/// `DrawRectangle(null, pen, rect)`.
fn pen_rect(painter: &egui::Painter, r: Rect, width: f32, color: Color32) {
    sharex_ui::border::border(painter, r.expand(width / 2.0), 0.0, width, None, Some(color));
}

/// The drag's 1px accent pen with `DashStyle([5, 4])`.
fn dashed_rect(painter: &egui::Painter, r: Rect, color: Color32) {
    let pts = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
    let mut d = 0.0f32;
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let len = (b - a).length();
        let dir = (b - a) / len.max(1e-6);
        let mut t = 0.0;
        while t < len {
            let phase = (d + t) % 9.0;
            let on = phase < 5.0;
            let step = if on { 5.0 - phase } else { 9.0 - phase }.min(len - t);
            if on {
                sharex_ui::border::border(painter, Rect::from_two_pos(a + dir * t, a + dir * (t + step)).expand(0.5), 0.0, 0.0, Some(color.into()), None);
            }
            t += step;
        }
        d += len;
    }
}

fn draw_measurement(ui: &egui::Ui, m: &impl Fn(f32, f32) -> Pos2, me: &Measurement, accent: Color32, shadow: Color32, area: Rect) {
    let painter = ui.painter().clone();
    let text = me.text();
    match me.kind {
        Kind::Line => {
            let (a, b) = endpoints(m, me);
            pen_line(&painter, a, b, 4.0, shadow);
            pen_line(&painter, a, b, 2.0, accent);
            let half = END_CAP_SIZE / 2.0;
            for p in [a, b] {
                if me.axis == Axis::Horizontal {
                    pen_line(&painter, pos2(p.x, p.y - half), pos2(p.x, p.y + half), 2.0, accent);
                } else {
                    pen_line(&painter, pos2(p.x - half, p.y), pos2(p.x + half, p.y), 2.0, accent);
                }
            }
            let c = line_label_center(ui.ctx(), &text, a, b, me.axis, area);
            draw_label(ui, &text, c, accent, area);
        }
        Kind::Rectangle => {
            let r = rect_of(m, me.bounds);
            painter.rect_filled(r, 0.0, Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 28));
            pen_rect(&painter, r, 4.0, shadow);
            pen_rect(&painter, r, 2.0, accent);
            let h = RECTANGLE_CROSS_SIZE / 2.0;
            let c = r.center();
            pen_line(&painter, pos2(c.x - h, c.y), pos2(c.x + h, c.y), 2.0, accent);
            pen_line(&painter, pos2(c.x, c.y - h), pos2(c.x, c.y + h), 2.0, accent);
            let lc = rect_label_center(ui.ctx(), r, &text, area);
            draw_label(ui, &text, lc, accent, area);
        }
    }
}
