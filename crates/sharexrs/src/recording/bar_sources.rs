//! The native recorder bar's sources (`ScreenRecorderBarWindow`'s
//! `Source`, `AddSource`, `RefreshSource`, `UpdateSource`,
//! `ValidateSources`, `BuildSourcePanel`, `BuildCameraPanel`,
//! `VolumeControl`) and `CameraLayoutPreview`: System audio and Microphone
//! (MP4 only) and Camera, each a toggle with an arrow opening its panel
//! (device, refresh, then the volume or the camera's overlay and capture
//! settings). A source that's on with its device gone shows its
//! "unavailable" text under the bar and disables Record.

use egui::{Color32, Rect, Ui, pos2, vec2};
use sharex_settings::{CameraCaptureResolution, CameraOverlayPosition, CameraOverlayShape, TaskSettingsCapture};
use sharex_strings::sharex as s;
use sharex_ui::keys::app_icons as icon;

/// One of the native bar's sources.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceKind {
    SystemAudio,
    Microphone,
    Camera,
}

/// `DeviceChoice`: a device's ID (as ShareX stores it) and name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceChoice {
    pub id: String,
    pub name: String,
}

/// A source's devices now and the default's name (`Enumerate`,
/// `DefaultName`; the camera has no default name: its first camera is).
pub type DeviceLister = Box<dyn Fn(SourceKind) -> Result<(Vec<DeviceChoice>, Option<String>), String>>;

impl SourceKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::SystemAudio => s::ScreenRecorderBar_SystemAudio(),
            Self::Microphone => s::ScreenRecorderBar_Microphone(),
            Self::Camera => s::ScreenRecorderBar_Camera(),
        }
    }
    pub fn icon(self) -> &'static str {
        match self {
            Self::SystemAudio => icon::Volume,
            Self::Microphone => icon::Microphone,
            Self::Camera => icon::Webcam,
        }
    }
    pub fn default_label(self) -> &'static str {
        match self {
            Self::SystemAudio => s::TaskSettingsWindow_NativeRecorderDefaultSystemAudioDevice(),
            Self::Microphone => s::TaskSettingsWindow_NativeRecorderDefaultMicrophone(),
            Self::Camera => s::TaskSettingsWindow_NativeRecorderCamera_DefaultDevice(),
        }
    }
    pub fn unavailable_label(self) -> &'static str {
        match self {
            Self::SystemAudio => s::TaskSettingsWindow_NativeRecorderUnavailableSystemAudioDevice(),
            Self::Microphone => s::TaskSettingsWindow_NativeRecorderUnavailableMicrophone(),
            Self::Camera => s::TaskSettingsWindow_NativeRecorderCamera_UnavailableDevice(),
        }
    }
    pub fn enabled(self, c: &TaskSettingsCapture) -> bool {
        match self {
            Self::SystemAudio => c.ScreenRecordSystemAudio,
            Self::Microphone => c.ScreenRecordMicrophone,
            Self::Camera => c.ScreenRecordCamera,
        }
    }
    pub fn set_enabled(self, c: &mut TaskSettingsCapture, on: bool) {
        match self {
            Self::SystemAudio => c.ScreenRecordSystemAudio = on,
            Self::Microphone => c.ScreenRecordMicrophone = on,
            Self::Camera => c.ScreenRecordCamera = on,
        }
    }
    pub fn selected(self, c: &TaskSettingsCapture) -> &str {
        match self {
            Self::SystemAudio => &c.ScreenRecordSystemAudioDeviceId,
            Self::Microphone => &c.ScreenRecordMicrophoneDeviceId,
            Self::Camera => &c.ScreenRecordCameraDeviceId,
        }
    }
    pub fn set_selected(self, c: &mut TaskSettingsCapture, id: &str) {
        let target = match self {
            Self::SystemAudio => &mut c.ScreenRecordSystemAudioDeviceId,
            Self::Microphone => &mut c.ScreenRecordMicrophoneDeviceId,
            Self::Camera => &mut c.ScreenRecordCameraDeviceId,
        };
        *target = id.to_owned();
    }
}

