//! Screen recording in the app: `ShareX/ScreenRecordManager.cs` with the
//! FFmpeg recorder (`crates/sharex-screen-recording`), its windows and the
//! hooks into `App` (see docs/SCREEN-RECORDING.md).
//!
//! A recording goes through phases: the area is chosen (the region
//! overlay in select-only mode, the active window, the custom region or
//! the last one), the recorder bar may review the settings, then the
//! recording frame (with its own tray icon) shows while a worker thread
//! runs [`run_ffmpeg_session`]; the finished file becomes a file task.
//!
//! ShareX's default recorder is its native one (`ScreenRecordUseNative`):
//! on Windows it runs `sharex_native_recorder` (Media Foundation H.264/AAC
//! or the adaptive GIF writer, `native.rs`), with FFmpeg grabbing the
//! screen for it until Windows Graphics Capture is ported; its sound and
//! camera wait for real WASAPI and camera capture. Elsewhere it falls back
//! to the FFmpeg recorder, as before.

pub mod bar;
pub mod ffmpeg_options;
pub mod frame;
mod native;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use sharex_capture::Rect as PxRect;
use sharex_screen_recording::discovery::{self, FFmpegCheckError};
use sharex_screen_recording::session::{
    FFmpegSession, PreparedRecording, RecordingControl, ScreenRecordOutput, ScreenRecordStartMethod, ScreenRecordState,
    ScreenRecordingStatus, SessionHooks, StartError, prepare_ffmpeg_recording, prepare_native_recording, run_ffmpeg_session,
};
use sharex_screen_recording::{DesktopScreen, RecorderInput};
use sharex_settings::types::Rectangle;
use sharex_settings::{ApplicationConfig, HotkeyType, TaskSettings, TaskSettingsCapture};
use sharex_strings::sharex as s;
use sharex_ui::gpu::Gpu;
use sharex_ui::menu::MenuEntry;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, NamedKey};
use winit::window::{WindowAttributes, WindowId, WindowLevel};

use crate::capture_job::{self, Metadata};
use crate::message_box::{MessageBox, MessageBoxIcon};
use crate::region_session::RegionSession;
use crate::tray::{self, TrayEvent};
use crate::tray_menu::TrayMenu;
use crate::window::EguiWindow;
use crate::{App, DialogPurpose, UserEvent};

use bar::{Bar, BarAction};
use frame::{Frame, FrameAction, Geometry, TrayIcon};

/// What the recording's worker thread and its tray icon tell the event loop.
#[derive(Debug)]
pub enum Event {
    /// `ChangeState` with the status after it.
    State(ScreenRecordState, ScreenRecordingStatus),
    /// `StartCountdown(milliseconds)`.
    Countdown(u32),
    /// `ChangeStateProgress`.
    Progress(i32),
    /// FFmpeg failed: its output.
    Error(String),
    /// `PlayNotificationSoundAsync(ActionCompleted)`.
    ActionCompleted,
    /// The worker ended: the recording's file, and the name pattern's
    /// counter after naming it.
    Done { path: Option<String>, counter: i32 },
    /// The frame's tray icon.
    Tray(TrayEvent),
    /// FFmpeg's download ended.
    FFmpegDownloaded,
    /// The native recorder failed: ShareX's message box.
    NativeError(String),
}

/// The frame's tray menu entries (`_trayStartItem` and the rest).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayCommand {
    StartStop,
    PauseResume,
    Restart,
    Abort,
}

impl TrayCommand {
    const ALL: [Self; 4] = [Self::StartStop, Self::PauseResume, Self::Restart, Self::Abort];

    /// The tray menu's entries do what the frame's buttons do; Abort asks
    /// first as the button does (`RequestAbortRecording`).
    fn action(self) -> FrameAction {
        match self {
            Self::StartStop => FrameAction::StartStop,
            Self::PauseResume => FrameAction::PauseResume,
            Self::Restart => FrameAction::Restart,
            Self::Abort => FrameAction::Abort,
        }
    }
}

/// `ScreenRecordManager`'s state while `IsRecording`.
pub struct Recording {
    output: ScreenRecordOutput,
    method: ScreenRecordStartMethod,
    task: TaskSettings,
    /// The hotkey whose settings the task came from (`TaskSettingsReference`),
    /// for committing the bar's edits.
    origin: Option<usize>,
    metadata: Metadata,
    /// The window recorded (active window recordings), for the bar's check.
    capture_window: isize,
    use_gif_fps: bool,
    phase: Phase,
}

enum Phase {
    /// Choosing the area (the region overlay is open, or nothing yet).
    Selecting(Option<Box<RegionSession>>),
    /// The recorder bar is open.
    Bar(Box<BarWindow>),
    /// Recording: the frame is open and the worker runs.
    Recording(Box<Active>),
}

/// The frame window, its tray icon and the worker's control.
struct Active {
    frame: FrameWindow,
    control: Arc<RecordingControl>,
    tray: Option<tray::Tray>,
    tray_shown: Option<(TrayIcon, String, bool)>,
    tray_menu: Option<TrayMenu<TrayCommand>>,
}

/// Whether a recording uses the native recorder: `ScreenRecordUseNative`,
/// on Windows (its encoders are Media Foundation's; elsewhere FFmpeg records).
fn uses_native(capture: &TaskSettingsCapture) -> bool {
    cfg!(windows) && capture.ScreenRecordUseNative
}

/// `StartRecording`'s checks for the recorder the settings choose.
fn prepare(output: ScreenRecordOutput, capture: &TaskSettingsCapture, area: Rectangle, bounds: Rectangle) -> Result<PreparedRecording, StartError> {
    let ffmpeg = discovery::resolve_ffmpeg(&capture.FFmpegOptions);
    if uses_native(capture) {
        prepare_native_recording(output, capture, area, bounds, ffmpeg)
    } else {
        prepare_ffmpeg_recording(output, capture, area, bounds, ffmpeg)
    }
}

impl App {
    /// `TaskHelpers.ExecuteJob` for the recorder jobs; false for others.
    /// `origin` is the hotkey the task's settings came from.
    pub(crate) fn run_recording_job(&mut self, event_loop: &ActiveEventLoop, job: &HotkeyType, task: &TaskSettings, origin: Option<usize>) -> bool {
        use ScreenRecordOutput::{FFmpeg, GIF};
        use ScreenRecordStartMethod::{ActiveWindow, CustomRegion, LastRegion, Region};
        let (output, method) = match job {
            HotkeyType::ScreenRecorder => (FFmpeg, Region),
            HotkeyType::ScreenRecorderActiveWindow => (FFmpeg, ActiveWindow),
            HotkeyType::ScreenRecorderCustomRegion => (FFmpeg, CustomRegion),
            HotkeyType::StartScreenRecorder => (FFmpeg, LastRegion),
            HotkeyType::ScreenRecorderGIF => (GIF, Region),
            HotkeyType::ScreenRecorderGIFActiveWindow => (GIF, ActiveWindow),
            HotkeyType::ScreenRecorderGIFCustomRegion => (GIF, CustomRegion),
            HotkeyType::StartScreenRecorderGIF => (GIF, LastRegion),
            HotkeyType::StopScreenRecording => {
                self.stop_recording();
                return true;
            }
            HotkeyType::PauseScreenRecording => {
                self.pause_screen_recording();
                return true;
            }
            HotkeyType::AbortScreenRecording => {
                self.abort_screen_recording();
                return true;
            }
            _ => return false,
        };
        self.start_stop_recording(event_loop, output, method, task.clone(), origin);
        true
    }

