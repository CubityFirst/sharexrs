//! `Encoding/MediaFoundationWriter.cs` and `Video/VideoTexturePool.cs`: the
//! MP4 sink writer (H.264 Main, or HEVC Main above H.264's limits, from NV12
//! GPU surfaces; AAC-LC from 48 kHz stereo PCM) exactly as ShareX sets it
//! up, falling back through the formats of [`crate::encoder_attempts`] when
//! an encoder rejects one, and the eight NV12 textures the encoder hands
//! back when it's done.

use std::collections::VecDeque;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use windows::Win32::Foundation::E_NOTIMPL;
use windows::Win32::Graphics::Direct3D11::{D3D11_BIND_RENDER_TARGET, D3D11_TEXTURE2D_DESC, ID3D11Texture2D, ID3D11VideoProcessorOutputView};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_NV12;
use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::System::Variant::VARIANT;
use windows::core::{GUID, HRESULT, HSTRING, IUnknown, Interface, PWSTR, implement};

use super::device::GraphicsDevice;
use super::err;
use super::processor::GpuVideoProcessor;
use crate::encoder_attempts::{Codec, EncoderAttempt, build_attempts};
use crate::options::{RecordingOptions, TICKS_PER_SECOND, VideoEncoderInfo};

/// `MediaFoundationWriter`.
pub struct MediaFoundationWriter {
    writer: IMFSinkWriter,
    _manager: IMFDXGIDeviceManager,
    video_stream: u32,
    audio_stream: u32,
    started: bool,
    finalized: bool,
    pub encoder: VideoEncoderInfo,
    pub encoder_bind_flags: u32,
    pub width: i32,
    pub height: i32,
    /// `FramesPerSecond`: the negotiated rate (at most 60, or 30, when the
    /// encoder took no faster one).
    pub frames_per_second: i32,
    /// `AdjustmentNote`: what was negotiated, when not what was asked for.
    pub adjustment_note: Option<String>,
}

/// How an attempt failed, as `Create`'s catch clauses tell them apart.
enum AttemptError {
    /// A Media Foundation call failed (`SharpGenException`).
    Mf { code: HRESULT, message: String },
    /// `NotSupportedException`.
    NotSupported(String),
    /// Anything else.
    Other(String),
}

fn mf(e: windows::core::Error) -> AttemptError {
    AttemptError::Mf { code: e.code(), message: err(e) }
}

// SAFETY: Media Foundation objects are free-threaded; the recorder uses the writer from its worker.
unsafe impl Send for MediaFoundationWriter {}

impl MediaFoundationWriter {
    /// `Create`: the formats of [`build_attempts`] in turn (H.264 when the
    /// capture fits its limits, else HEVC at full size first), moving on when
    /// an HEVC attempt fails or an encoder rejects the media type
    /// (`MF_E_INVALIDMEDIATYPE`); any other failure ends it.
    pub fn create(graphics: &GraphicsDevice, options: &RecordingOptions, width: i32, height: i32) -> Result<Self, String> {
        let (fps, bitrate) = (options.frames_per_second, options.video_bitrate);
        for (i, attempt) in build_attempts(width, height, fps, bitrate, options.require_hardware_encoder).iter().enumerate() {
            match Self::new(graphics, options, attempt) {
                Ok(mut w) => {
                    w.adjustment_note = attempt.adjustment_note(i, width, height, fps, bitrate);
                    return Ok(w);
                }
                // No usable HEVC encoder for this size/rate, or this media type
                // rejected: the next attempt.
                Err(AttemptError::Mf { code, .. }) if attempt.codec == Codec::Hevc || code == MF_E_INVALIDMEDIATYPE => {}
                Err(AttemptError::NotSupported(_)) if attempt.codec == Codec::Hevc => {}
                Err(AttemptError::Mf { message, .. } | AttemptError::NotSupported(message) | AttemptError::Other(message)) => return Err(message),
            }
        }
        Err(format!(
            "Windows Media Foundation rejected the video format ({width}x{height} at {fps} FPS, {} kbps). Update your graphics driver, install the Windows Media Feature Pack if using a Windows N edition, or disable the built-in screen recorder to use FFmpeg.",
            bitrate / 1000
        ))
    }

