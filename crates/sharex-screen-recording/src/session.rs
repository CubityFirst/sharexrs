//! A recording from start to file, with FFmpeg: the FFmpeg branch of
//! `ShareX/ScreenRecordManager.cs` (`StartRecording`, `StopRecording`,
//! `ProcessTwoPassEncoding`) and the state `ScreenRecordWindow` (the
//! recording frame and its Start/Stop, Pause, Restart, Abort buttons)
//! keeps, without its UI: [`RecordingControl`] is what the frame's buttons,
//! hotkeys and tray menu call; [`run_ffmpeg_session`] is the worker.
//!
//! FFmpeg can't pause, so ShareX stops FFmpeg on Pause, records the next
//! segment into the same path after renaming the first one `-concat`, and
//! joins them (`-f concat -c copy`) when the segment ends. Abort stops
//! FFmpeg the same way (`q`) and deletes the file. A two-pass recording
//! (always for GIF/WebP/APNG) records lossless x264 first, then encodes.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use sharex_settings::TaskSettingsCapture;
use sharex_settings::ffmpeg_options::FFmpegVideoCodec;
use sharex_settings::types::Rectangle;

use crate::args::{DesktopScreen, ScreenRecordingOptions, even_rectangle_size, intersect};
use crate::cli::{FFmpegCliManager, log};
use crate::dotnet::append_text_to_file_name;
use crate::recorder::{RecorderInput, ScreenRecorder};

/// `ShareX.ScreenRecordOutput`: what the hotkey asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenRecordOutput {
    FFmpeg,
    GIF,
}

/// `ShareX.ScreenRecordStartMethod`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenRecordStartMethod {
    Region,
    ActiveWindow,
    CustomRegion,
    LastRegion,
}

/// `ShareX.ScreenCaptureLib.ScreenRecordState`: what the worker tells the
/// recording frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenRecordState {
    Waiting,
    BeforeStart,
    AfterStart,
    AfterRecordingStart,
    RecordingEnd,
    Encoding,
}

/// `ShareX.ScreenCaptureLib.ScreenRecordingStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScreenRecordingStatus {
    #[default]
    Waiting,
    Working,
    Recording,
    Paused,
    Stopped,
    Aborted,
}

/// A `ManualResetEvent`.
#[derive(Default)]
struct ResetEvent {
    set: Mutex<bool>,
    cv: Condvar,
}

impl ResetEvent {
    fn set(&self) {
        *self.set.lock().unwrap_or_else(|e| e.into_inner()) = true;
        self.cv.notify_all();
    }

    fn reset(&self) {
        *self.set.lock().unwrap_or_else(|e| e.into_inner()) = false;
    }

    /// `WaitOne(timeout)`; `None` waits forever. Whether it was set.
    fn wait(&self, timeout: Option<Duration>) -> bool {
        let guard = self.set.lock().unwrap_or_else(|e| e.into_inner());
        match timeout {
            None => *self.cv.wait_while(guard, |s| !*s).unwrap_or_else(|e| e.into_inner()),
            Some(t) => *self.cv.wait_timeout_while(guard, t, |s| !*s).unwrap_or_else(|e| e.into_inner()).0,
        }
    }
}

type Listener<T> = Mutex<Option<Box<dyn Fn(T) + Send + Sync>>>;

/// `ScreenRecordWindow`'s recording state, without the window: the status,
/// `RecordResetEvent`, the restart request and the recording region (which
/// the frame lets the user move while waiting).
pub struct RecordingControl {
    status: Mutex<ScreenRecordingStatus>,
    reset: ResetEvent,
    restart_requested: AtomicBool,
    region: Mutex<Rectangle>,
    /// `UseInProcessPause`: the native recorder pauses in process; FFmpeg doesn't (false).
    in_process_pause: AtomicBool,
    stop_requested: Listener<()>,
    pause_requested: Listener<()>,
    resume_requested: Listener<()>,
    state_changed: Listener<(ScreenRecordState, ScreenRecordingStatus)>,
    countdown: Listener<u32>,
    progress: Listener<i32>,
}

