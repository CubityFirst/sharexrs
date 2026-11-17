//! Recording devices as FFmpeg reports them.
//!
//! - Windows: DirectShow, exactly as ShareX lists it
//!   (`FFmpegCLIManager.GetDirectShowDevices`, `-list_devices true -f dshow
//!   -i dummy`), plus the sources ShareX always offers
//!   ([`video_sources`], `FFmpegOptionsWindow.RefreshSourcesAsync`).
//! - macOS (ShareXrs addition; ShareX doesn't run there): AVFoundation,
//!   `-f avfoundation -list_devices true -i ""`.
//! - Linux (ShareXrs addition): PulseAudio/PipeWire sources from `pactl
//!   list short sources` (FFmpeg's `-sources pulse` needs a libpulse build,
//!   which static FFmpeg builds lack), ALSA's `default` otherwise; video is
//!   the X11 display or, on Wayland, frames ShareXrs captures itself (see
//!   [`crate::platform`]).
//!
//! Also `GetVideoInfo`'s parsing of `ffmpeg -i`.

use std::sync::OnceLock;

use regex::Regex;
use sharex_settings::FFmpegCaptureDevice;

/// `ShareX.HelpersLib.DirectShowDevices`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DirectShowDevices {
    pub video_devices: Vec<String>,
    pub audio_devices: Vec<String>,
}

/// `GetDirectShowDevices`' parsing of FFmpeg's device list: the headers
/// ("DirectShow video devices") of older FFmpegs, the `(video)`/`(audio)`
/// suffixes of newer ones, alternative names skipped.
pub fn parse_direct_show_devices(output: &str) -> DirectShowDevices {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r#"\[(?:dshow|in#0) @ \w+\] +"(.+)""#).expect("pattern"));
    let mut devices = DirectShowDevices::default();
    let mut is_audio = false;
    for line in dotnet_lines(output) {
        if line.contains("] DirectShow video devices") {
            is_audio = false;
            continue;
        } else if line.contains("] DirectShow audio devices") {
            is_audio = true;
            continue;
        }
        if let Some(c) = re.captures(line) {
            if line.ends_with("\" (video)") {
                is_audio = false;
            } else if line.ends_with("\" (audio)") {
                is_audio = true;
            }
            let name = c[1].to_string();
            if is_audio {
                devices.audio_devices.push(name);
            } else {
                devices.video_devices.push(name);
            }
        }
    }
    devices
}

/// `string.Lines()`: split at `\r\n`, `\r` or `\n`.
fn dotnet_lines(text: &str) -> impl Iterator<Item = &str> {
    text.split("\r\n").flat_map(|l| l.split(['\r', '\n']))
}

/// A source the FFmpeg options window offers: the value stored in
/// `VideoSource`/`AudioSource` and its title.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureSource {
    pub value: String,
    pub title: String,
}

/// `RefreshSourcesAsync`'s video list: None, gdigrab, ddagrab (Windows 10
/// and later), then the DirectShow video devices as "dshow (name)".
/// `none_title`, `gdi_title` and `dda_title` are the window's localised
/// strings (`FFmpegOptionsWindow_None`, `_Source_GDI`, `_Source_DDA`).
pub fn video_sources(devices: Option<&DirectShowDevices>, windows_10: bool, titles: [&str; 3]) -> Vec<CaptureSource> {
    let [none, gdi, dda] = titles;
    let mut v = vec![
        CaptureSource { value: FFmpegCaptureDevice::NONE.value.into(), title: none.into() },
        CaptureSource { value: FFmpegCaptureDevice::GDI_GRAB.value.into(), title: gdi.into() },
    ];
    if windows_10 {
        v.push(CaptureSource { value: FFmpegCaptureDevice::DDA_GRAB.value.into(), title: dda.into() });
    }
    if let Some(d) = devices {
        v.extend(d.video_devices.iter().map(|x| CaptureSource { value: x.clone(), title: format!("dshow ({x})") }));
    }
    v
}

