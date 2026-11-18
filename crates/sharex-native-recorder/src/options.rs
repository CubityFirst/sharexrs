//! `RecordingOptions.cs`: what a native recording records, its checks
//! (`Validate`), and what it reports (`VideoEncoderInfo`, `RecordingResult`).

use std::path::{Path, PathBuf};

pub use sharex_settings::{CameraCaptureResolution, CameraOverlayPosition, CameraOverlayShape};

/// .NET's `TimeSpan.TicksPerSecond`: the recorder's times are in 100 ns.
pub const TICKS_PER_SECOND: i64 = 10_000_000;
pub const TICKS_PER_MILLISECOND: i64 = 10_000;

/// `System.Drawing.Rectangle`, in physical desktop pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self { x, y, width, height }
    }
    pub fn right(&self) -> i32 {
        self.x + self.width
    }
    pub fn bottom(&self) -> i32 {
        self.y + self.height
    }
    pub fn is_empty(&self) -> bool {
        self.width == 0 && self.height == 0 && self.x == 0 && self.y == 0
    }
    /// `Rectangle.FromLTRB`.
    pub fn from_ltrb(l: i32, t: i32, r: i32, b: i32) -> Self {
        Self::new(l, t, r - l, b - t)
    }
    /// `Rectangle.Intersect`: `Empty` when they don't overlap.
    pub fn intersect(a: Rect, b: Rect) -> Rect {
        let x1 = a.x.max(b.x);
        let x2 = a.right().min(b.right());
        let y1 = a.y.max(b.y);
        let y2 = a.bottom().min(b.bottom());
        if x2 >= x1 && y2 >= y1 { Rect::from_ltrb(x1, y1, x2, y2) } else { Rect::default() }
    }
}

/// `RecordingOptions`: native H.264/AAC MP4 or adaptive GIF recording.
#[derive(Debug, Clone, PartialEq)]
pub struct RecordingOptions {
    /// The .mp4 or .gif extension selects the output format.
    pub output_path: PathBuf,
    pub region: Rect,
    /// When nonzero, capture this window (including when occluded) instead of `region`.
    pub window_handle: isize,
    pub frames_per_second: i32,
    pub video_bitrate: i32,
    pub audio_bitrate: i32,
    pub include_cursor: bool,
    /// On HDR displays, capture HDR frames and tone map them.
    pub hdr_tone_mapping: bool,
    pub capture_system_audio: bool,
    pub capture_microphone: bool,
    /// WASAPI endpoint ID; `None` selects the default multimedia render endpoint.
    pub system_audio_device_id: Option<String>,
    /// WASAPI endpoint ID; `None` selects the default communications capture endpoint.
    pub microphone_device_id: Option<String>,
    pub capture_camera: bool,
    /// Camera symbolic link; `None` selects the first available camera.
    pub camera_device_id: Option<String>,
    pub camera_resolution: CameraCaptureResolution,
    pub camera_frames_per_second: i32,
    pub camera_position: CameraOverlayPosition,
    /// Circles use a centred square crop; rectangles keep the whole camera image.
    pub camera_shape: CameraOverlayShape,
    /// Overlay width as a percentage of the recording, fitted to the shape.
    pub camera_width_percent: i32,
    /// Distance from the recording's corner in physical pixels, clamped to fit.
    pub camera_margin: i32,
    pub system_audio_gain: f32,
    pub microphone_gain: f32,
    /// For MP4, reject a software video encoder.
    pub require_hardware_encoder: bool,
    /// `TimeSpan` ticks; 0 = until stopped.
    pub duration: i64,
}

