//! The camera overlay's platform-independent parts: `CameraOverlay.GetBounds`
//! and the camera source (`CameraCapture`), which is a trait here: the Media
//! Foundation source reader that opens a real camera waits for a desktop
//! session, and [`SyntheticCamera`] stands in for it.

use crate::options::{CameraCaptureInfo, CameraOverlayPosition, CameraOverlayShape, Rect, RecordingOptions};

/// The camera's pixel layout (`CameraCapture.Subtype`): NV12, YUY2 or RGB32 (BGRA).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraPixelFormat {
    Nv12,
    Yuy2,
    Rgb32,
}

impl CameraPixelFormat {
    pub fn name(self) -> &'static str {
        match self {
            Self::Nv12 => "NV12",
            Self::Yuy2 => "YUY2",
            Self::Rgb32 => "RGB32",
        }
    }
    /// Bytes per row of the luma (or only) plane.
    pub fn row_bytes(self, width: i32) -> i32 {
        width
            * match self {
                Self::Nv12 => 1,
                Self::Yuy2 => 2,
                Self::Rgb32 => 4,
            }
    }
    /// The whole image's bytes, tightly packed.
    pub fn image_bytes(self, width: i32, height: i32) -> i32 {
        self.row_bytes(width) * height * if self == Self::Nv12 { 3 } else { 2 } / 2
    }
}

/// A camera frame in system memory, as a contiguous Media Foundation buffer
/// is read (`UploadCameraBuffer`'s `Lock` path): rows `stride` bytes apart,
/// bottom-up when `stride` is negative.
#[derive(Debug, Clone)]
pub struct CameraFrame {
    pub data: Vec<u8>,
    pub stride: i32,
    /// A real camera's Media Foundation sample (DXGI or system memory),
    /// which the GPU overlay reads as ShareX's `Update(IMFSample)` does;
    /// `data` is then empty.
    #[cfg(windows)]
    pub sample: Option<crate::windows::camera_capture::CameraSample>,
}

impl CameraFrame {
    /// A frame in system memory.
    pub fn memory(data: Vec<u8>, stride: i32) -> Self {
        Self {
            data,
            stride,
            #[cfg(windows)]
            sample: None,
        }
    }
}

/// What a camera negotiated (`CameraCapture`'s `Info`, `Subtype`, `Stride`,
/// `YuvMatrix`, `NominalRange`).
#[derive(Debug, Clone)]
pub struct CameraMode {
    pub info: CameraCaptureInfo,
    pub format: CameraPixelFormat,
    /// `MF_MT_DEFAULT_STRIDE`.
    pub stride: i32,
    /// `MF_MT_YUV_MATRIX` (1 = BT.709, 2 = BT.601).
    pub yuv_matrix: u32,
    /// `MF_MT_VIDEO_NOMINAL_RANGE` (1 = 0-255, 2 = 16-235).
    pub nominal_range: u32,
}

/// `CameraCapture`: asynchronous capture keeping only the latest frame.
pub trait CameraSource: Send {
    fn mode(&self) -> &CameraMode;
    fn start(&mut self) -> Result<(), String>;
    /// `TakeLatestFrame`: the newest frame since the last call.
    fn take_latest_frame(&mut self) -> Option<CameraFrame>;
    /// `Failure`: a stopped or failed camera.
    fn failure(&self) -> Option<String>;
}

/// Opens the camera `options` ask for (`new CameraCapture(graphics, options)`).
pub trait CameraDevices: Send + Sync {
    /// `canvas` is the recording's (the camera decodes on its GPU device when it has one).
    fn open(&self, options: &RecordingOptions, canvas: &mut dyn crate::source::Canvas) -> Result<Box<dyn CameraSource>, String>;
}

/// A camera's native media subtype, as `ConfigureFormat` sorts them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraSubtype {
    Nv12,
    Yuy2,
    Rgb32,
    Mjpg,
    H264,
    Other,
}

