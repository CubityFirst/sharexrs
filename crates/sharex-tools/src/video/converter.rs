//! Video converter: `VideoConverterOptions` (`TaskSettingsTools.VideoConverterOptions`
//! and its `GetFFmpegArgs`), the window's `VideoConverterViewModel.BuildArguments`,
//! and `VideoConversionOutputInfo.FromFFmpegOutput`.

use regex::Regex;
use serde_json::Value;

use crate::dotnet;
use crate::options::{enum_name, json_bool, json_i32, json_string};

/// `ConverterVideoCodecs` / `VideoConverterCodec` (same order).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Codec {
    X264,
    X265,
    H264Nvenc,
    HevcNvenc,
    H264Amf,
    HevcAmf,
    H264Qsv,
    HevcQsv,
    Vp8,
    Vp9,
    Av1,
    Xvid,
    Gif,
    Webp,
    Apng,
}

impl Codec {
    pub const ALL: [Codec; 15] = [
        Codec::X264,
        Codec::X265,
        Codec::H264Nvenc,
        Codec::HevcNvenc,
        Codec::H264Amf,
        Codec::HevcAmf,
        Codec::H264Qsv,
        Codec::HevcQsv,
        Codec::Vp8,
        Codec::Vp9,
        Codec::Av1,
        Codec::Xvid,
        Codec::Gif,
        Codec::Webp,
        Codec::Apng,
    ];

    /// The JSON names (`ConverterVideoCodecs`).
    pub const NAMES: [&'static str; 15] = [
        "x264", "x265", "h264_nvenc", "hevc_nvenc", "h264_amf", "hevc_amf", "h264_qsv", "hevc_qsv", "vp8", "vp9", "av1", "xvid", "gif",
        "webp", "apng",
    ];

    /// The window's names (`VideoConverterViewModel.Codecs`).
    pub fn display_name(self) -> &'static str {
        match self {
            Codec::X264 => "H.264 / x264",
            Codec::X265 => "H.265 / x265",
            Codec::H264Nvenc => "H.264 / NVENC",
            Codec::HevcNvenc => "HEVC / NVENC",
            Codec::H264Amf => "H.264 / AMF",
            Codec::HevcAmf => "HEVC / AMF",
            Codec::H264Qsv => "H.264 / Quick Sync",
            Codec::HevcQsv => "HEVC / Quick Sync",
            Codec::Vp8 => "VP8",
            Codec::Vp9 => "VP9",
            Codec::Av1 => "AV1",
            Codec::Xvid => "MPEG-4 / Xvid",
            Codec::Gif => "GIF",
            Codec::Webp => "WebP",
            Codec::Apng => "APNG",
        }
    }

    /// `GetFileExtension`.
    pub fn extension(self) -> &'static str {
        match self {
            Codec::Vp8 | Codec::Vp9 => "webm",
            Codec::Av1 => "mkv",
            Codec::Xvid => "avi",
            Codec::Gif => "gif",
            Codec::Webp => "webp",
            Codec::Apng => "apng",
            _ => "mp4",
        }
    }

    fn hevc(self) -> bool {
        matches!(self, Codec::X265 | Codec::HevcNvenc | Codec::HevcAmf | Codec::HevcQsv)
    }

    /// `FFmpegCLIManager`'s quality limits for the codec.
    fn limits(self) -> (i32, i32) {
        match self {
            Codec::Vp8 => (4, 63),
            Codec::Vp9 | Codec::Av1 => (0, 63),
            Codec::Xvid => (1, 31),
            _ => (0, 51),
        }
    }

    /// `ShowsQualityControls`.
    pub fn shows_quality_controls(self) -> bool {
        !matches!(self, Codec::Gif | Codec::Webp | Codec::Apng)
    }

    /// `CanChooseRateControl`: CRF or bitrate.
    pub fn can_choose_rate_control(self) -> bool {
        matches!(self, Codec::X264 | Codec::X265 | Codec::Vp8 | Codec::Vp9 | Codec::Av1 | Codec::Xvid)
    }

    /// The window's quality slider range (`QualityMinimum`/`QualityMaximum`).
    pub fn quality_range(self) -> (f64, f64) {
        let min = match self {
            Codec::Vp8 => 4.0,
            Codec::Xvid => 1.0,
            _ => 0.0,
        };
        let max = match self {
            Codec::Vp8 | Codec::Vp9 | Codec::Av1 => 63.0,
            Codec::Xvid => 31.0,
            _ => 51.0,
        };
        (min, max)
    }
}