/// A source's state (`Source`'s `Devices`, `ResolvedDefault`, `Available`).
#[derive(Clone, Debug)]
pub struct Source {
    pub kind: SourceKind,
    pub devices: Vec<DeviceChoice>,
    pub resolved_default: Option<String>,
    pub available: bool,
}

impl Source {
    pub fn new(kind: SourceKind) -> Self {
        Self { kind, devices: Vec::new(), resolved_default: None, available: false }
    }

    /// `RefreshSource`: the devices again; a failure lists none.
    pub fn refresh(&mut self, capture: &TaskSettingsCapture, lister: &DeviceLister) {
        match lister(self.kind) {
            Ok((devices, default)) => {
                self.resolved_default = if self.kind == SourceKind::Camera { devices.first().map(|d| d.name.clone()) } else { default };
                self.devices = devices;
            }
            Err(e) => {
                eprintln!("{e}");
                self.devices.clear();
                self.resolved_default = None;
            }
        }
        let selected = self.kind.selected(capture);
        self.available = if selected.is_empty() { self.resolved_default.is_some() } else { self.devices.iter().any(|d| d.id == selected) };
    }

    /// `UpdateSource`'s tooltip detail: the selected device's name, the default's, or "unavailable".
    pub fn device_name(&self, capture: &TaskSettingsCapture) -> String {
        let selected = self.kind.selected(capture);
        let name = if selected.is_empty() {
            self.resolved_default.as_deref()
        } else {
            self.devices.iter().find(|d| d.id == selected).map(|d| d.name.as_str())
        };
        name.unwrap_or(self.kind.unavailable_label()).to_owned()
    }

    /// `BuildSourcePanel`'s `Refresh`: the default first (with its name, or
    /// unavailable; "No cameras detected"), the devices by name, then the
    /// selected device if it's gone.
    pub fn choices(&self, capture: &TaskSettingsCapture) -> Vec<DeviceChoice> {
        let k = self.kind;
        let default = match &self.resolved_default {
            None if k == SourceKind::Camera => s::TaskSettingsWindow_NativeRecorderCamera_NoDevices().to_owned(),
            None => format!("{} ({})", k.default_label(), k.unavailable_label()),
            Some(name) => format!("{} ({name})", k.default_label()),
        };
        let mut choices = vec![DeviceChoice { id: String::new(), name: default }];
        let mut devices = self.devices.clone();
        // `StringComparer.CurrentCultureIgnoreCase` (stable).
        devices.sort_by_key(|d| d.name.to_lowercase());
        choices.extend(devices);
        let selected = k.selected(capture);
        if !choices.iter().any(|d| d.id == selected) {
            choices.push(DeviceChoice { id: selected.to_owned(), name: k.unavailable_label().to_owned() });
        }
        choices
    }
}

/// `ValidateSources`: the first source that's on without its device.
pub fn missing_source<'a>(sources: &'a [Source], capture: &TaskSettingsCapture) -> Option<&'a Source> {
    sources.iter().find(|s| s.kind.enabled(capture) && !s.available)
}

/// The native bar's sources: system audio and microphone for MP4, the camera always.
pub fn kinds(native: bool, gif: bool) -> Vec<SourceKind> {
    let mut k = Vec::new();
    if native && !gif {
        k.extend([SourceKind::SystemAudio, SourceKind::Microphone]);
    }
    if native {
        k.push(SourceKind::Camera);
    }
    k
}

/// The devices the app offers: MMDevice endpoints and Media Foundation cameras.
pub fn system_devices() -> DeviceLister {
    Box::new(|kind| {
        use sharex_native_recorder::devices as d;
        let audio = |list: Vec<d::AudioCaptureDevice>| list.into_iter().map(|x| DeviceChoice { id: x.id, name: x.name }).collect::<Vec<_>>();
        Ok(match kind {
            SourceKind::SystemAudio => (audio(d::system_audio_devices()?), d::default_system_audio_device().map(|x| x.name)),
            SourceKind::Microphone => (audio(d::microphones()?), d::default_microphone().map(|x| x.name)),
            SourceKind::Camera => (d::cameras()?.into_iter().map(|x| DeviceChoice { id: x.id, name: x.name }).collect(), None),
        })
    })
}

