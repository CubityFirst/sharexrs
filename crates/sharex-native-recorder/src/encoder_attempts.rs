//! The video formats `MediaFoundationWriter.Create` tries, in order, until
//! a Media Foundation encoder accepts one (`BuildAttempts`, `FitSize`,
//! `AlignAndPad`), and where the video processor puts the canvas in a
//! frame of another size (`GpuVideoProcessor.ComputeDestRect`). ShareX
//! #9317 (bef6e0e). Pure, so it's tested on every platform; the writer and
//! processor that use it are Windows only.

use std::collections::HashSet;

const H264_MAX_WIDTH: i32 = 4096;
const H264_MAX_HEIGHT: i32 = 2304;
/// H.264 Level 5.2 MaxFS (36,864 MB).
const H264_MAX_PIXELS: i64 = H264_MAX_WIDTH as i64 * H264_MAX_HEIGHT as i64;
/// H.264 Level 5.2 MaxMBPS.
const H264_MAX_PIXELS_PER_SECOND: i64 = 2_073_600 * 256;
const HEVC_MAX_WIDTH: i32 = 8192;
const HEVC_MAX_HEIGHT: i32 = 4352;
/// HEVC Level 6.2 MaxLumaPS.
const HEVC_MAX_PIXELS: i64 = HEVC_MAX_WIDTH as i64 * HEVC_MAX_HEIGHT as i64;
/// HEVC Level 6.2 MaxLumaSR.
const HEVC_MAX_PIXELS_PER_SECOND: i64 = 4_278_190_080;
/// Multiple of 16; satisfies NVENC (145), AMD AMF (128), Intel QSV and MS H.264 minimums.
pub const MIN_ENCODER_WIDTH: i32 = 160;
/// Multiple of 16; satisfies AMD AMF (128), NVENC, Intel QSV and MS H.264 minimums.
pub const MIN_ENCODER_HEIGHT: i32 = 128;
const HIGH_COMPATIBILITY_BITRATE: i32 = 50_000_000;
const COMPATIBILITY_BITRATE: i32 = 20_000_000;
const BASELINE_COMPATIBILITY_BITRATE: i32 = 10_000_000;

/// The video codec of an attempt (`VideoFormatGuids.H264` / `Hevc`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Codec {
    H264,
    Hevc,
}

/// `EncoderAttempt`: one media type to offer the sink writer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EncoderAttempt {
    pub codec: Codec,
    pub width: i32,
    pub height: i32,
    pub frames_per_second: i32,
    pub video_bitrate: i32,
    /// With the optional attributes: H.264/HEVC Main profile, BT.709
    /// colorimetry, `MF_LOW_LATENCY` and the AAC profile level. Without them,
    /// a "minimal media type".
    pub full_media_type: bool,
    /// `MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS`; without it, Windows' software MFT.
    pub enable_hardware_transforms: bool,
}

impl EncoderAttempt {
    /// `AdjustmentNote`: what was negotiated, when it isn't the first
    /// attempt or differs from what was asked for; `None` otherwise.
    pub fn adjustment_note(&self, index: usize, width: i32, height: i32, fps: i32, bitrate: i32) -> Option<String> {
        let unchanged = index == 0
            && self.width == width
            && self.height == height
            && self.frames_per_second == fps
            && self.video_bitrate == bitrate
            && self.codec == Codec::H264
            && self.full_media_type
            && self.enable_hardware_transforms;
        if unchanged {
            return None;
        }
        let codec = if self.codec == Codec::Hevc { "HEVC" } else { "H.264" };
        Some(format!(
            "Encoder negotiated {codec} {}x{} @ {} FPS, {} kbps{}{}",
            self.width,
            self.height,
            self.frames_per_second,
            self.video_bitrate / 1000,
            if self.full_media_type { "" } else { " (minimal media type)" },
            if self.enable_hardware_transforms { "" } else { " (software MFT)" },
        ))
    }
}