    fn new(graphics: &GraphicsDevice, options: &RecordingOptions, attempt: &EncoderAttempt) -> Result<Self, AttemptError> {
        let (width, height, fps, full) = (attempt.width, attempt.height, attempt.frames_per_second, attempt.full_media_type);
        let codec = if attempt.codec == Codec::Hevc { MFVideoFormat_HEVC } else { MFVideoFormat_H264 };
        // SAFETY: Media Foundation calls on objects created here.
        unsafe {
            let (mut token, mut manager) = (0u32, None);
            MFCreateDXGIDeviceManager(&mut token, &mut manager).map_err(mf)?;
            let manager = manager.ok_or_else(|| AttemptError::Other("no DXGI device manager".into()))?;
            manager.ResetDevice(&graphics.device, token).map_err(mf)?;
            let mut a = None;
            MFCreateAttributes(&mut a, 5).map_err(mf)?;
            let a = a.ok_or_else(|| AttemptError::Other("no attributes".into()))?;
            if attempt.enable_hardware_transforms {
                a.SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 1).map_err(mf)?;
            }
            if full {
                a.SetUINT32(&MF_LOW_LATENCY, 1).map_err(mf)?;
            }
            // The fixed-size GPU pool bounds in-flight samples. Do not make WriteSample wait for presentation time.
            a.SetUINT32(&MF_SINK_WRITER_DISABLE_THROTTLING, 1).map_err(mf)?;
            a.SetGUID(&MF_TRANSCODE_CONTAINERTYPE, &MFTranscodeContainerType_MPEG4).map_err(mf)?;
            a.SetUnknown(&MF_SINK_WRITER_D3D_MANAGER, &manager).map_err(mf)?;
            let writer = MFCreateSinkWriterFromURL(&HSTRING::from(options.output_path.as_os_str()), None, &a).map_err(mf)?;
            let video_out = create_video_type(&codec, width, height, fps, full).map_err(mf)?;
            video_out.SetUINT32(&MF_MT_AVG_BITRATE, attempt.video_bitrate as u32).map_err(mf)?;
            if full {
                // HEVC Main / H.264 Main.
                video_out.SetUINT32(&MF_MT_MPEG2_PROFILE, if attempt.codec == Codec::Hevc { 1 } else { 77 }).map_err(mf)?;
            }
            let video_stream = writer.AddStream(&video_out).map_err(mf)?;
            let video_in = create_video_type(&MFVideoFormat_NV12, width, height, fps, full).map_err(mf)?;
            writer.SetInputMediaType(video_stream, &video_in, None).map_err(mf)?;
            let mut w = Self {
                writer,
                _manager: manager,
                video_stream,
                audio_stream: 0,
                started: false,
                finalized: false,
                encoder: VideoEncoderInfo { name: String::new(), is_hardware_accelerated: false, is_d3d11_aware: false },
                encoder_bind_flags: 0,
                width,
                height,
                frames_per_second: fps,
                adjustment_note: None,
            };
            if options.has_audio() {
                w.add_audio(options.audio_bitrate, full).map_err(mf)?;
            }
            w.encoder = w.inspect_video_encoder()?;
            if options.require_hardware_encoder && (!w.encoder.is_hardware_accelerated || !w.encoder.is_d3d11_aware) {
                return Err(AttemptError::NotSupported(format!(
                    "Windows selected '{}', which does not provide D3D11 hardware video encoding on this GPU. Disable RequireHardwareEncoder to allow Windows software encoding.",
                    w.encoder.name
                )));
            }
            w.writer.BeginWriting().map_err(mf)?;
            w.started = true;
            Ok(w)
        }
    }

    fn add_audio(&mut self, bitrate: i32, include_profile_level: bool) -> windows::core::Result<()> {
        // SAFETY: media types created here, on this writer.
        unsafe {
            let output = create_type(&MFMediaType_Audio, &MFAudioFormat_AAC)?;
            output.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, 2)?;
            output.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, 48_000)?;
            output.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16)?;
            output.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, (bitrate / 8) as u32)?;
            output.SetUINT32(&MF_MT_AAC_PAYLOAD_TYPE, 0)?;
            if include_profile_level {
                output.SetUINT32(&MF_MT_AAC_AUDIO_PROFILE_LEVEL_INDICATION, 0x29)?;
            }
            self.audio_stream = self.writer.AddStream(&output)?;
            let input = create_type(&MFMediaType_Audio, &MFAudioFormat_PCM)?;
            input.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, 2)?;
            input.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, 48_000)?;
            input.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16)?;
            input.SetUINT32(&MF_MT_AUDIO_BLOCK_ALIGNMENT, 4)?;
            input.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, 192_000)?;
            self.writer.SetInputMediaType(self.audio_stream, &input, None)?;
        }
        Ok(())
    }

    /// `InspectVideoEncoder`: the encoder MFT Windows chose, made low latency
    /// without B-frames, and what it is.
    fn inspect_video_encoder(&mut self) -> Result<VideoEncoderInfo, AttemptError> {
        // SAFETY: COM calls on the writer's own transforms.
        unsafe {
            let extended: IMFSinkWriterEx = self.writer.cast().map_err(mf)?;
            for i in 0..8 {
                let mut category = GUID::zeroed();
                let mut transform = None;
                if extended.GetTransformForStream(self.video_stream, i, Some(&mut category), &mut transform).is_err() {
                    break;
                }
                let Some(transform) = transform else { break };
                if category != MFT_CATEGORY_VIDEO_ENCODER {
                    continue;
                }
                configure_low_latency(&transform);
                let a = transform.GetAttributes().map_err(mf)?;
                let hardware = get_string(&a, &MFT_ENUM_HARDWARE_URL_Attribute).is_some_and(|s| !s.is_empty());
                let d3d11 = a.GetUINT32(&MF_SA_D3D11_AWARE).is_ok_and(|v| v != 0);
                let name = get_string(&a, &MFT_FRIENDLY_NAME_Attribute)
                    .unwrap_or_else(|| if hardware { "Windows hardware video encoder" } else { "Windows video encoder" }.into());
                if let Ok(input) = transform.GetInputStreamAttributes(0)
                    && let Ok(flags) = input.GetUINT32(&MF_SA_D3D11_BINDFLAGS)
                {
                    self.encoder_bind_flags = flags;
                }
                return Ok(VideoEncoderInfo { name, is_hardware_accelerated: hardware, is_d3d11_aware: d3d11 });
            }
        }
        Err(AttemptError::NotSupported("Media Foundation did not expose the selected video encoder.".into()))
    }

    /// `WriteVideo`.
    pub fn write_video(&self, sample: &IMFSample, timestamp: i64, duration: i64, discontinuity: bool) -> Result<(), String> {
        // SAFETY: a sample from the texture pool.
        unsafe {
            sample.SetSampleTime(timestamp).map_err(err)?;
            sample.SetSampleDuration(duration).map_err(err)?;
            if discontinuity {
                sample.SetUINT32(&MFSampleExtension_Discontinuity, 1).map_err(err)?;
            }
            self.writer.WriteSample(self.video_stream, sample).map_err(err)
        }
    }

    /// `WriteAudio`: interleaved 16-bit stereo at frame `frame_offset`.
    pub fn write_audio(&self, pcm: &[i16], frame_offset: i64) -> Result<(), String> {
        let bytes = pcm.len() * 2;
        // SAFETY: the buffer holds `bytes` bytes, filled while locked.
        unsafe {
            let sample = MFCreateSample().map_err(err)?;
            let buffer = MFCreateMemoryBuffer(bytes as u32).map_err(err)?;
            let mut data = std::ptr::null_mut();
            buffer.Lock(&mut data, None, None).map_err(err)?;
            std::ptr::copy_nonoverlapping(pcm.as_ptr().cast::<u8>(), data, bytes);
            buffer.Unlock().map_err(err)?;
            buffer.SetCurrentLength(bytes as u32).map_err(err)?;
            sample.AddBuffer(&buffer).map_err(err)?;
            let time = frame_offset * TICKS_PER_SECOND / 48_000;
            sample.SetSampleTime(time).map_err(err)?;
            sample.SetSampleDuration((frame_offset + (pcm.len() / 2) as i64) * TICKS_PER_SECOND / 48_000 - time).map_err(err)?;
            self.writer.WriteSample(self.audio_stream, &sample).map_err(err)
        }
    }

    /// `Finish`: the end of the video segment, then the file's index.
    pub fn finish(&mut self) -> Result<(), String> {
        if self.started && !self.finalized {
            // SAFETY: a writer that began writing.
            unsafe {
                self.writer.NotifyEndOfSegment(self.video_stream).map_err(err)?;
                self.writer.Finalize().map_err(err)?;
            }
            self.finalized = true;
        }
        Ok(())
    }
}