/// `TaskSettingsPageBuilder.CaptureDeviceCombo`'s items: the default (or,
/// with no devices, `empty_label`), the devices by name, then the saved
/// device if it isn't connected. A listing failure leaves the default only.
pub fn form_choices(devices: Result<Vec<DeviceChoice>, String>, selected: &str, default_label: &str, unavailable: &str, empty_label: Option<&str>) -> Vec<DeviceChoice> {
    let mut choices = vec![DeviceChoice { id: String::new(), name: default_label.to_owned() }];
    match devices {
        Ok(mut list) => {
            if list.is_empty()
                && let Some(e) = empty_label
            {
                choices[0].name = e.to_owned();
            }
            list.sort_by_key(|d| d.name.to_lowercase());
            choices.extend(list);
        }
        Err(e) => eprintln!("{e}"),
    }
    if !choices.iter().any(|d| d.id == selected) {
        choices.push(DeviceChoice { id: selected.to_owned(), name: unavailable.to_owned() });
    }
    choices
}

/// The devices of a kind for forms drawn every frame: listed again at most
/// every two seconds (ShareX lists them when the combo box is made).
pub fn cached_devices(kind: SourceKind) -> Result<Vec<DeviceChoice>, String> {
    use std::sync::{Mutex, OnceLock};
    use std::time::{Duration, Instant};
    type Cache = Mutex<Vec<(SourceKind, Instant, Result<Vec<DeviceChoice>, String>)>>;
    static CACHE: OnceLock<Cache> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(Vec::new()));
    let mut c = cache.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, at, r)) = c.iter().find(|(k, _, _)| *k == kind)
        && at.elapsed() < Duration::from_secs(2)
    {
        return r.clone();
    }
    let r = system_devices()(kind).map(|(d, _)| d);
    c.retain(|(k, _, _)| *k != kind);
    c.push((kind, Instant::now(), r.clone()));
    r
}

/// `CameraLayoutPreview.Render`: the recording scaled into `rect`, the
/// camera tile (16:9, 4:3 for 640x480) where the overlay puts it.
pub fn paint_layout_preview(ui: &Ui, rect: Rect, c: &TaskSettingsCapture, region: (i32, i32), accent: Color32, outline: Color32) {
    let (tw, th) = region;
    if tw <= 0 || th <= 0 || rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }
    let scale = (rect.width() as f64 / tw as f64).min(rect.height() as f64 / th as f64);
    let screen = Rect::from_min_size(
        pos2(rect.min.x + ((rect.width() as f64 - tw as f64 * scale) / 2.0) as f32, rect.min.y + ((rect.height() as f64 - th as f64 * scale) / 2.0) as f32),
        vec2((tw as f64 * scale) as f32, (th as f64 * scale) as f32),
    );
    let painter = ui.painter();
    let stroke = egui::Stroke::new(1.0, outline);
    painter.rect(screen, 0.0, Color32::from_rgba_unmultiplied(128, 128, 128, 30), stroke, egui::StrokeKind::Middle);
    let tile = layout_tile(c, region);
    let r = Rect::from_min_size(
        pos2(screen.min.x + (tile.x as f64 * scale) as f32, screen.min.y + (tile.y as f64 * scale) as f32),
        vec2((tile.w as f64 * scale) as f32, (tile.h as f64 * scale) as f32),
    );
    if c.ScreenRecordCameraShape == CameraOverlayShape::Circle {
        let painter = painter.clone();
        painter.add(egui::Shape::ellipse_filled(r.center(), r.size() / 2.0, accent));
        painter.add(egui::Shape::ellipse_stroke(r.center(), r.size() / 2.0, stroke));
    } else {
        painter.rect(r, 2.0, accent, stroke, egui::StrokeKind::Middle);
    }
}

