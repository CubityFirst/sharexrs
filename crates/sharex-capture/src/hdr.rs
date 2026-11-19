//! `ShareX.ScreenCaptureLib/HDRScreenCapture.cs`, the part that works on
//! pixels: what ShareX does with a duplicated HDR desktop frame once it is
//! mapped (`CaptureOutput` after `Map`) and how it corrects the GDI capture
//! with it (`CopyOutputIntersection`). The DXGI desktop duplication and
//! the Windows tone mapper (WIC) are in `platform::hdr` (Windows); this
//! part runs on any frame, so it is tested on synthetic ones and compared
//! with ShareX's own code (`tools/tools-harness --hdr`).
//!
//! Images are RGBA here where ShareX's are BGRA; the channels are the same.

use image::RgbaImage;

use crate::geometry::Rect;

/// `SceneReferredSdrWhiteNits`: scRGB's 1.0.
pub const SCENE_REFERRED_SDR_WHITE_NITS: f32 = 80.0;
/// `DefaultHdrPeakNits`.
pub const DEFAULT_HDR_PEAK_NITS: f32 = 1000.0;
const TONE_MAP_KNEE: f32 = 0.75;
/// `SdrReferenceTolerance`: how far a GDI pixel may be from the SDR
/// reference and still be kept.
const SDR_REFERENCE_TOLERANCE: i32 = 4;

/// The duplicated desktop's format (`Texture2DDescription.Format`); ShareX
/// corrects only these two.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HdrFormat {
    /// `R16G16B16A16_Float`: scRGB, linear, 1.0 = 80 nits.
    Rgba16Float,
    /// `R10G10B10A2_UNorm`: BT.2020 primaries, PQ (ST 2084).
    R10G10B10A2,
}

impl HdrFormat {
    fn bytes_per_pixel(self) -> usize {
        match self {
            HdrFormat::Rgba16Float => 8,
            HdrFormat::R10G10B10A2 => 4,
        }
    }
}

/// A mapped frame (`MappedSubresource` with its `Texture2DDescription`):
/// `height` rows of `row_pitch` bytes.
#[derive(Clone, Copy, Debug)]
pub struct HdrFrame<'a> {
    pub format: HdrFormat,
    pub width: u32,
    pub height: u32,
    pub row_pitch: usize,
    pub data: &'a [u8],
}

impl HdrFrame<'_> {
    /// The pixel at (x, y) as linear sRGB relative to SDR white: FP16 divided
    /// by the white scale, or PQ decoded and converted from BT.2020.
    fn linear(&self, x: usize, y: usize, sdr_white_scale: f32) -> (f32, f32, f32) {
        let at = y * self.row_pitch + x * self.format.bytes_per_pixel();
        match self.format {
            HdrFormat::Rgba16Float => {
                let half = |i: usize| f16_to_f32(u16::from_le_bytes([self.data[at + i * 2], self.data[at + i * 2 + 1]]));
                (half(0) / sdr_white_scale, half(1) / sdr_white_scale, half(2) / sdr_white_scale)
            }
            HdrFormat::R10G10B10A2 => {
                let packed = u32::from_le_bytes([self.data[at], self.data[at + 1], self.data[at + 2], self.data[at + 3]]);
                let mut r = pq_to_linear_srgb((packed & 0x3FF) as f32 / 1023.0, sdr_white_scale);
                let mut g = pq_to_linear_srgb(((packed >> 10) & 0x3FF) as f32 / 1023.0, sdr_white_scale);
                let mut b = pq_to_linear_srgb(((packed >> 20) & 0x3FF) as f32 / 1023.0, sdr_white_scale);
                convert_rec2020_to_srgb(&mut r, &mut g, &mut b);
                (r, g, b)
            }
        }
    }
}

/// `DXGI_MODE_ROTATION` (`ModeRotation`): how the output is turned.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ModeRotation {
    #[default]
    Unspecified,
    Identity,
    Rotate90,
    Rotate180,
    Rotate270,
}

impl ModeRotation {
    /// From `DXGI_MODE_ROTATION`'s value.
    pub fn from_dxgi(value: i32) -> Self {
        match value {
            1 => Self::Identity,
            2 => Self::Rotate90,
            3 => Self::Rotate180,
            4 => Self::Rotate270,
            _ => Self::Unspecified,
        }
    }
}

