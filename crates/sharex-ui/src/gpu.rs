//! One wgpu device shared by every window (and by offscreen renders).

use egui_wgpu::wgpu;

pub struct Gpu {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl Gpu {
    pub fn new() -> Result<Self, String> {
        // One backend at a time: with all of them enabled, wgpu loads every
        // vendor's Vulkan, DX12 and OpenGL drivers just to pick one (100+
        // MB, snapr). DX12 first on Windows, presenting through
        // DirectComposition: a fullscreen Vulkan window looks like a game, so
        // NVIDIA's overlay hooks in on every capture. Metal on macOS, Vulkan
        // on Linux, GL as the last resort. `WGPU_BACKEND` overrides it.
        let preferred = if cfg!(windows) {
            [wgpu::Backends::DX12, wgpu::Backends::VULKAN]
        } else {
            [wgpu::Backends::PRIMARY, wgpu::Backends::GL]
        };
        let (instance, adapter) = preferred
            .into_iter()
            .find_map(|backends| {
                let defaults = wgpu::InstanceDescriptor::new_without_display_handle();
                let instance = wgpu::Instance::new(
                    wgpu::InstanceDescriptor {
                        backends,
                        backend_options: wgpu::BackendOptions {
                            dx12: wgpu::Dx12BackendOptions {
                                presentation_system: wgpu::Dx12SwapchainKind::DxgiFromVisual,
                                ..defaults.backend_options.dx12.clone()
                            },
                            ..defaults.backend_options.clone()
                        },
                        ..defaults
                    }
                    .with_env(),
                );
                let adapter =
                    pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).ok()?;
                Some((instance, adapter))
            })
            .ok_or("no usable graphics adapter")?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("sharexrs"),
            // As large as the hardware allows, for very wide monitors.
            required_limits: adapter.limits(),
            ..Default::default()
        }))
        .map_err(|e| format!("failed to open graphics device: {e}"))?;
        Ok(Self { instance, adapter, device, queue })
    }

    /// Lets egui-wgpu render a window on this same device.
    pub fn egui_setup(&self) -> egui_wgpu::WgpuSetup {
        egui_wgpu::WgpuSetup::Existing(egui_wgpu::WgpuSetupExisting {
            instance: self.instance.clone(),
            adapter: self.adapter.clone(),
            device: self.device.clone(),
            queue: self.queue.clone(),
        })
    }
}
