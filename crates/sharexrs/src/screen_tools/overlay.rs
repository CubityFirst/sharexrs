//! What the screen tools' overlays share: one borderless, topmost window
//! per monitor (ShareX stretches one window over every monitor, as the
//! region capture does; ShareXrs gives each monitor its own so each has its
//! own scale), transparent over the live desktop (the screen colour picker,
//! the ruler) or opaque over a frozen one (the classic ruler); pointers in
//! desktop pixels; ShareX's own cursors (`CursorAssetLoader`).

use std::collections::HashMap;

use sharex_capture::region::cursors::CustomCursor;
use sharex_capture::{Rect, Screen, platform};
use sharex_ui::gpu::Gpu;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event_loop::ActiveEventLoop;
use winit::window::{WindowAttributes, WindowId, WindowLevel};

use crate::window::EguiWindow;

/// One monitor's window.
pub struct MonitorWindow {
    pub window: EguiWindow,
    /// The monitor it covers (desktop pixels).
    pub bounds: Rect,
    /// Its scaling.
    pub scale: f64,
}

/// The overlay's windows.
pub struct Overlay {
    pub windows: Vec<MonitorWindow>,
    cursors: HashMap<(CustomCursor, u32), winit::window::CustomCursor>,
    shown: Option<CustomCursor>,
}

/// A borderless, topmost, untaskbarred window over `b`, still hidden.
pub fn window_attributes(title: &str, b: Rect, transparent: bool) -> WindowAttributes {
    #[allow(unused_mut)]
    let mut attrs = WindowAttributes::default()
        .with_title(title)
        .with_decorations(false)
        .with_resizable(false)
        .with_visible(false)
        .with_transparent(transparent)
        .with_window_level(WindowLevel::AlwaysOnTop)
        .with_position(PhysicalPosition::new(b.x, b.y))
        .with_inner_size(PhysicalSize::new(b.w.max(1) as u32, b.h.max(1) as u32));
    #[cfg(windows)]
    {
        use winit::platform::windows::WindowAttributesExtWindows;
        attrs = attrs.with_skip_taskbar(true);
        if transparent {
            attrs = attrs.with_no_redirection_bitmap(true);
        }
    }
    attrs
}

impl Overlay {
    /// Opens a window on each of `screens` (`transparent` over the live
    /// desktop), in `theme`, painted once by `paint` before it shows.
    pub fn open(
        event_loop: &ActiveEventLoop,
        gpu: &Gpu,
        screens: &[Screen],
        title: &str,
        transparent: bool,
        theme: egui::Theme,
    ) -> Result<Self, String> {
        let mut windows = Vec::new();
        for s in screens {
            #[allow(unused_mut)]
            let mut attrs = window_attributes(title, s.bounds, transparent);
            // winit turns a physical place into points by the main screen's
            // scale (docs/MACOS-LEARNINGS.md 3.8): give macOS the monitor's.
            #[cfg(target_os = "macos")]
            {
                let b = s.bounds;
                if let Some(m) = event_loop
                    .available_monitors()
                    .min_by_key(|m| (m.position().x - b.x).abs() + (m.position().y - b.y).abs())
                {
                    let scale = m.scale_factor();
                    attrs = attrs.with_position(m.position().to_logical::<f64>(scale)).with_inner_size(m.size().to_logical::<f64>(scale));
                }
            }
            let window = event_loop.create_window(attrs).map_err(|e| e.to_string())?;
            platform::prepare_overlay(&window);
            let w = if transparent { EguiWindow::transparent_exact(window, gpu)? } else { EguiWindow::exact(window, gpu)? };
            crate::set_theme(&w, theme);
            windows.push(MonitorWindow { window: w, bounds: s.bounds, scale: s.scale });
        }
        if windows.is_empty() {
            return Err("no monitors found".into());
        }
        Ok(Self { windows, cursors: HashMap::new(), shown: None })
    }

    pub fn owns(&self, id: WindowId) -> bool {
        self.index(id).is_some()
    }

    pub fn index(&self, id: WindowId) -> Option<usize> {
        self.windows.iter().position(|w| w.window.window.id() == id)
    }

    /// Shows the windows (after their first paint) and focuses the one
    /// under the pointer.
    pub fn show(&self, pointer: Option<(i32, i32)>) {
        for w in &self.windows {
            w.window.window.set_visible(true);
        }
        let focus = pointer.and_then(|(x, y)| self.windows.iter().position(|w| w.bounds.contains(x, y))).unwrap_or(0);
        crate::window::activate(&self.windows[focus].window.window);
        for w in &self.windows {
            platform::overlay_presented(&w.window.window);
        }
    }

    pub fn redraw(&self) {
        for w in &self.windows {
            w.window.window.request_redraw();
        }
    }

    pub fn hide(&self) {
        for w in &self.windows {
            w.window.window.set_visible(false);
        }
    }

    /// The desktop pixel under a position in window `i` (physical pixels
    /// from its top-left corner).
    pub fn desktop_point(&self, i: usize, p: PhysicalPosition<f64>) -> (i32, i32) {
        let b = self.windows[i].bounds;
        (b.x + (p.x + 0.001).floor() as i32, b.y + (p.y + 0.001).floor() as i32)
    }

    /// Shows one of ShareX's cursors on every window.
    pub fn set_cursor(&mut self, event_loop: &ActiveEventLoop, kind: CustomCursor) {
        if self.shown == Some(kind) {
            return;
        }
        self.shown = Some(kind);
        for w in &self.windows {
            let scale = (w.scale * 100.0).round() as u32;
            let cursor = self.cursors.entry((kind, scale)).or_insert_with(|| {
                let b = sharex_capture::region::cursors::cursor(kind, w.scale).expect("ShareX's cursors decode");
                let source = winit::window::CustomCursor::from_rgba(b.rgba, b.width, b.height, b.hotspot.0, b.hotspot.1).expect("a valid cursor");
                event_loop.create_custom_cursor(source)
            });
            w.window.window.set_cursor(cursor.clone());
        }
    }

    /// A system cursor on every window.
    pub fn set_cursor_icon(&mut self, icon: winit::window::CursorIcon) {
        self.shown = None;
        for w in &self.windows {
            w.window.window.set_cursor(icon);
        }
    }

    /// When the windows asked to be repainted.
    pub fn due(&mut self, now: std::time::Instant) -> Option<std::time::Instant> {
        let mut next = None;
        for w in &mut self.windows {
            match w.window.repaint_at {
                Some(at) if at <= now => {
                    w.window.window.request_redraw();
                    w.window.repaint_at = None;
                }
                Some(at) => next = Some(next.map_or(at, |n: std::time::Instant| n.min(at))),
                None => {}
            }
        }
        next
    }
}

/// The bounds of all screens (`Screens.All`'s union).
pub fn desktop_bounds(screens: &[Screen]) -> Rect {
    sharex_capture::screenshot::screen_bounds(screens)
}