/// The preview's tile in recording pixels (`Render`'s arithmetic, which
/// guesses the camera's aspect from the requested resolution).
pub fn layout_tile(c: &TaskSettingsCapture, region: (i32, i32)) -> sharex_capture::Rect {
    let (tw, th) = region;
    let aspect = if c.ScreenRecordCameraResolution == CameraCaptureResolution::Size640x480 { 4.0 / 3.0 } else { 16.0 / 9.0 };
    let margin = c.ScreenRecordCameraMargin.min(0.max(tw.min(th) / 2 - 1));
    let (aw, ah) = (tw - margin * 2, th - margin * 2);
    let mut w = (tw * c.ScreenRecordCameraWidthPercent / 100).clamp(2, aw.max(2));
    let circle = c.ScreenRecordCameraShape == CameraOverlayShape::Circle;
    let mut h = if circle { w } else { 2.max((w as f64 / aspect) as i32) };
    if h > ah {
        h = ah;
        w = if circle { h } else { ((h as f64 * aspect) as i32).clamp(2, aw.max(2)) };
    }
    let left = matches!(c.ScreenRecordCameraPosition, CameraOverlayPosition::TopLeft | CameraOverlayPosition::BottomLeft);
    let top = matches!(c.ScreenRecordCameraPosition, CameraOverlayPosition::TopLeft | CameraOverlayPosition::TopRight);
    sharex_capture::Rect::new(if left { margin } else { tw - margin - w }, if top { margin } else { th - margin - h }, w, h)
}

