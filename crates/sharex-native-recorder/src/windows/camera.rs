//! `Video/CameraOverlay.cs` and `Video/CameraCircleShader.cs`: camera frames
//! converted to a small BGRA tile by the GPU's video processor (scaled, the
//! circle's centred square crop), and for a circle, blended over a copy of
//! the screen under it by a pixel shader with an anti-aliased edge.

use windows::Win32::Graphics::Direct3D::D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_NV12, DXGI_FORMAT_YUY2};
use windows::core::Interface;

use super::device::GraphicsDevice;
use super::err;
use super::processor::{color_space, content_desc, input_view, output_view, rect, release, stream};
use crate::camera::{CameraFrame, CameraMode, CameraPixelFormat, overlay_bounds};
use windows::Win32::Media::MediaFoundation::{IMF2DBuffer, IMFDXGIBuffer, IMFSample};
use crate::options::{CameraOverlayShape, Rect, RecordingOptions};

/// `CameraOverlay`.
pub struct CameraOverlay {
    mode: CameraMode,
    format: DXGI_FORMAT,
    row_bytes: usize,
    image_bytes: usize,
    video_context: ID3D11VideoContext,
    video_device: ID3D11VideoDevice,
    enumerator: ID3D11VideoProcessorEnumerator,
    processor: ID3D11VideoProcessor,
    output: ID3D11VideoProcessorOutputView,
    circle: Option<CircleShader>,
    upload: Option<ID3D11Texture2D>,
    upload_bytes: Vec<u8>,
    /// The last camera sample, kept until the next GPU operation so its buffer isn't
    /// returned while the video processor's commands are still being submitted.
    retained: Option<super::camera_capture::CameraSample>,
    frame_number: u32,
    pub texture: ID3D11Texture2D,
    pub bounds: Rect,
    pub has_frame: bool,
}

// SAFETY: used by the recording worker only.
unsafe impl Send for CameraOverlay {}

impl CameraOverlay {
    pub fn new(graphics: &GraphicsDevice, mode: &CameraMode, options: &RecordingOptions, recording_width: i32, recording_height: i32) -> Result<Self, String> {
        let info = &mode.info;
        let format = match mode.format {
            CameraPixelFormat::Nv12 => DXGI_FORMAT_NV12,
            CameraPixelFormat::Yuy2 => DXGI_FORMAT_YUY2,
            CameraPixelFormat::Rgb32 => DXGI_FORMAT_B8G8R8A8_UNORM,
        };
        let bounds = overlay_bounds(options, info, recording_width, recording_height);
        let circular = options.camera_shape == CameraOverlayShape::Circle;
        // SAFETY: D3D11 video calls on objects created here, with valid descriptions.
        unsafe {
            let video_device: ID3D11VideoDevice = graphics.device.cast().map_err(err)?;
            let video_context: ID3D11VideoContext = graphics.context.cast().map_err(err)?;
            let enumerator = video_device
                .CreateVideoProcessorEnumerator(&content_desc(options.camera_frames_per_second, (info.width, info.height), (bounds.width, bounds.height)))
                .map_err(err)?;
            let input_ok = enumerator.CheckVideoProcessorFormat(format).map_err(err)? & D3D11_VIDEO_PROCESSOR_FORMAT_SUPPORT_INPUT.0 as u32 != 0;
            let output_ok =
                enumerator.CheckVideoProcessorFormat(DXGI_FORMAT_B8G8R8A8_UNORM).map_err(err)? & D3D11_VIDEO_PROCESSOR_FORMAT_SUPPORT_OUTPUT.0 as u32 != 0;
            if !input_ok || !output_ok {
                return Err(format!("The GPU cannot process the camera's {} format.", info.pixel_format));
            }
            let processor = video_device.CreateVideoProcessor(&enumerator, 0).map_err(err)?;
            let bind = D3D11_BIND_RENDER_TARGET.0 as u32 | if circular { D3D11_BIND_SHADER_RESOURCE.0 as u32 } else { 0 };
            let texture = graphics.create_texture(bounds.width, bounds.height, DXGI_FORMAT_B8G8R8A8_UNORM, bind)?;
            let output = output_view(&video_device, &texture, &enumerator)?;
            video_context.VideoProcessorSetStreamFrameFormat(&processor, 0, D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE);
            video_context.VideoProcessorSetStreamAutoProcessingMode(&processor, 0, false);
            // Media Foundation and D3D11 assign opposite numeric values to full/limited range.
            video_context.VideoProcessorSetStreamColorSpace(
                &processor,
                0,
                &color_space(0, if mode.yuv_matrix == 1 { 1 } else { 0 }, if mode.nominal_range == 1 { 2 } else { 1 }),
            );
            video_context.VideoProcessorSetOutputColorSpace(&processor, &color_space(0, 0, 2));
            video_context.VideoProcessorSetOutputAlphaFillMode(&processor, D3D11_VIDEO_PROCESSOR_ALPHA_FILL_MODE_OPAQUE, 0);
            let mut source = rect(0, 0, info.width, info.height);
            if circular {
                let subsampled = matches!(mode.format, CameraPixelFormat::Nv12 | CameraPixelFormat::Yuy2);
                let mut side = info.width.min(info.height);
                // Align subsampled camera formats to their chroma grid.
                if subsampled {
                    side &= !1;
                }
                let (mut left, mut top) = ((info.width - side) / 2, (info.height - side) / 2);
                if subsampled {
                    left &= !1;
                }
                if mode.format == CameraPixelFormat::Nv12 {
                    top &= !1;
                }
                source = rect(left, top, left + side, top + side);
            }
            video_context.VideoProcessorSetStreamSourceRect(&processor, 0, true, Some(&source));
            let destination = rect(0, 0, bounds.width, bounds.height);
            video_context.VideoProcessorSetStreamDestRect(&processor, 0, true, Some(&destination));
            video_context.VideoProcessorSetOutputTargetRect(&processor, true, Some(&destination));
            let circle = if circular { Some(CircleShader::new(graphics, &texture)?) } else { None };
            Ok(Self {
                mode: mode.clone(),
                format,
                row_bytes: mode.format.row_bytes(info.width) as usize,
                image_bytes: mode.format.image_bytes(info.width, info.height) as usize,
                video_context,
                video_device,
                enumerator,
                processor,
                output,
                circle,
                upload: None,
                upload_bytes: Vec::new(),
                retained: None,
                frame_number: 0,
                texture,
                bounds,
                has_frame: false,
            })
        }
    }

