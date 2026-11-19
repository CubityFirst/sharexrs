//! Popup windows for popups past their window's edge (`sharex_ui::popup`):
//! Avalonia opens each `Popup` (a menu level, a combo box's dropdown) in an
//! OS window of its own, kept on the screen's work area, not the owner's
//! client area.
//!
//! A popup stays a layer of its owner's egui context; when one doesn't fit
//! inside the owner's client area, [`crate::window::EguiWindow::paint`]
//! takes that layer's shapes out of the owner's frame and draws them into
//! a borderless, owned popup window here (another surface of the owner's
//! painter, so it shares its textures), cut to the popup's rounded corners.
//! The popup window never takes the activation (`WS_EX_NOACTIVATE`, shown
//! with `SW_SHOWNOACTIVATE`): the owner keeps the keyboard, as it does in
//! Avalonia. Its pointer input goes back to the owner ([`route_event`]),
//! moved into the owner's client coordinates, so the popup's own code
//! handles it as before.
//!
//! Windows only for now (where it's checked against ShareX); elsewhere
//! [`host_enabled`] is false and popups stay inside their window.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::epaint::ClippedShape;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId};

thread_local! {
    /// The event loop handling the current event, for creating popup windows
    /// while a window paints.
    static EVENT_LOOP: Cell<*const ActiveEventLoop> = const { Cell::new(std::ptr::null()) };
    /// Popup window → where its input goes.
    static ROUTES: RefCell<HashMap<WindowId, Route>> = RefCell::new(HashMap::new());
    /// The monitors, read at most every two seconds.
    static SCREENS: RefCell<Option<(Instant, Vec<sharex_capture::Screen>)>> = const { RefCell::new(None) };
}

/// Lets windows open popup windows while the event loop handles an event;
/// see [`enter`].
pub struct Scope<'a> {
    previous: *const ActiveEventLoop,
    _loop: PhantomData<&'a ActiveEventLoop>,
}

/// Called at the start of each `ApplicationHandler` callback: until the
/// returned scope drops, painting windows may create popup windows.
pub fn enter(event_loop: &ActiveEventLoop) -> Scope<'_> {
    let previous = EVENT_LOOP.with(|c| c.replace(event_loop as *const _));
    Scope { previous, _loop: PhantomData }
}

impl Drop for Scope<'_> {
    fn drop(&mut self) {
        EVENT_LOOP.with(|c| c.set(self.previous));
    }
}

fn with_event_loop<R>(f: impl FnOnce(&ActiveEventLoop) -> R) -> Option<R> {
    let p = EVENT_LOOP.with(|c| c.get());
    // SAFETY: set by `enter` from a reference that outlives its scope, and
    // reset when the scope drops.
    (!p.is_null()).then(|| f(unsafe { &*p }))
}

/// Whether a window can host popup windows now.
pub fn host_enabled() -> bool {
    cfg!(windows) && EVENT_LOOP.with(|c| !c.get().is_null())
}

/// Where a popup window's input goes.
#[derive(Clone, Debug)]
struct Route {
    owner: WindowId,
    /// The popup's client origin minus the owner's, in physical pixels.
    offset: (f64, f64),
    /// The owner's client area and its popups on the screen (physical): a
    /// pointer leaving a popup into one of them hasn't left the owner.
    areas: Vec<[i32; 4]>,
}

/// A popup window's event as the owner's: `Some((owner, event))` when `id`
/// is a popup window (`event` is `None` when the owner has nothing to do
/// with it).
pub fn route_event(id: WindowId, event: &WindowEvent) -> Option<(WindowId, Option<WindowEvent>)> {
    let route = ROUTES.with(|r| r.borrow().get(&id).cloned())?;
    let translated = match event {
        WindowEvent::CursorMoved { device_id, position } => Some(WindowEvent::CursorMoved {
            device_id: *device_id,
            position: winit::dpi::PhysicalPosition::new(position.x + route.offset.0, position.y + route.offset.1),
        }),
        WindowEvent::CursorLeft { .. } => {
            // Into the owner or another of its popups: they carry on.
            let inside = sharex_capture::screenshot::cursor_position()
                .is_some_and(|(x, y)| route.areas.iter().any(|a| x >= a[0] && y >= a[1] && x < a[0] + a[2] && y < a[1] + a[3]));
            (!inside).then(|| event.clone())
        }
        WindowEvent::MouseInput { .. } | WindowEvent::MouseWheel { .. } => Some(event.clone()),
        // Drawn with the owner.
        WindowEvent::RedrawRequested => Some(WindowEvent::RedrawRequested),
        _ => None,
    };
    Some((route.owner, translated))
}

