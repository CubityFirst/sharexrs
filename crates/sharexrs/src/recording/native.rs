//! ShareX's native recorder in the app (`ScreenRecordManager`'s
//! `useNative` path, `sharex_native_recorder::session`), on Windows.
//!
//! Its encoders are ported (Media Foundation H.264/HEVC + AAC, the D3D11
//! video processor, the adaptive GIF writer); its capture isn't yet, so:
//! - FFmpeg grabs the screen (gdigrab, drawing the pointer when
//!   `ScreenRecordShowCursor`) and hands it raw frames, where ShareX uses
//!   Windows Graphics Capture; an active window is recorded as its rectangle;
//! - system audio and the microphone (WASAPI) and the camera overlay's
//!   camera are left out, with a log line.

use std::sync::Arc;

use sharex_native_recorder::session::{NativeSession, run_native_session};
use sharex_screen_recording::session::{PreparedRecording, RecordingControl, SessionHooks};

/// The session for a prepared recording.
pub fn session(prepared: PreparedRecording, is_gif: bool) -> NativeSession {
    let c = &prepared.capture;
    if !is_gif && (c.ScreenRecordSystemAudio || c.ScreenRecordMicrophone) {
        eprintln!("Native recorder: system audio and microphone capture (WASAPI) aren't ported yet: recording without sound.");
    }
    if c.ScreenRecordCamera {
        eprintln!("Native recorder: camera capture isn't ported yet: recording without the camera overlay.");
    }
    if c.ScreenRecordHDRToneMapping {
        eprintln!("Native recorder: HDR tone mapping needs Windows Graphics Capture, which isn't ported yet.");
    }
    NativeSession {
        capture: prepared.capture.clone(),
        is_gif,
        // FFmpeg's grab records the window's rectangle, not the window (WGC's `CreateForWindow`).
        window_handle: 0,
        backends: backends(prepared.ffmpeg_path),
        adjust: Some(Box::new(|o| {
            o.capture_system_audio = false;
            o.capture_microphone = false;
            o.capture_camera = false;
            o.hdr_tone_mapping = false;
        })),
        log: Box::new(|m| eprintln!("{m}")),
    }
}

#[cfg(windows)]
fn backends(ffmpeg: std::path::PathBuf) -> sharex_native_recorder::Backends {
    use sharex_native_recorder::ffmpeg_source::FfmpegSources;
    sharex_native_recorder::Backends::system(Arc::new(FfmpegSources::gdigrab(ffmpeg)))
}

#[cfg(not(windows))]
fn backends(_ffmpeg: std::path::PathBuf) -> sharex_native_recorder::Backends {
    // `uses_native` is false off Windows: the FFmpeg recorder records there.
    unreachable!("the native recorder's encoders are Windows only")
}

/// Runs the session on the worker thread.
pub fn run(session: NativeSession, control: Arc<RecordingControl>, hooks: &mut dyn SessionHooks) -> Option<String> {
    run_native_session(session, control, hooks)
}