impl CameraSubtype {
    /// `IsRaw`: delivered as is, without a decoder.
    pub fn is_raw(self) -> bool {
        matches!(self, Self::Nv12 | Self::Yuy2 | Self::Rgb32)
    }
    pub fn pixel_format(self) -> Option<CameraPixelFormat> {
        match self {
            Self::Nv12 => Some(CameraPixelFormat::Nv12),
            Self::Yuy2 => Some(CameraPixelFormat::Yuy2),
            Self::Rgb32 => Some(CameraPixelFormat::Rgb32),
            _ => None,
        }
    }
}

/// One of the camera's native media types (`GetNativeMediaType`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NativeCameraMode {
    pub subtype: CameraSubtype,
    /// `MF_MT_FRAME_SIZE`: width in the high 32 bits, height in the low.
    pub frame_size: u64,
    /// `MF_MT_FRAME_RATE`: numerator high, denominator low.
    pub frame_rate: u64,
}

impl NativeCameraMode {
    pub fn size(&self) -> (i32, i32) {
        ((self.frame_size >> 32) as i32, self.frame_size as i32)
    }
    /// `(double)(rate >> 32) / Math.Max(1u, (uint)rate)`.
    pub fn fps(&self) -> f64 {
        (self.frame_rate >> 32) as f64 / (self.frame_rate as u32).max(1) as f64
    }
}

/// `ConfigureFormat`'s requested size.
pub fn requested_size(resolution: &crate::options::CameraCaptureResolution) -> (i32, i32) {
    use crate::options::CameraCaptureResolution as R;
    match resolution {
        R::Size640x480 => (640, 480),
        R::Size1920x1080 => (1920, 1080),
        _ => (1280, 720),
    }
}

/// `ConfigureFormat`'s score for a mode (lower is better), `None` for a
/// mode it skips: not raw, MJPEG or H.264; odd, tiny or above 4096.
pub fn mode_score(mode: &NativeCameraMode, requested: (i32, i32), camera_fps: i32) -> Option<f64> {
    if !mode.subtype.is_raw() && mode.subtype != CameraSubtype::Mjpg && mode.subtype != CameraSubtype::H264 {
        return None;
    }
    let (width, height) = mode.size();
    if width < 2 || height < 2 || width > 4096 || height > 4096 || (width & 1) != 0 || (height & 1) != 0 {
        return None;
    }
    let fps = mode.fps();
    let (rw, rh) = (requested.0 as f64, requested.1 as f64);
    let (w, h, cfps) = (width as f64, height as f64, camera_fps as f64);
    let score = ((w * h) / (rw * rh)).ln().abs() * 2.0
        + (w / h - rw / rh).abs()
        + (fps - cfps).abs() / cfps
        + if fps > cfps { 1.0 } else { 0.0 }
        + if mode.subtype.is_raw() { 0.0 } else { 0.25 };
    Some(score)
}

/// The modes `ConfigureFormat` tries, best first (`OrderBy`, stable), as indexes into `modes`.
pub fn ranked_modes(modes: &[NativeCameraMode], requested: (i32, i32), camera_fps: i32) -> Vec<usize> {
    let mut scored: Vec<(usize, f64)> = modes.iter().enumerate().filter_map(|(i, m)| mode_score(m, requested, camera_fps).map(|s| (i, s))).collect();
    scored.sort_by(|a, b| a.1.total_cmp(&b.1));
    scored.into_iter().map(|(i, _)| i).collect()
}

/// The decoder outputs `ConfigureFormat` asks for, in order, for an MJPEG or H.264 mode.
pub const DECODED_FORMATS: [CameraPixelFormat; 3] = [CameraPixelFormat::Nv12, CameraPixelFormat::Yuy2, CameraPixelFormat::Rgb32];

/// `ReadSelectedFormat`'s defaults: the stride (`MF_MT_DEFAULT_STRIDE` when
/// set and non-zero, else `bitmap_stride`), the YUV matrix (BT.709 above 576
/// lines) and the range (limited).
pub fn selected_mode(name: &str, format: CameraPixelFormat, frame_size: u64, frame_rate: u64, default_stride: Option<u32>, bitmap_stride: i32, yuv_matrix: Option<u32>, nominal_range: Option<u32>) -> CameraMode {
    let (width, height) = ((frame_size >> 32) as i32, frame_size as i32);
    let fps = (frame_rate >> 32) as f64 / (frame_rate as u32).max(1) as f64;
    CameraMode {
        info: CameraCaptureInfo { name: name.into(), width, height, frames_per_second: fps, pixel_format: format.name().into() },
        format,
        stride: match default_stride {
            Some(s) if s != 0 => s as i32,
            _ => bitmap_stride,
        },
        yuv_matrix: match yuv_matrix {
            Some(m) if m != 0 => m,
            _ => if height > 576 { 1 } else { 2 },
        },
        nominal_range: nominal_range.unwrap_or(2),
    }
}

