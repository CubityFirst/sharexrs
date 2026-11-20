//! Renders egui UI offscreen to an image, for comparing ShareXrs's windows
//! against screenshots of ShareX's (`sharexrs --render`, and tests) without
//! opening a window.

use egui_wgpu::wgpu;

use crate::gpu::Gpu;

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

thread_local! {
    static POPUP_HOST: std::cell::Cell<Option<egui::Rect>> = const { std::cell::Cell::new(None) };
    static POPUP_SCREENS: std::cell::RefCell<Option<crate::popup::Geometry>> = const { std::cell::RefCell::new(None) };
    static LAST_POPUPS: std::cell::RefCell<Vec<crate::popup::PopupLayer>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Renders from now on place popups as a host with this work area would
/// (`crate::popup::set_host`, in the window's points), for checking where
/// a popup past the window's edge goes; [`last_popups`] says where they
/// went. `None` (the default) keeps them in the window.
pub fn set_popup_host(work_area: Option<egui::Rect>) {
    POPUP_HOST.with(|h| h.set(work_area));
}

/// The screens a popup host has, for `crate::popup::position` (Avalonia
/// places each popup on the screen under its anchor, in whole device
/// pixels): with [`set_popup_host`], as `crate::popup::set_host_screens`.
pub fn set_popup_screens(geometry: Option<crate::popup::Geometry>) {
    POPUP_SCREENS.with(|g| *g.borrow_mut() = geometry);
}

/// The screens set with [`set_popup_screens`].
pub fn popup_screens() -> Option<crate::popup::Geometry> {
    POPUP_SCREENS.with(|g| g.borrow().clone())
}

/// The popups the last frame drew, when a popup host is set.
pub fn last_popups() -> Vec<crate::popup::PopupLayer> {
    LAST_POPUPS.with(|p| p.borrow().clone())
}

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
        crate::install(&ctx);
        ctx.set_theme(if dark { egui::Theme::Dark } else { egui::Theme::Light });
        ctx.set_pixels_per_point(ppp);
        gpu.configure_context(&ctx);
        let renderer = egui_wgpu::Renderer::new(&gpu.device, FORMAT, Default::default());
        Ok(Self { gpu, ctx, renderer, size, ppp, frame: 0, last: None, transparent: false })
    }

    /// Runs one frame with these input events.
    pub fn run(&mut self, events: Vec<egui::Event>, ui: &mut impl FnMut(&mut egui::Ui)) {
        if self.frame == 0 && self.ppp != 1.0 {
            // egui applies `set_pixels_per_point` at the next pass by scaling
            // the previous screen rect (its 10000-point default), so the
            // first pass at 1.25 would lay out on an 8000x8000 screen. An
            // empty pass takes that, and leaves its textures uploaded.
            self.pass(Vec::new(), &mut |_: &mut egui::Ui| {});
        }
        self.frame += 1;
        self.pass(events, ui);
    }

    fn pass(&mut self, events: Vec<egui::Event>, ui: &mut impl FnMut(&mut egui::Ui)) {
        let [w, h] = self.size;
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(w as f32 / self.ppp, h as f32 / self.ppp),
            )),
            time: Some(self.frame as f64 / 60.0),
            max_texture_side: Some(self.gpu.max_texture_side()),
            events,
            ..Default::default()
        };
        let host = POPUP_HOST.with(|h| h.get());
        let screens = popup_screens();
        let mut out = self.ctx.run_ui(input, |root| {
            if host.is_some() {
                crate::popup::set_host_screens(root.ctx(), host, screens.clone());
            }
            ui(root);
            if host.is_some() {
                let popups = crate::popup::take(root.ctx());
                LAST_POPUPS.with(|p| *p.borrow_mut() = popups);
            }
        });
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
