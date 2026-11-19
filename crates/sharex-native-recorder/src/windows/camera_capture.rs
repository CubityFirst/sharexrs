//! `Video/CameraCapture.cs` and `CameraCaptureDevices.cs`: a Media
//! Foundation camera read asynchronously, keeping only the latest frame
//! (the recording worker never waits for it). The source reader decodes on
//! the recording's GPU device (its DXGI manager, hardware transforms) and
//! never converts to RGB in software: colour conversion is the overlay's
//! video processor. The mode closest to the requested resolution and rate
//! wins ([`crate::camera::ranked_modes`]); MJPEG and H.264 modes are decoded
//! to NV12, YUY2 or RGB32 at their native size and rate.

use std::sync::{Arc, Mutex};

use windows::Win32::Foundation::E_FAIL;
use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Com::CoTaskMemFree;
use windows::core::{GUID, HRESULT, PWSTR, Ref, implement};

use super::err;
use crate::camera::{
    CAMERA_INIT_FAILED, CAMERA_NO_MODE, CAMERA_STOPPED, CameraDevices, CameraFrame, CameraMode, CameraPixelFormat, CameraSource, CameraSubtype, DECODED_FORMATS,
    NativeCameraMode, ranked_modes, requested_size, selected_mode,
};
use crate::devices::{CameraCaptureDevice, camera_unavailable_message, select_camera};
use crate::options::RecordingOptions;
use crate::source::Canvas;

const MF_E_NO_MORE_TYPES: HRESULT = HRESULT(0xC00D36B9_u32 as i32);
const FIRST_VIDEO_STREAM: u32 = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
const ALL_STREAMS: u32 = MF_SOURCE_READER_ALL_STREAMS.0 as u32;

/// A camera's Media Foundation sample, handed to the overlay (`IMFSample`).
#[derive(Debug, Clone)]
pub struct CameraSample(pub IMFSample);

// SAFETY: Media Foundation samples are free threaded.
unsafe impl Send for CameraSample {}
unsafe impl Sync for CameraSample {}

fn subtype_of(guid: GUID) -> CameraSubtype {
    if guid == MFVideoFormat_NV12 {
        CameraSubtype::Nv12
    } else if guid == MFVideoFormat_YUY2 {
        CameraSubtype::Yuy2
    } else if guid == MFVideoFormat_RGB32 {
        CameraSubtype::Rgb32
    } else if guid == MFVideoFormat_MJPG {
        CameraSubtype::Mjpg
    } else if guid == MFVideoFormat_H264 {
        CameraSubtype::H264
    } else {
        CameraSubtype::Other
    }
}

fn guid_of(format: CameraPixelFormat) -> GUID {
    match format {
        CameraPixelFormat::Nv12 => MFVideoFormat_NV12,
        CameraPixelFormat::Yuy2 => MFVideoFormat_YUY2,
        CameraPixelFormat::Rgb32 => MFVideoFormat_RGB32,
    }
}

/// `IMFActivate.GetString`.
fn string(activate: &IMFActivate, key: &GUID) -> windows::core::Result<String> {
    let (mut p, mut n) = (PWSTR::null(), 0u32);
    // SAFETY: the allocated string is freed with CoTaskMemFree.
    unsafe {
        activate.GetAllocatedString(key, &mut p, &mut n)?;
        let s = p.to_string().map_err(|_| windows::core::Error::from_hresult(E_FAIL));
        CoTaskMemFree(Some(p.0 as *const _));
        s
    }
}

