//! `Video/MonitorColorInfo.cs`, `Video/HdrToneMapShader.cs` and
//! ShareX.HelpersLib's `DisplayConfigHelper.GetSdrWhiteScale`: on an HDR
//! monitor (with `ScreenRecordHDRToneMapping`) Windows Graphics Capture
//! delivers FP16 scRGB (1.0 = 80 nits), which a pixel shader divides by the
//! SDR white level and compresses above it, then encodes as sRGB into the
//! BGRA canvas.

use windows::Win32::Devices::Display::{
    DISPLAYCONFIG_DEVICE_INFO_GET_SDR_WHITE_LEVEL, DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME, DISPLAYCONFIG_DEVICE_INFO_HEADER, DISPLAYCONFIG_MODE_INFO,
    DISPLAYCONFIG_PATH_INFO, DISPLAYCONFIG_SDR_WHITE_LEVEL, DISPLAYCONFIG_SOURCE_DEVICE_NAME, DisplayConfigGetDeviceInfo, GetDisplayConfigBufferSizes,
    QDC_ONLY_ACTIVE_PATHS, QueryDisplayConfig,
};
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::Graphics::Direct3D::D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::DXGI_COLOR_SPACE_RGB_FULL_G2084_NONE_P2020;
use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1, IDXGIOutput6};
use windows::Win32::Graphics::Gdi::HMONITOR;
use windows::core::Interface;

use super::device::GraphicsDevice;
use super::err;
use crate::options::Rect;

const SCRGB_NITS: f32 = 80.0;
const DEFAULT_PEAK_NITS: f32 = 1000.0;

/// `MonitorColorInfo`: whether a monitor shows an HDR desktop, its SDR white
/// (scRGB units, from "SDR content brightness") and peak luminance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MonitorColorInfo {
    pub is_hdr: bool,
    pub sdr_white_scale: f32,
    pub peak_nits: f32,
}

impl MonitorColorInfo {
    pub const SDR: Self = Self { is_hdr: false, sdr_white_scale: 1.0, peak_nits: SCRGB_NITS };

    /// `RelativePeak`: the display's peak over SDR white, the highlight headroom the tone mapper compresses.
    pub fn relative_peak(&self) -> f32 {
        (self.peak_nits / (SCRGB_NITS * self.sdr_white_scale)).clamp(1.25, 125.0)
    }

    /// The peak `Get` keeps: the reported one when it's a real luminance, else 1000 nits.
    pub fn peak_from(max_luminance: f32) -> f32 {
        if max_luminance.is_finite() && max_luminance >= SCRGB_NITS { max_luminance } else { DEFAULT_PEAK_NITS }
    }

    /// `Get`: the monitor's DXGI output; SDR when it isn't composed as BT.2020 PQ or can't be read.
    pub fn get(monitor: isize) -> Self {
        // SAFETY: DXGI enumeration; every object is released by its wrapper.
        let r = unsafe {
            (|| -> windows::core::Result<Self> {
                let factory: IDXGIFactory1 = CreateDXGIFactory1()?;
                let mut i = 0;
                while let Ok(adapter) = factory.EnumAdapters1(i) {
                    let mut j = 0;
                    while let Ok(output) = adapter.EnumOutputs(j) {
                        j += 1;
                        if output.GetDesc()?.Monitor != HMONITOR(monitor as _) {
                            continue;
                        }
                        let output6: IDXGIOutput6 = output.cast()?;
                        let d = output6.GetDesc1()?;
                        // Windows composes an HDR desktop in scRGB and scans it out as BT.2020 PQ.
                        if d.ColorSpace != DXGI_COLOR_SPACE_RGB_FULL_G2084_NONE_P2020 {
                            return Ok(Self::SDR);
                        }
                        let name = String::from_utf16_lossy(&d.DeviceName[..d.DeviceName.iter().position(|&c| c == 0).unwrap_or(d.DeviceName.len())]);
                        return Ok(Self { is_hdr: true, sdr_white_scale: sdr_white_scale(&name), peak_nits: Self::peak_from(d.MaxLuminance) });
                    }
                    i += 1;
                }
                Ok(Self::SDR)
            })()
        };
        r.unwrap_or_else(|e| {
            // Detection is best effort: the 8-bit capture path remains correct for SDR desktops.
            eprintln!("Could not query the display's HDR state. {}", err(e));
            Self::SDR
        })
    }
}

/// `GetSdrWhiteScale`'s value from `DISPLAYCONFIG_SDR_WHITE_LEVEL` (thousandths of 80 nits).
pub fn white_scale_from_level(level: u32) -> Option<f32> {
    (level > 0).then(|| (level as f32 / 1000.0).clamp(1.0, 125.0))
}

