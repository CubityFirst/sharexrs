//! How a capture (or any image a task runs, `UploadManager.RunImageTask`)
//! is encoded for saving and uploading: `TaskHelpers.PrepareImage` and
//! `SaveImageAsStream` over the task's `TaskSettingsImage` (the image
//! format, PNG bit depth, compression level and filter, JPEG quality and
//! subsampling, GIF quality, BMP bit depth, WebP quality, lossless and
//! effort, and the automatic switch to JPEG over a size), with
//! `SkiaImageHelpers`' encoders ([`crate::imaging`]) and
//! `PNGStripColorSpaceInformation`.
//!
//! JPEG, GIF, BMP and WebP come out byte for byte as ShareX writes them;
//! PNG decodes to the same pixels with the same colour type, but its zlib
//! stream isn't Skia's, so its size (which the switch to JPEG compares) is
//! a little different. `tools/tools-harness` checks it against ShareX's
//! own `PrepareImage` (docs/TOOLS.md).

use sharex_settings::{BMPBitDepth, EImageFormat, GIFQuality, PNGBitDepth, RawEnum, TaskSettingsImage};

use crate::imaging::{self, Bitmap, BmpBitDepth, GifQuality, JpegDownsample, PngBitDepth, PngFilter, PngOptions};

/// `ImageData`: the encoded image and the format it ended up in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageData {
    pub data: Vec<u8>,
    pub format: EImageFormat,
}

/// `EImageFormat.GetFileExtension`.
pub fn extension(format: &EImageFormat) -> &'static str {
    match format {
        EImageFormat::JPEG => "jpg",
        EImageFormat::WEBP => "webp",
        EImageFormat::GIF => "gif",
        EImageFormat::BMP => "bmp",
        _ => "png",
    }
}

/// `Enum.IsDefined(ImageFormat) ? ImageFormat : PNG`.
fn defined_format(format: &EImageFormat) -> EImageFormat {
    match format {
        EImageFormat::PNG | EImageFormat::JPEG | EImageFormat::WEBP | EImageFormat::GIF | EImageFormat::BMP => format.clone(),
        _ => EImageFormat::PNG,
    }
}

fn png_depth(d: &PNGBitDepth) -> PngBitDepth {
    match d {
        PNGBitDepth::Automatic => PngBitDepth::Automatic,
        PNGBitDepth::Bit32 => PngBitDepth::Bit32,
        PNGBitDepth::Bit24 => PngBitDepth::Bit24,
        _ => PngBitDepth::Default,
    }
}

/// `SKBitmap.Save` with a `BMPBitDepth` ShareX doesn't know falls to 24
/// bits (`SaveBMP`'s `depth == Bit32 || Automatic && transparent`).
fn bmp_depth(d: &BMPBitDepth) -> BmpBitDepth {
    match d {
        BMPBitDepth::Automatic => BmpBitDepth::Automatic,
        BMPBitDepth::Bit32 => BmpBitDepth::Bit32,
        _ => BmpBitDepth::Bit24,
    }
}

/// `SkiaImageHelpers.Quantize`: Adaptive, Grayscale, Bit4, else 8 bits.
fn gif_quality(q: &GIFQuality) -> GifQuality {
    match q {
        GIFQuality::Adaptive => GifQuality::Adaptive,
        GIFQuality::Grayscale => GifQuality::Grayscale,
        GIFQuality::Bit4 => GifQuality::Bit4,
        GIFQuality::Bit8 => GifQuality::Bit8,
        _ => GifQuality::Default,
    }
}

/// A SkiaSharp enum written by name or number: its value.
fn raw_value(v: &RawEnum, names: &[(&str, i64)]) -> Option<i64> {
    match &v.0 {
        serde_json::Value::Number(n) => n.as_i64(),
        // Json.NET's flags: "Sub, Up".
        serde_json::Value::String(s) => s.split(',').map(str::trim).try_fold(0i64, |acc, part| {
            names.iter().find(|(n, _)| n.eq_ignore_ascii_case(part)).map(|(_, v)| acc | v).or_else(|| part.parse::<i64>().ok().map(|v| acc | v))
        }),
        _ => None,
    }
}