/// `CameraCapture`'s messages.
pub const CAMERA_INIT_FAILED: &str = "Windows could not initialize the camera. Check camera permissions, whether another application is using it, and the selected capture mode.";
pub const CAMERA_NO_MODE: &str = "The camera did not provide a supported NV12, YUY2, RGB32, MJPEG, or H.264 capture mode.";
pub const CAMERA_STOPPED: &str = "The camera stopped delivering frames or changed its capture format.";

/// `CameraOverlay.GetBounds`: the tile's place on the recording.
pub fn overlay_bounds(options: &RecordingOptions, info: &CameraCaptureInfo, width: i32, height: i32) -> Rect {
    let margin = options.camera_margin.min(0.max(width.min(height) / 2 - 1));
    let available_width = width - margin * 2;
    let available_height = height - margin * 2;
    let mut tile_width = (width * options.camera_width_percent / 100).clamp(2, available_width.max(2));
    let circular = options.camera_shape == CameraOverlayShape::Circle;
    let mut tile_height = if circular { tile_width } else { 2.max((tile_width as i64 * info.height as i64 / info.width as i64) as i32) };
    if tile_height > available_height {
        tile_height = available_height;
        tile_width = if circular {
            tile_height
        } else {
            ((tile_height as i64 * info.width as i64 / info.height as i64) as i32).clamp(2, available_width.max(2))
        };
    }
    let right = matches!(options.camera_position, CameraOverlayPosition::TopRight | CameraOverlayPosition::BottomRight);
    let bottom = matches!(options.camera_position, CameraOverlayPosition::BottomLeft | CameraOverlayPosition::BottomRight);
    let x = if right { width - margin - tile_width } else { margin };
    let y = if bottom { height - margin - tile_height } else { margin };
    Rect::new(x, y, tile_width, tile_height)
}

/// A camera that delivers generated frames: a moving pattern in the
/// requested format, a new frame every `1/fps` of [`crate::clock::TimeSource`] time.
pub struct SyntheticCamera {
    mode: CameraMode,
    time: std::sync::Arc<dyn crate::clock::TimeSource>,
    started: Option<i64>,
    delivered: i64,
    /// Fails (as a disconnected camera) after this many frames.
    pub fail_after: Option<i64>,
    pub bottom_up: bool,
}

impl SyntheticCamera {
    pub fn new(time: std::sync::Arc<dyn crate::clock::TimeSource>, width: i32, height: i32, fps: f64, format: CameraPixelFormat) -> Self {
        let mode = CameraMode {
            info: CameraCaptureInfo { name: "Synthetic camera".into(), width, height, frames_per_second: fps, pixel_format: format.name().into() },
            format,
            stride: format.row_bytes(width),
            yuv_matrix: if height > 576 { 1 } else { 2 },
            nominal_range: 2,
        };
        Self { mode, time, started: None, delivered: 0, fail_after: None, bottom_up: false }
    }

