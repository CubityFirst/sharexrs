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
        let (instance, adapter, device, queue) = open_preferred()?;
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

/// The app's display connection (winit's `OwnedDisplayHandle`), which the
/// GL backend needs at instance creation to present to windows (EGL on
/// Wayland and X11). Set before the first [`Gpu::new`]; offscreen renders
/// go without.
type DisplayFn = Box<dyn Fn() -> Box<dyn wgpu::wgt::WgpuHasDisplayHandle> + Send + Sync>;
static DISPLAY: std::sync::OnceLock<DisplayFn> = std::sync::OnceLock::new();

pub fn set_display<D: wgpu::wgt::WgpuHasDisplayHandle + Clone>(display: D) {
    let _ = DISPLAY.set(Box::new(move || Box::new(display.clone())));
}

/// An instance for one set of backends (`WGPU_BACKEND` overrides them),
/// with the display when there is one (only GL uses it).
fn instance(backends: wgpu::Backends) -> wgpu::Instance {
    let mut defaults = wgpu::InstanceDescriptor::new_without_display_handle();
    if let Some(display) = DISPLAY.get() {
        defaults = defaults.with_display_handle(display());
    }
    let backend_options = wgpu::BackendOptions {
        dx12: wgpu::Dx12BackendOptions {
            presentation_system: wgpu::Dx12SwapchainKind::DxgiFromVisual,
            ..defaults.backend_options.dx12.clone()
        },
        ..defaults.backend_options.clone()
    };
    wgpu::Instance::new(wgpu::InstanceDescriptor { backends, backend_options, ..defaults }.with_env())
}

fn first_adapter(backends: wgpu::Backends) -> Option<(wgpu::Instance, wgpu::Adapter)> {
    let instance = instance(backends);
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).ok()?;
    Some((instance, adapter))
}

type Opened = (wgpu::Instance, wgpu::Adapter, wgpu::Device, wgpu::Queue);

/// The device ShareXrs draws with. One backend at a time: with all of them
/// enabled, wgpu loads every vendor's Vulkan, DX12 and OpenGL drivers just
/// to pick one (100+ MB, snapr). DX12 first on Windows, presenting through
/// DirectComposition: a fullscreen Vulkan window looks like a game, so
/// NVIDIA's overlay hooks in on every capture. Metal on macOS, Vulkan on
/// Linux, GL as the last resort. `WGPU_BACKEND` overrides it.
///
/// Linux: a hardware Vulkan adapter, else a hardware GL adapter that
/// draws (VMs' virgl), else software (llvmpipe). On WSL (`/dev/dxg`) the
/// GL one is tried first: there it is Mesa's D3D12 driver on the Windows
/// GPU, and Vulkan's only conformant adapter is llvmpipe (wgpu, and
/// ShareXrs, leave out the non-conformant Dozen, which draws the same but
/// starts slower), so asking Vulkan first costs ~300 ms for nothing. See
/// docs/PLATFORMS.md.
fn open_preferred() -> Result<Opened, String> {
    let open = |(instance, adapter): (wgpu::Instance, wgpu::Adapter)| {
        open_device(&adapter).map(|(device, queue)| (instance, adapter, device, queue))
    };
    #[cfg(target_os = "linux")]
    if std::env::var_os("WGPU_BACKEND").is_none() {
        // Chosen once per process, one thread at a time: later devices
        // (offscreen renders, tests) open the chosen backend only, so GL
        // isn't set up and torn down again and again (Mesa's EGL crashed
        // under parallel tests that did).
        static CHOSEN: std::sync::Mutex<Option<wgpu::Backends>> = std::sync::Mutex::new(None);
        let mut chosen = CHOSEN.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(backends) = *chosen
            && let Some(Ok(opened)) = first_adapter(backends).map(open)
        {
            return Ok(opened);
        }
        let opened = choose_linux(open)?;
        *chosen = Some(match opened.1.get_info().backend {
            wgpu::Backend::Gl => wgpu::Backends::GL,
            _ => wgpu::Backends::VULKAN,
        });
        return Ok(opened);
    }
    let preferred = if cfg!(windows) {
        [wgpu::Backends::DX12, wgpu::Backends::VULKAN]
    } else {
        [wgpu::Backends::PRIMARY, wgpu::Backends::GL]
    };
    open(preferred.into_iter().find_map(first_adapter).ok_or("no usable graphics adapter")?)
}

