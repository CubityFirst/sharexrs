//! `ShareX.ScreenCaptureLib/ScreenRecording/ScreenRecordingOptions.cs`: the
//! FFmpeg command line ShareX's FFmpeg recorder runs, and the two-pass and
//! GIF ones (`ScreenRecorder.FFmpegEncodeAsGIF`, `FFmpegCLIManager
//! .ConcatenateVideos`). Every string is built exactly as ShareX builds it;
//! `tools/recording-harness` checks that against ShareX's own DLLs.

use sharex_settings::ffmpeg_options::{
    FFmpegAMFQuality, FFmpegAMFUsage, FFmpegAudioCodec, FFmpegNVENCPreset, FFmpegNVENCTune, FFmpegPaletteGenStatsMode,
    FFmpegPaletteUseDither, FFmpegPreset, FFmpegQSVPreset, FFmpegVideoCodec,
};
use sharex_settings::types::Rectangle;
use sharex_settings::{FFmpegCaptureDevice, FFmpegOptions};

use crate::dotnet::{change_extension, escape_cli_text, format_float_0_0, replace_ordinal_ignore_case};

/// `ShareX.HelpersLib.DesktopScreen`, as far as ddagrab needs it: a
/// monitor's bounds in physical desktop pixels and whether it's primary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DesktopScreen {
    pub bounds: Rectangle,
    pub primary: bool,
}

/// `ShareX.ScreenCaptureLib.ScreenRecordingOptions`.
#[derive(Debug, Clone, PartialEq)]
pub struct ScreenRecordingOptions {
    /// Recording from the screen/devices; otherwise encoding `input_path`.
    pub is_recording: bool,
    /// The two-pass first pass: lossless x264 into an .mp4.
    pub is_lossless: bool,
    /// `None` is .NET's null (written as an empty string).
    pub input_path: Option<String>,
    pub output_path: Option<String>,
    pub fps: i32,
    pub capture_area: Rectangle,
    /// Seconds; 0 records until stopped.
    pub duration: f32,
    pub draw_cursor: bool,
    pub ffmpeg: FFmpegOptions,
}

impl Default for ScreenRecordingOptions {
    fn default() -> Self {
        Self {
            is_recording: false,
            is_lossless: false,
            input_path: None,
            output_path: None,
            fps: 0,
            capture_area: Rectangle::default(),
            duration: 0.0,
            draw_cursor: false,
            ffmpeg: FFmpegOptions::default(),
        }
    }
}

/// `Rectangle.Intersect`.
pub fn intersect(a: Rectangle, b: Rectangle) -> Rectangle {
    let x1 = a.x.max(b.x);
    let x2 = (a.x + a.width).min(b.x + b.width);
    let y1 = a.y.max(b.y);
    let y2 = (a.y + a.height).min(b.y + b.height);
    if x2 >= x1 && y2 >= y1 { Rectangle::new(x1, y1, x2 - x1, y2 - y1) } else { Rectangle::default() }
}

/// `CaptureHelpers.EvenRectangleSize`: width and height rounded down to
/// even numbers.
pub fn even_rectangle_size(mut r: Rectangle) -> Rectangle {
    r.width -= r.width & 1;
    r.height -= r.height & 1;
    r
}

/// The registry values ShareX writes under
/// `HKCU\Software\screen-capture-recorder` before recording from its own
/// DirectShow screen device, which reads them on start.
pub const SCREEN_CAPTURE_RECORDER_REGISTRY_PATH: &str = "Software\\screen-capture-recorder";

/// What a settings value that ShareX couldn't read means: it keeps the
/// property's default (`Serializer_Error`), while ShareXrs keeps the text.
fn known<T: Clone + PartialEq>(value: &T, unknown: impl Fn(&T) -> bool, default: T) -> T {
    if unknown(value) { default } else { value.clone() }
}

