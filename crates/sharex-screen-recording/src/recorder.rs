//! `ShareX.ScreenCaptureLib/ScreenRecording/ScreenRecorder.cs`, FFmpeg
//! output: one FFmpeg run per recording segment, stopped with `q`, and the
//! second pass of a two-pass (or GIF) recording.
//!
//! ShareX's other output (`ScreenRecordOutput.GIF`, screenshots into a
//! `HardDiskCache` encoded by `AnimatedGifCreator`) is never constructed in
//! ShareX 22.4 (`ScreenRecordManager` always passes `FFmpeg`; GIFs go
//! through FFmpeg's palettegen or the native recorder), so it isn't ported.

use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, Mutex};

use crate::args::{DesktopScreen, ScreenRecordingOptions, gif_encode_args};
use crate::cli::FFmpegCliManager;

type Handler<T> = Arc<Mutex<Option<Box<dyn Fn(T) + Send + Sync>>>>;
type RegistryWriter = Box<dyn Fn(&str, &[(&str, i32)]) + Send + Sync>;

/// Where the video and sound come from: ShareX's sources (gdigrab, ddagrab,
/// DirectShow; Windows), or a [`crate::platform`] input.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum RecorderInput {
    #[default]
    ShareX,
    Platform(crate::platform::Input, Option<crate::platform::AudioInput>),
}

/// `ScreenRecorder` with `ScreenRecordOutput.FFmpeg`.
pub struct ScreenRecorder {
    options: Mutex<ScreenRecordingOptions>,
    screens: Vec<DesktopScreen>,
    ffmpeg: FFmpegCliManager,
    is_recording: AtomicBool,
    recording_started: Handler<()>,
    encoding_progress: Handler<i32>,
    registry_writer: Mutex<RegistryWriter>,
    input: Mutex<RecorderInput>,
}

impl ScreenRecorder {
    /// `new ScreenRecorder(ScreenRecordOutput.FFmpeg, options, ...)`:
    /// creates the output's folder and the FFmpeg runner (`ShowError` on).
    /// `ffmpeg_path` is ShareX's `FFmpegOptions.FFmpegPath`
    /// ([`crate::discovery::ffmpeg_path`]) or what
    /// [`crate::discovery::resolve_ffmpeg`] found; `screens` are the
    /// monitors (for ddagrab).
    pub fn new(
        options: ScreenRecordingOptions,
        screens: Vec<DesktopScreen>,
        ffmpeg_path: impl Into<std::path::PathBuf>,
    ) -> Result<Self, String> {
        let output = options.output_path.clone().unwrap_or_default();
        if output.is_empty() {
            return Err("Screen recorder cache path is empty.".into());
        }
        create_directory_from_file_path(&output);
        let ffmpeg = FFmpegCliManager::new(ffmpeg_path);
        ffmpeg.set_show_error(true);
        let recording_started: Handler<()> = Arc::default();
        let encoding_progress: Handler<i32> = Arc::default();
        {
            let started = recording_started.clone();
            ffmpeg.on_encode_started(move || {
                if let Some(h) = started.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
                    h(());
                }
            });
            let progress = encoding_progress.clone();
            // `previousProgress`, 0 at first, so 0% is never reported first.
            let previous = AtomicI32::new(0);
            ffmpeg.on_encode_progress(move |p| on_encoding_progress_changed(&progress, &previous, p));
        }
        Ok(Self {
            options: Mutex::new(options),
            screens,
            ffmpeg,
            is_recording: AtomicBool::new(false),
            recording_started,
            encoding_progress,
            registry_writer: Mutex::new(Box::new(write_registry_dwords)),
            input: Mutex::default(),
        })
    }

    /// Records from a [`crate::platform`] input instead of ShareX's sources.
    pub fn set_input(&self, input: RecorderInput) {
        *self.input.lock().unwrap_or_else(|e| e.into_inner()) = input;
    }

    /// The command line this recorder runs for `options`.
    fn commands(&self, options: &ScreenRecordingOptions) -> Option<String> {
        match &*self.input.lock().unwrap_or_else(|e| e.into_inner()) {
            RecorderInput::ShareX => options.get_ffmpeg_commands(&self.screens),
            RecorderInput::Platform(input, audio) => crate::platform::platform_ffmpeg_commands(options, input, audio.as_ref()),
        }
    }

    /// The FFmpeg runner (for its output, or to watch its lines).
    pub fn ffmpeg(&self) -> &FFmpegCliManager {
        &self.ffmpeg
    }

    pub fn options(&self) -> ScreenRecordingOptions {
        self.options.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn is_recording(&self) -> bool {
        self.is_recording.load(Ordering::SeqCst)
    }

    /// `RecordingStarted`: FFmpeg began recording ("Press [q] to stop").
    pub fn on_recording_started(&self, f: impl Fn() + Send + Sync + 'static) {
        *self.recording_started.lock().unwrap_or_else(|e| e.into_inner()) = Some(Box::new(move |()| f()));
    }

    /// `EncodingProgressChanged`: the second pass's progress, whole
    /// percents, only when it changes.
    pub fn on_encoding_progress(&self, f: impl Fn(i32) + Send + Sync + 'static) {
        *self.encoding_progress.lock().unwrap_or_else(|e| e.into_inner()) = Some(Box::new(f));
    }

    /// `StartRecording`: runs FFmpeg until it ends (stopped, the duration
    /// is up, or it failed); whether it exited cleanly (ShareX ignores it).
    pub fn start_recording(&self) -> bool {
        let mut result = false;
        if !self.is_recording.swap(true, Ordering::SeqCst) {
            let options = self.options();
            let values = options.screen_capture_recorder_registry();
            if !values.is_empty() {
                let w = self.registry_writer.lock().unwrap_or_else(|e| e.into_inner());
                w(crate::args::SCREEN_CAPTURE_RECORDER_REGISTRY_PATH, &values);
            }
            if let Some(commands) = self.commands(&options) {
                result = self.ffmpeg.run(&commands);
            }
        }
        self.is_recording.store(false, Ordering::SeqCst);
        result
    }

    /// `StopRecording`: `q` to FFmpeg, which finishes the file; the third
    /// call kills it.
    pub fn stop_recording(&self) {
        self.ffmpeg.close();
    }

    /// A handle that stops this recorder from another thread.
    pub fn stopper(&self) -> FFmpegCliManager {
        self.ffmpeg.clone()
    }

    /// `FFmpegEncodeVideo`: the second pass, `input` re-encoded with the
    /// options' codec (custom commands included) into `output`.
    pub fn ffmpeg_encode_video(&self, input: &str, output: &str) -> bool {
        create_directory_from_file_path(output);
        let options = {
            let mut o = self.options.lock().unwrap_or_else(|e| e.into_inner());
            o.is_recording = false;
            o.is_lossless = false;
            o.input_path = Some(input.into());
            o.output_path = Some(output.into());
            o.clone()
        };
        self.ffmpeg.set_track_encode_progress(true);
        // Encoding a file: the input is the file, whatever the platform.
        let result = options.get_ffmpeg_commands(&self.screens).is_some_and(|c| self.ffmpeg.run(&c));
        self.ffmpeg.set_track_encode_progress(false);
        result
    }

    /// `FFmpegEncodeAsGIF`: the second pass of a GIF, through palettegen.
    pub fn ffmpeg_encode_as_gif(&self, input: &str, output: &str) -> bool {
        create_directory_from_file_path(output);
        self.ffmpeg.set_track_encode_progress(true);
        let args = gif_encode_args(input, output, &self.options().ffmpeg);
        let result = self.ffmpeg.run(&args);
        self.ffmpeg.set_track_encode_progress(false);
        result
    }

    /// Replaces the HKCU writes for screen-capture-recorder (tests, the
    /// harness).
    pub fn set_registry_writer(&self, f: impl Fn(&str, &[(&str, i32)]) + Send + Sync + 'static) {
        *self.registry_writer.lock().unwrap_or_else(|e| e.into_inner()) = Box::new(f);
    }
}

