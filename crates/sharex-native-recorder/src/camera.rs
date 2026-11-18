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
    fn open(&self, options: &RecordingOptions) -> Result<Box<dyn CameraSource>, String>;
}

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
        Some(CameraFrame { data, stride })
    }
    fn failure(&self) -> Option<String> {
        self.fail_after
            .filter(|f| self.delivered >= *f)
            .map(|_| "The camera stopped delivering frames or changed its capture format.".to_string())
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
}