fn with_screens<R>(f: impl FnOnce(&[sharex_capture::Screen]) -> R) -> R {
    SCREENS.with(|s| {
        let mut s = s.borrow_mut();
        if s.as_ref().is_none_or(|(at, _)| at.elapsed() > Duration::from_secs(2)) {
            *s = Some((Instant::now(), sharex_capture::screenshot::screens()));
        }
        f(s.as_ref().map(|(_, v)| v.as_slice()).unwrap_or_default())
    })
}

/// The work area of the monitor showing most of `client` (physical
/// screen pixels), as `[x, y, w, h]`.
fn work_area(screens: &[sharex_capture::Screen], client: [i32; 4]) -> Option<[i32; 4]> {
    let centre = (client[0] + client[2] / 2, client[1] + client[3] / 2);
    let screen = sharex_capture::screenshot::screen_from_point(screens, centre).or_else(|| screens.first())?;
    let w = screen.work_area;
    Some([w.x, w.y, w.w, w.h])
}

/// What Avalonia's positioner knows about `window`'s screens
/// (`ManagedPopupPositionerPopupImplHelper`: every screen's bounds and
/// work area, the client area on the screen, `DesktopScaling`).
pub fn geometry(screens: &[sharex_capture::Screen], client: [i32; 4], scaling: f64) -> sharex_ui::popup::Geometry {
    use sharex_ui::popup::{PxRect, ScreenInfo};
    let px = |r: sharex_capture::Rect| PxRect::new(r.x as f64, r.y as f64, r.w as f64, r.h as f64);
    sharex_ui::popup::Geometry {
        screens: screens.iter().map(|s| ScreenInfo { bounds: px(s.bounds), working_area: px(s.work_area) }).collect(),
        parent: PxRect::new(client[0] as f64, client[1] as f64, client[2] as f64, client[3] as f64),
        scaling,
    }
}

/// The owner's screen's work area in its points (relative to its client
/// origin), and the screens, for `sharex_ui::popup::set_host_screens`;
/// `None` when it can't host.
pub fn host_area(window: &Window) -> Option<(egui::Rect, sharex_ui::popup::Geometry)> {
    if !host_enabled() {
        return None;
    }
    let inner = window.inner_position().ok()?;
    let size = window.inner_size();
    let s = window.scale_factor() as f32;
    let client = [inner.x, inner.y, size.width as i32, size.height as i32];
    let (w, g) = with_screens(|screens| Some((work_area(screens, client)?, geometry(screens, client, window.scale_factor()))))?;
    let area = egui::Rect::from_min_max(
        egui::pos2((w[0] - inner.x) as f32 / s, (w[1] - inner.y) as f32 / s),
        egui::pos2((w[0] + w[2] - inner.x) as f32 / s, (w[1] + w[3] - inner.y) as f32 / s),
    );
    Some((area, g))
}

/// A popup layer moved out of the owner's frame.
pub struct Wanted {
    pub popup: sharex_ui::popup::PopupLayer,
    pub shapes: Vec<ClippedShape>,
}

/// One open popup window.
struct PopupWindow {
    window: Arc<Window>,
    viewport: egui::ViewportId,
    layer: egui::LayerId,
    /// Its client origin relative to the owner's, and size (physical).
    origin: (i32, i32),
    size: (u32, u32),
    radius: f32,
    shown: bool,
}

/// An owner's popup windows.
#[derive(Default)]
pub struct Popups {
    windows: Vec<PopupWindow>,
    next: u64,
}

/// A popup's place relative to the owner's client origin, in physical
/// pixels: its edges rounded as the owner draws them.
pub fn physical_rect(rect: egui::Rect, ppp: f32) -> ((i32, i32), (u32, u32)) {
    let (x0, y0) = ((rect.min.x * ppp).round() as i32, (rect.min.y * ppp).round() as i32);
    let (x1, y1) = ((rect.max.x * ppp).round() as i32, (rect.max.y * ppp).round() as i32);
    ((x0, y0), ((x1 - x0).max(1) as u32, (y1 - y0).max(1) as u32))
}

/// Whether a popup at `rect` (owner points) fits inside the owner's client
/// area `client` (it then stays in the owner's frame).
pub fn fits(client: egui::Rect, rect: egui::Rect) -> bool {
    client.expand(0.01).contains_rect(rect)
}

impl Popups {
    /// Whether the pointer is over one of the popups (the owner's
    /// `CursorLeft` then isn't the pointer leaving).
    pub fn contains_cursor(&self, owner: &Window) -> bool {
        let Some((x, y)) = sharex_capture::screenshot::cursor_position() else { return false };
        let Ok(inner) = owner.inner_position() else { return false };
        self.windows.iter().any(|p| {
            let (px, py) = (inner.x + p.origin.0, inner.y + p.origin.1);
            x >= px && y >= py && x < px + p.size.0 as i32 && y < py + p.size.1 as i32
        })
    }