impl ScreenRecordingOptions {
    /// The options as ShareX would hold them: enum values it can't read
    /// fall back to their defaults.
    pub(crate) fn effective_ffmpeg(&self) -> FFmpegOptions {
        let f = &self.ffmpeg;
        let d = FFmpegOptions::default();
        FFmpegOptions {
            VideoCodec: known(&f.VideoCodec, |v| matches!(v, FFmpegVideoCodec::Unknown(_)), d.VideoCodec),
            AudioCodec: known(&f.AudioCodec, |v| matches!(v, FFmpegAudioCodec::Unknown(_)), d.AudioCodec),
            x264_Preset: known(&f.x264_Preset, |v| matches!(v, FFmpegPreset::Unknown(_)), d.x264_Preset),
            NVENC_Preset: known(&f.NVENC_Preset, |v| matches!(v, FFmpegNVENCPreset::Unknown(_)), d.NVENC_Preset),
            NVENC_Tune: known(&f.NVENC_Tune, |v| matches!(v, FFmpegNVENCTune::Unknown(_)), d.NVENC_Tune),
            GIFStatsMode: known(&f.GIFStatsMode, |v| matches!(v, FFmpegPaletteGenStatsMode::Unknown(_)), d.GIFStatsMode),
            GIFDither: known(&f.GIFDither, |v| matches!(v, FFmpegPaletteUseDither::Unknown(_)), d.GIFDither),
            AMF_Usage: known(&f.AMF_Usage, |v| matches!(v, FFmpegAMFUsage::Unknown(_)), d.AMF_Usage),
            AMF_Quality: known(&f.AMF_Quality, |v| matches!(v, FFmpegAMFQuality::Unknown(_)), d.AMF_Quality),
            QSV_Preset: known(&f.QSV_Preset, |v| matches!(v, FFmpegQSVPreset::Unknown(_)), d.QSV_Preset),
            ..f.clone()
        }
    }

    /// Whether the video source is ShareX's screen-capture-recorder device,
    /// which takes its region from the registry
    /// ([`Self::screen_capture_recorder_registry`]).
    pub fn uses_screen_capture_recorder(&self) -> bool {
        self.is_recording
            && !self.ffmpeg.VideoSource.is_empty()
            && self.ffmpeg.VideoSource.eq_ignore_ascii_case(FFmpegCaptureDevice::SCREEN_CAPTURE_RECORDER.value)
    }

    /// The DWORDs `GetFFmpegCommands` writes under
    /// [`SCREEN_CAPTURE_RECORDER_REGISTRY_PATH`] first, in ShareX's order;
    /// empty unless [`Self::uses_screen_capture_recorder`].
    pub fn screen_capture_recorder_registry(&self) -> Vec<(&'static str, i32)> {
        if !self.uses_screen_capture_recorder() {
            return Vec::new();
        }
        let a = self.capture_area;
        vec![
            ("start_x", a.x),
            ("start_y", a.y),
            ("capture_width", a.width),
            ("capture_height", a.height),
            ("default_max_fps", 60),
            ("capture_mouse_default_1", i32::from(self.draw_cursor)),
        ]
    }

    /// `GetFFmpegCommands`: the user's custom command line with its
    /// placeholders filled in, or [`Self::get_ffmpeg_args`]; trimmed.
    /// `None` where ShareX would throw (recording with neither source).
    ///
    /// ShareX writes [`Self::screen_capture_recorder_registry`] here too;
    /// [`crate::recorder::ScreenRecorder`] does that before running FFmpeg.
    pub fn get_ffmpeg_commands(&self, screens: &[DesktopScreen]) -> Option<String> {
        let commands = match self.custom_commands() {
            Some(c) => c,
            None => self.get_ffmpeg_args(false, screens)?,
        };
        Some(commands.trim().to_string())
    }

    /// The custom command line with its placeholders filled in, when it
    /// applies (on, not empty, not the lossless first pass); untrimmed.
    pub fn custom_commands(&self) -> Option<String> {
        let f = &self.ffmpeg;
        if !self.is_lossless && f.UseCustomCommands && !f.CustomCommands.is_empty() {
            let mut c = f.CustomCommands.clone();
            let a = self.capture_area;
            let output = change_extension(self.output_path.as_deref(), f.extension()).unwrap_or_default();
            for (from, to) in [
                ("$fps$", self.fps.to_string()),
                ("$area_x$", a.x.to_string()),
                ("$area_y$", a.y.to_string()),
                ("$area_width$", a.width.to_string()),
                ("$area_height$", a.height.to_string()),
                ("$cursor$", if self.draw_cursor { "1" } else { "0" }.to_string()),
                ("$duration$", format_float_0_0(self.duration)),
                ("$output$", output),
            ] {
                c = replace_ordinal_ignore_case(&c, from, &to);
            }
            Some(c)
        } else {
            None
        }
    }