    /// `GetCompositionTexture`: the tile, or for a circle the tile blended
    /// over the screen under it.
    pub fn get_composition_texture(&mut self, graphics: &GraphicsDevice, canvas: &ID3D11Texture2D) -> Result<ID3D11Texture2D, String> {
        match &self.circle {
            Some(c) => c.compose(graphics, canvas, self.bounds),
            None => Ok(self.texture.clone()),
        }
    }

    /// `Update`: converts a new camera frame into the tile.
    pub fn update(&mut self, graphics: &GraphicsDevice, frame: &CameraFrame) -> Result<(), String> {
        let (texture, subresource) = match &frame.sample {
            Some(sample) => {
                // Takes the sample; the previous one is released after this update.
                let _previous = self.retained.replace(sample.clone());
                self.sample_texture(graphics, &sample.0)?
            }
            None => {
                self.upload_camera_buffer(graphics, frame)?;
                (self.upload.clone().expect("uploaded"), 0)
            }
        };
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        // SAFETY: a live texture.
        unsafe { texture.GetDesc(&mut desc) };
        let mips = desc.MipLevels.max(1);
        let input = input_view(&self.video_device, &texture, &self.enumerator, subresource % mips, subresource / mips)?;
        let mut streams = [stream(&input)];
        // SAFETY: valid processor, output view and stream.
        let r = unsafe { self.video_context.VideoProcessorBlt(&self.processor, &self.output, self.frame_number, &streams) };
        release(&mut streams);
        self.frame_number = self.frame_number.wrapping_add(1);
        r.map_err(err)?;
        self.has_frame = true;
        Ok(())
    }

