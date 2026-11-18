//! The screen colour picker: `ShareX.Avalonia/Windows/ScreenColorPickerWindow`
//! (`.axaml` and code-behind). A transparent, topmost overlay over every
//! monitor; the live screen shows through it, and a panel next to the
//! pointer shows the 15x15 pixels around it enlarged nine times, the
//! colour under it and its info text (`InfoText`). A left click picks
//! (Ctrl held: `FormatCtrl`), a right click or Escape cancels.

use egui::{Color32, CornerRadius, Pos2, Rect, Stroke, StrokeKind, TextureOptions, pos2, vec2};
use serde_json::Value;
use sharex_capture::region::cursors::CustomCursor;
use sharex_capture::{Rect as PxRect, Screen};
use sharex_tools::color::{Color, screen_picker_format};
use sharex_ui::theme::palette;
use sharex_ui::widgets;
use winit::dpi::PhysicalPosition;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, NamedKey};
use winit::window::WindowId;

use super::overlay::Overlay;

/// `PreviewOffset`.
const PREVIEW_OFFSET: f32 = 18.0;
/// `MagnifierPixelCount`.
pub const MAGNIFIER_PIXEL_COUNT: i32 = 15;

/// `ScreenColorPickerOptions`.
#[derive(Clone, Debug, PartialEq)]
pub struct ScreenColorPickerOptions {
    pub format: String,
    pub format_ctrl: String,
    pub info_text: String,
    pub show_magnifier: bool,
}

impl Default for ScreenColorPickerOptions {
    fn default() -> Self {
        Self { format: "$HEX".into(), format_ctrl: "$r255, $g255, $b255".into(), info_text: "#$HEX".into(), show_magnifier: true }
    }
}

impl ScreenColorPickerOptions {
    pub fn from_json(v: &Value) -> Self {
        let d = Self::default();
        let s = |k: &str, d: String| match v.get(k) {
            Some(Value::String(s)) => s.clone(),
            Some(Value::Null) => String::new(),
            _ => d,
        };
        Self {
            format: s("Format", d.format),
            format_ctrl: s("FormatCtrl", d.format_ctrl),
            info_text: s("InfoText", d.info_text),
            show_magnifier: v.get("ShowMagnifier").and_then(Value::as_bool).unwrap_or(d.show_magnifier),
        }
    }
}

/// `ScreenColorPickerResult`.
#[derive(Clone, Debug, PartialEq)]
pub struct PickResult {
    pub color: Color,
    pub position: (i32, i32),
    pub control_pressed: bool,
}

/// What became of the input.
pub enum Outcome {
    Continue,
    /// The window closed: the colour picked, or `None`.
    Done(Option<PickResult>),
}

#[derive(Clone, Copy, PartialEq)]
enum PointerAction {
    None,
    Select,
    Cancel,
}

/// What the panel shows (`UpdatePicker`).
#[derive(Clone, Debug, Default)]
pub struct PanelState {
    /// The 15x15 pixels around the pointer (straight RGBA), when shown.
    pub magnifier: Option<image::RgbaImage>,
    pub color: Option<Color>,
    pub info: String,
    /// Where the pointer is, in the overlay's points from the desktop's
    /// top-left corner.
    pub pointer: Pos2,
    /// `_pickerPreview.Opacity`: 0 until the first update.
    pub visible: bool,
}

pub struct ScreenPicker {
    overlay: Overlay,
    options: ScreenColorPickerOptions,
    /// The desktop (`Screens.All`'s bounds) and its scaling (the top-left
    /// monitor's, as ShareX's window takes).
    desktop: PxRect,
    scaling: f64,
    state: PanelState,
    textures: Vec<Option<egui::TextureHandle>>,
    /// `Bounds` of the panel at the last layout (points).
    panel_size: egui::Vec2,
    current: Option<(Color, (i32, i32))>,
    pressed: Option<(Color, (i32, i32))>,
    action: PointerAction,
    control: bool,
}

/// The screen's pixels around `(x, y)`: `BitBlt` of the screen DC
/// (`SRCCOPY`, so layered windows aren't in it), black off screen.
pub fn sample(x: i32, y: i32) -> Option<image::RgbaImage> {
    let r = MAGNIFIER_PIXEL_COUNT / 2;
    sharex_capture::platform::sample_screen(PxRect::new(x - r, y - r, MAGNIFIER_PIXEL_COUNT, MAGNIFIER_PIXEL_COUNT))
}