    /// `GetFFmpegArgs(isCustom)`: the command line from the options. With
    /// `is_custom`, the frame rate, area, cursor, duration and output are
    /// `$fps$`-style placeholders (the starting point ShareX gives the
    /// custom command box). `screens` are the monitors, for ddagrab.
    pub fn get_ffmpeg_args(&self, is_custom: bool, screens: &[DesktopScreen]) -> Option<String> {
        let ffmpeg = self.effective_ffmpeg();
        let f = &ffmpeg;
        if self.is_recording && !f.is_video_source_selected() && !f.is_audio_source_selected() {
            return None;
        }

        let mut args = String::new();
        let framerate = if is_custom { "$fps$".to_string() } else { self.fps.to_string() };

        if self.is_recording {
            if f.is_video_source_selected() {
                if f.VideoSource.eq_ignore_ascii_case(FFmpegCaptureDevice::GDI_GRAB.value) {
                    if f.is_audio_source_selected() {
                        append_input_device(&mut args, "dshow", true);
                        args.push_str(&format!("-i audio={} ", escape_cli_text(&f.AudioSource)));
                    }
                    let a = self.capture_area;
                    let (x, y, width, height, cursor) = if is_custom {
                        ("$area_x$".into(), "$area_y$".into(), "$area_width$".into(), "$area_height$".into(), "$cursor$".into())
                    } else {
                        (
                            a.x.to_string(),
                            a.y.to_string(),
                            a.width.to_string(),
                            a.height.to_string(),
                            if self.draw_cursor { "1" } else { "0" }.to_string(),
                        )
                    };
                    // https://ffmpeg.org/ffmpeg-devices.html#gdigrab
                    append_input_device(&mut args, "gdigrab", false);
                    args.push_str(&format!("-framerate {framerate} "));
                    args.push_str(&format!("-offset_x {x} "));
                    args.push_str(&format!("-offset_y {y} "));
                    args.push_str(&format!("-video_size {width}x{height} "));
                    args.push_str(&format!("-draw_mouse {cursor} "));
                    args.push_str("-i desktop ");
                } else if f.VideoSource.eq_ignore_ascii_case(FFmpegCaptureDevice::DDA_GRAB.value) {
                    if f.is_audio_source_selected() {
                        append_input_device(&mut args, "dshow", true);
                        args.push_str(&format!("-i audio={} ", escape_cli_text(&f.AudioSource)));
                    }
                    let (monitor_index, mut area) = ddagrab_area(self.capture_area, screens);
                    if f.is_even_size_required() {
                        area = even_rectangle_size(area);
                    }
                    // https://ffmpeg.org/ffmpeg-filters.html#ddagrab
                    append_input_device(&mut args, "lavfi", false);
                    args.push_str("-i ddagrab=");
                    args.push_str(&format!("output_idx={monitor_index}:"));
                    args.push_str(&format!("draw_mouse={}:", self.draw_cursor));
                    args.push_str(&format!("framerate={framerate}:"));
                    args.push_str(&format!("offset_x={}:", area.x));
                    args.push_str(&format!("offset_y={}:", area.y));
                    args.push_str(&format!("video_size={}x{}:", area.width, area.height));
                    args.push_str("output_fmt=bgra");
                    if f.VideoCodec != FFmpegVideoCodec::h264_nvenc && f.VideoCodec != FFmpegVideoCodec::hevc_nvenc {
                        args.push_str(",hwdownload");
                        args.push_str(",format=bgra");
                    }
                    args.push(' ');
                } else {
                    // https://ffmpeg.org/ffmpeg-devices.html#dshow
                    append_input_device(&mut args, "dshow", f.is_audio_source_selected());
                    args.push_str(&format!("-framerate {framerate} "));
                    args.push_str(&format!("-i video={}", escape_cli_text(&f.VideoSource)));
                    if f.is_audio_source_selected() {
                        args.push_str(&format!(":audio={} ", escape_cli_text(&f.AudioSource)));
                    } else {
                        args.push(' ');
                    }
                }
            } else if f.is_audio_source_selected() {
                append_input_device(&mut args, "dshow", true);
                args.push_str(&format!("-i audio={} ", escape_cli_text(&f.AudioSource)));
            }
        } else {
            args.push_str(&format!("-i \"{}\" ", self.input_path.as_deref().unwrap_or("")));
        }

        args.push_str(&self.output_args(f, &framerate, is_custom));
        Some(args)
    }

