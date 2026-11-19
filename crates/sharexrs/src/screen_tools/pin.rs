//! Pin to screen: `ShareX.Tools/Tools/PinToScreen` — the pinned image
//! (`PinToScreenWindow`: borderless, transparent, its border and shadow,
//! scale and opacity, move, minimize, its toolbar), the startup window that
//! chooses an image (`PinToScreenStartupWindow`) and the options window
//! (`PinToScreenOptionsWindow`). ShareX doesn't keep pins out of captures,
//! and neither does ShareXrs.

use std::time::{Duration, Instant};

use egui::{Color32, CornerRadius, Id, Rect, TextureOptions, Ui, pos2, vec2};
use image::RgbaImage;
use sharex_strings::tools as s;
use sharex_tools::pin::{self as engine, PinLayout, PinState, PinToScreenOptions, Placement};
use sharex_ui::controls::{self, ComboBox, ComboItem, DecimalUpDown, ToggleSwitch};
use sharex_ui::keys::app_icons as icon;
use sharex_ui::theme::{Thickness, palette};
use sharex_ui::widgets::{center_y, font, line_height, text_width};
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, NamedKey};
use winit::window::{WindowAttributes, WindowId, WindowLevel};

use crate::dialogs::{Cx, Dialog, Effect, Spec};
use crate::image_tools as tools_ui;
use crate::window::EguiWindow;

/// What a pin asks of the app.
pub enum PinAction {
    Close,
    /// `CopyImage(imageData)`.
    Copy(RgbaImage),
    /// The Options button: the options window for this pin.
    Options,
}

/// A pinned image.
pub struct PinWindow {
    pub window: EguiWindow,
    image: RgbaImage,
    texture: Option<egui::TextureHandle>,
    pub options: PinToScreenOptions,
    state: PinState,
    layout: PinLayout,
    /// The window's top-left (desktop pixels).
    position: (i32, i32),
    pointer_inside: bool,
    pointer: Option<egui::Pos2>,
    control: bool,
    shift: bool,
    last_press: Option<Instant>,
}

/// `GetDoubleClickTime`.
fn double_click_time() -> Duration {
    #[cfg(windows)]
    {
        // SAFETY: a plain query.
        let ms = unsafe { windows_sys::Win32::UI::Input::KeyboardAndMouse::GetDoubleClickTime() };
        Duration::from_millis(ms as u64)
    }
    #[cfg(not(windows))]
    Duration::from_millis(500)
}

impl PinWindow {
    /// `PinToScreenManager.Pin` / `OnOpened`: at `location` (the region's
    /// top-left, so the image covers what it shows), else placed in the
    /// working area of the screen under the pointer.
    pub fn open(
        event_loop: &ActiveEventLoop,
        gpu: &sharex_ui::gpu::Gpu,
        image: RgbaImage,
        options: PinToScreenOptions,
        location: Option<(i32, i32)>,
        theme: egui::Theme,
    ) -> Result<Self, String> {
        let screens = sharex_capture::screenshot::screens();
        let cursor = sharex_capture::platform::cursor_position().unwrap_or_default();
        let work = sharex_capture::screenshot::screen_from_point(&screens, location.unwrap_or(cursor))
            .or(screens.first())
            .map(|s| (s.work_area, s.scale))
            .unwrap_or((sharex_capture::Rect::new(0, 0, 1024, 768), 1.0));
        let state = PinState::new(&options);
        let layout = engine::layout(&options, &state, (image.width(), image.height()), work.1);
        let position = match location {
            Some((x, y)) => (x - layout.inset(), y - layout.inset()),
            None => {
                let w = work.0;
                engine::get_position(options.placement, options.placement_offset, (w.x, w.y, w.w, w.h), layout.window)
            }
        };
        let title = crate::rs_title(s::PinToScreenWindow_Pinned_image());
        #[allow(unused_mut)]
        let mut attrs = WindowAttributes::default()
            .with_title(title)
            .with_decorations(false)
            .with_resizable(false)
            .with_transparent(true)
            .with_visible(false)
            .with_window_level(if options.top_most { WindowLevel::AlwaysOnTop } else { WindowLevel::Normal })
            .with_position(PhysicalPosition::new(position.0, position.1))
            .with_inner_size(PhysicalSize::new(layout.window.0 as u32, layout.window.1 as u32));
        #[cfg(windows)]
        {
            use winit::platform::windows::WindowAttributesExtWindows;
            attrs = attrs.with_skip_taskbar(true).with_no_redirection_bitmap(true);
        }
        let window = event_loop.create_window(attrs).map_err(|e| e.to_string())?;
        let w = EguiWindow::transparent_exact(window, gpu)?;
        crate::set_theme(&w, theme);
        let mut me = Self {
            window: w,
            image,
            texture: None,
            options,
            state,
            layout,
            position,
            pointer_inside: false,
            pointer: None,
            control: false,
            shift: false,
            last_press: None,
        };
        me.set_cursor(event_loop, false);
        me.paint();
        me.window.window.set_visible(true);
        crate::window::activate(&me.window.window);
        Ok(me)
    }

