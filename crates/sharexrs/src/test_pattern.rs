//! `sharexrs --test-pattern [--window X,Y,W,H] [--seconds N]`: covers every
//! monitor with a fixed, recognisable picture (a borderless, topmost window
//! per monitor), so a capture overlay opened on top of it freezes only the
//! pattern and never the desktop behind it. `--window` adds a smaller
//! window on top (another pattern), for checking window snapping. Both
//! close by themselves after `--seconds` (default 300) or on Escape.
//!
//! The pattern is a function of the physical screen pixel, so two captures
//! of the same region (ShareX's and ShareXrs's) can be compared exactly.

use std::time::{Duration, Instant};

use sharex_ui::gpu::Gpu;
use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{WindowAttributes, WindowId, WindowLevel};

use crate::window::EguiWindow;

/// The pattern's colour at a screen pixel: a red/green ramp over a 16 px
/// blue checker, with white lines every 64 px. `alt` is the snapping
/// window's variant (inverted ramp, orange checker).
pub fn pixel(x: i32, y: i32, alt: bool) -> [u8; 4] {
    if x.rem_euclid(64) == 0 || y.rem_euclid(64) == 0 {
        return [255, 255, 255, 255];
    }
    let checker = (x.div_euclid(16) + y.div_euclid(16)) % 2 == 0;
    let r = (x.rem_euclid(256)) as u8;
    let g = (y.rem_euclid(256)) as u8;
    if alt {
        [255 - r, 255 - g, if checker { 160 } else { 0 }, 255]
    } else {
        [r, g, if checker { 200 } else { 70 }, 255]
    }
}

fn image(x: i32, y: i32, w: u32, h: u32, alt: bool) -> egui::ColorImage {
    let mut pixels = Vec::with_capacity((w * h) as usize);
    for j in 0..h as i32 {
        for i in 0..w as i32 {
            let [r, g, b, a] = pixel(x + i, y + j, alt);
            pixels.push(egui::Color32::from_rgba_unmultiplied(r, g, b, a));
        }
    }
    egui::ColorImage { size: [w as usize, h as usize], source_size: egui::vec2(w as f32, h as f32), pixels }
}

struct Pane {
    window: EguiWindow,
    texture: Option<egui::TextureHandle>,
    origin: (i32, i32),
    alt: bool,
}

struct App {
    extra: Option<(i32, i32, u32, u32)>,
    until: Instant,
    gpu: Option<Gpu>,
    panes: Vec<Pane>,
}

pub fn run(args: &[String]) -> Result<(), String> {
    let mut extra = None;
    let mut seconds = 300u64;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--window" => {
                let v: Vec<i64> = it
                    .next()
                    .ok_or("--window X,Y,W,H")?
                    .split(',')
                    .filter_map(|s| s.trim().parse().ok())
                    .collect();
                let [x, y, w, h] = v[..] else { return Err("--window X,Y,W,H".into()) };
                extra = Some((x as i32, y as i32, w as u32, h as u32));
            }
            "--seconds" => seconds = it.next().and_then(|s| s.parse().ok()).ok_or("--seconds N")?,
            other => return Err(format!("unknown argument {other}")),
        }
    }
    let event_loop = EventLoop::new().map_err(|e| e.to_string())?;
    let mut app = App { extra, until: Instant::now() + Duration::from_secs(seconds), gpu: None, panes: Vec::new() };
    event_loop.run_app(&mut app).map_err(|e| e.to_string())
}

impl App {
    fn open(&mut self, event_loop: &ActiveEventLoop, rect: (i32, i32, u32, u32), alt: bool) -> Result<(), String> {
        let gpu = match &self.gpu {
            Some(g) => g,
            None => self.gpu.insert(Gpu::new()?),
        };
        let (x, y, w, h) = rect;
        let attrs = WindowAttributes::default()
            .with_title(if alt { "ShareXrs test window" } else { "ShareXrs test pattern" })
            .with_decorations(false)
            .with_resizable(false)
            .with_visible(false)
            .with_window_level(WindowLevel::AlwaysOnTop)
            .with_position(PhysicalPosition::new(x, y))
            .with_inner_size(PhysicalSize::new(w, h));
        let window = event_loop.create_window(attrs).map_err(|e| e.to_string())?;
        let window = EguiWindow::new(window, gpu, None)?;
        window.window.set_visible(true);
        window.window.request_redraw();
        self.panes.push(Pane { window, texture: None, origin: (x, y), alt });
        Ok(())
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if !self.panes.is_empty() {
            return;
        }
        let monitors: Vec<_> = event_loop.available_monitors().collect();
        for m in monitors {
            let (p, s) = (m.position(), m.size());
            if let Err(e) = self.open(event_loop, (p.x, p.y, s.width, s.height), false) {
                eprintln!("test pattern: {e}");
            }
        }
        if let Some(r) = self.extra
            && let Err(e) = self.open(event_loop, r, true)
        {
            eprintln!("test window: {e}");
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let Some(pane) = self.panes.iter_mut().find(|p| p.window.window.id() == id) else { return };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput { event: ref k, .. }
                if k.state == ElementState::Pressed && k.logical_key == Key::Named(NamedKey::Escape) =>
            {
                event_loop.exit()
            }
            WindowEvent::RedrawRequested => {
                let size = pane.window.window.inner_size();
                let (origin, alt) = (pane.origin, pane.alt);
                let texture = &mut pane.texture;
                pane.window.paint(|ui| {
                    let tex = texture.get_or_insert_with(|| {
                        ui.ctx().load_texture(
                            "pattern",
                            image(origin.0, origin.1, size.width, size.height, alt),
                            egui::TextureOptions::NEAREST,
                        )
                    });
                    let rect = ui.ctx().content_rect();
                    ui.painter().image(
                        tex.id(),
                        rect,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE,
                    );
                });
            }
            other => pane.window.on_event(&other),
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if Instant::now() >= self.until {
            event_loop.exit();
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(self.until));
    }
}