impl RecordingOptions {
    /// The defaults of `RecordingOptions`' initialisers.
    pub fn new(output_path: impl Into<PathBuf>) -> Self {
        Self {
            output_path: output_path.into(),
            region: Rect::default(),
            window_handle: 0,
            frames_per_second: 30,
            video_bitrate: 8_000_000,
            audio_bitrate: 192_000,
            include_cursor: true,
            hdr_tone_mapping: false,
            capture_system_audio: true,
            capture_microphone: false,
            system_audio_device_id: None,
            microphone_device_id: None,
            capture_camera: false,
            camera_device_id: None,
            camera_resolution: CameraCaptureResolution::Size1280x720,
            camera_frames_per_second: 30,
            camera_position: CameraOverlayPosition::BottomRight,
            camera_shape: CameraOverlayShape::Rectangle,
            camera_width_percent: 20,
            camera_margin: 16,
            system_audio_gain: 1.0,
            microphone_gain: 1.0,
            require_hardware_encoder: true,
            duration: 0,
        }
    }

    /// `IsGif`: `Path.GetExtension(OutputPath)` is `.gif` (ordinal, any case).
    pub fn is_gif(&self) -> bool {
        dotnet_extension(&self.output_path).eq_ignore_ascii_case(".gif")
    }

    /// `HasAudio`.
    pub fn has_audio(&self) -> bool {
        !self.is_gif() && (self.capture_system_audio || self.capture_microphone)
    }

    /// `Validate`, with ShareX's messages.
    pub fn validate(&self) -> Result<(), String> {
        let path = self.output_path.to_string_lossy();
        if path.trim().is_empty() {
            return Err("The value cannot be an empty string or composed entirely of whitespace. (Parameter 'OutputPath')".into());
        }
        let gif = self.is_gif();
        if !gif && !dotnet_extension(&self.output_path).eq_ignore_ascii_case(".mp4") {
            return Err("Native recording writes H.264/AAC MP4 or adaptive GIF files. (Parameter 'OutputPath')".into());
        }
        if self.window_handle == 0 && (self.region.width < 2 || self.region.height < 2) {
            return Err("Select a window or a region of at least 2 × 2 pixels. (Parameter 'Region')".into());
        }
        let out_of_range = |name: &str| Err(format!("Specified argument was out of the range of valid values. (Parameter '{name}')"));
        if !(1..=240).contains(&self.frames_per_second) {
            return out_of_range("FramesPerSecond");
        }
        if gif && self.frames_per_second > 100 {
            return Err("GIF frames have a minimum duration of 10 ms. (Parameter 'FramesPerSecond')".into());
        }
        if !gif && !(100_000..=200_000_000).contains(&self.video_bitrate) {
            return out_of_range("VideoBitrate");
        }
        if !gif && ![96_000, 128_000, 160_000, 192_000].contains(&self.audio_bitrate) {
            return out_of_range("AudioBitrate");
        }
        if self.duration < 0 {
            return out_of_range("Duration");
        }
        if self.camera_resolution.value().is_none() {
            return out_of_range("CameraResolution");
        }
        if self.camera_position.value().is_none() {
            return out_of_range("CameraPosition");
        }
        if self.camera_shape.value().is_none() {
            return out_of_range("CameraShape");
        }
        if !(1..=60).contains(&self.camera_frames_per_second) {
            return out_of_range("CameraFramesPerSecond");
        }
        if !(5..=50).contains(&self.camera_width_percent) {
            return out_of_range("CameraWidthPercent");
        }
        if !(0..=1000).contains(&self.camera_margin) {
            return out_of_range("CameraMargin");
        }
        if !self.system_audio_gain.is_finite() || !(0.0..=4.0).contains(&self.system_audio_gain) {
            return out_of_range("SystemAudioGain");
        }
        if !self.microphone_gain.is_finite() || !(0.0..=4.0).contains(&self.microphone_gain) {
            return out_of_range("MicrophoneGain");
        }
        Ok(())
    }

    /// The width and height the recording has: `GraphicsCapture.GetTargets`
    /// for a region (MP4 needs even dimensions).
    pub fn region_size(&self) -> (i32, i32) {
        if self.is_gif() { (self.region.width, self.region.height) } else { (self.region.width & !1, self.region.height & !1) }
    }
}