    /// `ScreenRecordManager.StartStopRecording`: while recording, the same
    /// job presses Record on the bar or Start/Stop on the frame.
    fn start_stop_recording(
        &mut self,
        event_loop: &ActiveEventLoop,
        output: ScreenRecordOutput,
        method: ScreenRecordStartMethod,
        task: TaskSettings,
        origin: Option<usize>,
    ) {
        match self.recording.as_ref().map(|r| &r.phase) {
            Some(Phase::Bar(_)) => self.bar_action(event_loop, BarAction::Record),
            Some(Phase::Recording(_)) => self.frame_action(FrameAction::StartStop),
            Some(Phase::Selecting(_)) => {}
            None => self.start_recording(event_loop, output, method, task, origin),
        }
    }

    /// `ScreenRecordManager.StopRecording` (the Stop screen recording job):
    /// cancels the bar, or stops FFmpeg leaving the frame's status alone.
    fn stop_recording(&mut self) {
        match self.recording.as_ref().map(|r| &r.phase) {
            Some(Phase::Bar(_)) => {
                self.close_bar(false);
            }
            Some(Phase::Recording(a)) => a.control.request_stop(),
            _ => {}
        }
    }

    /// `ScreenRecordManager.PauseScreenRecording`.
    fn pause_screen_recording(&mut self) {
        if matches!(self.recording.as_ref().map(|r| &r.phase), Some(Phase::Recording(_))) {
            self.frame_action(FrameAction::PauseResume);
        }
    }

    /// `ScreenRecordManager.AbortRecording`: no confirmation.
    fn abort_screen_recording(&mut self) {
        match self.recording.as_ref().map(|r| &r.phase) {
            Some(Phase::Bar(_)) => {
                self.close_bar(false);
            }
            Some(Phase::Recording(_)) => self.frame_action(FrameAction::ConfirmAbort),
            _ => {}
        }
    }

    /// `ScreenRecordManager.StartRecording` up to choosing the area.
    fn start_recording(
        &mut self,
        event_loop: &ActiveEventLoop,
        output: ScreenRecordOutput,
        method: ScreenRecordStartMethod,
        task: TaskSettings,
        origin: Option<usize>,
    ) {
        let capture = task.CaptureSettings.clone().unwrap_or_default();
        let screens = sharex_capture::screenshot::screens();
        let bounds = to_rectangle(sharex_capture::screenshot::screen_bounds(&screens));
        // The FFmpeg checks come first (`CheckFFmpeg`, the sources), before
        // any area is chosen.
        let prepared = match prepare(output, &capture, bounds, bounds) {
            Ok(p) => p,
            Err(e) => {
                self.recording_start_failed(event_loop, e, &capture);
                return;
            }
        };
        if uses_native(&capture) {
            let kind = if output == ScreenRecordOutput::GIF { "Adaptive GIF" } else { "H.264/AAC MP4" };
            eprintln!("Starting native screen recording. {kind}, FPS: {}", prepared.fps);
            eprintln!("Windows Graphics Capture isn't ported yet: FFmpeg grabs the screen for the native recorder.");
        } else {
            if capture.ScreenRecordUseNative {
                eprintln!("ShareX's native screen recorder is Windows only: recording with FFmpeg instead.");
            }
            eprintln!(
                "Starting screen recording. Video encoder: \"{}\", Audio encoder: \"{}\", FPS: {}",
                prepared.capture.FFmpegOptions.VideoCodec.description(),
                prepared.capture.FFmpegOptions.AudioCodec.description(),
                prepared.fps
            );
        }
        let mut recording = Recording {
            output,
            method,
            task,
            origin,
            metadata: Metadata { image: image::RgbaImage::new(0, 0), window_title: None, process_name: None },
            capture_window: 0,
            use_gif_fps: prepared.use_gif_fps,
            phase: Phase::Selecting(None),
        };
        let rect = match method {
            ScreenRecordStartMethod::Region => {
                // `RegionCaptureTasks.GetRectangleRegionAsync`: the overlay,
                // no annotations, no pointer drawn in.
                let dark = self.theme() == egui::Theme::Dark;
                let opened = capture_job::prepare_selection(&recording.task, self.last_regions.clone())
                    .and_then(|request| RegionSession::open(event_loop, gpu(&mut self.gpu)?, request, dark));
                match opened {
                    Ok(session) => {
                        recording.phase = Phase::Selecting(Some(Box::new(session)));
                        self.recording = Some(recording);
                    }
                    Err(e) => eprintln!("screen recording: {e}"),
                }
                return;
            }
            ScreenRecordStartMethod::ActiveWindow => {
                let handle = sharex_capture::platform::foreground_window();
                let rect = if capture.CaptureClientArea {
                    sharex_capture::platform::client_rect(handle)
                } else {
                    recording.capture_window = handle;
                    sharex_capture::platform::window_rect(handle)
                };
                let info = sharex_capture::platform::window_info(handle);
                recording.metadata.window_title = Some(info.title);
                recording.metadata.process_name = info.process_name;
                rect
            }
            ScreenRecordStartMethod::CustomRegion => from_rectangle(capture.CaptureCustomRegion),
            ScreenRecordStartMethod::LastRegion => from_rectangle(self.settings.settings.ScreenRecordRegion),
        };
        self.recording = Some(recording);
        self.area_chosen(event_loop, rect);
    }

    /// ShareX's message boxes for `StartRecording`'s early returns.
    fn recording_start_failed(&mut self, event_loop: &ActiveEventLoop, e: StartError, capture: &TaskSettingsCapture) {
        let message = match e {
            StartError::FFmpeg(FFmpegCheckError::Not64Bit) => {
                MessageBox::ok(format!("ShareX - {}", s::FFmpegIsMissing()), s::FFmpegOnlySupports64BitOperatingSystems(), MessageBoxIcon::Warning)
            }
            StartError::FFmpeg(FFmpegCheckError::Missing(path)) => {
                self.ffmpeg_missing(event_loop, path, capture);
                return;
            }
            StartError::NoSource => MessageBox::ok(
                format!("ShareX - {}", s::FFmpeg_FFmpeg_error()),
                s::FFmpeg_FFmpeg_video_and_audio_source_both_can_t_be__None__(),
                MessageBoxIcon::Warning,
            ),
            StartError::InvalidRegion => return,
        };
        if let Err(e) = self.open_dialog(event_loop, message, DialogPurpose::Information) {
            eprintln!("couldn't open the message box: {e}");
        }
    }

