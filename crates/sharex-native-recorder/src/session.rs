//! `ScreenRecordManager`'s native path (`StartRecording`'s worker with
//! `useNative`, `RecordNativeAsync`, `CreateNativeRecorder`, and
//! `StopRecording`'s native branch), driving the recording frame's
//! [`RecordingControl`] as ShareX drives `ScreenRecordWindow`.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use sharex_screen_recording::session::{RecordingControl, ScreenRecordState, ScreenRecordingStatus, SessionHooks};
use sharex_settings::TaskSettingsCapture;

use crate::options::{Rect, RecordingOptions, TICKS_PER_SECOND};
use crate::recorder::{Backends, RecordError, ScreenRecorder};

/// `RecordNativeAsync`'s `RecordingOptions` from the task's capture settings.
pub fn recording_options(path: &Path, region: Rect, window_handle: isize, c: &TaskSettingsCapture, is_gif: bool) -> RecordingOptions {
    let mut o = RecordingOptions::new(path);
    o.region = region;
    o.window_handle = window_handle;
    o.frames_per_second = if is_gif { c.GIFFPS } else { c.ScreenRecordFPS };
    o.video_bitrate = c.ScreenRecordVideoBitrate.saturating_mul(1000);
    o.capture_system_audio = !is_gif && c.ScreenRecordSystemAudio;
    o.system_audio_device_id = Some(c.ScreenRecordSystemAudioDeviceId.clone());
    o.system_audio_gain = c.ScreenRecordSystemAudioGain;
    o.capture_microphone = !is_gif && c.ScreenRecordMicrophone;
    o.microphone_device_id = Some(c.ScreenRecordMicrophoneDeviceId.clone());
    o.microphone_gain = c.ScreenRecordMicrophoneGain;
    o.capture_camera = c.ScreenRecordCamera;
    o.camera_device_id = Some(c.ScreenRecordCameraDeviceId.clone()).filter(|s| !s.is_empty());
    o.camera_resolution = c.ScreenRecordCameraResolution.clone();
    o.camera_frames_per_second = c.ScreenRecordCameraFPS;
    o.camera_position = c.ScreenRecordCameraPosition.clone();
    o.camera_shape = c.ScreenRecordCameraShape.clone();
    o.camera_width_percent = c.ScreenRecordCameraWidthPercent;
    o.camera_margin = c.ScreenRecordCameraMargin;
    o.require_hardware_encoder = c.ScreenRecordRequireHardwareEncoder;
    o.include_cursor = c.ScreenRecordShowCursor;
    o.hdr_tone_mapping = c.ScreenRecordHDRToneMapping;
    // `TimeSpan.FromSeconds(ScreenRecordDuration)`.
    o.duration = if c.ScreenRecordFixedDuration { (c.ScreenRecordDuration as f64 * TICKS_PER_SECOND as f64) as i64 } else { 0 };
    o
}

/// [`NativeSession::adjust`].
pub type AdjustOptions = Box<dyn Fn(&mut RecordingOptions) + Send>;

/// What the session records.
pub struct NativeSession {
    pub capture: TaskSettingsCapture,
    pub is_gif: bool,
    /// The captured window (active window recordings without client area), else 0.
    pub window_handle: isize,
    pub backends: Backends,
    /// Changes the options before each recorder is made (tests, or a backend that can't do something).
    pub adjust: Option<AdjustOptions>,
    /// `DebugHelper.WriteLine`.
    pub log: Box<dyn Fn(&str) + Send>,
}

fn rect(r: sharex_settings::types::Rectangle) -> Rect {
    Rect::new(r.x, r.y, r.width, r.height)
}

/// `Path.GetFileNameWithoutExtension(path)-recording-<guid>.ext` beside it.
fn recording_path(path: &Path) -> PathBuf {
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = path.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    path.with_file_name(format!("{stem}-recording-{}{ext}", crate::gif::guid_n()))
}

