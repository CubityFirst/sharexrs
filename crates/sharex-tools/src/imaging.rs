//! ShareX's `SkiaImageHelpers` as the tools use it: loading an image
//! (`LoadImage`/`Decode`: SkCodec into premultiplied BGRA, EXIF orientation
//! applied), the drawing helpers (`CreateBitmap`, `FillBackground`,
//! `Resize`, `ResizeImage`, `CreateThumbnail`, the `DrawImage` shims) on
//! `sharex_skia`'s port of Skia's raster canvas, and the encoders
//! (`Save`, `SavePNG`, `SaveBMP`, `SaveJPEG`, WebP, `SaveGIF`).
//!
//! Pixels are [`Bitmap`]s: premultiplied RGBA, the same values as ShareX's
//! premultiplied BGRA `SKBitmap`s in another channel order.
//!
//! JPEG, BMP, GIF and WebP (lossy and lossless, with ShareX's libwebp) are
//! written byte for byte as ShareX writes them. PNGs decode to the same pixels
//! (Skia's zlib streams differ). See docs/TOOLS.md.

use std::path::Path;

pub use sharex_skia::raster::Bitmap;
use sharex_skia::raster::{self, Canvas, Paint, Rect, Sampling};

use crate::gif;

/// A straight (unpremultiplied) RGBA colour, `System.Drawing.Color`/`SKColor`.
pub type Rgba = [u8; 4];

pub const WHITE: Rgba = [255, 255, 255, 255];
pub const BLACK: Rgba = [0, 0, 0, 255];
pub const TRANSPARENT: Rgba = [0, 0, 0, 0];
/// `Color.WhiteSmoke`.
pub const WHITE_SMOKE: Rgba = [245, 245, 245, 255];

/// `SkEncodedOrigin`, from an EXIF orientation tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    TopLeft = 1,
    TopRight = 2,
    BottomRight = 3,
    BottomLeft = 4,
    LeftTop = 5,
    RightTop = 6,
    RightBottom = 7,
    LeftBottom = 8,
}

impl Origin {
    fn from_tag(v: u16) -> Origin {
        match v {
            2 => Origin::TopRight,
            3 => Origin::BottomRight,
            4 => Origin::BottomLeft,
            5 => Origin::LeftTop,
            6 => Origin::RightTop,
            7 => Origin::RightBottom,
            8 => Origin::LeftBottom,
            _ => Origin::TopLeft,
        }
    }
}

/// The EXIF orientation SkCodec reports (`SkCodec::getOrigin`): JPEG's APP1
/// `Exif` segment and WebP's `EXIF` chunk.
pub fn encoded_origin(bytes: &[u8]) -> Origin {
    let exif = if bytes.starts_with(&[0xFF, 0xD8]) {
        jpeg_exif(bytes)
    } else if is_webp(bytes) {
        webp_exif(bytes)
    } else {
        None
    };
    exif.and_then(exif_orientation).map(Origin::from_tag).unwrap_or(Origin::TopLeft)
}

fn jpeg_exif(b: &[u8]) -> Option<&[u8]> {
    let mut i = 2;
    while i + 4 <= b.len() {
        if b[i] != 0xFF {
            return None;
        }
        let marker = b[i + 1];
        if marker == 0xD9 || marker == 0xDA {
            return None;
        }
        let len = u16::from_be_bytes([b[i + 2], b[i + 3]]) as usize;
        let data = b.get(i + 4..i + 2 + len)?;
        if marker == 0xE1 && data.starts_with(b"Exif\0\0") {
            return Some(&data[6..]);
        }
        i += 2 + len;
    }
    None
}

fn is_webp(b: &[u8]) -> bool {
    b.len() >= 12 && &b[..4] == b"RIFF" && &b[8..12] == b"WEBP"
}

fn webp_exif(b: &[u8]) -> Option<&[u8]> {
    let mut i = 12;
    while i + 8 <= b.len() {
        let id = &b[i..i + 4];
        let size = u32::from_le_bytes(b[i + 4..i + 8].try_into().ok()?) as usize;
        let data = b.get(i + 8..i + 8 + size)?;
        if id == b"EXIF" {
            return Some(data.strip_prefix(b"Exif\0\0").unwrap_or(data));
        }
        i += 8 + size + (size & 1);
    }
    None
}

