//! What the worker hands frames and sound to: on Windows the GPU canvas,
//! video processor, texture pool and Media Foundation writer or GIF writer
//! (`windows::GpuMedia`), as `ScreenRecorder.Run` builds them; elsewhere
//! and in tests a CPU canvas ([`CpuMedia`]) feeding the same GIF writer.

use crate::camera::{CameraFrame, CameraMode};
use crate::gif::GifWriter;
use crate::options::{RecordingOptions, VideoEncoderInfo};
use crate::source::Canvas;

/// The encoders and canvas of one recording.
pub trait Media: Send {
    fn canvas(&mut self) -> &mut dyn Canvas;
    /// `Encoder`.
    fn encoder(&self) -> VideoEncoderInfo;
    /// `writer.FramesPerSecond`: the rate the encoder took, when it isn't
    /// the one asked for (`None`: as asked).
    fn frames_per_second(&self) -> Option<i32> {
        None
    }
    /// `writer.AdjustmentNote`: what the encoder negotiated, when it isn't
    /// what was asked for.
    fn adjustment_note(&self) -> Option<String> {
        None
    }
    /// The camera overlay (`new CameraOverlay(...)`).
    fn create_camera_overlay(&mut self, mode: &CameraMode, options: &RecordingOptions) -> Result<(), String>;
    /// `cameraOverlay.Update`.
    fn camera_update(&mut self, frame: &CameraFrame) -> Result<(), String>;
    /// `cameraOverlay.Disable`.
    fn camera_disable(&mut self);
    /// `textures.TryRent`: an NV12 texture for frame `frame` (always, for a GIF).
    /// `false` = the encoder is behind: the slot is dropped as a discontinuity.
    fn try_rent(&mut self, frame: i64) -> bool;
    /// One frame: the canvas (with the camera) to the encoder (the rented
    /// texture). `false` = the GIF cache is behind: dropped, the next slot tried at once.
    fn write_video(&mut self, frame: i64, timestamp: i64, end: i64, discontinuity: bool) -> Result<bool, String>;
    /// `writer.WriteAudio` (MP4 with sound only).
    fn write_audio(&mut self, pcm: &[i16], frame_offset: i64) -> Result<(), String>;
    /// `writer.Finish` (MP4).
    fn finish_video(&mut self) -> Result<(), String>;
    /// `gifWriter.Finish` (GIF).
    fn finish_gif(&mut self, duration: i64, progress: &mut dyn FnMut(i32), aborted: &dyn Fn() -> bool) -> Result<(), String>;
    fn is_gif(&self) -> bool;
}

/// Builds a recording's [`Media`] (`MediaFoundationWriter.Create`,
/// `GpuVideoProcessor`, `GifWriter`, `VideoTexturePool`).
pub trait MediaFactory: Send + Sync {
    fn create(&self, options: &RecordingOptions, width: i32, height: i32, monitor: isize) -> Result<Box<dyn Media>, String>;
}

/// A BGRA canvas in memory.
pub struct CpuCanvas {
    pub width: i32,
    pub height: i32,
    pub pixels: Vec<u8>,
}

impl CpuCanvas {
    pub fn new(width: i32, height: i32) -> Self {
        let mut c = Self { width, height, pixels: vec![0; width as usize * height as usize * 4] };
        c.clear();
        c
    }
}

impl Canvas for CpuCanvas {
    fn size(&self) -> (i32, i32) {
        (self.width, self.height)
    }
    fn clear(&mut self) {
        for p in self.pixels.chunks_exact_mut(4) {
            p.copy_from_slice(&[0, 0, 0, 255]);
        }
    }
    fn write_bgra(&mut self, x: i32, y: i32, width: i32, height: i32, data: &[u8], stride: usize) {
        let w = self.width as usize;
        for row in 0..height as usize {
            let d = ((y as usize + row) * w + x as usize) * 4;
            self.pixels[d..d + width as usize * 4].copy_from_slice(&data[row * stride..row * stride + width as usize * 4]);
        }
    }
}