/// The worker of `StartRecording` with the native recorder: the finished
/// file, or `None` (aborted, cancelled or failed; errors go to
/// [`SessionHooks::native_error`]).
pub fn run_native_session(session: NativeSession, control: Arc<RecordingControl>, hooks: &mut dyn SessionHooks) -> Option<String> {
    control.set_in_process_pause(true);
    let extension = if session.is_gif { "gif" } else { "mp4" };
    let path = hooks.output_path(extension)?;
    // `HandleExistsFile` kept an existing file's name: its policy is to overwrite.
    let replace_existing = Path::new(&path).exists();
    match record_native(&session, &control, hooks, Path::new(&path), replace_existing) {
        Ok(true) => None,
        Ok(false) => Some(path),
        Err(e) => {
            (session.log)(&e);
            hooks.native_error(&e);
            None
        }
    }
}

/// `RecordNativeAsync`: whether it was aborted (or produced nothing).
fn record_native(session: &NativeSession, control: &Arc<RecordingControl>, hooks: &mut dyn SessionHooks, path: &Path, replace_existing: bool) -> Result<bool, String> {
    // Commit a completed recording afterwards so aborts, restarts and startup errors keep an existing file.
    let recording = recording_path(path);
    let current: Arc<Mutex<Option<crate::recorder::RecorderHandle>>> = Arc::default();
    {
        // `StopRecording`'s native branch.
        let (cur, c) = (current.clone(), Arc::downgrade(control));
        control.on_stop_requested(move || {
            let Some(c) = c.upgrade() else { return };
            if let Some(r) = cur.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
                if c.status() == ScreenRecordingStatus::Aborted || c.restart_requested() {
                    r.request_abort();
                } else {
                    r.request_stop();
                }
            }
        });
        let cur = current.clone();
        control.on_pause_requested(move || {
            if let Some(r) = cur.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
                r.pause();
            }
        });
        let (cur, c, window) = (current.clone(), Arc::downgrade(control), session.window_handle);
        control.on_resume_requested(move || {
            let Some(c) = c.upgrade() else { return };
            if let Some(r) = cur.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
                if window == 0 {
                    if let Err(e) = r.resume_at(rect(c.recording_region())) {
                        eprintln!("Native recorder: {e}");
                    }
                } else {
                    r.resume();
                }
            }
        });
    }
    let make = |region: Rect| -> Result<ScreenRecorder, String> {
        let mut options = recording_options(&recording, region, session.window_handle, &session.capture, session.is_gif);
        if let Some(adjust) = &session.adjust {
            adjust(&mut options);
        }
        let recorder = ScreenRecorder::new(options, session.backends.clone())?;
        recorder.on_diagnostic(|m| eprintln!("Native recorder: {m}"));
        let c = control.clone();
        recorder.on_encoding_progress(move |p| {
            if p == 0 {
                c.change_state(ScreenRecordState::RecordingEnd);
                c.change_state(ScreenRecordState::Encoding);
            }
            c.emit_progress(p);
        });
        *current.lock().unwrap_or_else(|e| e.into_inner()) = Some(recorder.handle());
        Ok(recorder)
    };
    let canceled = |e: &RecordError| {
        matches!(e, RecordError::Canceled(_))
            && (matches!(control.status(), ScreenRecordingStatus::Aborted | ScreenRecordingStatus::Stopped) || control.restart_requested())
    };
    let capture = &session.capture;
    loop {
        let mut completed = false;
        let region = rect(control.recording_region());
        let mut recorder = make(region)?;
        let r = (|| -> Result<Option<bool>, RecordError> {
            control.change_state(ScreenRecordState::BeforeStart);
            // Set up the GPU, codecs and audio endpoints before the countdown.
            recorder.prepare()?;
            let delay = if capture.ScreenRecordStartDelayEnabled { (capture.ScreenRecordStartDelay * 1000.0) as i32 } else { 0 };
            if delay > 0 {
                control.emit_countdown(delay as u32);
                control.wait_reset(Some(Duration::from_millis(delay as u64)));
            }
            control.consume_restart_request();
            if matches!(control.status(), ScreenRecordingStatus::Aborted | ScreenRecordingStatus::Stopped) {
                return Ok(Some(true));
            }
            // A waiting recording region can be dragged: rebuild for where it is now.
            let now = rect(control.recording_region());
            if session.window_handle == 0 && now != region {
                drop(std::mem::replace(&mut recorder, make(now)?));
                recorder.prepare()?;
            }
            control.change_state(ScreenRecordState::AfterStart);
            recorder.start()?;
            if control.status() == ScreenRecordingStatus::Aborted || control.restart_requested() {
                recorder.request_abort();
            } else if control.status() == ScreenRecordingStatus::Stopped {
                recorder.request_stop();
            } else {
                control.change_state(ScreenRecordState::AfterRecordingStart);
            }
            let result = recorder.completion()?;
            completed = true;
            (session.log)(&format!(
                "Native screen recording completed. Encoder: {}, hardware: {}, frames: {}, dropped: {}, audio discontinuities: {}",
                result.encoder.name, result.encoder.is_hardware_accelerated, result.video_frames, result.dropped_video_frames, result.audio_discontinuities
            ));
            Ok(None)
        })();
        *current.lock().unwrap_or_else(|e| e.into_inner()) = None;
        drop(recorder);
        control.change_state(ScreenRecordState::RecordingEnd);
        match r {
            Ok(Some(aborted)) => return Ok(aborted),
            Ok(None) => {}
            // A stop during preparation or before the first frame removes the incomplete file.
            Err(e) if canceled(&e) => {}
            Err(e) => return Err(e.to_string()),
        }
        if control.restart_requested() {
            let _ = std::fs::remove_file(&recording);
            continue;
        }
        hooks.action_completed();
        let aborted = control.status() == ScreenRecordingStatus::Aborted;
        if aborted {
            let _ = std::fs::remove_file(&recording);
        }
        if !aborted && completed {
            let moved = if replace_existing { std::fs::rename(&recording, path) } else { move_new(&recording, path) };
            if let Err(e) = moved {
                return Err(format!(
                    "The recording finished, but its output could not be moved to '{}'. The completed recording is available at '{}'. ({e})",
                    path.display(),
                    recording.display()
                ));
            }
        }
        return Ok(aborted || !completed);
    }
}

