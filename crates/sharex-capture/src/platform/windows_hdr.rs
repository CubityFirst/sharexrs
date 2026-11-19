//! `HDRScreenCapture`'s Windows side: DXGI desktop duplication of every HDR
//! output a capture touches (`ApplyColorCorrection`,
//! `ApplyAdapterColorCorrection`, `CaptureOutput`, `TryAcquireDesktopFrame`)
//! and Windows' own tone mapper (`ConvertWithWindowsToneMapper`: WIC's
//! `IWICBitmapToneMapper`). The pixel work is `crate::hdr`.

use image::RgbaImage;
use windows::Win32::Foundation::HMODULE;
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_UNKNOWN, D3D_FEATURE_LEVEL_11_0, D3D_FEATURE_LEVEL_11_1};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE, D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC,
    D3D11_USAGE_STAGING, D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
};
use windows::Win32::Graphics::Dwm::DwmFlush;
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_COLOR_SPACE_RGB_FULL_G2084_NONE_P2020, DXGI_FORMAT, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_R10G10B10A2_UNORM, DXGI_FORMAT_R16G16B16A16_FLOAT,
};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, DXGI_OUTDUPL_FRAME_INFO, DXGI_OUTPUT_DESC1, IDXGIAdapter1, IDXGIFactory1, IDXGIOutput, IDXGIOutput5, IDXGIOutput6,
    IDXGIOutputDuplication, IDXGIResource,
};
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_WICPixelFormat32bppBGRA, GUID_WICPixelFormat32bppR10G10B10A2HDR10, GUID_WICPixelFormat64bppRGBAHalf, IWICImagingFactory,
    WICRect,
};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize};
use windows::core::Interface;

use crate::geometry::Rect;
use crate::hdr::{self, CapturedOutput, HdrFormat, HdrFrame, ModeRotation, OutputColor};

/// `FrameAcquireTimeout`.
const FRAME_ACQUIRE_TIMEOUT: u32 = 500;
/// `MaxFrameAcquireAttempts`.
const MAX_FRAME_ACQUIRE_ATTEMPTS: usize = 3;
/// `supportedFormats`.
const SUPPORTED_FORMATS: [DXGI_FORMAT; 3] = [DXGI_FORMAT_R16G16B16A16_FLOAT, DXGI_FORMAT_R10G10B10A2_UNORM, DXGI_FORMAT_B8G8R8A8_UNORM];
/// `WICBitmapToneMappingMode_Default`.
const TONE_MAPPING_MODE_DEFAULT: i32 = 1;

// wincodec.h (10.0.26100): not in the `windows` crate's metadata yet.
// `IWICImagingFactory2` only adds `CreateImageEncoder`, declared here as a
// raw slot so `CreateBitmapToneMapper` lands at its vtable index.
#[allow(non_snake_case)]
mod wic3 {
    use windows::Win32::Graphics::Imaging::{IWICBitmapSource, IWICBitmapSource_Impl, IWICBitmapSource_Vtbl, IWICImagingFactory, IWICImagingFactory_Impl, IWICImagingFactory_Vtbl};
    use windows::core::{GUID, HRESULT, interface};

    #[interface("489b3d8b-624a-4258-b678-7eece70f299d")]
    pub unsafe trait IWICImagingFactory3: IWICImagingFactory {
        fn CreateImageEncoder(&self, device: *mut core::ffi::c_void, encoder: *mut *mut core::ffi::c_void) -> HRESULT;
        fn CreateBitmapToneMapper(&self, tone_mapper: *mut Option<IWICBitmapToneMapper>) -> HRESULT;
    }

    #[interface("44728ded-1edf-4fe9-b50b-c89a264c9439")]
    pub unsafe trait IWICBitmapToneMapper: IWICBitmapSource {
        fn InitializeForHdrTarget(&self, source: *mut core::ffi::c_void, format: *const GUID, luminance: f32, white_level: f32, mode: i32) -> HRESULT;
        fn InitializeForSdrTarget(&self, source: *mut core::ffi::c_void, format: *const GUID, mode: i32) -> HRESULT;
    }