/// `MFEnumVideoDeviceSources`: the video capture devices' activation objects.
fn video_devices() -> windows::core::Result<Vec<IMFActivate>> {
    // SAFETY: the activation array is CoTaskMem allocated; each entry is taken (owned) then the array freed.
    unsafe {
        let mut attributes = None;
        MFCreateAttributes(&mut attributes, 1)?;
        let attributes = attributes.ok_or_else(|| windows::core::Error::from_hresult(E_FAIL))?;
        attributes.SetGUID(&MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE, &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID)?;
        let (mut array, mut count) = (std::ptr::null_mut(), 0u32);
        MFEnumDeviceSources(&attributes, &mut array, &mut count)?;
        let mut out = Vec::with_capacity(count as usize);
        for i in 0..count as usize {
            if let Some(a) = std::ptr::read(array.add(i)) {
                out.push(a);
            }
        }
        if !array.is_null() {
            CoTaskMemFree(Some(array as *const _));
        }
        Ok(out)
    }
}

/// `GetCameras`: every camera's symbolic link and name.
pub fn devices() -> Result<Vec<CameraCaptureDevice>, String> {
    use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};
    // SAFETY: COM and MF start-up/shut-down pairs on this thread (an STA UI thread is tolerated).
    unsafe {
        let hr = CoInitializeEx(None, COINIT_MULTITHREADED);
        if hr.is_err() && hr != windows::Win32::Foundation::RPC_E_CHANGED_MODE {
            return Err(err(hr.into()));
        }
        let r = (|| -> Result<Vec<CameraCaptureDevice>, String> {
            MFStartup(MF_VERSION, MFSTARTUP_LITE).map_err(err)?;
            let r = video_devices().and_then(|list| {
                list.iter()
                    .map(|a| Ok(CameraCaptureDevice { id: string(a, &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK)?, name: string(a, &MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME)? }))
                    .collect::<windows::core::Result<Vec<_>>>()
            });
            let _ = MFShutdown();
            r.map_err(err)
        })();
        if hr.is_ok() {
            CoUninitialize();
        }
        r
    }
}

/// The reader's state, shared with its callback.
#[derive(Default)]
struct State {
    reader: Option<IMFSourceReader>,
    latest: Option<IMFSample>,
    failure: Option<String>,
    stopped: bool,
    started: bool,
}

// SAFETY: Media Foundation's source reader and samples are free threaded; the state is only
// used under its lock.
unsafe impl Send for State {}

type Shared = Arc<Mutex<State>>;

fn lock(s: &Shared) -> std::sync::MutexGuard<'_, State> {
    s.lock().unwrap_or_else(|e| e.into_inner())
}

fn request_sample(reader: &IMFSourceReader) -> windows::core::Result<()> {
    // SAFETY: an asynchronous read: the result arrives in the callback.
    unsafe { reader.ReadSample(FIRST_VIDEO_STREAM, 0, None, None, None, None) }
}

/// `SourceReaderCallback`: hands each sample to `OnReadSample`.
#[implement(IMFSourceReaderCallback)]
struct ReaderCallback {
    shared: Shared,
}

impl IMFSourceReaderCallback_Impl for ReaderCallback_Impl {
    /// `OnReadSample`: keep the newest sample and ask for the next one; a failure stops the camera.
    fn OnReadSample(&self, status: HRESULT, _stream: u32, flags: u32, _timestamp: i64, sample: Ref<IMFSample>) -> windows::core::Result<()> {
        let mut s = lock(&self.shared);
        if s.stopped {
            return Ok(());
        }
        let r = (|| -> Result<(), String> {
            status.ok().map_err(err)?;
            let bad = (MF_SOURCE_READERF_ERROR.0 | MF_SOURCE_READERF_ENDOFSTREAM.0 | MF_SOURCE_READERF_CURRENTMEDIATYPECHANGED.0) as u32;
            if flags & bad != 0 {
                return Err(CAMERA_STOPPED.into());
            }
            if let Some(sample) = sample.as_ref() {
                // The callback lends its reference: keep our own until the worker takes it.
                s.latest = Some(sample.clone());
            }
            match &s.reader {
                Some(r) => request_sample(r).map_err(err),
                None => Ok(()),
            }
        })();
        if let Err(e) = r {
            s.failure = Some(e);
        }
        Ok(())
    }
    fn OnFlush(&self, _stream: u32) -> windows::core::Result<()> {
        Ok(())
    }
    fn OnEvent(&self, _stream: u32, _event: Ref<IMFMediaEvent>) -> windows::core::Result<()> {
        Ok(())
    }
}

