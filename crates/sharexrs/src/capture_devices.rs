//! The native recorder's device lists in Task settings › Screen recorder:
//! `ShareX.ScreenRecordingLib/AudioCaptureDevices.cs` (`GetSystemAudioDevices`,
//! `GetMicrophones`: the active MMDevice render and capture endpoints, id and
//! friendly name) and `CameraCaptureDevices.cs` (`GetCameras`: Media
//! Foundation's video capture sources, symbolic link and friendly name), and
//! `TaskSettingsPageBuilder.CaptureDeviceCombo`'s choices.
//!
//! Enumeration only reads: nothing is opened, started or recorded. It sits
//! behind [`CaptureDevices`] so tests and renders use fakes.

/// `AudioCaptureDevice` / `CameraCaptureDevice`: an id to save and a name to show.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaptureDevice {
    pub id: String,
    pub name: String,
}

impl CaptureDevice {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self { id: id.into(), name: name.into() }
    }
}

/// Which list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceKind {
    SystemAudio,
    Microphone,
    Camera,
}

/// Where the lists come from.
pub trait CaptureDevices {
    /// The devices of `kind`, or why they couldn't be listed (ShareX logs
    /// the exception and lists none).
    fn list(&self, kind: DeviceKind) -> Result<Vec<CaptureDevice>, String>;
}

/// The system's devices (Windows; none elsewhere, where ShareX's native
/// recorder doesn't exist).
pub struct SystemDevices;

impl CaptureDevices for SystemDevices {
    fn list(&self, kind: DeviceKind) -> Result<Vec<CaptureDevice>, String> {
        #[cfg(windows)]
        {
            match kind {
                DeviceKind::SystemAudio => windows_impl::audio_endpoints(true),
                DeviceKind::Microphone => windows_impl::audio_endpoints(false),
                DeviceKind::Camera => windows_impl::cameras(),
            }
        }
        #[cfg(not(windows))]
        {
            let _ = kind;
            Ok(Vec::new())
        }
    }
}

/// Fixed lists (tests and renders).
#[derive(Clone, Debug, Default)]
pub struct FakeDevices {
    pub system_audio: Vec<CaptureDevice>,
    pub microphones: Vec<CaptureDevice>,
    pub cameras: Vec<CaptureDevice>,
}

impl CaptureDevices for FakeDevices {
    fn list(&self, kind: DeviceKind) -> Result<Vec<CaptureDevice>, String> {
        Ok(match kind {
            DeviceKind::SystemAudio => self.system_audio.clone(),
            DeviceKind::Microphone => self.microphones.clone(),
            DeviceKind::Camera => self.cameras.clone(),
        })
    }
}

/// `CaptureDeviceCombo.Refresh`: the default entry (`""`), the devices by
/// name (`OrderBy(Name, CurrentCultureIgnoreCase)`); with no devices and an
/// `empty_label` (the camera's), the default entry says so; a saved id
/// that isn't listed is added with `unavailable_label` so it stays selected.
/// Returns the choices (id, title) and the selected one.
pub fn combo_choices(devices: &Result<Vec<CaptureDevice>, String>, selected_id: &str, default_label: &str, unavailable_label: &str, empty_label: Option<&str>) -> (Vec<CaptureDevice>, usize) {
    let mut choices = vec![CaptureDevice::new("", default_label)];
    match devices {
        Ok(available) => {
            let mut sorted = available.clone();
            sorted.sort_by(|a, b| sharex_helpers::culture::current_culture_ignore_case_cmp(&a.name, &b.name));
            choices.extend(sorted);
            if available.is_empty()
                && let Some(empty) = empty_label
            {
                choices[0] = CaptureDevice::new("", empty);
            }
        }
        // `DebugHelper.WriteException(ex)`.
        Err(e) => eprintln!("capture devices: {e}"),
    }
    if !choices.iter().any(|c| c.id == selected_id) {
        choices.push(CaptureDevice::new(selected_id, unavailable_label));
    }
    let selected = choices.iter().position(|c| c.id == selected_id).unwrap_or(0);
    (choices, selected)
}

#[cfg(windows)]
mod windows_impl {
    use super::CaptureDevice;
    use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
    use windows::Win32::Foundation::RPC_E_CHANGED_MODE;
    use windows::Win32::Media::Audio::{DEVICE_STATE_ACTIVE, IMMDeviceEnumerator, MMDeviceEnumerator, eCapture, eRender};
    use windows::Win32::Media::MediaFoundation::{
        IMFActivate, IMFAttributes, MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME, MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE, MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID,
        MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK, MF_VERSION, MFCreateAttributes, MFEnumDeviceSources, MFSTARTUP_LITE, MFShutdown, MFStartup,
    };
    use windows::Win32::System::Com::{CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, STGM_READ};
    use windows::Win32::System::Com::StructuredStorage::PropVariantToStringAlloc;
    use windows::core::{GUID, PWSTR};

    /// `CoInitializeEx(MTA)`, accepting a thread already in another
    /// apartment (`RPC_E_CHANGED_MODE`), and its `CoUninitialize`.
    struct Com(bool);

    impl Com {
        fn init() -> Result<Self, String> {
            // SAFETY: plain COM initialisation of this thread.
            let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
            if hr == RPC_E_CHANGED_MODE {
                return Ok(Self(false));
            }
            hr.ok().map_err(|e| e.to_string())?;
            Ok(Self(true))
        }
    }

