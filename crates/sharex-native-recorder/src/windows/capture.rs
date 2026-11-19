//! `Video/GraphicsCapture.cs`: Windows Graphics Capture. A region records
//! one capture session per monitor it touches (`CreateForMonitor`), each
//! frame's part of the region copied into the GPU canvas; a window records
//! the window itself (`CreateForWindow`, which keeps following it when it's
//! covered) and ends when it closes or changes size. Sessions draw the
//! pointer when `IncludeCursor`, have no yellow border when Windows allows
//! borderless capture, and deliver at most `fps` frames a second
//! (`MinUpdateInterval`). HDR monitors (with `HdrToneMapping`) are captured
//! in FP16 and tone mapped (`hdr`).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use windows::Foundation::{TimeSpan, TypedEventHandler};
use windows::Graphics::Capture::{Direct3D11CaptureFramePool, GraphicsCaptureAccess, GraphicsCaptureAccessKind, GraphicsCaptureItem, GraphicsCaptureSession};
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Security::Authorization::AppCapabilityAccess::AppCapabilityAccessStatus;
use windows::Win32::Foundation::{E_NOINTERFACE, HWND, LPARAM, RECT};
use windows::Win32::Graphics::Direct3D11::{D3D11_BIND_SHADER_RESOURCE, D3D11_BOX, ID3D11ShaderResourceView, ID3D11Texture2D};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_R16G16B16A16_FLOAT;
use windows::Win32::Graphics::Gdi::{EnumDisplayMonitors, HDC, HMONITOR, MONITOR_DEFAULTTONEAREST, MonitorFromWindow};
use windows::Win32::System::WinRT::Direct3D11::IDirect3DDxgiInterfaceAccess;
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;
use windows::Win32::UI::WindowsAndMessaging::IsWindow;
use windows::core::{BOOL, IInspectable, Interface};

use super::hdr::{HdrToneMapShader, MonitorColorInfo};
use super::{ComSession, GpuMedia, err};
use crate::clock::TimeSource;
use crate::options::{Rect, RecordingOptions, TICKS_PER_SECOND};
use crate::source::{Canvas, CaptureSources, CaptureTarget, FrameSource, copy_size, region_targets, window_size};

/// `GraphicsCapture.Target`: a monitor's (or the window's) item and its part of the recording.
#[derive(Clone)]
struct Target {
    item: GraphicsCaptureItem,
    place: CaptureTarget,
    window: isize,
}

/// One capture session (`GraphicsCapture.Source`).
struct Source {
    target: Target,
    pool: Direct3D11CaptureFramePool,
    session: GraphicsCaptureSession,
    initial: (i32, i32),
    color: MonitorColorInfo,
    /// HDR sources only: the cropped FP16 frame the tone mapper reads.
    hdr_frame: Option<(ID3D11Texture2D, ID3D11ShaderResourceView)>,
    closed_token: Option<i64>,
    has_frame: bool,
    closed: Arc<AtomicBool>,
}

impl Drop for Source {
    fn drop(&mut self) {
        if let Some(t) = self.closed_token {
            let _ = self.target.item.RemoveClosed(t);
        }
        let _ = self.session.Close();
        let _ = self.pool.Close();
    }
}

fn is_window(handle: isize) -> bool {
    // SAFETY: IsWindow takes any value.
    unsafe { IsWindow(Some(HWND(handle as _))) }.as_bool()
}

/// `CreateItem`: a capture item for a window or a monitor.
fn create_item(handle: isize, window: bool) -> Result<GraphicsCaptureItem, String> {
    let interop = windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>().map_err(err)?;
    // SAFETY: the interop creates an item for the handle (it fails for an invalid one).
    unsafe {
        if window { interop.CreateForWindow(HWND(handle as _)) } else { interop.CreateForMonitor(HMONITOR(handle as _)) }
    }
    .map_err(err)
}

