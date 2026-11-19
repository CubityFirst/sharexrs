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