const ANIMATION_ONLY: [&str; 4] = ["gif", "webp", "png", "apng"];

/// `VideoConverterOptions`.
#[derive(Debug, Clone, PartialEq)]
pub struct VideoConverterOptions {
    pub input_file_path: Option<String>,
    pub output_folder_path: Option<String>,
    pub output_file_name: Option<String>,
    pub video_codec: Codec,
    pub video_quality: i32,
    pub video_quality_use_bitrate: bool,
    pub video_quality_bitrate: i32,
    pub use_custom_arguments: bool,
    pub custom_arguments: String,
    pub auto_open_folder: bool,
}

impl Default for VideoConverterOptions {
    fn default() -> Self {
        VideoConverterOptions {
            input_file_path: None,
            output_folder_path: None,
            output_file_name: None,
            video_codec: Codec::X264,
            video_quality: 23,
            video_quality_use_bitrate: false,
            video_quality_bitrate: 3000,
            use_custom_arguments: false,
            custom_arguments: String::new(),
            auto_open_folder: true,
        }
    }
}

fn ends_with_ignore_case(s: &str, suffix: &str) -> bool {
    s.len() >= suffix.len() && s.is_char_boundary(s.len() - suffix.len()) && s[s.len() - suffix.len()..].eq_ignore_ascii_case(suffix)
}

impl VideoConverterOptions {
    pub fn from_json(v: &Value) -> Self {
        let d = Self::default();
        let s = |k: &str, def: &Option<String>| json_string(&v[k]).unwrap_or_else(|| def.clone());
        VideoConverterOptions {
            input_file_path: s("InputFilePath", &d.input_file_path),
            output_folder_path: s("OutputFolderPath", &d.output_folder_path),
            output_file_name: s("OutputFileName", &d.output_file_name),
            video_codec: enum_name(&v["VideoCodec"], &Codec::NAMES).map(|i| Codec::ALL[i]).unwrap_or(d.video_codec),
            video_quality: json_i32(&v["VideoQuality"]).unwrap_or(d.video_quality),
            video_quality_use_bitrate: json_bool(&v["VideoQualityUseBitrate"]).unwrap_or(d.video_quality_use_bitrate),
            video_quality_bitrate: json_i32(&v["VideoQualityBitrate"]).unwrap_or(d.video_quality_bitrate),
            use_custom_arguments: json_bool(&v["UseCustomArguments"]).unwrap_or(d.use_custom_arguments),
            custom_arguments: json_string(&v["CustomArguments"]).flatten().unwrap_or(d.custom_arguments),
            auto_open_folder: json_bool(&v["AutoOpenFolder"]).unwrap_or(d.auto_open_folder),
        }
    }

    pub fn to_json(&self) -> Value {
        serde_json::json!({
            "InputFilePath": self.input_file_path,
            "OutputFolderPath": self.output_folder_path,
            "OutputFileName": self.output_file_name,
            "VideoCodec": Codec::NAMES[self.video_codec as usize],
            "VideoQuality": self.video_quality,
            "VideoQualityUseBitrate": self.video_quality_use_bitrate,
            "VideoQualityBitrate": self.video_quality_bitrate,
            "UseCustomArguments": self.use_custom_arguments,
            "CustomArguments": self.custom_arguments,
            "AutoOpenFolder": self.auto_open_folder,
        })
    }

    /// `OutputFilePath`: folder and name, with the codec's extension when the
    /// name has none; empty when either is missing.
    pub fn output_file_path(&self) -> String {
        match (self.output_folder_path.as_deref(), self.output_file_name.as_deref()) {
            (Some(folder), Some(name)) if !folder.is_empty() && !name.is_empty() => {
                let path = dotnet::combine(folder, name);
                if dotnet::has_extension(name) { path } else { dotnet::change_extension(&path, self.video_codec.extension()) }
            }
            _ => String::new(),
        }
    }

    /// `IsInputFileAnimationOnly`.
    pub fn is_input_animation_only(&self) -> bool {
        let input = self.input_file_path.as_deref().unwrap_or("");
        ANIMATION_ONLY.iter().any(|x| ends_with_ignore_case(input, &format!(".{x}")))
    }