    impl Drop for Com {
        fn drop(&mut self) {
            if self.0 {
                // SAFETY: balances the successful CoInitializeEx above.
                unsafe { CoUninitialize() };
            }
        }
    }

    /// Copies and frees a string COM allocated.
    fn take(p: PWSTR) -> String {
        if p.is_null() {
            return String::new();
        }
        // SAFETY: `p` is a NUL-terminated string from COM, freed once here.
        unsafe {
            let s = p.to_string().unwrap_or_default();
            CoTaskMemFree(Some(p.0 as *const _));
            s
        }
    }

    /// `GetDevices(DataFlow.Render | Capture)`: active endpoints; one that
    /// disappears while it's read is skipped.
    pub fn audio_endpoints(render: bool) -> Result<Vec<CaptureDevice>, String> {
        let _com = Com::init()?;
        // SAFETY: COM calls on interfaces created here, released on drop.
        unsafe {
            let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).map_err(|e| e.to_string())?;
            let endpoints = enumerator.EnumAudioEndpoints(if render { eRender } else { eCapture }, DEVICE_STATE_ACTIVE).map_err(|e| e.to_string())?;
            let count = endpoints.GetCount().map_err(|e| e.to_string())?;
            let mut out = Vec::new();
            for i in 0..count {
                let Ok(device) = endpoints.Item(i) else { continue };
                let Ok(id) = device.GetId() else { continue };
                let id = take(id);
                let Ok(store) = device.OpenPropertyStore(STGM_READ) else { continue };
                let Ok(value) = store.GetValue(&PKEY_Device_FriendlyName) else { continue };
                let Ok(name) = PropVariantToStringAlloc(&value) else { continue };
                out.push(CaptureDevice::new(id, take(name)));
            }
            Ok(out)
        }
    }

    /// `GetCameras`: `MFEnumVideoDeviceSources` between `MFStartup(lite)`
    /// and `MFShutdown`.
    pub fn cameras() -> Result<Vec<CaptureDevice>, String> {
        let _com = Com::init()?;
        // SAFETY: Media Foundation calls on objects created here; the
        // activation array and its strings are freed with CoTaskMemFree.
        unsafe {
            MFStartup(MF_VERSION, MFSTARTUP_LITE).map_err(|e| e.to_string())?;
            let result = (|| -> Result<Vec<CaptureDevice>, String> {
                let mut attributes: Option<IMFAttributes> = None;
                MFCreateAttributes(&mut attributes, 1).map_err(|e| e.to_string())?;
                let attributes = attributes.ok_or("no attributes")?;
                attributes.SetGUID(&MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE, &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID).map_err(|e| e.to_string())?;
                let mut sources: *mut Option<IMFActivate> = std::ptr::null_mut();
                let mut count = 0u32;
                MFEnumDeviceSources(&attributes, &mut sources, &mut count).map_err(|e| e.to_string())?;
                let mut out = Vec::new();
                if !sources.is_null() {
                    let list = std::slice::from_raw_parts_mut(sources, count as usize);
                    for slot in list.iter_mut() {
                        if let Some(activate) = slot.take() {
                            let get = |key: &GUID| -> String {
                                let mut p = PWSTR::null();
                                let mut len = 0u32;
                                if activate.GetAllocatedString(key, &mut p, &mut len).is_ok() { take(p) } else { String::new() }
                            };
                            out.push(CaptureDevice::new(get(&MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK), get(&MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME)));
                        }
                    }
                    CoTaskMemFree(Some(sources as *const _));
                }
                Ok(out)
            })();
            let _ = MFShutdown();
            result
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choices_as_capture_device_combo_builds_them() {
        let devices = Ok(vec![CaptureDevice::new("b", "speakers"), CaptureDevice::new("a", "Headphones")]);
        let (c, sel) = combo_choices(&devices, "a", "Default", "Unavailable", None);
        let names: Vec<&str> = c.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, ["Default", "Headphones", "speakers"]);
        assert_eq!(sel, 1);
        // A saved device that's gone stays selected under its own id.
        let (c, sel) = combo_choices(&devices, "gone", "Default", "Unavailable", None);
        assert_eq!((c[sel].id.as_str(), c[sel].name.as_str()), ("gone", "Unavailable"));
        // No cameras: the default entry says so.
        let (c, sel) = combo_choices(&Ok(Vec::new()), "", "Default camera", "Unavailable", Some("No cameras"));
        assert_eq!((c.len(), sel, c[0].name.as_str()), (1, 0, "No cameras"));
        // A failed listing still offers the default.
        let (c, _) = combo_choices(&Err("x".into()), "", "Default", "Unavailable", Some("No cameras"));
        assert_eq!(c[0].name, "Default");
    }

    #[test]
    fn fakes_list_what_they_hold() {
        let f = FakeDevices { cameras: vec![CaptureDevice::new("cam", "Camera")], ..Default::default() };
        assert_eq!(f.list(DeviceKind::Camera).unwrap().len(), 1);
        assert!(f.list(DeviceKind::Microphone).unwrap().is_empty());
    }

    /// Reading the system's lists never fails hard (it opens nothing).
    #[cfg(windows)]
    #[test]
    fn system_lists_read_only() {
        for kind in [DeviceKind::SystemAudio, DeviceKind::Microphone, DeviceKind::Camera] {
            let _ = SystemDevices.list(kind);
        }
    }
}
