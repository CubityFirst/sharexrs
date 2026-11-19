//! Windows' display queries ShareX makes itself (read-only): the monitors'
//! device names (`NativeMethods.GetMonitorDeviceName`, `MONITORINFOEX.szDevice`)
//! and what display diagnostics reports (`DisplayDiagnosticsNative`).
//! Elsewhere these return nothing, as ShareX's do off Windows.

/// A monitor's bounds (desktop pixels) and its GDI device name
/// (`\\.\DISPLAY1`).
#[derive(Clone, Debug, PartialEq)]
pub struct MonitorName {
    pub bounds: sharex_capture::Rect,
    pub device: String,
}

/// Every monitor's device name (`EnumDisplayMonitors`, `GetMonitorInfo`).
#[cfg(windows)]
pub fn monitor_names() -> Vec<MonitorName> {
    use windows_sys::Win32::Foundation::{LPARAM, RECT};
    use windows_sys::Win32::Graphics::Gdi::{EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFOEXW};

    unsafe extern "system" fn each(monitor: HMONITOR, _dc: HDC, _rect: *mut RECT, data: LPARAM) -> i32 {
        // SAFETY: `data` is the Vec below, alive for the enumeration.
        let out = unsafe { &mut *(data as *mut Vec<MonitorName>) };
        let mut info: MONITORINFOEXW = unsafe { std::mem::zeroed() };
        info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        // SAFETY: a valid monitor handle and a sized MONITORINFOEXW.
        if unsafe { GetMonitorInfoW(monitor, &mut info as *mut MONITORINFOEXW as *mut _) } != 0 {
            let r = info.monitorInfo.rcMonitor;
            let len = info.szDevice.iter().position(|c| *c == 0).unwrap_or(info.szDevice.len());
            out.push(MonitorName {
                bounds: sharex_capture::Rect::new(r.left, r.top, r.right - r.left, r.bottom - r.top),
                device: String::from_utf16_lossy(&info.szDevice[..len]),
            });
        }
        1
    }

    let mut out: Vec<MonitorName> = Vec::new();
    // SAFETY: the callback only touches `out` during the call.
    unsafe {
        EnumDisplayMonitors(std::ptr::null_mut(), std::ptr::null(), Some(each), &mut out as *mut Vec<MonitorName> as LPARAM);
    }
    out
}

#[cfg(not(windows))]
pub fn monitor_names() -> Vec<MonitorName> {
    Vec::new()
}

/// `Screens.All.FirstOrDefault(x => string.Equals(GetDeviceName(x),
/// deviceName, OrdinalIgnoreCase))`: the screen whose monitor has that
/// device name.
pub fn screen_by_device_name<'a>(screens: &'a [sharex_capture::Screen], names: &[MonitorName], device: &str) -> Option<&'a sharex_capture::Screen> {
    if device.is_empty() {
        return None;
    }
    let bounds = names.iter().find(|n| n.device.eq_ignore_ascii_case(device))?.bounds;
    screens.iter().find(|s| s.bounds == bounds)
}

/// `DisplayDiagnosticsNative.DisplayMode`.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct DisplayMode {
    pub width: i32,
    pub height: i32,
    pub position: (i32, i32),
    pub bits_per_pixel: i32,
    pub frequency: i32,
    /// `DMDO_DEFAULT`, `DMDO_90`, `DMDO_180`, `DMDO_270` as 0 to 3.
    pub orientation: i32,
}

/// `DisplayDiagnosticsNative.DisplayDevice`.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct DisplayDevice {
    pub device_name: String,
    pub device_string: String,
    pub device_key: String,
    pub device_id: String,
}

/// `DisplayTargetInfo` (`DisplayConfigHelper.GetDisplayTargets`).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct DisplayTarget {
    pub source_device_name: String,
    pub friendly_name: String,
    pub output_technology: String,
    pub refresh_rate: f64,
    pub advanced_color_supported: bool,
    pub advanced_color_enabled: bool,
    pub bits_per_color_channel: i32,
    pub sdr_white_level_nits: u32,
}