/// `DisplayConfigHelper.GetSdrWhiteScale`: the SDR white level of the
/// active path whose GDI source is `device_name` (`\\.\DISPLAY1`), 1 when unknown.
pub fn sdr_white_scale(device_name: &str) -> f32 {
    // SAFETY: the display configuration queries with buffers of the sizes Windows gave.
    unsafe {
        let (mut path_count, mut mode_count) = (0u32, 0u32);
        if GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut path_count, &mut mode_count) != ERROR_SUCCESS {
            return 1.0;
        }
        let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); path_count as usize];
        let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); mode_count as usize];
        if QueryDisplayConfig(QDC_ONLY_ACTIVE_PATHS, &mut path_count, paths.as_mut_ptr(), &mut mode_count, modes.as_mut_ptr(), None) != ERROR_SUCCESS {
            return 1.0;
        }
        for path in &paths[..path_count as usize] {
            let mut source = DISPLAYCONFIG_SOURCE_DEVICE_NAME {
                header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
                    r#type: DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
                    size: size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32,
                    adapterId: path.sourceInfo.adapterId,
                    id: path.sourceInfo.id,
                },
                ..Default::default()
            };
            if DisplayConfigGetDeviceInfo(&mut source.header) != 0 {
                continue;
            }
            let n = &source.viewGdiDeviceName;
            let gdi = String::from_utf16_lossy(&n[..n.iter().position(|&c| c == 0).unwrap_or(n.len())]);
            if !crate::devices::equals_ordinal_ignore_case(&gdi, device_name) {
                continue;
            }
            let mut white = DISPLAYCONFIG_SDR_WHITE_LEVEL {
                header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
                    r#type: DISPLAYCONFIG_DEVICE_INFO_GET_SDR_WHITE_LEVEL,
                    size: size_of::<DISPLAYCONFIG_SDR_WHITE_LEVEL>() as u32,
                    adapterId: path.targetInfo.adapterId,
                    id: path.targetInfo.id,
                },
                SDRWhiteLevel: 0,
            };
            if DisplayConfigGetDeviceInfo(&mut white.header) == 0
                && let Some(scale) = white_scale_from_level(white.SDRWhiteLevel)
            {
                return scale;
            }
        }
    }
    1.0
}

const SOURCE: &str = r#"
        Texture2D<float4> source : register(t0);

        cbuffer Constants : register(b0)
        {
            float inversePaperWhite;
            float relativePeak;
            int2 offset;
        };

        struct Vertex { float4 position : SV_Position; };

        Vertex VS(uint id : SV_VertexID)
        {
            float2 uv = float2((id << 1) & 2, id & 2);
            Vertex output;
            output.position = float4(uv.x * 2 - 1, 1 - uv.y * 2, 0, 1);
            return output;
        }

        static const float knee = 0.75;

        float Shoulder(float value)
        {
            if (value <= knee) return value;
            float peak = max(relativePeak, knee + 0.001);
            float normalized = saturate((value - knee) / (peak - knee));
            float strength = (peak - knee) / (1 - knee);
            return knee + (1 - knee) * strength * normalized / (1 + (strength - 1) * normalized);
        }

        float3 ToneMap(float3 color)
        {
            // Everything within SDR white is SDR content, or HDR content that needs no compression.
            float maximum = max(color.r, max(color.g, color.b));
            if (maximum <= 1) return color;
            float luminance = dot(color, float3(0.2126, 0.7152, 0.0722));
            float mapped = Shoulder(luminance);
            if (luminance > 0.00001) color *= mapped / luminance;
            maximum = max(color.r, max(color.g, color.b));
            if (maximum > 1)
            {
                // Bring remaining out-of-range channels in toward gray, keeping luminance and hue direction.
                float gray = saturate(mapped);
                float chroma = maximum > gray ? (1 - gray) / (maximum - gray) : 0;
                color = gray + (color - gray) * chroma;
            }
            return color;
        }

        float3 EncodeSrgb(float3 color)
        {
            float3 low = color * 12.92;
            float3 high = 1.055 * pow(color, 1 / 2.4) - 0.055;
            return lerp(high, low, step(color, 0.0031308));
        }

        float4 PS(Vertex input) : SV_Target
        {
            float3 color = source.Load(int3(int2(input.position.xy) - offset, 0)).rgb * inversePaperWhite;
            // scRGB stores colors outside the sRGB gamut as negative values; NaN also becomes 0 here.
            color = ToneMap(max(color, 0));
            return float4(EncodeSrgb(saturate(color)), 1);
        }
        "#;

/// The shader's constant buffer.
#[repr(C)]
#[derive(Clone, Copy)]
struct Constants {
    inverse_paper_white: f32,
    relative_peak: f32,
    offset_x: i32,
    offset_y: i32,
}

/// `HdrToneMapShader`.
pub struct HdrToneMapShader {
    multithread: ID3D11Multithread,
    vertex: ID3D11VertexShader,
    pixel: ID3D11PixelShader,
    constants: ID3D11Buffer,
    rasterizer: ID3D11RasterizerState,
}

// SAFETY: used by the recording worker only (the device is multithread protected).
unsafe impl Send for HdrToneMapShader {}

