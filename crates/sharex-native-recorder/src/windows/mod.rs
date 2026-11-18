//! The Windows encoders and GPU path of `ScreenRecorder.Run`: a D3D11
//! device on the recorded monitor's adapter, the BGRA canvas and its NV12
//! conversion, the texture pool, the Media Foundation writer or the GIF
//! writer (fed from a staging copy), and the camera overlay.

pub mod camera;
pub mod device;
pub mod mf;
pub mod processor;

use windows::Win32::Graphics::Direct3D11::ID3D11Texture2D;
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Media::MediaFoundation::{MF_VERSION, MFSTARTUP_LITE, MFShutdown, MFStartup};
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};

use self::camera::CameraOverlay;
use self::device::GraphicsDevice;
use self::mf::{MediaFoundationWriter, VideoTexturePool};
use self::processor::GpuVideoProcessor;
use crate::camera::{CameraFrame, CameraMode};
use crate::gif::GifWriter;
use crate::media::{Media, MediaFactory};
use crate::options::{RecordingOptions, VideoEncoderInfo};
use crate::source::Canvas;

/// A Windows error as ShareX's exceptions show it.
pub(crate) fn err(e: windows::core::Error) -> String {
    let m = e.message();
    if m.is_empty() { format!("HRESULT 0x{:08X}", e.code().0) } else { format!("{} (0x{:08X})", m.trim_end(), e.code().0) }
}

/// COM (MTA) and, when asked, Media Foundation, for the current thread until dropped.
pub struct ComSession {
    com: bool,
    mf: bool,
}

impl ComSession {
    pub fn start(media_foundation: bool) -> Result<Self, String> {
        // SAFETY: initialisation calls, undone in `drop` on the same thread.
        unsafe {
            let com = CoInitializeEx(None, COINIT_MULTITHREADED).is_ok();
            let mut s = Self { com, mf: false };
            if media_foundation {
                MFStartup(MF_VERSION, MFSTARTUP_LITE).map_err(err)?;
                s.mf = true;
            }
            Ok(s)
        }
    }
}

impl Drop for ComSession {
    fn drop(&mut self) {
        // SAFETY: pairs with the successful calls in `start`.
        unsafe {
            if self.mf {
                let _ = MFShutdown();
            }
            if self.com {
                CoUninitialize();
            }
        }
    }
}

/// The GPU media of a Windows recording.
pub struct GpuMedia {
    camera: Option<CameraOverlay>,
    textures: Option<VideoTexturePool>,
    rented: Option<usize>,
    gif: Option<GifWriter>,
    staging: Option<ID3D11Texture2D>,
    frame: Vec<u8>,
    processor: GpuVideoProcessor,
    writer: Option<MediaFoundationWriter>,
    encoder: VideoEncoderInfo,
    /// The NV12 of the first frames written, while there's room (the harness).
    pub nv12_dump: Option<(usize, Vec<Vec<u8>>)>,
    graphics: GraphicsDevice,
    _session: ComSession,
}

impl GpuMedia {
    /// `Run`'s set-up: device, writer (`MediaFoundationWriter.Create`), processor, GIF writer, texture pool.
    pub fn new(options: &RecordingOptions, width: i32, height: i32, monitor: isize) -> Result<Self, String> {
        let gif = options.is_gif();
        let session = ComSession::start(!gif || options.capture_camera)?;
        let graphics = GraphicsDevice::new(monitor)?;
        let writer = if gif { None } else { Some(MediaFoundationWriter::create(&graphics, options, width, height)?) };
        let (ow, oh) = writer.as_ref().map_or((width, height), |w| (w.width, w.height));
        let processor = GpuVideoProcessor::new(&graphics, width, height, options.frames_per_second, !gif, ow, oh)?;
        let (gif_writer, staging) = if gif {
            (Some(GifWriter::new(options, width as u32, height as u32)?), Some(graphics.create_staging(width, height, DXGI_FORMAT_B8G8R8A8_UNORM)?))
        } else {
            (None, None)
        };
        let encoder = writer.as_ref().map_or_else(GifWriter::encoder, |w| w.encoder.clone());
        let textures = match &writer {
            Some(w) => Some(VideoTexturePool::new(&graphics, &processor, w.encoder_bind_flags)?),
            None => None,
        };
        Ok(Self {
            camera: None,
            textures,
            rented: None,
            gif: gif_writer,
            staging,
            frame: if gif { vec![0; width as usize * height as usize * 4] } else { Vec::new() },
            processor,
            writer,
            encoder,
            nv12_dump: None,
            graphics,
            _session: session,
        })
    }

    /// The writer's frame size (smaller than the canvas past the encoder's limits).
    pub fn output_size(&self) -> (i32, i32) {
        (self.processor.output_width, self.processor.output_height)
    }

