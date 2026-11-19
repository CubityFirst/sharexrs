//! Capture › Region (Transparent): `ShareX.ScreenCaptureLib/Presentation/
//! RegionCapture/RegionCaptureTransparentWindow.cs`. A rectangle is drawn
//! over the live desktop (nothing is frozen), and the screen is captured
//! once the windows are gone.
//!
//! ShareX stretches one window over the monitors; here each monitor gets
//! its own, as the region overlay does, all feeding one selection (the
//! pointer is captured while a button is down, so a drag can cross
//! monitors). The windows are nearly transparent (alpha 1, so they still
//! take the clicks) and show only the selection's outline: a white line
//! under black 5px dashes, one physical pixel wide.

use sharex_capture::{Rect, Screen, platform};
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, NamedKey};
use winit::window::{WindowAttributes, WindowId, WindowLevel};

/// `MinimumRectangleSize`: a selection must be wider and taller than this.
const MINIMUM_RECTANGLE_SIZE: i32 = 5;

/// What became of the input.
pub enum Outcome {
    Continue,
    /// The windows closed: the selected rectangle (desktop pixels), or
    /// `None` when cancelled.
    Done(Option<Rect>),
}

struct Surface {
    #[cfg(windows)]
    window: winit::window::Window,
    #[cfg(windows)]
    layer: layered::Layer,
    #[cfg(not(windows))]
    window: crate::window::EguiWindow,
    /// The monitor it covers.
    bounds: Rect,
}

impl Surface {
    fn window(&self) -> &winit::window::Window {
        #[cfg(windows)]
        return &self.window;
        #[cfg(not(windows))]
        return &self.window.window;
    }
}

pub struct TransparentRegion {
    surfaces: Vec<Surface>,
    /// The desktop pixels the selection is kept within.
    screen_bounds: Rect,
    start: (i32, i32),
    selecting: bool,
    suppress_right_release: bool,
    selection: Option<Rect>,
    /// Where the pointer is, desktop pixels.
    pointer: Option<(i32, i32)>,
    /// `lockCursorToWindow` (active monitor mode).
    cursor_lock: Option<sharex_capture::CursorLock>,
}

/// `CaptureHelpers.CreateRectangle`: the rectangle between two pixels,
/// both included.
fn create_rectangle(a: (i32, i32), b: (i32, i32)) -> Rect {
    let (x, y) = (a.0.min(b.0), a.1.min(b.1));
    Rect::new(x, y, (a.0 - b.0).abs() + 1, (a.1 - b.1).abs() + 1)
}

fn is_valid_selection(r: &Rect) -> bool {
    r.w > MINIMUM_RECTANGLE_SIZE && r.h > MINIMUM_RECTANGLE_SIZE
}

/// Whether the outline pixel `d` pixels along the path (clockwise from the
/// top-left corner) is under a black dash (`ImmutableDashStyle([5, 5])`).
fn dashed(d: i32) -> bool {
    d.rem_euclid(10) < 5
}

/// The selection's outline pixels in desktop pixels, each with whether
/// it's black (else white), through the centres of its edge pixels.
pub fn outline(r: Rect) -> Vec<(i32, i32, bool)> {
    let (x0, y0, x1, y1) = (r.x, r.y, r.right() - 1, r.bottom() - 1);
    let (w, h) = (x1 - x0, y1 - y0);
    let mut px = Vec::with_capacity(2 * (w + h).max(0) as usize + 1);
    for i in 0..w {
        px.push((x0 + i, y0, dashed(i)));
    }
    for j in 0..h {
        px.push((x1, y0 + j, dashed(w + j)));
    }
    for i in 0..w {
        px.push((x1 - i, y1, dashed(w + h + i)));
    }
    for j in 0..h {
        px.push((x0, y1 - j, dashed(2 * w + h + j)));
    }
    if w == 0 || h == 0 {
        px.push((x1, y1, dashed(w + h)));
    }
    px
}