/// `EnumDisplayMonitors`: every monitor's handle and bounds.
pub fn monitors() -> Result<Vec<(isize, Rect)>, String> {
    unsafe extern "system" fn collect(monitor: HMONITOR, _dc: HDC, bounds: *mut RECT, state: LPARAM) -> BOOL {
        // SAFETY: `state` is the Vec passed below; `bounds` is valid for the callback.
        unsafe {
            let list = &mut *(state.0 as *mut Vec<(isize, Rect)>);
            let b = &*bounds;
            list.push((monitor.0 as isize, Rect::from_ltrb(b.left, b.top, b.right, b.bottom)));
        }
        true.into()
    }
    let mut list: Vec<(isize, Rect)> = Vec::new();
    // SAFETY: the callback only runs during the call, with the Vec alive.
    if !unsafe { EnumDisplayMonitors(None, None, Some(collect), LPARAM(&mut list as *mut _ as isize)) }.as_bool() {
        return Err(err(windows::core::Error::from_thread()));
    }
    Ok(list)
}

/// `GetTargets`: the items to capture and the recording's size.
fn get_targets(options: &RecordingOptions) -> Result<(i32, i32, Vec<Target>), String> {
    if !GraphicsCaptureSession::IsSupported().unwrap_or(false) {
        return Err("Windows Graphics Capture is unavailable in this Windows session.".into());
    }
    if options.window_handle != 0 {
        if !is_window(options.window_handle) {
            return Err("The capture window no longer exists.".into());
        }
        let item = create_item(options.window_handle, true)?;
        let size = item.Size().map_err(err)?;
        let (width, height) = window_size(options, size.Width, size.Height)?;
        // SAFETY: a valid window.
        let monitor = unsafe { MonitorFromWindow(HWND(options.window_handle as _), MONITOR_DEFAULTTONEAREST) };
        let place = CaptureTarget { monitor: monitor.0 as isize, source: Rect::new(0, 0, width, height), destination: (0, 0) };
        return Ok((width, height, vec![Target { item, place, window: options.window_handle }]));
    }
    // The size check comes before the monitors, as ShareX's.
    crate::source::region_size(options)?;
    let (width, height, places) = region_targets(options, &monitors()?)?;
    let targets = places.into_iter().map(|place| Ok(Target { item: create_item(place.monitor, false)?, place, window: 0 })).collect::<Result<_, String>>()?;
    Ok((width, height, targets))
}

/// `RequestBorderlessAccess`: asked before the sessions exist so Windows
/// doesn't briefly show the border; optional (a host or policy may refuse).
fn request_borderless_access() -> bool {
    match GraphicsCaptureAccess::RequestAccessAsync(GraphicsCaptureAccessKind::Borderless).and_then(|op| op.join()) {
        Ok(status) => status == AppCapabilityAccessStatus::Allowed,
        Err(e) => {
            eprintln!("Borderless screen capture is unavailable: {}", err(e));
            false
        }
    }
}

/// [`CaptureSources`] on Windows Graphics Capture.
pub struct GraphicsCaptureSources;

impl CaptureSources for GraphicsCaptureSources {
    fn open(&self, options: &RecordingOptions, _time: Arc<dyn TimeSource>) -> Result<Box<dyn FrameSource>, String> {
        // The recording worker is an MTA thread, as .NET's.
        let com = ComSession::start(false)?;
        let (width, height, targets) = get_targets(options)?;
        Ok(Box::new(GraphicsCapture {
            options: options.clone(),
            width,
            height,
            targets,
            sources: Vec::new(),
            tone_mapper: None,
            borderless: false,
            latest: 0,
            _com: com,
        }))
    }
}

/// `GraphicsCapture`.
pub struct GraphicsCapture {
    options: RecordingOptions,
    width: i32,
    height: i32,
    targets: Vec<Target>,
    sources: Vec<Source>,
    tone_mapper: Option<HdrToneMapShader>,
    borderless: bool,
    latest: i64,
    _com: ComSession,
}

// SAFETY: WinRT capture objects are agile and the D3D objects are used by the recording worker only.
unsafe impl Send for GraphicsCapture {}

fn gpu(canvas: &mut dyn Canvas) -> Result<&mut GpuMedia, String> {
    canvas
        .as_any_mut()
        .and_then(|a| a.downcast_mut::<GpuMedia>())
        .ok_or_else(|| "Windows Graphics Capture records into the GPU canvas only.".to_string())
}