    /// `prepare_ffmpeg_recording` for the recording's settings at `rect`.
    fn prepare_at(&self, rect: PxRect) -> Option<Result<PreparedRecording, StartError>> {
        let r = self.recording.as_ref()?;
        let capture = r.task.CaptureSettings.clone().unwrap_or_default();
        let screens = sharex_capture::screenshot::screens();
        let bounds = to_rectangle(sharex_capture::screenshot::screen_bounds(&screens));
        Some(prepare(r.output, &capture, to_rectangle(rect), bounds))
    }

    /// The area is known: clipped to the screens and made even, then the
    /// bar or the recording. An empty area ends it quietly.
    fn area_chosen(&mut self, event_loop: &ActiveEventLoop, rect: PxRect) {
        let prepared = match self.prepare_at(rect) {
            Some(Ok(p)) => p,
            Some(Err(e)) => {
                if e != StartError::InvalidRegion {
                    eprintln!("screen recording: {e:?}");
                }
                self.recording = None;
                return;
            }
            None => return,
        };
        let rect = from_rectangle(prepared.capture_rectangle);
        if !prepared.capture.ScreenRecordShowBar {
            self.begin_recording(event_loop, prepared, rect);
            return;
        }
        let Some(r) = self.recording.as_ref() else { return };
        // `evenSize = useNative ? !isGif : FFmpegOptions.IsEvenSizeRequired`.
        let even = if uses_native(&prepared.capture) { !prepared.use_gif_fps } else { prepared.capture.FFmpegOptions.is_even_size_required() };
        let (gif, theme) = (r.use_gif_fps, self.current_theme);
        let screens = sharex_capture::screenshot::screens();
        let opened = gpu(&mut self.gpu).and_then(|g| BarWindow::open(event_loop, g, &screens, rect, prepared.capture.clone(), gif, even, theme));
        match opened {
            Ok(bar) => {
                if let Some(r) = self.recording.as_mut() {
                    r.phase = Phase::Bar(Box::new(bar));
                }
            }
            Err(e) => {
                eprintln!("the recorder bar: {e}");
                self.recording = None;
                let message = MessageBox::ok("ShareX", e, MessageBoxIcon::Error);
                let _ = self.open_dialog(event_loop, message, DialogPurpose::Information);
            }
        }
    }

    /// The bar closed: `Commit` (saved either way), then what to record
    /// when `record`.
    fn close_bar(&mut self, record: bool) -> Option<(PreparedRecording, PxRect)> {
        let r = self.recording.as_mut()?;
        let Phase::Bar(bar) = &r.phase else { return None };
        let draft = bar.bar.capture.clone();
        let region = bar.region;
        // `ScreenRecorderBarSettings.Commit`: onto the recording's settings
        // and the settings the task came from.
        bar::apply_capture(&draft, r.task.CaptureSettings.get_or_insert_default());
        let origin = r.origin;
        let hotkey = origin.and_then(|i| self.settings.hotkeys.Hotkeys.get_mut(i)).map(|h| &mut h.TaskSettings);
        let persisted = match hotkey {
            Some(t) if !t.UseDefaultCaptureSettings => t.CaptureSettings.get_or_insert_default(),
            _ => self.settings.settings.DefaultTaskSettings.CaptureSettings.get_or_insert_default(),
        };
        bar::apply_capture(&draft, persisted);
        if let Err(e) = self.settings.save_application_config(false) {
            eprintln!("couldn't save the settings: {e}");
        }
        if let Err(e) = self.settings.save_hotkeys_config(false) {
            eprintln!("couldn't save the hotkeys: {e}");
        }
        self.refresh_menus();
        if !record {
            self.recording = None;
            return None;
        }
        match self.prepare_at(region)? {
            Ok(prepared) => {
                let rect = from_rectangle(prepared.capture_rectangle);
                Some((prepared, rect))
            }
            Err(e) => {
                eprintln!("screen recording: {e:?}");
                self.recording = None;
                None
            }
        }
    }