    /// Frame `n` of the pattern, tightly packed top-down.
    pub fn pattern(format: CameraPixelFormat, width: i32, height: i32, n: i64) -> Vec<u8> {
        let (w, h) = (width as usize, height as usize);
        let mut out = vec![0u8; format.image_bytes(width, height) as usize];
        let shift = (n * 3) as usize;
        match format {
            CameraPixelFormat::Rgb32 => {
                for y in 0..h {
                    for x in 0..w {
                        let i = (y * w + x) * 4;
                        out[i] = ((x + shift) * 255 / w.max(1)) as u8;
                        out[i + 1] = (y * 255 / h.max(1)) as u8;
                        out[i + 2] = (((x ^ y) + shift) & 0xFF) as u8;
                        out[i + 3] = 255;
                    }
                }
            }
            CameraPixelFormat::Yuy2 => {
                for y in 0..h {
                    for x in (0..w).step_by(2) {
                        let i = (y * w + x) * 2;
                        out[i] = (16 + (x + shift) * 219 / w.max(1)) as u8;
                        out[i + 1] = (64 + (y * 128 / h.max(1))) as u8;
                        out[i + 2] = (16 + (x + 1 + shift) * 219 / w.max(1)).min(235) as u8;
                        out[i + 3] = (192 - ((x + shift) * 128 / w.max(1)) % 128) as u8;
                    }
                }
            }
            CameraPixelFormat::Nv12 => {
                for y in 0..h {
                    for x in 0..w {
                        out[y * w + x] = (16 + ((x + y + shift) % 220)) as u8;
                    }
                }
                for y in 0..h / 2 {
                    for x in (0..w).step_by(2) {
                        let i = w * h + y * w + x;
                        out[i] = (64 + (x * 128 / w.max(1))) as u8;
                        out[i + 1] = (192 - (y * 2 * 128 / h.max(1))) as u8;
                    }
                }
            }
        }
        out
    }
}