    /// Everything after the inputs: the user's arguments, the codecs, the
    /// duration and the output. Shared with [`crate::platform`]'s inputs.
    pub(crate) fn output_args(&self, f: &FFmpegOptions, framerate: &str, is_custom: bool) -> String {
        let mut args = String::new();
        if !f.UserArgs.is_empty() {
            args.push_str(&f.UserArgs);
            args.push(' ');
        }

        if f.is_video_source_selected() {
            append_video_codec(&mut args, f, self.is_lossless, self.is_recording, framerate);
        }

        if f.is_audio_source_selected() {
            match f.AudioCodec {
                // http://trac.ffmpeg.org/wiki/Encode/AAC; -ac 2 required otherwise failing with 7.1
                FFmpegAudioCodec::libvoaacenc => args.push_str(&format!("-c:a aac -ac 2 -b:a {}k ", f.AAC_Bitrate)),
                FFmpegAudioCodec::libopus => args.push_str(&format!("-c:a libopus -b:a {}k ", f.Opus_Bitrate)),
                FFmpegAudioCodec::libvorbis => args.push_str(&format!("-c:a libvorbis -qscale:a {} ", f.Vorbis_QScale)),
                FFmpegAudioCodec::libmp3lame => args.push_str(&format!("-c:a libmp3lame -qscale:a {} ", f.MP3_QScale)),
                FFmpegAudioCodec::Unknown(_) => {}
            }
        }

        if self.duration > 0.0 {
            let duration = if is_custom { "$duration$".to_string() } else { format_float_0_0(self.duration) };
            args.push_str(&format!("-t {duration} "));
        }

        args.push_str("-y ");

        let output = if is_custom {
            "$output$".to_string()
        } else {
            let ext = if self.is_lossless { "mp4" } else { f.extension() };
            change_extension(self.output_path.as_deref(), ext).unwrap_or_default()
        };
        args.push_str(&format!("\"{output}\""));

        args
    }
}

