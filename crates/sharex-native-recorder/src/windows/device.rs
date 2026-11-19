//! `Video/GraphicsDevice.cs`: the D3D11 device everything shares (capture
//! copies, the video processor, the encoder through its DXGI manager), on
//! the adapter driving the recorded monitor.

use windows::Win32::Foundation::HMODULE;
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_UNKNOWN, D3D_FEATURE_LEVEL_11_0, D3D_FEATURE_LEVEL_11_1};
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT, DXGI_SAMPLE_DESC};
use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIAdapter, IDXGIAdapter1, IDXGIFactory1};
use windows::Win32::Graphics::Gdi::HMONITOR;
use windows::core::Interface;

use super::err;

/// `GraphicsDevice`.
pub struct GraphicsDevice {
    pub device: ID3D11Device,
    pub context: ID3D11DeviceContext,
    /// `WinRTDevice`: the device as Windows Graphics Capture takes it.
    pub winrt_device: windows::Graphics::DirectX::Direct3D11::IDirect3DDevice,
}

// SAFETY: the device is created multithread protected (`SetMultithreadProtected`)
// and the recorder uses its immediate context from one thread at a time.
unsafe impl Send for GraphicsDevice {}

impl GraphicsDevice {
    /// On the adapter with `monitor` among its outputs, else the default.
    pub fn new(monitor: isize) -> Result<Self, String> {
        // SAFETY: plain D3D11 creation calls with valid out pointers.
        unsafe {
            let adapter = find_adapter(monitor);
            let (mut device, mut context) = (None, None);
            D3D11CreateDevice(
                adapter.as_ref().map(|a| a.cast::<IDXGIAdapter>()).transpose().map_err(err)?.as_ref(),
                if adapter.is_none() { D3D_DRIVER_TYPE_HARDWARE } else { D3D_DRIVER_TYPE_UNKNOWN },
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT | D3D11_CREATE_DEVICE_VIDEO_SUPPORT,
                Some(&[D3D_FEATURE_LEVEL_11_1, D3D_FEATURE_LEVEL_11_0]),
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            )
            .map_err(err)?;
            let (device, context): (ID3D11Device, ID3D11DeviceContext) = (device.ok_or("no device")?, context.ok_or("no context")?);
            let multithread: ID3D11Multithread = context.cast().map_err(err)?;
            let _ = multithread.SetMultithreadProtected(true);
            let dxgi: windows::Win32::Graphics::Dxgi::IDXGIDevice = device.cast().map_err(err)?;
            let inspectable = windows::Win32::System::WinRT::Direct3D11::CreateDirect3D11DeviceFromDXGIDevice(&dxgi).map_err(err)?;
            let winrt_device = inspectable.cast().map_err(err)?;
            Ok(Self { device, context, winrt_device })
        }
    }

    /// `CreateTexture`: one mip, default usage.
    pub fn create_texture(&self, width: i32, height: i32, format: DXGI_FORMAT, bind_flags: u32) -> Result<ID3D11Texture2D, String> {
        let desc = D3D11_TEXTURE2D_DESC {
            Width: width as u32,
            Height: height as u32,
            MipLevels: 1,
            ArraySize: 1,
            Format: format,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: bind_flags,
            CPUAccessFlags: 0,
            MiscFlags: 0,
        };
        let mut texture = None;
        // SAFETY: a valid description and out pointer.
        unsafe { self.device.CreateTexture2D(&desc, None, Some(&mut texture)) }.map_err(err)?;
        texture.ok_or_else(|| "no texture".into())
    }

    /// A CPU-readable copy target (`GifWriter`'s staging texture).
    pub fn create_staging(&self, width: i32, height: i32, format: DXGI_FORMAT) -> Result<ID3D11Texture2D, String> {
        let desc = D3D11_TEXTURE2D_DESC {
            Width: width as u32,
            Height: height as u32,
            MipLevels: 1,
            ArraySize: 1,
            Format: format,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Usage: D3D11_USAGE_STAGING,
            BindFlags: 0,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            MiscFlags: 0,
        };
        let mut texture = None;
        // SAFETY: a valid description and out pointer.
        unsafe { self.device.CreateTexture2D(&desc, None, Some(&mut texture)) }.map_err(err)?;
        texture.ok_or_else(|| "no texture".into())
    }

    /// Writes CPU pixels into a texture region (`UpdateSubresource`).
    pub fn upload(&self, texture: &ID3D11Texture2D, area: crate::options::Rect, data: &[u8], row_pitch: usize) {
        let b = D3D11_BOX { left: area.x as u32, top: area.y as u32, front: 0, right: area.right() as u32, bottom: area.bottom() as u32, back: 1 };
        // SAFETY: `data` holds `height` rows of `row_pitch` bytes.
        unsafe { self.context.UpdateSubresource(texture, 0, Some(&b), data.as_ptr().cast(), row_pitch as u32, 0) };
    }

    /// Reads a BGRA texture back into tightly packed rows (staging copy and map).
    pub fn read_bgra(&self, staging: &ID3D11Texture2D, texture: &ID3D11Texture2D, width: i32, height: i32, out: &mut [u8]) -> Result<(), String> {
        // SAFETY: the staging texture has the texture's size and format; rows are copied within the mapping.
        unsafe {
            self.context.CopyResource(staging, texture);
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            self.context.Map(staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped)).map_err(err)?;
            let row = width as usize * 4;
            for y in 0..height as usize {
                let src = std::slice::from_raw_parts((mapped.pData as *const u8).add(y * mapped.RowPitch as usize), row);
                out[y * row..(y + 1) * row].copy_from_slice(src);
            }
            self.context.Unmap(staging, 0);
        }
        Ok(())
    }
}

impl GraphicsDevice {
    /// Reads an NV12 texture back: the luma rows, then the interleaved chroma rows.
    pub fn read_nv12(&self, texture: &ID3D11Texture2D, width: i32, height: i32) -> Result<Vec<u8>, String> {
        let staging = self.create_staging(width, height, windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_NV12)?;
        let (w, h) = (width as usize, height as usize);
        let mut out = vec![0u8; w * h * 3 / 2];
        // SAFETY: the staging texture has the texture's size and format; rows are copied within the mapping.
        unsafe {
            self.context.CopyResource(&staging, texture);
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            self.context.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped)).map_err(err)?;
            let pitch = mapped.RowPitch as usize;
            for y in 0..h + h / 2 {
                let src = std::slice::from_raw_parts((mapped.pData as *const u8).add(y * pitch), w);
                out[y * w..(y + 1) * w].copy_from_slice(src);
            }
            self.context.Unmap(&staging, 0);
        }
        Ok(out)
    }
}

/// `FindAdapter`: the adapter one of whose outputs is `monitor`.
fn find_adapter(monitor: isize) -> Option<IDXGIAdapter1> {
    if monitor == 0 {
        return None;
    }
    // SAFETY: DXGI enumeration; every object is released by its wrapper.
    unsafe {
        let factory: IDXGIFactory1 = CreateDXGIFactory1().ok()?;
        let mut i = 0;
        while let Ok(adapter) = factory.EnumAdapters1(i) {
            let mut j = 0;
            while let Ok(output) = adapter.EnumOutputs(j) {
                if let Ok(desc) = output.GetDesc()
                    && desc.Monitor == HMONITOR(monitor as _)
                {
                    return Some(adapter);
                }
                j += 1;
            }
            i += 1;
        }
    }
    None
}