/// `CapturedOutput`: Windows' tone-mapped picture of one output and, when
/// that came from Windows' tone mapper, the SDR reference (whose alpha
/// marks the pixels brighter than SDR white).
#[derive(Clone, Debug, PartialEq)]
pub struct CapturedOutput {
    pub tone_mapped: RgbaImage,
    pub sdr_reference: Option<RgbaImage>,
}

/// What `CaptureOutput` knows about the output besides its frame
/// (`OutputDescription1`, `GetSdrWhiteScale`, `GetPeakNits`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OutputColor {
    pub sdr_white_scale: f32,
    pub peak_nits: f32,
    pub rotation: ModeRotation,
    /// `GetOutputBounds`: `DesktopCoordinates`.
    pub bounds: Rect,
}

/// `CaptureOutput` once the frame is mapped: Windows' tone mapper
/// (`tone_mapper`: `ConvertWithWindowsToneMapper`, BGRA-free RGBA here) and
/// the SDR reference, or, when either fails, ShareX's own tone curve
/// (`ConvertToSrgb`); both turned as the output is; `None` when the result
/// isn't the output's size.
pub fn captured_output(
    frame: &HdrFrame,
    color: &OutputColor,
    tone_mapper: impl FnOnce(&HdrFrame) -> Result<RgbaImage, String>,
) -> Option<CapturedOutput> {
    // `try { toneMapped = ConvertWithWindowsToneMapper; sdrReference = ConvertToSdrReference }
    // catch { toneMapped = ConvertToSrgb }`. The reference can't throw here.
    let mut output = match tone_mapper(frame) {
        Ok(tone_mapped) => CapturedOutput { tone_mapped, sdr_reference: Some(convert_to_sdr_reference(frame, color.sdr_white_scale)) },
        Err(_) => CapturedOutput { tone_mapped: convert_to_srgb(frame, color.sdr_white_scale, color.peak_nits), sdr_reference: None },
    };
    rotate_output(&mut output.tone_mapped, color.rotation);
    if let Some(reference) = &mut output.sdr_reference {
        rotate_output(reference, color.rotation);
    }
    if output.tone_mapped.width() as i32 != color.bounds.w || output.tone_mapped.height() as i32 != color.bounds.h {
        return None;
    }
    Some(output)
}

/// `GetPeakNits`: the output's `MaxLuminance`, or 1000 nits when it isn't a
/// real luminance.
pub fn peak_nits(max_luminance: f32) -> f32 {
    if !max_luminance.is_finite() || max_luminance < SCENE_REFERRED_SDR_WHITE_NITS { DEFAULT_HDR_PEAK_NITS } else { max_luminance }
}

/// `ConvertToSdrReference`: the frame as SDR would show it (clipped at SDR
/// white), alpha 255 where any channel is outside 0..1 (or not finite),
/// else 0.
pub fn convert_to_sdr_reference(frame: &HdrFrame, sdr_white_scale: f32) -> RgbaImage {
    let mut out = RgbaImage::new(frame.width, frame.height);
    for y in 0..frame.height as usize {
        for x in 0..frame.width as usize {
            let (r, g, b) = frame.linear(x, y, sdr_white_scale);
            let extended = !is_sdr_channel(r) || !is_sdr_channel(g) || !is_sdr_channel(b);
            out.put_pixel(x as u32, y as u32, image::Rgba([linear_to_srgb_byte(r), linear_to_srgb_byte(g), linear_to_srgb_byte(b), if extended { 255 } else { 0 }]));
        }
    }
    out
}

/// `IsSdrChannel`.
fn is_sdr_channel(value: f32) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

/// `ConvertToSrgb`: ShareX's own tone curve and gamut compression, opaque.
pub fn convert_to_srgb(frame: &HdrFrame, sdr_white_scale: f32, peak_nits: f32) -> RgbaImage {
    let relative_peak = (peak_nits / (SCENE_REFERRED_SDR_WHITE_NITS * sdr_white_scale)).clamp(1.25, 125.0);
    let mut out = RgbaImage::new(frame.width, frame.height);
    for y in 0..frame.height as usize {
        for x in 0..frame.width as usize {
            let (mut r, mut g, mut b) = frame.linear(x, y, sdr_white_scale);
            tone_map_and_compress_gamut(&mut r, &mut g, &mut b, relative_peak);
            out.put_pixel(x as u32, y as u32, image::Rgba([linear_to_srgb_byte(r), linear_to_srgb_byte(g), linear_to_srgb_byte(b), 255]));
        }
    }
    out
}