/// The video codec part of `GetFFmpegArgs`.
fn append_video_codec(args: &mut String, f: &FFmpegOptions, is_lossless: bool, is_recording: bool, framerate: &str) {
    if is_lossless || f.VideoCodec != FFmpegVideoCodec::apng {
        let video_codec = if is_lossless {
            "libx264"
        } else if f.VideoCodec == FFmpegVideoCodec::libvpx_vp9 {
            "libvpx-vp9"
        } else {
            f.VideoCodec.name()
        };
        args.push_str(&format!("-c:v {video_codec} "));
        args.push_str(&format!("-r {framerate} ")); // output FPS
    }

    if is_lossless {
        args.push_str(&format!("-preset {} ", FFmpegPreset::ultrafast.name()));
        args.push_str("-tune zerolatency ");
        args.push_str("-qp 0 ");
        return;
    }

    match f.VideoCodec {
        // https://trac.ffmpeg.org/wiki/Encode/H.264, https://trac.ffmpeg.org/wiki/Encode/H.265
        FFmpegVideoCodec::libx264 | FFmpegVideoCodec::libx265 => {
            args.push_str(&format!("-preset {} ", f.x264_Preset.name()));
            if is_recording {
                args.push_str("-tune zerolatency ");
            }
            if f.x264_Use_Bitrate {
                args.push_str(&format!("-b:v {}k ", f.x264_Bitrate));
            } else {
                args.push_str(&format!("-crf {} ", f.x264_CRF));
            }
            args.push_str("-pix_fmt yuv420p ");
            args.push_str("-movflags +faststart ");
        }
        // https://trac.ffmpeg.org/wiki/Encode/VP8, https://trac.ffmpeg.org/wiki/Encode/VP9
        FFmpegVideoCodec::libvpx | FFmpegVideoCodec::libvpx_vp9 => {
            if is_recording {
                args.push_str("-deadline realtime ");
            }
            args.push_str(&format!("-b:v {}k ", f.VPx_Bitrate));
            args.push_str("-pix_fmt yuv420p ");
        }
        // https://trac.ffmpeg.org/wiki/Encode/MPEG-4
        FFmpegVideoCodec::libxvid => args.push_str(&format!("-qscale:v {} ", f.XviD_QScale)),
        // https://trac.ffmpeg.org/wiki/HWAccelIntro#NVENC
        FFmpegVideoCodec::h264_nvenc | FFmpegVideoCodec::hevc_nvenc => {
            args.push_str(&format!("-preset {} ", f.NVENC_Preset.name()));
            args.push_str(&format!("-tune {} ", f.NVENC_Tune.name()));
            args.push_str(&format!("-b:v {}k ", f.NVENC_Bitrate));
            args.push_str("-movflags +faststart ");
        }
        FFmpegVideoCodec::h264_amf | FFmpegVideoCodec::hevc_amf => {
            args.push_str(&format!("-usage {} ", f.AMF_Usage.name()));
            args.push_str(&format!("-quality {} ", f.AMF_Quality.name()));
            args.push_str(&format!("-b:v {}k ", f.AMF_Bitrate));
            args.push_str("-pix_fmt yuv420p ");
        }
        // https://trac.ffmpeg.org/wiki/Hardware/QuickSync
        FFmpegVideoCodec::h264_qsv | FFmpegVideoCodec::hevc_qsv => {
            args.push_str(&format!("-preset {} ", f.QSV_Preset.name()));
            args.push_str(&format!("-b:v {}k ", f.QSV_Bitrate));
        }
        // https://www.ffmpeg.org/ffmpeg-codecs.html#libwebp
        FFmpegVideoCodec::libwebp => {
            args.push_str("-lossless 0 ");
            args.push_str("-preset default ");
            args.push_str("-loop 0 ");
        }
        FFmpegVideoCodec::apng => {
            args.push_str("-f apng ");
            args.push_str("-plays 0 ");
        }
        FFmpegVideoCodec::gif | FFmpegVideoCodec::Unknown(_) => {}
    }

    // https://trac.ffmpeg.org/wiki/Encode/H.265#FinalCutandApplestuffcompatibility
    if matches!(
        f.VideoCodec,
        FFmpegVideoCodec::libx265 | FFmpegVideoCodec::hevc_nvenc | FFmpegVideoCodec::hevc_amf | FFmpegVideoCodec::hevc_qsv
    ) {
        args.push_str("-tag:v hvc1 ");
    }
}

/// `AppendInputDevice`.
fn append_input_device(args: &mut String, input_device: &str, audio_source: bool) {
    args.push_str(&format!("-f {input_device} "));
    // The maximum number of queued packets when reading from the file or device.
    args.push_str("-thread_queue_size 1024 ");
    // The default real time buffer size is 3041280 (3M).
    args.push_str("-rtbufsize 256M ");
    if audio_source {
        // The audio device's buffer in milliseconds (latency).
        args.push_str("-audio_buffer_size 80 ");
    }
}

/// ddagrab's monitor and the area on it: the monitor (primary first, then
/// in order) the capture area covers most, the area relative to it.
/// With no monitors, ShareX would throw; this gives monitor 0, nothing.
pub fn ddagrab_area(capture_area: Rectangle, screens: &[DesktopScreen]) -> (usize, Rectangle) {
    let mut ordered: Vec<DesktopScreen> = screens.to_vec();
    // OrderBy(x => !x.Primary): stable, primary first.
    ordered.sort_by_key(|s| !s.primary);
    let mut monitor_index = 0;
    let mut area = ordered.first().map(|s| s.bounds).unwrap_or_default();
    let mut max_area = 0;
    for (i, screen) in ordered.iter().enumerate() {
        let inter = intersect(screen.bounds, capture_area);
        let inter_area = inter.width.wrapping_mul(inter.height);
        if inter_area > max_area {
            max_area = inter_area;
            monitor_index = i;
            area = Rectangle::new(inter.x - screen.bounds.x, inter.y - screen.bounds.y, inter.width, inter.height);
        }
    }
    (monitor_index, area)
}