    /// `ApplicationState.Settings.ScreenRecordRegion = captureRectangle`,
    /// then the frame and the worker.
    fn begin_recording(&mut self, event_loop: &ActiveEventLoop, prepared: PreparedRecording, rect: PxRect) {
        self.settings.settings.ScreenRecordRegion = to_rectangle(rect);
        let theme = self.current_theme;
        let Ok(g) = gpu(&mut self.gpu) else {
            self.recording = None;
            return;
        };
        let Some(r) = self.recording.as_mut() else { return };
        let capture = &prepared.capture;
        let duration = if capture.ScreenRecordFixedDuration { capture.ScreenRecordDuration } else { 0.0 };
        let frame = Frame::new(
            (rect.w, rect.h),
            capture.ScreenRecordShowTimer,
            capture.ScreenRecordShowButtonLabels,
            capture.ScreenRecordAskConfirmationOnAbort,
            duration,
        );
        let screens = sharex_capture::screenshot::screens();
        // `ActivateWindow = startMethod == Region`; `ExcludeFromCapture = useNative`.
        let activate = r.method == ScreenRecordStartMethod::Region;
        let exclude = capture.ScreenRecordUseNative;
        let window = match FrameWindow::open(event_loop, g, &screens, rect, frame, activate, exclude, theme) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("the recording frame: {e}");
                self.recording = None;
                return;
            }
        };
        let control = RecordingControl::new(to_rectangle(rect));
        let proxy = self.proxy.clone();
        let send = move |e: Event| {
            let _ = proxy.send_event(UserEvent::Recording(e));
        };
        let s1 = send.clone();
        control.on_state_changed(move |state, status| s1(Event::State(state, status)));
        let s2 = send.clone();
        control.on_countdown(move |ms| s2(Event::Countdown(ms)));
        let s3 = send.clone();
        control.on_progress(move |p| s3(Event::Progress(p)));
        let s4 = send.clone();
        let tray_callback: tray::Callback = Arc::new(move |e| s4(Event::Tray(e)));
        let tray = match tray::Tray::new("ShareX", false, tray_callback) {
            Ok(t) => Some(t),
            Err(e) => {
                eprintln!("couldn't add the recording's tray icon: {e}");
                None
            }
        };
        let mut active = Active { frame: window, control: control.clone(), tray, tray_shown: None, tray_menu: None };
        active.sync_tray();
        // The worker (`Task.Run`), naming files as `TaskHelpers` does.
        let hooks = Hooks {
            task: r.task.clone(),
            app: self.settings.settings.clone(),
            parent: self.settings.paths.screenshots_parent_folder(Some(&self.settings.settings)),
            metadata: r.metadata.clone(),
            counter: self.settings.settings.NameParserAutoIncrementNumber,
            send: Box::new(send),
        };
        let spawned = if uses_native(&prepared.capture) {
            let session = native::session(prepared, r.output == ScreenRecordOutput::GIF);
            std::thread::Builder::new().name("screen-recording".into()).spawn(move || {
                let mut hooks = hooks;
                let path = native::run(session, control, &mut hooks);
                (hooks.send)(Event::Done { path, counter: hooks.counter });
            })
        } else {
            let session = FFmpegSession {
                prepared,
                screens: screens.iter().map(|s| DesktopScreen { bounds: to_rectangle(s.bounds), primary: s.primary }).collect(),
                input: RecorderInput::default(),
            };
            std::thread::Builder::new().name("screen-recording".into()).spawn(move || {
                let mut hooks = hooks;
                let path = run_ffmpeg_session(session, control, &mut hooks);
                (hooks.send)(Event::Done { path, counter: hooks.counter });
            })
        };
        if let Err(e) = spawned {
            eprintln!("couldn't start recording: {e}");
            self.recording = None;
            return;
        }
        r.phase = Phase::Recording(Box::new(active));
    }

    /// The recording's events (`UserEvent::Recording`).
    pub(crate) fn recording_event(&mut self, event_loop: &ActiveEventLoop, event: Event) {
        match event {
            Event::Done { path, counter } => {
                self.settings.settings.NameParserAutoIncrementNumber = counter;
                self.finish_recording(path);
            }
            Event::Error(output) => {
                // `OutputBoxWindowIntegration.Show(output, "FFmpeg error")`:
                // the output box isn't ported; a message box shows it.
                eprintln!("{output}");
                let title = sharex_strings::helpers_lib::FFmpegCLIManager_Error_title();
                let message = MessageBox::ok(title, output, MessageBoxIcon::Error);
                if let Err(e) = self.open_dialog(event_loop, message, DialogPurpose::Information) {
                    eprintln!("couldn't open the message box: {e}");
                }
            }
            Event::NativeError(message) => {
                // `MessageBox.Show(e.Message, "ShareX", OK, Error)`.
                let message = MessageBox::ok("ShareX", message, MessageBoxIcon::Error);
                if let Err(e) = self.open_dialog(event_loop, message, DialogPurpose::Information) {
                    eprintln!("couldn't open the message box: {e}");
                }
            }
            Event::ActionCompleted => {
                let play = self.recording.as_ref().is_none_or(|r| r.task.GeneralSettings.as_ref().is_none_or(|g| g.PlaySoundAfterAction));
                if play {
                    crate::sound::play_action_completed();
                }
            }
            Event::Tray(e) => self.recording_tray_event(event_loop, e),
            Event::FFmpegDownloaded => self.ffmpeg_downloaded(event_loop),
            Event::State(..) | Event::Countdown(_) | Event::Progress(_) => {
                let Some(Phase::Recording(a)) = self.recording.as_mut().map(|r| &mut r.phase) else { return };
                let f = &mut a.frame.frame;
                match event {
                    Event::State(state, status) => f.change_state(state, status),
                    Event::Countdown(ms) => f.start_countdown(ms),
                    Event::Progress(p) => f.change_state_progress(p),
                    _ => {}
                }
                if a.frame.frame.hidden {
                    a.frame.window.window.set_visible(false);
                }
                a.frame.configure();
                a.sync_tray();
                a.frame.refresh();
            }
        }
    }

    /// The worker ended: the frame and its tray icon go, and a recording
    /// that's there becomes a file task (after the after capture window,
    /// `WorkerTask.CreateFileJobTask`).
    fn finish_recording(&mut self, path: Option<String>) {
        let Some(r) = self.recording.take() else { return };
        drop(r.phase);
        let Some(path) = path.filter(|p| std::path::Path::new(p).is_file()) else { return };
        eprintln!("Screen recording saved: {path}");
        let task = r.task;
        if task.AfterCaptureJob.contains(sharex_settings::AfterCaptureTasks::ShowAfterCaptureWindow) {
            eprintln!("The after capture window isn't ported yet: the recording goes on as accepted.");
        }
        let mut ctx = self.new_task_context();
        let task = crate::upload::manager::create_file_job_task(std::path::Path::new(&path), task, &mut ctx);
        if let Some(t) = task {
            self.start_upload_tasks(vec![t]);
        }
    }

    /// What the frame, its tray menu or a hotkey asked of the frame.
    fn frame_action(&mut self, action: FrameAction) {
        let Some(Phase::Recording(a)) = self.recording.as_mut().map(|r| &mut r.phase) else { return };
        // The native recorder resumes at the frame's region (it can be dragged while paused).
        a.control.set_recording_region(to_rectangle(a.frame.recording_region()));
        let f = &mut a.frame.frame;
        match action {
            FrameAction::StartStop => a.control.start_stop_recording(),
            FrameAction::PauseResume => {
                let before = a.control.status();
                a.control.pause_resume_recording();
                // `UseInProcessPause`: the frame stops and restarts its own timer.
                if a.control.use_in_process_pause() {
                    match (before, a.control.status()) {
                        (ScreenRecordingStatus::Recording, ScreenRecordingStatus::Paused) => f.paused_in_process(),
                        (ScreenRecordingStatus::Paused, ScreenRecordingStatus::Recording) => f.resumed_in_process(),
                        _ => {}
                    }
                }
            }
            FrameAction::Restart => {
                if matches!(a.control.status(), ScreenRecordingStatus::Recording | ScreenRecordingStatus::Paused) {
                    a.control.restart_recording();
                    f.restarted(a.control.status());
                }
            }
            FrameAction::Abort if f.ask_confirmation_on_abort => f.show_abort_confirmation(true),
            FrameAction::Abort | FrameAction::ConfirmAbort => {
                f.show_abort_confirmation(false);
                a.control.abort_recording();
            }
            FrameAction::CancelAbort => f.show_abort_confirmation(false),
            FrameAction::BeginDrag => {
                if f.can_drag() {
                    a.frame.drag = sharex_capture::screenshot::cursor_position().map(|c| (c, a.frame.position));
                }
            }
        }
        a.frame.frame.status = a.control.status();
        a.frame.configure();
        a.control.set_recording_region(to_rectangle(a.frame.recording_region()));
        a.sync_tray();
        a.frame.refresh();
    }

    /// The frame's tray icon: a click is Start/Stop; the right button opens
    /// its menu (Windows; elsewhere the menu is native).
    fn recording_tray_event(&mut self, event_loop: &ActiveEventLoop, e: TrayEvent) {
        match e {
            TrayEvent::Up(tray::Button::Left) => self.frame_action(FrameAction::StartStop),
            TrayEvent::Up(tray::Button::Right) if cfg!(windows) => {
                let theme = self.current_theme;
                let Ok(g) = gpu(&mut self.gpu) else { return };
                let Some(Phase::Recording(a)) = self.recording.as_mut().map(|r| &mut r.phase) else { return };
                if a.tray_menu.take().is_some() {
                    return;
                }
                let entries: Vec<MenuEntry<TrayCommand>> = a
                    .frame
                    .frame
                    .tray_menu()
                    .into_iter()
                    .zip(TrayCommand::ALL)
                    .map(|((header, key, enabled), command)| {
                        let mut e = MenuEntry::item(header, key, command);
                        e.enabled = enabled;
                        e
                    })
                    .collect();
                let screens = sharex_capture::screenshot::screens();
                let cursor = sharex_capture::screenshot::cursor_position().unwrap_or_default();
                match TrayMenu::open(event_loop, g, &screens, cursor, entries, theme, &[]) {
                    Ok(m) => a.tray_menu = Some(m),
                    Err(e) => eprintln!("couldn't open the recording's tray menu: {e}"),
                }
            }
            TrayEvent::Command(i) => {
                if let Some(c) = TrayCommand::ALL.get(i) {
                    self.frame_action(c.action());
                }
            }
            _ => {}
        }
    }

    /// Routes a window event to the recording's windows; false when none of
    /// them is `id`.
    pub(crate) fn recording_window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: &WindowEvent) -> bool {
        let Some(r) = self.recording.as_mut() else { return false };
        match &mut r.phase {
            Phase::Selecting(None) => false,
            Phase::Selecting(Some(session)) => {
                if !session.owns(id) {
                    return false;
                }
                match session.window_event(event_loop, id, event.clone()) {
                    sharex_capture::region::Outcome::Continue => {}
                    sharex_capture::region::Outcome::Cancelled => self.recording = None,
                    sharex_capture::region::Outcome::Captured(result) => {
                        // `SetLastRegion`, `metadata.UpdateInfo(selection.WindowInfo)`.
                        self.last_regions = result.regions.clone();
                        r.phase = Phase::Selecting(None);
                        if let Some(info) = &result.window_info {
                            r.metadata.window_title = Some(info.title.clone());
                            r.metadata.process_name = info.process_name.clone();
                        }
                        self.area_chosen(event_loop, result.screen_rectangle);
                    }
                }
                true
            }
            Phase::Bar(bar) => {
                if !bar.owns(id) {
                    return false;
                }
                if let Some(action) = bar.window_event(id, event) {
                    self.bar_action(event_loop, action);
                }
                true
            }
            Phase::Recording(a) => {
                if a.tray_menu.as_ref().is_some_and(|m| m.owns(id)) {
                    let out = a.tray_menu.as_mut().map(|m| m.window_event(event.clone())).unwrap_or_default();
                    if out.closed {
                        a.tray_menu = None;
                    }
                    if let Some(c) = out.command {
                        self.frame_action(c.action());
                    }
                    return true;
                }
                if a.frame.window.window.id() != id {
                    return false;
                }
                if let Some(action) = a.frame.window_event(event) {
                    self.frame_action(action);
                }
                true
            }
        }
    }

    /// The bar's Record, Cancel and drag.
    fn bar_action(&mut self, event_loop: &ActiveEventLoop, action: BarAction) {
        let capture_window = self.recording.as_ref().map_or(0, |r| r.capture_window);
        let Some(Phase::Bar(bar)) = self.recording.as_mut().map(|r| &mut r.phase) else { return };
        match action {
            BarAction::Record => {
                // `RequestRecord`: an active window recording records the
                // window where it is now, if it's still there.
                if capture_window != 0 {
                    let screens = sharex_capture::screenshot::screens();
                    let bounds = sharex_capture::screenshot::screen_bounds(&screens);
                    let rect = sharex_capture::platform::window_rect(capture_window).intersect(&bounds);
                    let visible = sharex_capture::platform::is_window_visible(capture_window);
                    if !visible || sharex_capture::platform::is_minimized(capture_window) || rect.w <= 0 || rect.h <= 0 {
                        bar.bar.show_error(s::ScreenRecorderBar_TargetUnavailable());
                        bar.refresh();
                        return;
                    }
                    bar.region = if bar.even { from_rectangle(sharex_screen_recording::args::even_rectangle_size(to_rectangle(rect))) } else { rect };
                }
                if let Some((prepared, rect)) = self.close_bar(true) {
                    self.begin_recording(event_loop, prepared, rect);
                }
            }
            BarAction::Cancel => {
                self.close_bar(false);
            }
            BarAction::BeginDrag => {
                bar.drag = sharex_capture::screenshot::cursor_position().map(|c| (c, bar.at));
            }
        }
    }

    /// When a recording window wants drawing next (the timer ticks every
    /// 50 ms, `_refreshTimer`).
    pub(crate) fn recording_wake_at(&mut self) -> Option<Instant> {
        let r = self.recording.as_mut()?;
        match &mut r.phase {
            Phase::Selecting(s) => s.as_mut().and_then(|s| s.wake_at()),
            Phase::Bar(b) => {
                let now = Instant::now();
                if b.window.repaint_at.is_some_and(|at| at <= now) {
                    b.window.window.request_redraw();
                    b.window.repaint_at = None;
                }
                b.window.repaint_at
            }
            Phase::Recording(a) => {
                let now = Instant::now();
                if let Some(m) = &mut a.tray_menu {
                    m.redraw_if_due(now);
                }
                if a.frame.frame.ticking() {
                    let at = a.frame.last_paint + Duration::from_millis(50);
                    if at <= now {
                        a.frame.window.window.request_redraw();
                    }
                    Some(at.max(now + Duration::from_millis(5)))
                } else {
                    a.frame.window.repaint_at
                }
            }
        }
    }

    /// `TaskHelpers.CheckFFmpeg`: FFmpeg isn't where the settings say.
    /// ShareX shows where it looked; ShareXrs doesn't ship FFmpeg, so it
    /// offers ShareX's FFmpeg downloader too ([`ffmpeg_options`]).
    fn ffmpeg_missing(&mut self, event_loop: &ActiveEventLoop, path: PathBuf, _capture: &TaskSettingsCapture) {
        eprintln!("FFmpeg does not exist at: {}", path.display());
        let text = format!(
            "{}\r\n{}\r\n\r\n{}",
            s::FFmpegDoesNotExistAtTheFollowingPath(),
            path.display(),
            sharex_strings::helpers_lib::UpdateMessageBox_UpdateMessageBox_Portable()
        );
        let message = MessageBox::yes_no(format!("ShareX - {}", s::FFmpegIsMissing()), text, MessageBoxIcon::Warning);
        if let Err(e) = self.open_dialog(event_loop, message, DialogPurpose::DownloadFFmpeg(path)) {
            eprintln!("couldn't open the message box: {e}");
        }
    }
}

