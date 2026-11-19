//! The tray menu on Windows: `MainWindow.ShowTrayMenu`. ShareX opens its
//! compact `ContextMenu` (`BuildTrayMenu`) from an invisible anchor window
//! at the pointer (`CreateTrayMenuAnchor`), `BottomEdgeAlignedLeft`, kept
//! on the monitor's work area; each menu level is its own popup window.
//!
//! Here one borderless, topmost tool window covers the work area of the
//! monitor under the pointer, draws the menu and its submenus with
//! `sharex_ui::menu` and is clipped (a window region) to the levels drawn,
//! so everywhere else clicks go to the windows below, and the menu closes
//! when the window loses activation (light dismiss), as the anchor does.

use sharex_capture::{Rect as PxRect, Screen};
use sharex_ui::gpu::Gpu;
use sharex_ui::menu::{Flyout, FlyoutOutput, MenuEntry, Placement};
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{WindowAttributes, WindowId, WindowLevel};

use crate::main_menu::MenuCommand;
use crate::window::EguiWindow;

/// The anchor window's size: ShareX asks for 1x1, and Windows makes it
/// 32x39 (measured at 100%: `SM_CYMIN` tall), which is what the menu is
/// placed against.
const ANCHOR_SIZE: (f32, f32) = (32.0, 39.0);

pub struct TrayMenu<C: Clone = MenuCommand> {
    window: EguiWindow,
    flyout: Flyout<C>,
    /// The levels the window region was last cut to.
    shown: Vec<egui::Rect>,
    /// Losing the activation closes it (the tray menu); the quick task
    /// menu closes only on an explicit action.
    light_dismiss: bool,
}

/// What the menu did.
pub struct Output<C = MenuCommand> {
    pub command: Option<C>,
    pub closed: bool,
}

impl<C> Default for Output<C> {
    fn default() -> Self {
        Self { command: None, closed: false }
    }
}

impl<C: Clone> TrayMenu<C> {
    /// Opens `entries` at `cursor` (desktop pixels) on its monitor's work
    /// area; `open` is a submenu path to open at once
    /// (`TrayAutoExpandCaptureMenu` opens Capture).
    pub fn open(
        event_loop: &ActiveEventLoop,
        gpu: &Gpu,
        screens: &[Screen],
        cursor: (i32, i32),
        entries: Vec<MenuEntry<C>>,
        theme: egui::Theme,
        open: &[usize],
    ) -> Result<Self, String> {
        let screen = sharex_capture::screenshot::screen_from_point(screens, cursor).ok_or("no monitors found")?;
        let work: PxRect = screen.work_area;
        #[allow(unused_mut)]
        let mut attrs = WindowAttributes::default()
            .with_title("")
            .with_decorations(false)
            .with_resizable(false)
            .with_visible(false)
            .with_window_level(WindowLevel::AlwaysOnTop)
            .with_position(PhysicalPosition::new(work.x, work.y))
            .with_inner_size(PhysicalSize::new(work.w as u32, work.h as u32));
        #[cfg(windows)]
        {
            use winit::platform::windows::WindowAttributesExtWindows;
            attrs = attrs.with_skip_taskbar(true);
        }
        let window = event_loop.create_window(attrs).map_err(|e| e.to_string())?;
        // A tool window with no open/close animation, as the overlays.
        sharex_capture::platform::prepare_overlay(&window);
        let w = EguiWindow::new(window, gpu, None)?;
        crate::set_theme(&w, theme);
        let scale = w.window.scale_factor() as f32;
        let at = egui::pos2((cursor.0 - work.x) as f32 / scale, (cursor.1 - work.y) as f32 / scale);
        let anchor = egui::Rect::from_min_size(at, egui::vec2(ANCHOR_SIZE.0, ANCHOR_SIZE.1) / scale);
        let mut flyout = Flyout::menu(egui::Id::new("tray-menu"), entries, anchor, Placement::BottomEdgeAlignedLeft);
        flyout.edit_label = sharex_strings::sharex::MainMenuBuilder_EditTools().to_string();
        if !open.is_empty() {
            flyout.open_path(open);
        }
        let mut me = Self { window: w, flyout, shown: Vec::new(), light_dismiss: true };
        // Laid out once to know where the menu goes, then drawn, then shown.
        me.paint();
        me.paint();
        me.window.window.set_visible(true);
        me.window.window.focus_window();
        me.window.window.request_redraw();
        Ok(me)
    }

