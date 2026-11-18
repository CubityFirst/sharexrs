//! `Video/GpuVideoProcessor.cs`: the BGRA canvas the capture composes into,
//! and its conversion to limited-range BT.709 NV12 (scaled to the encoder's
//! size) by the GPU's video processor, with the camera overlay as a second
//! stream or copied in.

use std::mem::ManuallyDrop;

use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_NV12, DXGI_RATIONAL};
use windows::core::Interface;

use super::camera::CameraOverlay;
use super::device::GraphicsDevice;
use super::err;

/// `VideoProcessorColorSpace` packed as D3D11's bitfield: Usage:1,
/// RGB_Range:1, YCbCr_Matrix:1, YCbCr_xvYCC:1, Nominal_Range:2.
pub(crate) fn color_space(rgb_range: u32, ycbcr_matrix: u32, nominal_range: u32) -> D3D11_VIDEO_PROCESSOR_COLOR_SPACE {
    D3D11_VIDEO_PROCESSOR_COLOR_SPACE { _bitfield: (rgb_range << 1) | (ycbcr_matrix << 2) | (nominal_range << 4) }
}

pub(crate) fn stream(view: &ID3D11VideoProcessorInputView) -> D3D11_VIDEO_PROCESSOR_STREAM {
    D3D11_VIDEO_PROCESSOR_STREAM { Enable: true.into(), pInputSurface: ManuallyDrop::new(Some(view.clone())), ..Default::default() }
}

/// Releases the views a `stream` took.
pub(crate) fn release(streams: &mut [D3D11_VIDEO_PROCESSOR_STREAM]) {
    for s in streams {
        // SAFETY: each was made by `stream` and is dropped once.
        unsafe { ManuallyDrop::drop(&mut s.pInputSurface) };
    }
}

pub(crate) fn content_desc(fps: i32, input: (i32, i32), output: (i32, i32)) -> D3D11_VIDEO_PROCESSOR_CONTENT_DESC {
    D3D11_VIDEO_PROCESSOR_CONTENT_DESC {
        InputFrameFormat: D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE,
        InputFrameRate: DXGI_RATIONAL { Numerator: fps as u32, Denominator: 1 },
        InputWidth: input.0 as u32,
        InputHeight: input.1 as u32,
        OutputFrameRate: DXGI_RATIONAL { Numerator: fps as u32, Denominator: 1 },
        OutputWidth: output.0 as u32,
        OutputHeight: output.1 as u32,
        Usage: D3D11_VIDEO_USAGE_PLAYBACK_NORMAL,
    }
}

pub(crate) fn input_view(
    video: &ID3D11VideoDevice,
    texture: &ID3D11Texture2D,
    enumerator: &ID3D11VideoProcessorEnumerator,
    mip: u32,
    array: u32,
) -> Result<ID3D11VideoProcessorInputView, String> {
    let desc = D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC {
        FourCC: 0,
        ViewDimension: D3D11_VPIV_DIMENSION_TEXTURE2D,
        Anonymous: D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC_0 { Texture2D: D3D11_TEX2D_VPIV { MipSlice: mip, ArraySlice: array } },
    };
    let mut view = None;
    // SAFETY: valid resource, enumerator and description.
    unsafe { video.CreateVideoProcessorInputView(texture, enumerator, &desc, Some(&mut view)) }.map_err(err)?;
    view.ok_or_else(|| "no input view".into())
}

pub(crate) fn output_view(
    video: &ID3D11VideoDevice,
    texture: &ID3D11Texture2D,
    enumerator: &ID3D11VideoProcessorEnumerator,
) -> Result<ID3D11VideoProcessorOutputView, String> {
    let desc = D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC {
        ViewDimension: D3D11_VPOV_DIMENSION_TEXTURE2D,
        Anonymous: D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC_0 { Texture2D: D3D11_TEX2D_VPOV { MipSlice: 0 } },
    };
    let mut view = None;
    // SAFETY: valid resource, enumerator and description.
    unsafe { video.CreateVideoProcessorOutputView(texture, enumerator, &desc, Some(&mut view)) }.map_err(err)?;
    view.ok_or_else(|| "no output view".into())
}

pub(crate) fn rect(l: i32, t: i32, r: i32, b: i32) -> RECT {
    RECT { left: l, top: t, right: r, bottom: b }
}

struct Video {
    device: ID3D11VideoDevice,
    context: ID3D11VideoContext,
    enumerator: ID3D11VideoProcessorEnumerator,
    processor: ID3D11VideoProcessor,
    input: ID3D11VideoProcessorInputView,
}

/// `GpuVideoProcessor`.
pub struct GpuVideoProcessor {
    pub width: i32,
    pub height: i32,
    /// NV12 output size; smaller than the canvas when the encoder can't take the capture size.
    pub output_width: i32,
    pub output_height: i32,
    pub canvas: ID3D11Texture2D,
    canvas_view: ID3D11RenderTargetView,
    video: Option<Video>,
    supports_camera_composition: bool,
    camera_input: Option<ID3D11VideoProcessorInputView>,
    composite: Option<ID3D11Texture2D>,
    composite_input: Option<ID3D11VideoProcessorInputView>,
}

