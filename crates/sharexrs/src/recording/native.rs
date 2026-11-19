//! ShareX's native recorder in the app (`ScreenRecordManager`'s
//! `useNative` path, `sharex_native_recorder::session`), on Windows:
//! Windows Graphics Capture (the region's monitors, or the active window
//! itself), WASAPI system audio and microphone, the Media Foundation
//! camera, encoded by Media Foundation (H.264/HEVC + AAC) or the adaptive
//! GIF writer.

use std::sync::Arc;

use sharex_native_recorder::session::{NativeSession, run_native_session};
use sharex_screen_recording::session::{PreparedRecording, RecordingControl, SessionHooks};

/// The session for a prepared recording; `window` is the captured window
/// (active window recordings without client area), else 0.
pub fn session(prepared: PreparedRecording, is_gif: bool, window: isize) -> NativeSession {
    NativeSession {
        capture: prepared.capture.clone(),
        is_gif,
        window_handle: window,
        backends: backends(),
        adjust: None,
        log: Box::new(|m| eprintln!("{m}")),
    }
}

#[cfg(windows)]
fn backends() -> sharex_native_recorder::Backends {
    use sharex_native_recorder::windows::camera_capture::MfCameraDevices;
    use sharex_native_recorder::windows::capture::GraphicsCaptureSources;
    use sharex_native_recorder::windows::wasapi::WasapiDevices;
    let mut b = sharex_native_recorder::Backends::system(Arc::new(GraphicsCaptureSources));
    b.audio = Some(Arc::new(WasapiDevices));
    b.camera = Some(Arc::new(MfCameraDevices));
    b
}

#[cfg(not(windows))]
fn backends() -> sharex_native_recorder::Backends {
    // `uses_native` is false off Windows: the FFmpeg recorder records there.
    unreachable!("the native recorder is Windows only")
}

/// Runs the session on the worker thread.
pub fn run(session: NativeSession, control: Arc<RecordingControl>, hooks: &mut dyn SessionHooks) -> Option<String> {
    run_native_session(session, control, hooks)
}