/// WSL's paravirtualized GPU.
#[cfg(target_os = "linux")]
fn wsl_gpu() -> bool {
    std::path::Path::new("/dev/dxg").exists()
}

/// On WSL, asks Mesa's GL for the Windows GPU (its D3D12 driver): Arch's
/// Mesa picks it by itself, Ubuntu's draws in software (llvmpipe) unless
/// `GALLIUM_DRIVER=d3d12`. Only when nothing was asked for (`GALLIUM_DRIVER`,
/// `LIBGL_ALWAYS_SOFTWARE`). Where the driver is missing, GL fails and
/// [`Gpu::new`] goes on to Vulkan. Nothing outside Linux.
///
/// # Safety
///
/// Sets an environment variable: call before any other thread starts.
pub unsafe fn prefer_wsl_gpu() {
    #[cfg(target_os = "linux")]
    if wsl_gpu() && std::env::var_os("GALLIUM_DRIVER").is_none() && std::env::var_os("LIBGL_ALWAYS_SOFTWARE").is_none() {
        // SAFETY: the caller has no other threads yet.
        unsafe { std::env::set_var("GALLIUM_DRIVER", "d3d12") };
    }
}

/// Linux's choice (see [`open_preferred`]).
#[cfg(target_os = "linux")]
fn choose_linux(open: impl Fn((wgpu::Instance, wgpu::Adapter)) -> Result<Opened, String>) -> Result<Opened, String> {
    let software = |a: &wgpu::Adapter| a.get_info().device_type == wgpu::DeviceType::Cpu;
    // A hardware GL adapter, if it draws.
    let hardware_gl = || {
        let gl = first_adapter(wgpu::Backends::GL).filter(|(_, a)| !software(a))?;
        let (instance, adapter, device, queue) = open(gl).ok()?;
        if draws(&device, &queue) {
            return Some((instance, adapter, device, queue));
        }
        eprintln!("Graphics adapter {} failed its test; not using it.", adapter.get_info().name);
        None
    };
    let wsl = wsl_gpu();
    if wsl && let Some(opened) = hardware_gl() {
        return Ok(opened);
    }
    let vulkan = first_adapter(wgpu::Backends::VULKAN);
    if let Some(found) = vulkan.as_ref().filter(|(_, a)| !software(a)) {
        return open(found.clone());
    }
    if !wsl && let Some(opened) = hardware_gl() {
        return Ok(opened);
    }
    open(vulkan.or_else(|| first_adapter(wgpu::Backends::GL)).ok_or("no usable graphics adapter")?)
}

fn open_device(adapter: &wgpu::Adapter) -> Result<(wgpu::Device, wgpu::Queue), String> {
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
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
    .map_err(|e| format!("failed to open graphics device: {e}"))
}

/// Whether a device draws: a 4x4 target cleared to a colour and read back.
#[cfg(target_os = "linux")]
fn draws(device: &wgpu::Device, queue: &wgpu::Queue) -> bool {
    const SIDE: u32 = 4;
    const ROW: u32 = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let size = wgpu::Extent3d { width: SIDE, height: SIDE, depth_or_array_layers: 1 };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("adapter test"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("adapter test"),
        size: (ROW * SIDE) as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.2, g: 0.4, b: 0.6, a: 1.0 }),
                store: wgpu::StoreOp::Store,
            },
        })],
        ..Default::default()
    });
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(ROW), rows_per_image: Some(SIDE) },
        },
        size,
    );
    queue.submit([encoder.finish()]);
    let slice = buffer.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    if device.poll(wgpu::PollType::wait_indefinitely()).is_err() {
        return false;
    }
    let Ok(data) = slice.get_mapped_range() else { return false };
    let close = |v: u8, want: u8| v.abs_diff(want) <= 2;
    (0..SIDE as usize).all(|y| {
        (0..SIDE as usize).all(|x| {
            let p = &data[y * ROW as usize + x * 4..][..4];
            close(p[0], 51) && close(p[1], 102) && close(p[2], 153) && p[3] == 255
        })
    })
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