    /// `IWICImagingFactory3.CreateBitmapToneMapper`.
    ///
    /// # Safety
    /// A COM call on a live factory.
    pub unsafe fn create_tone_mapper(factory: &IWICImagingFactory3) -> windows::core::Result<IWICBitmapToneMapper> {
        let mut tone_mapper = None;
        unsafe { factory.CreateBitmapToneMapper(&mut tone_mapper) }.ok()?;
        tone_mapper.ok_or_else(|| windows::core::Error::from_hresult(windows::Win32::Foundation::E_POINTER))
    }

    /// `IWICBitmapToneMapper.InitializeForSdrTarget`.
    ///
    /// # Safety
    /// A COM call on a live tone mapper and source.
    pub unsafe fn initialize_for_sdr_target(tone_mapper: &IWICBitmapToneMapper, source: *mut core::ffi::c_void, format: &GUID, mode: i32) -> windows::core::Result<()> {
        unsafe { tone_mapper.InitializeForSdrTarget(source, format, mode) }.ok()
    }
}
use wic3::IWICImagingFactory3;

/// COM on this thread for WIC, as .NET's threads have it.
struct ComScope(bool);

impl ComScope {
    fn new() -> Self {
        // SAFETY: balanced by `Drop` when it succeeded; a thread already in
        // another apartment keeps it (RPC_E_CHANGED_MODE).
        Self(unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok())
    }
}

impl Drop for ComScope {
    fn drop(&mut self) {
        if self.0 {
            // SAFETY: pairs the successful `CoInitializeEx` above.
            unsafe { CoUninitialize() };
        }
    }
}

/// `ConvertWithWindowsToneMapper`: WIC's tone mapper from the frame's format
/// (`64bppRGBAHalf` or `32bppR10G10B10A2HDR10`) to SDR `32bppBGRA` in its
/// default mode; RGBA here. Fails where Windows has no tone mapper
/// (before `IWICImagingFactory3`), which ShareX catches.
pub fn wic_tone_map(frame: &HdrFrame) -> Result<RgbaImage, String> {
    let _com = ComScope::new();
    let source_format = match frame.format {
        HdrFormat::Rgba16Float => GUID_WICPixelFormat64bppRGBAHalf,
        HdrFormat::R10G10B10A2 => GUID_WICPixelFormat32bppR10G10B10A2HDR10,
    };
    let (width, height) = (frame.width, frame.height);
    // `checked(mapped.RowPitch * height)`
    let size = frame.row_pitch.checked_mul(height as usize).ok_or("overflow")?;
    let data = frame.data.get(..size).ok_or("short frame")?;
    // SAFETY: WIC calls on live interfaces; buffers sized as declared.
    unsafe {
        let factory: IWICImagingFactory = CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER).map_err(|e| e.to_string())?;
        let factory3: IWICImagingFactory3 = factory.cast().map_err(|e| e.to_string())?;
        let source = factory.CreateBitmapFromMemory(width, height, &source_format, frame.row_pitch as u32, data).map_err(|e| e.to_string())?;
        let tone_mapper = wic3::create_tone_mapper(&factory3).map_err(|e| e.to_string())?;
        wic3::initialize_for_sdr_target(&tone_mapper, source.as_raw(), &GUID_WICPixelFormat32bppBGRA, TONE_MAPPING_MODE_DEFAULT)
            .map_err(|e| e.to_string())?;
        let stride = width as usize * 4;
        let mut bgra = vec![0u8; stride * height as usize];
        let rect = WICRect { X: 0, Y: 0, Width: width as i32, Height: height as i32 };
        tone_mapper.CopyPixels(&rect, stride as u32, &mut bgra).map_err(|e| e.to_string())?;
        for p in bgra.chunks_exact_mut(4) {
            p.swap(0, 2);
        }
        RgbaImage::from_raw(width, height, bgra).ok_or_else(|| "size".to_string())
    }
}

