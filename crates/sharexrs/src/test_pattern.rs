//! `sharexrs --test-pattern [--window X,Y,W,H] [--framed X,Y,W,H] [--scroll X,Y,W,H] [--seconds N]`:
//! covers every monitor with a fixed, recognisable picture (a borderless,
//! topmost window per monitor), so a capture overlay opened on top of it
//! freezes only the pattern and never the desktop behind it. `--window` adds
//! a smaller window on top (another pattern), for checking window snapping.
//! `--framed` adds a normal window with a title bar, rounded corners and a
//! shadow (the other pattern in its client area, placed by screen pixel),
//! for window captures: client area only, transparency and shadow.
//! `--scroll` adds a scrollable window on top: a tall generated page
//! ([`page_pixel`]) that the mouse wheel (40 px a notch), Down/Up (20 px),
//! Page Down/Up (the height less 40) and Home/End scroll, for scrolling
//! capture. They close by themselves after `--seconds` (default 300) or on
//! Escape.
//!
//! The pattern is a function of the physical screen pixel, so two captures
//! of the same region (ShareX's and ShareXrs's) can be compared exactly.

use std::time::{Duration, Instant};

use sharex_ui::gpu::Gpu;
use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, MouseScrollDelta, WindowEvent};
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

/// The scroll window's page height.
pub const PAGE_HEIGHT: u32 = 2400;

/// The scroll window's page at (x, y): lines of grey "word" blocks with a
/// coloured mark at the left of each line, every line laid out by a fixed
/// pseudo-random sequence of its index, and a blue band every 300 px.
/// Deterministic, so the pictures two apps stitch of it can be checked
/// against [`page_image`].
pub fn page_pixel(x: i32, y: i32, width: i32) -> [u8; 4] {
    const BG: [u8; 4] = [248, 248, 250, 255];
    if y % 300 < 6 {
        let v = (y / 300) as u8;
        return [40 + v * 20, 60, 160 - v * 10, 255];
    }
    let line = y / 22;
    let in_line = y % 22;
    if !(4..16).contains(&in_line) {
        return BG;
    }
    let mut seed = (line as u32).wrapping_mul(2_654_435_761).wrapping_add(12345);
    let mut next = || {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        seed >> 8
    };
    if x < 4 {
        return [(next() & 255) as u8, (next() & 255) as u8, (next() & 255) as u8, 255];
    }
    let mut cx = 10;
    while cx < width - 10 {
        let w = 6 + (next() % 50) as i32;
        let gap = 4 + (next() % 8) as i32;
        let c = 20 + (next() % 120) as u8;
        if x >= cx && x < (cx + w).min(width - 10) {
            return [c, c.saturating_add(20), c.saturating_add(40), 255];
        }
        cx += w + gap;
    }
    BG
}

/// The whole page, `width` wide.
pub fn page_image(width: u32) -> image::RgbaImage {
    image::RgbaImage::from_fn(width, PAGE_HEIGHT, |x, y| image::Rgba(page_pixel(x as i32, y as i32, width as i32)))
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

/// What a window shows.
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Pattern,
    /// `--window`: the alternative pattern.
    Window,
    /// `--framed`: the alternative pattern in a decorated window.
    Framed,
    /// `--scroll`: the page at this offset.
    Scroll(i32),
}

struct Pane {
    window: EguiWindow,
    texture: Option<egui::TextureHandle>,
    origin: (i32, i32),
    kind: Kind,
}

impl Pane {
    /// Scrolls the page by `dy` pixels, kept within it.
    fn scroll_by(&mut self, dy: i32) {
        let h = self.window.window.inner_size().height as i32;
        if let Kind::Scroll(o) = &mut self.kind {
            *o = (*o + dy).clamp(0, (PAGE_HEIGHT as i32 - h).max(0));
        }
        self.window.window.request_redraw();
    }
}

struct App {
    extra: Option<(i32, i32, u32, u32)>,
    framed: Option<(i32, i32, u32, u32)>,
    scroll: Option<(i32, i32, u32, u32)>,
    until: Instant,
    gpu: Option<Gpu>,
    panes: Vec<Pane>,
}

fn parse_rect(v: Option<&String>, name: &str) -> Result<(i32, i32, u32, u32), String> {
    let v: Vec<i64> = v.ok_or(format!("{name} X,Y,W,H"))?.split(',').filter_map(|s| s.trim().parse().ok()).collect();
    let [x, y, w, h] = v[..] else { return Err(format!("{name} X,Y,W,H")) };
    Ok((x as i32, y as i32, w as u32, h as u32))
}

pub fn run(args: &[String]) -> Result<(), String> {
    let mut extra = None;
    let mut framed = None;
    let mut scroll = None;
    let mut seconds = 300u64;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--window" => extra = Some(parse_rect(it.next(), "--window")?),
            "--framed" => framed = Some(parse_rect(it.next(), "--framed")?),
            "--scroll" => scroll = Some(parse_rect(it.next(), "--scroll")?),
            "--seconds" => seconds = it.next().and_then(|s| s.parse().ok()).ok_or("--seconds N")?,
            other => return Err(format!("unknown argument {other}")),
        }
    }
    let event_loop = EventLoop::new().map_err(|e| e.to_string())?;
    let mut app = App { extra, framed, scroll, until: Instant::now() + Duration::from_secs(seconds), gpu: None, panes: Vec::new() };
    event_loop.run_app(&mut app).map_err(|e| e.to_string())
}

