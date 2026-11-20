//! ShareX.ScreenRecordingLib, ShareX's native screen recorder (the default,
//! `ScreenRecordUseNative`), ported (docs/SCREEN-RECORDING.md, §2).
//!
//! | ShareX | Here |
//! |---|---|
//! | `RecordingOptions.cs` | [`options`] |
//! | `RecordingClock.cs` | [`clock`] |
//! | `ScreenRecorder.cs` | [`recorder`] (the worker loop, frame pacing, pause/resume/stop/abort) |
//! | `ScreenRecordManager.RecordNativeAsync` (ShareX project) | [`session`] |
//! | `Native/FrameTimer.cs` | [`signals`] |
//! | `Audio/AudioMixer.cs` | [`mixer`] |
//! | `Audio/WasapiCapture.cs` (packets) | [`audio`] (a trait over endpoints); the WASAPI streams are `windows::wasapi` |
//! | `AudioCaptureDevices.cs`, `CameraCaptureDevices.cs` | [`devices`] |
//! | `Video/GraphicsCapture.cs` | [`source`] (a trait, `GetTargets`' rules, synthetic frames, [`ffmpeg_source`]); Windows Graphics Capture is `windows::capture` |
//! | `Video/CameraOverlay.cs` (`GetBounds`), `CameraCapture.cs` | [`camera`] (a trait, `ConfigureFormat`'s scoring); the Media Foundation camera is `windows::camera_capture` |
//! | `Encoding/GifWriter.cs` | [`gif`] (ShareX.HelpersLib's quantizer and LZW are `sharex_tools::gif`) |
//! | `MediaFoundationWriter.BuildAttempts`, `FitSize`, `AlignAndPad`, `GpuVideoProcessor.ComputeDestRect` | [`encoder_attempts`] |
//! | `Video/GraphicsDevice.cs`, `GpuVideoProcessor.cs`, `VideoTexturePool.cs`, `CameraOverlay.cs`, `CameraCircleShader.cs`, `MonitorColorInfo.cs`, `HdrToneMapShader.cs`, `Encoding/MediaFoundationWriter.cs` | `windows` (Windows only) |

pub mod audio;
pub mod camera;
pub mod clock;
pub mod devices;
pub mod encoder_attempts;
pub mod ffmpeg_source;
pub mod gif;
pub mod media;
pub mod mixer;
pub mod options;
pub mod recorder;
pub mod session;
pub mod signals;
pub mod source;
#[cfg(windows)]
pub mod windows;

pub use options::{Rect, RecordingOptions, RecordingResult, VideoEncoderInfo};
pub use recorder::{Backends, RecordError, RecorderHandle, ScreenRecorder};

/// The message of a GIF encode or recording that was aborted.
pub(crate) const ABORTED: &str = "Recording aborted.";