/// `GetJPEGEncoderOptions`: an undefined subsampling is 4:2:0.
pub fn jpeg_subsampling(v: &RawEnum) -> JpegDownsample {
    match raw_value(v, &[("Downsample420", 0), ("Downsample422", 1), ("Downsample444", 2)]) {
        Some(1) => JpegDownsample::Downsample422,
        Some(2) => JpegDownsample::Downsample444,
        _ => JpegDownsample::Downsample420,
    }
}

/// `TaskHelpers.GetPNGEncoderOptions`: one filter, or all of them; the
/// level clamped to 0-9.
pub fn png_options(s: &TaskSettingsImage) -> PngOptions {
    let names = [("NoFilters", 0), ("None", 8), ("Sub", 16), ("Up", 32), ("Avg", 64), ("Paeth", 128), ("AllFilters", 248)];
    let filter = match raw_value(&s.ImagePNGFilter, &names) {
        Some(8) => PngFilter::None,
        Some(16) => PngFilter::Sub,
        Some(32) => PngFilter::Up,
        Some(64) => PngFilter::Avg,
        Some(128) => PngFilter::Paeth,
        _ => PngFilter::All,
    };
    PngOptions { level: s.ImagePNGCompressionLevel.clamp(0, 9) as u8, filter }
}

/// `SkiaImageHelpers.PNGStripChunks`: the chunks named dropped, the rest
/// copied as they are (a truncated chunk ends the copy).
pub fn png_strip_chunks(png: &[u8], chunks: &[&[u8; 4]]) -> Vec<u8> {
    let mut out = Vec::with_capacity(png.len());
    if png.len() < 8 {
        out.extend_from_slice(png);
        return out;
    }
    out.extend_from_slice(&png[..8]);
    let mut i = 8;
    while i + 8 <= png.len() {
        let len = u32::from_be_bytes([png[i], png[i + 1], png[i + 2], png[i + 3]]) as usize;
        let ty = &png[i + 4..i + 8];
        // The data and its CRC.
        let end = (i + 8 + len + 4).min(png.len());
        if !chunks.iter().any(|c| &c[..] == ty) {
            out.extend_from_slice(&png[i..end]);
        }
        i = end;
    }
    out
}

/// `PNGStripColorSpaceInformation`: gAMA, cHRM, sRGB and iCCP.
pub fn png_strip_color_space_information(png: &[u8]) -> Vec<u8> {
    png_strip_chunks(png, &[b"gAMA", b"cHRM", b"sRGB", b"iCCP"])
}

/// `SkiaImageHelpers.FillBackground(img, Color.White)`.
fn on_white(bmp: &Bitmap) -> Bitmap {
    imaging::fill_background(bmp, [255, 255, 255, 255])
}

/// `SkiaImageHelpers.SaveJPEGAutoQuality(img, sizeLimit, decrement, min,
/// max, subsampling)`: the highest quality, stepping down, whose JPEG fits.
pub fn save_jpeg_auto_quality(bmp: &Bitmap, size_limit: i64, decrement: i32, min: i32, max: i32, subsampling: JpegDownsample) -> Vec<u8> {
    let decrement = decrement.clamp(1, 100);
    let min = min.clamp(0, 100);
    let max = max.clamp(0, 100);
    if min >= max {
        return imaging::save_jpeg(bmp, min, subsampling);
    }
    let mut out = Vec::new();
    let mut quality = max;
    while quality >= min {
        out = imaging::save_jpeg(bmp, quality, subsampling);
        if out.len() as i64 <= size_limit {
            break;
        }
        quality -= decrement;
    }
    out
}