/// Tag 0x0112 in IFD0 of a TIFF header.
fn exif_orientation(t: &[u8]) -> Option<u16> {
    let le = match t.get(..2)? {
        b"II" => true,
        b"MM" => false,
        _ => return None,
    };
    let u16_at = |o: usize| -> Option<u16> {
        let s: [u8; 2] = t.get(o..o + 2)?.try_into().ok()?;
        Some(if le { u16::from_le_bytes(s) } else { u16::from_be_bytes(s) })
    };
    let u32_at = |o: usize| -> Option<u32> {
        let s: [u8; 4] = t.get(o..o + 4)?.try_into().ok()?;
        Some(if le { u32::from_le_bytes(s) } else { u32::from_be_bytes(s) })
    };
    let ifd = u32_at(4)? as usize;
    let count = u16_at(ifd)? as usize;
    for k in 0..count {
        let e = ifd + 2 + k * 12;
        if u16_at(e)? == 0x0112 {
            let v = u16_at(e + 8)?;
            return (1..=8).contains(&v).then_some(v);
        }
    }
    None
}

/// Decodes an image file's first frame to straight RGBA, as SkCodec does.
pub fn decode_straight(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    if let Some(d) = crate::jpeg_decode::decode(bytes) {
        return Ok(d);
    }
    let img = image::load_from_memory(bytes).map_err(|e| e.to_string())?;
    let rgba = match img {
        // SkSwizzler keeps the high byte of 16-bit samples.
        image::DynamicImage::ImageRgba16(i) => {
            let (w, h) = i.dimensions();
            (w, h, i.into_raw().iter().map(|v| (v >> 8) as u8).collect())
        }
        image::DynamicImage::ImageRgb16(i) => {
            let (w, h) = i.dimensions();
            let mut out = Vec::with_capacity((w * h * 4) as usize);
            for p in i.into_raw().chunks_exact(3) {
                out.extend_from_slice(&[(p[0] >> 8) as u8, (p[1] >> 8) as u8, (p[2] >> 8) as u8, 255]);
            }
            (w, h, out)
        }
        image::DynamicImage::ImageLuma16(i) => {
            let (w, h) = i.dimensions();
            let mut out = Vec::with_capacity((w * h * 4) as usize);
            for v in i.into_raw() {
                let g = (v >> 8) as u8;
                out.extend_from_slice(&[g, g, g, 255]);
            }
            (w, h, out)
        }
        image::DynamicImage::ImageLumaA16(i) => {
            let (w, h) = i.dimensions();
            let mut out = Vec::with_capacity((w * h * 4) as usize);
            for p in i.into_raw().chunks_exact(2) {
                let g = (p[0] >> 8) as u8;
                out.extend_from_slice(&[g, g, g, (p[1] >> 8) as u8]);
            }
            (w, h, out)
        }
        other => {
            let i = other.to_rgba8();
            let (w, h) = i.dimensions();
            (w, h, i.into_raw())
        }
    };
    Ok(rgba)
}

/// `SkiaImageHelpers.Decode(stream, applyOrientation)`.
pub fn decode(bytes: &[u8], apply_orientation: bool) -> Result<Bitmap, String> {
    let (w, h, mut rgba) = decode_straight(bytes)?;
    let bmp = if is_webp(bytes) {
        // libwebp premultiplies itself (MODE_bgrA, ApplyAlphaMultiply_SSE2):
        // (c * a * 0x8081) >> 23, i.e. c * a / 255 rounded down.
        for p in rgba.chunks_exact_mut(4) {
            let a = p[3] as u32;
            for c in &mut p[..3] {
                *c = ((*c as u32 * a * 0x8081) >> 23) as u8;
            }
        }
        Bitmap::from_premultiplied(w, h, rgba)
    } else {
        Bitmap::from_straight(w, h, &rgba)
    };
    if !apply_orientation {
        return Ok(bmp);
    }
    Ok(orient(&bmp, encoded_origin(bytes)))
}

