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
}

impl EguiWindow {
    pub fn new(window: Window, gpu: &Gpu, resize_border: Option<f32>) -> Result<Self, String> {
        let window = Arc::new(window);
        let ctx = egui::Context::default();
        sharex_ui::install(&ctx, 100.0);
        let config = egui_wgpu::WgpuConfiguration { wgpu_setup: gpu.egui_setup(), ..Default::default() };
        let mut painter = pollster::block_on(Painter::new(ctx.clone(), config, false, Default::default()));
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
        Ok(Self { window, state, painter, repaint_at: None, resize_border, resize_hover: None, clear_color: None })
    }

    pub fn ctx(&self) -> &egui::Context {
        self.state.egui_ctx()
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
            WindowEvent::CursorMoved { position, .. } => {
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
            }
            _ => {}
        }
        if self.state.on_window_event(&self.window, event).repaint {
            self.window.request_redraw();
        }
    }

    fn resize_direction(&self, x: f64, y: f64) -> Option<ResizeDirection> {
        let border = self.resize_border? as f64 * self.window.scale_factor();
        if self.window.is_maximized() || !self.window.is_resizable() {
            return None;
        }
        let size = self.window.inner_size();
        let (w, h) = (size.width as f64, size.height as f64);
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
        let mut output = ctx.run_ui(input, |root| {
            if let Some(f) = ui.take() {
                result = Some(f(root));
            }
        });
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