/// `BuildAttempts`: H.264 when the capture fits its limits (HEVC first
/// when it doesn't), then without the optional attributes, padded to the
/// encoders' minimums and macroblocks, HEVC for large or fast captures,
/// lower bitrates, at most 60 then 30 FPS, 4K then 1080p, and Windows'
/// software encoder when hardware isn't required. Each distinct once.
pub fn build_attempts(width: i32, height: i32, fps: i32, bitrate: i32, require_hardware_encoder: bool) -> Vec<EncoderAttempt> {
    let mut attempts = Vec::new();
    let mut seen = HashSet::new();
    let mut add = |codec, w, h, rate, bps, full_media_type, enable_hardware_transforms| {
        let candidate = EncoderAttempt { codec, width: w, height: h, frames_per_second: rate, video_bitrate: bps, full_media_type, enable_hardware_transforms };
        if seen.insert(candidate) {
            attempts.push(candidate);
        }
    };
    use Codec::{H264, Hevc};

    let (h264_width, h264_height) = fit_size(width, height, H264_MAX_WIDTH, H264_MAX_HEIGHT, H264_MAX_PIXELS, fps, H264_MAX_PIXELS_PER_SECOND, 2);
    let exceeds_h264_limits = h264_width != width || h264_height != height;

    // 1. If the capture exceeds H.264 Level 5.2 (size, height, or macroblock rate), try HEVC at full resolution/FPS first.
    if exceeds_h264_limits {
        let (hevc_width, hevc_height) = fit_size(width, height, HEVC_MAX_WIDTH, HEVC_MAX_HEIGHT, HEVC_MAX_PIXELS, fps, HEVC_MAX_PIXELS_PER_SECOND, 2);
        add(Hevc, hevc_width, hevc_height, fps, bitrate, true, true);
        add(Hevc, hevc_width, hevc_height, fps, bitrate, false, true);
    }

    // 2. Primary H.264 attempt at target dimensions/FPS/bitrate, then without optional profile/colorimetry attributes.
    add(H264, h264_width, h264_height, fps, bitrate, true, true);
    add(H264, h264_width, h264_height, fps, bitrate, false, true);

    // 3. Pad small regions to hardware minimums (160x128) and align to 16-pixel macroblock boundaries.
    let (aligned_width, aligned_height) = align_and_pad(h264_width, h264_height, H264_MAX_WIDTH, H264_MAX_HEIGHT, H264_MAX_PIXELS, fps, H264_MAX_PIXELS_PER_SECOND);
    add(H264, aligned_width, aligned_height, fps, bitrate, true, true);
    add(H264, aligned_width, aligned_height, fps, bitrate, false, true);

    // 4. For high-resolution or high-FPS captures that fit Level 5.2 on paper but were rejected by the GPU's H.264 MFT, try HEVC before degrading.
    if !exceeds_h264_limits && (width > 1920 || height > 1088 || fps > 60) && width >= MIN_ENCODER_WIDTH && height >= MIN_ENCODER_HEIGHT {
        add(Hevc, width, height, fps, bitrate, true, true);
        add(Hevc, width, height, fps, bitrate, false, true);
    }

    // 5. Clamp high bitrates that exceed hardware/level limits.
    let high_compat_bitrate = bitrate.min(HIGH_COMPATIBILITY_BITRATE);
    if high_compat_bitrate < bitrate {
        add(H264, h264_width, h264_height, fps, high_compat_bitrate, false, true);
        add(H264, aligned_width, aligned_height, fps, high_compat_bitrate, false, true);
    }
    let compat_bitrate = bitrate.min(COMPATIBILITY_BITRATE);
    if compat_bitrate < high_compat_bitrate {
        add(H264, h264_width, h264_height, fps, compat_bitrate, false, true);
        add(H264, aligned_width, aligned_height, fps, compat_bitrate, false, true);
    }

    // 6. Cap frame rate at 60 FPS for encoders that reject >60 FPS, restoring full resolution when possible.
    if fps > 60 {
        let (w60, h60) = fit_size(width, height, H264_MAX_WIDTH, H264_MAX_HEIGHT, H264_MAX_PIXELS, 60, H264_MAX_PIXELS_PER_SECOND, 2);
        let (aw60, ah60) = align_and_pad(w60, h60, H264_MAX_WIDTH, H264_MAX_HEIGHT, H264_MAX_PIXELS, 60, H264_MAX_PIXELS_PER_SECOND);
        add(H264, w60, h60, 60, compat_bitrate, true, true);
        add(H264, aw60, ah60, 60, compat_bitrate, false, true);
    }

    // 7. Step down to common lower hardware ceilings (4K UHD 3840x2160, then 1080p 1920x1088).
    let fallback_fps = fps.min(60);
    for (cap_width, cap_height) in [(3840, 2160), (1920, 1088)] {
        let cap_pixels = cap_width as i64 * cap_height as i64;
        let (cw, ch) = fit_size(width, height, cap_width, cap_height, cap_pixels, fallback_fps, H264_MAX_PIXELS_PER_SECOND, 2);
        let (acw, ach) = align_and_pad(cw, ch, cap_width, cap_height, cap_pixels, fallback_fps, H264_MAX_PIXELS_PER_SECOND);
        add(H264, acw, ach, fallback_fps, compat_bitrate, false, true);
    }
    if fallback_fps > 30 {
        let (cw30, ch30) = fit_size(width, height, 1920, 1088, 1920 * 1088, 30, H264_MAX_PIXELS_PER_SECOND, 2);
        let (acw30, ach30) = align_and_pad(cw30, ch30, 1920, 1088, 1920 * 1088, 30, H264_MAX_PIXELS_PER_SECOND);
        add(H264, acw30, ach30, 30, bitrate.min(BASELINE_COMPATIBILITY_BITRATE), false, true);
    }

    // 8. When software encoding is allowed, bypass hardware MFTs if a driver's hardware MFT rejected all media types.
    if !require_hardware_encoder {
        let (sw_w, sw_h) = fit_size(width, height, H264_MAX_WIDTH, H264_MAX_HEIGHT, H264_MAX_PIXELS, fallback_fps, H264_MAX_PIXELS_PER_SECOND, 2);
        let (asw_w, asw_h) = align_and_pad(sw_w, sw_h, H264_MAX_WIDTH, H264_MAX_HEIGHT, H264_MAX_PIXELS, fallback_fps, H264_MAX_PIXELS_PER_SECOND);
        add(H264, sw_w, sw_h, fallback_fps, compat_bitrate, true, false);
        add(H264, asw_w, asw_h, fallback_fps, compat_bitrate, false, false);
        let fps30 = fps.min(30);
        let (sw1080_w, sw1080_h) = fit_size(width, height, 1920, 1088, 1920 * 1088, fps30, H264_MAX_PIXELS_PER_SECOND, 2);
        let (asw1080_w, asw1080_h) = align_and_pad(sw1080_w, sw1080_h, 1920, 1088, 1920 * 1088, fps30, H264_MAX_PIXELS_PER_SECOND);
        add(H264, asw1080_w, asw1080_h, fps30, bitrate.min(BASELINE_COMPATIBILITY_BITRATE), false, false);
    }

    attempts
}