impl App {
    fn open(&mut self, event_loop: &ActiveEventLoop, rect: (i32, i32, u32, u32), kind: Kind) -> Result<(), String> {
        let gpu = match &self.gpu {
            Some(g) => g,
            None => self.gpu.insert(Gpu::new()?),
        };
        let (x, y, w, h) = rect;
        let title = match kind {
            Kind::Pattern => "ShareXrs test pattern",
            Kind::Window => "ShareXrs test window",
            Kind::Framed => "ShareXrs framed test window",
            Kind::Scroll(_) => "ShareXrs scroll test",
        };
        let attrs = WindowAttributes::default()
            .with_title(title)
            .with_decorations(kind == Kind::Framed)
            .with_resizable(false)
            .with_visible(false)
            .with_window_level(WindowLevel::AlwaysOnTop)
            .with_position(PhysicalPosition::new(x, y))
            .with_inner_size(PhysicalSize::new(w, h));
        let window = event_loop.create_window(attrs).map_err(|e| e.to_string())?;
        // Without dithering: the pattern and the page show pixel for pixel, so
        // two grabs of the same place are the same.
        let window = EguiWindow::exact(window, gpu)?;
        window.window.set_visible(true);
        window.window.request_redraw();
        self.panes.push(Pane { window, texture: None, origin: (x, y), kind });
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
            if let Err(e) = self.open(event_loop, (p.x, p.y, s.width, s.height), Kind::Pattern) {
                eprintln!("test pattern: {e}");
            }
        }
        if let Some(r) = self.extra
            && let Err(e) = self.open(event_loop, r, Kind::Window)
        {
            eprintln!("test window: {e}");
        }
        if let Some(r) = self.framed
            && let Err(e) = self.open(event_loop, r, Kind::Framed)
        {
            eprintln!("framed test window: {e}");
        }
        if let Some(r) = self.scroll
            && let Err(e) = self.open(event_loop, r, Kind::Scroll(0))
        {
            eprintln!("scroll window: {e}");
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let Some(pane) = self.panes.iter_mut().find(|p| p.window.window.id() == id) else { return };
        let scrolls = matches!(pane.kind, Kind::Scroll(_));
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput { event: ref k, .. }
                if k.state == ElementState::Pressed && k.logical_key == Key::Named(NamedKey::Escape) =>
            {
                event_loop.exit()
            }
            WindowEvent::MouseWheel { delta, .. } if scrolls => {
                let lines = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
                };
                pane.scroll_by((-lines * 40.0).round() as i32);
            }
            WindowEvent::KeyboardInput { event: ref k, .. } if scrolls && k.state == ElementState::Pressed => {
                let page = pane.window.window.inner_size().height as i32 - 40;
                match &k.logical_key {
                    Key::Named(NamedKey::ArrowDown) => pane.scroll_by(20),
                    Key::Named(NamedKey::ArrowUp) => pane.scroll_by(-20),
                    Key::Named(NamedKey::PageDown) => pane.scroll_by(page),
                    Key::Named(NamedKey::PageUp) => pane.scroll_by(-page),
                    Key::Named(NamedKey::Home) => pane.scroll_by(-(PAGE_HEIGHT as i32)),
                    Key::Named(NamedKey::End) => pane.scroll_by(PAGE_HEIGHT as i32),
                    _ => {}
                }
            }
            WindowEvent::RedrawRequested => {
                let size = pane.window.window.inner_size();
                let (mut origin, kind) = (pane.origin, pane.kind);
                // A framed window's client area, where its pattern starts.
                if kind == Kind::Framed
                    && let Ok(p) = pane.window.window.inner_position()
                {
                    origin = (p.x, p.y);
                }
                let texture = &mut pane.texture;
                pane.window.paint(|ui| {
                    let rect = ui.ctx().content_rect();
                    match kind {
                        Kind::Scroll(offset) => {
                            let tex = texture.get_or_insert_with(|| {
                                let page = page_image(size.width);
                                let image = egui::ColorImage::from_rgba_unmultiplied([page.width() as usize, page.height() as usize], page.as_raw());
                                ui.ctx().load_texture("page", image, egui::TextureOptions::NEAREST)
                            });
                            let v0 = offset as f32 / PAGE_HEIGHT as f32;
                            let v1 = (offset + size.height as i32) as f32 / PAGE_HEIGHT as f32;
                            ui.painter().image(tex.id(), rect, egui::Rect::from_min_max(egui::pos2(0.0, v0), egui::pos2(1.0, v1)), egui::Color32::WHITE);
                        }
                        kind => {
                            let tex = texture.get_or_insert_with(|| {
                                ui.ctx().load_texture(
                                    "pattern",
                                    image(origin.0, origin.1, size.width, size.height, matches!(kind, Kind::Window | Kind::Framed)),
                                    egui::TextureOptions::NEAREST,
                                )
                            });
                            ui.painter().image(tex.id(), rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
                        }
                    }
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

#[cfg(test)]
mod tests {
    #[test]
    fn page_is_deterministic_and_varied() {
        let a = super::page_image(300);
        assert_eq!(a, super::page_image(300));
        // No two text lines alike (so stitching has one answer).
        let rows: std::collections::HashSet<Vec<u8>> = (0..super::PAGE_HEIGHT / 22).map(|l| image::imageops::crop_imm(&a, 0, l * 22 + 8, 300, 1).to_image().into_raw()).collect();
        assert!(rows.len() > 100);
    }

    #[test]
    fn page_stitches_back_to_itself() {
        use sharex_capture::scrolling::Stitcher;
        let page = super::page_image(420);
        let mut s = Stitcher::new(true);
        let mut off = 0;
        loop {
            let frame = image::imageops::crop_imm(&page, 0, off, 420, 480).to_image();
            assert!(s.add(&frame), "offset {off}: {:?}", s.status);
            if off == super::PAGE_HEIGHT - 480 {
                break;
            }
            off = (off + 80).min(super::PAGE_HEIGHT - 480);
        }
        assert!(s.result.unwrap() == page);
    }
}