// SAFETY: used by the recording worker only (the device is multithread protected).
unsafe impl Send for GpuVideoProcessor {}

impl GpuVideoProcessor {
    pub fn new(graphics: &GraphicsDevice, width: i32, height: i32, fps: i32, convert_to_nv12: bool, output_width: i32, output_height: i32) -> Result<Self, String> {
        let output_width = if output_width > 0 { output_width } else { width };
        let output_height = if output_height > 0 { output_height } else { height };
        let canvas = graphics.create_texture(width, height, DXGI_FORMAT_B8G8R8A8_UNORM, D3D11_BIND_RENDER_TARGET.0 as u32)?;
        let mut canvas_view = None;
        // SAFETY: a render-target texture.
        unsafe { graphics.device.CreateRenderTargetView(&canvas, None, Some(&mut canvas_view)) }.map_err(err)?;
        let mut p = Self {
            width,
            height,
            output_width,
            output_height,
            canvas,
            canvas_view: canvas_view.ok_or("no render target view")?,
            video: None,
            supports_camera_composition: false,
            camera_input: None,
            composite: None,
            composite_input: None,
        };
        p.clear(graphics);
        // GIF reads the original BGRA pixels without requiring NV12 or a video encoder.
        if !convert_to_nv12 {
            return Ok(p);
        }
        // SAFETY: D3D11 video calls on objects created here, with valid descriptions.
        unsafe {
            let device: ID3D11VideoDevice = graphics.device.cast().map_err(err)?;
            let context: ID3D11VideoContext = graphics.context.cast().map_err(err)?;
            let enumerator = device.CreateVideoProcessorEnumerator(&content_desc(fps, (width, height), (output_width, output_height))).map_err(err)?;
            let support = enumerator.CheckVideoProcessorFormat(DXGI_FORMAT_NV12).map_err(err)?;
            if support & D3D11_VIDEO_PROCESSOR_FORMAT_SUPPORT_OUTPUT.0 as u32 == 0 {
                return Err("The GPU cannot produce NV12 video surfaces.".into());
            }
            let processor = device.CreateVideoProcessor(&enumerator, 0).map_err(err)?;
            let mut caps = D3D11_VIDEO_PROCESSOR_CAPS::default();
            enumerator.GetVideoProcessorCaps(&mut caps).map_err(err)?;
            p.supports_camera_composition = caps.MaxInputStreams >= 2 && caps.MaxStreamStates >= 2;
            let input = input_view(&device, &p.canvas, &enumerator, 0, 0)?;
            context.VideoProcessorSetStreamColorSpace(&processor, 0, &color_space(0, 1, 2));
            context.VideoProcessorSetOutputColorSpace(&processor, &color_space(0, 1, 1));
            context.VideoProcessorSetStreamFrameFormat(&processor, 0, D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE);
            context.VideoProcessorSetStreamAutoProcessingMode(&processor, 0, false);
            let output = rect(0, 0, output_width, output_height);
            context.VideoProcessorSetStreamSourceRect(&processor, 0, true, Some(&rect(0, 0, width, height)));
            context.VideoProcessorSetStreamDestRect(&processor, 0, true, Some(&output));
            context.VideoProcessorSetOutputTargetRect(&processor, true, Some(&output));
            p.video = Some(Video { device, context, enumerator, processor, input });
        }
        Ok(p)
    }

    /// `Clear`: opaque black.
    pub fn clear(&self, graphics: &GraphicsDevice) {
        // SAFETY: the canvas's own view.
        unsafe { graphics.context.ClearRenderTargetView(&self.canvas_view, &[0.0, 0.0, 0.0, 1.0]) };
    }

    /// `CreateOutputView`: an NV12 texture as the conversion's target.
    pub fn create_output_view(&self, texture: &ID3D11Texture2D) -> Result<ID3D11VideoProcessorOutputView, String> {
        let v = self.video.as_ref().ok_or("no video processor")?;
        output_view(&v.device, texture, &v.enumerator)
    }

    /// `GetBgraFrame`: the canvas, or a copy with the camera tile on it.
    pub fn get_bgra_frame(&mut self, graphics: &GraphicsDevice, camera: Option<&mut CameraOverlay>) -> Result<ID3D11Texture2D, String> {
        let Some(camera) = camera.filter(|c| c.has_frame) else { return Ok(self.canvas.clone()) };
        if self.composite.is_none() {
            self.composite = Some(graphics.create_texture(self.width, self.height, DXGI_FORMAT_B8G8R8A8_UNORM, D3D11_BIND_RENDER_TARGET.0 as u32)?);
        }
        let composite = self.composite.clone().expect("created");
        let overlay = camera.get_composition_texture(graphics, &self.canvas)?;
        let b = camera.bounds;
        // SAFETY: same-format textures; the tile lies inside the canvas.
        unsafe {
            graphics.context.CopyResource(&composite, &self.canvas);
            graphics.context.CopySubresourceRegion(&composite, 0, b.x as u32, b.y as u32, 0, &overlay, 0, None);
        }
        Ok(composite)
    }