/// `FitSize`: downscaled to the encoder's frame and macroblock-rate limits,
/// keeping the aspect ratio, to a multiple of `alignment` (a power of two).
#[allow(clippy::too_many_arguments)]
pub fn fit_size(width: i32, height: i32, max_width: i32, max_height: i32, max_pixels: i64, fps: i32, max_pixels_per_second: i64, alignment: i32) -> (i32, i32) {
    let effective_max_pixels = max_pixels.min(max_pixels_per_second / fps.max(1) as i64);
    let scale = 1f64.min(
        (max_width as f64 / width as f64)
            .min(max_height as f64 / height as f64)
            .min((effective_max_pixels as f64 / (width as i64 * height as i64) as f64).sqrt()),
    );
    if scale >= 1.0 && width % alignment == 0 && height % alignment == 0 {
        return (width, height);
    }
    let mask = !(alignment - 1);
    // C#'s `(int)` cast truncates toward zero.
    let mut fitted_width = alignment.max((width as f64 * scale) as i32 & mask);
    let mut fitted_height = alignment.max((height as f64 * scale) as i32 & mask);
    while fitted_width as i64 * fitted_height as i64 > effective_max_pixels && (fitted_width > alignment || fitted_height > alignment) {
        if fitted_width >= fitted_height && fitted_width > alignment {
            fitted_width -= alignment;
        } else if fitted_height > alignment {
            fitted_height -= alignment;
        }
    }
    (fitted_width, fitted_height)
}

