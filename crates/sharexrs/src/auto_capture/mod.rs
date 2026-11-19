//! Capture › Auto capture (`ShareX/Presentation/AutoCapture`:
//! `AutoCaptureWindowIntegration`, `AutoCaptureWindow`, and
//! `TaskHelpers.OpenAutoCapture`/`StartAutoCapture`/`StopAutoCapture`).
//!
//! One window ([`window`]) at a time. While it runs, the app ticks its
//! timers ([`timer`]): every repeat time it grabs `AutoCaptureRegion` and
//! runs the image task (`UploadManager.RunImageTask(bitmap, TaskSettings,
//! true, true)`) without the annotate/photo editor tasks, sounds or toasts.
//! Minimized to the tray, the window hides and a clock icon in the tray
//! brings it back.

pub mod render;
pub mod timer;
pub mod window;

use std::sync::Arc;
use std::time::Instant;

use sharex_settings::types::Rectangle;
use sharex_settings::{AfterCaptureTasks, TaskSettings};
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::WindowId;

use crate::App;
use crate::region_session::RegionSession;
use crate::tray::{self, TrayEvent};
use window::AutoCaptureWindow;

/// `AutoCaptureWindowIntegration._window`, its region capture and its
/// tray icon.
#[derive(Default)]
pub struct AutoCapture {
    window: Option<WindowId>,
    /// `SelectRegionAsync`'s overlay.
    selecting: Option<Box<RegionSession>>,
    /// `_trayIcon` while the window is hidden to the tray.
    tray: Option<tray::Tray>,
}

impl App {
    fn auto_window(&mut self) -> Option<(WindowId, &mut AutoCaptureWindow)> {
        let id = self.auto_capture.window?;
        let d = self.dialogs.iter_mut().find(|d| d.window.window.id() == id)?;
        let w = d.dialog.as_auto_capture()?;
        Some((id, w))
    }

    /// `GetOrCreateWindow`, then `TaskSettings = taskSettings` and
    /// `ShowAndActivate`.
    fn auto_capture_show(&mut self, event_loop: &ActiveEventLoop, task: TaskSettings) -> Option<WindowId> {
        if self.auto_window().is_none() {
            let before = self.dialogs.len();
            let w = AutoCaptureWindow::new(task.clone(), &self.settings.settings);
            self.open_dialog_window(event_loop, Box::new(w));
            if self.dialogs.len() == before {
                return None;
            }
            self.auto_capture.window = self.dialogs.last().map(|d| d.window.window.id());
        }
        let (id, w) = self.auto_window()?;
        w.task = task;
        self.auto_capture_tray(event_loop, id, false);
        self.restore_and_activate(id);
        Some(id)
    }

    /// `TaskHelpers.OpenAutoCapture` (`AutoCaptureWindowIntegration.Show`).
    pub(crate) fn open_auto_capture(&mut self, event_loop: &ActiveEventLoop, task: TaskSettings) {
        self.auto_capture_show(event_loop, task);
    }

    /// `StartAutoCapture`: shown and started unless it runs.
    pub(crate) fn start_auto_capture(&mut self, event_loop: &ActiveEventLoop, task: TaskSettings) {
        if self.auto_window().is_some_and(|(_, w)| w.is_running()) {
            return;
        }
        let Some(id) = self.auto_capture_show(event_loop, task) else { return };
        let effects = match self.dialogs.iter_mut().find(|d| d.window.window.id() == id).and_then(|d| d.dialog.as_auto_capture()) {
            Some(w) => w.execute(&self.settings.settings),
            None => Vec::new(),
        };
        self.dialog_effects(event_loop, id, effects);
    }

    /// `StopAutoCapture`: stopped if it runs.
    pub(crate) fn stop_auto_capture(&mut self, event_loop: &ActiveEventLoop) {
        let settings = self.settings.settings.clone();
        let Some((id, w)) = self.auto_window() else { return };
        if w.is_running() {
            let effects = w.execute(&settings);
            self.dialog_effects(event_loop, id, effects);
            if let Some(d) = self.dialogs.iter().find(|d| d.window.window.id() == id) {
                d.window.window.request_redraw();
            }
        }
    }

    pub(crate) fn auto_capture_window_closed(&mut self, id: WindowId) {
        if self.auto_capture.window == Some(id) {
            self.auto_capture.window = None;
            self.auto_capture.selecting = None;
        }
    }

    /// `SelectRegionAsync`: the region capture with the task's region
    /// options, select only.
    pub(crate) fn auto_capture_select_region(&mut self, event_loop: &ActiveEventLoop) {
        if self.auto_capture.selecting.is_some() {
            return;
        }
        let Some((_, w)) = self.auto_window() else { return };
        let mut task = w.task.clone();
        let defaults = &self.settings.settings.DefaultTaskSettings;
        task.CaptureSettings = Some(crate::scrolling_capture::capture_settings(&task, defaults));
        task.UseDefaultCaptureSettings = false;
        let dark = self.theme() == egui::Theme::Dark;
        let opened = crate::capture_job::prepare_selection(&task, self.last_regions.clone()).and_then(|request| {
            let gpu = match &self.gpu {
                Some(g) => g,
                None => self.gpu.insert(sharex_ui::gpu::Gpu::new()?),
            };
            RegionSession::open(event_loop, gpu, request, dark)
        });
        match opened {
            Ok(s) => self.auto_capture.selecting = Some(Box::new(s)),
            Err(e) => eprintln!("auto capture: {e}"),
        }
    }