/// The camera panel's choices, in `Enum.GetValues` order with ShareX's labels.
pub fn shape_choices() -> [(CameraOverlayShape, &'static str); 2] {
    [
        (CameraOverlayShape::Rectangle, s::TaskSettingsWindow_NativeRecorderCamera_Rectangle()),
        (CameraOverlayShape::Circle, s::TaskSettingsWindow_NativeRecorderCamera_Circle()),
    ]
}

pub fn position_choices() -> [(CameraOverlayPosition, &'static str); 4] {
    [
        (CameraOverlayPosition::TopLeft, s::ApplicationSettingsWindow_TopLeft()),
        (CameraOverlayPosition::TopRight, s::ApplicationSettingsWindow_TopRight()),
        (CameraOverlayPosition::BottomLeft, s::ApplicationSettingsWindow_BottomLeft()),
        (CameraOverlayPosition::BottomRight, s::ApplicationSettingsWindow_BottomRight()),
    ]
}

pub fn resolution_choices() -> [(CameraCaptureResolution, &'static str); 3] {
    [
        (CameraCaptureResolution::Size640x480, "640 × 480"),
        (CameraCaptureResolution::Size1280x720, "1280 × 720"),
        (CameraCaptureResolution::Size1920x1080, "1920 × 1080"),
    ]
}

/// `VolumeControl`'s percentage (`{volume.Value:0}%`) and the slider's value from a gain.
pub fn volume_percent(gain: f32) -> f32 {
    (gain * 100.0).clamp(0.0, 400.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lister(devices: Vec<(&'static str, &'static str)>, default: Option<&'static str>) -> DeviceLister {
        Box::new(move |_| Ok((devices.iter().map(|(i, n)| DeviceChoice { id: (*i).into(), name: (*n).into() }).collect(), default.map(Into::into))))
    }

    #[test]
    fn availability_as_sharex() {
        let mut c = TaskSettingsCapture::default();
        let mut s = Source::new(SourceKind::Microphone);
        // Default device, none there.
        s.refresh(&c, &lister(vec![], None));
        assert!(!s.available);
        assert_eq!(s.device_name(&c), s::TaskSettingsWindow_NativeRecorderUnavailableMicrophone());
        s.refresh(&c, &lister(vec![("{b}", "Beta"), ("{a}", "alpha")], Some("Beta")));
        assert!(s.available);
        assert_eq!(s.device_name(&c), "Beta");
        let names: Vec<String> = s.choices(&c).into_iter().map(|d| d.name).collect();
        assert_eq!(names, vec![format!("{} (Beta)", s::TaskSettingsWindow_NativeRecorderDefaultMicrophone()), "alpha".into(), "Beta".into()]);
        // A chosen device that's gone.
        c.ScreenRecordMicrophoneDeviceId = "{gone}".into();
        s.refresh(&c, &lister(vec![("{a}", "alpha")], Some("alpha")));
        assert!(!s.available);
        let choices = s.choices(&c);
        assert_eq!(choices.last().unwrap(), &DeviceChoice { id: "{gone}".into(), name: s::TaskSettingsWindow_NativeRecorderUnavailableMicrophone().into() });
        c.ScreenRecordMicrophone = true;
        assert_eq!(missing_source(std::slice::from_ref(&s), &c).map(|s| s.kind), Some(SourceKind::Microphone));
        c.ScreenRecordMicrophone = false;
        assert!(missing_source(std::slice::from_ref(&s), &c).is_none());
    }

    #[test]
    fn the_camera_defaults_to_its_first_device() {
        let c = TaskSettingsCapture::default();
        let mut s = Source::new(SourceKind::Camera);
        s.refresh(&c, &lister(vec![], None));
        assert_eq!(s.choices(&c)[0].name, s::TaskSettingsWindow_NativeRecorderCamera_NoDevices());
        s.refresh(&c, &lister(vec![(r"\\?\usb#1", "Webcam")], None));
        assert!(s.available);
        assert_eq!(s.choices(&c)[0].name, format!("{} (Webcam)", s::TaskSettingsWindow_NativeRecorderCamera_DefaultDevice()));
        let failing: DeviceLister = Box::new(|_| Err("no".into()));
        s.refresh(&c, &failing);
        assert!(!s.available && s.devices.is_empty());
    }

    #[test]
    fn form_choices_as_sharex() {
        let d = |i: &str, n: &str| DeviceChoice { id: i.into(), name: n.into() };
        let names = |v: Vec<DeviceChoice>| v.into_iter().map(|d| d.name).collect::<Vec<_>>();
        assert_eq!(names(form_choices(Ok(vec![d("b", "b"), d("a", "A")]), "", "Default", "Gone", None)), ["Default", "A", "b"]);
        assert_eq!(names(form_choices(Ok(vec![]), "", "First", "Gone", Some("None"))), ["None"]);
        assert_eq!(names(form_choices(Ok(vec![d("a", "A")]), "x", "Default", "Gone", None)), ["Default", "A", "Gone"]);
        assert_eq!(names(form_choices(Err("no".into()), "", "Default", "Gone", Some("None"))), ["Default"]);
    }

    #[test]
    fn kinds_per_recorder() {
        assert_eq!(kinds(true, false), vec![SourceKind::SystemAudio, SourceKind::Microphone, SourceKind::Camera]);
        assert_eq!(kinds(true, true), vec![SourceKind::Camera]);
        assert!(kinds(false, false).is_empty());
    }

    #[test]
    fn preview_tile_as_sharex() {
        let mut c = TaskSettingsCapture::default();
        c.ScreenRecordCameraWidthPercent = 20;
        c.ScreenRecordCameraMargin = 16;
        c.ScreenRecordCameraPosition = CameraOverlayPosition::BottomRight;
        c.ScreenRecordCameraShape = CameraOverlayShape::Rectangle;
        c.ScreenRecordCameraResolution = CameraCaptureResolution::Size1280x720;
        assert_eq!(layout_tile(&c, (1920, 1080)), sharex_capture::Rect::new(1920 - 16 - 384, 1080 - 16 - 216, 384, 216));
        c.ScreenRecordCameraResolution = CameraCaptureResolution::Size640x480;
        assert_eq!(layout_tile(&c, (1920, 1080)).h, 288);
        c.ScreenRecordCameraShape = CameraOverlayShape::Circle;
        c.ScreenRecordCameraPosition = CameraOverlayPosition::TopLeft;
        assert_eq!(layout_tile(&c, (1920, 1080)), sharex_capture::Rect::new(16, 16, 384, 384));
        assert_eq!(volume_percent(5.0), 400.0);
        assert_eq!(volume_percent(0.5), 50.0);
    }
}