/// .NET's `Math.Max(float, float)`: NaN if either is NaN.
fn cs_max(a: f32, b: f32) -> f32 {
    if a.is_nan() || b.is_nan() {
        return f32::NAN;
    }
    if a == b {
        // +0 over -0, as IEEE 754's maximum.
        return if a.is_sign_negative() { b } else { a };
    }
    if a > b { a } else { b }
}

/// `Math.Clamp(float, min, max)`: NaN passes through.
fn cs_clamp(value: f32, min: f32, max: f32) -> f32 {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

/// `ToneMapAndCompressGamut`.
fn tone_map_and_compress_gamut(red: &mut f32, green: &mut f32, blue: &mut f32, relative_peak: f32) {
    *red = cs_max(*red, 0.0);
    *green = cs_max(*green, 0.0);
    *blue = cs_max(*blue, 0.0);
    let mut maximum = cs_max(*red, cs_max(*green, *blue));
    // Values entirely inside the SDR/sRGB range are left untouched.
    if maximum <= 1.0 {
        return;
    }
    let luminance = *red * 0.2126 + *green * 0.7152 + *blue * 0.0722;
    let mapped_luminance = tone_map_shoulder(luminance, relative_peak);
    if luminance > 0.00001 {
        let scale = mapped_luminance / luminance;
        *red *= scale;
        *green *= scale;
        *blue *= scale;
    }
    maximum = cs_max(*red, cs_max(*green, *blue));
    if maximum > 1.0 {
        // Wide-gamut colours into sRGB, keeping luminance and hue direction.
        let gray = cs_clamp(mapped_luminance, 0.0, 1.0);
        let chroma_scale = if maximum > gray { (1.0 - gray) / (maximum - gray) } else { 0.0 };
        *red = gray + (*red - gray) * chroma_scale;
        *green = gray + (*green - gray) * chroma_scale;
        *blue = gray + (*blue - gray) * chroma_scale;
    }
}

/// `ToneMapShoulder`.
fn tone_map_shoulder(value: f32, relative_peak: f32) -> f32 {
    if value <= TONE_MAP_KNEE {
        return cs_max(value, 0.0);
    }
    let peak = cs_max(relative_peak, TONE_MAP_KNEE + 0.001);
    let normalized = cs_clamp((value - TONE_MAP_KNEE) / (peak - TONE_MAP_KNEE), 0.0, 1.0);
    let curve_strength = (peak - TONE_MAP_KNEE) / (1.0 - TONE_MAP_KNEE);
    let shoulder = curve_strength * normalized / (1.0 + (curve_strength - 1.0) * normalized);
    TONE_MAP_KNEE + (1.0 - TONE_MAP_KNEE) * shoulder
}

/// `PqToLinearSrgb`: ST 2084 to nits, over SDR white.
fn pq_to_linear_srgb(encoded: f32, sdr_white_scale: f32) -> f32 {
    const M1: f32 = 2610.0 / 16384.0;
    const M2: f32 = 2523.0 / 32.0;
    const C1: f32 = 3424.0 / 4096.0;
    const C2: f32 = 2413.0 / 128.0;
    const C3: f32 = 2392.0 / 128.0;
    let power = cs_clamp(encoded, 0.0, 1.0).powf(1.0 / M2);
    let normalized_nits = (cs_max(power - C1, 0.0) / cs_max(C2 - C3 * power, 0.00001)).powf(1.0 / M1);
    let nits = normalized_nits * 10000.0;
    nits / (SCENE_REFERRED_SDR_WHITE_NITS * sdr_white_scale)
}

/// `ConvertRec2020ToSrgb` (ShareX's constants as written).
#[allow(clippy::excessive_precision)]
fn convert_rec2020_to_srgb(red: &mut f32, green: &mut f32, blue: &mut f32) {
    let (r, g, b) = (*red, *green, *blue);
    *red = 1.660491 * r - 0.587641 * g - 0.072850 * b;
    *green = -0.124550 * r + 1.132900 * g - 0.008349 * b;
    *blue = -0.018151 * r - 0.100579 * g + 1.118730 * b;
}

/// `LinearToSrgbByte`.
fn linear_to_srgb_byte(value: f32) -> u8 {
    let value = cs_clamp(value, 0.0, 1.0);
    let encoded = if value <= 0.0031308 { value * 12.92 } else { 1.055 * value.powf(1.0 / 2.4) - 0.055 };
    // `(byte)Math.Clamp((int)MathF.Round(encoded * 255f), 0, 255)`: ties to
    // even, a NaN converts to 0 (.NET's saturating conversion, as Rust's).
    ((encoded * 255.0).round_ties_even() as i32).clamp(0, 255) as u8
}

/// The bits of an IEEE half as a float (`BitConverter.UInt16BitsToHalf`,
/// then `(float)`): exact.
pub fn f16_to_f32(bits: u16) -> f32 {
    let sign = ((bits >> 15) as u32) << 31;
    let exponent = ((bits >> 10) & 0x1F) as u32;
    let mantissa = (bits & 0x3FF) as u32;
    let value = match exponent {
        0 if mantissa == 0 => sign,
        0 => {
            // Subnormal: normalise.
            let mut e = 127 - 15 + 1;
            let mut m = mantissa;
            while m & 0x400 == 0 {
                m <<= 1;
                e -= 1;
            }
            sign | (e << 23) | ((m & 0x3FF) << 13)
        }
        0x1F => sign | 0x7F80_0000 | (mantissa << 13),
        _ => sign | ((exponent + 127 - 15) << 23) | (mantissa << 13),
    };
    f32::from_bits(value)
}

/// `RotateOutput`: `SkiaImageHelpers.RotateFlipInPlace(bitmap, 1|2|3)`,
/// as `CopyToneMappedPixels` then reads it: the unpremultiplied picture is
/// drawn turned into a premultiplied bitmap that replaces it
/// ([`rotate_output_premultiplied`]), which `SkiaPixelBuffer` reads back
/// unpremultiplied (Skia's `ReadPixels`). So a turned output's
/// translucent pixels lose precision, and the SDR reference's unmarked
/// pixels (alpha 0) become black, as in ShareX.
pub fn rotate_output(image: &mut RgbaImage, rotation: ModeRotation) {
    if rotate_output_premultiplied(image, rotation) {
        for p in image.pixels_mut() {
            p.0 = sharex_skia::raster::unpremul(p.0);
        }
    }
}

/// `RotateOutput`'s bitmap itself: turned and premultiplied (Skia's
/// premultiply); false (and untouched) when the output isn't turned.
pub fn rotate_output_premultiplied(image: &mut RgbaImage, rotation: ModeRotation) -> bool {
    let quarter = match rotation {
        ModeRotation::Rotate90 => 1,
        ModeRotation::Rotate180 => 2,
        ModeRotation::Rotate270 => 3,
        _ => return false,
    };
    let (w, h) = (image.width(), image.height());
    let (rw, rh) = if quarter % 2 == 1 { (h, w) } else { (w, h) };
    let mut result = RgbaImage::new(rw, rh);
    for y in 0..h {
        for x in 0..w {
            // `Translate(rw/2, rh/2); RotateDegrees(q*90); Translate(-w/2, -h/2)`
            // at pixel centres: whole pixels, nearest.
            let (dx, dy) = match quarter {
                1 => (h - 1 - y, x),
                2 => (w - 1 - x, h - 1 - y),
                _ => (y, w - 1 - x),
            };
            let p = image.get_pixel(x, y).0;
            let a = p[3];
            let premul = |c: u8| sharex_skia::raster::premul_byte(c, a);
            result.put_pixel(dx, dy, image::Rgba([premul(p[0]), premul(p[1]), premul(p[2]), a]));
        }
    }
    *image = result;
    true
}

/// `CopyOutputIntersection`: the part of the output inside the capture,
/// into `destination` (the GDI capture of `capture`).
pub fn copy_output_intersection(destination: &mut RgbaImage, capture: Rect, output: &CapturedOutput, output_bounds: Rect) {
    let intersection = capture.intersect(&output_bounds);
    if intersection.is_empty() {
        return;
    }
    let source = Rect::new(intersection.x - output_bounds.x, intersection.y - output_bounds.y, intersection.w, intersection.h);
    let target = Rect::new(intersection.x - capture.x, intersection.y - capture.y, intersection.w, intersection.h);
    if let Some(reference) = &output.sdr_reference {
        copy_tone_mapped_pixels(destination, target, &output.tone_mapped, reference, source);
        return;
    }
    // `DrawImage(toneMapped, source, destination, Src)`: whole pixels, a copy.
    for y in 0..source.h {
        for x in 0..source.w {
            let (sx, sy, tx, ty) = (source.x + x, source.y + y, target.x + x, target.y + y);
            if sx < 0 || sy < 0 || sx >= output.tone_mapped.width() as i32 || sy >= output.tone_mapped.height() as i32 {
                continue;
            }
            if tx < 0 || ty < 0 || tx >= destination.width() as i32 || ty >= destination.height() as i32 {
                continue;
            }
            destination.put_pixel(tx as u32, ty as u32, *output.tone_mapped.get_pixel(sx as u32, sy as u32));
        }
    }
}

/// `CopyToneMappedPixels`: brighter-than-SDR pixels from Windows' tone
/// mapper; other pixels from the SDR reference where the GDI capture is
/// more than 4 levels off it (SDR content of HDR-aware apps), else kept.
fn copy_tone_mapped_pixels(destination: &mut RgbaImage, target: Rect, tone_mapped: &RgbaImage, reference: &RgbaImage, source: Rect) {
    for y in 0..source.h {
        for x in 0..source.w {
            let (sx, sy) = ((source.x + x) as u32, (source.y + y) as u32);
            let (tx, ty) = ((target.x + x) as u32, (target.y + y) as u32);
            let r = reference.get_pixel(sx, sy).0;
            let d = destination.get_pixel_mut(tx, ty);
            let extended = r[3] != 0;
            let matches_sdr = (0..3).all(|i| (d[i] as i32 - r[i] as i32).abs() <= SDR_REFERENCE_TOLERANCE);
            if extended {
                let t = tone_mapped.get_pixel(sx, sy).0;
                *d = image::Rgba([t[0], t[1], t[2], 255]);
            } else if !matches_sdr {
                *d = image::Rgba([r[0], r[1], r[2], 255]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fp16_frame(pixels: &[[u16; 4]]) -> Vec<u8> {
        pixels.iter().flat_map(|p| p.iter().flat_map(|c| c.to_le_bytes())).collect()
    }

    #[test]
    fn halves_convert_exactly() {
        assert_eq!(f16_to_f32(0x3C00), 1.0);
        assert_eq!(f16_to_f32(0xC000), -2.0);
        assert_eq!(f16_to_f32(0x7BFF), 65504.0);
        assert_eq!(f16_to_f32(0x0001), 2f32.powi(-24));
        assert_eq!(f16_to_f32(0x03FF), 1023.0 * 2f32.powi(-24));
        assert!(f16_to_f32(0x7C00).is_infinite());
        assert!(f16_to_f32(0x7E00).is_nan());
        assert!(f16_to_f32(0x8000).is_sign_negative());
    }

    #[test]
    fn sdr_values_are_untouched_and_highlights_marked() {
        // 0.5 and 1.0 stay SDR; 2.0 is a highlight.
        let data = fp16_frame(&[[0x3800, 0x3800, 0x3800, 0x3C00], [0x3C00, 0x3C00, 0x3C00, 0x3C00], [0x4000, 0x3C00, 0x3800, 0x3C00]]);
        let frame = HdrFrame { format: HdrFormat::Rgba16Float, width: 3, height: 1, row_pitch: 24, data: &data };
        let reference = convert_to_sdr_reference(&frame, 1.0);
        assert_eq!(reference.get_pixel(0, 0).0, [188, 188, 188, 0]);
        assert_eq!(reference.get_pixel(1, 0).0, [255, 255, 255, 0]);
        assert_eq!(reference.get_pixel(2, 0).0[3], 255);
        let srgb = convert_to_srgb(&frame, 1.0, 1000.0);
        assert_eq!(srgb.get_pixel(0, 0).0, [188, 188, 188, 255]);
        let p = srgb.get_pixel(2, 0).0;
        assert!(p[0] > p[2]);
    }

    #[test]
    fn rotation_turns_and_premultiplies() {
        let mut img = RgbaImage::from_fn(3, 2, |x, y| image::Rgba([x as u8 * 10 + y as u8, 50, 60, if x == 0 { 0 } else { 255 }]));
        rotate_output(&mut img, ModeRotation::Rotate90);
        assert_eq!((img.width(), img.height()), (2, 3));
        // (x, y) -> (h-1-y, x)
        assert_eq!(img.get_pixel(1, 2).0, [20, 50, 60, 255]);
        assert_eq!(img.get_pixel(0, 2).0, [21, 50, 60, 255]);
        assert_eq!(img.get_pixel(1, 0).0, [0, 0, 0, 0]);
        let mut half = RgbaImage::from_pixel(1, 1, image::Rgba([207, 0, 255, 64]));
        assert!(rotate_output_premultiplied(&mut half, ModeRotation::Rotate180));
        assert_eq!(half.get_pixel(0, 0).0, [52, 0, 64, 64]);
        let mut img2 = RgbaImage::from_fn(3, 2, |x, y| image::Rgba([x as u8, y as u8, 0, 255]));
        rotate_output(&mut img2, ModeRotation::Rotate270);
        // (x, y) -> (y, w-1-x)
        assert_eq!(img2.get_pixel(1, 0).0, [2, 1, 0, 255]);
        let mut img3 = img2.clone();
        rotate_output(&mut img3, ModeRotation::Identity);
        assert_eq!(img3, img2);
    }

    #[test]
    fn the_intersection_is_corrected() {
        let tone_mapped = RgbaImage::from_pixel(4, 4, image::Rgba([1, 2, 3, 255]));
        let mut reference = RgbaImage::from_pixel(4, 4, image::Rgba([100, 100, 100, 0]));
        reference.put_pixel(2, 2, image::Rgba([255, 255, 255, 255]));
        let output = CapturedOutput { tone_mapped, sdr_reference: Some(reference) };
        // Capture (10,10) 4x4, output (8,8) 4x4: capture (0..2, 0..2) is output (2..4, 2..4).
        let mut gdi = RgbaImage::from_pixel(4, 4, image::Rgba([102, 98, 104, 255]));
        gdi.put_pixel(1, 0, image::Rgba([0, 0, 0, 255]));
        copy_output_intersection(&mut gdi, Rect::new(10, 10, 4, 4), &output, Rect::new(8, 8, 4, 4));
        assert_eq!(gdi.get_pixel(0, 0).0, [1, 2, 3, 255]); // a highlight: tone-mapped
        assert_eq!(gdi.get_pixel(1, 0).0, [100, 100, 100, 255]); // more than 4 off: the reference
        assert_eq!(gdi.get_pixel(0, 1).0, [102, 98, 104, 255]); // within 4: kept
        assert_eq!(gdi.get_pixel(2, 2).0, [102, 98, 104, 255]); // outside the output
        // Without a reference, the tone-mapped pixels are copied.
        let plain = CapturedOutput { sdr_reference: None, ..output };
        copy_output_intersection(&mut gdi, Rect::new(10, 10, 4, 4), &plain, Rect::new(8, 8, 4, 4));
        assert_eq!(gdi.get_pixel(1, 1).0, [1, 2, 3, 255]);
        assert_eq!(gdi.get_pixel(2, 1).0, [102, 98, 104, 255]);
    }

    #[test]
    fn nan_and_infinity_follow_dotnet() {
        let data = fp16_frame(&[[0x7E00, 0x7C00, 0xFC00, 0x3C00]]);
        let frame = HdrFrame { format: HdrFormat::Rgba16Float, width: 1, height: 1, row_pitch: 8, data: &data };
        assert_eq!(convert_to_srgb(&frame, 1.0, 1000.0).get_pixel(0, 0).0, [0, 255, 0, 255]);
        assert_eq!(convert_to_sdr_reference(&frame, 1.0).get_pixel(0, 0).0, [0, 255, 0, 255]);
    }

    #[test]
    fn peak_as_sharex() {
        assert_eq!(peak_nits(f32::NAN), 1000.0);
        assert_eq!(peak_nits(79.9), 1000.0);
        assert_eq!(peak_nits(600.0), 600.0);
    }
}
