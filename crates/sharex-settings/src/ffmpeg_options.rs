//! `ShareX.ScreenCaptureLib/ScreenRecording/FFmpegOptions.cs`,
//! `FFmpegCaptureDevice.cs` and the FFmpeg enums of
//! `ShareX.ScreenCaptureLib/Enums.cs`: how ShareX's FFmpeg screen recorder
//! records, as `TaskSettingsCapture.FFmpegOptions` stores it. The recorder
//! itself is in `sharex-screen-recording`.

use crate::{dotnet_enum, settings};

dotnet_enum! {
    /// `ShareX.ScreenCaptureLib.FFmpegVideoCodec`.
    pub enum FFmpegVideoCodec {
        libx264, libx265, libvpx, libvpx_vp9, libxvid, h264_nvenc, hevc_nvenc, h264_amf, hevc_amf,
        h264_qsv, hevc_qsv, gif, libwebp, apng,
    }
}

dotnet_enum! {
    /// `ShareX.ScreenCaptureLib.FFmpegAudioCodec`.
    pub enum FFmpegAudioCodec { libvoaacenc, libopus, libvorbis, libmp3lame }
}

dotnet_enum! {
    /// `ShareX.ScreenCaptureLib.FFmpegPreset` (x264/x265 `-preset`).
    pub enum FFmpegPreset { ultrafast, superfast, veryfast, faster, fast, medium, slow, slower, veryslow, placebo }
}

dotnet_enum! {
    /// `ShareX.ScreenCaptureLib.FFmpegTune`.
    pub enum FFmpegTune { film, animation, grain, stillimage, psnr, ssim, fastdecode, zerolatency }
}

dotnet_enum! {
    /// `ShareX.ScreenCaptureLib.FFmpegNVENCPreset`.
    pub enum FFmpegNVENCPreset { p1, p2, p3, p4, p5, p6, p7 }
}

dotnet_enum! {
    /// `ShareX.ScreenCaptureLib.FFmpegNVENCTune`.
    pub enum FFmpegNVENCTune { hq, ll, ull, lossless }
}

dotnet_enum! {
    /// `ShareX.ScreenCaptureLib.FFmpegAMFUsage`.
    pub enum FFmpegAMFUsage { transcoding, ultralowlatency, lowlatency, webcam, high_quality, lowlatency_high_quality }
}

dotnet_enum! {
    /// `ShareX.ScreenCaptureLib.FFmpegAMFQuality`.
    pub enum FFmpegAMFQuality { speed, balanced, quality }
}

dotnet_enum! {
    /// `ShareX.ScreenCaptureLib.FFmpegQSVPreset`.
    pub enum FFmpegQSVPreset { veryfast, faster, fast, medium, slow, slower, veryslow }
}

dotnet_enum! {
    /// `ShareX.ScreenCaptureLib.FFmpegPaletteGenStatsMode`.
    pub enum FFmpegPaletteGenStatsMode { full, diff, single }
}

dotnet_enum! {
    /// `ShareX.ScreenCaptureLib.FFmpegPaletteUseDither`.
    pub enum FFmpegPaletteUseDither { none, bayer, heckbert, floyd_steinberg, sierra2, sierra2_4a, sierra3, burkes, atkinson }
}

impl FFmpegVideoCodec {
    /// `[Description]`, what ShareX's codec combo box shows.
    pub fn description(&self) -> &str {
        match self {
            Self::libx264 => "H.264 / x264",
            Self::libx265 => "H.265 / x265",
            Self::libvpx => "VP8",
            Self::libvpx_vp9 => "VP9",
            Self::libxvid => "MPEG-4 / Xvid",
            Self::h264_nvenc => "H.264 / NVENC",
            Self::hevc_nvenc => "HEVC / NVENC",
            Self::h264_amf => "H.264 / AMF",
            Self::hevc_amf => "HEVC / AMF",
            Self::h264_qsv => "H.264 / Quick Sync",
            Self::hevc_qsv => "HEVC / Quick Sync",
            Self::gif => "GIF",
            Self::libwebp => "WebP",
            Self::apng => "APNG",
            Self::Unknown(s) => s,
        }
    }
}

impl FFmpegAudioCodec {
    /// `[Description]`.
    pub fn description(&self) -> &str {
        match self {
            Self::libvoaacenc => "AAC",
            Self::libopus => "Opus",
            Self::libvorbis => "Vorbis",
            Self::libmp3lame => "MP3",
            Self::Unknown(s) => s,
        }
    }
}