    /// `HideToTray` (`true`) / the tray icon hidden (`RestoreFromTray`, `OnClosed`).
    pub(crate) fn auto_capture_tray(&mut self, _event_loop: &ActiveEventLoop, id: WindowId, hide: bool) {
        if hide {
            if let Some(d) = self.dialogs.iter().find(|d| d.window.window.id() == id) {
                d.window.window.set_visible(false);
            }
            if self.auto_capture.tray.is_none() {
                let proxy = self.proxy.clone();
                let callback: tray::Callback = Arc::new(move |e| {
                    let _ = proxy.send_event(crate::UserEvent::AutoCaptureTray(e));
                });
                match tray::Tray::new(sharex_strings::sharex::AutoCaptureWindow_Title(), false, callback) {
                    #[allow(unused_mut)]
                    Ok(mut t) => {
                        // `TrayIconImage.Bind(_trayIcon, AppIcons.Clock)`.
                        let size = tray::Tray::icon_size();
                        let color = if sharex_capture::platform::light_taskbar() { egui::Color32::BLACK } else { egui::Color32::WHITE };
                        if let Some(image) = sharex_ui::icons::image(sharex_ui::keys::app_icons::Clock, size, color) {
                            t.set_icon(&image);
                        }
                        self.auto_capture.tray = Some(t);
                    }
                    Err(e) => eprintln!("couldn't add the auto capture tray icon: {e}"),
                }
            }
            #[allow(unused_mut)]
            if let Some(mut t) = self.auto_capture.tray.as_mut() {
                t.set_visible(true);
            }
        } else {
            self.auto_capture.tray = None;
        }
    }

    /// The tray icon clicked: `RestoreFromTray`.
    pub(crate) fn auto_capture_tray_event(&mut self, event: TrayEvent) {
        if matches!(event, TrayEvent::Up(tray::Button::Left) | TrayEvent::Up(tray::Button::Middle) | TrayEvent::Up(tray::Button::Right)) {
            self.auto_capture.tray = None;
            if let Some(id) = self.auto_capture.window {
                self.restore_and_activate(id);
            }
        }
    }

    /// The region overlay's events; false when `id` isn't one of its windows.
    pub(crate) fn auto_capture_window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: &WindowEvent) -> bool {
        let Some(session) = self.auto_capture.selecting.as_mut().filter(|s| s.owns(id)) else {
            // `OnWindowPropertyChanged`: minimized with minimize to tray on, it hides.
            if let WindowEvent::Resized(size) = event
                && size.width == 0
                && Some(id) == self.auto_capture.window
                && self.settings.settings.AutoCaptureMinimizeToTray
                && self.dialogs.iter().any(|d| d.window.window.id() == id && d.window.window.is_minimized() == Some(true))
            {
                self.auto_capture_tray(event_loop, id, true);
            }
            return false;
        };
        match session.window_event(event_loop, id, event.clone()) {
            sharex_capture::region::Outcome::Continue => {}
            sharex_capture::region::Outcome::Cancelled => self.auto_capture.selecting = None,
            sharex_capture::region::Outcome::Captured(result) => {
                self.auto_capture.selecting = None;
                self.last_regions = result.regions.clone();
                let r = result.screen_rectangle;
                self.settings.settings.AutoCaptureRegion = Rectangle::new(r.x, r.y, r.w, r.h);
                if let Some(id) = self.auto_capture.window
                    && let Some(d) = self.dialogs.iter().find(|d| d.window.window.id() == id)
                {
                    d.window.window.request_redraw();
                }
            }
        }
        true
    }

    /// The timers: a screenshot when due (`OnScreenshotTimerTick`,
    /// `TakeScreenshot`), the status every 250 ms; when to wake next.
    pub(crate) fn auto_capture_tick(&mut self, _event_loop: &ActiveEventLoop) -> Option<Instant> {
        let selecting = self.auto_capture.selecting.as_mut().and_then(|s| s.wake_at());
        let busy = self.state.tasks.manager.tasks.iter().any(|t| t.is_busy());
        let now = Instant::now();
        let region = self.settings.settings.AutoCaptureRegion;
        let Some((id, w)) = self.auto_window() else { return selecting };
        let shoot = w.timer.tick(now, busy);
        let mut redraw = false;
        if w.timer.status_due(now) {
            w.update_status(now);
            redraw = true;
        }
        let task = shoot.then(|| w.task.clone());
        let wake = w.timer.wake_at();
        if let Some(mut task) = task {
            redraw = true;
            // `TakeScreenshot`.
            if !timer::is_empty(region) {
                let rect = sharex_capture::Rect::new(region.x, region.y, region.width, region.height);
                let screens = sharex_capture::screenshot::screens();
                let capture = task.CaptureSettings.clone().unwrap_or_default();
                let shot = sharex_capture::screenshot::Screenshot { capture_cursor: capture.ShowCursor, ..Default::default() };
                if let Some(image) = shot.capture_rectangle(&screens, rect) {
                    task.AfterCaptureJob.remove(AfterCaptureTasks::AnnotateImage);
                    task.AfterCaptureJob.remove(AfterCaptureTasks::OpenInPhotoEditor);
                    let g = task.GeneralSettings.get_or_insert_default();
                    g.PlaySoundAfterUpload = false;
                    g.PlaySoundAfterAction = false;
                    g.ShowToastNotificationAfterTaskCompleted = false;
                    if let Some((_, w)) = self.auto_window() {
                        w.task = task.clone();
                    }
                    self.run_image_task_for(image, task);
                }
            }
        }
        if redraw && let Some(d) = self.dialogs.iter().find(|d| d.window.window.id() == id) {
            d.window.window.request_redraw();
        }
        [selecting, wake].into_iter().flatten().min()
    }
}