/// `TaskHelpers`' file naming for the worker, and its reports.
struct Hooks {
    task: TaskSettings,
    app: ApplicationConfig,
    parent: PathBuf,
    metadata: Metadata,
    counter: i32,
    send: Box<dyn Fn(Event) + Send>,
}

impl Hooks {
    /// `GetScreenshotsFolder` + `GetFileName`.
    fn path(&mut self, extension: &str) -> PathBuf {
        let folder = capture_job::screenshots_folder(&self.task, &self.app, &self.parent, &self.metadata);
        folder.join(capture_job::file_name(&self.task, &mut self.counter, extension, &self.metadata))
    }
}

impl SessionHooks for Hooks {
    fn output_path(&mut self, extension: &str) -> Option<String> {
        let path = self.path(extension);
        capture_job::handle_exists_file(&path, &self.task).map(|p| p.to_string_lossy().into_owned())
    }

    fn two_pass_output_path(&mut self, extension: &str) -> String {
        self.path(extension).to_string_lossy().into_owned()
    }

    fn action_completed(&mut self) {
        (self.send)(Event::ActionCompleted);
    }

    fn ffmpeg_error(&mut self, output: &str) {
        (self.send)(Event::Error(output.to_owned()));
    }

    fn native_error(&mut self, message: &str) {
        (self.send)(Event::NativeError(message.to_owned()));
    }
}