    fn scaling(&self) -> f64 {
        self.window.window.scale_factor()
    }

    fn set_cursor(&self, event_loop: &ActiveEventLoop, dragging: bool) {
        use sharex_capture::region::cursors::{CustomCursor, cursor};
        let kind = if dragging { CustomCursor::ClosedHand } else { CustomCursor::OpenHand };
        if let Some(c) = cursor(kind, self.scaling())
            && let Ok(src) = winit::window::CustomCursor::from_rgba(c.rgba, c.width, c.height, c.hotspot.0, c.hotspot.1)
        {
            self.window.window.set_cursor(event_loop.create_custom_cursor(src));
        }
    }

    /// `ResizeWindow(keepCenter)`.
    fn resize(&mut self, keep_center: bool) {
        let previous = self.layout;
        self.layout = engine::layout(&self.options, &self.state, (self.image.width(), self.image.height()), self.scaling());
        let (w, h) = self.layout.window;
        if keep_center {
            self.position = (self.position.0 + (previous.window.0 - w) / 2, self.position.1 + (previous.window.1 - h) / 2);
            self.window.window.set_outer_position(PhysicalPosition::new(self.position.0, self.position.1));
        }
        let _ = self.window.window.request_inner_size(PhysicalSize::new(w as u32, h as u32));
        self.window.window.request_redraw();
    }

    /// `ApplyOptions` after the options window saved.
    pub fn apply_options(&mut self, options: PinToScreenOptions) {
        self.options = options;
        self.window.window.set_window_level(if self.options.top_most { WindowLevel::AlwaysOnTop } else { WindowLevel::Normal });
        self.texture = None;
        self.resize(self.options.keep_center_location);
    }

    fn toolbar_visible(&self) -> bool {
        let s = self.scaling();
        engine::toolbar_visible(self.pointer_inside, (self.layout.window.0 as f64 / s, self.layout.window.1 as f64 / s))
    }

