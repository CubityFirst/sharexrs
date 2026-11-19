//! Capture › Scrolling capture (`ShareX.ScreenCaptureLib`'s
//! `ScrollingCaptureWindowIntegration`, `ScrollingCaptureService`,
//! `ScrollingCaptureManager.StartCapture`/`SelectWindowAsync`,
//! `ScrollingCaptureRegionWindow`, and `TaskHelpers.OpenScrollingCapture`).
//!
//! The window ([`window`]) asks for a window to capture: it minimizes, and
//! 250 ms later the region capture overlay opens (select only, the default
//! region options but the task's active monitor mode). The rectangle and the
//! window picked go to [`sharex_capture::scrolling::run_capture`] on a
//! thread, with the region's lime outline shown around it
//! (`ScrollingCaptureRegionWindow`); the result comes back to the window.

pub mod render;
pub mod window;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use sharex_capture::Rect as PxRect;
use sharex_capture::scrolling::{CaptureOutcome, ScreenTarget, ScrollingCaptureOptions};
use sharex_settings::TaskSettings;
use sharex_ui::gpu::Gpu;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{WindowAttributes, WindowId, WindowLevel};

use crate::App;
use crate::recording::frame::{self, Geometry};
use crate::region_session::RegionSession;
use crate::window::EguiWindow;
use window::{Event, ScrollingCaptureWindow};

/// `RecordingRegionBorder.AccentBrush="Lime"`.
const LIME: egui::Color32 = egui::Color32::from_rgb(0x00, 0xFF, 0x00);

/// Where a scrolling capture is.
#[derive(Default)]
enum Phase {
    #[default]
    Idle,
    /// `SelectWindowAsync`'s `Task.Delay(250)` after minimizing.
    Waiting(Instant),
    /// The region capture overlay is open.
    Selecting(Box<RegionSession>),
    /// `StartCapture`'s loop runs on a thread.
    Capturing(Box<Running>),
}

struct Running {
    stop: Arc<AtomicBool>,
    rx: mpsc::Receiver<CaptureOutcome>,
    /// `ScrollingCaptureRegionWindow` (with `ShowRegion`).
    outline: Option<(EguiWindow, Geometry)>,
}

/// `ScrollingCaptureWindowIntegration._window` and its capture.
#[derive(Default)]
pub struct ScrollingCapture {
    window: Option<WindowId>,
    /// The task the window was opened for (`uploadRequested`'s settings,
    /// the region options).
    task: Option<TaskSettings>,
    phase: Phase,
}

/// `TaskSettings.CaptureSettingsReference`: the task's own capture
/// settings, or the default task's when it uses those.
pub fn capture_settings(task: &TaskSettings, defaults: &TaskSettings) -> sharex_settings::task_settings::TaskSettingsCapture {
    let c = if task.UseDefaultCaptureSettings { defaults.CaptureSettings.as_ref() } else { task.CaptureSettings.as_ref() };
    c.or(defaults.CaptureSettings.as_ref()).cloned().unwrap_or_default()
}

impl App {
    /// `TaskHelpers.OpenScrollingCapture` / `ScrollingCaptureWindowIntegration
    /// .StartStopAsync`: the window opens (and selects a window at once),
    /// or the open one starts or stops.
    pub(crate) fn open_scrolling_capture(&mut self, event_loop: &ActiveEventLoop, task: TaskSettings) {
        if let Some(id) = self.scrolling.window.filter(|id| self.dialogs.iter().any(|d| d.window.window.id() == *id)) {
            let effects = self.with_scrolling_window(id, |w| w.start_stop());
            self.dialog_effects(event_loop, id, effects);
            return;
        }
        let defaults = &self.settings.settings.DefaultTaskSettings;
        let options = ScrollingCaptureOptions::from_json(&capture_settings(&task, defaults).ScrollingCaptureOptions);
        let before = self.dialogs.len();
        self.open_dialog_window(event_loop, Box::new(ScrollingCaptureWindow::new(options, task.clone())));
        if self.dialogs.len() == before {
            return;
        }
        let Some(d) = self.dialogs.last() else { return };
        let id = d.window.window.id();
        // `WindowState = Minimized` in the constructor; `OnOpened` starts.
        d.window.window.set_minimized(true);
        self.scrolling = ScrollingCapture { window: Some(id), task: Some(task), phase: Phase::Idle };
        let effects = self.with_scrolling_window(id, |w| w.start_stop());
        self.dialog_effects(event_loop, id, effects);
    }

    fn with_scrolling_window(&mut self, id: WindowId, f: impl FnOnce(&mut ScrollingCaptureWindow) -> Vec<crate::dialogs::Effect>) -> Vec<crate::dialogs::Effect> {
        let Some(d) = self.dialogs.iter_mut().find(|d| d.window.window.id() == id) else { return Vec::new() };
        let Some(w) = d.dialog.as_scrolling() else { return Vec::new() };
        let effects = f(w);
        d.window.window.request_redraw();
        effects
    }