    /// `Convert`: the canvas (and camera) into `output` as NV12.
    pub fn convert(&mut self, graphics: &GraphicsDevice, output: &ID3D11VideoProcessorOutputView, frame_number: u32, camera: Option<&mut CameraOverlay>) -> Result<(), String> {
        let v = self.video.as_ref().ok_or("no video processor")?;
        let (device, context, enumerator, processor) = (v.device.clone(), v.context.clone(), v.enumerator.clone(), v.processor.clone());
        let mut input = v.input.clone();
        if let Some(camera) = camera.filter(|c| c.has_frame) {
            let overlay = camera.get_composition_texture(graphics, &self.canvas)?;
            let b = camera.bounds;
            if self.supports_camera_composition {
                let attempt = (|| -> Result<(), String> {
                    if self.camera_input.is_none() {
                        let view = input_view(&device, &overlay, &enumerator, 0, 0)?;
                        // SAFETY: stream 1 of a processor that has two.
                        unsafe {
                            context.VideoProcessorSetStreamFrameFormat(&processor, 1, D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE);
                            context.VideoProcessorSetStreamAutoProcessingMode(&processor, 1, false);
                            context.VideoProcessorSetStreamColorSpace(&processor, 1, &color_space(0, 0, 2));
                            context.VideoProcessorSetStreamSourceRect(&processor, 1, true, Some(&rect(0, 0, b.width, b.height)));
                            // Bounds are canvas pixels; scale them with the screen when the output is downscaled.
                            let (ow, oh, w, h) = (self.output_width as i64, self.output_height as i64, self.width as i64, self.height as i64);
                            context.VideoProcessorSetStreamDestRect(
                                &processor,
                                1,
                                true,
                                Some(&rect(
                                    (b.x as i64 * ow / w) as i32,
                                    (b.y as i64 * oh / h) as i32,
                                    (b.right() as i64 * ow / w) as i32,
                                    (b.bottom() as i64 * oh / h) as i32,
                                )),
                            );
                        }
                        self.camera_input = Some(view);
                    }
                    let mut streams = [stream(&input), stream(self.camera_input.as_ref().expect("created"))];
                    // SAFETY: valid processor, output view and streams.
                    let r = unsafe { context.VideoProcessorBlt(&processor, output, frame_number, &streams) };
                    release(&mut streams);
                    r.map_err(err)
                })();
                match attempt {
                    Ok(()) => return Ok(()),
                    Err(_) => {
                        // Some drivers report multiple streams but reject a smaller overlay surface.
                        // Retry with GPU texture copies, then keep that path for the session.
                        self.supports_camera_composition = false;
                        self.camera_input = None;
                    }
                }
            }
            // Devices limited to one input still composite on the GPU. Never modify the
            // persistent screen canvas: a stopped camera must reveal the original screen.
            self.ensure_composite(graphics, &device, &enumerator)?;
            let composite = self.composite.as_ref().expect("created");
            // SAFETY: same-format textures; the tile lies inside the canvas.
            unsafe {
                graphics.context.CopyResource(composite, &self.canvas);
                graphics.context.CopySubresourceRegion(composite, 0, b.x as u32, b.y as u32, 0, &overlay, 0, None);
            }
            input = self.composite_input.clone().expect("created");
        } else if let (Some(composite), Some(view)) = (&self.composite, &self.composite_input) {
            // Keep the fallback input surface stable after camera loss.
            // SAFETY: same-format textures.
            unsafe { graphics.context.CopyResource(composite, &self.canvas) };
            input = view.clone();
        }
        let mut streams = [stream(&input)];
        // SAFETY: valid processor, output view and stream.
        let r = unsafe { context.VideoProcessorBlt(&processor, output, frame_number, &streams) };
        release(&mut streams);
        r.map_err(err)
    }

    fn ensure_composite(&mut self, graphics: &GraphicsDevice, device: &ID3D11VideoDevice, enumerator: &ID3D11VideoProcessorEnumerator) -> Result<(), String> {
        if self.composite.is_none() {
            let texture = graphics.create_texture(self.width, self.height, DXGI_FORMAT_B8G8R8A8_UNORM, D3D11_BIND_RENDER_TARGET.0 as u32)?;
            self.composite_input = Some(input_view(device, &texture, enumerator, 0, 0)?);
            self.composite = Some(texture);
        } else if self.composite_input.is_none() {
            let texture = self.composite.clone().expect("checked");
            self.composite_input = Some(input_view(device, &texture, enumerator, 0, 0)?);
        }
        Ok(())
    }
}