/// `ShareX.ScreenCaptureLib.FFmpegCaptureDevice`: a source ShareX offers
/// besides the DirectShow devices FFmpeg lists. `value` is what
/// `FFmpegOptions.VideoSource`/`AudioSource` store.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FFmpegCaptureDevice {
    pub value: &'static str,
    pub title: &'static str,
}

impl FFmpegCaptureDevice {
    pub const NONE: Self = Self { value: "", title: "None" };
    pub const GDI_GRAB: Self = Self { value: "gdigrab", title: "gdigrab (Graphics Device Interface)" };
    pub const DDA_GRAB: Self = Self { value: "ddagrab", title: "ddagrab (Desktop Duplication API)" };
    /// ShareX's own DirectShow screen device (`ShareX/RecorderDevices`).
    pub const SCREEN_CAPTURE_RECORDER: Self =
        Self { value: "screen-capture-recorder", title: "dshow (screen-capture-recorder)" };
    /// ShareX's own DirectShow loopback audio device (`ShareX/RecorderDevices`).
    pub const VIRTUAL_AUDIO_CAPTURER: Self =
        Self { value: "virtual-audio-capturer", title: "dshow (virtual-audio-capturer)" };
}

settings! {
    /// `ShareX.ScreenCaptureLib.FFmpegOptions`.
    pub struct FFmpegOptions {
        // General
        OverrideCLIPath: bool = false,
        CLIPath: String = String::new(),
        VideoSource: String = FFmpegCaptureDevice::GDI_GRAB.value.into(),
        AudioSource: String = FFmpegCaptureDevice::NONE.value.into(),
        VideoCodec: FFmpegVideoCodec = FFmpegVideoCodec::libx264,
        AudioCodec: FFmpegAudioCodec = FFmpegAudioCodec::libvoaacenc,
        UserArgs: String = String::new(),
        UseCustomCommands: bool = false,
        CustomCommands: String = String::new(),
        // Video
        x264_Preset: FFmpegPreset = FFmpegPreset::ultrafast,
        x264_CRF: i32 = 28,
        x264_Use_Bitrate: bool = false,
        /// kbps.
        x264_Bitrate: i32 = 3000,
        /// kbps.
        VPx_Bitrate: i32 = 3000,
        XviD_QScale: i32 = 10,
        NVENC_Preset: FFmpegNVENCPreset = FFmpegNVENCPreset::p4,
        NVENC_Tune: FFmpegNVENCTune = FFmpegNVENCTune::ll,
        /// kbps.
        NVENC_Bitrate: i32 = 3000,
        GIFStatsMode: FFmpegPaletteGenStatsMode = FFmpegPaletteGenStatsMode::full,
        GIFDither: FFmpegPaletteUseDither = FFmpegPaletteUseDither::sierra2_4a,
        GIFBayerScale: i32 = 2,
        AMF_Usage: FFmpegAMFUsage = FFmpegAMFUsage::lowlatency,
        AMF_Quality: FFmpegAMFQuality = FFmpegAMFQuality::speed,
        /// kbps.
        AMF_Bitrate: i32 = 3000,
        QSV_Preset: FFmpegQSVPreset = FFmpegQSVPreset::fast,
        /// kbps.
        QSV_Bitrate: i32 = 3000,
        // Audio
        /// kbps.
        AAC_Bitrate: i32 = 128,
        /// kbps.
        Opus_Bitrate: i32 = 128,
        Vorbis_QScale: i32 = 3,
        MP3_QScale: i32 = 4,
    }
}