    /// Makes sure each wanted popup has a window; the ones that couldn't
    /// get one are returned, to be drawn in the owner after all.
    pub fn prepare(&mut self, owner: &Window, painter: &mut egui_wgpu::winit::Painter, wanted: Vec<Wanted>) -> (Vec<Wanted>, Vec<ClippedShape>) {
        let mut keep = Vec::new();
        let mut fallback = Vec::new();
        for w in wanted {
            if self.windows.iter().any(|p| p.layer == w.popup.layer) || self.create(owner, painter, &w) {
                keep.push(w);
            } else {
                fallback.extend(w.shapes);
            }
        }
        (keep, fallback)
    }

    fn create(&mut self, owner: &Window, painter: &mut egui_wgpu::winit::Painter, w: &Wanted) -> bool {
        let ppp = owner.scale_factor() as f32;
        let (origin, size) = physical_rect(w.popup.rect, ppp);
        let Ok(inner) = owner.inner_position() else { return false };
        #[allow(unused_mut)]
        let mut attrs = winit::window::WindowAttributes::default()
            .with_title("")
            .with_decorations(false)
            .with_resizable(false)
            .with_visible(false)
            .with_active(false)
            .with_position(winit::dpi::PhysicalPosition::new(inner.x + origin.0, inner.y + origin.1))
            .with_inner_size(winit::dpi::PhysicalSize::new(size.0, size.1));
        #[cfg(windows)]
        {
            use winit::platform::windows::WindowAttributesExtWindows;
            attrs = attrs.with_skip_taskbar(true);
            if let Some(h) = sharex_capture::platform::window_handle(owner) {
                attrs = attrs.with_owner_window(h as _);
            }
        }
        let Some(Ok(window)) = with_event_loop(|el| el.create_window(attrs)) else { return false };
        // A tool window with no open/close animation that never activates.
        sharex_capture::platform::prepare_overlay(&window);
        no_activate(&window);
        let window = Arc::new(window);
        self.next += 1;
        let viewport = egui::ViewportId::from_hash_of(("sharexrs-popup", owner.id(), self.next));
        if pollster::block_on(painter.set_window(viewport, Some(window.clone()))).is_err() {
            return false;
        }
        set_region(&window, size, w.popup.corner_radius, ppp);
        self.windows.push(PopupWindow { window, viewport, layer: w.popup.layer, origin, size, radius: w.popup.corner_radius, shown: false });
        true
    }

    /// Places, draws and shows the wanted popups (after the owner's frame
    /// is painted, so the textures are up to date) and closes the others.
    pub fn paint(&mut self, owner: &Window, painter: &mut egui_wgpu::winit::Painter, ctx: &egui::Context, ppp: f32, clear: [f32; 4], wanted: Vec<Wanted>) {
        let inner = owner.inner_position().ok();
        let mut live = Vec::new();
        for w in wanted {
            let Some(p) = self.windows.iter_mut().find(|p| p.layer == w.popup.layer) else { continue };
            live.push(p.layer);
            let (origin, size) = physical_rect(w.popup.rect, ppp);
            if size != p.size {
                p.size = size;
                let _ = p.window.request_inner_size(winit::dpi::PhysicalSize::new(size.0, size.1));
                if let (Some(sw), Some(sh)) = (std::num::NonZeroU32::new(size.0), std::num::NonZeroU32::new(size.1)) {
                    painter.on_window_resized(p.viewport, sw, sh);
                }
                set_region(&p.window, size, w.popup.corner_radius, ppp);
            } else if w.popup.corner_radius != p.radius {
                set_region(&p.window, size, w.popup.corner_radius, ppp);
            }
            p.radius = w.popup.corner_radius;
            if let Some(inner) = inner
                && (origin != p.origin || !p.shown)
            {
                p.origin = origin;
                p.window.set_outer_position(winit::dpi::PhysicalPosition::new(inner.x + origin.0, inner.y + origin.1));
            }
            // The layer's shapes, moved to the popup window's origin.
            let shift = egui::vec2(origin.0 as f32 / ppp, origin.1 as f32 / ppp);
            let shapes: Vec<ClippedShape> = w
                .shapes
                .into_iter()
                .map(|mut c| {
                    c.clip_rect = c.clip_rect.translate(-shift);
                    c.shape.translate(-shift);
                    c
                })
                .collect();
            let primitives = ctx.tessellate(shapes, ppp);
            painter.paint_and_update_textures(p.viewport, ppp, clear, &primitives, &mut Default::default(), Vec::new(), &p.window);
            if !p.shown {
                p.shown = true;
                show_no_activate(&p.window);
            }
        }
        let before = self.windows.len();
        self.windows.retain(|p| live.contains(&p.layer));
        if self.windows.len() != before {
            let mut active: egui::ViewportIdSet = self.windows.iter().map(|p| p.viewport).collect();
            active.insert(egui::ViewportId::ROOT);
            painter.gc_viewports(&active);
        }
        self.update_routes(owner);
    }