    /// Draws the pin; returns what its toolbar asked for.
    pub fn paint(&mut self) -> Option<PinAction> {
        let layout = self.layout;
        let options = self.options.clone();
        let opacity = self.state.window_opacity() as f32;
        let show_toolbar = self.toolbar_visible();
        let scale_text = format!("{}%", self.state.scale);
        let image = &self.image;
        let texture = &mut self.texture;
        let mut action = None;
        let mut reset = false;
        let toolbar_geometry = |ui: &Ui, scale_text: &str| {
            let tw = text_width(ui, scale_text, font(13.0, 400));
            let scale_w = 1.0 + 8.0 + tw + 8.0 + 1.0;
            let content = 34.0 + 2.0 + scale_w + 2.0 + 34.0 + 2.0 + 34.0;
            let w = content + 10.0;
            let area = ui.max_rect();
            let x = area.min.x + ((area.width() - w) / 2.0).round_ties_even();
            (Rect::from_min_size(pos2(x, area.min.y), vec2(w, 42.0)), scale_w)
        };
        self.window.paint(|ui| {
            ui.multiply_opacity(opacity);
            let ppp = ui.ctx().pixels_per_point();
            let p = palette(ui.ctx());
            let painter = ui.painter().clone();
            let inset = layout.shadow as f32 / ppp;
            let frame = Rect::from_min_size(
                pos2(inset, inset),
                vec2((layout.image.0 + 2 * layout.border) as f32 / ppp, (layout.image.1 + 2 * layout.border) as f32 / ppp),
            );
            if options.shadow {
                // `BoxShadow { Blur = 14, OffsetY = 3, Color = #6E000000 }`.
                let shadow = egui::epaint::Shadow { offset: [0, 3], blur: 14, spread: 0, color: Color32::from_black_alpha(110) };
                painter.add(shadow.as_shape(frame, CornerRadius::ZERO));
            }
            let bg = &options.background_color;
            let bc = &options.border_color;
            let border_px = layout.border as f32 / ppp;
            painter.rect_filled(frame, 0.0, Color32::from_rgba_unmultiplied(bg.r, bg.g, bg.b, bg.a));
            if border_px > 0.0 {
                let c = Color32::from_rgba_unmultiplied(bc.r, bc.g, bc.b, bc.a);
                painter.rect_stroke(frame, 0.0, egui::Stroke::new(border_px, c), egui::StrokeKind::Inside);
            }
            let img_rect = frame.shrink(border_px);
            let filter = if options.high_quality_scale { TextureOptions::LINEAR } else { TextureOptions::NEAREST };
            let t = texture.get_or_insert_with(|| {
                let ci = egui::ColorImage::from_rgba_unmultiplied([image.width() as usize, image.height() as usize], image.as_raw());
                ui.ctx().load_texture("pinned image", ci, filter)
            });
            painter.image(t.id(), img_rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            if show_toolbar {
                let (r, scale_w) = toolbar_geometry(ui, &scale_text);
                tools_ui::panel(ui, r);
                let mut x = r.min.x + 5.0;
                let y = r.min.y + 5.0;
                let picker = crate::image_effects::picker_button;
                sharex_ui::tooltip::show(ui, Rect::from_min_size(pos2(x, y), vec2(34.0, 32.0)), Id::new("pin-copy-tip"), s::PinToScreenWindow_Copy_image());
                if picker(ui, Rect::from_min_size(pos2(x, y), vec2(34.0, 32.0)), Id::new("pin-copy"), icon::Copy, true)
                    .clicked()
                {
                    action = Some(PinAction::Copy(image.clone()));
                }
                x += 36.0;
                let sr = Rect::from_min_size(pos2(x, y), vec2(scale_w, 32.0));
                sharex_ui::tooltip::show(ui, sr, Id::new("pin-scale-tip"), s::PinToScreenWindow_Reset_scale_and_opacity());
                let resp = ui.interact(sr, Id::new("pin-scale"), egui::Sense::click());
                if resp.hovered() || resp.is_pointer_button_down_on() {
                    sharex_ui::border::border(&painter, sr, 4.0, 0.0, Some(p.control_background_disabled.into()), None);
                }
                let f = font(13.0, 400);
                tools_ui::text_at(ui, pos2(sr.min.x + 9.0, center_y(sr, line_height(ui, &f))), &scale_text, f, p.text);
                if resp.clicked() {
                    reset = true;
                }
                x += scale_w + 2.0;
                sharex_ui::tooltip::show(ui, Rect::from_min_size(pos2(x, y), vec2(34.0, 32.0)), Id::new("pin-options-tip"), s::PinToScreenWindow_Options());
                if picker(ui, Rect::from_min_size(pos2(x, y), vec2(34.0, 32.0)), Id::new("pin-options"), icon::Settings, true)
                    .clicked()
                {
                    action = Some(PinAction::Options);
                }
                x += 36.0;
                sharex_ui::tooltip::show(ui, Rect::from_min_size(pos2(x, y), vec2(34.0, 32.0)), Id::new("pin-close-tip"), s::PinToScreenWindow_Close());
                if picker(ui, Rect::from_min_size(pos2(x, y), vec2(34.0, 32.0)), Id::new("pin-close"), icon::Close, true)
                    .clicked()
                {
                    action = Some(PinAction::Close);
                }
            }
        });
        if reset {
            self.reset();
        }
        action
    }

    fn toolbar_contains(&self, p: egui::Pos2) -> bool {
        if !self.toolbar_visible() {
            return false;
        }
        let w = self.window.window.inner_size().width as f32 / self.scaling() as f32;
        // The toolbar spans at most the middle; a press there is the toolbar's.
        let tw = 34.0 * 3.0 + 6.0 + 60.0 + 10.0;
        p.y < 42.0 && (p.x - w / 2.0).abs() < tw / 2.0 + 10.0
    }

    /// `ChangeScale`.
    fn change_scale(&mut self, delta: i32) {
        if self.state.change_scale(delta) {
            self.resize(self.options.keep_center_location);
        }
    }

    fn change_opacity(&mut self, delta: i32) {
        self.state.change_opacity(delta);
        self.window.window.request_redraw();
    }

    /// `ResetImage`.
    fn reset(&mut self) {
        self.state.reset();
        self.resize(self.options.keep_center_location);
    }

    fn toggle_minimized(&mut self) {
        self.state.minimized = !self.state.minimized;
        self.resize(self.options.keep_center_location);
    }

    fn move_by(&mut self, dx: i32, dy: i32) {
        self.position = (self.position.0 + dx, self.position.1 + dy);
        self.window.window.set_outer_position(PhysicalPosition::new(self.position.0, self.position.1));
    }

    pub fn window_event(&mut self, event_loop: &ActiveEventLoop, event: WindowEvent) -> Option<PinAction> {
        match &event {
            WindowEvent::RedrawRequested => return self.paint(),
            WindowEvent::CloseRequested => return Some(PinAction::Close),
            WindowEvent::Moved(p) => self.position = (p.x, p.y),
            WindowEvent::CursorEntered { .. } => {
                self.pointer_inside = true;
                self.window.window.request_redraw();
            }
            WindowEvent::CursorLeft { .. } => {
                self.pointer_inside = false;
                self.window.window.request_redraw();
            }
            WindowEvent::CursorMoved { position, .. } => {
                let s = self.scaling();
                self.pointer = Some(pos2((position.x / s) as f32, (position.y / s) as f32));
                if !self.pointer_inside {
                    self.pointer_inside = true;
                    self.window.window.request_redraw();
                }
            }
            WindowEvent::ModifiersChanged(m) => {
                self.control = m.state().control_key();
                self.shift = m.state().shift_key();
            }
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. }
                if !self.pointer.is_some_and(|p| self.toolbar_contains(p)) =>
            {
                let now = Instant::now();
                if self.last_press.is_some_and(|t| now - t <= double_click_time()) {
                    self.last_press = None;
                    self.toggle_minimized();
                } else {
                    self.last_press = Some(now);
                    // `BeginMoveDrag`: the system's move loop.
                    self.set_cursor(event_loop, true);
                    let _ = self.window.window.drag_window();
                    self.set_cursor(event_loop, false);
                }
                return None;
            }
            WindowEvent::MouseInput { state: ElementState::Released, button: MouseButton::Right, .. } => return Some(PinAction::Close),
            WindowEvent::MouseInput { state: ElementState::Released, button: MouseButton::Middle, .. } if !self.state.minimized => {
                self.reset();
                return None;
            }
            WindowEvent::MouseWheel { delta, .. } if !self.state.minimized => {
                let y = match delta {
                    MouseScrollDelta::LineDelta(_, y) => *y as f64,
                    MouseScrollDelta::PixelDelta(p) => p.y,
                };
                let dir = if y > 0.0 { 1 } else if y < 0.0 { -1 } else { 0 };
                if dir != 0 {
                    if self.control {
                        self.change_opacity(dir * self.options.opacity_step.max(1));
                    } else if !self.shift {
                        self.change_scale(dir * self.options.scale_step.max(1));
                    }
                }
                return None;
            }
            WindowEvent::KeyboardInput { event: key, .. } if key.state == ElementState::Pressed => {
                let d = if self.shift { 10 } else { 1 };
                match &key.logical_key {
                    Key::Named(NamedKey::ArrowLeft) => self.move_by(-d, 0),
                    Key::Named(NamedKey::ArrowRight) => self.move_by(d, 0),
                    Key::Named(NamedKey::ArrowUp) => self.move_by(0, -d),
                    Key::Named(NamedKey::ArrowDown) => self.move_by(0, d),
                    _ => {
                        use winit::keyboard::{KeyCode, PhysicalKey};
                        let code = match key.physical_key {
                            PhysicalKey::Code(c) => Some(c),
                            _ => None,
                        };
                        if self.control && !self.shift && code == Some(KeyCode::KeyC) {
                            return Some(PinAction::Copy(self.image.clone()));
                        }
                        let plus = matches!(code, Some(KeyCode::NumpadAdd | KeyCode::Equal));
                        let minus = matches!(code, Some(KeyCode::NumpadSubtract | KeyCode::Minus));
                        if !self.state.minimized && (plus || minus) {
                            let dir = if plus { 1 } else { -1 };
                            if self.control {
                                self.change_opacity(dir * self.options.opacity_step.max(1));
                            } else {
                                self.change_scale(dir * self.options.scale_step.max(1));
                            }
                        }
                    }
                }
                return None;
            }
            _ => {}
        }
        self.window.on_event(&event);
        None
    }
}