    /// The window's `SelectWindowAsync`: minimized, 250 ms, then the overlay.
    pub(crate) fn scrolling_select(&mut self, id: WindowId) {
        if let Some(d) = self.dialogs.iter().find(|d| d.window.window.id() == id) {
            d.window.window.set_minimized(true);
        }
        self.scrolling.window = Some(id);
        self.scrolling.phase = Phase::Waiting(Instant::now() + Duration::from_millis(250));
    }

    /// `ScrollingCaptureService.StopCapture`.
    pub(crate) fn scrolling_stop(&mut self) {
        if let Phase::Capturing(r) = &self.scrolling.phase {
            r.stop.store(true, Ordering::SeqCst);
        }
    }

    /// The options overlay saved: the task's options are the ones edited
    /// (`CaptureSettingsReference.ScrollingCaptureOptions`).
    pub(crate) fn scrolling_options_saved(&mut self, options: ScrollingCaptureOptions) {
        let uses_defaults = self.scrolling.task.as_ref().is_none_or(|t| t.UseDefaultCaptureSettings);
        if uses_defaults {
            let c = self.settings.settings.DefaultTaskSettings.CaptureSettings.get_or_insert_default();
            options.write_json(&mut c.ScrollingCaptureOptions);
        } else if let Some(t) = self.scrolling.task.as_mut() {
            options.write_json(&mut t.CaptureSettings.get_or_insert_default().ScrollingCaptureOptions);
        }
    }

    /// The window closed.
    pub(crate) fn scrolling_window_closed(&mut self, id: WindowId) {
        if self.scrolling.window == Some(id) {
            self.scrolling_stop();
            self.scrolling.window = None;
        }
    }

    /// `PlayNotificationSoundAsync(NotificationSound.ActionCompleted, taskSettings)`.
    pub(crate) fn scrolling_sound(&self) {
        if self.scrolling.task.as_ref().and_then(|t| t.GeneralSettings.as_ref()).is_none_or(|g| g.PlaySoundAfterAction) {
            crate::sound::play_action_completed();
        }
    }

    /// Sends `event` to the window.
    fn scrolling_event(&mut self, event_loop: &ActiveEventLoop, event: Event) {
        let Some(id) = self.scrolling.window else { return };
        let effects = self.with_scrolling_window(id, |w| w.event(event));
        self.dialog_effects(event_loop, id, effects);
    }

    /// `SelectWindowAsync`'s region capture: the default region options with
    /// the task's active monitor mode, select only.
    fn scrolling_open_overlay(&mut self, event_loop: &ActiveEventLoop) {
        let defaults = &self.settings.settings.DefaultTaskSettings;
        let mut task = self.scrolling.task.clone().unwrap_or_else(|| TaskSettings::default_task_settings(defaults));
        let mut capture = capture_settings(&task, defaults);
        let active = capture.RegionCaptureOptions.ActiveMonitorMode;
        capture.RegionCaptureOptions = sharex_settings::RegionCaptureOptions { ActiveMonitorMode: active, ..Default::default() };
        task.CaptureSettings = Some(capture);
        task.UseDefaultCaptureSettings = false;
        let dark = self.theme() == egui::Theme::Dark;
        let gpu = match &self.gpu {
            Some(g) => Ok(g),
            None => Gpu::new().map(|g| &*self.gpu.insert(g)),
        };
        let opened = gpu.and_then(|gpu| {
            crate::capture_job::prepare_selection_with(&task, Vec::new(), |screens| {
                crate::region_session::ready_overlays(event_loop, gpu, screens)
            })
            .and_then(|request| RegionSession::open(event_loop, gpu, request, dark))
        });
        match opened {
            Ok(session) => self.scrolling.phase = Phase::Selecting(Box::new(session)),
            Err(e) => {
                eprintln!("scrolling capture: {e}");
                self.scrolling.phase = Phase::Idle;
                self.scrolling_event(event_loop, Event::SelectionCancelled);
            }
        }
    }

    /// A window and rectangle are selected: `CaptureSelectedWindowAsync`
    /// and `StartCapture` (the outline, then the loop on a thread).
    fn scrolling_start(&mut self, event_loop: &ActiveEventLoop, handle: isize, rect: PxRect) {
        self.scrolling_event(event_loop, Event::CaptureStarted);
        let Some(id) = self.scrolling.window else { return };
        let options = self
            .dialogs
            .iter_mut()
            .find(|d| d.window.window.id() == id)
            .and_then(|d| d.dialog.as_scrolling().map(|w| w.options.clone()))
            .unwrap_or_default();
        let outline = if options.show_region {
            match self.scrolling_outline(event_loop, rect) {
                Ok(o) => Some(o),
                Err(e) => {
                    eprintln!("scrolling capture region window: {e}");
                    None
                }
            }
        } else {
            None
        };
        let stop = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        let flag = stop.clone();
        let proxy = self.proxy.clone();
        std::thread::spawn(move || {
            let mut target = ScreenTarget { handle, rect };
            let outcome = sharex_capture::scrolling::run_capture(&options, &mut target, &flag);
            let _ = tx.send(outcome);
            let _ = proxy.send_event(crate::UserEvent::Wake);
        });
        self.scrolling.phase = Phase::Capturing(Box::new(Running { stop, rx, outline }));
    }