impl GraphicsCapture {
    fn create_sources(&mut self, media: &GpuMedia) -> Result<(), String> {
        let graphics = &media.graphics;
        for target in self.targets.clone() {
            // On an HDR display Windows' 8-bit conversion blows out HDR-aware apps: capture FP16 and tone map.
            let color = if self.options.hdr_tone_mapping { MonitorColorInfo::get(target.place.monitor) } else { MonitorColorInfo::SDR };
            let size = target.item.Size().map_err(err)?;
            let format = if color.is_hdr { DirectXPixelFormat::R16G16B16A16Float } else { DirectXPixelFormat::B8G8R8A8UIntNormalized };
            let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(&graphics.winrt_device, format, 2, size).map_err(err)?;
            let session = match pool.CreateCaptureSession(&target.item) {
                Ok(s) => s,
                Err(e) => {
                    let _ = pool.Close();
                    return Err(err(e));
                }
            };
            let mut source = Source {
                target: target.clone(),
                pool,
                session,
                initial: (size.Width, size.Height),
                color,
                hdr_frame: None,
                closed_token: None,
                has_frame: false,
                closed: Arc::new(AtomicBool::new(false)),
            };
            if color.is_hdr {
                if self.tone_mapper.is_none() {
                    self.tone_mapper = Some(HdrToneMapShader::new(graphics)?);
                }
                let s = target.place.source;
                let texture = graphics.create_texture(s.width, s.height, DXGI_FORMAT_R16G16B16A16_FLOAT, D3D11_BIND_SHADER_RESOURCE.0 as u32)?;
                let mut view = None;
                // SAFETY: a shader-resource texture.
                unsafe { graphics.device.CreateShaderResourceView(&texture, None, Some(&mut view)) }.map_err(err)?;
                source.hdr_frame = Some((texture, view.ok_or("no shader view")?));
            }
            configure_capture_interval(&source.session, self.options.frames_per_second)?;
            source.session.SetIsCursorCaptureEnabled(self.options.include_cursor).map_err(err)?;
            if self.borderless {
                source.session.SetIsBorderRequired(false).map_err(err)?;
            }
            let closed = source.closed.clone();
            let handler = TypedEventHandler::<GraphicsCaptureItem, IInspectable>::new(move |_, _| {
                closed.store(true, Ordering::SeqCst);
                Ok(())
            });
            source.closed_token = Some(target.item.Closed(&handler).map_err(err)?);
            self.sources.push(source);
        }
        Ok(())
    }

    fn start_sources(&self) -> Result<(), String> {
        self.sources.iter().try_for_each(|s| s.session.StartCapture().map_err(err))
    }

    /// `Update` for one source; `Ok(())` when it has no new frame.
    fn update_source(&mut self, index: usize, media: &mut GpuMedia) -> Result<(), String> {
        let single = self.sources.len() == 1;
        let source = &mut self.sources[index];
        if source.closed.load(Ordering::SeqCst) {
            return Ok(());
        }
        // Closed isn't always delivered before capture stops publishing frames: the window's
        // validity is a deterministic end even without a WinRT dispatcher.
        if source.target.window != 0 && !is_window(source.target.window) {
            source.closed.store(true, Ordering::SeqCst);
            return Ok(());
        }
        // Bounded drain: the producer may keep publishing while this thread consumes.
        let mut latest = None;
        for _ in 0..2 {
            match source.pool.TryGetNextFrame() {
                Ok(frame) => {
                    if let Some(old) = latest.replace(frame) {
                        let _ = old.Close();
                    }
                }
                Err(e) if e.code().is_ok() => break,
                Err(e) => return Err(err(e)),
            }
        }
        let Some(frame) = latest else { return Ok(()) };
        let result = (|| -> Result<(), String> {
            let content = frame.ContentSize().map_err(err)?;
            if (content.Width, content.Height) != source.initial {
                source.closed.store(true, Ordering::SeqCst);
                return Ok(());
            }
            let crop = source.target.place.source;
            let Some((copy_width, copy_height)) = copy_size(crop, content.Width, content.Height) else { return Ok(()) };
            let surface = frame.Surface().map_err(err)?;
            let access: IDirect3DDxgiInterfaceAccess = surface.cast().map_err(err)?;
            // SAFETY: the frame's surface is a D3D11 texture on the recording's device.
            let texture: ID3D11Texture2D = unsafe { access.GetInterface() }.map_err(err)?;
            let graphics = &media.graphics;
            let processor = &media.processor;
            // Clear padding outside the source (odd sizes are cropped to even NV12 sizes).
            if single && source.target.place.source.x == 0 && source.target.place.source.y == 0 {
                processor.clear(graphics);
            }
            let b = D3D11_BOX { left: crop.x as u32, top: crop.y as u32, front: 0, right: (crop.x + copy_width) as u32, bottom: (crop.y + copy_height) as u32, back: 1 };
            let (dx, dy) = source.target.place.destination;
            if let Some((hdr, view)) = &source.hdr_frame {
                // Frame-pool surfaces aren't guaranteed to be shader readable: copy the crop first.
                // SAFETY: same-format textures; the box lies inside the frame.
                unsafe { graphics.context.CopySubresourceRegion(hdr, 0, 0, 0, 0, &texture, 0, Some(&b)) };
                self.tone_mapper.as_ref().expect("tone mapper").draw(graphics, &processor.canvas_view, view, Rect::new(dx, dy, copy_width, copy_height), source.color);
            } else {
                // SAFETY: BGRA textures; the destination lies inside the canvas.
                unsafe { graphics.context.CopySubresourceRegion(&processor.canvas, 0, dx as u32, dy as u32, 0, &texture, 0, Some(&b)) };
            }
            source.has_frame = true;
            self.latest = self.latest.max(frame.SystemRelativeTime().map_err(err)?.Duration);
            Ok(())
        })();
        let _ = frame.Close();
        result
    }
}