unsafe fn create_type(major: &GUID, subtype: &GUID) -> windows::core::Result<IMFMediaType> {
    // SAFETY: a new media type.
    unsafe {
        let t = MFCreateMediaType()?;
        t.SetGUID(&MF_MT_MAJOR_TYPE, major)?;
        t.SetGUID(&MF_MT_SUBTYPE, subtype)?;
        Ok(t)
    }
}

/// `CreateVideoType`: progressive, square pixels, and with
/// `include_color_info` BT.709 limited range.
fn create_video_type(subtype: &GUID, width: i32, height: i32, fps: i32, include_color_info: bool) -> windows::core::Result<IMFMediaType> {
    // SAFETY: a new media type.
    unsafe {
        let t = create_type(&MFMediaType_Video, subtype)?;
        t.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
        t.SetUINT64(&MF_MT_FRAME_SIZE, ((width as u64) << 32) | height as u64)?;
        t.SetUINT64(&MF_MT_FRAME_RATE, ((fps as u64) << 32) | 1)?;
        t.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, (1u64 << 32) | 1)?;
        if include_color_info {
            t.SetUINT32(&MF_MT_VIDEO_PRIMARIES, 2)?; // BT.709
            t.SetUINT32(&MF_MT_TRANSFER_FUNCTION, 5)?; // BT.709
            t.SetUINT32(&MF_MT_YUV_MATRIX, 1)?; // BT.709
            t.SetUINT32(&MF_MT_VIDEO_NOMINAL_RANGE, 2)?; // 16-235
        }
        Ok(t)
    }
}