/// The canvas transform `Decode` draws with for an EXIF origin (nearest
/// sampling at pixel centres, so a pure pixel permutation).
pub fn orient(bmp: &Bitmap, origin: Origin) -> Bitmap {
    if origin == Origin::TopLeft {
        return bmp.clone();
    }
    let (w, h) = (bmp.width(), bmp.height());
    let swap = origin as u8 >= Origin::LeftTop as u8;
    let (ow, oh) = if swap { (h, w) } else { (w, h) };
    let mut out = Bitmap::new(ow, oh);
    for y in 0..oh {
        for x in 0..ow {
            // The source pixel the inverse transform lands on.
            let (sx, sy) = match origin {
                Origin::TopRight => (w - 1 - x, y),
                Origin::BottomRight => (w - 1 - x, h - 1 - y),
                Origin::BottomLeft => (x, h - 1 - y),
                Origin::LeftTop => (y, x),
                Origin::RightTop => (y, h - 1 - x),
                Origin::RightBottom => (w - 1 - y, h - 1 - x),
                Origin::LeftBottom => (w - 1 - y, x),
                Origin::TopLeft => (x, y),
            };
            out.set_pixel(x, y, bmp.pixel(sx, sy));
        }
    }
    out
}

/// `SkiaImageHelpers.LoadImage(filePath)` with `HelpersOptions.RotateImageByExifOrientationData`
/// (true unless the user turned it off).
pub fn load_image(path: impl AsRef<Path>) -> Option<Bitmap> {
    load_image_with(path, true)
}

pub fn load_image_with(path: impl AsRef<Path>, apply_orientation: bool) -> Option<Bitmap> {
    let bytes = std::fs::read(path).ok()?;
    decode(&bytes, apply_orientation).ok()
}

/// `SkiaImageHelpers.CreateBitmap(width, height)`: transparent.
pub fn create_bitmap(width: u32, height: u32) -> Bitmap {
    Bitmap::new(width, height)
}

/// `SKBitmap.GetPixel`: `SkPixmap::getColor`, unpremultiplied with
/// `SkUnPreMultiply::PMColorToColor`'s table.
pub fn get_color(bmp: &Bitmap, x: u32, y: u32) -> Rgba {
    unpremul_table(bmp.pixel(x, y))
}

/// `SkUnPreMultiply::PMColorToColor`: `(scale * c + (1 << 23)) >> 24` with
/// `scale = gTable[a]`, `(255 << 24) / a` rounded.
pub fn unpremul_table(p: Rgba) -> Rgba {
    let a = p[3] as u32;
    if a == 0 || a == 255 {
        return p;
    }
    let scale = ((255u32 << 24) + a / 2) / a;
    let f = |c: u8| ((scale as u64 * c as u64 + (1 << 23)) >> 24) as u8;
    [f(p[0]), f(p[1]), f(p[2]), p[3]]
}

/// Premultiplied to straight as Skia's encoders and `SkBitmap::getColor`
/// convert: the float pipeline's `unpremul`, stored as `(v * 255 + 0.5)`
/// truncated (fitted to ShareX's PNGs: ties round up, unlike `readPixels`).
pub fn unpremul_encode(p: Rgba) -> Rgba {
    match p[3] {
        255 => p,
        0 => [0, 0, 0, 0],
        _ => {
            let c = raster::unpremul_f(raster::load(p));
            let st = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
            [st(c[0]), st(c[1]), st(c[2]), p[3]]
        }
    }
}

/// [`unpremul_encode`] for every pixel.
pub fn encoder_pixels(bmp: &Bitmap) -> Vec<Rgba> {
    (0..(bmp.width() * bmp.height()) as usize).map(|i| unpremul_encode(bmp.px(i))).collect()
}

/// `SkiaPixelBuffer`'s straight pixels (`ReadPixels` into unpremultiplied BGRA).
pub fn straight_pixels(bmp: &Bitmap) -> Vec<Rgba> {
    (0..(bmp.width() * bmp.height()) as usize).map(|i| raster::unpremul(bmp.px(i))).collect()
}

/// `SkiaImageHelpers.IsImageTransparent`.
pub fn is_transparent(bmp: &Bitmap) -> bool {
    (0..(bmp.width() * bmp.height()) as usize).any(|i| bmp.px(i)[3] < 255)
}