/// Why the startup window is waiting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Region,
    Clipboard,
    File,
}

/// `PinToScreenStartupWindow`.
pub struct PinStartupWindow {
    pub error: String,
    busy: bool,
}

impl PinStartupWindow {
    pub fn new() -> Self {
        Self { error: String::new(), busy: false }
    }

    /// `SelectAsync`'s answer: a source gave nothing.
    pub fn failed(&mut self, source: Source) {
        self.busy = false;
        self.error = match source {
            Source::Region => s::PinToScreenStartupViewModel_No_region_selected(),
            Source::Clipboard => s::PinToScreenStartupViewModel_Clipboard_no_image(),
            Source::File => s::PinToScreenStartupViewModel_No_image_selected(),
        }
        .to_string();
    }

    fn height(&self, ui: &Ui) -> f32 {
        let heading = tools_ui::heading_height(ui);
        let normal = line_height(ui, &tools_ui::normal_font());
        let panel = 2.0 + 24.0 + heading + 6.0 + normal;
        let error = if self.error.trim().is_empty() { 0.0 } else { 12.0 + normal };
        tools_ui::MARGIN * 2.0 + panel + 12.0 + 88.0 + error
    }
}

impl Default for PinStartupWindow {
    fn default() -> Self {
        Self::new()
    }
}