    /// `QuickTaskMenu.ShowMenu`: the menu with its first item centred on
    /// `cursor` (`popup.HorizontalOffset/VerticalOffset = -center`), slid
    /// to stay on the work area, closed only by its own items or Escape.
    pub fn open_centred(event_loop: &ActiveEventLoop, gpu: &Gpu, screens: &[Screen], cursor: (i32, i32), entries: Vec<MenuEntry<C>>, theme: egui::Theme) -> Result<Self, String> {
        let mut me = Self::open(event_loop, gpu, screens, cursor, entries, theme, &[])?;
        me.light_dismiss = false;
        me.flyout.style = sharex_ui::menu::MenuStyle::CompactPlainSeparators;
        let scale = me.window.window.scale_factor() as f32;
        let screen = sharex_capture::screenshot::screen_from_point(screens, cursor).ok_or("no monitors found")?;
        let at = egui::pos2((cursor.0 - screen.work_area.x) as f32 / scale, (cursor.1 - screen.work_area.y) as f32 / scale);
        me.flyout.set_placement(Placement::Slide(at));
        me.paint();
        if let Some(root) = me.flyout.rects().first().copied() {
            // The first item's centre: the border, the menu's padding and
            // Fluent's 4px scroller margin above a menu item's height.
            let d = sharex_ui::theme::density(me.window.ctx());
            let centre = egui::vec2((root.width() / 2.0).round(), 1.0 + d.menu_padding.top + 4.0 + d.menu_item_height / 2.0);
            me.flyout.set_placement(Placement::Slide(at - centre));
            me.paint();
            me.paint();
        }
        me.window.window.request_redraw();
        Ok(me)
    }

    /// A `ContextMenu` opened at the pointer (`Placement::Pointer`), in
    /// the plain `ContextMenu MenuItem` style: the Actions toolbar's title menu.
    pub fn open_context(event_loop: &ActiveEventLoop, gpu: &Gpu, screens: &[Screen], cursor: (i32, i32), entries: Vec<MenuEntry<C>>, theme: egui::Theme) -> Result<Self, String> {
        let mut me = Self::open(event_loop, gpu, screens, cursor, entries, theme, &[])?;
        let scale = me.window.window.scale_factor() as f32;
        let screen = sharex_capture::screenshot::screen_from_point(screens, cursor).ok_or("no monitors found")?;
        let at = egui::pos2((cursor.0 - screen.work_area.x) as f32 / scale, (cursor.1 - screen.work_area.y) as f32 / scale);
        me.flyout.style = sharex_ui::menu::MenuStyle::Standard;
        me.flyout.set_placement(Placement::Pointer(at));
        me.paint();
        me.paint();
        me.window.window.request_redraw();
        Ok(me)
    }

    pub fn owns(&self, id: WindowId) -> bool {
        self.window.window.id() == id
    }

    pub fn repaint_at(&self) -> Option<std::time::Instant> {
        self.window.repaint_at
    }

    /// Asks for the repaint egui wanted by `now`, once.
    pub fn redraw_if_due(&mut self, now: std::time::Instant) {
        if self.window.repaint_at.is_some_and(|at| at <= now) {
            self.window.window.request_redraw();
            self.window.repaint_at = None;
        }
    }

    fn paint(&mut self) -> Output<C> {
        let flyout = &mut self.flyout;
        let FlyoutOutput { command, closed } = self.window.paint(|ui| {
            let bounds = ui.max_rect();
            flyout.show(ui, bounds)
        });
        let rects = self.flyout.rects().to_vec();
        if rects != self.shown {
            set_region(&self.window.window, &rects);
            self.shown = rects;
        }
        Output { command, closed }
    }

    pub fn window_event(&mut self, event: WindowEvent) -> Output<C> {
        match event {
            WindowEvent::RedrawRequested => self.paint(),
            // `OnTrayMenuAnchorDeactivated`.
            WindowEvent::Focused(false) if self.light_dismiss => Output { command: None, closed: true },
            WindowEvent::CloseRequested => Output { command: None, closed: true },
            other => {
                self.window.on_event(&other);
                Output::default()
            }
        }
    }
}

/// Cuts the window to the menu's levels (rounded as they're drawn: 6px for
/// the root, 5 for submenus), so it shows and takes the pointer only there.
#[cfg(windows)]
fn set_region(window: &winit::window::Window, rects: &[egui::Rect]) {
    use windows_sys::Win32::Graphics::Gdi::{CombineRgn, CreateRectRgn, CreateRoundRectRgn, DeleteObject, RGN_OR, SetWindowRgn};
    let Some(hwnd) = sharex_capture::platform::window_handle(window) else { return };
    let s = window.scale_factor() as f32;
    // SAFETY: regions created here; SetWindowRgn takes ownership of the
    // combined one, the others are deleted.
    unsafe {
        let region = CreateRectRgn(0, 0, 0, 0);
        for (i, r) in rects.iter().enumerate() {
            let radius = (if i == 0 { 6.0 } else { 5.0 } * s).round() as i32;
            let (l, t) = ((r.min.x * s).round() as i32, (r.min.y * s).round() as i32);
            let (rr, b) = ((r.max.x * s).round() as i32, (r.max.y * s).round() as i32);
            let part = CreateRoundRectRgn(l, t, rr + 1, b + 1, radius * 2, radius * 2);
            CombineRgn(region, region, part, RGN_OR);
            DeleteObject(part);
        }
        SetWindowRgn(hwnd as _, region, 1);
    }
}

#[cfg(not(windows))]
fn set_region(_window: &winit::window::Window, _rects: &[egui::Rect]) {}