/// `CameraCapture`.
pub struct MfCamera {
    shared: Shared,
    source: Option<IMFMediaSource>,
    reader: Option<IMFSourceReader>,
    _manager: Option<IMFDXGIDeviceManager>,
    mode: CameraMode,
}

// SAFETY: MF objects are free threaded; the state is locked.
unsafe impl Send for MfCamera {}

fn placeholder_mode() -> CameraMode {
    selected_mode("", CameraPixelFormat::Rgb32, 0, 0, None, 0, None, None)
}

/// The kind of failure the constructor had: ShareX passes `NotSupportedException`s on, wraps the rest.
enum OpenError {
    NotSupported(String),
    Other(String),
}

impl From<windows::core::Error> for OpenError {
    fn from(e: windows::core::Error) -> Self {
        Self::Other(err(e))
    }
}

impl MfCamera {
    pub fn open(options: &RecordingOptions, canvas: &mut dyn Canvas) -> Result<Self, String> {
        let shared: Shared = Arc::new(Mutex::new(State::default()));
        let mut camera = Self { shared: shared.clone(), source: None, reader: None, _manager: None, mode: placeholder_mode() };
        match camera.init(options, canvas) {
            Ok(()) => Ok(camera),
            Err(e) => {
                camera.dispose();
                match e {
                    OpenError::NotSupported(m) => Err(m),
                    OpenError::Other(m) => {
                        eprintln!("Camera initialisation failed: {m}");
                        Err(CAMERA_INIT_FAILED.into())
                    }
                }
            }
        }
    }

    fn init(&mut self, options: &RecordingOptions, canvas: &mut dyn Canvas) -> Result<(), OpenError> {
        let devices = video_devices()?;
        let links: Vec<String> = devices.iter().map(|d| string(d, &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK).unwrap_or_default()).collect();
        let id = options.camera_device_id.as_deref();
        let index = select_camera(links.iter().map(String::as_str), id).ok_or_else(|| OpenError::NotSupported(camera_unavailable_message(id).into()))?;
        let selected = &devices[index];
        let name = string(selected, &MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME)?;
        // SAFETY: Media Foundation calls on objects created here.
        unsafe {
            let source: IMFMediaSource = selected.ActivateObject()?;
            self.source = Some(source.clone());
            // The recording's GPU device decodes (ShareX always has one; the CPU test canvas doesn't).
            let device = canvas.as_any_mut().and_then(|a| a.downcast_mut::<super::GpuMedia>()).map(|m| m.graphics.device.clone());
            let manager = match device {
                Some(device) => {
                    let (mut token, mut manager) = (0u32, None);
                    MFCreateDXGIDeviceManager(&mut token, &mut manager)?;
                    let manager = manager.ok_or_else(|| windows::core::Error::from_hresult(E_FAIL))?;
                    manager.ResetDevice(&device, token)?;
                    Some(manager)
                }
                None => None,
            };
            let callback: IMFSourceReaderCallback = ReaderCallback { shared: self.shared.clone() }.into();
            let mut attributes = None;
            MFCreateAttributes(&mut attributes, 3)?;
            let attributes = attributes.ok_or_else(|| windows::core::Error::from_hresult(E_FAIL))?;
            attributes.SetUnknown(&MF_SOURCE_READER_ASYNC_CALLBACK, &callback)?;
            if let Some(m) = &manager {
                attributes.SetUnknown(&MF_SOURCE_READER_D3D_MANAGER, m)?;
            }
            attributes.SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 1)?;
            // No software RGB conversion: colour conversion is on the GPU.
            let reader = MFCreateSourceReaderFromMediaSource(&source, &attributes)?;
            self._manager = manager;
            self.reader = Some(reader.clone());
            lock(&self.shared).reader = Some(reader.clone());
            reader.SetStreamSelection(ALL_STREAMS, false)?;
            reader.SetStreamSelection(FIRST_VIDEO_STREAM, true)?;
            self.mode = configure_format(&reader, &name, options)?;
        }
        Ok(())
    }

    /// `Dispose`: detach the callback, then flush and shut the source down outside the lock.
    fn dispose(&mut self) {
        {
            let mut s = lock(&self.shared);
            if s.stopped {
                return;
            }
            s.stopped = true;
            s.latest = None;
            // Breaks the reader <-> callback cycle.
            s.reader = None;
        }
        // SAFETY: shutting down objects created in `init`; errors are ignored as ShareX's.
        unsafe {
            if let Some(r) = &self.reader {
                let _ = r.Flush(FIRST_VIDEO_STREAM);
            }
            if let Some(s) = &self.source {
                let _ = s.Shutdown();
            }
        }
        self.reader = None;
        self.source = None;
    }
}