impl Dialog for PinStartupWindow {
    fn spec(&self) -> Spec {
        Spec::fit_height(crate::rs_title(s::PinToScreenStartupWindow_ShareX_Pin_to_Screen()), 560.0, 0.0)
    }

    fn content_height(&mut self, ui: &mut Ui) -> f32 {
        self.height(ui)
    }

    fn ui(&mut self, ui: &mut Ui, _cx: &mut Cx) -> Vec<Effect> {
        let p = palette(ui.ctx());
        let c = tools_ui::background(ui);
        let heading = tools_ui::heading_height(ui);
        let nf = tools_ui::normal_font();
        let normal = line_height(ui, &nf);
        let ph = 2.0 + 24.0 + heading + 6.0 + normal;
        let panel = Rect::from_min_size(c.min, vec2(c.width(), ph));
        tools_ui::panel(ui, panel);
        let pc = panel.shrink(13.0);
        tools_ui::text_at(ui, pc.min, s::PinToScreenStartupWindow_Choose_an_image(), tools_ui::heading_font(), p.text);
        tools_ui::text_at(
            ui,
            pos2(pc.min.x, pc.min.y + heading + 6.0),
            s::PinToScreenStartupWindow_Pin_an_image_above_your_other_windows_for_quick_reference(),
            nf.clone(),
            p.text_secondary,
        );
        let y = panel.max.y + 12.0;
        let w = (c.width() - 20.0) / 3.0;
        let mut effects = Vec::new();
        let buttons = [
            (icon::Region, s::PinToScreenStartupWindow_Select_region(), Source::Region),
            (icon::ClipboardPaste, s::PinToScreenStartupWindow_From_clipboard(), Source::Clipboard),
            (icon::FolderOpen, s::PinToScreenStartupWindow_Select_image(), Source::File),
        ];
        for (k, (key, text, source)) in buttons.into_iter().enumerate() {
            let x0 = (c.min.x + k as f32 * (w + 10.0)).round_ties_even();
            let x1 = (c.min.x + k as f32 * (w + 10.0) + w).round_ties_even();
            let r = Rect::from_min_max(pos2(x0, y), pos2(x1, y + 88.0));
            if controls::Button::default().show(ui, r, Id::new(("pin-startup", k)), "", !self.busy).clicked() {
                self.busy = true;
                self.error.clear();
                effects.push(Effect::PinFrom(source));
            }
            // `StackPanel Spacing="8"`: a 24px icon over the text, centred.
            // The `StackPanel` is centred in the button's unrounded column
            // (inside its border and 10px padding), the text at its left
            // and the icon centred over it.
            let tf = font(13.0, 400);
            let lh = line_height(ui, &tf);
            let top = center_y(r, 24.0 + 8.0 + lh);
            let tw = text_width(ui, text, tf.clone());
            let stack = tw.max(24.0);
            let col = c.min.x + k as f32 * (w + 10.0);
            let sx = (col + 11.0 + (w - 22.0 - stack) / 2.0).round_ties_even();
            let ix = sx + ((stack - 24.0) / 2.0).round_ties_even();
            let opacity = if self.busy { 0.35 } else { 1.0 };
            sharex_ui::icons::paint(ui.painter(), Rect::from_min_size(pos2(ix, top), vec2(24.0, 24.0)), key, p.text.gamma_multiply(opacity));
            tools_ui::text_at(ui, pos2(sx + ((stack - tw) / 2.0).round_ties_even(), top + 32.0), text, tf, p.text.gamma_multiply(opacity));
        }
        if !self.error.trim().is_empty() {
            let ey = y + 88.0 + 12.0;
            tools_ui::text_at(ui, pos2(c.min.x, ey), &self.error, nf, p.cancel_border);
        }
        // `OnKeyUp`: Escape closes.
        if ui.input(|i| i.key_released(egui::Key::Escape)) {
            effects.push(Effect::Close);
        }
        let h = self.height(ui);
        effects.push(Effect::SetHeight(h));
        effects
    }

    fn pin_failed(&mut self, source: Source) {
        self.failed(source);
    }
}

/// `PinToScreenOptionsWindow` for one pin (`ShowDialog(this)`).
pub struct PinOptionsWindow {
    pub pin: WindowId,
    o: PinToScreenOptions,
    /// The colour flyouts' state.
    picking: Option<u8>,
}

impl PinOptionsWindow {
    pub fn new(pin: WindowId, options: PinToScreenOptions) -> Self {
        Self { pin, o: options, picking: None }
    }
}

