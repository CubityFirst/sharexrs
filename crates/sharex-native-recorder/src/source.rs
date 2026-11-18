//! The frame source: what `Video/GraphicsCapture.cs` is to ShareX's
//! recorder (the latest frame of every monitor the region touches, copied
//! into the canvas; the end of a closed window; moving to a new region
//! while paused). Windows Graphics Capture waits for a desktop session; a
//! [`FrameSource`] stands in, as [`SyntheticSource`] (generated frames) or
//! FFmpeg's screen grab (`ffmpeg_source`).
//!
//! The pointer: ShareX's native recorder doesn't draw it, WGC does
//! (`IsCursorCaptureEnabled = IncludeCursor`), so a source draws it or not.

use std::sync::Arc;

use crate::clock::TimeSource;
use crate::options::{Rect, RecordingOptions, TICKS_PER_SECOND};

/// The recording's BGRA canvas (`GpuVideoProcessor.Canvas`).
pub trait Canvas {
    fn size(&self) -> (i32, i32);
    /// `Clear`: opaque black.
    fn clear(&mut self);
    /// Copies `height` rows of `width` BGRA pixels, `stride` bytes apart, to (`x`, `y`).
    fn write_bgra(&mut self, x: i32, y: i32, width: i32, height: i32, data: &[u8], stride: usize);
}

/// `GraphicsCapture` as the worker uses it.
pub trait FrameSource: Send {
    /// The recording's size (`GetTargets`' `width`, `height`).
    fn size(&self) -> (i32, i32);
    /// The monitor whose adapter records (`targets[0].Monitor`); 0 = the default adapter.
    fn monitor(&self) -> isize {
        0
    }
    fn start(&mut self) -> Result<(), String>;
    /// `Update`: the newest frame, if any, into the canvas.
    fn update(&mut self, canvas: &mut dyn Canvas) -> Result<(), String>;
    /// Every source has delivered a frame.
    fn has_frame(&self) -> bool;
    /// QPC time (100 ns) of the newest frame.
    fn latest_timestamp(&self) -> i64;
    /// The window closed or a source changed size.
    fn has_ended(&self) -> bool;
    /// `Retarget`: capture `region` (same size) from now on; clears the canvas.
    fn retarget(&mut self, region: Rect, canvas: &mut dyn Canvas) -> Result<(), String>;
}

/// Opens a recording's source (`GraphicsCapture.GetTargets` + the constructor).
pub trait CaptureSources: Send + Sync {
    fn open(&self, options: &RecordingOptions, time: Arc<dyn TimeSource>) -> Result<Box<dyn FrameSource>, String>;
}

/// The `GetTargets` size checks for a region (window capture takes its size from the window).
pub fn region_size(options: &RecordingOptions) -> Result<(i32, i32), String> {
    let (w, h) = options.region_size();
    if w > 16384 || h > 16384 {
        return Err("The capture region exceeds D3D11 texture limits.".into());
    }
    Ok((w, h))
}

/// Draws frame `n` of a region (`region`, `width`, `height`) into BGRA.
pub type Pattern = Arc<dyn Fn(i64, Rect, i32, i32, &mut [u8]) + Send + Sync>;

/// A generated test pattern that moves: frames at `capture_fps` of the
/// [`TimeSource`]'s time, each a function of the frame number and the region.
pub struct SyntheticSource {
    width: i32,
    height: i32,
    region: Rect,
    time: Arc<dyn TimeSource>,
    capture_fps: i64,
    started_at: Option<i64>,
    delivered: i64,
    latest: i64,
    has_frame: bool,
    /// Ends (as a closed window) at this frame.
    pub end_at: Option<i64>,
    /// Frames `n` in `[a, b)` repeat frame `a - 1` (a still screen).
    pub still: Vec<(i64, i64)>,
    pattern: Pattern,
    buffer: Vec<u8>,
}