/// `RefreshSourcesAsync`'s audio list: None, then the DirectShow audio
/// devices.
pub fn audio_sources(devices: Option<&DirectShowDevices>, none_title: &str) -> Vec<CaptureSource> {
    let mut v = vec![CaptureSource { value: FFmpegCaptureDevice::NONE.value.into(), title: none_title.into() }];
    if let Some(d) = devices {
        v.extend(d.audio_devices.iter().map(|x| CaptureSource { value: x.clone(), title: format!("dshow ({x})") }));
    }
    v
}

/// `RefreshSourcesAsync`'s fix-up of the chosen sources after listing:
/// with `select_recorder_devices` (after installing ShareX's recorder
/// devices) prefer them; a source that's gone falls back to gdigrab/None.
pub fn reconcile_sources(
    options: &mut sharex_settings::FFmpegOptions,
    video: &[CaptureSource],
    audio: &[CaptureSource],
    select_recorder_devices: bool,
) {
    let has = |list: &[CaptureSource], value: &str| list.iter().any(|s| s.value.eq_ignore_ascii_case(value));
    if select_recorder_devices && has(video, FFmpegCaptureDevice::SCREEN_CAPTURE_RECORDER.value) {
        options.VideoSource = FFmpegCaptureDevice::SCREEN_CAPTURE_RECORDER.value.into();
    } else if !has(video, &options.VideoSource) {
        options.VideoSource = FFmpegCaptureDevice::GDI_GRAB.value.into();
    }
    if select_recorder_devices && has(audio, FFmpegCaptureDevice::VIRTUAL_AUDIO_CAPTURER.value) {
        options.AudioSource = FFmpegCaptureDevice::VIRTUAL_AUDIO_CAPTURER.value.into();
    } else if !has(audio, &options.AudioSource) {
        options.AudioSource = FFmpegCaptureDevice::NONE.value.into();
    }
}

/// `ShareX.HelpersLib.VideoInfo`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct VideoInfo {
    pub file_path: String,
    /// Ticks.
    pub duration: i64,
    /// kb/s.
    pub bitrate: i32,
    pub video_codec: Option<String>,
    pub video_resolution: Option<(i32, i32)>,
    pub video_fps: Option<f64>,
    pub audio_codec: Option<String>,
}

/// `GetVideoInfo`'s parsing; `None` without a duration line.
pub fn parse_video_info(path: &str, output: &str) -> Option<VideoInfo> {
    static INPUT: OnceLock<Regex> = OnceLock::new();
    static VIDEO: OnceLock<Regex> = OnceLock::new();
    static AUDIO: OnceLock<Regex> = OnceLock::new();
    // .NET's `.` doesn't match `\n`; neither does Rust's.
    let input = INPUT.get_or_init(|| {
        Regex::new(r"(?i)Duration: (?<Duration>\d{2}:\d{2}:\d{2}\.\d{2}),.+?start: (?<Start>\d+\.\d+),.+?bitrate: (?<Bitrate>\d+) kb/s")
            .expect("pattern")
    });
    let video = VIDEO.get_or_init(|| {
        Regex::new(r"(?i)Stream #\d+:\d+(?:\(.+?\))?: Video: (?<Codec>.+?) \(.+?,.+?, (?<Width>\d+)x(?<Height>\d+).+?, (?<FPS>\d+(?:\.\d+)?) fps")
            .expect("pattern")
    });
    let audio = AUDIO.get_or_init(|| Regex::new(r"(?i)Stream #\d+:\d+(?:\(.+?\))?: Audio: (?<Codec>.+?)(?: \(|,)").expect("pattern"));

    let m = input.captures(output)?;
    let mut info = VideoInfo {
        file_path: path.into(),
        duration: crate::dotnet::parse_timespan(&m["Duration"])?,
        bitrate: m["Bitrate"].parse().ok()?,
        ..Default::default()
    };
    if let Some(v) = video.captures(output) {
        info.video_codec = Some(v["Codec"].to_string());
        info.video_resolution = Some((v["Width"].parse().ok()?, v["Height"].parse().ok()?));
        info.video_fps = v["FPS"].parse().ok();
    }
    if let Some(a) = audio.captures(output) {
        info.audio_codec = Some(a["Codec"].to_string());
    }
    Some(info)
}