impl CameraSource for SyntheticCamera {
    fn mode(&self) -> &CameraMode {
        &self.mode
    }
    fn start(&mut self) -> Result<(), String> {
        self.started = Some(self.time.now());
        Ok(())
    }
    fn take_latest_frame(&mut self) -> Option<CameraFrame> {
        let start = self.started?;
        if self.fail_after.is_some_and(|f| self.delivered >= f) {
            return None;
        }
        let due = ((self.time.now() - start) as f64 * self.mode.info.frames_per_second / crate::options::TICKS_PER_SECOND as f64) as i64 + 1;
        if due <= self.delivered {
            return None;
        }
        self.delivered = due;
        let m = &self.mode;
        let mut data = Self::pattern(m.format, m.info.width, m.info.height, due);
        let mut stride = m.stride;
        if self.bottom_up && m.format != CameraPixelFormat::Nv12 {
            let row = m.format.row_bytes(m.info.width) as usize;
            let flipped: Vec<u8> = data.chunks_exact(row).rev().flatten().copied().collect();
            data = flipped;
            stride = -stride;
        }
        Some(CameraFrame::memory(data, stride))
    }
    fn failure(&self) -> Option<String> {
        self.fail_after
            .filter(|f| self.delivered >= *f)
            .map(|_| CAMERA_STOPPED.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(w: i32, h: i32) -> CameraCaptureInfo {
        CameraCaptureInfo { name: String::new(), width: w, height: h, frames_per_second: 30.0, pixel_format: "NV12".into() }
    }

    #[test]
    fn bounds_as_sharex() {
        let mut o = RecordingOptions::new("a.mp4");
        // 20% of 1920 = 384 wide, 16:9 = 216 high, bottom right, 16 px margin.
        assert_eq!(overlay_bounds(&o, &info(1280, 720), 1920, 1080), Rect::new(1920 - 16 - 384, 1080 - 16 - 216, 384, 216));
        o.camera_shape = CameraOverlayShape::Circle;
        o.camera_position = CameraOverlayPosition::TopLeft;
        assert_eq!(overlay_bounds(&o, &info(1280, 720), 1920, 1080), Rect::new(16, 16, 384, 384));
        // A short recording: the tile fits the height.
        o.camera_width_percent = 50;
        assert_eq!(overlay_bounds(&o, &info(1280, 720), 400, 100), Rect::new(16, 16, 68, 68));
        o.camera_shape = CameraOverlayShape::Rectangle;
        assert_eq!(overlay_bounds(&o, &info(1280, 720), 400, 100), Rect::new(16, 16, 120, 68));
        // The margin shrinks to fit a tiny recording.
        o.camera_margin = 1000;
        assert_eq!(overlay_bounds(&o, &info(640, 480), 10, 10).x, 4);
    }

    fn mode(subtype: CameraSubtype, w: u64, h: u64, num: u64, den: u64) -> NativeCameraMode {
        NativeCameraMode { subtype, frame_size: (w << 32) | h, frame_rate: (num << 32) | den }
    }

    #[test]
    fn mode_scores_as_sharex() {
        use crate::options::CameraCaptureResolution as R;
        assert_eq!(requested_size(&R::Size640x480), (640, 480));
        assert_eq!(requested_size(&R::Size1920x1080), (1920, 1080));
        assert_eq!(requested_size(&R::Unknown("x".into())), (1280, 720));
        let req = (1280, 720);
        // The exact mode at the exact rate scores 0; MJPEG costs 0.25 more.
        assert_eq!(mode_score(&mode(CameraSubtype::Nv12, 1280, 720, 30, 1), req, 30), Some(0.0));
        assert_eq!(mode_score(&mode(CameraSubtype::Mjpg, 1280, 720, 30, 1), req, 30), Some(0.25));
        // Faster than asked: the difference plus 1.
        assert_eq!(mode_score(&mode(CameraSubtype::Yuy2, 1280, 720, 60, 1), req, 30), Some(2.0));
        // 30000/1001.
        let s = mode_score(&mode(CameraSubtype::Nv12, 1280, 720, 30000, 1001), req, 30).unwrap();
        assert_eq!(s, ((30.0 - 30000.0 / 1001.0) as f64).abs() / 30.0);
        // A zero denominator counts as 1.
        assert_eq!(mode(CameraSubtype::Nv12, 2, 2, 15, 0).fps(), 15.0);
        // 640x480 for 720p: log of the area ratio twice plus the aspect difference plus the rate.
        let s = mode_score(&mode(CameraSubtype::Rgb32, 640, 480, 15, 1), req, 30).unwrap();
        assert_eq!(s, ((640.0 * 480.0) / (1280.0 * 720.0) as f64).ln().abs() * 2.0 + (640.0 / 480.0 - 1280.0 / 720.0 as f64).abs() + 0.5);
        // Skipped: odd, tiny, too large, other subtypes.
        assert_eq!(mode_score(&mode(CameraSubtype::Nv12, 1281, 720, 30, 1), req, 30), None);
        assert_eq!(mode_score(&mode(CameraSubtype::Nv12, 0, 720, 30, 1), req, 30), None);
        assert_eq!(mode_score(&mode(CameraSubtype::Nv12, 4098, 720, 30, 1), req, 30), None);
        assert_eq!(mode_score(&mode(CameraSubtype::Other, 1280, 720, 30, 1), req, 30), None);
    }

    #[test]
    fn modes_ranked_stably() {
        let modes = [
            mode(CameraSubtype::Mjpg, 1920, 1080, 30, 1),
            mode(CameraSubtype::Yuy2, 1280, 720, 10, 1),
            mode(CameraSubtype::Mjpg, 1280, 720, 30, 1),
            mode(CameraSubtype::Other, 1280, 720, 30, 1),
            mode(CameraSubtype::Nv12, 1280, 720, 30, 1),
            mode(CameraSubtype::Nv12, 1280, 720, 30, 1),
        ];
        assert_eq!(ranked_modes(&modes, (1280, 720), 30), vec![4, 5, 2, 1, 0]);
    }

    #[test]
    fn selected_mode_defaults() {
        let m = selected_mode("Cam", CameraPixelFormat::Yuy2, (640 << 32) | 480, (30 << 32) | 1, None, 1280, None, None);
        assert_eq!((m.info.width, m.info.height, m.info.frames_per_second, m.stride, m.yuv_matrix, m.nominal_range), (640, 480, 30.0, 1280, 2, 2));
        assert_eq!(m.info.pixel_format, "YUY2");
        let m = selected_mode("Cam", CameraPixelFormat::Rgb32, (1280 << 32) | 720, (30 << 32) | 1, Some(0), -5120, Some(0), Some(1));
        assert_eq!((m.stride, m.yuv_matrix, m.nominal_range), (-5120, 1, 1));
        let m = selected_mode("Cam", CameraPixelFormat::Nv12, (1280 << 32) | 720, (30 << 32) | 1, Some(1280), 0, Some(2), Some(2));
        assert_eq!((m.stride, m.yuv_matrix), (1280, 2));
    }
}