impl RecordingControl {
    pub fn new(region: Rectangle) -> Arc<Self> {
        let control = Arc::new(Self {
            status: Mutex::default(),
            reset: ResetEvent::default(),
            restart_requested: AtomicBool::new(false),
            region: Mutex::new(region),
            in_process_pause: AtomicBool::new(false),
            stop_requested: Mutex::default(),
            pause_requested: Mutex::default(),
            resume_requested: Mutex::default(),
            state_changed: Mutex::default(),
            countdown: Mutex::default(),
            progress: Mutex::default(),
        });
        control.change_state(ScreenRecordState::Waiting);
        control
    }

    /// `UseInProcessPause` (`useNative`).
    pub fn set_in_process_pause(&self, on: bool) {
        self.in_process_pause.store(on, Ordering::SeqCst);
    }

    pub fn use_in_process_pause(&self) -> bool {
        self.in_process_pause.load(Ordering::SeqCst)
    }

    /// `PauseRequested` (in-process pause only).
    pub fn on_pause_requested(&self, f: impl Fn() + Send + Sync + 'static) {
        *self.pause_requested.lock().unwrap_or_else(|e| e.into_inner()) = Some(Box::new(move |()| f()));
    }

    /// `ResumeRequested` (in-process pause only).
    pub fn on_resume_requested(&self, f: impl Fn() + Send + Sync + 'static) {
        *self.resume_requested.lock().unwrap_or_else(|e| e.into_inner()) = Some(Box::new(move |()| f()));
    }