impl ScreenPicker {
    /// `PickAsync`: the overlay over every monitor (`ConfigureOverlayBounds`),
    /// updated for where the pointer is now (`OnLoaded`).
    pub fn open(
        event_loop: &ActiveEventLoop,
        gpu: &sharex_ui::gpu::Gpu,
        options: ScreenColorPickerOptions,
        theme: egui::Theme,
    ) -> Result<Self, String> {
        let screens: Vec<Screen> = sharex_capture::screenshot::screens();
        let desktop = super::overlay::desktop_bounds(&screens);
        let scaling = sharex_capture::screenshot::screen_from_point(&screens, (desktop.x, desktop.y))
            .or(screens.first())
            .map_or(1.0, |s| s.scale);
        let title = format!("ShareXrs - {}", sharex_strings::sharex::ScreenColorPicker());
        let mut overlay = Overlay::open(event_loop, gpu, &screens, &title, true, theme)?;
        overlay.set_cursor(event_loop, CustomCursor::Crosshair);
        let n = overlay.windows.len();
        let mut me = Self {
            overlay,
            options,
            desktop,
            scaling,
            state: PanelState::default(),
            textures: vec![None; n],
            panel_size: vec2(1.0, 1.0),
            current: None,
            pressed: None,
            action: PointerAction::None,
            control: false,
        };
        let cursor = sharex_capture::platform::cursor_position();
        if let Some(p) = cursor {
            me.update(p);
        }
        for i in 0..n {
            me.paint(i);
        }
        me.overlay.show(cursor);
        Ok(me)
    }

    pub fn owns(&self, id: WindowId) -> bool {
        self.overlay.owns(id)
    }

    pub fn wake_at(&mut self, now: std::time::Instant) -> Option<std::time::Instant> {
        self.overlay.due(now)
    }

    /// `UpdatePicker`: the pixels and colour under `p` (desktop pixels).
    fn update(&mut self, p: (i32, i32)) {
        let pixels = sample(p.0, p.1);
        if let Some(px) = &pixels {
            let c = px.get_pixel((MAGNIFIER_PIXEL_COUNT / 2) as u32, (MAGNIFIER_PIXEL_COUNT / 2) as u32);
            let color = Color::rgb(c[0], c[1], c[2]);
            self.state.info = screen_picker_format(&self.options.info_text, &color, p);
            self.state.color = Some(color.clone());
            self.current = Some((color, p));
        }
        self.state.magnifier = if self.options.show_magnifier { pixels } else { None };
        let s = self.scaling as f32;
        self.state.pointer = pos2((p.0 - self.desktop.x) as f32 / s, (p.1 - self.desktop.y) as f32 / s);
        self.state.visible = true;
        for t in &mut self.textures {
            *t = None;
        }
        self.overlay.redraw();
    }

    fn paint(&mut self, i: usize) {
        let w = &self.overlay.windows[i];
        let s = self.scaling as f32;
        let origin = pos2((w.bounds.x - self.desktop.x) as f32 / s, (w.bounds.y - self.desktop.y) as f32 / s);
        let area = vec2(self.desktop.w as f32 / s, self.desktop.h as f32 / s);
        let state = &self.state;
        let tex = &mut self.textures[i];
        let last = self.panel_size;
        let mw = &mut self.overlay.windows[i];
        let size = mw.window.paint(|ui| {
            // The window's origin is the desktop's at `origin`.
            let shift = -origin.to_vec2();
            paint_panel(ui, state, last, area, shift, tex)
        });
        self.panel_size = size;
    }

    /// `Complete`.
    fn complete(&mut self, result: Option<PickResult>) -> Outcome {
        self.overlay.hide();
        Outcome::Done(result)
    }

    pub fn window_event(&mut self, id: WindowId, event: WindowEvent) -> Outcome {
        let Some(i) = self.overlay.index(id) else { return Outcome::Continue };
        match event {
            WindowEvent::RedrawRequested => self.paint(i),
            WindowEvent::CursorMoved { position, .. } => {
                let p = self.overlay.desktop_point(i, position);
                self.update(p);
            }
            WindowEvent::ModifiersChanged(m) => self.control = m.state().control_key(),
            WindowEvent::MouseInput { state: ElementState::Pressed, button, .. } => {
                if let Some(p) = sharex_capture::platform::cursor_position() {
                    self.update(p);
                }
                match button {
                    MouseButton::Left => {
                        self.pressed = self.current.clone();
                        self.action = PointerAction::Select;
                    }
                    MouseButton::Right => self.action = PointerAction::Cancel,
                    _ => {}
                }
            }
            WindowEvent::MouseInput { state: ElementState::Released, .. } if self.action != PointerAction::None => {
                let action = std::mem::replace(&mut self.action, PointerAction::None);
                let result = match (action, self.pressed.take()) {
                    (PointerAction::Select, Some((color, position))) => Some(PickResult { color, position, control_pressed: self.control }),
                    _ => None,
                };
                return self.complete(result);
            }
            WindowEvent::KeyboardInput { event, .. }
                if event.state == ElementState::Pressed && event.logical_key == Key::Named(NamedKey::Escape) =>
            {
                return self.complete(None);
            }
            WindowEvent::CloseRequested => return self.complete(None),
            other => self.overlay.windows[i].window.on_event(&other),
        }
        Outcome::Continue
    }

    /// The pointer position over window `i` in desktop pixels (for tests).
    #[allow(dead_code)]
    pub fn point_in(&self, i: usize, p: PhysicalPosition<f64>) -> (i32, i32) {
        self.overlay.desktop_point(i, p)
    }
}