impl HdrToneMapShader {
    pub fn new(graphics: &GraphicsDevice) -> Result<Self, String> {
        let d = &graphics.device;
        let vs = super::compile_shader(SOURCE, "HDR tone mapping", "VS", "vs_5_0")?;
        let ps = super::compile_shader(SOURCE, "HDR tone mapping", "PS", "ps_5_0")?;
        // SAFETY: D3D11 object creation with valid descriptions and bytecode.
        unsafe {
            let multithread: ID3D11Multithread = graphics.context.cast().map_err(err)?;
            let (mut vertex, mut pixel, mut constants, mut rasterizer) = (None, None, None, None);
            d.CreateVertexShader(&vs, None, Some(&mut vertex)).map_err(err)?;
            d.CreatePixelShader(&ps, None, Some(&mut pixel)).map_err(err)?;
            // Vortice's BufferDescription(size, bindFlags): default usage, no CPU access.
            let desc = D3D11_BUFFER_DESC {
                ByteWidth: size_of::<Constants>() as u32,
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
                CPUAccessFlags: 0,
                MiscFlags: 0,
                StructureByteStride: 0,
            };
            d.CreateBuffer(&desc, None, Some(&mut constants)).map_err(err)?;
            // Vortice's RasterizerDescription(cull, fill): depth clip on, the rest off.
            let raster = D3D11_RASTERIZER_DESC {
                FillMode: D3D11_FILL_SOLID,
                CullMode: D3D11_CULL_NONE,
                FrontCounterClockwise: false.into(),
                DepthBias: 0,
                DepthBiasClamp: 0.0,
                SlopeScaledDepthBias: 0.0,
                DepthClipEnable: true.into(),
                ScissorEnable: false.into(),
                MultisampleEnable: false.into(),
                AntialiasedLineEnable: false.into(),
            };
            d.CreateRasterizerState(&raster, Some(&mut rasterizer)).map_err(err)?;
            Ok(Self {
                multithread,
                vertex: vertex.ok_or("no vertex shader")?,
                pixel: pixel.ok_or("no pixel shader")?,
                constants: constants.ok_or("no constant buffer")?,
                rasterizer: rasterizer.ok_or("no rasterizer")?,
            })
        }
    }

    /// `Draw`: `source` (sized to `destination`) tone mapped into the BGRA `target`.
    pub fn draw(&self, graphics: &GraphicsDevice, target: &ID3D11RenderTargetView, source: &ID3D11ShaderResourceView, destination: Rect, color: MonitorColorInfo) {
        let c = &graphics.context;
        let values = Constants {
            inverse_paper_white: 1.0 / color.sdr_white_scale,
            relative_peak: color.relative_peak(),
            offset_x: destination.x,
            offset_y: destination.y,
        };
        // SAFETY: the device's immediate context, guarded by its multithread lock for the
        // sequence (Media Foundation shares the device); bindings are released before leaving.
        unsafe {
            self.multithread.Enter();
            c.UpdateSubresource(&self.constants, 0, None, (&values as *const Constants).cast(), 0, 0);
            c.IASetInputLayout(None);
            c.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            c.VSSetShader(&self.vertex, None);
            c.PSSetShader(&self.pixel, None);
            c.PSSetConstantBuffers(0, Some(&[Some(self.constants.clone())]));
            c.PSSetShaderResources(0, Some(&[Some(source.clone())]));
            c.RSSetState(&self.rasterizer);
            c.RSSetViewports(Some(&[D3D11_VIEWPORT {
                TopLeftX: destination.x as f32,
                TopLeftY: destination.y as f32,
                Width: destination.width as f32,
                Height: destination.height as f32,
                MinDepth: 0.0,
                MaxDepth: 1.0,
            }]));
            c.OMSetBlendState(None, None, 0xFFFF_FFFF);
            c.OMSetRenderTargets(Some(&[Some(target.clone())]), None);
            c.Draw(3, 0);
            // Release bindings before the video processor or encoder reads the canvas.
            c.PSSetShaderResources(0, Some(&[None]));
            c.OMSetRenderTargets(Some(&[None]), None);
            self.multithread.Leave();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_info_as_sharex() {
        assert_eq!(MonitorColorInfo::SDR.relative_peak(), 1.25);
        let hdr = MonitorColorInfo { is_hdr: true, sdr_white_scale: 2.5, peak_nits: 1000.0 };
        assert_eq!(hdr.relative_peak(), 5.0);
        assert_eq!(MonitorColorInfo { peak_nits: 1e9, ..hdr }.relative_peak(), 125.0);
        assert_eq!(MonitorColorInfo::peak_from(f32::NAN), 1000.0);
        assert_eq!(MonitorColorInfo::peak_from(79.9), 1000.0);
        assert_eq!(MonitorColorInfo::peak_from(600.0), 600.0);
        assert_eq!(white_scale_from_level(0), None);
        assert_eq!(white_scale_from_level(500), Some(1.0));
        assert_eq!(white_scale_from_level(2500), Some(2.5));
        assert_eq!(white_scale_from_level(u32::MAX), Some(125.0));
    }

    /// Reads whatever the machine's displays say, including nothing.
    #[test]
    fn queries_tolerate_any_display() {
        assert!(sdr_white_scale(r"\\.\NODISPLAY") == 1.0);
        let _ = MonitorColorInfo::get(0);
    }
}