/// `OnEncodingProgressChanged`.
fn on_encoding_progress_changed(handler: &Handler<i32>, previous: &AtomicI32, progress: f32) {
    let current = progress as i32;
    let h = handler.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(h) = h.as_ref()
        && current != previous.load(Ordering::SeqCst)
    {
        h(current);
        previous.store(current, Ordering::SeqCst);
    }
}

/// `FileHelpers.CreateDirectoryFromFilePath`.
pub fn create_directory_from_file_path(path: &str) {
    if let Some(dir) = std::path::Path::new(path).parent().filter(|d| !d.as_os_str().is_empty()) {
        let _ = std::fs::create_dir_all(dir);
    }
}

/// `GetFFmpegCommands`' registry writes for ShareX's screen-capture-recorder
/// DirectShow device (`RegistryHelpers.CreateRegistry`, DWORDs under
/// HKCU), which reads its region from there. Windows only.
pub fn write_registry_dwords(path: &str, values: &[(&str, i32)]) {
    #[cfg(windows)]
    registry::set_dwords(path, values);
    #[cfg(not(windows))]
    let _ = (path, values);
}

#[cfg(windows)]
mod registry {
    use windows_sys::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, KEY_WRITE, REG_DWORD, REG_OPTION_NON_VOLATILE, RegCloseKey, RegCreateKeyExW,
        RegSetValueExW,
    };

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    pub fn set_dwords(path: &str, values: &[(&str, i32)]) {
        let mut key: HKEY = std::ptr::null_mut();
        let path = wide(path);
        // SAFETY: valid null-terminated strings and out pointer; the key is closed below.
        let status = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                path.as_ptr(),
                0,
                std::ptr::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_WRITE,
                std::ptr::null(),
                &mut key,
                std::ptr::null_mut(),
            )
        };
        if status != 0 {
            return;
        }
        for (name, value) in values {
            let name = wide(name);
            let data = value.to_le_bytes();
            // SAFETY: `key` is open; `data` is 4 bytes, as REG_DWORD needs.
            unsafe { RegSetValueExW(key, name.as_ptr(), 0, REG_DWORD, data.as_ptr(), 4) };
        }
        // SAFETY: opened above.
        unsafe { RegCloseKey(key) };
    }
}