    /// `Update(IMFSample)`'s texture: the sample's own DXGI texture when the
    /// camera decodes on the recording's device, else its pixels uploaded
    /// (`UploadCameraBuffer`: the 2D buffer's pitch, or the default stride).
    fn sample_texture(&mut self, graphics: &GraphicsDevice, sample: &IMFSample) -> Result<(ID3D11Texture2D, u32), String> {
        // SAFETY: Media Foundation buffer access on a live sample; locked memory is read only
        // between Lock and Unlock, within the lengths and pitches the buffer reports.
        unsafe {
            let buffer = if sample.GetBufferCount().map_err(err)? == 1 { sample.GetBufferByIndex(0) } else { sample.ConvertToContiguousBuffer() }.map_err(err)?;
            if let Ok(dxgi) = buffer.cast::<IMFDXGIBuffer>() {
                let mut raw = std::ptr::null_mut();
                dxgi.GetResource(&ID3D11Texture2D::IID, &mut raw).map_err(err)?;
                let texture = ID3D11Texture2D::from_raw(raw);
                // A camera can choose another adapter; then only its image takes the CPU bridge.
                if texture.GetDevice().is_ok_and(|d| d.as_raw() == graphics.device.as_raw()) {
                    return Ok((texture, dxgi.GetSubresourceIndex().map_err(err)?));
                }
            }
            let (w, h) = (self.mode.info.width, self.mode.info.height);
            if self.upload.is_none() {
                self.upload = Some(graphics.create_texture(w, h, self.format, 0)?);
            }
            if self.upload_bytes.is_empty() {
                self.upload_bytes = vec![0; self.image_bytes];
            }
            if let Ok(two) = buffer.cast::<IMF2DBuffer>() {
                let (mut scanline, mut pitch) = (std::ptr::null_mut(), 0i32);
                two.Lock2D(&mut scanline, &mut pitch).map_err(err)?;
                let r = self.copy_rows_from(scanline, pitch as i64);
                let _ = two.Unlock2D();
                r?;
            } else {
                let (mut data, mut length) = (std::ptr::null_mut(), 0u32);
                buffer.Lock(&mut data, None, Some(&mut length)).map_err(err)?;
                let pitch = self.mode.stride as i64;
                let absolute = pitch.abs();
                let rows = if self.mode.format == CameraPixelFormat::Nv12 { h as i64 * 3 / 2 } else { h as i64 };
                let r = if absolute < self.row_bytes as i64 || absolute * rows > length as i64 {
                    Err("The camera delivered a truncated video frame or invalid stride.".to_string())
                } else {
                    let top = if pitch < 0 { data.add(((h - 1) as i64 * absolute) as usize) } else { data };
                    self.copy_rows_from(top, pitch)
                };
                let _ = buffer.Unlock();
                r?;
            }
            let upload = self.upload.clone().expect("created");
            graphics.upload(&upload, Rect::new(0, 0, w, h), &self.upload_bytes, self.row_bytes);
            Ok((upload, 0))
        }
    }

    /// `CopyRows` from locked memory: `top` is the first row, `pitch` bytes apart.
    ///
    /// # Safety
    /// `top` must hold the image's rows (and NV12's chroma rows) at `pitch`.
    unsafe fn copy_rows_from(&mut self, top: *const u8, pitch: i64) -> Result<(), String> {
        let rb = self.row_bytes;
        if (pitch.unsigned_abs() as usize) < rb || (self.mode.format == CameraPixelFormat::Nv12 && pitch < 0) {
            return Err("The camera delivered an unsupported video stride.".into());
        }
        let h = self.mode.info.height as i64;
        let rows = if self.mode.format == CameraPixelFormat::Nv12 { h + h / 2 } else { h };
        for row in 0..rows {
            // SAFETY: the caller's buffer holds the rows at `pitch` from `top`.
            let src = unsafe { std::slice::from_raw_parts(top.offset((row * pitch) as isize), rb) };
            self.upload_bytes[row as usize * rb..(row as usize + 1) * rb].copy_from_slice(src);
        }
        Ok(())
    }

    /// `UploadCameraBuffer`'s contiguous-buffer path: rows at the default
    /// stride (bottom-up when negative) into an upload texture.
    fn upload_camera_buffer(&mut self, graphics: &GraphicsDevice, frame: &CameraFrame) -> Result<(), String> {
        let (w, h) = (self.mode.info.width, self.mode.info.height);
        if self.upload.is_none() {
            self.upload = Some(graphics.create_texture(w, h, self.format, 0)?);
        }
        if self.upload_bytes.is_empty() {
            self.upload_bytes = vec![0; self.image_bytes];
        }
        let pitch = frame.stride as i64;
        let absolute = pitch.unsigned_abs() as usize;
        let rows = if self.mode.format == CameraPixelFormat::Nv12 { h as usize * 3 / 2 } else { h as usize };
        if absolute < self.row_bytes || absolute * rows > frame.data.len() {
            return Err("The camera delivered a truncated video frame or invalid stride.".into());
        }
        let top = if pitch < 0 { (h as usize - 1) * absolute } else { 0 };
        self.copy_rows(&frame.data, top as i64, pitch)?;
        graphics.upload(self.upload.as_ref().expect("created"), Rect::new(0, 0, w, h), &self.upload_bytes, self.row_bytes);
        Ok(())
    }