impl TransparentRegion {
    /// `SelectAsync`: covers `screens` (the active one in active monitor
    /// mode) and focuses the one under the pointer.
    pub fn open(
        event_loop: &ActiveEventLoop,
        #[allow(unused_variables)] gpu: &sharex_ui::gpu::Gpu,
        screens: &[Screen],
        screen_bounds: Rect,
        lock_cursor: bool,
    ) -> Result<Self, String> {
        let mut surfaces = Vec::new();
        for screen in screens.iter().filter(|s| !s.bounds.intersect(&screen_bounds).is_empty()) {
            let b = screen.bounds.intersect(&screen_bounds);
            #[allow(unused_mut)]
            let mut attrs = WindowAttributes::default()
                .with_title("ShareX - Region capture (Transparent)")
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
            #[cfg(not(windows))]
            {
                attrs = attrs.with_transparent(true);
            }
            let window = event_loop.create_window(attrs).map_err(|e| e.to_string())?;
            platform::prepare_overlay(&window);
            #[cfg(windows)]
            let surface = {
                let layer = layered::Layer::new(&window, b).ok_or("couldn't make the window layered")?;
                Surface { window, layer, bounds: b }
            };
            #[cfg(not(windows))]
            let surface = {
                let mut w = crate::window::EguiWindow::new(window, gpu, None)?;
                // Alpha 1, so the window still takes the clicks.
                w.clear_color = Some([0.0, 0.0, 0.0, 1.0 / 255.0]);
                Surface { window: w, bounds: b }
            };
            surfaces.push(surface);
        }
        if surfaces.is_empty() {
            return Err("no monitors found".into());
        }
        let mut me = Self {
            surfaces,
            screen_bounds,
            start: (0, 0),
            selecting: false,
            suppress_right_release: false,
            selection: None,
            pointer: platform::cursor_position(),
            cursor_lock: None,
        };
        if lock_cursor {
            me.cursor_lock = me.surfaces.first().map(|s| sharex_capture::CursorLock::new(platform::window_handle(s.window())));
        }
        let crosshair = sharex_capture::region::cursors::CustomCursor::Crosshair;
        for i in 0..me.surfaces.len() {
            me.draw(i);
            let s = &me.surfaces[i];
            s.window().set_visible(true);
            let scale = s.window().scale_factor();
            if let Some(c) = sharex_capture::region::cursors::cursor(crosshair, scale)
                && let Ok(source) = winit::window::CustomCursor::from_rgba(c.rgba, c.width, c.height, c.hotspot.0, c.hotspot.1)
            {
                s.window().set_cursor(event_loop.create_custom_cursor(source));
            }
        }
        let focus = me
            .pointer
            .and_then(|(x, y)| me.surfaces.iter().position(|s| s.bounds.contains(x, y)))
            .unwrap_or(0);
        me.surfaces[focus].window().focus_window();
        Ok(me)
    }

    pub fn owns(&self, id: WindowId) -> bool {
        self.surfaces.iter().any(|s| s.window().id() == id)
    }

    /// Draws the selection's part on surface `i`.
    fn draw(&mut self, i: usize) {
        let selection = self.selection;
        let s = &mut self.surfaces[i];
        #[cfg(windows)]
        s.layer.draw(selection.map(outline).as_deref().unwrap_or(&[]));
        #[cfg(not(windows))]
        {
            let origin = (s.bounds.x, s.bounds.y);
            s.window.paint(|ui| {
                let ppp = ui.ctx().pixels_per_point();
                for (x, y, black) in selection.map(outline).unwrap_or_default() {
                    let p = egui::pos2((x - origin.0) as f32 / ppp, (y - origin.1) as f32 / ppp);
                    let color = if black { egui::Color32::BLACK } else { egui::Color32::WHITE };
                    ui.painter().rect_filled(egui::Rect::from_min_size(p, egui::Vec2::splat(1.0 / ppp)), 0.0, color);
                }
            });
        }
    }

    fn redraw(&mut self) {
        for i in 0..self.surfaces.len() {
            self.draw(i);
        }
    }