/// What [`CpuMedia`] was given for an MP4 (there's no encoder off Windows).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mp4Call {
    Video { frame: i64, timestamp: i64, end: i64, discontinuity: bool },
    Audio { frame_offset: i64, frames: usize },
    Finish,
}

/// The CPU media: GIFs through [`GifWriter`], MP4s recorded as calls.
pub struct CpuMedia {
    canvas: CpuCanvas,
    gif: Option<GifWriter>,
    pub mp4_calls: std::sync::Arc<std::sync::Mutex<Vec<Mp4Call>>>,
    /// Slots that run out (an encoder that's behind), by frame number.
    pub no_slot: Vec<i64>,
    /// A frame rate the "encoder" negotiated, as a Media Foundation fallback does.
    pub negotiated_fps: Option<i32>,
}

impl Media for CpuMedia {
    fn canvas(&mut self) -> &mut dyn Canvas {
        &mut self.canvas
    }
    fn frames_per_second(&self) -> Option<i32> {
        self.negotiated_fps
    }
    fn adjustment_note(&self) -> Option<String> {
        self.negotiated_fps.map(|fps| format!("Encoder negotiated H.264 at {fps} FPS"))
    }
    fn encoder(&self) -> VideoEncoderInfo {
        if self.gif.is_some() {
            VideoEncoderInfo::gif()
        } else {
            VideoEncoderInfo { name: "Recorded calls".into(), is_hardware_accelerated: false, is_d3d11_aware: false }
        }
    }
    fn create_camera_overlay(&mut self, _mode: &CameraMode, _options: &RecordingOptions) -> Result<(), String> {
        Err("The camera overlay needs the GPU.".into())
    }
    fn camera_update(&mut self, _frame: &CameraFrame) -> Result<(), String> {
        Ok(())
    }
    fn camera_disable(&mut self) {}
    fn try_rent(&mut self, frame: i64) -> bool {
        self.gif.is_some() || !self.no_slot.contains(&frame)
    }
    fn write_video(&mut self, frame: i64, timestamp: i64, end: i64, discontinuity: bool) -> Result<bool, String> {
        if let Some(g) = &mut self.gif {
            return g.try_write_video(&self.canvas.pixels, timestamp);
        }
        self.mp4_calls.lock().unwrap_or_else(|e| e.into_inner()).push(Mp4Call::Video { frame, timestamp, end, discontinuity });
        Ok(true)
    }
    fn write_audio(&mut self, pcm: &[i16], frame_offset: i64) -> Result<(), String> {
        self.mp4_calls.lock().unwrap_or_else(|e| e.into_inner()).push(Mp4Call::Audio { frame_offset, frames: pcm.len() / 2 });
        Ok(())
    }
    fn finish_video(&mut self) -> Result<(), String> {
        if self.gif.is_none() {
            self.mp4_calls.lock().unwrap_or_else(|e| e.into_inner()).push(Mp4Call::Finish);
        }
        Ok(())
    }
    fn finish_gif(&mut self, duration: i64, progress: &mut dyn FnMut(i32), aborted: &dyn Fn() -> bool) -> Result<(), String> {
        match &mut self.gif {
            Some(g) => g.finish(duration, progress, aborted),
            None => Ok(()),
        }
    }
    fn is_gif(&self) -> bool {
        self.gif.is_some()
    }
}

/// Makes [`CpuMedia`].
#[derive(Default)]
pub struct CpuMediaFactory {
    pub mp4_calls: std::sync::Arc<std::sync::Mutex<Vec<Mp4Call>>>,
    pub no_slot: Vec<i64>,
    pub negotiated_fps: Option<i32>,
}

impl MediaFactory for CpuMediaFactory {
    fn create(&self, options: &RecordingOptions, width: i32, height: i32, _monitor: isize) -> Result<Box<dyn Media>, String> {
        let gif = if options.is_gif() { Some(GifWriter::new(options, width as u32, height as u32)?) } else { None };
        Ok(Box::new(CpuMedia { canvas: CpuCanvas::new(width, height), gif, mp4_calls: self.mp4_calls.clone(), no_slot: self.no_slot.clone(), negotiated_fps: self.negotiated_fps }))
    }
}