/// A `NumericUpDown` with a `FormatString` like `0'%'`.
#[allow(clippy::too_many_arguments)]
fn number(ui: &mut Ui, rect: Rect, id: &str, value: &mut i32, min: i32, max: i32, suffix: &'static str, spinner: bool) {
    let mut v = *value as f64;
    let d = DecimalUpDown { minimum: min as f64, maximum: max as f64, increment: 1.0, decimals: 0, enabled: true };
    if d.show_with(ui, rect, Id::new(("pin-options", id)), &mut v, &|v| format!("{}{suffix}", v.round() as i64), spinner) {
        *value = v.round() as i32;
    }
}

impl Dialog for PinOptionsWindow {
    fn spec(&self) -> Spec {
        Spec::sized(crate::rs_title(s::PinToScreenOptionsWindow_ShareX_Pin_to_Screen_options()), 760.0, 560.0, 700.0, 520.0)
    }

    fn ui(&mut self, ui: &mut Ui, _cx: &mut Cx) -> Vec<Effect> {
        let p = palette(ui.ctx());
        let c = tools_ui::background(ui);
        let col_w = ((c.width() - 12.0) / 2.0).floor();
        let lh = tools_ui::heading_height(ui);
        let row = |x: f32, y: f32| Rect::from_min_size(pos2(x, y), vec2(col_w - 26.0, 40.0));
        let label_w = 150.0;
        // Left: Size and opacity, then Placement.
        let left = c.min.x;
        let size_h = 26.0 + lh + 4.0 * 40.0 + 32.0 + 5.0 * 10.0;
        let size_panel = Rect::from_min_size(pos2(left, c.min.y), vec2(col_w, size_h));
        tools_ui::panel(ui, size_panel);
        let mut y = size_panel.min.y + 13.0;
        tools_ui::heading(ui, Rect::from_min_size(pos2(left + 13.0, y), vec2(col_w - 26.0, lh)), s::PinToScreenOptionsWindow_Size_and_opacity());
        y += lh + 10.0;
        for (k, (label, value, min, max)) in [
            (s::PinToScreenOptionsWindow_Initial_scale(), 0, 20, 500),
            (s::PinToScreenOptionsWindow_Scale_step(), 1, 1, 100),
            (s::PinToScreenOptionsWindow_Initial_opacity(), 2, 10, 100),
            (s::PinToScreenOptionsWindow_Opacity_step(), 3, 1, 90),
        ]
        .into_iter()
        .enumerate()
        {
            let r = row(left + 13.0, y);
            let control = tools_ui::label_row(ui, r, label_w, label);
            let v = match value {
                0 => &mut self.o.initial_scale,
                1 => &mut self.o.scale_step,
                2 => &mut self.o.initial_opacity,
                _ => &mut self.o.opacity_step,
            };
            number(ui, control, &format!("n{k}"), v, min, max, "%", true);
            y += 50.0;
        }
        toggle_row(ui, Rect::from_min_size(pos2(left + 13.0, y), vec2(col_w - 26.0, 32.0)), "pin-hq", s::PinToScreenOptionsWindow_High_quality_scaling(), &mut self.o.high_quality_scale);
        let place_top = size_panel.max.y + 12.0;
        let place_h = 26.0 + lh + 2.0 * 40.0 + 32.0 + 3.0 * 10.0;
        let place_panel = Rect::from_min_size(pos2(left, place_top), vec2(col_w, place_h));
        tools_ui::panel(ui, place_panel);
        let mut y = place_top + 13.0;
        tools_ui::heading(ui, Rect::from_min_size(pos2(left + 13.0, y), vec2(col_w - 26.0, lh)), s::PinToScreenOptionsWindow_Placement());
        y += lh + 10.0;
        let control = tools_ui::label_row(ui, row(left + 13.0, y), label_w, s::PinToScreenOptionsWindow_Default_position());
        let items: Vec<ComboItem> = placement_names().into_iter().map(ComboItem::text).collect();
        let selected = Placement::ALL.iter().position(|x| *x == self.o.placement);
        let combo = ComboBox { padding: Thickness::hv(8.0, 6.0), enabled: true };
        let cw = combo.auto_width(ui, &items, selected).min(control.width());
        if let Some(i) = combo.show(ui, Rect::from_min_size(control.min, vec2(cw, 40.0)), Id::new("pin-placement"), &items, selected) {
            self.o.placement = Placement::ALL[i.min(8)];
        }
        y += 50.0;
        let control = tools_ui::label_row(ui, row(left + 13.0, y), label_w, s::PinToScreenOptionsWindow_Screen_edge_offset());
        number(ui, control, "offset", &mut self.o.placement_offset, 0, 300, " px", true);
        y += 50.0;
        toggle_row(ui, Rect::from_min_size(pos2(left + 13.0, y), vec2(col_w - 26.0, 32.0)), "pin-center", s::PinToScreenOptionsWindow_Keep_center_while_resizing(), &mut self.o.keep_center_location);
        // Right: Appearance, Minimized preview.
        let right = c.min.x + col_w + 12.0;
        let rw = c.max.x - right;
        let app_h = 26.0 + lh + 3.0 * 32.0 + 2.0 * 40.0 + 5.0 * 10.0;
        let app_panel = Rect::from_min_size(pos2(right, c.min.y), vec2(rw, app_h));
        tools_ui::panel(ui, app_panel);
        let mut y = c.min.y + 13.0;
        tools_ui::heading(ui, Rect::from_min_size(pos2(right + 13.0, y), vec2(rw - 26.0, lh)), s::PinToScreenOptionsWindow_Appearance());
        y += lh + 10.0;
        for (k, label) in [s::PinToScreenOptionsWindow_Always_on_top(), s::PinToScreenOptionsWindow_Window_shadow(), s::PinToScreenOptionsWindow_Image_border()].into_iter().enumerate() {
            let v = match k {
                0 => &mut self.o.top_most,
                1 => &mut self.o.shadow,
                _ => &mut self.o.border,
            };
            toggle_row(ui, Rect::from_min_size(pos2(right + 13.0, y), vec2(rw - 26.0, 32.0)), &format!("pin-t{k}"), label, v);
            y += 42.0;
        }
        let rr = Rect::from_min_size(pos2(right + 13.0, y), vec2(rw - 26.0, 40.0));
        tools_ui::row_label(ui, Rect::from_min_size(rr.min, vec2(label_w, 40.0)), s::PinToScreenOptionsWindow_Border_size());
        let num = Rect::from_min_max(pos2(rr.min.x + label_w + 10.0, rr.min.y), pos2(rr.max.x - 50.0, rr.max.y));
        number(ui, num, "border", &mut self.o.border_size, 1, 30, " px", true);
        let border_anchor = Rect::from_min_size(pos2(rr.max.x - 40.0, rr.min.y), vec2(40.0, 40.0));
        // `ToolTip.Tip` (the row is disabled without a border: no tip).
        if self.o.border {
            sharex_ui::tooltip::show(ui, border_anchor, Id::new("pin-border-color-tip"), s::PinToScreenOptionsWindow_Border_color());
        }
        if color_button(ui, border_anchor, "pin-border-color", &self.o.border_color, self.o.border) {
            self.picking = Some(0);
        }
        y += 50.0;
        let rr = Rect::from_min_size(pos2(right + 13.0, y), vec2(rw - 26.0, 40.0));
        tools_ui::row_label(ui, Rect::from_min_size(rr.min, vec2(label_w, 40.0)), s::PinToScreenOptionsWindow_Background_color());
        let bg_anchor = Rect::from_min_size(pos2(rr.max.x - 40.0, rr.min.y), vec2(40.0, 40.0));
        sharex_ui::tooltip::show(ui, bg_anchor, Id::new("pin-bg-color-tip"), s::PinToScreenOptionsWindow_Background_color());
        if color_button(ui, bg_anchor, "pin-bg-color", &self.o.background_color, true) {
            self.picking = Some(1);
        }
        let prev_top = app_panel.max.y + 12.0;
        let prev_h = 26.0 + lh + 40.0 + 10.0;
        let prev_panel = Rect::from_min_size(pos2(right, prev_top), vec2(rw, prev_h));
        tools_ui::panel(ui, prev_panel);
        tools_ui::heading(ui, Rect::from_min_size(pos2(right + 13.0, prev_top + 13.0), vec2(rw - 26.0, lh)), s::PinToScreenOptionsWindow_Minimized_preview());
        let rr = Rect::from_min_size(pos2(right + 13.0, prev_top + 13.0 + lh + 10.0), vec2(rw - 26.0, 40.0));
        tools_ui::row_label(ui, Rect::from_min_size(rr.min, vec2(label_w, 40.0)), s::PinToScreenOptionsWindow_Preview_size());
        let times_w = text_width(ui, "×", font(13.0, 400));
        let star = ((rr.width() - label_w - times_w - 24.0) / 2.0).max(0.0);
        let x0 = rr.min.x + label_w + 8.0;
        let (mut mw, mut mh) = self.o.minimize_size;
        number(ui, Rect::from_min_size(pos2(x0, rr.min.y), vec2(star, 40.0)), "minw", &mut mw, 1, 1000, "", false);
        controls::label(ui, Rect::from_min_size(pos2(x0 + star + 8.0, rr.min.y), vec2(times_w, 40.0)), "×", font(13.0, 400), p.text);
        number(ui, Rect::from_min_size(pos2(x0 + star + 16.0 + times_w, rr.min.y), vec2(star, 40.0)), "minh", &mut mh, 1, 1000, "", false);
        self.o.minimize_size = (mw, mh);
        // The bottom row: the hint, Save and Cancel.
        let by = c.max.y - 40.0;
        let save = s::PinToScreenOptionsWindow_Save();
        let cancel = s::PinToScreenOptionsWindow_Cancel();
        let cw = tools_ui::text_button_width(ui, cancel);
        let sw = tools_ui::text_button_width(ui, save);
        let cancel_r = Rect::from_min_size(pos2(c.max.x - cw, by), vec2(cw, 40.0));
        let save_r = Rect::from_min_size(pos2(cancel_r.min.x - 8.0 - sw, by), vec2(sw, 40.0));
        let nf = tools_ui::normal_font();
        controls::label(
            ui,
            Rect::from_min_max(pos2(c.min.x, by + 4.0), pos2(save_r.min.x - 8.0, by + 40.0)),
            s::PinToScreenOptionsWindow_Mouse_wheel_changes_scale_Ctrl_mouse_wheel_changes_opacity(),
            nf,
            p.text_secondary,
        );
        let mut effects = Vec::new();
        if tools_ui::text_button(ui, save_r, Id::new("pin-options-save"), save, true).clicked() {
            // `Save`: the colours become plain ARGB (`ToDrawing`).
            let mut o = self.o.clone();
            let plain = |c: &sharex_tools::color::Color| sharex_tools::color::Color::argb(c.a, c.r, c.g, c.b);
            o.background_color = plain(&o.background_color);
            o.border_color = plain(&o.border_color);
            effects.push(Effect::PinOptionsSaved(self.pin, Box::new(o)));
            effects.push(Effect::Close);
        }
        if tools_ui::text_button(ui, cancel_r, Id::new("pin-options-cancel"), cancel, true).clicked() {
            effects.push(Effect::Close);
        }
        if let Some(which) = self.picking {
            let target = if which == 0 { &mut self.o.border_color } else { &mut self.o.background_color };
            let mut rgba = [target.r, target.g, target.b, target.a];
            let anchor = if which == 0 { Rect::from_min_size(pos2(c.max.x - 53.0, c.min.y + 13.0 + lh + 10.0 + 126.0), vec2(40.0, 40.0)) } else { bg_anchor };
            // `<Flyout><cp:ColorView/></Flyout>`: the view's defaults (alpha on).
            let (changed, keep) = controls::color_popup_styled(ui, Id::new(("pin-color", which)), anchor, &mut rgba, false, controls::ColorPopup::Flyout { alpha: true, size: None });
            if changed {
                let [r, g, b, a] = rgba;
                *target = sharex_tools::color::Color::argb(a, r, g, b);
            }
            if !keep {
                self.picking = None;
            }
        }
        effects
    }
}