impl Active {
    /// The tray icon shows what the frame last set (`SetTrayIcon`,
    /// `SetTrayText`, `IsVisible`).
    fn sync_tray(&mut self) {
        let Some(tray) = &mut self.tray else { return };
        let t = &self.frame.frame.tray;
        let now = (t.icon, t.text.clone(), t.visible);
        if self.tray_shown.as_ref() == Some(&now) {
            return;
        }
        if self.tray_shown.as_ref().is_none_or(|s| s.0 != t.icon) {
            let size = tray::Tray::icon_size();
            let image = match t.icon {
                TrayIcon::Glyph(key, color) => {
                    let theme = if sharex_capture::platform::light_taskbar() { egui::Color32::BLACK } else { egui::Color32::WHITE };
                    sharex_ui::icons::image(key, size, color.unwrap_or(theme))
                }
                // `Helpers.GetProgressIconBytes(progress, Color.FromArgb(140, 0, 36))`.
                TrayIcon::Progress(p) => Some(tray::progress_icon_in(size, p, [140, 0, 36])),
            };
            if let Some(image) = image {
                tray.set_icon(&image);
            }
        }
        tray.set_tooltip(&t.text);
        tray.set_visible(t.visible);
        if cfg!(not(windows)) {
            let entries = self
                .frame
                .frame
                .tray_menu()
                .into_iter()
                .enumerate()
                .map(|(i, (header, _, enabled))| tray::NativeEntry {
                    header: header.to_owned(),
                    separator: false,
                    enabled,
                    checked: false,
                    command: Some(i),
                    children: None,
                })
                .collect();
            tray.set_menu(entries);
        }
        self.tray_shown = Some(now);
    }
}

impl Drop for Active {
    fn drop(&mut self) {
        // The menu and icon go before the frame (`Dispose`).
        self.tray_menu = None;
        self.tray = None;
    }
}

/// `ScreenRecordWindow`: the frame and the toolbar in one transparent
/// window, cut to them.
struct FrameWindow {
    window: EguiWindow,
    frame: Frame,
    geometry: Geometry,
    /// The window's top-left (desktop pixels).
    position: (i32, i32),
    /// Dragging the timer: where the pointer and the window started.
    drag: Option<((i32, i32), (i32, i32))>,
    last_paint: Instant,
    shown_region: Option<Vec<PxRect>>,
}

impl FrameWindow {
    #[allow(clippy::too_many_arguments)]
    fn open(
        event_loop: &ActiveEventLoop,
        gpu: &Gpu,
        screens: &[sharex_capture::Screen],
        rect: PxRect,
        frame: Frame,
        activate: bool,
        exclude: bool,
        theme: egui::Theme,
    ) -> Result<Self, String> {
        let screen = sharex_capture::screenshot::screen_from_point(screens, (rect.x, rect.y));
        let scaling = screen.map_or(1.0, |s| s.scale);
        let geometry = Geometry::new(rect, frame.toolbar_width(), scaling, screen.map(|s| s.work_area));
        let position = geometry.position(rect);
        let window = overlay_window(event_loop, OverlayKind::Frame, position, (geometry.width, geometry.height), activate)?;
        if exclude && !sharex_capture::platform::exclude_from_capture(&window) {
            eprintln!("Could not exclude the recording controls from capture.");
        }
        let w = EguiWindow::transparent(window, gpu)?;
        crate::set_theme(&w, theme);
        let mut me = Self { window: w, frame, geometry, position, drag: None, last_paint: Instant::now(), shown_region: None };
        me.apply_region();
        me.paint();
        me.window.window.set_visible(true);
        if activate {
            me.window.window.focus_window();
        }
        me.window.window.request_redraw();
        Ok(me)
    }

    /// `ConfigureGeometry` after the toolbar's width changed: the recorded
    /// area stays where it is.
    fn configure(&mut self) {
        let region = self.recording_region();
        let screens = sharex_capture::screenshot::screens();
        let screen = sharex_capture::screenshot::screen_from_point(&screens, (region.x, region.y));
        let scaling = screen.map_or(1.0, |s| s.scale);
        let g = Geometry::new(region, self.frame.toolbar_width(), scaling, screen.map(|s| s.work_area));
        if g == self.geometry {
            return;
        }
        self.geometry = g;
        self.position = g.position(region);
        let _ = self.window.window.request_inner_size(PhysicalSize::new(g.width as u32, g.height as u32));
        self.window.window.set_outer_position(PhysicalPosition::new(self.position.0, self.position.1));
        self.apply_region();
    }

    /// `ApplyNativeWindowRegion`: the frame's ring and the toolbar.
    fn apply_region(&mut self) {
        let g = &self.geometry;
        let (l, t, w, h) = (g.frame_left, g.frame_top, g.frame_width, g.frame_height);
        let b = frame::BORDER;
        let rects = vec![
            PxRect::new(l, t, w, b),
            PxRect::new(l, t + h - b, w, b),
            PxRect::new(l, t, b, h),
            PxRect::new(l + w - b, t, b, h),
            PxRect::new(g.toolbar_left, g.toolbar_top, g.toolbar_width, g.toolbar_height),
        ];
        if self.shown_region.as_ref() != Some(&rects) {
            sharex_capture::platform::set_window_region(&self.window.window, &rects);
            self.shown_region = Some(rects);
        }
    }

    fn refresh(&self) {
        self.window.window.request_redraw();
    }

    fn paint(&mut self) -> Option<FrameAction> {
        let (frame, g) = (&mut self.frame, self.geometry);
        let action = self.window.paint(|ui| frame.ui(ui, &g));
        self.last_paint = Instant::now();
        action
    }