/// `SKCanvas.Clear(color)` over a new canvas the size of `src`, then `src` on top
/// (`SkiaImageHelpers.FillBackground`).
pub fn fill_background(src: &Bitmap, color: Rgba) -> Bitmap {
    let mut out = create_bitmap(src.width(), src.height());
    {
        let mut c = Canvas::new(&mut out);
        c.clear(color);
        c.draw_bitmap(src, 0.0, 0.0, None);
    }
    out
}

/// `SkiaImageHelpers.DrawImage(canvas, bitmap, source, destination, paint)`.
pub fn draw_image(c: &mut Canvas, image: &Bitmap, src: Rect, dst: Rect, paint: Option<&Paint>) {
    c.draw_image_sx(image, src, dst, paint);
}

fn full_rect(b: &Bitmap) -> Rect {
    Rect::xywh(0.0, 0.0, b.width() as f32, b.height() as f32)
}

/// `SkiaImageHelpers.Resize(source, width, height)`.
pub fn resize(src: &Bitmap, width: u32, height: u32) -> Bitmap {
    let mut out = create_bitmap(width, height);
    {
        let mut c = Canvas::new(&mut out);
        draw_image(&mut c, src, full_rect(src), Rect::xywh(0.0, 0.0, width as f32, height as f32), None);
    }
    out
}

/// `ImageSamplingMode` as `SkiaImageHelpers.ResizeImage` maps it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SamplingMode {
    NearestNeighbor,
    Bilinear,
    HighQualityBilinear,
    HighQualityBicubic,
}

/// `SkiaImageHelpers.ResizeImage(source, width, height, mode)` (returns the
/// source itself when the size doesn't change or is invalid).
pub fn resize_image(src: Bitmap, width: i32, height: i32, mode: SamplingMode) -> Bitmap {
    if width < 1 || height < 1 || (src.width() as i32 == width && src.height() as i32 == height) {
        return src;
    }
    let mut out = create_bitmap(width as u32, height as u32);
    let sampling = match mode {
        SamplingMode::NearestNeighbor => Sampling::Nearest,
        SamplingMode::Bilinear | SamplingMode::HighQualityBilinear => Sampling::Linear,
        SamplingMode::HighQualityBicubic => Sampling::Mitchell,
    };
    // canvas.DrawImage(image, dest, sampling): the whole image into dest.
    Canvas::new(&mut out).draw_image_rect(
        &src,
        None,
        Rect::xywh(0.0, 0.0, width as f32, height as f32),
        sampling,
        None,
    );
    out
}

/// `SkiaImageHelpers.CreateThumbnail(source, width, height)`: the centred crop
/// with the target's aspect ratio, drawn (Mitchell) into the target size.
pub fn create_thumbnail(src: &Bitmap, width: i32, height: i32) -> Result<Bitmap, String> {
    if width < 1 || height < 1 {
        return Err("width and height must be at least 1".into());
    }
    let scale = (width as f32 / src.width() as f32).max(height as f32 / src.height() as f32);
    let sw = width as f32 / scale;
    let sh = height as f32 / scale;
    let crop = Rect::xywh((src.width() as f32 - sw) / 2.0, (src.height() as f32 - sh) / 2.0, sw, sh);
    let mut out = create_bitmap(width as u32, height as u32);
    {
        let mut c = Canvas::new(&mut out);
        draw_image(&mut c, src, crop, Rect::xywh(0.0, 0.0, width as f32, height as f32), None);
    }
    Ok(out)
}

/// `SkiaImageHelpers.CropBitmap`.
pub fn crop_bitmap(src: &Bitmap, x: i32, y: i32, w: i32, h: i32) -> Option<Bitmap> {
    if w <= 0 || h <= 0 || x < 0 || y < 0 || x + w > src.width() as i32 || y + h > src.height() as i32 {
        return None;
    }
    let mut out = create_bitmap(w as u32, h as u32);
    Canvas::new(&mut out).draw_bitmap(src, -x as f32, -y as f32, None);
    Some(out)
}

// ---------------------------------------------------------------- encoding