    /// The canvas read back (tests and the harness).
    pub fn read_canvas(&mut self) -> Result<Vec<u8>, String> {
        let (w, h) = (self.processor.width, self.processor.height);
        let staging = self.graphics.create_staging(w, h, DXGI_FORMAT_B8G8R8A8_UNORM)?;
        let mut out = vec![0; w as usize * h as usize * 4];
        self.graphics.read_bgra(&staging, &self.processor.canvas, w, h, &mut out)?;
        Ok(out)
    }

    /// `GetBgraFrame` read back: the canvas with the camera tile.
    pub fn read_bgra_frame(&mut self) -> Result<Vec<u8>, String> {
        let (w, h) = (self.processor.width, self.processor.height);
        let texture = self.processor.get_bgra_frame(&self.graphics, self.camera.as_mut())?;
        let staging = self.graphics.create_staging(w, h, DXGI_FORMAT_B8G8R8A8_UNORM)?;
        let mut out = vec![0; w as usize * h as usize * 4];
        self.graphics.read_bgra(&staging, &texture, w, h, &mut out)?;
        Ok(out)
    }
}

impl Canvas for GpuMedia {
    fn size(&self) -> (i32, i32) {
        (self.processor.width, self.processor.height)
    }
    fn clear(&mut self) {
        self.processor.clear(&self.graphics);
    }
    fn write_bgra(&mut self, x: i32, y: i32, width: i32, height: i32, data: &[u8], stride: usize) {
        self.graphics.upload(&self.processor.canvas, crate::options::Rect::new(x, y, width, height), data, stride);
    }
}

impl Media for GpuMedia {
    fn canvas(&mut self) -> &mut dyn Canvas {
        self
    }
    fn encoder(&self) -> VideoEncoderInfo {
        self.encoder.clone()
    }
    fn create_camera_overlay(&mut self, mode: &CameraMode, options: &RecordingOptions) -> Result<(), String> {
        self.camera = Some(CameraOverlay::new(&self.graphics, mode, options, self.processor.width, self.processor.height)?);
        Ok(())
    }
    fn camera_update(&mut self, frame: &CameraFrame) -> Result<(), String> {
        match &mut self.camera {
            Some(c) => c.update(&self.graphics, frame),
            None => Ok(()),
        }
    }
    fn camera_disable(&mut self) {
        if let Some(c) = &mut self.camera {
            c.disable();
        }
    }
    fn try_rent(&mut self, _frame: i64) -> bool {
        match &self.textures {
            None => true,
            Some(t) => {
                self.rented = t.try_rent();
                self.rented.is_some()
            }
        }
    }
    fn write_video(&mut self, frame: i64, timestamp: i64, end: i64, discontinuity: bool) -> Result<bool, String> {
        if let Some(gif) = &mut self.gif {
            let (w, h) = (self.processor.width, self.processor.height);
            let texture = self.processor.get_bgra_frame(&self.graphics, self.camera.as_mut())?;
            self.graphics.read_bgra(self.staging.as_ref().expect("gif staging"), &texture, w, h, &mut self.frame)?;
            return gif.try_write_video(&self.frame, timestamp);
        }
        let (Some(textures), Some(writer)) = (&self.textures, &self.writer) else { return Err("no encoder".into()) };
        let slot = self.rented.take().ok_or("no texture rented")?;
        if let Err(e) = self.processor.convert(&self.graphics, textures.view(slot), frame as u32, self.camera.as_mut()) {
            textures.give_back(slot);
            return Err(e);
        }
        if let Some((count, frames)) = &mut self.nv12_dump
            && frames.len() < *count
        {
            frames.push(self.graphics.read_nv12(textures.texture(slot), self.processor.output_width, self.processor.output_height)?);
        }
        let sample = textures.create_sample(slot)?;
        writer.write_video(&sample, timestamp, end - timestamp, discontinuity)?;
        Ok(true)
    }
    fn write_audio(&mut self, pcm: &[i16], frame_offset: i64) -> Result<(), String> {
        match &self.writer {
            Some(w) => w.write_audio(pcm, frame_offset),
            None => Ok(()),
        }
    }
    fn finish_video(&mut self) -> Result<(), String> {
        match &mut self.writer {
            Some(w) => w.finish(),
            None => Ok(()),
        }
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

/// Makes [`GpuMedia`].
pub struct GpuMediaFactory;

impl MediaFactory for GpuMediaFactory {
    fn create(&self, options: &RecordingOptions, width: i32, height: i32, monitor: isize) -> Result<Box<dyn Media>, String> {
        Ok(Box::new(GpuMedia::new(options, width, height, monitor)?))
    }
}