/// A label with a `ToggleSwitch` at the right (`ColumnDefinitions="*,Auto"`, `MinHeight="30"`).
fn toggle_row(ui: &Ui, rect: Rect, id: &str, label: &str, on: &mut bool) {
    let p = palette(ui.ctx());
    controls::label(ui, rect, label, font(13.0, 400), p.text);
    let pos = pos2(rect.max.x - ToggleSwitch::SIZE.x, center_y(rect, ToggleSwitch::SIZE.y));
    ToggleSwitch { enabled: true }.show(ui, pos, Id::new(id), on);
}

/// The 40-wide button holding a 20x20 swatch (`CornerRadius="3"`).
fn color_button(ui: &Ui, rect: Rect, id: &str, c: &sharex_tools::color::Color, enabled: bool) -> bool {
    let p = palette(ui.ctx());
    let r = controls::Button::compact(Thickness::all(4.0)).show(ui, rect, Id::new(id), "", enabled);
    let sw = Rect::from_center_size(rect.center(), vec2(20.0, 20.0));
    let fill = Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a);
    sharex_ui::border::border(ui.painter(), sw, 3.0, 1.0, Some(fill.into()), Some(p.border));
    r.clicked()
}

/// `PinToScreenOptionsViewModel.Placements`' names.
fn placement_names() -> [&'static str; 9] {
    [
        s::PinToScreenOptionsViewModel_Top_left(),
        s::PinToScreenOptionsViewModel_Top_center(),
        s::PinToScreenOptionsViewModel_Top_right(),
        s::PinToScreenOptionsViewModel_Middle_left(),
        s::PinToScreenOptionsViewModel_Center(),
        s::PinToScreenOptionsViewModel_Middle_right(),
        s::PinToScreenOptionsViewModel_Bottom_left(),
        s::PinToScreenOptionsViewModel_Bottom_center(),
        s::PinToScreenOptionsViewModel_Bottom_right(),
    ]
}