/// `ApplyColorCorrection`: corrects `destination` (the GDI capture of
/// `capture`) from every HDR output it touches, on every adapter; true when
/// any was corrected.
pub fn apply_color_correction(destination: &mut RgbaImage, capture: Rect) -> Result<bool, String> {
    // SAFETY: DXGI enumeration; every object is released by its wrapper.
    let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1() }.map_err(|e| e.to_string())?;
    let mut corrected = false;
    let mut index = 0;
    // SAFETY: as above.
    while let Ok(adapter) = unsafe { factory.EnumAdapters1(index) } {
        corrected |= apply_adapter_color_correction(&adapter, destination, capture);
        index += 1;
    }
    Ok(corrected)
}

/// `Rectangle.IntersectsWith`.
fn intersects(a: Rect, b: Rect) -> bool {
    b.x < a.right() && a.x < b.right() && b.y < a.bottom() && a.y < b.bottom()
}

/// `GetOutputBounds`.
fn output_bounds(d: &DXGI_OUTPUT_DESC1) -> Rect {
    let r = d.DesktopCoordinates;
    Rect::new(r.left, r.top, r.right - r.left, r.bottom - r.top)
}

/// `IsHdrOutput`: on the desktop and composed as BT.2020 PQ.
fn is_hdr_output(d: &DXGI_OUTPUT_DESC1) -> bool {
    d.AttachedToDesktop.as_bool() && d.ColorSpace == DXGI_COLOR_SPACE_RGB_FULL_G2084_NONE_P2020
}

/// `ApplyAdapterColorCorrection`: one device per adapter, made for its first
/// HDR output in the capture; an output that fails is logged and skipped.
fn apply_adapter_color_correction(adapter: &IDXGIAdapter1, destination: &mut RgbaImage, capture: Rect) -> bool {
    let mut device: Option<(ID3D11Device, ID3D11DeviceContext)> = None;
    let mut corrected = false;
    let mut index = 0;
    // SAFETY: DXGI/D3D11 calls on live interfaces.
    while let Ok(output) = unsafe { adapter.EnumOutputs(index) } {
        index += 1;
        // Ok(false): no device for the adapter (`return corrected`).
        let result = (|| -> Result<bool, String> {
            let output6: IDXGIOutput6 = output.cast().map_err(|e| e.to_string())?;
            // SAFETY: as above.
            let description = unsafe { output6.GetDesc1() }.map_err(|e| e.to_string())?;
            let bounds = output_bounds(&description);
            if !is_hdr_output(&description) || !intersects(capture, bounds) {
                return Ok(true);
            }
            if device.is_none() {
                let (mut d, mut c) = (None, None);
                // SAFETY: as above.
                let made = unsafe {
                    D3D11CreateDevice(
                        adapter,
                        D3D_DRIVER_TYPE_UNKNOWN,
                        HMODULE::default(),
                        D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                        Some(&[D3D_FEATURE_LEVEL_11_1, D3D_FEATURE_LEVEL_11_0]),
                        D3D11_SDK_VERSION,
                        Some(&mut d),
                        None,
                        Some(&mut c),
                    )
                };
                match (made, d, c) {
                    (Ok(()), Some(d), Some(c)) => device = Some((d, c)),
                    // `return corrected`: the adapter's other outputs are skipped too.
                    _ => return Ok(false),
                }
            }
            let (d, c) = device.as_ref().expect("made above");
            if let Some(captured) = capture_output(&output, &description, d, c)? {
                hdr::copy_output_intersection(destination, capture, &captured, bounds);
                corrected = true;
            }
            Ok(true)
        })();
        match result {
            Ok(false) => return corrected,
            Ok(true) => {}
            Err(e) => eprintln!("HDR capture failed for a display output. {e}"),
        }
    }
    corrected
}