/// `SKEncodedImageFormat`/`ImageFileFormat` as ShareX saves them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFileFormat {
    Png,
    Jpeg,
    Gif,
    Bmp,
    Webp,
}

/// `SkiaImageHelpers.GetImageFormat(path)`: by extension, PNG otherwise.
pub fn image_format_for(path: &str) -> ImageFileFormat {
    match crate::dotnet::extension(path).to_lowercase().as_str() {
        ".jpg" | ".jpeg" | ".jpe" | ".jfif" => ImageFileFormat::Jpeg,
        ".gif" => ImageFileFormat::Gif,
        ".bmp" => ImageFileFormat::Bmp,
        ".webp" => ImageFileFormat::Webp,
        _ => ImageFileFormat::Png,
    }
}

/// `PNGBitDepth`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PngBitDepth {
    Default,
    Automatic,
    Bit32,
    Bit24,
}

/// `BMPBitDepth`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BmpBitDepth {
    Automatic,
    Bit32,
    Bit24,
}

/// `SKJpegEncoderDownsample`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JpegDownsample {
    Downsample420,
    Downsample422,
    Downsample444,
}

/// `GIFQuality`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GifQuality {
    Default,
    Bit8,
    Bit4,
    Grayscale,
    Adaptive,
}

/// The pixel rows an `SkPngEncoder` writes: RGBA, or RGB for an opaque bitmap.
fn png_bytes(width: u32, height: u32, rgba: &[u8], opaque: bool) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, width, height);
        enc.set_depth(png::BitDepth::Eight);
        if opaque {
            enc.set_color(png::ColorType::Rgb);
        } else {
            enc.set_color(png::ColorType::Rgba);
        }
        let mut w = enc.write_header().expect("png header");
        if opaque {
            let rgb: Vec<u8> = rgba.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
            w.write_image_data(&rgb).expect("png data");
        } else {
            w.write_image_data(rgba).expect("png data");
        }
    }
    out
}

/// `SKImage.FromBitmap(bitmap).Encode(SKEncodedImageFormat.Png, 100)` for a
/// premultiplied bitmap.
pub fn encode_png(bmp: &Bitmap) -> Vec<u8> {
    let rgba: Vec<u8> = encoder_pixels(bmp).into_iter().flatten().collect();
    png_bytes(bmp.width(), bmp.height(), &rgba, false)
}

/// PNG of an opaque `SKBitmap` (`SKAlphaType.Opaque`): RGB rows.
pub fn encode_png_opaque(bmp: &Bitmap) -> Vec<u8> {
    let rgba: Vec<u8> = (0..(bmp.width() * bmp.height()) as usize).flat_map(|i| bmp.px(i)).collect();
    png_bytes(bmp.width(), bmp.height(), &rgba, true)
}

/// `SkiaImageHelpers.SavePNG(bitmap, stream, depth)`.
pub fn save_png(bmp: &Bitmap, depth: PngBitDepth) -> Vec<u8> {
    let opaque_output = depth == PngBitDepth::Bit24 || (depth == PngBitDepth::Automatic && !is_transparent(bmp));
    if opaque_output {
        // Copied into an opaque bitmap through straight pixels with alpha 255.
        let rgba: Vec<u8> = straight_pixels(bmp).into_iter().flat_map(|p| [p[0], p[1], p[2], 255]).collect();
        png_bytes(bmp.width(), bmp.height(), &rgba, true)
    } else if depth == PngBitDepth::Bit32 {
        // ReadPixels into an unpremultiplied bitmap, encoded as is.
        let rgba: Vec<u8> = straight_pixels(bmp).into_iter().flatten().collect();
        png_bytes(bmp.width(), bmp.height(), &rgba, false)
    } else {
        encode_png(bmp)
    }
}