    fn emit(listener: &Listener<()>) {
        if let Some(f) = listener.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            f(());
        }
    }

    pub fn status(&self) -> ScreenRecordingStatus {
        *self.status.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn set_status(&self, s: ScreenRecordingStatus) {
        *self.status.lock().unwrap_or_else(|e| e.into_inner()) = s;
    }

    /// `RecordingRegion`.
    pub fn recording_region(&self) -> Rectangle {
        *self.region.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The frame was moved (while waiting).
    pub fn set_recording_region(&self, r: Rectangle) {
        *self.region.lock().unwrap_or_else(|e| e.into_inner()) = r;
    }

    /// `StopRequested`: stop what's recording (ShareX:
    /// `ScreenRecordManager.StopRecording`).
    pub fn on_stop_requested(&self, f: impl Fn() + Send + Sync + 'static) {
        *self.stop_requested.lock().unwrap_or_else(|e| e.into_inner()) = Some(Box::new(move |()| f()));
    }

    /// `ChangeState`'s UI side (tray icon and text, timer, buttons): the
    /// state and the status after it.
    pub fn on_state_changed(&self, f: impl Fn(ScreenRecordState, ScreenRecordingStatus) + Send + Sync + 'static) {
        *self.state_changed.lock().unwrap_or_else(|e| e.into_inner()) = Some(Box::new(move |(a, b)| f(a, b)));
    }

    /// `StartCountdown(milliseconds)`.
    pub fn on_countdown(&self, f: impl Fn(u32) + Send + Sync + 'static) {
        *self.countdown.lock().unwrap_or_else(|e| e.into_inner()) = Some(Box::new(f));
    }

    /// `ChangeStateProgress`: the second pass's percent.
    pub fn on_progress(&self, f: impl Fn(i32) + Send + Sync + 'static) {
        *self.progress.lock().unwrap_or_else(|e| e.into_inner()) = Some(Box::new(f));
    }

    fn emit_stop_requested(&self) {
        if let Some(f) = self.stop_requested.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            f(());
        }
    }

    /// `RecordResetEvent.WaitOne(timeout)`: whether it was set.
    pub fn wait_reset(&self, timeout: Option<Duration>) -> bool {
        self.reset.wait(timeout)
    }

    /// `StartCountdown`.
    pub fn emit_countdown(&self, ms: u32) {
        if let Some(f) = self.countdown.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            f(ms);
        }
    }

    /// `ChangeStateProgress`.
    pub fn emit_progress(&self, p: i32) {
        if let Some(f) = self.progress.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            f(p);
        }
    }

    /// `ScreenRecordManager.StopRecording` (the Stop screen recording
    /// hotkey): stops the current FFmpeg run directly, leaving the status
    /// as it is; the worker then ends as after a stop.
    pub fn request_stop(&self) {
        self.emit_stop_requested();
    }

    /// `StartStopRecording`: the Start/Stop button and the recorder hotkey.
    pub fn start_stop_recording(&self) {
        match self.status() {
            ScreenRecordingStatus::Working => self.abort_recording(),
            ScreenRecordingStatus::Recording => {
                self.set_status(ScreenRecordingStatus::Stopped);
                self.emit_stop_requested();
            }
            ScreenRecordingStatus::Paused => {
                self.set_status(ScreenRecordingStatus::Stopped);
                if self.use_in_process_pause() {
                    self.emit_stop_requested();
                }
                self.reset.set();
            }
            _ => self.reset.set(),
        }
    }

    /// `PauseResumeRecording`. With FFmpeg, pausing stops the segment and
    /// resuming starts the next one.
    pub fn pause_resume_recording(&self) {
        match self.status() {
            ScreenRecordingStatus::Recording => {
                self.set_status(ScreenRecordingStatus::Paused);
                self.reset.reset();
                if self.use_in_process_pause() {
                    Self::emit(&self.pause_requested);
                } else {
                    self.emit_stop_requested();
                }
            }
            ScreenRecordingStatus::Paused if self.use_in_process_pause() => {
                Self::emit(&self.resume_requested);
                self.set_status(ScreenRecordingStatus::Recording);
            }
            _ => self.reset.set(),
        }
    }

    /// `AbortRecording` (the frame asks first with
    /// `ScreenRecordAskConfirmationOnAbort`; that's the UI's).
    pub fn abort_recording(&self) {
        self.set_status(ScreenRecordingStatus::Aborted);
        self.emit_stop_requested();
        self.reset.set();
    }

    /// `RestartRecording`: only while recording or paused; throws the
    /// recording away and starts again (with the start delay).
    pub fn restart_recording(&self) {
        let status = self.status();
        if !matches!(status, ScreenRecordingStatus::Recording | ScreenRecordingStatus::Paused) {
            return;
        }
        let stop_active = status == ScreenRecordingStatus::Recording
            || (self.use_in_process_pause() && status == ScreenRecordingStatus::Paused);
        self.restart_requested.store(true, Ordering::SeqCst);
        self.set_status(ScreenRecordingStatus::Waiting);
        if stop_active {
            self.emit_stop_requested();
        }
        self.reset.set();
    }

    pub fn consume_restart_request(&self) -> bool {
        self.restart_requested.swap(false, Ordering::SeqCst)
    }

    pub fn restart_requested(&self) -> bool {
        self.restart_requested.load(Ordering::SeqCst)
    }

    /// `ChangeState`: the statuses it sets, then the UI listener.
    pub fn change_state(&self, state: ScreenRecordState) {
        match state {
            ScreenRecordState::AfterStart => self.set_status(ScreenRecordingStatus::Working),
            ScreenRecordState::AfterRecordingStart => self.set_status(ScreenRecordingStatus::Recording),
            _ => {}
        }
        let status = self.status();
        if let Some(f) = self.state_changed.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            f((state, status));
        }
    }
}

/// Why a recording doesn't start (`StartRecording`'s early returns).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartError {
    /// `CheckFFmpeg` failed.
    FFmpeg(crate::discovery::FFmpegCheckError),
    /// `FFmpeg_FFmpeg_video_and_audio_source_both_can_t_be__None__`.
    NoSource,
    /// The region is empty once clipped to the screens.
    InvalidRegion,
}