    fn window_event(&mut self, event: &WindowEvent) -> Option<FrameAction> {
        match event {
            WindowEvent::RedrawRequested => return self.paint(),
            // Closing it aborts (`OnClosed`).
            WindowEvent::CloseRequested => return Some(FrameAction::ConfirmAbort),
            WindowEvent::CursorMoved { .. } if self.drag.is_some() => self.drag_to_pointer(),
            WindowEvent::MouseInput { state: ElementState::Released, button: MouseButton::Left, .. } if self.drag.is_some() => {
                self.drag = None;
            }
            _ => {}
        }
        self.window.on_event(event);
        None
    }

    /// `OnTimerPointerMoved`: the window follows the pointer while the area
    /// stays on the screens.
    fn drag_to_pointer(&mut self) {
        let Some(((px, py), (wx, wy))) = self.drag else { return };
        let Some((cx, cy)) = sharex_capture::screenshot::cursor_position() else { return };
        let candidate = (wx + cx - px, wy + cy - py);
        let region = self.geometry.recording_region(candidate, self.frame.capture_size);
        let screens = sharex_capture::screenshot::screens();
        if sharex_capture::screenshot::screen_bounds(&screens).contains_rect(&region) {
            self.position = candidate;
            self.window.window.set_outer_position(PhysicalPosition::new(candidate.0, candidate.1));
        } else {
            self.drag = Some(((cx, cy), self.position));
        }
    }

    /// `RecordingRegion`.
    fn recording_region(&self) -> PxRect {
        self.geometry.recording_region(self.position, self.frame.capture_size)
    }
}

/// The recorder bar: one transparent window over the work area of the
/// screen it's on, cut to the bar (and its start delay menu), and the
/// region's goldenrod outline (`ScrollingCaptureRegionWindow`) under it.
struct BarWindow {
    window: EguiWindow,
    outline: EguiWindow,
    outline_geometry: Geometry,
    work: PxRect,
    bar: Bar,
    /// The bar's top-left (desktop pixels).
    at: (i32, i32),
    placed: bool,
    /// `RecordingRegion`.
    region: PxRect,
    even: bool,
    drag: Option<((i32, i32), (i32, i32))>,
    shown_region: Option<Vec<PxRect>>,
}

impl BarWindow {
    #[allow(clippy::too_many_arguments)]
    fn open(
        event_loop: &ActiveEventLoop,
        gpu: &Gpu,
        screens: &[sharex_capture::Screen],
        region: PxRect,
        capture: TaskSettingsCapture,
        gif: bool,
        even: bool,
        theme: egui::Theme,
    ) -> Result<Self, String> {
        // The outline first, so the bar is above it (`SetWindowPos` under
        // the bar), click-through.
        let og = Geometry::new(region, 0.0, 1.0, None);
        let size = (og.frame_width, og.frame_height);
        let window = overlay_window(event_loop, OverlayKind::Outline, og.position(region), size, false)?;
        let _ = window.set_cursor_hittest(false);
        let mut outline = EguiWindow::transparent(window, gpu)?;
        crate::set_theme(&outline, theme);
        sharex_capture::platform::set_window_region(
            &outline.window,
            &[PxRect::new(0, 0, size.0, 1), PxRect::new(0, size.1 - 1, size.0, 1), PxRect::new(0, 0, 1, size.1), PxRect::new(size.0 - 1, 0, 1, size.1)],
        );
        outline.paint(|ui| frame::paint_border(ui, &og, frame::GOLDENROD));
        outline.window.set_visible(true);
        // `ScreenFromBounds`: the screen the region covers most.
        let screen = screens
            .iter()
            .max_by_key(|s| {
                let i = s.bounds.intersect(&region);
                (i.w.max(0) as i64) * (i.h.max(0) as i64)
            })
            .ok_or("no monitors found")?;
        let work = screen.work_area;
        let window = overlay_window(event_loop, OverlayKind::Bar, (work.x, work.y), (work.w, work.h), true)?;
        let w = EguiWindow::transparent(window, gpu)?;
        crate::set_theme(&w, theme);
        let mut me = Self {
            window: w,
            outline,
            outline_geometry: og,
            work,
            bar: Bar::new(capture, gif),
            at: (work.x, work.y),
            placed: false,
            region,
            even,
            drag: None,
            shown_region: None,
        };
        // Laid out to know its size, placed near the region, then shown.
        me.paint();
        me.paint();
        me.window.window.set_visible(true);
        me.window.window.focus_window();
        me.window.window.request_redraw();
        Ok(me)
    }

    fn owns(&self, id: WindowId) -> bool {
        self.window.window.id() == id || self.outline.window.id() == id
    }

    fn refresh(&self) {
        self.window.window.request_redraw();
    }

    fn paint(&mut self) -> Option<BarAction> {
        let ppp = self.window.window.scale_factor() as f32;
        let (work, mut at, placed, region) = (self.work, self.at, self.placed, self.region);
        let bar = &mut self.bar;
        let (action, size, popups) = self.window.paint(|ui| {
            let size = bar.size(ui);
            let px = ((size.x * ppp).ceil() as i32, (size.y * ppp).ceil() as i32);
            at = if placed { bar::clamp_to_work(at, work, px) } else { bar::position_near_region(region, work, px, ppp as f64) };
            let origin = egui::pos2((at.0 - work.x) as f32 / ppp, (at.1 - work.y) as f32 / ppp);
            let action = bar.ui(ui, origin);
            (action, px, bar.popup_rects())
        });
        self.at = at;
        self.placed = true;
        let mut rects = vec![PxRect::new(at.0 - work.x, at.1 - work.y, size.0, size.1)];
        for r in popups {
            let (x, y) = ((r.min.x * ppp).floor() as i32, (r.min.y * ppp).floor() as i32);
            rects.push(PxRect::new(x, y, (r.width() * ppp).ceil() as i32 + 1, (r.height() * ppp).ceil() as i32 + 1));
        }
        if self.shown_region.as_ref() != Some(&rects) {
            sharex_capture::platform::set_window_region(&self.window.window, &rects);
            self.shown_region = Some(rects);
        }
        action
    }