/// `TaskHelpers.SaveImageAsStream(img, imageFormat, taskSettings)`: the
/// image in `format` with the task's options (no size limit).
pub fn save_image_as_stream(bmp: &Bitmap, format: &EImageFormat, s: &TaskSettingsImage, strip_png_color_space: bool) -> Vec<u8> {
    match defined_format(format) {
        EImageFormat::JPEG => imaging::save_jpeg(&on_white(bmp), s.ImageJPEGQuality, jpeg_subsampling(&s.ImageJPEGSubsampling)),
        EImageFormat::GIF => imaging::save_gif(bmp, gif_quality(&s.ImageGIFQuality)),
        EImageFormat::BMP => imaging::save_bmp(bmp, bmp_depth(&s.ImageBMPBitDepth)),
        // `GetWebPEncoderOptions`: the quality, or the effort when lossless.
        EImageFormat::WEBP => {
            let q = if s.ImageWebPLossless { s.ImageWebPCompressionEffort } else { s.ImageWebPQuality };
            imaging::save_webp(bmp, s.ImageWebPLossless, q.clamp(0, 100), false)
        }
        _ => {
            let png = imaging::save_png_with(bmp, png_depth(&s.ImagePNGBitDepth), png_options(s));
            if strip_png_color_space { png_strip_color_space_information(&png) } else { png }
        }
    }
}

