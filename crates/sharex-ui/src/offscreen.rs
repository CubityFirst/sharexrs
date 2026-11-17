//! Renders egui UI offscreen to an image, for comparing ShareXrs's windows
//! against screenshots of ShareX's (`sharexrs --render`, and tests) without
//! opening a window.

use egui_wgpu::wgpu;

use crate::gpu::Gpu;

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// An egui context drawn into a texture frame after frame, for scripting
/// input (clicks, hovers) and saving what a frame showed.
pub struct Offscreen {
    gpu: Gpu,
    pub ctx: egui::Context,
    renderer: egui_wgpu::Renderer,
    size: [u32; 2],
    ppp: f32,
    frame: u32,
    last: Option<egui::FullOutput>,
    /// Draw on transparency rather than the panel colour (for icons).
    pub transparent: bool,
}

impl Offscreen {
    /// `size` in physical pixels, at `ppp` pixels per point.
    pub fn new(size: [u32; 2], ppp: f32, dark: bool) -> Result<Self, String> {
        let gpu = Gpu::new()?;
        let ctx = egui::Context::default();
        crate::install(&ctx, 100.0);
        ctx.set_theme(if dark { egui::Theme::Dark } else { egui::Theme::Light });
        ctx.set_pixels_per_point(ppp);
        let renderer = egui_wgpu::Renderer::new(&gpu.device, FORMAT, Default::default());
        Ok(Self { gpu, ctx, renderer, size, ppp, frame: 0, last: None, transparent: false })
    }

    /// Runs one frame with these input events.
    pub fn run(&mut self, events: Vec<egui::Event>, ui: &mut impl FnMut(&mut egui::Ui)) {
        let [w, h] = self.size;
        self.frame += 1;
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(w as f32 / self.ppp, h as f32 / self.ppp),
            )),
            time: Some(self.frame as f64 / 60.0),
            events,
            ..Default::default()
        };
        let mut out = self.ctx.run_ui(input, ui);
        let mut textures = std::mem::take(&mut out.textures_delta);
        for (id, deltas) in &textures.set {
            for delta in deltas {
                self.renderer.update_texture(&self.gpu.device, &self.gpu.queue, *id, delta);
            }
        }
        for id in &textures.free {
            self.renderer.free_texture(id);
        }
        textures.clear();
        self.last = Some(out);
    }

    /// Runs `ui` for a few frames so layout and animations settle, then
    /// draws the last one.
    pub fn render(&mut self, mut ui: impl FnMut(&mut egui::Ui)) -> image::RgbaImage {
        for _ in 0..6 {
            self.run(Vec::new(), &mut ui);
        }
        self.draw()
    }

    /// Draws the last frame.
    pub fn draw(&mut self) -> image::RgbaImage {
        let [w, h] = self.size;
        let output = self.last.take().expect("ran at least once");
        let prims = self.ctx.tessellate(output.shapes.clone(), output.pixels_per_point);
        self.last = Some(output);
        let gpu = &self.gpu;
        let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = target.create_view(&Default::default());
        let screen = egui_wgpu::ScreenDescriptor { size_in_pixels: [w, h], pixels_per_point: self.ppp };
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        let cmds = self.renderer.update_buffers(&gpu.device, &gpu.queue, &mut encoder, &prims, &screen);
        let clear = egui::Rgba::from(self.ctx.global_style().visuals.panel_fill);
        {
            let mut pass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(if self.transparent {
                                wgpu::Color::TRANSPARENT
                            } else {
                                wgpu::Color { r: clear.r() as f64, g: clear.g() as f64, b: clear.b() as f64, a: 1.0 }
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    ..Default::default()
                })
                .forget_lifetime();
            self.renderer.render(&mut pass, &prims, &screen);
        }
        let row = (w * 4).div_ceil(256) * 256;
        let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (row * h) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            target.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: None },
            },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        gpu.queue.submit(cmds.into_iter().chain([encoder.finish()]));
        buffer.slice(..).map_async(wgpu::MapMode::Read, |r| r.expect("map"));
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).expect("poll");
        let data = buffer.slice(..).get_mapped_range().expect("mapped");
        let mut img = image::RgbaImage::new(w, h);
        for y in 0..h {
            let src = &data[(y * row) as usize..(y * row + w * 4) as usize];
            img.as_mut()[(y * w * 4) as usize..((y + 1) * w * 4) as usize].copy_from_slice(src);
        }
        if self.transparent {
            // egui blends in premultiplied alpha; PNGs are straight.
            for p in img.pixels_mut() {
                let a = p[3] as u32;
                if a > 0 && a < 255 {
                    for c in 0..3 {
                        p[c] = ((p[c] as u32 * 255 + a / 2) / a).min(255) as u8;
                    }
                }
            }
        }
        img
    }
}