/// An AVFoundation device: its index (what `-i "<video>:<audio>"` takes)
/// and name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvFoundationDevice {
    pub index: u32,
    pub name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AvFoundationDevices {
    /// Cameras and "Capture screen N".
    pub video_devices: Vec<AvFoundationDevice>,
    pub audio_devices: Vec<AvFoundationDevice>,
}

impl AvFoundationDevices {
    /// The screens, "Capture screen N" in display order.
    pub fn screens(&self) -> impl Iterator<Item = &AvFoundationDevice> {
        self.video_devices.iter().filter(|d| d.name.starts_with("Capture screen"))
    }
}

/// Parses `ffmpeg -f avfoundation -list_devices true -i ""` (ShareXrs
/// addition):
///
/// ```text
/// [AVFoundation indev @ 0x7f8] AVFoundation video devices:
/// [AVFoundation indev @ 0x7f8] [0] FaceTime HD Camera
/// [AVFoundation indev @ 0x7f8] [1] Capture screen 0
/// [AVFoundation indev @ 0x7f8] AVFoundation audio devices:
/// [AVFoundation indev @ 0x7f8] [0] MacBook Pro Microphone
/// ```
pub fn parse_avfoundation_devices(output: &str) -> AvFoundationDevices {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"\[AVFoundation [^\]]*\] \[(\d+)\] (.+)$").expect("pattern"));
    let mut devices = AvFoundationDevices::default();
    let mut audio = false;
    for line in dotnet_lines(output) {
        if line.contains("AVFoundation video devices") {
            audio = false;
        } else if line.contains("AVFoundation audio devices") {
            audio = true;
        } else if let Some(c) = re.captures(line)
            && let Ok(index) = c[1].parse()
        {
            let d = AvFoundationDevice { index, name: c[2].trim_end().to_string() };
            if audio { devices.audio_devices.push(d) } else { devices.video_devices.push(d) }
        }
    }
    devices
}

/// A PulseAudio/PipeWire source (ShareXrs addition): `name` is what
/// `-f pulse -i <name>` takes; monitors record what's playing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PulseSource {
    pub name: String,
    pub is_monitor: bool,
}

/// Parses `pactl list short sources` (tab separated: index, name, driver,
/// format, state).
pub fn parse_pactl_sources(output: &str) -> Vec<PulseSource> {
    output
        .lines()
        .filter_map(|l| l.split('\t').nth(1))
        .filter(|n| !n.is_empty())
        .map(|n| PulseSource { name: n.to_string(), is_monitor: n.ends_with(".monitor") })
        .collect()
}

/// The platform's devices, listed through FFmpeg (or `pactl` on Linux).
pub enum PlatformDevices {
    DirectShow(DirectShowDevices),
    AvFoundation(AvFoundationDevices),
    Pulse(Vec<PulseSource>),
}