/// `TaskHelpers.PrepareImage(img, taskSettings)`: the task's format, or a
/// JPEG on white when `ImageAutoUseJPEG` is on (and the format isn't JPEG
/// already) and the file would be over `ImageAutoUseJPEGSize` KB (1000
/// bytes each), at the JPEG quality or, with `ImageAutoJPEGQuality`, the
/// best quality from 100 down to 70 in steps of 2 that fits.
/// `strip_png_color_space`: `ApplicationConfig.PNGStripColorSpaceInformation`.
pub fn prepare_image(bmp: &Bitmap, s: &TaskSettingsImage, strip_png_color_space: bool) -> ImageData {
    let format = defined_format(&s.ImageFormat);
    let auto_jpeg = s.ImageAutoUseJPEG && format != EImageFormat::JPEG;
    let limit = s.ImageAutoUseJPEGSize as i64 * 1000;
    let data = save_image_as_stream(bmp, &format, s, strip_png_color_space);
    if auto_jpeg && data.len() as i64 > limit {
        let white = on_white(bmp);
        let subsampling = jpeg_subsampling(&s.ImageJPEGSubsampling);
        let data = if s.ImageAutoJPEGQuality {
            save_jpeg_auto_quality(&white, limit, 2, 70, 100, subsampling)
        } else {
            imaging::save_jpeg(&white, s.ImageJPEGQuality, subsampling)
        };
        return ImageData { data, format: EImageFormat::JPEG };
    }
    ImageData { data, format }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Noise (an LCG), so PNGs are big.
    fn bitmap(w: u32, h: u32, alpha: u8) -> Bitmap {
        let mut v = 1u32;
        let px: Vec<u8> = (0..w * h)
            .flat_map(|_| {
                v = v.wrapping_mul(1_103_515_245).wrapping_add(12345);
                [(v >> 8) as u8, (v >> 16) as u8, (v >> 24) as u8, alpha]
            })
            .collect();
        Bitmap::from_straight(w, h, &px)
    }

    #[test]
    fn formats_and_extensions() {
        let bmp = bitmap(8, 6, 255);
        let mut s = TaskSettingsImage::default();
        for (format, magic, ext) in [
            (EImageFormat::PNG, &b"\x89PNG"[..], "png"),
            (EImageFormat::JPEG, &b"\xFF\xD8\xFF"[..], "jpg"),
            (EImageFormat::GIF, &b"GIF8"[..], "gif"),
            (EImageFormat::BMP, &b"BM"[..], "bmp"),
            (EImageFormat::WEBP, &b"RIFF"[..], "webp"),
        ] {
            s.ImageFormat = format.clone();
            let d = prepare_image(&bmp, &s, false);
            assert_eq!(d.format, format);
            assert!(d.data.starts_with(magic), "{format:?}");
            assert_eq!(extension(&d.format), ext);
        }
        s.ImageFormat = EImageFormat::Unknown("TIFF".into());
        assert_eq!(prepare_image(&bmp, &s, false).format, EImageFormat::PNG);
    }

    #[test]
    fn switches_to_jpeg_over_the_size() {
        let bmp = bitmap(64, 64, 255);
        let mut s = TaskSettingsImage { ImageAutoUseJPEGSize: 1, ..Default::default() };
        assert_eq!(prepare_image(&bmp, &s, false).format, EImageFormat::JPEG);
        s.ImageAutoUseJPEG = false;
        assert_eq!(prepare_image(&bmp, &s, false).format, EImageFormat::PNG);
        // BMP is over the limit too.
        s.ImageAutoUseJPEG = true;
        s.ImageFormat = EImageFormat::BMP;
        assert_eq!(prepare_image(&bmp, &s, false).format, EImageFormat::JPEG);
        // Auto quality: the first quality down from 100 that fits (the
        // last one tried, 70, when none does).
        s.ImageAutoJPEGQuality = true;
        let white = on_white(&bmp);
        let jpegs: Vec<Vec<u8>> = (70..=100).rev().step_by(2).map(|q| imaging::save_jpeg(&white, q, JpegDownsample::Downsample420)).collect();
        let limit = jpegs[5].len().div_ceil(1000);
        s.ImageAutoUseJPEGSize = limit as i32;
        let want = jpegs.iter().find(|j| j.len() <= limit * 1000).unwrap();
        let d = prepare_image(&bmp, &s, false);
        assert_eq!(d.format, EImageFormat::JPEG);
        assert_eq!(&d.data, want);
    }

    #[test]
    fn png_options_and_depths() {
        let mut s = TaskSettingsImage::default();
        assert_eq!(png_options(&s), PngOptions { level: 1, filter: PngFilter::All });
        s.ImagePNGFilter = RawEnum::name("Paeth");
        s.ImagePNGCompressionLevel = 12;
        assert_eq!(png_options(&s), PngOptions { level: 9, filter: PngFilter::Paeth });
        s.ImagePNGFilter = RawEnum::name("Sub, Up");
        assert_eq!(png_options(&s).filter, PngFilter::All);
        s.ImagePNGFilter = RawEnum(serde_json::json!(32));
        assert_eq!(png_options(&s).filter, PngFilter::Up);
        assert_eq!(jpeg_subsampling(&RawEnum::name("Downsample444")), JpegDownsample::Downsample444);
        assert_eq!(jpeg_subsampling(&RawEnum(serde_json::json!(7))), JpegDownsample::Downsample420);
        // Default depth: the bitmap as it is (RGBA); Automatic: RGB when opaque.
        let bmp = bitmap(4, 4, 255);
        let color_type = |png: &[u8]| png[25];
        let png = save_image_as_stream(&bmp, &EImageFormat::PNG, &TaskSettingsImage::default(), false);
        assert_eq!(color_type(&png), 6);
        let s = TaskSettingsImage { ImagePNGBitDepth: PNGBitDepth::Automatic, ..Default::default() };
        assert_eq!(color_type(&save_image_as_stream(&bmp, &EImageFormat::PNG, &s, false)), 2);
        let s = TaskSettingsImage { ImagePNGBitDepth: PNGBitDepth::Bit24, ..Default::default() };
        assert_eq!(color_type(&save_image_as_stream(&bitmap(4, 4, 100), &EImageFormat::PNG, &s, false)), 2);
    }

    #[test]
    fn strips_color_space_chunks() {
        let chunk = |ty: &[u8; 4], data: &[u8]| {
            let mut c = (data.len() as u32).to_be_bytes().to_vec();
            c.extend_from_slice(ty);
            c.extend_from_slice(data);
            c.extend_from_slice(&[0, 0, 0, 0]);
            c
        };
        let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
        let ihdr = chunk(b"IHDR", &[0; 13]);
        let idat = chunk(b"IDAT", &[1, 2, 3]);
        let iend = chunk(b"IEND", &[]);
        for c in [&ihdr, &chunk(b"sRGB", &[0]), &chunk(b"gAMA", &[0; 4]), &idat, &chunk(b"iCCP", &[9; 5]), &iend] {
            png.extend_from_slice(c);
        }
        let mut want = b"\x89PNG\r\n\x1a\n".to_vec();
        for c in [&ihdr, &idat, &iend] {
            want.extend_from_slice(c);
        }
        assert_eq!(png_strip_color_space_information(&png), want);
    }
}
