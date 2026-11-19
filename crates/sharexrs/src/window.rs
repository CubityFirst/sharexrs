//! One OS window drawing egui through the shared wgpu device: what every
//! ShareXrs window (main window, settings, tools) is built on.

use std::sync::Arc;
use std::time::{Duration, Instant};

use egui_wgpu::winit::Painter;
use sharex_ui::gpu::Gpu;
use winit::event::WindowEvent;
use winit::window::{CursorIcon, ResizeDirection, Window};

pub struct EguiWindow {
    pub window: Arc<Window>,
    state: egui_winit::State,
    painter: Painter,
    /// When egui asked to be repainted next (a blinking cursor, an animation).
    pub repaint_at: Option<Instant>,
    /// Borderless windows resize from this many points inside their edge.
    resize_border: Option<f32>,
    resize_hover: Option<ResizeDirection>,
    /// What the window is cleared to before egui draws (premultiplied
    /// RGBA); the theme's panel colour when unset.
    pub clear_color: Option<[f32; 4]>,
    /// Where the pointer last moved in the window: winit doesn't repeat a
    /// `CursorMoved` at the same spot after the pointer left and came back
    /// (a click at the same place twice, the window cut to a region),
    /// while egui forgets the pointer when it leaves.
    last_cursor: Option<winit::dpi::PhysicalPosition<f64>>,
    /// The left button is down in the window (winit has captured the mouse).
    left_down: bool,
    /// Where the last left press was on the screen and where the window
    /// was then, in physical pixels: what [`Self::begin_move_drag`] anchors to.
    press: Option<((i32, i32), (i32, i32))>,
    /// A move drag in progress: the window follows the pointer from `press`.
    move_drag: Option<((i32, i32), (i32, i32))>,
    /// Popups past the window's edge, each in its own window
    /// ([`crate::popup_window`]).
    popups: crate::popup_window::Popups,
}

impl Drop for EguiWindow {
    fn drop(&mut self) {
        self.popups.clear(self.window.id());
    }
}

/// Where a window dragged from `press` (the pointer on the screen) while
/// at `origin` goes with the pointer at `cursor`: by the pointer's whole
/// movement since the press, so it ends exactly where the pointer does.
pub fn move_drag_target(press: (i32, i32), origin: (i32, i32), cursor: (i32, i32)) -> (i32, i32) {
    (origin.0 + cursor.0 - press.0, origin.1 + cursor.1 - press.1)
}

impl EguiWindow {
    pub fn new(window: Window, gpu: &Gpu, resize_border: Option<f32>) -> Result<Self, String> {
        Self::with_alpha(window, gpu, resize_border, false, true)
    }

    /// A window whose transparent pixels show what's behind it
    /// (`TransparencyLevelHint="Transparent"`): the swap chain is composited
    /// with premultiplied alpha (DirectComposition on Windows), and the
    /// window is cleared to transparent.
    pub fn transparent(window: Window, gpu: &Gpu) -> Result<Self, String> {
        let mut w = Self::with_alpha(window, gpu, None, true, true)?;
        w.clear_color = Some([0.0; 4]);
        Ok(w)
    }

    /// [`Self::transparent`] without egui's dithering, so pictures show
    /// pixel for pixel (pinned images, the screen tools' overlays).
    pub fn transparent_exact(window: Window, gpu: &Gpu) -> Result<Self, String> {
        let mut w = Self::with_alpha(window, gpu, None, true, false)?;
        w.clear_color = Some([0.0; 4]);
        Ok(w)
    }

    /// [`Self::new`] without egui's dithering (a frozen screen shown
    /// pixel for pixel).
    pub fn exact(window: Window, gpu: &Gpu) -> Result<Self, String> {
        Self::with_alpha(window, gpu, None, false, false)
    }

    fn with_alpha(window: Window, gpu: &Gpu, resize_border: Option<f32>, alpha: bool, dithering: bool) -> Result<Self, String> {
        let window = Arc::new(window);
        let ctx = egui::Context::default();
        sharex_ui::install(&ctx);
        let config = egui_wgpu::WgpuConfiguration { wgpu_setup: gpu.egui_setup(), ..Default::default() };
        let options = egui_wgpu::RendererOptions { dithering, ..Default::default() };
        let mut painter = pollster::block_on(Painter::new(ctx.clone(), config, alpha, options));
        pollster::block_on(painter.set_window(egui::ViewportId::ROOT, Some(window.clone())))
            .map_err(|e| format!("couldn't set up the window: {e}"))?;
        let state = egui_winit::State::new(
            ctx,
            egui::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            window.theme(),
            painter.max_texture_side(),
        );
        Ok(Self { window, state, painter, repaint_at: None, resize_border, resize_hover: None, clear_color: None, last_cursor: None, left_down: false, press: None, move_drag: None, popups: Default::default() })
    }