    /// `ToScreenPixel`: the desktop pixel under a position in surface `i`,
    /// kept within the screen bounds.
    fn to_screen_pixel(&self, i: usize, p: PhysicalPosition<f64>) -> (i32, i32) {
        let b = self.surfaces[i].bounds;
        let sb = self.screen_bounds;
        let x = b.x + (p.x + 0.001).floor() as i32;
        let y = b.y + (p.y + 0.001).floor() as i32;
        (x.clamp(sb.x, sb.right() - 1), y.clamp(sb.y, sb.bottom() - 1))
    }

    fn update_selection(&mut self, at: (i32, i32)) {
        let r = create_rectangle(self.start, at);
        let selection = is_valid_selection(&r).then_some(r);
        if selection != self.selection {
            self.selection = selection;
            self.redraw();
        }
    }

    fn reset_selection(&mut self) {
        self.selecting = false;
        if self.selection.take().is_some() {
            self.redraw();
        }
    }

    /// `Complete`: the cleared outline is drawn before the windows go, so
    /// none of it is left on screen.
    fn complete(&mut self, result: Option<Rect>) -> Outcome {
        self.selecting = false;
        self.selection = None;
        self.redraw();
        for s in &self.surfaces {
            s.window().set_visible(false);
        }
        Outcome::Done(result)
    }

    pub fn window_event(&mut self, id: WindowId, event: WindowEvent) -> Outcome {
        let Some(i) = self.surfaces.iter().position(|s| s.window().id() == id) else { return Outcome::Continue };
        match event {
            WindowEvent::KeyboardInput { event, .. }
                if event.state == ElementState::Pressed && event.logical_key == Key::Named(NamedKey::Escape) =>
            {
                return self.complete(None);
            }
            WindowEvent::CloseRequested => return self.complete(None),
            WindowEvent::CursorMoved { position, .. } => {
                let at = self.to_screen_pixel(i, position);
                self.pointer = Some(at);
                if self.selecting {
                    self.update_selection(at);
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let Some(at) = self.pointer else { return Outcome::Continue };
                match (button, state) {
                    (MouseButton::Left, ElementState::Pressed) => {
                        self.start = at;
                        self.selecting = true;
                        self.suppress_right_release = false;
                        self.update_selection(at);
                    }
                    (MouseButton::Left, ElementState::Released) => {
                        if self.selecting {
                            let r = create_rectangle(self.start, at);
                            if is_valid_selection(&r) {
                                return self.complete(Some(r));
                            }
                        }
                        self.reset_selection();
                    }
                    // A right click while drawing cancels the selection, and
                    // its release mustn't also cancel the capture.
                    (MouseButton::Right, ElementState::Pressed) if self.selecting => {
                        self.reset_selection();
                        self.suppress_right_release = true;
                    }
                    (MouseButton::Right, ElementState::Released) => {
                        if self.suppress_right_release {
                            self.suppress_right_release = false;
                        } else if self.selecting {
                            self.reset_selection();
                        } else {
                            return self.complete(None);
                        }
                    }
                    _ => {}
                }
            }
            WindowEvent::Focused(focused) => {
                if let Some(lock) = self.cursor_lock.as_mut() {
                    lock.focused(focused);
                }
            }
            #[cfg(not(windows))]
            WindowEvent::RedrawRequested => self.draw(i),
            #[cfg(not(windows))]
            other => self.surfaces[i].window.on_event(&other),
            #[cfg(windows)]
            _ => {}
        }
        Outcome::Continue
    }
}

/// A per-pixel alpha window (`UpdateLayeredWindow`), which shows its
/// bitmap as is: no GPU surface, so nothing but the outline is drawn.
#[cfg(windows)]
mod layered {
    use std::ffi::c_void;

    use sharex_capture::Rect;
    use windows_sys::Win32::Foundation::{HWND, POINT, SIZE};
    use windows_sys::Win32::Graphics::Gdi::{
        AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION, CreateCompatibleDC,
        CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, GdiFlush, HBITMAP, HDC, HGDIOBJ, SelectObject,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GWL_EXSTYLE, GetWindowLongPtrW, SetWindowLongPtrW, ULW_ALPHA, UpdateLayeredWindow, WS_EX_LAYERED,
    };
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

    pub struct Layer {
        hwnd: HWND,
        dc: HDC,
        bitmap: HBITMAP,
        previous: HGDIOBJ,
        bits: *mut u8,
        bounds: Rect,
    }