/// `AlignAndPad`: tiny regions padded to the hardware minimums and sizes
/// rounded up to 16-pixel macroblocks, within the encoder's limits.
pub fn align_and_pad(width: i32, height: i32, max_width: i32, max_height: i32, max_pixels: i64, fps: i32, max_pixels_per_second: i64) -> (i32, i32) {
    let mut aligned_width = MIN_ENCODER_WIDTH.max((width + 15) & !15);
    let mut aligned_height = MIN_ENCODER_HEIGHT.max((height + 15) & !15);
    if aligned_width > max_width {
        aligned_width = MIN_ENCODER_WIDTH.max(max_width & !15);
    }
    if aligned_height > max_height {
        aligned_height = MIN_ENCODER_HEIGHT.max(max_height & !15);
    }
    let effective_max_pixels = max_pixels.min(max_pixels_per_second / fps.max(1) as i64);
    if aligned_width as i64 * aligned_height as i64 > effective_max_pixels {
        (aligned_width, aligned_height) = fit_size(aligned_width, aligned_height, max_width, max_height, max_pixels, fps, max_pixels_per_second, 16);
        aligned_width = MIN_ENCODER_WIDTH.max(aligned_width);
        aligned_height = MIN_ENCODER_HEIGHT.max(aligned_height);
    }
    (aligned_width, aligned_height)
}