impl FFmpegOptions {
    /// `FFmpegOptions.Extension`: the output's extension for the codec (or,
    /// without a video source, the audio codec).
    pub fn extension(&self) -> &'static str {
        if !self.VideoSource.is_empty() {
            match self.VideoCodec {
                FFmpegVideoCodec::libx264
                | FFmpegVideoCodec::libx265
                | FFmpegVideoCodec::h264_nvenc
                | FFmpegVideoCodec::hevc_nvenc
                | FFmpegVideoCodec::h264_amf
                | FFmpegVideoCodec::hevc_amf
                | FFmpegVideoCodec::h264_qsv
                | FFmpegVideoCodec::hevc_qsv => return "mp4",
                FFmpegVideoCodec::libvpx | FFmpegVideoCodec::libvpx_vp9 => return "webm",
                FFmpegVideoCodec::libxvid => return "avi",
                FFmpegVideoCodec::gif => return "gif",
                FFmpegVideoCodec::libwebp => return "webp",
                FFmpegVideoCodec::apng => return "apng",
                FFmpegVideoCodec::Unknown(_) => {}
            }
        } else if !self.AudioSource.is_empty() {
            match self.AudioCodec {
                FFmpegAudioCodec::libvoaacenc => return "m4a",
                FFmpegAudioCodec::libopus => return "opus",
                FFmpegAudioCodec::libvorbis => return "ogg",
                FFmpegAudioCodec::libmp3lame => return "mp3",
                FFmpegAudioCodec::Unknown(_) => {}
            }
        }
        "mp4"
    }

    pub fn is_source_selected(&self) -> bool {
        self.is_video_source_selected() || self.is_audio_source_selected()
    }

    pub fn is_video_source_selected(&self) -> bool {
        !self.VideoSource.is_empty()
    }

    /// Animated images (GIF, WebP, APNG) have no sound, so a selected audio
    /// source only counts without one.
    pub fn is_audio_source_selected(&self) -> bool {
        !self.AudioSource.is_empty() && (!self.is_video_source_selected() || !self.is_animated_image())
    }

    pub fn is_animated_image(&self) -> bool {
        matches!(self.VideoCodec, FFmpegVideoCodec::gif | FFmpegVideoCodec::libwebp | FFmpegVideoCodec::apng)
    }

    /// Video encoders want even sizes; animated images don't.
    pub fn is_even_size_required(&self) -> bool {
        !self.is_animated_image()
    }

    /// `FixSources` ("TEMP: For backward compatibility"): the names ShareX
    /// stored before 15.0.1.
    pub fn fix_sources(&mut self) {
        if self.VideoSource.eq_ignore_ascii_case("None") {
            self.VideoSource = FFmpegCaptureDevice::NONE.value.into();
        } else if self.VideoSource.eq_ignore_ascii_case("GDI grab") {
            self.VideoSource = FFmpegCaptureDevice::GDI_GRAB.value.into();
        }
        if self.AudioSource.eq_ignore_ascii_case("None") {
            self.AudioSource = FFmpegCaptureDevice::NONE.value.into();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What ShareX writes for a new `FFmpegOptions` (fresh profile).
    const SHAREX_DEFAULT: &str = r#"{"OverrideCLIPath":false,"CLIPath":"","VideoSource":"gdigrab","AudioSource":"","VideoCodec":"libx264","AudioCodec":"libvoaacenc","UserArgs":"","UseCustomCommands":false,"CustomCommands":"","x264_Preset":"ultrafast","x264_CRF":28,"x264_Use_Bitrate":false,"x264_Bitrate":3000,"VPx_Bitrate":3000,"XviD_QScale":10,"NVENC_Preset":"p4","NVENC_Tune":"ll","NVENC_Bitrate":3000,"GIFStatsMode":"full","GIFDither":"sierra2_4a","GIFBayerScale":2,"AMF_Usage":"lowlatency","AMF_Quality":"speed","AMF_Bitrate":3000,"QSV_Preset":"fast","QSV_Bitrate":3000,"AAC_Bitrate":128,"Opus_Bitrate":128,"Vorbis_QScale":3,"MP3_QScale":4}"#;

    #[test]
    fn default_is_sharex_default() {
        assert_eq!(serde_json::to_string(&FFmpegOptions::default()).unwrap(), SHAREX_DEFAULT);
        let read: FFmpegOptions = serde_json::from_str(SHAREX_DEFAULT).unwrap();
        assert_eq!(read, FFmpegOptions::default());
    }

    #[test]
    fn extension_and_sources() {
        let mut o = FFmpegOptions::default();
        assert_eq!(o.extension(), "mp4");
        o.VideoCodec = FFmpegVideoCodec::libvpx_vp9;
        assert_eq!(o.extension(), "webm");
        o.VideoSource.clear();
        o.AudioSource = "Microphone".into();
        o.AudioCodec = FFmpegAudioCodec::libopus;
        assert_eq!(o.extension(), "opus");
        o.AudioSource.clear();
        assert_eq!(o.extension(), "mp4");
        assert!(!o.is_source_selected());

        let mut o = FFmpegOptions { AudioSource: "Mic".into(), VideoCodec: FFmpegVideoCodec::gif, ..Default::default() };
        assert!(!o.is_audio_source_selected());
        assert!(!o.is_even_size_required());
        o.VideoCodec = FFmpegVideoCodec::libx264;
        assert!(o.is_audio_source_selected());
    }

    #[test]
    fn fixes_old_sources() {
        let mut o = FFmpegOptions { VideoSource: "GDI grab".into(), AudioSource: "none".into(), ..Default::default() };
        o.fix_sources();
        assert_eq!((o.VideoSource.as_str(), o.AudioSource.as_str()), ("gdigrab", ""));
    }
}