/// `SkiaImageHelpers.SaveBMP` (byte for byte).
pub fn save_bmp(bmp: &Bitmap, depth: BmpBitDepth) -> Vec<u8> {
    let alpha = depth == BmpBitDepth::Bit32 || (depth == BmpBitDepth::Automatic && is_transparent(bmp));
    let (w, h) = (bmp.width() as usize, bmp.height() as usize);
    let bpp = if alpha { 4 } else { 3 };
    let stride = (w * bpp).div_ceil(4) * 4;
    let header_size: u32 = if alpha { 108 } else { 40 };
    let pixel_offset = 14 + header_size;
    let pixel_size = (stride * h) as u32;
    let mut out = Vec::with_capacity((pixel_offset + pixel_size) as usize);
    out.extend_from_slice(&0x4D42u16.to_le_bytes());
    out.extend_from_slice(&(pixel_offset + pixel_size).to_le_bytes());
    out.extend_from_slice(&0i32.to_le_bytes());
    out.extend_from_slice(&pixel_offset.to_le_bytes());
    out.extend_from_slice(&header_size.to_le_bytes());
    out.extend_from_slice(&(w as i32).to_le_bytes());
    out.extend_from_slice(&(h as i32).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&((bpp * 8) as u16).to_le_bytes());
    out.extend_from_slice(&(if alpha { 3i32 } else { 0 }).to_le_bytes());
    out.extend_from_slice(&pixel_size.to_le_bytes());
    out.extend_from_slice(&3780i32.to_le_bytes());
    out.extend_from_slice(&3780i32.to_le_bytes());
    out.extend_from_slice(&0i32.to_le_bytes());
    out.extend_from_slice(&0i32.to_le_bytes());
    if alpha {
        for m in [0x00FF0000u32, 0x0000FF00, 0x000000FF, 0xFF000000, 0x73524742] {
            out.extend_from_slice(&m.to_le_bytes());
        }
        out.extend_from_slice(&[0u8; 48]);
    }
    let px = straight_pixels(bmp);
    let mut row = vec![0u8; stride];
    for y in (0..h).rev() {
        for x in 0..w {
            let c = px[y * w + x];
            let o = x * bpp;
            row[o] = c[2];
            row[o + 1] = c[1];
            row[o + 2] = c[0];
            if alpha {
                row[o + 3] = c[3];
            }
        }
        out.extend_from_slice(&row);
    }
    out
}

/// `SkiaImageHelpers.SaveJPEG(bitmap, stream, quality, subsampling)`
/// (libjpeg-turbo as Skia drives it, byte for byte; see [`crate::jpeg`]).
/// Alpha is ignored (`SKJpegEncoderAlphaOption.Ignore`: the premultiplied
/// colour is written). No ICC profile: the bitmaps ShareX creates have no
/// colour space.
pub fn save_jpeg(bmp: &Bitmap, quality: i32, subsampling: JpegDownsample) -> Vec<u8> {
    save_jpeg_with(bmp, quality, subsampling, false)
}

/// [`save_jpeg`] for a bitmap with a colour space (decoded by `SKCodec` and
/// kept as is, as the image converter does): Skia adds its sRGB ICC profile.
pub fn save_jpeg_with(bmp: &Bitmap, quality: i32, subsampling: JpegDownsample, icc: bool) -> Vec<u8> {
    let rgb: Vec<u8> = (0..(bmp.width() * bmp.height()) as usize)
        .flat_map(|i| {
            let p = bmp.px(i);
            [p[0], p[1], p[2]]
        })
        .collect();
    let sub = match subsampling {
        JpegDownsample::Downsample420 => crate::jpeg::Subsampling::S420,
        JpegDownsample::Downsample422 => crate::jpeg::Subsampling::S422,
        JpegDownsample::Downsample444 => crate::jpeg::Subsampling::S444,
    };
    crate::jpeg::encode_rgb(&rgb, bmp.width() as usize, bmp.height() as usize, quality.clamp(0, 100), sub, icc)
}

/// `SKPixmap.Encode(SKWebpEncoderOptions(compression, quality))` (the image
/// converter, `SkiaImageHelpers.SaveWebP`): libwebp as Skia drives it
/// ([`crate::webp`]), byte for byte. `icc`: the bitmap has a colour space
/// (decoded by SkCodec and kept), so Skia's sRGB profile is embedded. Empty
/// when libwebp can't encode it (an empty bitmap, over 16383 px).
pub fn save_webp(bmp: &Bitmap, lossless: bool, quality: i32, icc: bool) -> Vec<u8> {
    let compression = if lossless { crate::webp::Compression::Lossless } else { crate::webp::Compression::Lossy };
    crate::webp::encode(bmp, compression, quality.clamp(0, 100) as f32, icc).unwrap_or_default()
}