    /// `Arguments`: the custom ones, or `GetFFmpegArgs`.
    pub fn arguments(&self) -> String {
        if self.use_custom_arguments { self.custom_arguments.clone() } else { self.ffmpeg_args() }
    }

    /// `GetFFmpegArgs`.
    pub fn ffmpeg_args(&self) -> String {
        let (min, max) = self.video_codec.limits();
        let q = self.video_quality.clamp(min, max);
        let rate = (self.video_quality_use_bitrate, self.video_quality_bitrate);
        arguments(
            self.input_file_path.as_deref().unwrap_or(""),
            &self.output_file_path(),
            self.video_codec,
            rate.0,
            q,
            rate.1,
            self.is_input_animation_only(),
        )
    }
}

/// The argument string both builders share.
fn arguments(input: &str, output: &str, codec: Codec, use_bitrate: bool, quality: i32, bitrate: i32, animation_only: bool) -> String {
    let mut a = format!("-i \"{input}\" ");
    let rate = |crf: String| if use_bitrate { format!("-b:v {bitrate}k ") } else { crf };
    match codec {
        Codec::X264 => {
            a += "-c:v libx264 -preset medium ";
            a += &rate(format!("-crf {quality} "));
            a += "-pix_fmt yuv420p -movflags +faststart ";
        }
        Codec::X265 => {
            a += "-c:v libx265 -preset medium ";
            a += &rate(format!("-crf {quality} "));
        }
        Codec::H264Nvenc => a += &format!("-c:v h264_nvenc -preset p4 -tune hq -profile:v high -b:v {bitrate}k "),
        Codec::HevcNvenc => a += &format!("-c:v hevc_nvenc -preset p4 -tune hq -profile:v main -b:v {bitrate}k "),
        Codec::H264Amf => a += &format!("-c:v h264_amf -usage transcoding -profile main -quality balanced -b:v {bitrate}k "),
        Codec::HevcAmf => a += &format!("-c:v hevc_amf -usage transcoding -profile main -quality balanced -b:v {bitrate}k "),
        Codec::H264Qsv => a += &format!("-c:v h264_qsv -preset medium -b:v {bitrate}k "),
        Codec::HevcQsv => a += &format!("-c:v hevc_qsv -preset medium -b:v {bitrate}k "),
        Codec::Vp8 => {
            a += "-c:v libvpx ";
            a += &rate(format!("-crf {quality} -b:v 100M "));
        }
        Codec::Vp9 => {
            a += "-c:v libvpx-vp9 ";
            a += &rate(format!("-crf {quality} -b:v 0 "));
        }
        Codec::Av1 => {
            a += "-c:v libsvtav1 ";
            a += &rate(format!("-crf {quality} "));
        }
        Codec::Xvid => {
            a += "-c:v libxvid ";
            a += &rate(format!("-q:v {quality} "));
        }
        Codec::Gif => a += "-lavfi \"palettegen=stats_mode=full[palette],[0:v][palette]paletteuse=dither=sierra2_4a\" ",
        Codec::Webp => a += "-c:v libwebp -lossless 0 -preset default -loop 0 ",
        Codec::Apng => a += "-f apng -plays 0 ",
    }
    if codec.hevc() {
        a += "-tag:v hvc1 ";
    }
    if !animation_only {
        match codec {
            Codec::X264 | Codec::X265 | Codec::H264Nvenc | Codec::HevcNvenc | Codec::H264Amf | Codec::HevcAmf | Codec::H264Qsv | Codec::HevcQsv => {
                a += "-c:a aac -b:a 128k "
            }
            Codec::Vp8 | Codec::Vp9 => a += "-c:a libvorbis -q:a 3 ",
            Codec::Av1 => a += "-c:a libopus -b:a 128k ",
            Codec::Xvid => a += "-c:a libmp3lame -q:a 4 ",
            _ => {}
        }
    }
    a += &format!("-y \"{output}\"");
    a
}

/// The window's state that `VideoConverterViewModel.BuildArguments` reads.
#[derive(Debug, Clone, PartialEq)]
pub struct ConverterWindow {
    pub input_file_path: String,
    pub output_folder_path: String,
    pub output_file_name: String,
    pub codec: Codec,
    pub use_bitrate: bool,
    pub video_quality: f64,
    pub video_bitrate: f64,
}