/// `ConfigureCaptureInterval`: at most `fps` frames a second; older Windows
/// (no `IGraphicsCaptureSession5`) keeps its own cadence.
fn configure_capture_interval(session: &GraphicsCaptureSession, fps: i32) -> Result<(), String> {
    // Newer Windows can otherwise throttle WGC to 60 FPS even on a high-refresh display.
    match session.SetMinUpdateInterval(TimeSpan { Duration: TICKS_PER_SECOND / fps as i64 }) {
        Err(e) if e.code() == E_NOINTERFACE => Ok(()),
        r => r.map_err(err),
    }
}

impl FrameSource for GraphicsCapture {
    fn size(&self) -> (i32, i32) {
        (self.width, self.height)
    }
    fn monitor(&self) -> isize {
        self.targets[0].place.monitor
    }
    /// The constructor: borderless access, then the sessions on the canvas's device.
    fn attach(&mut self, canvas: &mut dyn Canvas) -> Result<(), String> {
        let media = gpu(canvas)?;
        self.borderless = request_borderless_access();
        let r = self.create_sources(media);
        if r.is_err() {
            self.sources.clear();
        }
        r
    }
    fn start(&mut self) -> Result<(), String> {
        self.start_sources()
    }
    fn update(&mut self, canvas: &mut dyn Canvas) -> Result<(), String> {
        let media = gpu(canvas)?;
        for i in 0..self.sources.len() {
            self.update_source(i, media)?;
        }
        Ok(())
    }
    fn has_frame(&self) -> bool {
        !self.sources.is_empty() && self.sources.iter().all(|s| s.has_frame)
    }
    fn latest_timestamp(&self) -> i64 {
        self.latest
    }
    fn has_ended(&self) -> bool {
        self.sources.iter().any(|s| s.closed.load(Ordering::SeqCst))
    }
    /// `GetTargets` at the moved region, then `Retarget`: new sessions, a cleared canvas.
    fn retarget(&mut self, region: Rect, canvas: &mut dyn Canvas) -> Result<(), String> {
        let moved = RecordingOptions { region, ..self.options.clone() };
        let (_, _, targets) = get_targets(&moved)?;
        let media = gpu(canvas)?;
        self.sources.clear();
        media.processor.clear(&media.graphics);
        self.latest = 0;
        self.targets = targets;
        let r = self.create_sources(media).and_then(|_| self.start_sources());
        if r.is_err() {
            self.sources.clear();
        }
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Read-only: lists the monitors (none on a headless machine is fine).
    #[test]
    fn monitors_tolerate_any_layout() {
        if let Ok(list) = monitors() {
            assert!(list.iter().all(|(_, b)| b.width > 0 && b.height > 0));
        }
    }

    #[test]
    fn a_closed_window_is_refused() {
        let mut o = RecordingOptions::new("a.mp4");
        // A handle no window has.
        o.window_handle = 0x7FFF_FFF0;
        if GraphicsCaptureSession::IsSupported().unwrap_or(false) {
            assert_eq!(get_targets(&o).err().as_deref(), Some("The capture window no longer exists."));
        }
    }
}