/// `SkiaImageHelpers.Quantize(bitmap, quality)`.
pub fn quantize(bmp: &Bitmap, quality: GifQuality) -> gif::IndexedImage {
    match quality {
        GifQuality::Adaptive => gif::AdaptiveGifQuantizer::new().quantize(bmp),
        GifQuality::Grayscale => gif::quantize_grayscale(bmp),
        GifQuality::Bit4 => gif::quantize_octree(bmp, 15, 4),
        _ => gif::quantize_octree(bmp, 255, 4),
    }
}

/// `SkiaImageHelpers.SaveGIF(bitmap, quality)`.
pub fn save_gif(bmp: &Bitmap, quality: GifQuality) -> Vec<u8> {
    quantize(bmp, quality).save_gif()
}

/// `SkiaImageHelpers.Save(bitmap, stream, format, quality = 100)`.
pub fn encode(bmp: &Bitmap, format: ImageFileFormat, quality: i32) -> Vec<u8> {
    match format {
        ImageFileFormat::Gif => save_gif(bmp, GifQuality::Adaptive),
        ImageFileFormat::Bmp => save_bmp(bmp, BmpBitDepth::Bit24),
        ImageFileFormat::Png => encode_png(bmp),
        // SKImage.Encode(Jpeg, q): libjpeg-turbo, 4:2:0.
        ImageFileFormat::Jpeg => save_jpeg(bmp, quality, JpegDownsample::Downsample420),
        // SKBitmap.Encode(Webp, q): SkEncodeImage's lossy WebP.
        ImageFileFormat::Webp => save_webp(bmp, false, quality, false),
    }
}

/// `SkiaImageHelpers.SaveImage(img, filePath)`: the format from the extension,
/// creating the folder.
pub fn save_image(bmp: &Bitmap, path: &str) -> std::io::Result<()> {
    if let Some(dir) = Path::new(path).parent() {
        if !dir.as_os_str().is_empty() {
            std::fs::create_dir_all(dir)?;
        }
    }
    std::fs::write(path, encode(bmp, image_format_for(path), 100))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unpremul_rounds_ties_up() {
        assert_eq!(unpremul_encode([0, 0, 0, 0]), [0, 0, 0, 0]);
        // 7 * 255 / 70 = 25.5
        assert_eq!(unpremul_encode([7, 0, 0, 70])[0], 26);
    }

    #[test]
    fn exif_orientation_from_jpeg() {
        // SOI, APP1 Exif with an II TIFF header and one IFD0 entry (orientation 6).
        let mut tiff = b"II*\0\x08\0\0\0".to_vec();
        tiff.extend_from_slice(&1u16.to_le_bytes());
        tiff.extend_from_slice(&0x0112u16.to_le_bytes());
        tiff.extend_from_slice(&3u16.to_le_bytes());
        tiff.extend_from_slice(&1u32.to_le_bytes());
        tiff.extend_from_slice(&6u16.to_le_bytes());
        tiff.extend_from_slice(&[0, 0, 0, 0, 0, 0]);
        let mut seg = b"Exif\0\0".to_vec();
        seg.extend_from_slice(&tiff);
        let mut jpeg = vec![0xFF, 0xD8, 0xFF, 0xE1];
        jpeg.extend_from_slice(&((seg.len() + 2) as u16).to_be_bytes());
        jpeg.extend_from_slice(&seg);
        jpeg.extend_from_slice(&[0xFF, 0xD9]);
        assert_eq!(encoded_origin(&jpeg), Origin::RightTop);
    }

    #[test]
    fn orientation_rotates() {
        let mut b = Bitmap::new(2, 1);
        b.set_pixel(0, 0, [255, 0, 0, 255]);
        b.set_pixel(1, 0, [0, 255, 0, 255]);
        let r = orient(&b, Origin::RightTop);
        assert_eq!((r.width(), r.height()), (1, 2));
        assert_eq!(r.pixel(0, 0), [255, 0, 0, 255]);
    }
}