/// `ConfigureFormat`: the native modes scored, then tried best first.
unsafe fn configure_format(reader: &IMFSourceReader, name: &str, options: &RecordingOptions) -> Result<CameraMode, OpenError> {
    let requested = requested_size(&options.camera_resolution);
    let mut types = Vec::new();
    let mut modes = Vec::new();
    for index in 0..512 {
        // SAFETY: enumerating the reader's native types.
        let t = match unsafe { reader.GetNativeMediaType(FIRST_VIDEO_STREAM, index) } {
            Ok(t) => t,
            Err(e) if e.code() == MF_E_NO_MORE_TYPES => break,
            Err(e) => return Err(e.into()),
        };
        // SAFETY: attribute reads on a media type.
        let mode = unsafe {
            NativeCameraMode {
                subtype: subtype_of(t.GetGUID(&MF_MT_SUBTYPE)?),
                frame_size: t.GetUINT64(&MF_MT_FRAME_SIZE).unwrap_or(0),
                frame_rate: t.GetUINT64(&MF_MT_FRAME_RATE).unwrap_or(0),
            }
        };
        types.push(t);
        modes.push(mode);
    }
    let mut last_error: Option<windows::core::Error> = None;
    for i in ranked_modes(&modes, requested, options.camera_frames_per_second) {
        let (t, mode) = (&types[i], modes[i]);
        // SAFETY: setting the reader's media type.
        if let Err(e) = unsafe { reader.SetCurrentMediaType(FIRST_VIDEO_STREAM, None, t) } {
            last_error = Some(e);
            continue;
        }
        if mode.subtype.is_raw() {
            // SAFETY: reading the type just set.
            match unsafe { read_selected_format(reader, name) } {
                Ok(m) => return Ok(m),
                Err(e) => {
                    last_error = Some(e);
                    continue;
                }
            }
        }
        // Decoders expose different raw formats: keep the chosen native size and rate.
        for decoded in DECODED_FORMATS {
            // SAFETY: a new media type set on the reader.
            let r = unsafe {
                (|| -> windows::core::Result<CameraMode> {
                    let d = MFCreateMediaType()?;
                    d.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
                    d.SetGUID(&MF_MT_SUBTYPE, &guid_of(decoded))?;
                    d.SetUINT64(&MF_MT_FRAME_SIZE, t.GetUINT64(&MF_MT_FRAME_SIZE)?)?;
                    d.SetUINT64(&MF_MT_FRAME_RATE, t.GetUINT64(&MF_MT_FRAME_RATE)?)?;
                    reader.SetCurrentMediaType(FIRST_VIDEO_STREAM, None, &d)?;
                    read_selected_format(reader, name)
                })()
            };
            match r {
                Ok(m) => return Ok(m),
                Err(e) => last_error = Some(e),
            }
        }
    }
    if let Some(e) = last_error {
        eprintln!("The camera's last capture mode failed: {}", err(e));
    }
    Err(OpenError::NotSupported(CAMERA_NO_MODE.into()))
}