    /// `CopyRows`.
    fn copy_rows(&mut self, data: &[u8], top: i64, pitch: i64) -> Result<(), String> {
        let rb = self.row_bytes;
        if (pitch.unsigned_abs() as usize) < rb || (self.mode.format == CameraPixelFormat::Nv12 && pitch < 0) {
            return Err("The camera delivered an unsupported video stride.".into());
        }
        let h = self.mode.info.height as i64;
        for row in 0..h {
            let s = (top + row * pitch) as usize;
            self.upload_bytes[row as usize * rb..(row as usize + 1) * rb].copy_from_slice(&data[s..s + rb]);
        }
        if self.mode.format == CameraPixelFormat::Nv12 {
            for row in 0..h / 2 {
                let s = (top + (h + row) * pitch) as usize;
                let d = (h + row) as usize * rb;
                self.upload_bytes[d..d + rb].copy_from_slice(&data[s..s + rb]);
            }
        }
        Ok(())
    }

    /// `Disable`: the camera stopped; the screen shows again.
    pub fn disable(&mut self) {
        self.has_frame = false;
    }
}

const CIRCLE_SOURCE: &str = r#"
        Texture2D camera : register(t0);
        SamplerState linearClamp : register(s0);

        struct Vertex { float4 position : SV_Position; float2 uv : TEXCOORD0; };

        Vertex VS(uint id : SV_VertexID)
        {
            Vertex output;
            output.uv = float2((id << 1) & 2, id & 2);
            output.position = float4(output.uv.x * 2 - 1, 1 - output.uv.y * 2, 0, 1);
            return output;
        }

        float4 PS(Vertex input) : SV_Target
        {
            float distance = length(input.uv - 0.5);
            float coverage = saturate((0.5 - distance) / max(fwidth(distance), 0.000001) + 0.5);
            clip(coverage - 0.000001);
            return float4(camera.Sample(linearClamp, input.uv).rgb, coverage);
        }
        "#;

fn compile(entry: &str, target: &str) -> Result<Vec<u8>, String> {
    super::compile_shader(CIRCLE_SOURCE, "camera overlay", entry, target)
}

/// `CameraCircleShader`.
struct CircleShader {
    multithread: ID3D11Multithread,
    vertex: ID3D11VertexShader,
    pixel: ID3D11PixelShader,
    camera_view: ID3D11ShaderResourceView,
    target_view: ID3D11RenderTargetView,
    sampler: ID3D11SamplerState,
    blend: ID3D11BlendState,
    rasterizer: ID3D11RasterizerState,
    texture: ID3D11Texture2D,
}