/// What `StartRecording` settles before recording.
#[derive(Debug, Clone, PartialEq)]
pub struct PreparedRecording {
    /// The task's capture settings as the recording uses them (the codec
    /// set to GIF for a GIF recording, two-pass forced for animated images).
    pub capture: TaskSettingsCapture,
    /// `GIFFPS` for GIFs, `ScreenRecordFPS` otherwise.
    pub fps: i32,
    pub use_gif_fps: bool,
    /// The region clipped to the screens, even-sized for video codecs.
    pub capture_rectangle: Rectangle,
    pub ffmpeg_path: PathBuf,
}

/// The FFmpeg recorder's part of `StartRecording` before any window opens:
/// the codec for a GIF, two-pass for animated images, the frame rate, the
/// FFmpeg check, the source check, and the region clipped to
/// `screen_bounds` (`CaptureHelpers.GetScreenBounds`) and made even.
/// `ffmpeg_path` is where FFmpeg was found ([`crate::discovery`]).
pub fn prepare_ffmpeg_recording(
    output: ScreenRecordOutput,
    capture: &TaskSettingsCapture,
    capture_rectangle: Rectangle,
    screen_bounds: Rectangle,
    ffmpeg_path: Result<PathBuf, crate::discovery::FFmpegCheckError>,
) -> Result<PreparedRecording, StartError> {
    let mut capture = capture.clone();
    let is_gif = output == ScreenRecordOutput::GIF;
    if is_gif {
        capture.FFmpegOptions.VideoCodec = FFmpegVideoCodec::gif;
    }
    if capture.FFmpegOptions.is_animated_image() {
        capture.ScreenRecordTwoPassEncoding = true;
    }
    let use_gif_fps = is_gif || capture.FFmpegOptions.VideoCodec == FFmpegVideoCodec::gif;
    let fps = if use_gif_fps { capture.GIFFPS } else { capture.ScreenRecordFPS };
    let ffmpeg_path = ffmpeg_path.map_err(StartError::FFmpeg)?;
    if !capture.FFmpegOptions.is_source_selected() {
        return Err(StartError::NoSource);
    }
    let mut rect = intersect(capture_rectangle, screen_bounds);
    if capture.FFmpegOptions.is_even_size_required() {
        rect = even_rectangle_size(rect);
    }
    // `Rectangle.IsValid()`: positive width and height.
    if rect.width <= 0 || rect.height <= 0 {
        return Err(StartError::InvalidRegion);
    }
    Ok(PreparedRecording { capture, fps, use_gif_fps, capture_rectangle: rect, ffmpeg_path })
}

/// `StartRecording`'s checks for the native recorder (`useNative`): no
/// FFmpeg options are involved, the frame rate is `GIFFPS` or
/// `ScreenRecordFPS`, and only an MP4 needs an even size. ShareXrs adds one:
/// FFmpeg must be there, since it grabs the screen for the native recorder
/// until Windows Graphics Capture is ported.
pub fn prepare_native_recording(
    output: ScreenRecordOutput,
    capture: &TaskSettingsCapture,
    capture_rectangle: Rectangle,
    screen_bounds: Rectangle,
    ffmpeg_path: Result<PathBuf, crate::discovery::FFmpegCheckError>,
) -> Result<PreparedRecording, StartError> {
    let is_gif = output == ScreenRecordOutput::GIF;
    let fps = if is_gif { capture.GIFFPS } else { capture.ScreenRecordFPS };
    let ffmpeg_path = ffmpeg_path.map_err(StartError::FFmpeg)?;
    let mut rect = intersect(capture_rectangle, screen_bounds);
    if !is_gif {
        rect = even_rectangle_size(rect);
    }
    if rect.width <= 0 || rect.height <= 0 {
        return Err(StartError::InvalidRegion);
    }
    Ok(PreparedRecording { capture: capture.clone(), fps, use_gif_fps: is_gif, capture_rectangle: rect, ffmpeg_path })
}

