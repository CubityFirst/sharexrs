//! Image converter: `ImageConverterService` and `ImageConverterViewModel.ConvertImages`.

use sharex_skia::raster::{Canvas, Rect, Sampling};

use crate::dotnet;
use crate::imaging::{self, Bitmap, BmpBitDepth, GifQuality, JpegDownsample, PngBitDepth, Rgba};

/// `ImageConverterOutputFormat`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Png,
    Jpeg,
    Webp,
    Gif,
    Bmp,
}

impl OutputFormat {
    /// `GetFileExtension`.
    pub fn extension(self) -> &'static str {
        match self {
            OutputFormat::Png => "png",
            OutputFormat::Jpeg => "jpg",
            OutputFormat::Webp => "webp",
            OutputFormat::Gif => "gif",
            OutputFormat::Bmp => "bmp",
        }
    }
}

/// Every encoder setting the window exposes, with its defaults.
#[derive(Debug, Clone, PartialEq)]
pub struct ConvertOptions {
    pub format: OutputFormat,
    /// JPEG/WebP quality, or the WebP compression effort when lossless
    /// (`GetEncoderQuality`).
    pub quality: i32,
    /// JPEG, BMP 24-bit and PNG 24-bit flatten onto this (alpha ignored).
    pub background: Rgba,
    pub jpeg_subsampling: JpegDownsample,
    pub gif_quality: GifQuality,
    pub png_bit_depth: PngBitDepth,
    pub bmp_bit_depth: BmpBitDepth,
    pub webp_lossless: bool,
}

impl Default for ConvertOptions {
    fn default() -> Self {
        ConvertOptions {
            format: OutputFormat::Png,
            quality: 90,
            background: imaging::WHITE,
            jpeg_subsampling: JpegDownsample::Downsample420,
            gif_quality: GifQuality::Adaptive,
            png_bit_depth: PngBitDepth::Automatic,
            bmp_bit_depth: BmpBitDepth::Bit24,
            webp_lossless: false,
        }
    }
}

pub const DEFAULT_FILE_NAME: &str = "$filename_converted";
const MAX_PREVIEW_DIMENSION: u32 = 1600;

/// `ImageConverterService.LoadImage`: SkCodec's pixels, EXIF orientation applied.
pub fn load_image(path: &str) -> Option<Bitmap> {
    imaging::load_image(path)
}

/// `FlattenBackground`: an opaque bitmap cleared to the background's RGB, the
/// image drawn on top.
pub fn flatten_background(src: &Bitmap, background: Rgba) -> Bitmap {
    imaging::fill_background(src, [background[0], background[1], background[2], 255])
}

/// `ImageConverterService.Encode`.
pub fn encode(bmp: &Bitmap, o: &ConvertOptions) -> Vec<u8> {
    match o.format {
        OutputFormat::Png => {
            let flattened = (o.png_bit_depth == PngBitDepth::Bit24).then(|| flatten_background(bmp, o.background));
            imaging::save_png(flattened.as_ref().unwrap_or(bmp), o.png_bit_depth)
        }
        OutputFormat::Gif => imaging::save_gif(bmp, o.gif_quality),
        OutputFormat::Bmp => {
            let flattened = (o.bmp_bit_depth == BmpBitDepth::Bit24).then(|| flatten_background(bmp, o.background));
            imaging::save_bmp(flattened.as_ref().unwrap_or(bmp), o.bmp_bit_depth)
        }
        OutputFormat::Jpeg => {
            let flattened = flatten_background(bmp, o.background);
            imaging::save_jpeg_with(&flattened, o.quality, o.jpeg_subsampling, true)
        }
        OutputFormat::Webp => imaging::save_webp(bmp, o.webp_lossless, o.quality.clamp(0, 100)),
    }
}

/// `GetPreviewSize`.
pub fn preview_size(w: u32, h: u32) -> (u32, u32) {
    let largest = w.max(h);
    if largest <= MAX_PREVIEW_DIMENSION {
        return (w, h);
    }
    let scale = MAX_PREVIEW_DIMENSION as f64 / largest as f64;
    (
        ((w as f64 * scale).round_ties_even() as u32).max(1),
        ((h as f64 * scale).round_ties_even() as u32).max(1),
    )
}

/// `CreatePreviewBitmap`: Catmull-Rom into the preview size.
pub fn preview_bitmap(src: &Bitmap, w: u32, h: u32) -> Bitmap {
    let mut out = imaging::create_bitmap(w, h);
    Canvas::new(&mut out).draw_image_rect(
        src,
        None,
        Rect::xywh(0.0, 0.0, w as f32, h as f32),
        Sampling::CatmullRom,
        None,
    );
    out
}

/// `ImageConverterService.CreatePreview`: the encoded preview and the source size.
pub fn create_preview(path: &str, o: &ConvertOptions) -> (Vec<u8>, u32, u32) {
    let Some(source) = load_image(path) else { return (Vec::new(), 0, 0) };
    let (w, h) = preview_size(source.width(), source.height());
    let preview = preview_bitmap(&source, w, h);
    (encode(&preview, o), source.width(), source.height())
}

/// The output path for one image (`ConvertImages`).
pub fn output_path(output_folder: &str, output_file_name: &str, source: &str, format: OutputFormat) -> String {
    let name = dotnet::file_name_without_extension(source);
    let path = dotnet::combine(output_folder, &output_file_name.replace("$filename", name));
    dotnet::change_extension(&path, format.extension())
}

/// `ImageConverterViewModel.ConvertImages`.
pub fn convert_images(files: &[String], o: &ConvertOptions, output_folder: &str, output_file_name: &str) -> std::io::Result<Vec<String>> {
    let mut out = Vec::new();
    for file in files {
        if !std::path::Path::new(file).is_file() {
            continue;
        }
        let Some(source) = load_image(file) else { continue };
        let path = output_path(output_folder, output_file_name, file, o.format);
        let data = encode(&source, o);
        if let Some(dir) = std::path::Path::new(&path).parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&path, data)?;
        out.push(path);
    }
    Ok(out)
}
