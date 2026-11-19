//! `ScreenRecorder.cs`: one native recording. `prepare` sets everything up
//! (during the start delay), `start` begins capturing, `pause`/`resume`
//! (optionally at a moved region of the same size) keep the encoders and
//! media timeline alive, `request_stop` finishes the file, `request_abort`
//! discards it; `completion` gives what was written.
//!
//! Frames are taken on a fixed grid of `1/fps` slots of media time; late
//! slots are dropped, never caught up. Audio follows the video's clock 50 ms
//! behind (WASAPI packets arrive late), in 10 ms blocks.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;

use crate::audio::{AudioCapture, AudioDevices, EndpointCapture};
use crate::camera::{CameraDevices, CameraSource};
use crate::clock::{RecordingClock, TimeSource, VirtualTime};
use crate::media::MediaFactory;
use crate::mixer::{AudioMixer, write_audio_until};
use crate::options::{Rect, RecordingOptions, RecordingResult, TICKS_PER_MILLISECOND, TICKS_PER_SECOND, VideoEncoderInfo};
use crate::signals::{Signals, Waiter, Wake};
use crate::source::CaptureSources;

/// Why a recording ended without a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordError {
    /// `OperationCanceledException`: stopped before it began, or aborted.
    Canceled(String),
    Failed(String),
}

impl std::fmt::Display for RecordError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Canceled(m) | Self::Failed(m) => f.write_str(m),
        }
    }
}

impl From<&str> for RecordError {
    fn from(s: &str) -> Self {
        Self::from(s.to_string())
    }
}

impl From<String> for RecordError {
    fn from(s: String) -> Self {
        if s == crate::ABORTED { Self::Canceled(s) } else { Self::Failed(s) }
    }
}

/// A `TaskCompletionSource`.
struct Promise<T: Clone> {
    value: Mutex<Option<Result<T, RecordError>>>,
    cond: Condvar,
}