/// `ReadSelectedFormat`.
unsafe fn read_selected_format(reader: &IMFSourceReader, name: &str) -> windows::core::Result<CameraMode> {
    // SAFETY: attribute reads on the reader's current type.
    unsafe {
        let current = reader.GetCurrentMediaType(FIRST_VIDEO_STREAM)?;
        let subtype = current.GetGUID(&MF_MT_SUBTYPE)?;
        let format = subtype_of(subtype).pixel_format().unwrap_or(CameraPixelFormat::Rgb32);
        let size = current.GetUINT64(&MF_MT_FRAME_SIZE)?;
        let rate = current.GetUINT64(&MF_MT_FRAME_RATE)?;
        let width = (size >> 32) as u32;
        let default_stride = current.GetUINT32(&MF_MT_DEFAULT_STRIDE).ok();
        // BitConverter.ToInt32(Subtype.ToByteArray()): the GUID's first field, the FOURCC.
        let bitmap_stride = if matches!(default_stride, Some(s) if s != 0) { 0 } else { MFGetStrideForBitmapInfoHeader(subtype.data1, width)? };
        Ok(selected_mode(
            name,
            format,
            size,
            rate,
            default_stride,
            bitmap_stride,
            current.GetUINT32(&MF_MT_YUV_MATRIX).ok(),
            current.GetUINT32(&MF_MT_VIDEO_NOMINAL_RANGE).ok(),
        ))
    }
}

impl CameraSource for MfCamera {
    fn mode(&self) -> &CameraMode {
        &self.mode
    }
    fn start(&mut self) -> Result<(), String> {
        let mut s = lock(&self.shared);
        if s.stopped {
            return Err("Cannot access a disposed object.".into());
        }
        if s.started {
            return Ok(());
        }
        if let Some(r) = &s.reader {
            request_sample(r).map_err(err)?;
        }
        s.started = true;
        Ok(())
    }
    fn take_latest_frame(&mut self) -> Option<CameraFrame> {
        let sample = lock(&self.shared).latest.take()?;
        Some(CameraFrame { data: Vec::new(), stride: self.mode.stride, sample: Some(CameraSample(sample)) })
    }
    fn failure(&self) -> Option<String> {
        lock(&self.shared).failure.clone()
    }
}

impl Drop for MfCamera {
    fn drop(&mut self) {
        self.dispose();
    }
}

/// [`CameraDevices`] on Media Foundation.
pub struct MfCameraDevices;

impl CameraDevices for MfCameraDevices {
    fn open(&self, options: &RecordingOptions, canvas: &mut dyn Canvas) -> Result<Box<dyn CameraSource>, String> {
        Ok(Box::new(MfCamera::open(options, canvas)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subtypes() {
        assert_eq!(subtype_of(MFVideoFormat_NV12), CameraSubtype::Nv12);
        assert_eq!(subtype_of(MFVideoFormat_MJPG), CameraSubtype::Mjpg);
        assert_eq!(subtype_of(MFVideoFormat_H264), CameraSubtype::H264);
        assert_eq!(subtype_of(MFVideoFormat_ARGB32), CameraSubtype::Other);
        for f in DECODED_FORMATS {
            assert_eq!(subtype_of(guid_of(f)).pixel_format(), Some(f));
        }
        // RGB32's FOURCC is D3DFMT_X8R8G8B8 (22): 4 bytes a pixel.
        // SAFETY: a pure computation.
        assert_eq!(unsafe { MFGetStrideForBitmapInfoHeader(MFVideoFormat_RGB32.data1, 640) }.unwrap().abs(), 2560);
        assert_eq!(unsafe { MFGetStrideForBitmapInfoHeader(MFVideoFormat_YUY2.data1, 640) }.unwrap().abs(), 1280);
    }

    /// Read-only enumeration, whatever cameras exist (none in CI).
    #[test]
    fn camera_list_tolerates_any_result() {
        if let Ok(list) = devices() {
            assert!(list.iter().all(|d| !d.id.is_empty()));
        }
    }
}