/// Lists the recording devices this platform's FFmpeg can open.
pub fn list_devices(ffmpeg: &std::path::Path) -> PlatformDevices {
    if cfg!(windows) {
        PlatformDevices::DirectShow(crate::cli::FFmpegCliManager::new(ffmpeg).get_direct_show_devices())
    } else if cfg!(target_os = "macos") {
        let m = crate::cli::FFmpegCliManager::new(ffmpeg);
        m.run("-hide_banner -f avfoundation -list_devices true -i \"\"");
        PlatformDevices::AvFoundation(parse_avfoundation_devices(&m.output()))
    } else {
        let out = std::process::Command::new("pactl").args(["list", "short", "sources"]).output();
        PlatformDevices::Pulse(out.map(|o| parse_pactl_sources(&String::from_utf8_lossy(&o.stdout))).unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_new_and_old_dshow_lists() {
        let new = "[in#0 @ 000001] \"Integrated Camera\" (video)\r\n[in#0 @ 000001]   Alternative name \"@device_pnp_\\\\?\\usb\"\r\n[in#0 @ 000001] \"Microphone (Realtek(R) Audio)\" (audio)\r\n[in#0 @ 000001]   Alternative name \"@device_cm_{33D9}\"\r\n";
        let d = parse_direct_show_devices(new);
        assert_eq!(d.video_devices, ["Integrated Camera"]);
        assert_eq!(d.audio_devices, ["Microphone (Realtek(R) Audio)"]);

        let old = "[dshow @ 0abc] DirectShow video devices (some may be both video and audio devices)\n[dshow @ 0abc]  \"screen-capture-recorder\"\n[dshow @ 0abc] DirectShow audio devices\n[dshow @ 0abc]  \"virtual-audio-capturer\"\n";
        let d = parse_direct_show_devices(old);
        assert_eq!(d.video_devices, ["screen-capture-recorder"]);
        assert_eq!(d.audio_devices, ["virtual-audio-capturer"]);
    }

    #[test]
    fn parses_avfoundation_lists() {
        let out = "[AVFoundation indev @ 0x7f8] AVFoundation video devices:\n[AVFoundation indev @ 0x7f8] [0] FaceTime HD Camera\n[AVFoundation indev @ 0x7f8] [1] Capture screen 0\n[AVFoundation indev @ 0x7f8] AVFoundation audio devices:\n[AVFoundation indev @ 0x7f8] [0] MacBook Pro Microphone\n";
        let d = parse_avfoundation_devices(out);
        assert_eq!(d.video_devices.len(), 2);
        assert_eq!(d.screens().map(|s| s.index).collect::<Vec<_>>(), [1]);
        assert_eq!(d.audio_devices[0].name, "MacBook Pro Microphone");
    }

    #[test]
    fn parses_pactl() {
        let s = parse_pactl_sources("47\talsa_output.pci.analog-stereo.monitor\tPipeWire\ts32le 2ch 48000Hz\tSUSPENDED\n48\talsa_input.pci.analog-stereo\tPipeWire\ts32le 2ch 48000Hz\tRUNNING\n");
        assert_eq!(s.len(), 2);
        assert!(s[0].is_monitor && !s[1].is_monitor);
    }

    #[test]
    fn parses_video_info() {
        let out = "Input #0, mov,mp4,m4a,3gp,3g2,mj2, from 'a.mp4':\n  Duration: 00:00:02.00, start: 0.000000, bitrate: 1095 kb/s\n  Stream #0:0[0x1](und): Video: h264 (High) (avc1 / 0x31637661), yuv420p(progressive), 320x240 [SAR 1:1 DAR 4:3], 1000 kb/s, 30 fps, 30 tbr, 15360 tbn (default)\n  Stream #0:1[0x2](und): Audio: aac (LC) (mp4a / 0x6134706D), 44100 Hz, stereo, fltp, 128 kb/s (default)\n";
        let i = parse_video_info("a.mp4", out).unwrap();
        assert_eq!((i.duration, i.bitrate), (20_000_000, 1095));
        // ShareX's stream patterns predate the "[0x1]" stream ids FFmpeg
        // prints now, so they find nothing in today's output (as in ShareX).
        assert_eq!((i.video_codec, i.audio_codec), (None, None));

        let old = "  Duration: 00:00:02.00, start: 0.000000, bitrate: 1095 kb/s
    Stream #0:0(und): Video: h264 (High) (avc1 / 0x31637661), yuv420p, 320x240 [SAR 1:1 DAR 4:3], 1000 kb/s, 30 fps, 30 tbr
    Stream #0:1(und): Audio: aac (LC) (mp4a / 0x6134706D), 44100 Hz
";
        let i = parse_video_info("a.mp4", old).unwrap();
        assert_eq!(i.video_codec.as_deref(), Some("h264"));
        assert_eq!(i.video_resolution, Some((320, 240)));
        assert_eq!(i.video_fps, Some(30.0));
        assert_eq!(i.audio_codec.as_deref(), Some("aac"));
    }
}