/// `CaptureOutput`: duplicates the output in an HDR format, copies its
/// frame to a staging texture, maps it and turns it into a
/// [`CapturedOutput`].
fn capture_output(
    output: &IDXGIOutput,
    description: &DXGI_OUTPUT_DESC1,
    device: &ID3D11Device,
    context: &ID3D11DeviceContext,
) -> Result<Option<CapturedOutput>, String> {
    let output5: IDXGIOutput5 = output.cast().map_err(|e| e.to_string())?;
    // SAFETY: DXGI/D3D11 calls on live interfaces; the frame is released
    // and the staging texture unmapped on every path.
    unsafe {
        let duplication = output5.DuplicateOutput1(device, 0, &SUPPORTED_FORMATS).map_err(|e| e.to_string())?;
        let Some(resource) = try_acquire_desktop_frame(&duplication) else { return Ok(None) };
        let result = (|| -> Result<Option<CapturedOutput>, String> {
            let texture: ID3D11Texture2D = resource.cast().map_err(|e| e.to_string())?;
            let mut desc = D3D11_TEXTURE2D_DESC::default();
            texture.GetDesc(&mut desc);
            let format = match desc.Format {
                DXGI_FORMAT_R16G16B16A16_FLOAT => HdrFormat::Rgba16Float,
                DXGI_FORMAT_R10G10B10A2_UNORM => HdrFormat::R10G10B10A2,
                _ => return Ok(None),
            };
            let mut staging_desc = desc;
            staging_desc.Usage = D3D11_USAGE_STAGING;
            staging_desc.BindFlags = 0;
            staging_desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
            staging_desc.MiscFlags = 0;
            let mut staging = None;
            device.CreateTexture2D(&staging_desc, None, Some(&mut staging)).map_err(|e| e.to_string())?;
            let staging = staging.ok_or("no staging texture")?;
            context.CopyResource(&staging, &texture);
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            if context.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped)).is_err() {
                return Ok(None);
            }
            let name = &description.DeviceName;
            let name = String::from_utf16_lossy(&name[..name.iter().position(|&c| c == 0).unwrap_or(name.len())]);
            let color = OutputColor {
                sdr_white_scale: sharex_helpers::display_config::sdr_white_scale(&name),
                peak_nits: hdr::peak_nits(description.MaxLuminance),
                rotation: ModeRotation::from_dxgi(description.Rotation.0),
                bounds: output_bounds(description),
            };
            let row_pitch = mapped.RowPitch as usize;
            let data = std::slice::from_raw_parts(mapped.pData as *const u8, row_pitch * desc.Height as usize);
            let frame = HdrFrame { format, width: desc.Width, height: desc.Height, row_pitch, data };
            let captured = hdr::captured_output(&frame, &color, wic_tone_map);
            context.Unmap(&staging, 0);
            Ok(captured)
        })();
        let _ = duplication.ReleaseFrame();
        result
    }
}

/// `TryAcquireDesktopFrame`: up to three frames, skipping pointer-only
/// updates (a new duplication's first frame can have no desktop image).
fn try_acquire_desktop_frame(duplication: &IDXGIOutputDuplication) -> Option<IDXGIResource> {
    for _ in 0..MAX_FRAME_ACQUIRE_ATTEMPTS {
        // SAFETY: DWM and DXGI calls on a live duplication.
        unsafe {
            let _ = DwmFlush();
            let mut info = DXGI_OUTDUPL_FRAME_INFO::default();
            let mut resource = None;
            if duplication.AcquireNextFrame(FRAME_ACQUIRE_TIMEOUT, &mut info, &mut resource).is_err() {
                return None;
            }
            if info.LastPresentTime != 0 {
                return resource;
            }
            let _ = duplication.ReleaseFrame();
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WIC's tone mapper on a synthetic FP16 frame (no screen): Windows
    /// without `IWICImagingFactory3` fails, which is ShareX's fallback.
    #[test]
    fn wic_tone_maps_a_synthetic_frame() {
        let pixels: [[u16; 4]; 2] = [[0x3800, 0x3800, 0x3800, 0x3C00], [0x4400, 0x4000, 0x3C00, 0x3C00]];
        let data: Vec<u8> = pixels.iter().flat_map(|p| p.iter().flat_map(|c| c.to_le_bytes())).collect();
        let frame = HdrFrame { format: HdrFormat::Rgba16Float, width: 2, height: 1, row_pitch: 16, data: &data };
        match wic_tone_map(&frame) {
            Ok(img) => assert_eq!((img.width(), img.height()), (2, 1)),
            Err(e) => eprintln!("no WIC tone mapper here: {e}"),
        }
    }
}
