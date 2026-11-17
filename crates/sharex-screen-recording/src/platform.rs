//! The FFmpeg recorder's inputs outside Windows (ShareXrs additions: ShareX
//! records only on Windows, with gdigrab, ddagrab or DirectShow).
//!
//! The same `FFmpegOptions` mean the same thing everywhere: ShareX's
//! desktop sources (`gdigrab`, `ddagrab`) become the platform's screen
//! input, other `VideoSource`/`AudioSource` values name the platform's
//! devices, and everything after the inputs (user arguments, codecs,
//! duration, output) is ShareX's ([`ScreenRecordingOptions::output_args`]).
//! The inputs follow `AppendInputDevice`'s pattern (`-thread_queue_size
//! 1024`, audio first).
//!
//! - macOS: AVFoundation (`-f avfoundation -i "<screen>:<audio>"`), the
//!   region cropped from the screen in pixels. FFmpeg's screen input there
//!   uses `AVCaptureScreenInput`, deprecated in macOS 15 for ScreenCaptureKit,
//!   and it can't record what's playing; so [`Input::RawVideo`] (frames
//!   ShareXrs captures with ScreenCaptureKit, as snapr does) is the
//!   preferred path, AVFoundation the fallback.
//! - Linux on X11: `x11grab` with the region as the display offset, sound
//!   from PulseAudio/PipeWire (`-f pulse`) or ALSA.
//! - Linux on Wayland: FFmpeg has no screen input (`kmsgrab` needs
//!   `CAP_SYS_ADMIN` and sees no cursor; there's no PipeWire input in
//!   mainline FFmpeg), so ShareXrs captures (xdg-desktop-portal ScreenCast
//!   or wlr-screencopy, as snapr) and pipes raw frames: [`Input::RawVideo`].
//!
//! With [`Input::RawVideo`] FFmpeg reads frames from stdin, so it's stopped
//! by closing stdin, not with `q`.

use sharex_settings::FFmpegCaptureDevice;
use sharex_settings::types::Rectangle;

use crate::args::ScreenRecordingOptions;
use crate::dotnet::escape_cli_text;

/// A sound source outside Windows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioInput {
    /// macOS: an AVFoundation audio device, by index or name.
    AvFoundation(String),
    /// Linux: a PulseAudio/PipeWire source (`.monitor` ones record what's
    /// playing).
    Pulse(String),
    /// Linux: an ALSA device (`default`, `hw:0`).
    Alsa(String),
    /// An FFmpeg filter graph (`sine=frequency=440`): synthetic sound, for
    /// tests.
    Lavfi(String),
}

/// Where the video comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    /// macOS: AVFoundation. `screen` is the "Capture screen N" device's
    /// index (or a camera's index/name for a camera source); `screen_bounds`
    /// is that screen in desktop pixels, to crop the region from.
    AvFoundation { device: String, screen_bounds: Option<Rectangle> },
    /// Linux on X11: the display (`$DISPLAY`, ":0.0").
    X11 { display: String },
    /// Linux: a V4L2 camera (`/dev/video0`).
    V4l2 { device: String },
    /// Frames ShareXrs writes to FFmpeg's stdin, `bgra`, the capture
    /// area's size.
    RawVideo { pixel_format: &'static str },
    /// An FFmpeg filter graph as the source (`-f lavfi -i testsrc=...`):
    /// synthetic video, for tests and previews; never the screen.
    Lavfi { graph: String },
}

/// Whether a `VideoSource` means "the desktop" (ShareX's gdigrab/ddagrab).
pub fn is_desktop_source(video_source: &str) -> bool {
    [FFmpegCaptureDevice::GDI_GRAB.value, FFmpegCaptureDevice::DDA_GRAB.value]
        .iter()
        .any(|d| video_source.eq_ignore_ascii_case(d))
}

/// [`platform_ffmpeg_args`] as `GetFFmpegCommands` uses ShareX's: the
/// custom command line when it applies, trimmed.
pub fn platform_ffmpeg_commands(options: &ScreenRecordingOptions, input: &Input, audio: Option<&AudioInput>) -> Option<String> {
    let commands = match options.custom_commands() {
        Some(c) => c,
        None => platform_ffmpeg_args(options, input, audio)?,
    };
    Some(commands.trim().to_string())
}