/// What the session needs from the app: file names (ShareX's
/// `TaskHelpers.GetScreenshotsFolder`/`GetFileName`/`HandleExistsFile`),
/// sounds and the error box.
pub trait SessionHooks: Send {
    /// The recording's path for `extension`; `None` cancels (the user
    /// declined to overwrite).
    fn output_path(&mut self, extension: &str) -> Option<String>;
    /// The two-pass output's path for `extension` (a new name; ShareX
    /// doesn't check whether it exists).
    fn two_pass_output_path(&mut self, extension: &str) -> String;
    /// `PlayNotificationSoundAsync(NotificationSound.ActionCompleted)`.
    fn action_completed(&mut self) {}
    /// FFmpeg failed: its output (ShareX's output box).
    fn ffmpeg_error(&mut self, _output: &str) {}
    /// The native recorder failed: ShareX's "ShareX" error message box.
    fn native_error(&mut self, _message: &str) {}
}

/// The session's inputs besides the control.
pub struct FFmpegSession {
    pub prepared: PreparedRecording,
    pub screens: Vec<DesktopScreen>,
    /// ShareX's sources, or a [`crate::platform`] input (macOS, Linux).
    pub input: RecorderInput,
}

/// The worker of `StartRecording` for FFmpeg: records (segments, pauses,
/// restarts) until stopped or aborted, joins the segments, runs the second
/// pass. The finished file, or `None` (aborted, cancelled, failed).
pub fn run_ffmpeg_session(session: FFmpegSession, control: Arc<RecordingControl>, hooks: &mut dyn SessionHooks) -> Option<String> {
    let FFmpegSession { prepared, screens, input } = session;
    let capture = &prepared.capture;
    let duration = if capture.ScreenRecordFixedDuration { capture.ScreenRecordDuration } else { 0.0 };
    let mut abort_requested = false;
    let current: Arc<Mutex<Option<FFmpegCliManager>>> = Arc::default();
    {
        // `ScreenRecordManager.StopRecording`: stop the current segment.
        let current = current.clone();
        control.on_stop_requested(move || {
            if let Some(ffmpeg) = current.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
                ffmpeg.close();
            }
        });
    }
    let mut recorder: Option<ScreenRecorder> = None;

    let extension = if capture.ScreenRecordTwoPassEncoding { "mp4" } else { capture.FFmpegOptions.extension() };
    let mut path = hooks.output_path(extension).unwrap_or_default();
    let (mut concat_path, mut temp_path) = (String::new(), String::new());
    if path.is_empty() {
        abort_requested = true;
    } else {
        concat_path = append_text_to_file_name(&path, "-concat");
        delete_file(&concat_path);
        temp_path = append_text_to_file_name(&path, "-temp");
        delete_file(&temp_path);
    }

    let waiting_or_paused = |c: &RecordingControl| matches!(c.status(), ScreenRecordingStatus::Waiting | ScreenRecordingStatus::Paused);

    while !abort_requested && waiting_or_paused(&control) {
        control.change_state(ScreenRecordState::BeforeStart);
        if control.status() == ScreenRecordingStatus::Paused {
            control.reset.wait(None);
        } else {
            let delay = if capture.ScreenRecordStartDelayEnabled { (capture.ScreenRecordStartDelay * 1000.0) as i32 } else { 0 };
            if delay > 0 {
                control.emit_countdown(delay as u32);
                control.reset.wait(Some(Duration::from_millis(delay as u64)));
            }
        }
        if control.status() == ScreenRecordingStatus::Aborted {
            abort_requested = true;
        }
        if control.consume_restart_request() {
            recorder = None;
            delete_file(&path);
            delete_file(&concat_path);
            delete_file(&temp_path);
        }
        if waiting_or_paused(&control) {
            if control.status() == ScreenRecordingStatus::Paused && Path::new(&path).exists() {
                rename_file(&path, &concat_path);
            }
            control.change_state(ScreenRecordState::AfterStart);
            let options = ScreenRecordingOptions {
                is_recording: true,
                is_lossless: capture.ScreenRecordTwoPassEncoding,
                ffmpeg: capture.FFmpegOptions.clone(),
                fps: prepared.fps,
                duration,
                output_path: Some(path.clone()),
                capture_area: control.recording_region(),
                draw_cursor: capture.ScreenRecordShowCursor,
                input_path: None,
            };
            let r = match ScreenRecorder::new(options, screens.clone(), &prepared.ffmpeg_path) {
                Ok(r) => r,
                Err(e) => {
                    log(&e);
                    break;
                }
            };
            r.set_input(input.clone());
            {
                let c = control.clone();
                r.on_recording_started(move || c.change_state(ScreenRecordState::AfterRecordingStart));
                let c = control.clone();
                r.on_encoding_progress(move |p| c.emit_progress(p));
                let errors: Arc<Mutex<Option<String>>> = Arc::default();
                let e2 = errors.clone();
                r.ffmpeg().on_error(move |out| *e2.lock().unwrap_or_else(|e| e.into_inner()) = Some(out));
                *current.lock().unwrap_or_else(|e| e.into_inner()) = Some(r.stopper());
                if !matches!(control.status(), ScreenRecordingStatus::Aborted | ScreenRecordingStatus::Stopped) {
                    r.start_recording();
                }
                if let Some(out) = errors.lock().unwrap_or_else(|e| e.into_inner()).take() {
                    hooks.ffmpeg_error(&out);
                }
            }
            recorder = Some(r);
            control.change_state(ScreenRecordState::RecordingEnd);
            if control.status() == ScreenRecordingStatus::Aborted {
                abort_requested = true;
            }
            if control.restart_requested() {
                continue;
            }
        }
        hooks.action_completed();
        if Path::new(&concat_path).exists() {
            let ffmpeg = FFmpegCliManager::new(&prepared.ffmpeg_path);
            ffmpeg.set_show_error(true);
            ffmpeg.concatenate_videos(&[&concat_path, &path], &temp_path, true);
            rename_file(&temp_path, &path);
        }
    }

    if capture.ScreenRecordTwoPassEncoding
        && !abort_requested
        && let Some(r) = &recorder
        && Path::new(&path).exists()
    {
        control.change_state(ScreenRecordState::Encoding);
        *current.lock().unwrap_or_else(|e| e.into_inner()) = Some(r.stopper());
        path = process_two_pass_encoding(r, &path, capture, hooks);
    }
    drop(recorder);
    *current.lock().unwrap_or_else(|e| e.into_inner()) = None;

    if abort_requested {
        delete_file(&path);
    }
    delete_file(&concat_path);
    delete_file(&temp_path);

    (!abort_requested && !path.is_empty() && Path::new(&path).exists()).then_some(path)
}