/// `GpuVideoProcessor.ComputeDestRect`: where the canvas goes in an output
/// frame of another size (left, top, right, bottom). Centred unscaled when
/// the frame is larger (padding), else scaled to fit and centred
/// (letterboxed), on even pixels.
pub fn compute_dest_rect(source_width: i32, source_height: i32, output_width: i32, output_height: i32) -> (i32, i32, i32, i32) {
    if output_width == source_width && output_height == source_height {
        return (0, 0, output_width, output_height);
    }
    if output_width >= source_width && output_height >= source_height {
        let left = ((output_width - source_width) / 2) & !1;
        let top = ((output_height - source_height) / 2) & !1;
        return (left, top, left + source_width, top + source_height);
    }
    let scale = (output_width as f64 / source_width as f64).min(output_height as f64 / source_height as f64);
    // `Math.Round` rounds half to even.
    let scaled_width = (((source_width as f64 * scale).round_ties_even() as i32) & !1).clamp(2, output_width);
    let scaled_height = (((source_height as f64 * scale).round_ties_even() as i32) & !1).clamp(2, output_height);
    let dest_left = ((output_width - scaled_width) / 2) & !1;
    let dest_top = ((output_height - scaled_height) / 2) & !1;
    (dest_left, dest_top, dest_left + scaled_width, dest_top + scaled_height)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h264_fit(w: i32, h: i32, fps: i32) -> (i32, i32) {
        fit_size(w, h, H264_MAX_WIDTH, H264_MAX_HEIGHT, H264_MAX_PIXELS, fps, H264_MAX_PIXELS_PER_SECOND, 2)
    }

    fn attempt(codec: Codec, width: i32, height: i32, frames_per_second: i32, video_bitrate: i32, full_media_type: bool, enable_hardware_transforms: bool) -> EncoderAttempt {
        EncoderAttempt { codec, width, height, frames_per_second, video_bitrate, full_media_type, enable_hardware_transforms }
    }

    #[test]
    fn fit_size_as_sharex() {
        assert_eq!(h264_fit(1920, 1080, 30), (1920, 1080));
        // Too wide for H.264: scaled to 4096 wide, aspect kept.
        assert_eq!(h264_fit(7680, 2160, 30), (4096, 1152));
        assert_eq!(h264_fit(5120, 2880, 30), (4096, 2304));
        // Odd sizes are rounded down to even even when they fit.
        assert_eq!(h264_fit(1001, 601, 30), (1000, 600));
        // MaxMBPS: 4096x2304 at 120 FPS is over 530,841,600 px/s.
        let (w, h) = h264_fit(4096, 2304, 120);
        assert!(w as i64 * h as i64 <= H264_MAX_PIXELS_PER_SECOND / 120, "{w}x{h}");
        assert_eq!((w % 2, h % 2), (0, 0));
        assert_eq!(fit_size(5120, 2880, HEVC_MAX_WIDTH, HEVC_MAX_HEIGHT, HEVC_MAX_PIXELS, 30, HEVC_MAX_PIXELS_PER_SECOND, 2), (5120, 2880));
        // 16-pixel alignment.
        assert_eq!(fit_size(4000, 3000, 4096, 2304, H264_MAX_PIXELS, 30, H264_MAX_PIXELS_PER_SECOND, 16), (3072, 2304));
    }

    #[test]
    fn align_and_pad_as_sharex() {
        let pad = |w, h| align_and_pad(w, h, H264_MAX_WIDTH, H264_MAX_HEIGHT, H264_MAX_PIXELS, 30, H264_MAX_PIXELS_PER_SECOND);
        // Tiny regions are padded to the minimums.
        assert_eq!(pad(100, 50), (160, 128));
        // Rounded up to macroblocks.
        assert_eq!(pad(1000, 600), (1008, 608));
        assert_eq!(pad(1920, 1080), (1920, 1088));
        // Never over the encoder's limits.
        assert_eq!(pad(4096, 2304), (4096, 2304));
        assert_eq!(pad(4090, 2300), (4096, 2304));
    }

    #[test]
    fn first_attempt_is_what_was_asked_for() {
        let a = build_attempts(1920, 1080, 30, 8_000_000, true);
        assert_eq!(a[0], attempt(Codec::H264, 1920, 1080, 30, 8_000_000, true, true));
        assert_eq!(a[1], attempt(Codec::H264, 1920, 1080, 30, 8_000_000, false, true));
        assert_eq!(a[2], attempt(Codec::H264, 1920, 1088, 30, 8_000_000, true, true));
        assert_eq!(a[3], attempt(Codec::H264, 1920, 1088, 30, 8_000_000, false, true));
        assert_eq!(a[0].adjustment_note(0, 1920, 1080, 30, 8_000_000), None);
        // Hardware required: never Windows' software encoder.
        assert!(a.iter().all(|a| a.enable_hardware_transforms));
        // No duplicates.
        let unique: HashSet<_> = a.iter().collect();
        assert_eq!(unique.len(), a.len());
    }

    #[test]
    fn whole_list_for_1080p30() {
        // The bitrate is under every cap, the FPS under 60: only the 4K and
        // 1080p ceilings (both the aligned size, already tried) are left.
        let a = build_attempts(1920, 1080, 30, 8_000_000, true);
        assert_eq!(a.len(), 4, "{a:#?}");
        let sw = build_attempts(1920, 1080, 30, 8_000_000, false);
        assert_eq!(
            &sw[4..],
            &[
                attempt(Codec::H264, 1920, 1080, 30, 8_000_000, true, false),
                attempt(Codec::H264, 1920, 1088, 30, 8_000_000, false, false),
            ]
        );
    }

    #[test]
    fn beyond_h264_tries_hevc_first() {
        let a = build_attempts(7680, 2160, 30, 8_000_000, true);
        assert_eq!(a[0], attempt(Codec::Hevc, 7680, 2160, 30, 8_000_000, true, true));
        assert_eq!(a[1], attempt(Codec::Hevc, 7680, 2160, 30, 8_000_000, false, true));
        assert_eq!(a[2], attempt(Codec::H264, 4096, 1152, 30, 8_000_000, true, true));
        assert!(a[0].adjustment_note(0, 7680, 2160, 30, 8_000_000).is_some());
    }

    #[test]
    fn large_or_fast_captures_try_hevc_before_degrading() {
        let a = build_attempts(2560, 1440, 120, 100_000_000, false);
        let hevc = a.iter().position(|a| *a == attempt(Codec::Hevc, 2560, 1440, 120, 100_000_000, true, true)).expect("HEVC at full size");
        let first_lower_bitrate = a.iter().position(|a| a.video_bitrate < 100_000_000).expect("a lower bitrate");
        assert!(hevc < first_lower_bitrate);
        // Bitrate steps: 50 then 20 Mbps.
        assert!(a.contains(&attempt(Codec::H264, 2560, 1440, 120, 50_000_000, false, true)));
        assert!(a.contains(&attempt(Codec::H264, 2560, 1440, 120, 20_000_000, false, true)));
        // 60 FPS cap at full resolution.
        assert!(a.contains(&attempt(Codec::H264, 2560, 1440, 60, 20_000_000, true, true)));
        // 1080p at 30 FPS and 10 Mbps.
        assert!(a.contains(&attempt(Codec::H264, 1920, 1088, 30, 10_000_000, false, true)));
        // Software last.
        let last = a.last().unwrap();
        assert_eq!(*last, attempt(Codec::H264, 1920, 1088, 30, 10_000_000, false, false));
        assert_eq!(last.adjustment_note(a.len() - 1, 2560, 1440, 120, 100_000_000).unwrap(), "Encoder negotiated H.264 1920x1088 @ 30 FPS, 10000 kbps (minimal media type) (software MFT)");
        // Each frame within the macroblock rate of its FPS.
        for a in &a {
            if a.codec == Codec::H264 {
                assert!(a.width as i64 * a.height as i64 <= H264_MAX_PIXELS_PER_SECOND / a.frames_per_second as i64, "{a:?}");
            }
        }
    }

    #[test]
    fn small_regions_are_padded() {
        let a = build_attempts(100, 50, 30, 8_000_000, true);
        assert_eq!(a[0], attempt(Codec::H264, 100, 50, 30, 8_000_000, true, true));
        assert_eq!(a[2], attempt(Codec::H264, 160, 128, 30, 8_000_000, true, true));
        assert_eq!(a[2].adjustment_note(2, 100, 50, 30, 8_000_000).unwrap(), "Encoder negotiated H.264 160x128 @ 30 FPS, 8000 kbps");
    }

    #[test]
    fn dest_rect_as_sharex() {
        assert_eq!(compute_dest_rect(1920, 1080, 1920, 1080), (0, 0, 1920, 1080));
        // Padded: centred, on even pixels.
        assert_eq!(compute_dest_rect(100, 50, 160, 128), (30, 38, 130, 88));
        assert_eq!(compute_dest_rect(1920, 1080, 1920, 1088), (0, 4, 1920, 1084));
        // Scaled: same aspect fills the frame.
        assert_eq!(compute_dest_rect(7680, 2160, 4096, 1152), (0, 0, 4096, 1152));
        // Scaled to fit and letterboxed.
        assert_eq!(compute_dest_rect(2000, 1000, 1920, 1088), (0, 64, 1920, 1024));
    }
}
