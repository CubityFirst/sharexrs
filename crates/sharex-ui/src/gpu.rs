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
            // As large as the hardware allows, for very wide monitors, but
            // for the bindings: on DX12 that's the size of the one
            // shader-visible descriptor heap, made up front in video memory
            // and mapped into the process (1,000,000 descriptors, 32 MB on
            // NVIDIA). egui binds one per texture, and 65,536 textures (2 MB
            // of descriptors) would be gigabytes of thumbnails.
            required_limits: wgpu::Limits {
                max_non_sampler_bindings: adapter.limits().max_non_sampler_bindings.min(65_536),
                ..adapter.limits()
            },
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
        log_adapter(&adapter, &device);
        Ok(Self { instance, adapter, device, queue })
    }

    /// The largest texture side ShareXrs uploads: the device's, or
    /// `SHAREXRS_MAX_TEXTURE` when that's smaller. Larger images are drawn
    /// as tiles (`crate::tiled`).
    pub fn max_texture_side(&self) -> usize {
        texture_side_limit(self.device.limits().max_texture_dimension_2d) as usize
    }

    /// Tells a context made for this device the largest texture side
    /// before its first frame: egui assumes 2048 until a frame's input says
    /// otherwise, and `load_texture` asserts against it (the region
    /// overlay uploads the frozen screen before its first frame).
    pub fn configure_context(&self, ctx: &egui::Context) {
        let side = self.max_texture_side();
        ctx.input_mut(|i| i.max_texture_side = side);
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

/// `device_max`, or `SHAREXRS_MAX_TEXTURE` when that's smaller (at least
/// 1024: egui's font atlas asserts that much). The cap is for testing what a
/// weak or virtual GPU (2048 on some VMs, remote sessions and GL drivers)
/// does with large images; it caps the textures ShareXrs uploads, not the
/// device, so windows (whose swap chains have the same limit) still open.
pub fn texture_side_limit(device_max: u32) -> u32 {
    static CAP: std::sync::OnceLock<Option<u32>> = std::sync::OnceLock::new();
    match *CAP.get_or_init(|| std::env::var("SHAREXRS_MAX_TEXTURE").ok().and_then(|v| v.trim().parse::<u32>().ok())) {
        Some(cap) => cap.max(1024).min(device_max),
        None => device_max,
    }
}

/// Logs the adapter wgpu picked and its texture limit, once per process
/// (each offscreen render makes a device).
fn log_adapter(adapter: &wgpu::Adapter, device: &wgpu::Device) {
    static LOGGED: std::sync::Once = std::sync::Once::new();
    LOGGED.call_once(|| {
        let info = adapter.get_info();
        let max = device.limits().max_texture_dimension_2d;
        let used = texture_side_limit(max);
        let capped = if used < max { format!(", capped to {used} by SHAREXRS_MAX_TEXTURE") } else { String::new() };
        let driver = [info.driver.as_str(), info.driver_info.as_str()].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(" ");
        eprintln!("Graphics adapter: {} ({:?}, {:?}, driver {driver}), max texture size: {max}{capped}", info.name, info.backend, info.device_type);
    });
}