/// `Path.GetExtension`: from the last `.` of the file name (`""` when it
/// ends the name or there's none).
pub fn dotnet_extension(path: &Path) -> String {
    let s = path.to_string_lossy();
    let name_start = s.rfind(['\\', '/', ':']).map_or(0, |i| i + 1);
    let name = &s[name_start..];
    match name.rfind('.') {
        Some(i) if i + 1 < name.len() => name[i..].to_string(),
        _ => String::new(),
    }
}

/// `VideoEncoderInfo`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoEncoderInfo {
    pub name: String,
    pub is_hardware_accelerated: bool,
    pub is_d3d11_aware: bool,
}

impl VideoEncoderInfo {
    /// `GifWriter.Encoder`.
    pub fn gif() -> Self {
        Self { name: "ShareX adaptive GIF encoder".into(), is_hardware_accelerated: false, is_d3d11_aware: false }
    }
}

/// `RecordingResult`.
#[derive(Debug, Clone, PartialEq)]
pub struct RecordingResult {
    pub output_path: PathBuf,
    /// Ticks.
    pub duration: i64,
    pub video_frames: i64,
    pub dropped_video_frames: i64,
    pub audio_discontinuities: i64,
    pub encoder: VideoEncoderInfo,
}

/// `CameraCaptureInfo`: the camera's negotiated mode.
#[derive(Debug, Clone, PartialEq)]
pub struct CameraCaptureInfo {
    pub name: String,
    pub width: i32,
    pub height: i32,
    pub frames_per_second: f64,
    /// "NV12", "YUY2" or "RGB32".
    pub pixel_format: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(path: &str) -> RecordingOptions {
        let mut o = RecordingOptions::new(path);
        o.region = Rect::new(0, 0, 100, 100);
        o
    }

    #[test]
    fn extension_as_dotnet() {
        assert_eq!(dotnet_extension(Path::new("a/b.GIF")), ".GIF");
        assert_eq!(dotnet_extension(Path::new("a/.gif")), ".gif");
        assert_eq!(dotnet_extension(Path::new("a/b.")), "");
        assert_eq!(dotnet_extension(Path::new("a.b/c")), "");
        assert!(opts("x.Gif").is_gif());
        assert!(!opts("x.mp4").has_audio() || opts("x.mp4").capture_system_audio);
        assert!(!opts("x.gif").has_audio());
    }

    #[test]
    fn validate_as_sharex() {
        assert!(opts("x.mp4").validate().is_ok());
        assert!(opts("x.gif").validate().is_ok());
        assert!(opts("x.mkv").validate().unwrap_err().contains("H.264/AAC MP4"));
        let mut o = opts("x.gif");
        o.frames_per_second = 101;
        assert!(o.validate().unwrap_err().contains("10 ms"));
        let mut o = opts("x.mp4");
        o.audio_bitrate = 100_000;
        assert!(o.validate().unwrap_err().contains("AudioBitrate"));
        let mut o = opts("x.gif");
        o.audio_bitrate = 1;
        o.video_bitrate = 1;
        assert!(o.validate().is_ok());
        let mut o = opts("x.mp4");
        o.region = Rect::new(0, 0, 1, 100);
        assert!(o.validate().unwrap_err().contains("2 × 2"));
        o.window_handle = 5;
        assert!(o.validate().is_ok());
        let mut o = opts("x.mp4");
        o.system_audio_gain = f32::NAN;
        assert!(o.validate().is_err());
        let mut o = opts("x.mp4");
        o.camera_shape = CameraOverlayShape::Unknown("Hex".into());
        assert!(o.validate().unwrap_err().contains("CameraShape"));
    }

    #[test]
    fn intersect() {
        assert_eq!(Rect::intersect(Rect::new(0, 0, 10, 10), Rect::new(5, 5, 10, 10)), Rect::new(5, 5, 5, 5));
        assert_eq!(Rect::intersect(Rect::new(0, 0, 10, 10), Rect::new(20, 5, 10, 10)), Rect::default());
    }
}
