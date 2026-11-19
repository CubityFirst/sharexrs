//! `AudioCaptureDevices.cs` and `CameraCaptureDevices.cs`: the devices a
//! native recording can use, as ShareX lists and stores them. An audio
//! device's `id` is its MMDevice endpoint ID (`RecordingOptions
//! .SystemAudioDeviceId`/`MicrophoneDeviceId`, `TaskSettingsCapture
//! .ScreenRecordSystemAudioDeviceId`/`ScreenRecordMicrophoneDeviceId`); a
//! camera's is its Media Foundation symbolic link (`CameraDeviceId`,
//! `ScreenRecordCameraDeviceId`). An empty ID means the default device.
//!
//! The lists are Windows only (MMDevice, Media Foundation); elsewhere they
//! are empty, as the native recorder isn't used there.

/// `AudioCaptureDevice`: a Windows audio endpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioCaptureDevice {
    pub id: String,
    pub name: String,
}

/// `CameraCaptureDevice`: a Windows camera.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CameraCaptureDevice {
    pub id: String,
    pub name: String,
}

/// `GetMicrophones`: active microphones and other audio input endpoints.
pub fn microphones() -> Result<Vec<AudioCaptureDevice>, String> {
    #[cfg(windows)]
    return crate::windows::wasapi::devices(false);
    #[cfg(not(windows))]
    Ok(Vec::new())
}

/// `GetSystemAudioDevices`: active output endpoints, recordable with WASAPI loopback.
pub fn system_audio_devices() -> Result<Vec<AudioCaptureDevice>, String> {
    #[cfg(windows)]
    return crate::windows::wasapi::devices(true);
    #[cfg(not(windows))]
    Ok(Vec::new())
}

/// `GetDefaultSystemAudioDevice`: the default output (multimedia role), without opening a stream.
pub fn default_system_audio_device() -> Option<AudioCaptureDevice> {
    #[cfg(windows)]
    return crate::windows::wasapi::default_device(true);
    #[cfg(not(windows))]
    None
}

/// `GetDefaultMicrophone`: the default input (communications role, else multimedia).
pub fn default_microphone() -> Option<AudioCaptureDevice> {
    #[cfg(windows)]
    return crate::windows::wasapi::default_device(false);
    #[cfg(not(windows))]
    None
}

/// `GetCameras`: Media Foundation's video capture devices.
pub fn cameras() -> Result<Vec<CameraCaptureDevice>, String> {
    #[cfg(windows)]
    return crate::windows::camera_capture::devices();
    #[cfg(not(windows))]
    Ok(Vec::new())
}

/// .NET's `string.Equals(a, b, StringComparison.OrdinalIgnoreCase)`: both
/// strings upper-cased one UTF-16 unit at a time (simple case mapping).
pub fn equals_ordinal_ignore_case(a: &str, b: &str) -> bool {
    fn upper(c: char) -> char {
        let mut u = c.to_uppercase();
        match (u.next(), u.next()) {
            // Only one-to-one mappings: .NET never expands a character (ß stays ß).
            (Some(x), None) => x,
            _ => c,
        }
    }
    a.chars().count() == b.chars().count() && a.chars().zip(b.chars()).all(|(x, y)| x == y || upper(x) == upper(y))
}

/// `CameraCapture`'s device choice: the first camera when no ID is set,
/// else the one whose symbolic link equals it ignoring case.
pub fn select_camera<'a>(links: impl IntoIterator<Item = &'a str>, device_id: Option<&str>) -> Option<usize> {
    let id = device_id.unwrap_or("");
    links.into_iter().position(|link| id.is_empty() || equals_ordinal_ignore_case(link, id))
}

/// The message when [`select_camera`] finds nothing.
pub fn camera_unavailable_message(device_id: Option<&str>) -> &'static str {
    if device_id.unwrap_or("").is_empty() {
        "No camera is available. Connect a camera or turn off the camera overlay."
    } else {
        "The selected camera is unavailable. Select another camera or turn off the camera overlay."
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_choice_as_sharex() {
        let links = [r"\\?\usb#vid_046d&pid_085c&mi_00#7&1a2b#{e5323777-f976-4f5b-9b55-b94699c46e44}\global", r"\\?\swd#vcamdevapi#0"];
        assert_eq!(select_camera(links, None), Some(0));
        assert_eq!(select_camera(links, Some("")), Some(0));
        assert_eq!(select_camera(links, Some(r"\\?\SWD#VCAMDEVAPI#0")), Some(1));
        assert_eq!(select_camera(links, Some(r"\\?\swd#other")), None);
        assert_eq!(select_camera([], None), None);
        assert_eq!(camera_unavailable_message(None), "No camera is available. Connect a camera or turn off the camera overlay.");
        assert_eq!(camera_unavailable_message(Some("x")), "The selected camera is unavailable. Select another camera or turn off the camera overlay.");
    }

    #[test]
    fn ordinal_ignore_case() {
        assert!(equals_ordinal_ignore_case("{0.0.1.00000000}.{ABC}", "{0.0.1.00000000}.{abc}"));
        assert!(!equals_ordinal_ignore_case("a", "ab"));
        assert!(equals_ordinal_ignore_case("é", "É"));
        assert!(!equals_ordinal_ignore_case("ß", "SS"));
    }

    /// Read-only enumeration: whatever the machine has, including nothing.
    #[test]
    fn enumeration_tolerates_any_result() {
        for list in [microphones(), system_audio_devices()] {
            if let Ok(devices) = list {
                assert!(devices.iter().all(|d| !d.id.is_empty()));
            }
        }
        if let Ok(cameras) = cameras() {
            assert!(cameras.iter().all(|c| !c.id.is_empty()));
        }
        let _ = (default_microphone(), default_system_audio_device());
    }
}