/// One monitor as display diagnostics probes it (`MonitorData`).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct NativeMonitor {
    pub handle: isize,
    pub device_name: String,
    pub primary: bool,
    pub bounds: sharex_capture::Rect,
    pub working_area: sharex_capture::Rect,
    pub effective_dpi: Option<(u32, u32)>,
    pub angular_dpi: Option<(u32, u32)>,
    pub raw_dpi: Option<(u32, u32)>,
    pub windows_scale: Option<i32>,
    pub mode: Option<DisplayMode>,
    pub adapter: Option<DisplayDevice>,
    pub monitor_device: Option<DisplayDevice>,
    pub target: Option<DisplayTarget>,
}

/// Everything display diagnostics asks Windows, at once (so the report
/// can be built, and tested, from it).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Snapshot {
    /// `GetProcessDpiAwareness` (thread context, then the process).
    pub dpi_awareness: String,
    pub system_dpi: Option<u32>,
    /// `AppCompatFlags\Layers` for the program, per user and machine.
    pub compat_user: Option<String>,
    pub compat_machine: Option<String>,
    /// `CurrentVersion`: `ProductName` (Windows 11 fixed), the version, the build.
    pub edition: String,
    pub version: String,
    pub build: String,
    pub elevated: bool,
    pub gdi_virtual_screen: sharex_capture::Rect,
    pub gdi_monitor_count: i32,
    pub same_display_format: bool,
    pub monitors: Vec<NativeMonitor>,
    /// The main taskbar: edge (0 left, 1 top, 2 right, 3 bottom), bounds, auto-hide.
    pub taskbar: Option<(i32, sharex_capture::Rect, bool)>,
    pub secondary_taskbars: Vec<sharex_capture::Rect>,
    /// The adapters attached to the desktop and their driver ("version (date)").
    pub adapters: Vec<(DisplayDevice, String)>,
    pub cursor: Option<(i32, i32)>,
    pub physical_cursor: Option<(i32, i32)>,
    /// `MonitorFromPoint(cursor, MONITOR_DEFAULTTONULL)`.
    pub cursor_monitor: Option<isize>,
    /// This window's `GetWindowRect` and `GetDpiForWindow`.
    pub window_bounds: Option<sharex_capture::Rect>,
    pub window_dpi: Option<u32>,
}

/// `DisplayConfigHelper.GetOutputTechnologyName`.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn output_technology_name(value: u32) -> String {
    match value {
        0xFFFF_FFFF => "Other".into(),
        0 => "VGA".into(),
        1 => "S-Video".into(),
        2 => "Composite video".into(),
        3 => "Component video".into(),
        4 => "DVI".into(),
        5 => "HDMI".into(),
        6 => "LVDS".into(),
        8 => "D-Jpn".into(),
        9 => "SDI".into(),
        10 => "DisplayPort".into(),
        11 => "Embedded DisplayPort".into(),
        12 => "UDI".into(),
        13 => "Embedded UDI".into(),
        14 => "SDTV dongle".into(),
        15 => "Miracast".into(),
        16 => "Indirect (wired)".into(),
        17 => "Indirect (virtual)".into(),
        18 => "DisplayPort (USB tunnel)".into(),
        0x8000_0000 => "Internal".into(),
        v => format!("Unknown ({v})"),
    }
}

/// Windows 11 still says "Windows 10" in `ProductName`.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn windows_edition(product_name: &str, build: &str) -> String {
    match build.parse::<u32>() {
        Ok(b) if b >= 22000 => product_name.replace("Windows 10", "Windows 11"),
        _ => product_name.to_owned(),
    }
}

#[cfg(windows)]
mod probe {
    use super::*;
    use windows_sys::Win32::Foundation::{HWND, POINT, RECT};
    use windows_sys::Win32::Graphics::Gdi as gdi;
    use windows_sys::Win32::UI::HiDpi as dpi;
    use windows_sys::Win32::UI::WindowsAndMessaging as wm;