    impl Layer {
        pub fn new(window: &winit::window::Window, bounds: Rect) -> Option<Self> {
            let RawWindowHandle::Win32(h) = window.window_handle().ok()?.as_raw() else { return None };
            let hwnd = h.hwnd.get() as HWND;
            // SAFETY: our own window, still hidden; the DIB section and DC
            // are owned by the layer and freed in Drop.
            unsafe {
                let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
                SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style | WS_EX_LAYERED as isize);
                let dc = CreateCompatibleDC(std::ptr::null_mut());
                let mut bi: BITMAPINFO = std::mem::zeroed();
                bi.bmiHeader = BITMAPINFOHEADER {
                    biSize: size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: bounds.w,
                    biHeight: -bounds.h,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB,
                    ..std::mem::zeroed()
                };
                let mut bits: *mut c_void = std::ptr::null_mut();
                let bitmap = CreateDIBSection(dc, &bi, DIB_RGB_COLORS, &mut bits, std::ptr::null_mut(), 0);
                if bitmap.is_null() || bits.is_null() {
                    DeleteDC(dc);
                    return None;
                }
                let previous = SelectObject(dc, bitmap);
                Some(Self { hwnd, dc, bitmap, previous, bits: bits.cast(), bounds })
            }
        }

        /// Shows `outline` (desktop pixels: black or white) over alpha 1.
        pub fn draw(&mut self, outline: &[(i32, i32, bool)]) {
            let (w, h) = (self.bounds.w as usize, self.bounds.h as usize);
            // SAFETY: the DIB section is w*h BGRA pixels, alive with `self`.
            let pixels = unsafe { std::slice::from_raw_parts_mut(self.bits, w * h * 4) };
            // Premultiplied black at alpha 1.
            for px in pixels.chunks_exact_mut(4) {
                px.copy_from_slice(&[0, 0, 0, 1]);
            }
            for &(x, y, black) in outline {
                let (lx, ly) = (x - self.bounds.x, y - self.bounds.y);
                if lx < 0 || ly < 0 || lx as usize >= w || ly as usize >= h {
                    continue;
                }
                let at = (ly as usize * w + lx as usize) * 4;
                let v = if black { 0 } else { 255 };
                pixels[at..at + 4].copy_from_slice(&[v, v, v, 255]);
            }
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            let at = POINT { x: self.bounds.x, y: self.bounds.y };
            let size = SIZE { cx: self.bounds.w, cy: self.bounds.h };
            let origin = POINT { x: 0, y: 0 };
            // SAFETY: a layered window of ours and the DC holding its bitmap.
            unsafe {
                GdiFlush();
                UpdateLayeredWindow(
                    self.hwnd,
                    std::ptr::null_mut(),
                    &at,
                    &size,
                    self.dc,
                    &origin,
                    0,
                    &blend,
                    ULW_ALPHA,
                );
            }
        }
    }

    impl Drop for Layer {
        fn drop(&mut self) {
            // SAFETY: created in `new`, no longer used.
            unsafe {
                SelectObject(self.dc, self.previous);
                DeleteObject(self.bitmap);
                DeleteDC(self.dc);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selections_include_both_corners() {
        assert_eq!(create_rectangle((10, 20), (4, 25)), Rect::new(4, 20, 7, 6));
        assert!(!is_valid_selection(&Rect::new(0, 0, 5, 50)));
        assert!(is_valid_selection(&Rect::new(0, 0, 6, 6)));
    }

    #[test]
    fn the_outline_goes_round_the_edge_pixels_once() {
        let r = Rect::new(10, 10, 12, 8);
        let px = outline(r);
        assert_eq!(px.len(), 2 * (11 + 7));
        let mut unique: Vec<(i32, i32)> = px.iter().map(|&(x, y, _)| (x, y)).collect();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), px.len());
        // Dashes of 5, from the top-left corner.
        assert!(px[..5].iter().all(|p| p.2) && px[5..10].iter().all(|p| !p.2));
        assert!(px.iter().all(|&(x, y, _)| x == 10 || x == 21 || y == 10 || y == 17));
    }
}