/// `ScreenRecorder.FFmpegEncodeAsGIF`'s arguments: one pass through
/// palettegen and paletteuse.
pub fn gif_encode_args(input: &str, output: &str, f: &FFmpegOptions) -> String {
    let stats = known(&f.GIFStatsMode, |v| matches!(v, FFmpegPaletteGenStatsMode::Unknown(_)), FFmpegPaletteGenStatsMode::full);
    let dither = known(&f.GIFDither, |v| matches!(v, FFmpegPaletteUseDither::Unknown(_)), FFmpegPaletteUseDither::sierra2_4a);
    let mut args = String::new();
    args.push_str(&format!("-i \"{input}\" "));
    // https://ffmpeg.org/ffmpeg-filters.html#palettegen-1
    args.push_str(&format!("-lavfi \"palettegen=stats_mode={}[palette],", stats.name()));
    // https://ffmpeg.org/ffmpeg-filters.html#paletteuse
    args.push_str(&format!("[0:v][palette]paletteuse=dither={}", dither.name()));
    if dither == FFmpegPaletteUseDither::bayer {
        args.push_str(&format!(":bayer_scale={}", f.GIFBayerScale));
    }
    if stats == FFmpegPaletteGenStatsMode::single {
        args.push_str(":new=1");
    }
    args.push_str("\" ");
    args.push_str("-y ");
    args.push_str(&format!("\"{output}\""));
    args
}

/// `FFmpegCLIManager.ConcatenateVideos`: the list file it writes
/// (`<output>.txt`) and the arguments.
pub fn concat_list(input_files: &[&str]) -> String {
    input_files.iter().map(|f| format!("file '{f}'")).collect::<Vec<_>>().join(crate::dotnet::NEW_LINE)
}

pub fn concat_args(list_file: &str, output_file: &str) -> String {
    format!("-f concat -safe 0 -i \"{list_file}\" -c copy \"{output_file}\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recording() -> ScreenRecordingOptions {
        ScreenRecordingOptions {
            is_recording: true,
            fps: 30,
            capture_area: Rectangle::new(10, 20, 640, 480),
            draw_cursor: true,
            output_path: Some(r"C:\out\rec.mp4".into()),
            ..Default::default()
        }
    }

    #[test]
    fn default_gdigrab_x264() {
        assert_eq!(
            recording().get_ffmpeg_commands(&[]).unwrap(),
            r#"-f gdigrab -thread_queue_size 1024 -rtbufsize 256M -framerate 30 -offset_x 10 -offset_y 20 -video_size 640x480 -draw_mouse 1 -i desktop -c:v libx264 -r 30 -preset ultrafast -tune zerolatency -crf 28 -pix_fmt yuv420p -movflags +faststart -y "C:\out\rec.mp4""#
        );
    }

    #[test]
    fn no_source_is_none() {
        let mut o = recording();
        o.ffmpeg.VideoSource.clear();
        assert_eq!(o.get_ffmpeg_commands(&[]), None);
    }

    #[test]
    fn ddagrab_picks_the_most_covered_monitor() {
        let screens = [
            DesktopScreen { bounds: Rectangle::new(-1920, 0, 1920, 1080), primary: false },
            DesktopScreen { bounds: Rectangle::new(0, 0, 2560, 1440), primary: true },
        ];
        assert_eq!(ddagrab_area(Rectangle::new(-1000, 100, 1200, 500), &screens), (1, Rectangle::new(920, 100, 1000, 500)));
        assert_eq!(ddagrab_area(Rectangle::new(-100, 100, 1200, 500), &screens), (0, Rectangle::new(0, 100, 1100, 500)));
    }
}