/// `File.Move(source, destination, overwrite: false)`.
fn move_new(source: &Path, destination: &Path) -> std::io::Result<()> {
    if destination.exists() {
        return Err(std::io::Error::new(std::io::ErrorKind::AlreadyExists, "The file already exists."));
    }
    std::fs::rename(source, destination)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::VirtualTime;
    use crate::media::CpuMediaFactory;
    use crate::source::SyntheticSources;
    use sharex_settings::types::Rectangle;

    struct Hooks {
        dir: PathBuf,
        errors: Vec<String>,
        completed: usize,
    }

    impl SessionHooks for Hooks {
        fn output_path(&mut self, extension: &str) -> Option<String> {
            Some(self.dir.join(format!("out.{extension}")).to_string_lossy().into_owned())
        }
        fn two_pass_output_path(&mut self, _: &str) -> String {
            unreachable!()
        }
        fn action_completed(&mut self) {
            self.completed += 1;
        }
        fn native_error(&mut self, message: &str) {
            self.errors.push(message.into());
        }
    }

    fn session(t: &Arc<VirtualTime>, gif: bool, tick: Option<crate::recorder::TickHook>) -> NativeSession {
        let capture = TaskSettingsCapture { ScreenRecordStartDelayEnabled: false, ScreenRecordSystemAudio: false, ..Default::default() };
        NativeSession {
            capture,
            is_gif: gif,
            window_handle: 0,
            backends: Backends {
                capture: Arc::new(SyntheticSources { capture_fps: 60 }),
                audio: None,
                camera: None,
                media: Arc::new(CpuMediaFactory::default()),
                time: t.clone(),
                virtual_time: Some(t.clone()),
                tick,
            },
            adjust: None,
            log: Box::new(|_| {}),
        }
    }

    fn dir() -> PathBuf {
        let d = std::env::temp_dir().join(format!("sharexrs-native-session-{}", crate::gif::guid_n()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn files(d: &Path) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(d).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        v.sort();
        v
    }

    /// Presses the frame's buttons from the recorder's own loop.
    type Step = (i64, fn(&RecordingControl));

    /// Each step fires once the recorder's elapsed time reaches it, and only
    /// while the frame shows a recording (Recording or Paused), as a user
    /// sees it. Virtual time lets the recorder's worker run ahead of the
    /// session thread, which sets Recording after `start` returns; a press
    /// while still Working would be an abort (ShareX's Working branch).
    fn drive(control: &Arc<RecordingControl>, script: Vec<Step>) -> crate::recorder::TickHook {
        let (c, step) = (Arc::downgrade(control), Mutex::new(0usize));
        Arc::new(move |r| {
            let Some(c) = c.upgrade() else { return };
            if !matches!(c.status(), ScreenRecordingStatus::Recording | ScreenRecordingStatus::Paused) {
                return;
            }
            let mut s = step.lock().unwrap();
            if let Some((at, action)) = script.get(*s)
                && r.elapsed() >= *at
            {
                *s += 1;
                action(&c);
            }
        })
    }

    #[test]
    fn records_pauses_and_stops() {
        let d = dir();
        let t = VirtualTime::new(TICKS_PER_SECOND);
        let control = RecordingControl::new(Rectangle { x: 0, y: 0, width: 64, height: 48 });
        let tick = drive(&control, vec![
            (TICKS_PER_SECOND / 4, |c| c.pause_resume_recording()),
            (0, |c| {
                c.set_recording_region(Rectangle { x: 10, y: 10, width: 64, height: 48 });
                c.pause_resume_recording()
            }),
            (TICKS_PER_SECOND / 2, |c| c.start_stop_recording()),
        ]);
        let mut hooks = Hooks { dir: d.clone(), errors: vec![], completed: 0 };
        let out = run_native_session(session(&t, false, Some(tick)), control.clone(), &mut hooks);
        assert_eq!(hooks.errors, Vec::<String>::new());
        assert_eq!(out, Some(d.join("out.mp4").to_string_lossy().into_owned()));
        assert_eq!(files(&d), ["out.mp4"]);
        assert_eq!(hooks.completed, 1);
        assert_eq!(control.status(), ScreenRecordingStatus::Stopped);
    }

    #[test]
    fn abort_keeps_nothing() {
        let d = dir();
        let t = VirtualTime::new(TICKS_PER_SECOND);
        let control = RecordingControl::new(Rectangle { x: 0, y: 0, width: 64, height: 48 });
        let tick = drive(&control, vec![(TICKS_PER_SECOND / 4, |c| c.abort_recording())]);
        let mut hooks = Hooks { dir: d.clone(), errors: vec![], completed: 0 };
        assert_eq!(run_native_session(session(&t, true, Some(tick)), control, &mut hooks), None);
        assert!(hooks.errors.is_empty(), "{:?}", hooks.errors);
        assert!(files(&d).is_empty());
    }

    #[test]
    fn restart_records_again() {
        let d = dir();
        let t = VirtualTime::new(TICKS_PER_SECOND);
        let control = RecordingControl::new(Rectangle { x: 0, y: 0, width: 64, height: 48 });
        let tick = drive(&control, vec![
            (TICKS_PER_SECOND / 4, |c| c.restart_recording()),
            (TICKS_PER_SECOND / 4, |c| c.start_stop_recording()),
        ]);
        let mut hooks = Hooks { dir: d.clone(), errors: vec![], completed: 0 };
        let out = run_native_session(session(&t, true, Some(tick)), control, &mut hooks);
        assert!(hooks.errors.is_empty(), "{:?}", hooks.errors);
        assert_eq!(out, Some(d.join("out.gif").to_string_lossy().into_owned()));
        assert_eq!(files(&d), ["out.gif"]);
    }

    #[test]
    fn errors_reach_the_hooks() {
        let d = dir();
        let t = VirtualTime::new(TICKS_PER_SECOND);
        let control = RecordingControl::new(Rectangle { x: 0, y: 0, width: 64, height: 48 });
        let mut s = session(&t, false, None);
        s.capture.ScreenRecordSystemAudio = true;
        let mut hooks = Hooks { dir: d.clone(), errors: vec![], completed: 0 };
        assert_eq!(run_native_session(s, control, &mut hooks), None);
        assert_eq!(hooks.errors, ["Windows audio capture is unavailable."]);
        assert!(files(&d).is_empty());
    }
}
