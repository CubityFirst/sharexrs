//! `ShareX.HelpersLib/Native/DisplayConfigHelper.cs`'s `GetSdrWhiteScale`,
//! which ShareX's HDR screenshot correction (`HDRScreenCapture`) and the
//! native recorder's tone mapping (`MonitorColorInfo`) share.

/// `GetSdrWhiteScale`'s value from `DISPLAYCONFIG_SDR_WHITE_LEVEL`
/// (thousandths of 80 nits): `Math.Clamp(level / 1000f, 1f, 125f)`, `None`
/// for 0.
pub fn white_scale_from_level(level: u32) -> Option<f32> {
    (level > 0).then(|| (level as f32 / 1000.0).clamp(1.0, 125.0))
}

/// `DisplayConfigHelper.GetSdrWhiteScale`: the SDR white level ("SDR content
/// brightness") of the active path whose GDI source is `device_name`
/// (`\\.\DISPLAY1`, compared ignoring case), 1 when unknown or off Windows.
#[cfg(windows)]
pub fn sdr_white_scale(device_name: &str) -> f32 {
    use windows_sys::Win32::Devices::Display::{
        DISPLAYCONFIG_DEVICE_INFO_GET_SDR_WHITE_LEVEL, DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME, DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_PATH_INFO,
        DISPLAYCONFIG_SDR_WHITE_LEVEL, DISPLAYCONFIG_SOURCE_DEVICE_NAME, DisplayConfigGetDeviceInfo, GetDisplayConfigBufferSizes, QDC_ONLY_ACTIVE_PATHS,
        QueryDisplayConfig,
    };
    // SAFETY: the display configuration queries with buffers of the sizes
    // Windows gave; the structs are plain data, zero-initialised.
    unsafe {
        let (mut path_count, mut mode_count) = (0u32, 0u32);
        if GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut path_count, &mut mode_count) != 0 {
            return 1.0;
        }
        let mut paths: Vec<DISPLAYCONFIG_PATH_INFO> = vec![std::mem::zeroed(); path_count as usize];
        let mut modes: Vec<DISPLAYCONFIG_MODE_INFO> = vec![std::mem::zeroed(); mode_count as usize];
        if QueryDisplayConfig(QDC_ONLY_ACTIVE_PATHS, &mut path_count, paths.as_mut_ptr(), &mut mode_count, modes.as_mut_ptr(), std::ptr::null_mut()) != 0 {
            return 1.0;
        }
        for path in &paths[..path_count as usize] {
            let mut source: DISPLAYCONFIG_SOURCE_DEVICE_NAME = std::mem::zeroed();
            source.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME;
            source.header.size = size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32;
            source.header.adapterId = path.sourceInfo.adapterId;
            source.header.id = path.sourceInfo.id;
            if DisplayConfigGetDeviceInfo(&mut source.header) != 0 {
                continue;
            }
            let n = &source.viewGdiDeviceName;
            let gdi = String::from_utf16_lossy(&n[..n.iter().position(|&c| c == 0).unwrap_or(n.len())]);
            if !crate::culture::ordinal_ignore_case_eq(&gdi, device_name) {
                continue;
            }
            let mut white: DISPLAYCONFIG_SDR_WHITE_LEVEL = std::mem::zeroed();
            white.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_SDR_WHITE_LEVEL;
            white.header.size = size_of::<DISPLAYCONFIG_SDR_WHITE_LEVEL>() as u32;
            white.header.adapterId = path.targetInfo.adapterId;
            white.header.id = path.targetInfo.id;
            if DisplayConfigGetDeviceInfo(&mut white.header) == 0
                && let Some(scale) = white_scale_from_level(white.SDRWhiteLevel)
            {
                return scale;
            }
        }
    }
    1.0
}

/// `GetSdrWhiteScale` outside Windows: no display configuration, so 1.
#[cfg(not(windows))]
pub fn sdr_white_scale(_device_name: &str) -> f32 {
    1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn white_levels_as_sharex() {
        assert_eq!(white_scale_from_level(0), None);
        assert_eq!(white_scale_from_level(500), Some(1.0));
        assert_eq!(white_scale_from_level(2500), Some(2.5));
        assert_eq!(white_scale_from_level(u32::MAX), Some(125.0));
    }

    /// Reads whatever the machine's displays say, including nothing.
    #[test]
    fn unknown_displays_are_one() {
        assert_eq!(sdr_white_scale(r"\\.\NODISPLAY"), 1.0);
    }
}