impl<T: Clone> Promise<T> {
    fn new() -> Self {
        Self { value: Mutex::new(None), cond: Condvar::new() }
    }
    fn set(&self, v: Result<T, RecordError>) {
        let mut g = self.value.lock().unwrap_or_else(|e| e.into_inner());
        if g.is_none() {
            *g = Some(v);
            self.cond.notify_all();
        }
    }
    fn wait(&self) -> Result<T, RecordError> {
        let mut g = self.value.lock().unwrap_or_else(|e| e.into_inner());
        while g.is_none() {
            g = self.cond.wait(g).unwrap_or_else(|e| e.into_inner());
        }
        g.clone().expect("set")
    }
    fn get(&self) -> Option<Result<T, RecordError>> {
        self.value.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

/// Where a recording's frames, sound and camera come from, and what encodes them.
#[derive(Clone)]
pub struct Backends {
    pub capture: Arc<dyn CaptureSources>,
    /// `None`: a recording that asks for sound fails as WASAPI would without a device.
    pub audio: Option<Arc<dyn AudioDevices>>,
    /// `None`: a recording that asks for the camera fails as ShareX does without one.
    pub camera: Option<Arc<dyn CameraDevices>>,
    pub media: Arc<dyn MediaFactory>,
    pub time: Arc<dyn TimeSource>,
    /// Run on a [`VirtualTime`] (tests): waits move the clock, audio is drained in line.
    pub virtual_time: Option<Arc<VirtualTime>>,
    /// Called by the worker before each wait (tests drive pause/stop from it).
    pub tick: Option<TickHook>,
}

impl Backends {
    /// The platform's encoders (Media Foundation and the GPU on Windows) on the real clock.
    #[cfg(windows)]
    pub fn system(capture: Arc<dyn CaptureSources>) -> Self {
        Self {
            capture,
            audio: None,
            camera: None,
            media: Arc::new(crate::windows::GpuMediaFactory),
            time: Arc::new(crate::clock::SystemTime),
            virtual_time: None,
            tick: None,
        }
    }
}

/// [`Backends::tick`].
pub type TickHook = Arc<dyn Fn(&RecorderHandle) + Send + Sync>;

type Callback<T> = Arc<Mutex<Option<Box<dyn FnMut(T) + Send>>>>;

struct Control {
    resume_requested: bool,
    resume_region: Option<Rect>,
    start_requested: bool,
    disposed: bool,
}

struct Shared {
    options: RecordingOptions,
    clock: Arc<RecordingClock>,
    signals: Arc<Signals>,
    control: Mutex<Control>,
    prepared: Promise<()>,
    started: Promise<()>,
    completed: Promise<RecordingResult>,
    recording: AtomicBool,
    aborted: AtomicBool,
    encoder: Mutex<Option<VideoEncoderInfo>>,
    diagnostic: Callback<String>,
    progress: Callback<i32>,
}

/// The controls a recording takes from other threads (the frame, hotkeys).
#[derive(Clone)]
pub struct RecorderHandle {
    shared: Arc<Shared>,
}

/// `ScreenRecorder`.
pub struct ScreenRecorder {
    handle: RecorderHandle,
    backends: Backends,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl std::ops::Deref for ScreenRecorder {
    type Target = RecorderHandle;
    fn deref(&self) -> &RecorderHandle {
        &self.handle
    }
}

impl ScreenRecorder {
    pub fn new(options: RecordingOptions, backends: Backends) -> Result<Self, String> {
        options.validate()?;
        let mut options = options;
        options.output_path = std::path::absolute(&options.output_path).map_err(|e| e.to_string())?;
        let clock = Arc::new(RecordingClock::new(backends.time.clone()));
        Ok(Self {
            handle: RecorderHandle { shared: Arc::new(Shared {
                options,
                clock,
                signals: Signals::new(),
                control: Mutex::new(Control { resume_requested: false, resume_region: None, start_requested: false, disposed: false }),
                prepared: Promise::new(),
                started: Promise::new(),
                completed: Promise::new(),
                recording: AtomicBool::new(false),
                aborted: AtomicBool::new(false),
                encoder: Mutex::new(None),
                diagnostic: Arc::new(Mutex::new(None)),
                progress: Arc::new(Mutex::new(None)),
            }) },
            backends,
            worker: Mutex::new(None),
        })
    }

    pub fn handle(&self) -> RecorderHandle {
        self.handle.clone()
    }

    /// `PrepareAsync`: starts the worker, waits until it's ready to capture.
    pub fn prepare(&self) -> Result<(), RecordError> {
        {
            let mut w = self.worker.lock().unwrap_or_else(|e| e.into_inner());
            if w.is_none() {
                let (handle, backends) = (self.handle.clone(), self.backends.clone());
                *w = Some(
                    std::thread::Builder::new()
                        .name("ShareX native recording".into())
                        .spawn(move || run(&handle, &backends))
                        .map_err(|e| RecordError::Failed(e.to_string()))?,
                );
            }
        }
        self.shared.prepared.wait()
    }

    /// `StartAsync`: begins capturing; returns once the first frame is written.
    pub fn start(&self) -> Result<(), RecordError> {
        self.prepare()?;
        {
            let mut c = self.shared.control.lock().unwrap_or_else(|e| e.into_inner());
            if c.start_requested {
                return Err(RecordError::Failed("A recorder instance can only be started once.".into()));
            }
            c.start_requested = true;
            self.shared.signals.set_begin();
        }
        self.shared.started.wait()
    }

    /// `Completion`: waits for the worker's result.
    pub fn completion(&self) -> Result<RecordingResult, RecordError> {
        if self.worker.lock().unwrap_or_else(|e| e.into_inner()).is_none() {
            return Err(RecordError::Failed("The recorder has not been prepared.".into()));
        }
        self.shared.completed.wait()
    }

    /// `StopAsync`.
    pub fn stop(&self) -> Result<RecordingResult, RecordError> {
        self.request_stop();
        self.completion()
    }
}

impl RecorderHandle {
    /// `Diagnostic`: nonfatal camera errors.
    pub fn on_diagnostic(&self, f: impl FnMut(String) + Send + 'static) {
        *self.shared.diagnostic.lock().unwrap_or_else(|e| e.into_inner()) = Some(Box::new(f));
    }

    /// `EncodingProgressChanged`: GIF encoding after capture stopped, 0 to 100.
    pub fn on_encoding_progress(&self, f: impl FnMut(i32) + Send + 'static) {
        *self.shared.progress.lock().unwrap_or_else(|e| e.into_inner()) = Some(Box::new(f));
    }

    pub fn encoder(&self) -> Option<VideoEncoderInfo> {
        self.shared.encoder.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
    pub fn is_recording(&self) -> bool {
        self.shared.recording.load(Ordering::SeqCst)
    }
    pub fn is_paused(&self) -> bool {
        self.shared.clock.is_paused()
    }
    /// `Elapsed`, in ticks.
    pub fn elapsed(&self) -> i64 {
        self.shared.clock.elapsed()
    }
    pub fn options(&self) -> &RecordingOptions {
        &self.shared.options
    }

    pub fn pause(&self) {
        let mut c = self.shared.control.lock().unwrap_or_else(|e| e.into_inner());
        if c.disposed {
            return;
        }
        c.resume_requested = false;
        c.resume_region = None;
        self.shared.clock.pause();
        self.shared.signals.set_changed();
    }

    pub fn resume(&self) {
        self.request_resume(None);
    }

    /// `Resume(Rectangle)`: resume desktop capture at a new position, same output size.
    pub fn resume_at(&self, region: Rect) -> Result<(), String> {
        let o = &self.shared.options;
        if o.window_handle != 0 {
            return Err("Window capture follows its target window rather than a desktop region.".into());
        }
        let mask = if o.is_gif() { -1 } else { !1 };
        if (region.width & mask) != (o.region.width & mask) || (region.height & mask) != (o.region.height & mask) {
            return Err("Moving the recording region cannot change its output dimensions. (Parameter 'region')".into());
        }
        self.request_resume(Some(region));
        Ok(())
    }

    fn request_resume(&self, region: Option<Rect>) {
        let mut c = self.shared.control.lock().unwrap_or_else(|e| e.into_inner());
        if c.disposed || !self.shared.clock.is_paused() {
            return;
        }
        c.resume_region = region;
        c.resume_requested = true;
        self.shared.signals.set_changed();
    }

    /// Nonblocking stop; [`Self::completion`] finalises the output.
    pub fn request_stop(&self) {
        if !self.shared.control.lock().unwrap_or_else(|e| e.into_inner()).disposed {
            self.shared.signals.set_stop();
        }
    }

    /// Discard this recording and cancel any pending GIF encoding.
    pub fn request_abort(&self) {
        self.shared.aborted.store(true, Ordering::SeqCst);
        self.request_stop();
    }

    /// The result if the worker has finished.
    pub fn try_completion(&self) -> Option<Result<RecordingResult, RecordError>> {
        self.shared.completed.get()
    }
}

impl Drop for ScreenRecorder {
    fn drop(&mut self) {
        self.shared.signals.set_stop();
        if let Some(w) = self.worker.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = w.join();
        }
        self.shared.control.lock().unwrap_or_else(|e| e.into_inner()).disposed = true;
    }
}

fn report<T>(cb: &Callback<T>, v: T) {
    if let Some(f) = cb.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        f(v);
    }
}

/// `Run`.
fn run(handle: &RecorderHandle, backends: &Backends) {
    let shared = &handle.shared;
    let mut owns_output = false;
    let result = record(handle, backends, &mut owns_output);
    shared.clock.stop();
    shared.recording.store(false, Ordering::SeqCst);
    match result {
        Ok(r) => shared.completed.set(Ok(r)),
        Err(mut e) => {
            if owns_output && let Err(cleanup) = std::fs::remove_file(&shared.options.output_path) {
                e = RecordError::Failed(format!("Recording failed and its incomplete output could not be removed. ({e}) ({cleanup})"));
            }
            shared.prepared.set(Err(e.clone()));
            shared.started.set(Err(e.clone()));
            shared.completed.set(Err(e));
        }
    }
}

fn record(handle: &RecorderHandle, backends: &Backends, owns_output: &mut bool) -> Result<RecordingResult, RecordError> {
    let shared = &handle.shared;
    let tick = || {
        if let Some(t) = &backends.tick {
            t(handle);
        }
    };
    let options = &shared.options;
    let clock = &shared.clock;
    let signals = &shared.signals;
    let fps = options.frames_per_second as i64;
    let mut waiter = match &backends.virtual_time {
        Some(t) => Waiter::Virtual(t.clone()),
        None => Waiter::real()?,
    };
    let mut capture = backends.capture.open(options, backends.time.clone())?;
    let (width, height) = capture.size();
    if let Some(dir) = options.output_path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    // Reserve the name atomically. Never overwrite or delete a pre-existing user file.
    std::fs::OpenOptions::new().write(true).create_new(true).open(&options.output_path).map_err(|e| e.to_string())?;
    *owns_output = true;
    let mut media = backends.media.create(options, width, height, capture.monitor())?;
    *shared.encoder.lock().unwrap_or_else(|e| e.into_inner()) = Some(media.encoder());
    capture.attach(media.canvas())?;
    let mut camera: Option<Box<dyn CameraSource>> = if options.capture_camera {
        let devices = backends.camera.as_ref().ok_or("No camera is available. Connect a camera or turn off the camera overlay.")?;
        Some(devices.open(options, media.canvas())?)
    } else {
        None
    };
    if let Some(c) = &camera {
        media.create_camera_overlay(c.mode(), options)?;
    }
    let mixer = Arc::new(AudioMixer::new(clock.clone()));
    let mut audio: Option<Box<dyn AudioCapture>> = if options.has_audio() {
        let devices = backends.audio.as_ref().ok_or("Windows audio capture is unavailable.")?;
        Some(match devices.capture(options, mixer.clone(), backends.time.clone()) {
            Some(capture) => capture?,
            None => Box::new(EndpointCapture::open(devices.as_ref(), options, mixer.clone(), backends.time.clone(), backends.virtual_time.is_none())?),
        })
    } else {
        None
    };
    shared.prepared.set(Ok(()));
    if waiter.wait_begin_or_stop(signals) {
        return Err(RecordError::Canceled("Recording stopped before capture began.".into()));
    }
    if let Some(c) = &mut camera {
        c.start()?;
    }
    if let Some(a) = &mut audio {
        a.start();
    }
    capture.start()?;
    let wait_start = clock.now();
    while !capture.has_frame() {
        tick();
        if waiter.stop_wait(signals, 5) {
            return Err(RecordError::Canceled("Recording stopped before the first video frame.".into()));
        }
        if let Some(f) = audio.as_ref().and_then(|a| a.failure()) {
            return Err(RecordError::Failed(format!("Windows audio capture failed. ({f})")));
        }
        capture.update(media.canvas())?;
        if capture.has_ended() {
            return Err(RecordError::Failed("The capture target closed or changed size before recording started.".into()));
        }
        if clock.now() - wait_start > 10 * TICKS_PER_SECOND {
            return Err(RecordError::Failed("Windows did not deliver a capture frame. The window may be minimized, protected, or unavailable.".into()));
        }
    }
    clock.start(capture.latest_timestamp());
    shared.recording.store(true, Ordering::SeqCst);
    let (mut next_frame, mut written, mut dropped, mut audio_frame) = (0i64, 0i64, 0i64, 0i64);
    let mut discontinuity = false;
    let mut camera_disabled = false;
    let (mut next_camera_frame, mut last_camera_frame) = (0i64, 0i64);
    // 10 ms, 48 kHz stereo.
    let mut audio_block = vec![0i16; 480 * 2];
    let mut duration = 0i64;
    let mut current_region = options.region;
    let mut resume_wait_start = 0i64;
    let has_audio = options.has_audio();
    let camera_fps = options.camera_frames_per_second as i64;
    let write_audio = |media: &mut Box<dyn crate::media::Media>, audio: &mut Option<Box<dyn AudioCapture>>, audio_frame: &mut i64, block: &mut [i16], until: i64, partial: bool| {
        if !has_audio || media.is_gif() {
            return Ok(());
        }
        if let Some(a) = audio.as_mut() {
            a.poll();
        }
        write_audio_until(&mixer, block, audio_frame, until, partial, &mut |pcm, offset| media.write_audio(pcm, offset))
    };
    while !signals.is_stopped() {
        if capture.has_ended() {
            break;
        }
        if let Some(f) = audio.as_ref().and_then(|a| a.failure()) {
            return Err(RecordError::Failed(format!("Windows audio capture failed (the endpoint may have changed or been disconnected). ({f})")));
        }
        if clock.is_paused() {
            let requested = {
                let c = shared.control.lock().unwrap_or_else(|e| e.into_inner());
                if c.resume_requested { c.resume_region } else { None }
            };
            if let Some(region) = requested
                && region != current_region
            {
                // Only the capture changes; the GPU, encoder, camera, audio and media timeline stay.
                capture.retarget(region, media.canvas())?;
                current_region = region;
                resume_wait_start = clock.now();
            }
            // Drain the capture so resume never shows frames retained during the pause.
            capture.update(media.canvas())?;
            if capture.has_ended() {
                break;
            }
            if !capture.has_frame() && clock.now() - resume_wait_start > 10 * TICKS_PER_SECOND {
                return Err(RecordError::Failed("Windows did not deliver a frame for the moved recording region.".into()));
            }
            write_audio(&mut media, &mut audio, &mut audio_frame, &mut audio_block, clock.elapsed(), false)?;
            {
                let mut c = shared.control.lock().unwrap_or_else(|e| e.into_inner());
                // Keep media time paused until the new region has a frame.
                if c.resume_requested && c.resume_region == requested && capture.has_frame() {
                    clock.resume();
                    c.resume_requested = false;
                    c.resume_region = None;
                }
            }
            discontinuity = true;
            if !clock.is_paused() {
                continue;
            }
            tick();
            if waiter.wait(signals, 20 * TICKS_PER_MILLISECOND) == Wake::Stop {
                break;
            }
            continue;
        }
        let elapsed = clock.elapsed();
        if options.duration > 0 && elapsed >= options.duration {
            break;
        }
        let frame_time = next_frame * TICKS_PER_SECOND / fps;
        if elapsed >= frame_time {
            // Skip overdue frame slots. Never accelerate playback or queue a catch-up burst.
            let due_frame = elapsed * fps / TICKS_PER_SECOND;
            if written > 0 && due_frame > next_frame {
                dropped += due_frame - next_frame;
                next_frame = due_frame;
                discontinuity = true;
            }
            capture.update(media.canvas())?;
            let timestamp = next_frame * TICKS_PER_SECOND / fps;
            if capture.has_ended() {
                break;
            }
            let end = (next_frame + 1) * TICKS_PER_SECOND / fps;
            if media.try_rent(next_frame) {
                if let Some(cam) = camera.as_mut().filter(|_| !camera_disabled) {
                    let r = (|| -> Result<(), String> {
                        if let Some(f) = cam.failure() {
                            return Err(f);
                        }
                        // The camera has its own cadence. At high screen FPS, reuse its existing tile.
                        if elapsed >= next_camera_frame {
                            if let Some(frame) = cam.take_latest_frame() {
                                media.camera_update(&frame)?;
                                last_camera_frame = elapsed;
                            }
                            // Align to media-clock slots so a late update doesn't push the deadline on.
                            let next_camera_slot = elapsed * camera_fps / TICKS_PER_SECOND + 1;
                            next_camera_frame = (next_camera_slot * TICKS_PER_SECOND + camera_fps - 1) / camera_fps;
                        }
                        if elapsed - last_camera_frame > 10 * TICKS_PER_SECOND {
                            return Err("The camera did not deliver a new frame for ten seconds.".into());
                        }
                        Ok(())
                    })();
                    if let Err(e) = r {
                        camera_disabled = true;
                        media.camera_disable();
                        report(&shared.diagnostic, format!("Camera overlay disabled; screen and audio recording continue. {e}"));
                        camera = None;
                    }
                }
                if !media.write_video(next_frame, timestamp, end, discontinuity)? {
                    dropped += 1;
                    next_frame += 1;
                    continue;
                }
                written += 1;
                duration = end;
                discontinuity = false;
                shared.started.set(Ok(()));
            } else {
                dropped += 1;
                discontinuity = true;
            }
            next_frame += 1;
        }
        // Allow 50 ms for WASAPI packets to arrive; timestamps stay aligned with video.
        write_audio(&mut media, &mut audio, &mut audio_frame, &mut audio_block, 0i64.max(clock.elapsed() - 50 * TICKS_PER_MILLISECOND), false)?;
        let remaining = next_frame * TICKS_PER_SECOND / fps - clock.elapsed();
        tick();
        if waiter.wait(signals, remaining) == Wake::Stop {
            break;
        }
    }
    // Stop and drain capture before writing the final audio blocks and finalising the container.
    if let Some(a) = &mut audio {
        a.stop();
    }
    let audio_failure = audio.as_ref().and_then(|a| a.failure());
    drop(audio);
    clock.stop();
    shared.recording.store(false, Ordering::SeqCst);
    let elapsed = clock.elapsed();
    duration = duration.max(if options.duration > 0 { elapsed.min(options.duration) } else { elapsed });
    if shared.aborted.load(Ordering::SeqCst) {
        return Err(RecordError::Canceled("Recording aborted.".into()));
    }
    let _ = audio_failure;
    let mut none = None;
    write_audio(&mut media, &mut none, &mut audio_frame, &mut audio_block, duration, true)?;
    if written == 0 {
        return Err(RecordError::Failed("No video frames were accepted by the encoder.".into()));
    }
    media.finish_video()?;
    if media.is_gif() {
        drop(capture);
        drop(camera);
        let aborted = || shared.aborted.load(Ordering::SeqCst);
        media.finish_gif(duration, &mut |p| report(&shared.progress, p), &aborted)?;
    }
    let encoder = media.encoder();
    drop(media);
    Ok(RecordingResult {
        output_path: options.output_path.clone(),
        duration,
        video_frames: written,
        dropped_video_frames: dropped,
        audio_discontinuities: mixer.discontinuities(),
        encoder,
    })
}

#[cfg(test)]
#[path = "recorder_tests.rs"]
mod tests;