/// `ConfigureLowLatency`: optional driver hints through `ICodecAPI`.
unsafe fn configure_low_latency(transform: &IMFTransform) {
    let Ok(codec) = transform.cast::<ICodecAPI>() else { return };
    // SAFETY: ICodecAPI calls with valid keys and variants.
    unsafe {
        if codec.IsSupported(&CODECAPI_AVEncCommonLowLatency).is_ok() {
            let _ = codec.SetValue(&CODECAPI_AVEncCommonLowLatency, &VARIANT::from(true));
        }
        if codec.IsSupported(&CODECAPI_AVEncMPVDefaultBPictureCount).is_ok() {
            let _ = codec.SetValue(&CODECAPI_AVEncMPVDefaultBPictureCount, &VARIANT::from(0u32));
        }
    }
}

fn get_string(a: &IMFAttributes, key: &GUID) -> Option<String> {
    let mut p = PWSTR::null();
    let mut len = 0u32;
    // SAFETY: the string is allocated by MF and freed here.
    unsafe {
        a.GetAllocatedString(key, &mut p, &mut len).ok()?;
        let s = p.to_string().ok();
        CoTaskMemFree(Some(p.0 as _));
        s
    }
}

/// `SampleReleaseCallback`: Media Foundation releases a tracked sample;
/// its texture goes back to the pool.
#[implement(IMFAsyncCallback)]
struct SampleReleased {
    slot: usize,
    available: Arc<Mutex<VecDeque<usize>>>,
    disposed: Arc<AtomicBool>,
}