/// The command line for `options` with this platform's `input` (and
/// `audio`, when an audio source is selected). `None` without a source, as
/// ShareX's builder.
pub fn platform_ffmpeg_args(options: &ScreenRecordingOptions, input: &Input, audio: Option<&AudioInput>) -> Option<String> {
    let f = options.effective_ffmpeg();
    if !options.is_recording {
        return options.get_ffmpeg_args(false, &[]);
    }
    if !f.is_video_source_selected() && !f.is_audio_source_selected() {
        return None;
    }
    let framerate = options.fps.to_string();
    let a = options.capture_area;
    let cursor = if options.draw_cursor { "1" } else { "0" };
    let mut args = String::new();

    let audio = audio.filter(|_| f.is_audio_source_selected());
    // Sound first, like ShareX's dshow audio before gdigrab; AVFoundation
    // takes it with the video.
    match audio {
        Some(AudioInput::Pulse(source)) => {
            args.push_str("-f pulse -thread_queue_size 1024 ");
            args.push_str(&format!("-i {} ", escape_cli_text(source)));
        }
        Some(AudioInput::Alsa(device)) => {
            args.push_str("-f alsa -thread_queue_size 1024 ");
            args.push_str(&format!("-i {} ", escape_cli_text(device)));
        }
        Some(AudioInput::Lavfi(graph)) => {
            args.push_str("-f lavfi -thread_queue_size 1024 ");
            args.push_str(&format!("-i {} ", escape_cli_text(graph)));
        }
        // AVFoundation's sound comes with its video (or alone, below).
        _ => {}
    }

    let mut video_filter = None;
    if f.is_video_source_selected() {
        match input {
            Input::AvFoundation { device, screen_bounds } => {
                let sound = match audio {
                    Some(AudioInput::AvFoundation(d)) => d.as_str(),
                    _ => "none",
                };
                args.push_str("-f avfoundation -thread_queue_size 1024 ");
                args.push_str(&format!("-capture_cursor {cursor} "));
                args.push_str(&format!("-framerate {framerate} "));
                args.push_str(&format!("-i {} ", escape_cli_text(&format!("{device}:{sound}"))));
                if let Some(b) = screen_bounds {
                    video_filter = Some(format!("crop={}:{}:{}:{}", a.width, a.height, a.x - b.x, a.y - b.y));
                }
            }
            Input::X11 { display } => {
                args.push_str("-f x11grab -thread_queue_size 1024 ");
                args.push_str(&format!("-framerate {framerate} "));
                args.push_str(&format!("-video_size {}x{} ", a.width, a.height));
                args.push_str(&format!("-draw_mouse {cursor} "));
                args.push_str(&format!("-i {display}+{},{} ", a.x, a.y));
            }
            Input::V4l2 { device } => {
                args.push_str("-f v4l2 -thread_queue_size 1024 ");
                args.push_str(&format!("-framerate {framerate} "));
                args.push_str(&format!("-i {} ", escape_cli_text(device)));
            }
            Input::Lavfi { graph } => {
                args.push_str("-f lavfi -thread_queue_size 1024 ");
                args.push_str(&format!("-i {} ", escape_cli_text(graph)));
            }
            Input::RawVideo { pixel_format } => {
                args.push_str("-f rawvideo -thread_queue_size 1024 ");
                args.push_str(&format!("-pixel_format {pixel_format} "));
                args.push_str(&format!("-video_size {}x{} ", a.width, a.height));
                args.push_str(&format!("-framerate {framerate} "));
                args.push_str("-i - ");
            }
        }
    } else if let Some(AudioInput::AvFoundation(d)) = audio {
        args.push_str("-f avfoundation -thread_queue_size 1024 ");
        args.push_str(&format!("-i {} ", escape_cli_text(&format!("none:{d}"))));
    }

    if let Some(vf) = video_filter {
        args.push_str(&format!("-filter:v {vf} "));
    }
    args.push_str(&options.output_args(&f, &framerate, false));
    Some(args)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::ScreenRecordingOptions;

    fn options() -> ScreenRecordingOptions {
        ScreenRecordingOptions {
            is_recording: true,
            fps: 30,
            capture_area: Rectangle::new(100, 50, 640, 480),
            draw_cursor: true,
            output_path: Some("/tmp/rec.mp4".into()),
            ..Default::default()
        }
    }

    #[test]
    fn x11_with_pulse() {
        let mut o = options();
        o.ffmpeg.AudioSource = "alsa_output.monitor".into();
        let args = platform_ffmpeg_args(&o, &Input::X11 { display: ":0.0".into() }, Some(&AudioInput::Pulse("alsa_output.monitor".into()))).unwrap();
        assert_eq!(
            args,
            r#"-f pulse -thread_queue_size 1024 -i "alsa_output.monitor" -f x11grab -thread_queue_size 1024 -framerate 30 -video_size 640x480 -draw_mouse 1 -i :0.0+100,50 -c:v libx264 -r 30 -preset ultrafast -tune zerolatency -crf 28 -pix_fmt yuv420p -movflags +faststart -c:a aac -ac 2 -b:a 128k -y "/tmp/rec.mp4""#
        );
    }

    #[test]
    fn avfoundation_crops_the_region() {
        let o = options();
        let input = Input::AvFoundation { device: "1".into(), screen_bounds: Some(Rectangle::new(0, 0, 2880, 1800)) };
        let args = platform_ffmpeg_args(&o, &input, None).unwrap();
        assert!(args.starts_with(r#"-f avfoundation -thread_queue_size 1024 -capture_cursor 1 -framerate 30 -i "1:none" -filter:v crop=640:480:100:50 -c:v libx264"#), "{args}");
    }

    #[test]
    fn raw_video_from_stdin() {
        let args = platform_ffmpeg_args(&options(), &Input::RawVideo { pixel_format: "bgra" }, None).unwrap();
        assert!(args.starts_with("-f rawvideo -thread_queue_size 1024 -pixel_format bgra -video_size 640x480 -framerate 30 -i - -c:v libx264"), "{args}");
    }
}