    /// Re-registers where each popup's input goes.
    fn update_routes(&self, owner: &Window) {
        let (Ok(inner), size) = (owner.inner_position(), owner.inner_size()) else { return };
        let mut areas = vec![[inner.x, inner.y, size.width as i32, size.height as i32]];
        areas.extend(self.windows.iter().map(|p| [inner.x + p.origin.0, inner.y + p.origin.1, p.size.0 as i32, p.size.1 as i32]));
        ROUTES.with(|r| {
            let mut r = r.borrow_mut();
            r.retain(|_, route| route.owner != owner.id());
            for p in &self.windows {
                r.insert(p.window.id(), Route { owner: owner.id(), offset: (p.origin.0 as f64, p.origin.1 as f64), areas: areas.clone() });
            }
        });
    }

    /// Closes them all (the owner is going away).
    pub fn clear(&mut self, owner: WindowId) {
        self.windows.clear();
        ROUTES.with(|r| r.borrow_mut().retain(|_, route| route.owner != owner));
    }
}

/// `WS_EX_NOACTIVATE`: clicking the popup leaves the owner active.
#[cfg(windows)]
fn no_activate(window: &Window) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GWL_EXSTYLE, GetWindowLongPtrW, SetWindowLongPtrW, WS_EX_NOACTIVATE};
    let Some(h) = sharex_capture::platform::window_handle(window) else { return };
    // SAFETY: our own window.
    unsafe {
        let style = GetWindowLongPtrW(h as _, GWL_EXSTYLE);
        SetWindowLongPtrW(h as _, GWL_EXSTYLE, style | WS_EX_NOACTIVATE as isize);
    }
}

#[cfg(not(windows))]
fn no_activate(_window: &Window) {}

/// Shows the popup without activating it (`ShowWindow(SW_SHOWNOACTIVATE)`;
/// winit's `set_visible` activates).
#[cfg(windows)]
fn show_no_activate(window: &Window) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{SW_SHOWNOACTIVATE, ShowWindow};
    let Some(h) = sharex_capture::platform::window_handle(window) else { return };
    // SAFETY: our own window.
    unsafe { ShowWindow(h as _, SW_SHOWNOACTIVATE) };
}

#[cfg(not(windows))]
fn show_no_activate(window: &Window) {
    window.set_visible(true);
}

/// Cuts the popup window to its rounded corners (the popup's border is
/// drawn inside them), as the tray menu's levels are.
#[cfg(windows)]
fn set_region(window: &Window, size: (u32, u32), radius: f32, ppp: f32) {
    use windows_sys::Win32::Graphics::Gdi::{CreateRoundRectRgn, SetWindowRgn};
    let Some(h) = sharex_capture::platform::window_handle(window) else { return };
    let r = (radius * ppp).round() as i32;
    // SAFETY: SetWindowRgn takes ownership of the region.
    unsafe {
        let region = CreateRoundRectRgn(0, 0, size.0 as i32 + 1, size.1 as i32 + 1, r * 2, r * 2);
        SetWindowRgn(h as _, region, 1);
    }
}

#[cfg(not(windows))]
fn set_region(_window: &Window, _size: (u32, u32), _radius: f32, _ppp: f32) {}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{pos2, vec2};

    #[test]
    fn popups_past_the_client_area_leave_it() {
        // VID-02: the codec list of the 850x550 Video converter at 125%
        // hangs below the client; a popup inside stays in the window.
        let client = egui::Rect::from_min_size(pos2(0.0, 0.0), vec2(850.0, 550.0));
        let inside = egui::Rect::from_min_size(pos2(20.0, 100.0), vec2(200.0, 300.0));
        let below = egui::Rect::from_min_size(pos2(20.0, 300.0), vec2(200.0, 504.0));
        assert!(fits(client, inside));
        assert!(!fits(client, below));
        assert_eq!(physical_rect(below, 1.25), ((25, 375), (250, 630)));
        assert_eq!(physical_rect(egui::Rect::from_min_size(pos2(10.3, 0.0), vec2(1.0, 1.0)), 1.25), ((13, 0), (1, 1)));
    }

    #[test]
    fn only_popup_windows_are_routed() {
        assert!(route_event(WindowId::from(u64::MAX), &WindowEvent::RedrawRequested).is_none());
    }
}