    fn rect(r: RECT) -> sharex_capture::Rect {
        sharex_capture::Rect::new(r.left, r.top, r.right - r.left, r.bottom - r.top)
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn string(buf: &[u16]) -> String {
        let len = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
        String::from_utf16_lossy(&buf[..len])
    }

    /// `RegGetValue` of a string (`REG_SZ`), read-only.
    pub(super) fn reg_string(root: windows_sys::Win32::System::Registry::HKEY, path: &str, name: &str) -> Option<String> {
        use windows_sys::Win32::System::Registry::{RRF_RT_REG_SZ, RegGetValueW};
        let (p, n) = (wide(path), wide(name));
        let mut size = 0u32;
        // SAFETY: sizes the value first, then reads it into a buffer that size.
        unsafe {
            if RegGetValueW(root, p.as_ptr(), n.as_ptr(), RRF_RT_REG_SZ, std::ptr::null_mut(), std::ptr::null_mut(), &mut size) != 0 {
                return None;
            }
            let mut buf = vec![0u16; (size as usize).div_ceil(2).max(1)];
            if RegGetValueW(root, p.as_ptr(), n.as_ptr(), RRF_RT_REG_SZ, std::ptr::null_mut(), buf.as_mut_ptr().cast(), &mut size) != 0 {
                return None;
            }
            Some(string(&buf))
        }
    }

    /// `RegGetValue` of a `REG_DWORD`.
    fn reg_dword(root: windows_sys::Win32::System::Registry::HKEY, path: &str, name: &str) -> Option<u32> {
        use windows_sys::Win32::System::Registry::{RRF_RT_REG_DWORD, RegGetValueW};
        let (p, n) = (wide(path), wide(name));
        let mut v = 0u32;
        let mut size = 4u32;
        // SAFETY: a DWORD-sized out buffer.
        let ok = unsafe { RegGetValueW(root, p.as_ptr(), n.as_ptr(), RRF_RT_REG_DWORD, std::ptr::null_mut(), (&mut v as *mut u32).cast(), &mut size) };
        (ok == 0).then_some(v)
    }

    fn monitors() -> Vec<(isize, String, bool, sharex_capture::Rect, sharex_capture::Rect)> {
        unsafe extern "system" fn each(monitor: gdi::HMONITOR, _dc: gdi::HDC, _r: *mut RECT, data: isize) -> i32 {
            // SAFETY: `data` is the Vec below, alive for the enumeration.
            let out = unsafe { &mut *(data as *mut Vec<(isize, String, bool, sharex_capture::Rect, sharex_capture::Rect)>) };
            let mut info: gdi::MONITORINFOEXW = unsafe { std::mem::zeroed() };
            info.monitorInfo.cbSize = std::mem::size_of::<gdi::MONITORINFOEXW>() as u32;
            if unsafe { gdi::GetMonitorInfoW(monitor, (&mut info as *mut gdi::MONITORINFOEXW).cast()) } != 0 {
                out.push((
                    monitor as isize,
                    string(&info.szDevice),
                    info.monitorInfo.dwFlags & wm::MONITORINFOF_PRIMARY != 0,
                    rect(info.monitorInfo.rcMonitor),
                    rect(info.monitorInfo.rcWork),
                ));
            }
            1
        }
        let mut out = Vec::new();
        // SAFETY: the callback only touches `out` during the call.
        unsafe { gdi::EnumDisplayMonitors(std::ptr::null_mut(), std::ptr::null(), Some(each), (&mut out as *mut Vec<_>) as isize) };
        out
    }

    fn monitor_dpi(monitor: isize, kind: dpi::MONITOR_DPI_TYPE) -> Option<(u32, u32)> {
        let (mut x, mut y) = (0u32, 0u32);
        // SAFETY: a monitor handle from the enumeration.
        (unsafe { dpi::GetDpiForMonitor(monitor as gdi::HMONITOR, kind, &mut x, &mut y) } == 0).then_some((x, y))
    }

    fn scale_factor(monitor: isize) -> Option<i32> {
        let mut scale = 0;
        // SAFETY: as above.
        (unsafe { windows_sys::Win32::UI::Shell::GetScaleFactorForMonitor(monitor as gdi::HMONITOR, &mut scale) } == 0).then_some(scale)
    }

    fn display_mode(device: &str) -> Option<DisplayMode> {
        let mut mode: gdi::DEVMODEW = unsafe { std::mem::zeroed() };
        mode.dmSize = std::mem::size_of::<gdi::DEVMODEW>() as u16;
        let name = wide(device);
        // SAFETY: a sized DEVMODEW.
        if unsafe { gdi::EnumDisplaySettingsExW(name.as_ptr(), gdi::ENUM_CURRENT_SETTINGS, &mut mode, 0) } == 0 {
            return None;
        }
        // SAFETY: display modes fill the display variant of the union.
        let (pos, orientation) = unsafe { (mode.Anonymous1.Anonymous2.dmPosition, mode.Anonymous1.Anonymous2.dmDisplayOrientation) };
        Some(DisplayMode {
            width: mode.dmPelsWidth as i32,
            height: mode.dmPelsHeight as i32,
            position: (pos.x, pos.y),
            bits_per_pixel: mode.dmBitsPerPel as i32,
            frequency: mode.dmDisplayFrequency as i32,
            orientation: orientation as i32,
        })
    }

    fn device(d: &gdi::DISPLAY_DEVICEW) -> DisplayDevice {
        DisplayDevice { device_name: string(&d.DeviceName), device_string: string(&d.DeviceString), device_key: string(&d.DeviceKey), device_id: string(&d.DeviceID) }
    }

    fn enum_devices(parent: Option<&str>, index: u32) -> Option<gdi::DISPLAY_DEVICEW> {
        let mut d: gdi::DISPLAY_DEVICEW = unsafe { std::mem::zeroed() };
        d.cb = std::mem::size_of::<gdi::DISPLAY_DEVICEW>() as u32;
        let name = parent.map(wide);
        let p = name.as_ref().map_or(std::ptr::null(), |n| n.as_ptr());
        // SAFETY: a sized DISPLAY_DEVICEW.
        (unsafe { gdi::EnumDisplayDevicesW(p, index, &mut d, 0) } != 0).then_some(d)
    }

    /// `GetDisplayDevices`: the adapter behind the GDI source, and its monitor.
    fn display_devices(source: &str) -> (Option<DisplayDevice>, Option<DisplayDevice>) {
        let mut adapter = None;
        let mut i = 0;
        while let Some(d) = enum_devices(None, i) {
            if string(&d.DeviceName).eq_ignore_ascii_case(source) {
                adapter = Some(device(&d));
                break;
            }
            i += 1;
        }
        (adapter, enum_devices(Some(source), 0).map(|d| device(&d)))
    }

    fn attached_adapters() -> Vec<DisplayDevice> {
        let mut out = Vec::new();
        let mut i = 0;
        while let Some(d) = enum_devices(None, i) {
            if d.StateFlags & gdi::DISPLAY_DEVICE_ATTACHED_TO_DESKTOP != 0 {
                out.push(device(&d));
            }
            i += 1;
        }
        out
    }

    /// `GetDriverVersion`: `DriverVersion` and `DriverDate` under the
    /// adapter's `\Registry\Machine\...` key.
    fn driver_version(key: &str) -> String {
        const PREFIX: &str = r"\Registry\Machine\";
        if key.len() < PREFIX.len() || !key[..PREFIX.len()].eq_ignore_ascii_case(PREFIX) {
            return String::new();
        }
        let path = &key[PREFIX.len()..];
        let hklm = windows_sys::Win32::System::Registry::HKEY_LOCAL_MACHINE;
        let version = reg_string(hklm, path, "DriverVersion").unwrap_or_default();
        let date = reg_string(hklm, path, "DriverDate").unwrap_or_default();
        if date.is_empty() { version } else { format!("{version} ({date})") }
    }

    /// `DisplayConfigHelper.GetDisplayTargets`.
    fn display_targets() -> Vec<DisplayTarget> {
        use windows_sys::Win32::Devices::Display as dc;
        let mut out = Vec::new();
        let (mut paths_n, mut modes_n) = (0u32, 0u32);
        // SAFETY: plain Win32 queries into buffers of the sizes asked.
        unsafe {
            if dc::GetDisplayConfigBufferSizes(dc::QDC_ONLY_ACTIVE_PATHS, &mut paths_n, &mut modes_n) != 0 {
                return out;
            }
            let mut paths: Vec<dc::DISPLAYCONFIG_PATH_INFO> = vec![std::mem::zeroed(); paths_n as usize];
            let mut modes: Vec<dc::DISPLAYCONFIG_MODE_INFO> = vec![std::mem::zeroed(); modes_n as usize];
            if dc::QueryDisplayConfig(dc::QDC_ONLY_ACTIVE_PATHS, &mut paths_n, paths.as_mut_ptr(), &mut modes_n, modes.as_mut_ptr(), std::ptr::null_mut()) != 0 {
                return out;
            }
            for path in paths.iter().take(paths_n as usize) {
                let t = &path.targetInfo;
                let mut target = DisplayTarget {
                    output_technology: output_technology_name(t.outputTechnology as u32),
                    refresh_rate: if t.refreshRate.Denominator > 0 { t.refreshRate.Numerator as f64 / t.refreshRate.Denominator as f64 } else { 0.0 },
                    ..Default::default()
                };
                let header = |kind: i32, size: usize, adapter, id| dc::DISPLAYCONFIG_DEVICE_INFO_HEADER { r#type: kind, size: size as u32, adapterId: adapter, id };
                let mut source: dc::DISPLAYCONFIG_SOURCE_DEVICE_NAME = std::mem::zeroed();
                source.header = header(dc::DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME, std::mem::size_of_val(&source), path.sourceInfo.adapterId, path.sourceInfo.id);
                if dc::DisplayConfigGetDeviceInfo(&mut source.header) == 0 {
                    target.source_device_name = string(&source.viewGdiDeviceName);
                }
                let mut name: dc::DISPLAYCONFIG_TARGET_DEVICE_NAME = std::mem::zeroed();
                name.header = header(dc::DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME, std::mem::size_of_val(&name), t.adapterId, t.id);
                if dc::DisplayConfigGetDeviceInfo(&mut name.header) == 0 {
                    target.friendly_name = string(&name.monitorFriendlyDeviceName);
                }
                let mut color: dc::DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO = std::mem::zeroed();
                color.header = header(dc::DISPLAYCONFIG_DEVICE_INFO_GET_ADVANCED_COLOR_INFO, std::mem::size_of_val(&color), t.adapterId, t.id);
                if dc::DisplayConfigGetDeviceInfo(&mut color.header) == 0 {
                    let value = color.Anonymous.value;
                    target.advanced_color_supported = value & 0x1 != 0;
                    target.advanced_color_enabled = value & 0x2 != 0;
                    target.bits_per_color_channel = color.bitsPerColorChannel as i32;
                }
                let mut white: dc::DISPLAYCONFIG_SDR_WHITE_LEVEL = std::mem::zeroed();
                white.header = header(dc::DISPLAYCONFIG_DEVICE_INFO_GET_SDR_WHITE_LEVEL, std::mem::size_of_val(&white), t.adapterId, t.id);
                if dc::DisplayConfigGetDeviceInfo(&mut white.header) == 0 && white.SDRWhiteLevel > 0 {
                    // Thousandths of 80 nits.
                    target.sdr_white_level_nits = white.SDRWhiteLevel * 80 / 1000;
                }
                out.push(target);
            }
        }
        out
    }

    /// `GetProcessDpiAwareness`: the thread's context, then the process's.
    fn dpi_awareness() -> String {
        // SAFETY: plain queries.
        unsafe {
            let ctx = dpi::GetThreadDpiAwarenessContext();
            let mut name = if dpi::AreDpiAwarenessContextsEqual(ctx, dpi::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) != 0 {
                "Per-monitor v2".to_owned()
            } else if dpi::AreDpiAwarenessContextsEqual(ctx, dpi::DPI_AWARENESS_CONTEXT_UNAWARE_GDISCALED) != 0 {
                "Unaware (GDI scaled)".to_owned()
            } else {
                match dpi::GetAwarenessFromDpiAwarenessContext(ctx) {
                    0 => "Unaware",
                    1 => "System aware",
                    2 => "Per-monitor v1",
                    _ => "Unknown",
                }
                .to_owned()
            };
            let mut process = 0;
            if dpi::GetProcessDpiAwareness(std::ptr::null_mut(), &mut process) == 0 {
                let p = match process {
                    0 => "unaware".to_owned(),
                    1 => "system aware".to_owned(),
                    2 => "per-monitor aware".to_owned(),
                    v => v.to_string(),
                };
                name.push_str(&format!(" (process: {p})"));
            }
            name
        }
    }

    fn compat_layers(root: windows_sys::Win32::System::Registry::HKEY) -> Option<String> {
        let exe = std::env::current_exe().ok()?;
        reg_string(root, r"Software\Microsoft\Windows NT\CurrentVersion\AppCompatFlags\Layers", &exe.to_string_lossy())
    }

    /// `GetTaskbar` and `GetSecondaryTaskbars`.
    fn taskbars() -> (Option<(i32, sharex_capture::Rect, bool)>, Vec<sharex_capture::Rect>) {
        use windows_sys::Win32::UI::Shell::{ABM_GETSTATE, ABM_GETTASKBARPOS, APPBARDATA, SHAppBarMessage};
        // SAFETY: sized APPBARDATA structs; window enumeration by class.
        unsafe {
            let mut data: APPBARDATA = std::mem::zeroed();
            data.cbSize = std::mem::size_of::<APPBARDATA>() as u32;
            let main = if SHAppBarMessage(ABM_GETTASKBARPOS, &mut data) != 0 {
                let mut state: APPBARDATA = std::mem::zeroed();
                state.cbSize = std::mem::size_of::<APPBARDATA>() as u32;
                let auto_hide = SHAppBarMessage(ABM_GETSTATE, &mut state) & 1 != 0;
                Some((data.uEdge as i32, rect(data.rc), auto_hide))
            } else {
                None
            };
            let mut secondary = Vec::new();
            let class = wide("Shell_SecondaryTrayWnd");
            let mut w: HWND = std::ptr::null_mut();
            loop {
                w = wm::FindWindowExW(std::ptr::null_mut(), w, class.as_ptr(), std::ptr::null());
                if w.is_null() {
                    break;
                }
                let mut r: RECT = std::mem::zeroed();
                if wm::IsWindowVisible(w) != 0 && wm::GetWindowRect(w, &mut r) != 0 {
                    secondary.push(rect(r));
                }
            }
            (main, secondary)
        }
    }

    pub fn snapshot(window: Option<isize>) -> Snapshot {
        use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
        let targets = display_targets();
        let monitors = monitors()
            .into_iter()
            .map(|(handle, device_name, primary, bounds, working_area)| {
                let (adapter, monitor_device) = display_devices(&device_name);
                NativeMonitor {
                    handle,
                    primary,
                    bounds,
                    working_area,
                    effective_dpi: monitor_dpi(handle, dpi::MDT_EFFECTIVE_DPI),
                    angular_dpi: monitor_dpi(handle, dpi::MDT_ANGULAR_DPI),
                    raw_dpi: monitor_dpi(handle, dpi::MDT_RAW_DPI),
                    windows_scale: scale_factor(handle),
                    mode: display_mode(&device_name),
                    adapter,
                    monitor_device,
                    target: targets.iter().find(|t| t.source_device_name.eq_ignore_ascii_case(&device_name)).cloned(),
                    device_name,
                }
            })
            .collect();
        let cv = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";
        let build = reg_string(HKEY_LOCAL_MACHINE, cv, "CurrentBuild").unwrap_or_default();
        let ubr = reg_dword(HKEY_LOCAL_MACHINE, cv, "UBR");
        let product = reg_string(HKEY_LOCAL_MACHINE, cv, "ProductName").unwrap_or_else(|| "Windows".into());
        let (taskbar, secondary_taskbars) = taskbars();
        let mut seen = Vec::new();
        let adapters = attached_adapters()
            .into_iter()
            .filter(|a| {
                let key = format!("{}|{}", a.device_string, a.device_key).to_lowercase();
                let new = !seen.contains(&key);
                seen.push(key);
                new
            })
            .map(|a| {
                let driver = driver_version(&a.device_key);
                (a, driver)
            })
            .collect();
        // SAFETY: plain queries.
        let (cursor, physical, cursor_monitor, gdi_virtual, count, same) = unsafe {
            let mut p = POINT { x: 0, y: 0 };
            let cursor = (wm::GetCursorPos(&mut p) != 0).then_some((p.x, p.y));
            let mut q = POINT { x: 0, y: 0 };
            let physical = (wm::GetPhysicalCursorPos(&mut q) != 0).then_some((q.x, q.y));
            let cursor_monitor = cursor.map(|(x, y)| gdi::MonitorFromPoint(POINT { x, y }, gdi::MONITOR_DEFAULTTONULL) as isize).filter(|m| *m != 0);
            let gdi_virtual = sharex_capture::Rect::new(
                wm::GetSystemMetrics(wm::SM_XVIRTUALSCREEN),
                wm::GetSystemMetrics(wm::SM_YVIRTUALSCREEN),
                wm::GetSystemMetrics(wm::SM_CXVIRTUALSCREEN),
                wm::GetSystemMetrics(wm::SM_CYVIRTUALSCREEN),
            );
            (cursor, physical, cursor_monitor, gdi_virtual, wm::GetSystemMetrics(wm::SM_CMONITORS), wm::GetSystemMetrics(wm::SM_SAMEDISPLAYFORMAT) != 0)
        };
        let (window_bounds, window_dpi) = match window {
            Some(h) => {
                let hwnd = h as HWND;
                let mut r: RECT = unsafe { std::mem::zeroed() };
                // SAFETY: this window's own handle.
                let bounds = (unsafe { wm::GetWindowRect(hwnd, &mut r) } != 0).then(|| rect(r));
                let d = unsafe { dpi::GetDpiForWindow(hwnd) };
                (bounds, (d > 0).then_some(d))
            }
            None => (None, None),
        };
        Snapshot {
            dpi_awareness: dpi_awareness(),
            // SAFETY: a plain query.
            system_dpi: Some(unsafe { dpi::GetDpiForSystem() }),
            compat_user: compat_layers(HKEY_CURRENT_USER),
            compat_machine: compat_layers(HKEY_LOCAL_MACHINE),
            edition: windows_edition(&product, &build),
            version: reg_string(HKEY_LOCAL_MACHINE, cv, "DisplayVersion").or_else(|| reg_string(HKEY_LOCAL_MACHINE, cv, "ReleaseId")).unwrap_or_else(|| "Unknown".into()),
            build: match ubr {
                Some(u) => format!("{build}.{u}"),
                None => build,
            },
            // `StartupOptions.IsAdmin`.
            // SAFETY: a plain query.
            elevated: unsafe { windows_sys::Win32::UI::Shell::IsUserAnAdmin() } != 0,
            gdi_virtual_screen: gdi_virtual,
            gdi_monitor_count: count,
            same_display_format: same,
            monitors,
            taskbar,
            secondary_taskbars,
            adapters,
            cursor,
            physical_cursor: physical,
            cursor_monitor,
            window_bounds,
            window_dpi,
        }
    }
}

/// Probes Windows for display diagnostics (`window`: this window's handle).
#[cfg(windows)]
pub fn snapshot(window: Option<isize>) -> Option<Snapshot> {
    Some(probe::snapshot(window))
}

/// Nothing to probe off Windows (`CollectAvaloniaOnly`).
#[cfg(not(windows))]
pub fn snapshot(_window: Option<isize>) -> Option<Snapshot> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen(x: i32, primary: bool) -> sharex_capture::Screen {
        let bounds = sharex_capture::Rect::new(x, 0, 1920, 1080);
        sharex_capture::Screen { bounds, work_area: bounds, scale: 1.0, primary }
    }

    #[test]
    fn a_screen_is_found_by_its_device_name_ignoring_case() {
        let screens = [screen(0, true), screen(1920, false)];
        let names = [
            MonitorName { bounds: screens[0].bounds, device: r"\\.\DISPLAY1".into() },
            MonitorName { bounds: screens[1].bounds, device: r"\\.\DISPLAY2".into() },
        ];
        assert_eq!(screen_by_device_name(&screens, &names, r"\\.\display2").map(|s| s.bounds.x), Some(1920));
        assert!(screen_by_device_name(&screens, &names, r"\\.\DISPLAY9").is_none());
        assert!(screen_by_device_name(&screens, &names, "").is_none());
    }

    #[test]
    fn the_probe_answers_whatever_the_hardware() {
        // Read-only queries: on any machine (CI's included) they answer
        // without failing; a headless one may have no monitor at all.
        let snap = snapshot(None);
        assert_eq!(snap.is_some(), cfg!(windows));
        let Some(s) = snap else { return };
        assert!(s.monitors.iter().all(|m| !m.device_name.is_empty()));
        assert!(s.monitors.len() <= 64);
        assert!(!s.dpi_awareness.is_empty());
        assert!(s.adapters.iter().all(|(a, _)| !a.device_name.is_empty()));
        assert_eq!(output_technology_name(10), "DisplayPort");
        assert_eq!(output_technology_name(7), "Unknown (7)");
        assert_eq!(windows_edition("Windows 10 Pro", "26100"), "Windows 11 Pro");
        assert_eq!(windows_edition("Windows 10 Pro", "19045"), "Windows 10 Pro");
    }

    #[test]
    fn every_monitor_has_a_name_here() {
        // Whatever the machine (CI included): names, when there are any,
        // aren't empty, and none outside Windows.
        let names = monitor_names();
        assert!(names.iter().all(|n| !n.device.is_empty()));
        if !cfg!(windows) {
            assert!(names.is_empty());
        }
    }
}