impl SyntheticSource {
    pub fn new(options: &RecordingOptions, time: Arc<dyn TimeSource>, capture_fps: i64) -> Result<Self, String> {
        let (width, height) = region_size(options)?;
        Ok(Self {
            width,
            height,
            region: options.region,
            time,
            capture_fps,
            started_at: None,
            delivered: 0,
            latest: 0,
            has_frame: false,
            end_at: None,
            still: Vec::new(),
            pattern: Arc::new(test_pattern),
            buffer: vec![0; width as usize * height as usize * 4],
        })
    }

    pub fn with_pattern(mut self, pattern: Pattern) -> Self {
        self.pattern = pattern;
        self
    }

    fn frame_number(&self, n: i64) -> i64 {
        for &(a, b) in &self.still {
            if n >= a && n < b {
                return a - 1;
            }
        }
        n
    }
}

/// The default pattern: a gradient that scrolls, a bouncing box and a
/// counter bar, so consecutive frames differ in a few places.
pub fn test_pattern(n: i64, region: Rect, width: i32, height: i32, out: &mut [u8]) {
    let (w, h) = (width as usize, height as usize);
    let box_size = (w.min(h) / 4).max(2);
    let period_x = (w - box_size).max(1) * 2;
    let period_y = (h - box_size).max(1) * 2;
    let step = n as usize * 3;
    let bx = { let p = step % period_x; if p < period_x / 2 { p } else { period_x - p } };
    let by = { let p = (step * 2 / 3) % period_y; if p < period_y / 2 { p } else { period_y - p } };
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) * 4;
            let gx = (x as i64 + region.x as i64 + n) as u8;
            let gy = (y as i64 + region.y as i64) as u8;
            let in_box = x >= bx && x < bx + box_size && y >= by && y < by + box_size;
            let bar = y >= h.saturating_sub(4) && (x * 64 / w.max(1)) as i64 == n % 64;
            let px = if in_box {
                [40, 180, 250, 255]
            } else if bar {
                [255, 255, 255, 255]
            } else {
                [gx, gy, ((x / 8 + y / 8) % 2 * 60 + 40) as u8, 255]
            };
            out[i..i + 4].copy_from_slice(&px);
        }
    }
}

impl FrameSource for SyntheticSource {
    fn size(&self) -> (i32, i32) {
        (self.width, self.height)
    }
    fn start(&mut self) -> Result<(), String> {
        self.started_at = Some(self.time.now());
        Ok(())
    }
    fn update(&mut self, canvas: &mut dyn Canvas) -> Result<(), String> {
        let Some(start) = self.started_at else { return Ok(()) };
        if self.has_ended() {
            return Ok(());
        }
        let now = self.time.now();
        // Frames arrive one capture interval after the start; the newest one wins.
        let due = (now - start) * self.capture_fps / TICKS_PER_SECOND;
        if due < 1 || due <= self.delivered {
            return Ok(());
        }
        self.delivered = due;
        let n = self.frame_number(due - 1);
        (self.pattern)(n, self.region, self.width, self.height, &mut self.buffer);
        canvas.write_bgra(0, 0, self.width, self.height, &self.buffer, self.width as usize * 4);
        self.latest = start + due * TICKS_PER_SECOND / self.capture_fps;
        self.has_frame = true;
        Ok(())
    }
    fn has_frame(&self) -> bool {
        self.has_frame
    }
    fn latest_timestamp(&self) -> i64 {
        self.latest
    }
    fn has_ended(&self) -> bool {
        self.end_at.is_some_and(|e| self.delivered >= e)
    }
    fn retarget(&mut self, region: Rect, canvas: &mut dyn Canvas) -> Result<(), String> {
        canvas.clear();
        self.region = region;
        self.latest = 0;
        self.has_frame = false;
        self.started_at = Some(self.time.now());
        self.delivered = 0;
        Ok(())
    }
}

/// [`CaptureSources`] that open [`SyntheticSource`]s.
pub struct SyntheticSources {
    pub capture_fps: i64,
}

impl CaptureSources for SyntheticSources {
    fn open(&self, options: &RecordingOptions, time: Arc<dyn TimeSource>) -> Result<Box<dyn FrameSource>, String> {
        Ok(Box::new(SyntheticSource::new(options, time, self.capture_fps)?))
    }
}