    /// `new ScrollingCaptureRegionWindow(selectedRectangle)`: a 1px dashed
    /// lime frame around the region, click-through, topmost, not activated.
    fn scrolling_outline(&mut self, event_loop: &ActiveEventLoop, rect: PxRect) -> Result<(EguiWindow, Geometry), String> {
        let g = Geometry::new(rect, 0.0, 1.0, None);
        let at = g.position(rect);
        let size = (g.frame_width, g.frame_height);
        #[allow(unused_mut)]
        let mut attrs = WindowAttributes::default()
            .with_title(crate::rs_title(sharex_strings::screen_capture_lib::ScrollingCaptureWindow_Title()))
            .with_decorations(false)
            .with_resizable(false)
            .with_transparent(true)
            .with_visible(false)
            .with_active(false)
            .with_window_level(WindowLevel::AlwaysOnTop)
            .with_position(PhysicalPosition::new(at.0, at.1))
            .with_inner_size(PhysicalSize::new(size.0.max(1) as u32, size.1.max(1) as u32));
        #[cfg(windows)]
        {
            use winit::platform::windows::WindowAttributesExtWindows;
            attrs = attrs.with_skip_taskbar(true).with_no_redirection_bitmap(true);
        }
        let window = event_loop.create_window(attrs).map_err(|e| e.to_string())?;
        sharex_capture::platform::prepare_overlay(&window);
        let _ = window.set_cursor_hittest(false);
        let gpu = match &self.gpu {
            Some(g) => g,
            None => self.gpu.insert(Gpu::new()?),
        };
        let mut w = EguiWindow::transparent(window, gpu)?;
        crate::set_theme(&w, self.theme());
        sharex_capture::platform::set_window_region(
            &w.window,
            &[PxRect::new(0, 0, size.0, 1), PxRect::new(0, size.1 - 1, size.0, 1), PxRect::new(0, 0, 1, size.1), PxRect::new(size.0 - 1, 0, 1, size.1)],
        );
        w.paint(|ui| frame::paint_border(ui, &g, LIME));
        w.window.set_visible(true);
        Ok((w, g))
    }

    /// Routes an event to the scrolling capture's overlay or outline; false
    /// when `id` is neither.
    pub(crate) fn scrolling_window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: &WindowEvent) -> bool {
        match &mut self.scrolling.phase {
            Phase::Selecting(session) if session.owns(id) => {
                match session.window_event(event_loop, id, event.clone()) {
                    sharex_capture::region::Outcome::Continue => {}
                    sharex_capture::region::Outcome::Cancelled => {
                        self.scrolling.phase = Phase::Idle;
                        self.scrolling_event(event_loop, Event::SelectionCancelled);
                    }
                    sharex_capture::region::Outcome::Captured(result) => {
                        self.scrolling.phase = Phase::Idle;
                        // `selectedWindow != null`.
                        match result.window_info.as_ref().filter(|w| w.handle != 0) {
                            Some(w) => self.scrolling_start(event_loop, w.handle, result.screen_rectangle),
                            None => self.scrolling_event(event_loop, Event::SelectionCancelled),
                        }
                    }
                }
                true
            }
            Phase::Capturing(r) if r.outline.as_ref().is_some_and(|(w, _)| w.window.id() == id) => {
                if matches!(event, WindowEvent::RedrawRequested)
                    && let Some((w, g)) = r.outline.as_mut()
                {
                    let g = *g;
                    w.paint(|ui| frame::paint_border(ui, &g, LIME));
                }
                true
            }
            _ => false,
        }
    }

    /// Moves the capture on (the delay before the overlay, the loop's
    /// result); when to be woken next.
    pub(crate) fn scrolling_tick(&mut self, event_loop: &ActiveEventLoop) -> Option<Instant> {
        let now = Instant::now();
        match &mut self.scrolling.phase {
            Phase::Waiting(at) if *at <= now => {
                self.scrolling.phase = Phase::Idle;
                self.scrolling_open_overlay(event_loop);
                None
            }
            Phase::Waiting(at) => Some(*at),
            Phase::Selecting(session) => session.wake_at(),
            Phase::Capturing(r) => match r.rx.try_recv() {
                Ok(outcome) => {
                    // `finally`: the outline closes; the result goes back.
                    self.scrolling.phase = Phase::Idle;
                    self.scrolling_event(event_loop, Event::CaptureFinished(outcome.status, outcome.result));
                    None
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.scrolling.phase = Phase::Idle;
                    self.scrolling_event(event_loop, Event::CaptureFinished(Default::default(), None));
                    None
                }
                Err(mpsc::TryRecvError::Empty) => Some(now + Duration::from_millis(100)),
            },
            Phase::Idle => None,
        }
    }

    /// The window's `RestoreAndActivate`.
    pub(crate) fn restore_and_activate(&mut self, id: WindowId) {
        if let Some(d) = self.dialogs.iter().find(|d| d.window.window.id() == id) {
            d.window.window.set_minimized(false);
            d.window.window.set_visible(true);
            crate::window::activate(&d.window.window);
            d.window.window.request_redraw();
        }
    }
}