impl IMFAsyncCallback_Impl for SampleReleased_Impl {
    fn GetParameters(&self, _flags: *mut u32, _queue: *mut u32) -> windows::core::Result<()> {
        // E_NOTIMPL: the default MF work queue.
        Err(E_NOTIMPL.into())
    }
    fn Invoke(&self, _result: windows::core::Ref<IMFAsyncResult>) -> windows::core::Result<()> {
        if !self.disposed.load(Ordering::SeqCst) {
            self.available.lock().unwrap_or_else(|e| e.into_inner()).push_back(self.slot);
        }
        Ok(())
    }
}

struct Slot {
    texture: ID3D11Texture2D,
    view: ID3D11VideoProcessorOutputView,
    callback: IMFAsyncCallback,
}

/// `VideoTexturePool`: textures come back only after the encoder releases their sample.
pub struct VideoTexturePool {
    slots: Vec<Slot>,
    available: Arc<Mutex<VecDeque<usize>>>,
    disposed: Arc<AtomicBool>,
}

// SAFETY: D3D11/MF objects used by the recording worker; the queue is locked.
unsafe impl Send for VideoTexturePool {}

impl VideoTexturePool {
    pub fn new(graphics: &GraphicsDevice, processor: &GpuVideoProcessor, encoder_bind_flags: u32) -> Result<Self, String> {
        let available = Arc::new(Mutex::new(VecDeque::new()));
        let disposed = Arc::new(AtomicBool::new(false));
        let mut slots = Vec::new();
        for slot in 0..8 {
            let texture = graphics.create_texture(
                processor.output_width,
                processor.output_height,
                DXGI_FORMAT_NV12,
                encoder_bind_flags | D3D11_BIND_RENDER_TARGET.0 as u32,
            )?;
            let view = processor.create_output_view(&texture)?;
            let callback: IMFAsyncCallback = SampleReleased { slot, available: available.clone(), disposed: disposed.clone() }.into();
            slots.push(Slot { texture, view, callback });
            available.lock().unwrap_or_else(|e| e.into_inner()).push_back(slot);
        }
        Ok(Self { slots, available, disposed })
    }

    pub fn try_rent(&self) -> Option<usize> {
        self.available.lock().unwrap_or_else(|e| e.into_inner()).pop_front()
    }

    pub fn give_back(&self, slot: usize) {
        if !self.disposed.load(Ordering::SeqCst) {
            self.available.lock().unwrap_or_else(|e| e.into_inner()).push_back(slot);
        }
    }

    pub fn texture(&self, slot: usize) -> &ID3D11Texture2D {
        &self.slots[slot].texture
    }

    pub fn view(&self, slot: usize) -> &ID3D11VideoProcessorOutputView {
        &self.slots[slot].view
    }

    /// `CreateSample`: a tracked sample over the slot's texture.
    pub fn create_sample(&self, slot: usize) -> Result<IMFSample, String> {
        let s = &self.slots[slot];
        // SAFETY: MF sample and DXGI buffer creation over a valid texture.
        let r = unsafe {
            (|| -> Result<IMFSample, String> {
                let sample = MFCreateVideoSampleFromSurface(None::<&IUnknown>).map_err(err)?;
                let buffer = MFCreateDXGISurfaceBuffer(&ID3D11Texture2D::IID, &s.texture, 0, false).map_err(err)?;
                let mut desc = D3D11_TEXTURE2D_DESC::default();
                s.texture.GetDesc(&mut desc);
                buffer.SetCurrentLength(desc.Width * desc.Height * 3 / 2).map_err(err)?;
                sample.AddBuffer(&buffer).map_err(err)?;
                let tracked: IMFTrackedSample = sample.cast().map_err(err)?;
                tracked.SetAllocator(&s.callback, None::<&IUnknown>).map_err(err)?;
                Ok(sample)
            })()
        };
        if r.is_err() {
            self.give_back(slot);
        }
        r
    }
}

impl Drop for VideoTexturePool {
    fn drop(&mut self) {
        self.disposed.store(true, Ordering::SeqCst);
    }
}

/// The output's folder exists and the name is reserved by ShareX's
/// `FileMode.CreateNew` (never overwriting a user's file).
pub fn reserve_output(path: &Path) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::OpenOptions::new().write(true).create_new(true).open(path).map(|_| ()).map_err(|e| e.to_string())
}