    pub fn ctx(&self) -> &egui::Context {
        self.state.egui_ctx()
    }

    /// Whether the pointer is over one of the window's popup windows: a
    /// `CursorLeft` then isn't the pointer leaving (a menu level past the
    /// window's edge is still the window's).
    pub fn pointer_on_popup(&self) -> bool {
        self.popups.contains_cursor(&self.window)
    }

    /// Passes a window event to egui, except on a borderless window's resize
    /// edge, where a press starts resizing the window instead.
    pub fn on_event(&mut self, event: &WindowEvent) {
        match event {
            WindowEvent::Resized(size) => {
                if let (Some(w), Some(h)) =
                    (std::num::NonZeroU32::new(size.width), std::num::NonZeroU32::new(size.height))
                {
                    self.painter.on_window_resized(egui::ViewportId::ROOT, w, h);
                }
            }
            WindowEvent::CursorEntered { device_id } => {
                let _ = self.state.on_window_event(&self.window, event);
                if let Some(position) = self.last_cursor {
                    let moved = WindowEvent::CursorMoved { device_id: *device_id, position };
                    let _ = self.state.on_window_event(&self.window, &moved);
                }
                self.window.request_redraw();
                return;
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.last_cursor = Some(*position);
                if self.move_drag.is_some() {
                    self.follow_pointer();
                }
                let dir = self.resize_direction(position.x, position.y);
                if dir != self.resize_hover {
                    self.resize_hover = dir;
                    if let Some(d) = dir {
                        self.window.set_cursor(cursor_for(d));
                    }
                }
                if dir.is_some() {
                    // The edge is the window's, not egui's.
                    return;
                }
            }
            WindowEvent::MouseInput { state: winit::event::ElementState::Pressed, button: winit::event::MouseButton::Left, .. } => {
                if let Some(d) = self.resize_hover {
                    let _ = self.window.drag_resize_window(d);
                    return;
                }
                self.left_down = true;
                self.press = match (self.window.inner_position(), self.window.outer_position(), self.last_cursor) {
                    (Ok(inner), Ok(outer), Some(c)) => {
                        Some(((inner.x + c.x.round() as i32, inner.y + c.y.round() as i32), (outer.x, outer.y)))
                    }
                    _ => None,
                };
            }
            WindowEvent::MouseInput { state: winit::event::ElementState::Released, button: winit::event::MouseButton::Left, .. } => {
                if self.move_drag.take().is_some() {
                    self.follow_pointer_to_end();
                }
                self.left_down = false;
            }
            WindowEvent::Focused(false) => {
                self.move_drag = None;
            }
            // Onto one of its popups: the pointer is still the window's.
            WindowEvent::CursorLeft { .. } if self.popups.contains_cursor(&self.window) => return,
            _ => {}
        }
        if self.state.on_window_event(&self.window, event).repaint {
            self.window.request_redraw();
        }
    }

    /// `BeginMoveDrag(e)` from a press egui handled a frame later. Avalonia
    /// calls `DefWindowProc(WM_NCLBUTTONDOWN, HTCAPTION)` inside the press
    /// handler, so the system's move loop anchors at the press
    /// (`GetMessagePos`); winit's `drag_window` posts the message a frame
    /// later and anchors wherever the pointer is by then, losing what it
    /// moved in between (APP-13a), and running the loop synchronously
    /// inside winit's handler would re-enter it. So on Windows the window
    /// follows the pointer itself, from where it was pressed, until the
    /// button comes up; elsewhere the system's drag (anchored at its event).
    pub fn begin_move_drag(&mut self) {
        if cfg!(windows)
            && self.left_down
            && let Some(press) = self.press
        {
            self.move_drag = Some(press);
            self.follow_pointer();
            return;
        }
        let _ = self.window.drag_window();
    }

    fn follow_pointer(&self) {
        let Some((press, origin)) = self.move_drag else { return };
        self.move_to_pointer(press, origin);
    }

    /// The release's position (the drag ends where the button came up).
    fn follow_pointer_to_end(&self) {
        if let Some((press, origin)) = self.press {
            self.move_to_pointer(press, origin);
        }
    }

    fn move_to_pointer(&self, press: (i32, i32), origin: (i32, i32)) {
        let Some(cursor) = sharex_capture::screenshot::cursor_position() else { return };
        let (x, y) = move_drag_target(press, origin, cursor);
        if self.window.outer_position().is_ok_and(|p| (p.x, p.y) != (x, y)) {
            self.window.set_outer_position(winit::dpi::PhysicalPosition::new(x, y));
        }
    }

