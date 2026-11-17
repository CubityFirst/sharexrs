//! ShareX's FFmpeg screen recorder (`ShareX.ScreenCaptureLib/ScreenRecording`,
//! the FFmpeg branch of `ShareX/ScreenRecordManager.cs`,
//! `ShareX.HelpersLib/Media/FFmpegCLIManager.cs`). The map of how ShareX
//! records, natively and with FFmpeg, is `docs/SCREEN-RECORDING.md`.
//!
//! - [`args`]: the command lines, exactly as ShareX builds them
//!   (`tools/recording-harness` diffs them against ShareX's DLLs),
//! - [`cli`]: running FFmpeg, its output, `q` to stop,
//! - [`recorder`]: one recording segment and the second pass,
//! - [`session`]: a recording from start to file (pause by segments,
//!   restart, abort, two-pass), and the recording frame's state,
//! - [`discovery`]: where FFmpeg is,
//! - [`devices`]: DirectShow (ShareX), AVFoundation and PulseAudio devices,
//! - [`platform`]: the inputs on macOS and Linux (ShareXrs additions).
//!
//! The options model is `sharex_settings::FFmpegOptions` (with the task's
//! `ScreenRecord*` settings in `TaskSettingsCapture`).

pub mod args;
pub mod cli;
pub mod devices;
pub mod discovery;
pub mod dotnet;
pub mod platform;
pub mod recorder;
pub mod session;

pub use args::{DesktopScreen, ScreenRecordingOptions};
pub use cli::FFmpegCliManager;
pub use recorder::{RecorderInput, ScreenRecorder};
pub use session::{
    FFmpegSession, PreparedRecording, RecordingControl, ScreenRecordOutput, ScreenRecordStartMethod, ScreenRecordState,
    ScreenRecordingStatus, SessionHooks, StartError, prepare_ffmpeg_recording, run_ffmpeg_session,
};
pub use sharex_settings::{FFmpegCaptureDevice, FFmpegOptions};