impl CircleShader {
    fn new(graphics: &GraphicsDevice, camera: &ID3D11Texture2D) -> Result<Self, String> {
        let d = &graphics.device;
        let (vs, ps) = (compile("VS", "vs_5_0")?, compile("PS", "ps_5_0")?);
        // SAFETY: D3D11 object creation with valid descriptions and bytecode.
        unsafe {
            let multithread: ID3D11Multithread = graphics.context.cast().map_err(err)?;
            let (mut vertex, mut pixel, mut camera_view, mut target_view, mut sampler, mut blend, mut rasterizer) = (None, None, None, None, None, None, None);
            d.CreateVertexShader(&vs, None, Some(&mut vertex)).map_err(err)?;
            d.CreatePixelShader(&ps, None, Some(&mut pixel)).map_err(err)?;
            d.CreateShaderResourceView(camera, None, Some(&mut camera_view)).map_err(err)?;
            let mut desc = D3D11_TEXTURE2D_DESC::default();
            camera.GetDesc(&mut desc);
            let texture = graphics.create_texture(desc.Width as i32, desc.Height as i32, DXGI_FORMAT_B8G8R8A8_UNORM, D3D11_BIND_RENDER_TARGET.0 as u32)?;
            d.CreateRenderTargetView(&texture, None, Some(&mut target_view)).map_err(err)?;
            // Vortice's SamplerDescription(filter, address): MaxAnisotropy 1, MipLODBias 0,
            // ComparisonFunc Never, BorderColor 0, MinLOD float.MinValue, MaxLOD float.MaxValue.
            let sampler_desc = D3D11_SAMPLER_DESC {
                Filter: D3D11_FILTER_MIN_MAG_MIP_LINEAR,
                AddressU: D3D11_TEXTURE_ADDRESS_CLAMP,
                AddressV: D3D11_TEXTURE_ADDRESS_CLAMP,
                AddressW: D3D11_TEXTURE_ADDRESS_CLAMP,
                MipLODBias: 0.0,
                MaxAnisotropy: 1,
                ComparisonFunc: D3D11_COMPARISON_NEVER,
                BorderColor: [0.0; 4],
                MinLOD: f32::MIN,
                MaxLOD: f32::MAX,
            };
            d.CreateSamplerState(&sampler_desc, Some(&mut sampler)).map_err(err)?;
            // Vortice's BlendDescription(src, dst, srcAlpha, dstAlpha): render target 0 enabled, add ops, all channels.
            let mut blend_desc = D3D11_BLEND_DESC::default();
            blend_desc.RenderTarget[0] = D3D11_RENDER_TARGET_BLEND_DESC {
                BlendEnable: true.into(),
                SrcBlend: D3D11_BLEND_SRC_ALPHA,
                DestBlend: D3D11_BLEND_INV_SRC_ALPHA,
                BlendOp: D3D11_BLEND_OP_ADD,
                SrcBlendAlpha: D3D11_BLEND_ONE,
                DestBlendAlpha: D3D11_BLEND_INV_SRC_ALPHA,
                BlendOpAlpha: D3D11_BLEND_OP_ADD,
                RenderTargetWriteMask: D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8,
            };
            for i in 1..8 {
                blend_desc.RenderTarget[i] = blend_desc.RenderTarget[0];
            }
            d.CreateBlendState(&blend_desc, Some(&mut blend)).map_err(err)?;
            // Vortice's RasterizerDescription(cull, fill): depth clip on, the rest off.
            let raster_desc = D3D11_RASTERIZER_DESC {
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
            d.CreateRasterizerState(&raster_desc, Some(&mut rasterizer)).map_err(err)?;
            Ok(Self {
                multithread,
                vertex: vertex.ok_or("no vertex shader")?,
                pixel: pixel.ok_or("no pixel shader")?,
                camera_view: camera_view.ok_or("no shader view")?,
                target_view: target_view.ok_or("no target view")?,
                sampler: sampler.ok_or("no sampler")?,
                blend: blend.ok_or("no blend state")?,
                rasterizer: rasterizer.ok_or("no rasterizer")?,
                texture,
            })
        }
    }

    /// `Compose`: the screen under the tile, then the circle blended over it.
    fn compose(&self, graphics: &GraphicsDevice, canvas: &ID3D11Texture2D, bounds: Rect) -> Result<ID3D11Texture2D, String> {
        let c = &graphics.context;
        // SAFETY: the device's immediate context, guarded by its multithread lock for the sequence.
        unsafe {
            self.multithread.Enter();
            let b = D3D11_BOX { left: bounds.x as u32, top: bounds.y as u32, front: 0, right: bounds.right() as u32, bottom: bounds.bottom() as u32, back: 1 };
            c.CopySubresourceRegion(&self.texture, 0, 0, 0, 0, canvas, 0, Some(&b));
            c.IASetInputLayout(None);
            c.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            c.VSSetShader(&self.vertex, None);
            c.PSSetShader(&self.pixel, None);
            c.PSSetSamplers(0, Some(&[Some(self.sampler.clone())]));
            c.RSSetState(&self.rasterizer);
            c.RSSetViewports(Some(&[D3D11_VIEWPORT {
                TopLeftX: 0.0,
                TopLeftY: 0.0,
                Width: bounds.width as f32,
                Height: bounds.height as f32,
                MinDepth: 0.0,
                MaxDepth: 1.0,
            }]));
            c.OMSetBlendState(&self.blend, None, 0xFFFF_FFFF);
            c.OMSetRenderTargets(Some(&[Some(self.target_view.clone())]), None);
            c.PSSetShaderResources(0, Some(&[Some(self.camera_view.clone())]));
            c.Draw(3, 0);
            // Release bindings before the next camera conversion or video-processor read.
            c.PSSetShaderResources(0, Some(&[None]));
            c.OMSetRenderTargets(Some(&[None]), None);
            self.multithread.Leave();
        }
        Ok(self.texture.clone())
    }
}