/// `ProcessTwoPassEncoding`: the lossless recording encoded with the
/// chosen codec (or as a GIF) into a new file; the recording is deleted.
/// The new path, whether the encode worked or not (as ShareX).
fn process_two_pass_encoding(recorder: &ScreenRecorder, input: &str, capture: &TaskSettingsCapture, hooks: &mut dyn SessionHooks) -> String {
    let output = hooks.two_pass_output_path(capture.FFmpegOptions.extension());
    if capture.FFmpegOptions.VideoCodec == FFmpegVideoCodec::gif {
        recorder.ffmpeg_encode_as_gif(input, &output);
    } else {
        recorder.ffmpeg_encode_video(input, &output);
    }
    if !input.eq_ignore_ascii_case(&output) && Path::new(input).exists() {
        let _ = std::fs::remove_file(input);
    }
    output
}

/// `FileHelpers.DeleteFile`.
fn delete_file(path: &str) {
    if !path.is_empty() && Path::new(path).is_file() {
        let _ = std::fs::remove_file(path);
    }
}

/// `FileHelpers.RenameFile(path, newPath)` with a full new path: fails
/// (and logs; ShareX shows a message box) when the target exists.
fn rename_file(path: &str, new_path: &str) {
    if path.is_empty() || !Path::new(path).is_file() {
        return;
    }
    if Path::new(new_path).exists() {
        log(&format!("Rename file error: {new_path} already exists"));
        return;
    }
    if let Err(e) = std::fs::rename(path, new_path) {
        log(&format!("Rename file error: {e}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_preparation() {
        let mut capture = TaskSettingsCapture::default();
        capture.FFmpegOptions.VideoSource = String::new();
        capture.FFmpegOptions.AudioSource = String::new();
        let bounds = Rectangle { x: 0, y: 0, width: 1920, height: 1080 };
        let area = Rectangle { x: 10, y: 10, width: 101, height: 51 };
        let ok = Ok(PathBuf::from("ffmpeg.exe"));
        // No source check; MP4 even, GIF as is.
        let mp4 = prepare_native_recording(ScreenRecordOutput::FFmpeg, &capture, area, bounds, ok.clone()).unwrap();
        assert_eq!((mp4.capture_rectangle.width, mp4.capture_rectangle.height, mp4.fps), (100, 50, capture.ScreenRecordFPS));
        let gif = prepare_native_recording(ScreenRecordOutput::GIF, &capture, area, bounds, ok).unwrap();
        assert_eq!((gif.capture_rectangle.width, gif.fps, gif.use_gif_fps), (101, capture.GIFFPS, true));
        assert_eq!(gif.capture.FFmpegOptions.VideoCodec, capture.FFmpegOptions.VideoCodec);
    }

    #[test]
    fn control_transitions_like_the_frame() {
        let c = RecordingControl::new(Rectangle::new(0, 0, 10, 10));
        let stops = Arc::new(Mutex::new(0));
        let s = stops.clone();
        c.on_stop_requested(move || *s.lock().unwrap() += 1);
        assert_eq!(c.status(), ScreenRecordingStatus::Waiting);
        c.change_state(ScreenRecordState::AfterStart);
        assert_eq!(c.status(), ScreenRecordingStatus::Working);
        c.change_state(ScreenRecordState::AfterRecordingStart);
        c.pause_resume_recording();
        assert_eq!((c.status(), *stops.lock().unwrap()), (ScreenRecordingStatus::Paused, 1));
        assert!(!c.reset.wait(Some(Duration::ZERO)));
        c.pause_resume_recording(); // resume: lets the worker record the next segment
        assert_eq!(c.status(), ScreenRecordingStatus::Paused);
        assert!(c.reset.wait(Some(Duration::ZERO)));
        c.restart_recording();
        assert_eq!(c.status(), ScreenRecordingStatus::Waiting);
        assert!(c.consume_restart_request() && !c.restart_requested());
        // Start/Stop while Working aborts.
        c.change_state(ScreenRecordState::AfterStart);
        c.start_stop_recording();
        assert_eq!(c.status(), ScreenRecordingStatus::Aborted);
    }

    #[test]
    fn prepare_follows_start_recording() {
        let capture = TaskSettingsCapture::default();
        let screen = Rectangle::new(0, 0, 1920, 1080);
        let ok = || Ok(PathBuf::from("ffmpeg"));
        let p = prepare_ffmpeg_recording(ScreenRecordOutput::GIF, &capture, Rectangle::new(-5, 10, 101, 51), screen, ok()).unwrap();
        assert_eq!(p.capture.FFmpegOptions.VideoCodec, FFmpegVideoCodec::gif);
        assert!(p.capture.ScreenRecordTwoPassEncoding);
        assert_eq!((p.fps, p.capture_rectangle), (15, Rectangle::new(0, 10, 96, 51)));
        let p = prepare_ffmpeg_recording(ScreenRecordOutput::FFmpeg, &capture, Rectangle::new(-5, 10, 101, 51), screen, ok()).unwrap();
        assert_eq!((p.fps, p.capture_rectangle), (30, Rectangle::new(0, 10, 96, 50)));
        let mut none = capture.clone();
        none.FFmpegOptions.VideoSource.clear();
        assert_eq!(prepare_ffmpeg_recording(ScreenRecordOutput::FFmpeg, &none, screen, screen, ok()), Err(StartError::NoSource));
        assert_eq!(
            prepare_ffmpeg_recording(ScreenRecordOutput::FFmpeg, &capture, Rectangle::new(5000, 0, 10, 10), screen, ok()),
            Err(StartError::InvalidRegion)
        );
    }
}