    fn window_event(&mut self, id: WindowId, event: &WindowEvent) -> Option<BarAction> {
        if self.outline.window.id() == id {
            if matches!(event, WindowEvent::RedrawRequested) {
                let og = self.outline_geometry;
                self.outline.paint(|ui| frame::paint_border(ui, &og, frame::GOLDENROD));
            }
            return None;
        }
        match event {
            WindowEvent::RedrawRequested => return self.paint(),
            WindowEvent::CloseRequested => return Some(BarAction::Cancel),
            WindowEvent::KeyboardInput { event: k, .. } if k.state == ElementState::Pressed && k.logical_key == Key::Named(NamedKey::Escape) => {
                let action = self.bar.escape();
                self.refresh();
                return action;
            }
            // `Deactivated`: `ClosePanel`.
            WindowEvent::Focused(false) => {
                self.bar.deactivated();
                self.refresh();
            }
            WindowEvent::CursorMoved { .. } if self.drag.is_some() => {
                if let (Some(((px, py), (bx, by))), Some((cx, cy))) = (self.drag, sharex_capture::screenshot::cursor_position()) {
                    self.at = (bx + cx - px, by + cy - py);
                    self.refresh();
                }
            }
            WindowEvent::MouseInput { state: ElementState::Released, button: MouseButton::Left, .. } if self.drag.is_some() => {
                self.drag = None;
            }
            _ => {}
        }
        self.window.on_event(event);
        None
    }
}

/// Which of the recording's windows.
enum OverlayKind {
    Frame,
    Bar,
    Outline,
}

/// A borderless, topmost, transparent tool window out of the taskbar.
fn overlay_window(event_loop: &ActiveEventLoop, kind: OverlayKind, at: (i32, i32), size: (i32, i32), active: bool) -> Result<winit::window::Window, String> {
    let title = match kind {
        OverlayKind::Frame => sharex_strings::screen_capture_lib::ScreenRecordWindow_Title(),
        OverlayKind::Bar => Bar::title(),
        OverlayKind::Outline => sharex_strings::screen_capture_lib::ScrollingCaptureWindow_Title(),
    };
    #[allow(unused_mut)]
    let mut attrs = WindowAttributes::default()
        .with_title(crate::rs_title(title))
        .with_decorations(false)
        .with_resizable(false)
        .with_transparent(true)
        .with_visible(false)
        .with_active(active)
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
    Ok(window)
}

fn gpu(slot: &mut Option<Gpu>) -> Result<&Gpu, String> {
    match slot {
        Some(g) => Ok(g),
        None => Ok(slot.insert(Gpu::new()?)),
    }
}

fn to_rectangle(r: PxRect) -> Rectangle {
    Rectangle::new(r.x, r.y, r.w, r.h)
}

fn from_rectangle(r: Rectangle) -> PxRect {
    PxRect::new(r.x, r.y, r.width, r.height)
}

/// What `--render` draws of a recording window.
pub struct RenderOptions {
    pub content: String,
    pub text: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub scale: f32,
    pub hover: Option<egui::Pos2>,
    pub clicks: Vec<egui::Pos2>,
}

/// `sharexrs --render ffmpeg-options | recording-toolbar | recorder-bar`.
pub fn render(screen: &str, dark: bool, o: &RenderOptions) -> Result<image::RgbaImage, String> {
    use sharex_ui::offscreen::Offscreen;
    let behind = egui::Color32::from_rgb(0x80, 0x80, 0x80);
    match screen {
        "ffmpeg-options" => {
            let mut capture = TaskSettingsCapture::default();
            if let Some(path) = &o.text {
                capture.FFmpegOptions.OverrideCLIPath = true;
                capture.FFmpegOptions.CLIPath = path.clone();
            }
            let mut state = ffmpeg_options::FFmpegOptionsWindow::new(&capture);
            state.wait_for_devices();
            state.set_page(&o.content);
            let w = o.width.unwrap_or((ffmpeg_options::WIDTH * o.scale) as u32);
            let h = o.height.unwrap_or((ffmpeg_options::HEIGHT * o.scale) as u32);
            let mut screen = Offscreen::new([w, h], o.scale, dark)?;
            Ok(crate::render::render_frames(&mut screen, o.hover, &o.clicks, &mut |ui| {
                state.ui(ui);
            }))
        }
        "recording-toolbar" => {
            let c = o.content.as_str();
            let labels = c.contains("-labels");
            let timer = !c.contains("-notimer");
            let mut f = Frame::new((400, 300), timer, labels, true, 0.0);
            let base = c.split('-').next().unwrap_or("waiting");
            match base {
                "recording" => {
                    f.change_state(ScreenRecordState::BeforeStart, ScreenRecordingStatus::Waiting);
                    f.change_state(ScreenRecordState::AfterStart, ScreenRecordingStatus::Working);
                    f.change_state(ScreenRecordState::AfterRecordingStart, ScreenRecordingStatus::Recording);
                }
                "working" => {
                    f.change_state(ScreenRecordState::BeforeStart, ScreenRecordingStatus::Waiting);
                    f.change_state(ScreenRecordState::AfterStart, ScreenRecordingStatus::Working);
                }
                "paused" => {
                    f.change_state(ScreenRecordState::BeforeStart, ScreenRecordingStatus::Waiting);
                    f.change_state(ScreenRecordState::AfterRecordingStart, ScreenRecordingStatus::Recording);
                    f.change_state(ScreenRecordState::RecordingEnd, ScreenRecordingStatus::Paused);
                }
                "confirm" => f.show_abort_confirmation(true),
                "before" => f.change_state(ScreenRecordState::BeforeStart, ScreenRecordingStatus::Waiting),
                _ => {}
            }
            f.timer.stop();
            f.timer_text_override = o.text.clone();
            let w = o.width.unwrap_or(f.toolbar_width().ceil() as u32);
            let h = o.height.unwrap_or(frame::TOOLBAR_HEIGHT as u32);
            let mut screen = Offscreen::new([w, h], o.scale, dark)?;
            Ok(crate::render::render_frames(&mut screen, o.hover, &o.clicks, &mut |ui| {
                ui.painter().rect_filled(ui.max_rect(), 0.0, behind);
                let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(f.toolbar_width(), frame::TOOLBAR_HEIGHT));
                f.toolbar_only(ui, rect);
            }))
        }
        "recorder-bar" => {
            let c = o.content.as_str();
            let capture = TaskSettingsCapture { ScreenRecordShowButtonLabels: c.contains("labels"), ..Default::default() };
            let mut bar = Bar::new(capture, c.contains("gif"));
            if c.starts_with("options") {
                bar.open_options(c.contains("bar"));
            }
            let ctx = egui::Context::default();
            sharex_ui::install(&ctx);
            ctx.set_theme(if dark { egui::Theme::Dark } else { egui::Theme::Light });
            let mut size = egui::Vec2::ZERO;
            let mut measured = ctx.run_ui(egui::RawInput::default(), |ui| size = bar.size(ui));
            measured.textures_delta.clear();
            let w = o.width.unwrap_or((size.x * o.scale).ceil() as u32);
            let h = o.height.unwrap_or((size.y * o.scale).ceil() as u32);
            let mut screen = Offscreen::new([w, h], o.scale, dark)?;
            Ok(crate::render::render_frames(&mut screen, o.hover, &o.clicks, &mut |ui| {
                ui.painter().rect_filled(ui.max_rect(), 0.0, behind);
                bar.ui(ui, egui::Pos2::ZERO);
            }))
        }
        other => Err(format!("no screen named {other}")),
    }
}