/// The panel's content width and the info text's lines.
fn measure(ui: &egui::Ui, state: &PanelState) -> (egui::Vec2, std::sync::Arc<egui::Galley>) {
    let font = widgets::font(14.0, 600);
    let p = palette(ui.ctx());
    // `TextWrapping="Wrap"` within `MaxWidth="240"`.
    let job = egui::text::LayoutJob {
        wrap: egui::text::TextWrapping { max_width: 240.0, ..Default::default() },
        ..egui::text::LayoutJob::single_section(state.info.replace("\r\n", "\n"), egui::TextFormat::simple(font.clone(), p.text))
    };
    let galley = ui.fonts_mut(|f| f.layout_job(job));
    let lines = galley.rows.len().max(1) as f32;
    let text_h = (widgets::line_height(ui, &font) * lines).ceil();
    let text_w = galley.size().x.ceil().min(240.0);
    let magnifier = if state.magnifier.is_some() { 136.0 } else { 0.0 };
    let row_h = text_h.max(28.0);
    let content_w = (28.0 + 8.0 + text_w).max(magnifier);
    let content_h = if state.magnifier.is_some() { 136.0 + 8.0 + row_h } else { row_h };
    // `Padding="8"` and a 1px border.
    (vec2(content_w + 18.0, content_h + 18.0), galley)
}

/// Draws the panel (`PickerPreview`) where `MovePreviewNextToCursor` puts
/// it; `area` is the overlay's size and `shift` moves desktop points into
/// this window. Returns the panel's size.
pub fn paint_panel(
    ui: &mut egui::Ui,
    state: &PanelState,
    last_size: egui::Vec2,
    area: egui::Vec2,
    shift: egui::Vec2,
    tex: &mut Option<egui::TextureHandle>,
) -> egui::Vec2 {
    let (size, galley) = measure(ui, state);
    if !state.visible {
        return size;
    }
    let p = palette(ui.ctx());
    // `MovePreviewNextToCursor` with the panel's last measured bounds.
    let (pw, ph) = (last_size.x.max(1.0), last_size.y.max(1.0));
    let c = state.pointer;
    let mut x = c.x + PREVIEW_OFFSET;
    let mut y = c.y + PREVIEW_OFFSET;
    if x + pw > area.x {
        x = c.x - pw - PREVIEW_OFFSET;
    }
    if y + ph > area.y {
        y = c.y - ph - PREVIEW_OFFSET;
    }
    let x = x.clamp(0.0, (area.x - pw).max(0.0)).round_ties_even();
    let y = y.clamp(0.0, (area.y - ph).max(0.0)).round_ties_even();
    let panel = Rect::from_min_size(pos2(x, y) + shift, size);
    let painter = ui.painter().clone();
    // `BoxShadow="0 8 24 #66000000"`.
    let shadow = egui::epaint::Shadow { offset: [0, 8], blur: 24, spread: 0, color: Color32::from_black_alpha(0x66) };
    painter.add(shadow.as_shape(panel, CornerRadius::same(8)));
    sharex_ui::border::border(&painter, panel, 8.0, 1.0, Some(p.background_main.into()), Some(p.border));
    let content = panel.shrink(9.0);
    let mut top = content.min.y;
    if let Some(m) = &state.magnifier {
        let t = tex.get_or_insert_with(|| {
            let ci = egui::ColorImage::from_rgba_unmultiplied([m.width() as usize, m.height() as usize], m.as_raw());
            ui.ctx().load_texture("screen color picker magnifier", ci, TextureOptions::NEAREST)
        });
        // The 136x136 grid centres the 135x135 image (0.5, rounded to 0)
        // and the 11x11 marker (62.5, rounded to 62).
        let grid = Rect::from_min_size(pos2(content.min.x, top), vec2(136.0, 136.0));
        let img = Rect::from_min_size(grid.min, vec2(135.0, 135.0));
        painter.image(t.id(), img, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        let marker = Rect::from_min_size(grid.min + vec2(62.0, 62.0), vec2(11.0, 11.0));
        // `BoxShadow="0 0 0 1 #CC000000"` outside a white 1px border.
        painter.rect_stroke(marker.expand(1.0), 0.0, Stroke::new(1.0, Color32::from_black_alpha(102)), StrokeKind::Inside);
        painter.rect_stroke(marker, 0.0, Stroke::new(1.0, Color32::WHITE), StrokeKind::Inside);
        top += 136.0 + 8.0;
    }
    let row_h = (size.y - 18.0 - (top - content.min.y)).max(28.0);
    let row = Rect::from_min_size(pos2(content.min.x, top), vec2(content.width(), row_h));
    let swatch = Rect::from_min_size(pos2(row.min.x, widgets::center_y(row, 28.0)), vec2(28.0, 28.0));
    let fill = state.color.as_ref().map_or(Color32::TRANSPARENT, |c| Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a));
    sharex_ui::border::border(&painter, swatch, 4.0, 1.0, Some(fill.into()), Some(p.border));
    let font = widgets::font(14.0, 600);
    let text_h = widgets::line_height(ui, &font) * galley.rows.len().max(1) as f32;
    let ty = widgets::center_y(row, text_h);
    painter.galley(pos2(row.min.x + 36.0, ty), galley, p.text);
    size
}