impl ConverterWindow {
    /// The window's `OutputFilePath`.
    pub fn output_file_path(&self) -> String {
        if self.output_folder_path.trim().is_empty() || self.output_file_name.trim().is_empty() {
            return String::new();
        }
        let path = dotnet::combine(&self.output_folder_path, &self.output_file_name);
        if dotnet::has_extension(&self.output_file_name) { path } else { dotnet::change_extension(&path, self.codec.extension()) }
    }

    /// `BuildArguments`.
    pub fn build_arguments(&self) -> String {
        let (min, max) = self.codec.quality_range();
        let quality = (self.video_quality.round_ties_even() as i32).clamp(min as i32, max as i32);
        let bitrate = (self.video_bitrate as i64).max(0) as i32;
        let animation_only = [".gif", ".webp", ".png", ".apng"].iter().any(|x| ends_with_ignore_case(&self.input_file_path, x));
        arguments(&self.input_file_path, &self.output_file_path(), self.codec, self.use_bitrate, quality, bitrate, animation_only)
    }
}

/// `VideoConversionOutputInfo`.
#[derive(Debug, Clone, PartialEq)]
pub struct OutputInfo {
    pub file_size: u64,
    pub average_bitrate_kbps: Option<f64>,
    pub width: Option<i32>,
    pub height: Option<i32>,
}

/// `VideoConversionOutputInfo.FromFFmpegOutput`: the bitrate FFmpeg reports
/// (or file size over duration when it says N/A) and the first video
/// stream's resolution.
pub fn output_info(ffmpeg_output: &str, file_size: u64) -> OutputInfo {
    let duration = Regex::new(r"Duration:\s*(\d+):(\d+):(\d+(?:\.\d+)?)").unwrap();
    let bitrate = Regex::new(r"Duration:.*?bitrate:\s*(\d+(?:\.\d+)?)\s*kb/s").unwrap();
    let resolution = Regex::new(r"Stream #.*?: Video: .*?, (\d+)x(\d+)").unwrap();
    let mut kbps = bitrate.captures(ffmpeg_output).and_then(|m| m[1].parse::<f64>().ok()).filter(|v| *v > 0.0);
    if kbps.is_none() {
        if let Some(m) = duration.captures(ffmpeg_output) {
            let s = m[1].parse::<f64>().unwrap_or(0.0) * 3600.0 + m[2].parse::<f64>().unwrap_or(0.0) * 60.0 + m[3].parse::<f64>().unwrap_or(0.0);
            if s > 0.0 && file_size > 0 {
                kbps = Some(file_size as f64 * 8.0 / 1000.0 / s);
            }
        }
    }
    let (mut width, mut height) = (None, None);
    if let Some(m) = resolution.captures(ffmpeg_output) {
        if let (Ok(w), Ok(h)) = (m[1].parse::<i32>(), m[2].parse::<i32>()) {
            if w > 0 && h > 0 {
                width = Some(w);
                height = Some(h);
            }
        }
    }
    OutputInfo { file_size, average_bitrate_kbps: kbps, width, height }
}

/// `TaskHelpers.GetVideoConversionOutputInfo`'s probe: FFmpeg's stream summary
/// of the output (`-hide_banner -i "<file>"`, which exits with an error).
pub fn probe_arguments(output_file: &str) -> String {
    format!("-hide_banner -i \"{output_file}\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_arguments() {
        let o = VideoConverterOptions {
            input_file_path: Some("in.mov".into()),
            output_folder_path: Some("out".into()),
            output_file_name: Some("clip".into()),
            ..Default::default()
        };
        let sep = if cfg!(windows) { '\\' } else { '/' };
        assert_eq!(
            o.ffmpeg_args(),
            format!("-i \"in.mov\" -c:v libx264 -preset medium -crf 23 -pix_fmt yuv420p -movflags +faststart -c:a aac -b:a 128k -y \"out{sep}clip.mp4\"")
        );
    }

    #[test]
    fn output_info_from_ffmpeg() {
        let text = "  Duration: 00:00:02.00, start: 0.000000, bitrate: N/A\n  Stream #0:0: Video: gif, bgra, 160x90, 10 fps";
        let i = output_info(text, 25000);
        assert_eq!(i.average_bitrate_kbps, Some(100.0));
        assert_eq!((i.width, i.height), (Some(160), Some(90)));
    }
}
