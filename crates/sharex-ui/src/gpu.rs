//! One wgpu device shared by every window (and by offscreen renders).

use egui_wgpu::wgpu;

/// The first device, for the `memory-profile` report.
#[cfg(feature = "memory-profile")]
pub static PROFILED_DEVICE: std::sync::OnceLock<wgpu::Device> = std::sync::OnceLock::new();

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
            // wgpu's default (`Performance`) reserves its memory in blocks of
            // 128 MB of video memory and 64 MB of upload memory (DX12 and
            // Vulkan), both mapped into the process: 192 MB of GPU memory
            // and private bytes for the ~50 MB of textures a ShareXrs window
            // set uses. `MemoryUsage` starts at 8 and 4 MB and doubles each
            // new block (8, 16, 32...: ~2x what's used). Blocks of 4 MB
            // (gpu-allocator's least) hold the small textures and buffers;
            // anything larger (a frozen screen, its upload) gets a block of
            // its own size, freed with it.
            memory_hints: wgpu::MemoryHints::Manual { suballocated_device_memory_block_size: 4 << 20..4 << 20 },
            ..Default::default()
        }))
        .map_err(|e| format!("failed to open graphics device: {e}"))?;
        #[cfg(feature = "memory-profile")]
        let _ = PROFILED_DEVICE.set(device.clone());
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