    fn resize_direction(&self, x: f64, y: f64) -> Option<ResizeDirection> {
        let border = self.resize_border? as f64 * self.window.scale_factor();
        if self.window.is_maximized() || !self.window.is_resizable() {
            return None;
        }
        let size = self.window.inner_size();
        let (w, h) = (size.width as f64, size.height as f64);
        if x < 0.0 || y < 0.0 || x >= w || y >= h {
            // Over a popup past the edge.
            return None;
        }
        let (l, r, t, b) = (x < border, x >= w - border, y < border, y >= h - border);
        Some(match (l, r, t, b) {
            (true, _, true, _) => ResizeDirection::NorthWest,
            (_, true, true, _) => ResizeDirection::NorthEast,
            (true, _, _, true) => ResizeDirection::SouthWest,
            (_, true, _, true) => ResizeDirection::SouthEast,
            (true, ..) => ResizeDirection::West,
            (_, true, ..) => ResizeDirection::East,
            (_, _, true, _) => ResizeDirection::North,
            (_, _, _, true) => ResizeDirection::South,
            _ => return None,
        })
    }

    /// Runs one frame of `ui` and draws it.
    pub fn paint<T>(&mut self, ui: impl FnOnce(&mut egui::Ui) -> T) -> T {
        let input = self.state.take_egui_input(&self.window);
        let ctx = self.ctx().clone();
        let mut result = None;
        let mut ui = Some(ui);
        // Popups may leave the window ([`crate::popup_window`]).
        let (host, screens) = crate::popup_window::host_area(&self.window).unzip();
        let mut moved_out: Vec<crate::popup_window::Wanted> = Vec::new();
        let mut output = ctx.run_ui(input, |root| {
            sharex_ui::popup::set_host_screens(root.ctx(), host, screens.clone());
            if let Some(f) = ui.take() {
                result = Some(f(root));
                let popups = sharex_ui::popup::take(root.ctx());
                if host.is_some() {
                    let client = root.ctx().viewport_rect();
                    for popup in popups.into_iter().filter(|p| !crate::popup_window::fits(client, p.rect)) {
                        let shapes = root
                            .ctx()
                            .graphics_mut(|g| g.get_mut(popup.layer).map(std::mem::take))
                            .map(|list| list.all_entries().cloned().collect())
                            .unwrap_or_default();
                        moved_out.push(crate::popup_window::Wanted { popup, shapes });
                    }
                }
            }
        });
        let (moved_out, fallback) = self.popups.prepare(&self.window, &mut self.painter, moved_out);
        output.shapes.extend(fallback);
        self.state.handle_platform_output(&self.window, std::mem::take(&mut output.platform_output));
        if self.resize_hover.is_some() {
            // egui just set its own cursor; the edge's wins.
            if let Some(d) = self.resize_hover {
                self.window.set_cursor(cursor_for(d));
            }
        }
        let primitives = ctx.tessellate(std::mem::take(&mut output.shapes), output.pixels_per_point);
        let clear = self.clear_color.unwrap_or_else(|| egui::Rgba::from(ctx.global_style().visuals.panel_fill).to_array());
        self.painter.paint_and_update_textures(
            egui::ViewportId::ROOT,
            output.pixels_per_point,
            clear,
            &primitives,
            &mut output.textures_delta,
            Vec::new(),
            &self.window,
        );
        output.textures_delta.clear();
        self.popups.paint(&self.window, &mut self.painter, &ctx, output.pixels_per_point, clear, moved_out);
        self.repaint_at = None;
        if let Some(vp) = output.viewport_output.get(&egui::ViewportId::ROOT) {
            if vp.repaint_delay.is_zero() {
                self.window.request_redraw();
            } else if vp.repaint_delay < Duration::from_secs(3600) {
                self.repaint_at = Some(Instant::now() + vp.repaint_delay);
            }
        }
        result.expect("ui ran")
    }
}

fn cursor_for(d: ResizeDirection) -> CursorIcon {
    match d {
        ResizeDirection::East | ResizeDirection::West => CursorIcon::EwResize,
        ResizeDirection::North | ResizeDirection::South => CursorIcon::NsResize,
        ResizeDirection::NorthEast | ResizeDirection::SouthWest => CursorIcon::NeswResize,
        ResizeDirection::NorthWest | ResizeDirection::SouthEast => CursorIcon::NwseResize,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_move_drag_follows_the_whole_pointer_movement() {
        // APP-13a: a drag of -160,-80 from client 26,19 moves the toolbar by
        // exactly that, however many steps the pointer took.
        let press = (771 + 26, 680 + 19);
        let origin = (771, 680);
        let step = |i: i32| (press.0 - 160 * i / 12, press.1 - 80 * i / 12);
        assert_eq!(super::move_drag_target(press, origin, step(1)), (758, 674));
        let cursor = step(12);
        assert_eq!(super::move_drag_target(press, origin, cursor), (611, 600));
    }
}
