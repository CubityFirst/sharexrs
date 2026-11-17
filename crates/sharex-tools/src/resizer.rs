//! Image resizer: `ImageResizerService` and `ImageResizerViewModel.ResizeImages`.

use sharex_skia::raster::{Canvas, Rect};

use crate::dotnet;
use crate::imaging::{self, Bitmap, ImageFileFormat, Rgba};

/// `ImageResizeMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeMode {
    Fill,
    Fit,
    Stretch,
}

/// `ImageResizeOutputFormat`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Png,
    Jpeg,
}

const MAX_PREVIEW_DIMENSION: i32 = 1600;

/// `GetFillSourceRectangle`: the centred part of the source with the target's
/// aspect ratio.
fn fill_source_rect(sw: i32, sh: i32, dw: i32, dh: i32) -> Rect {
    let source_aspect = sw as f64 / sh as f64;
    let dest_aspect = dw as f64 / dh as f64;
    if source_aspect > dest_aspect {
        let width = (sh as f64 * dest_aspect) as f32;
        return Rect::xywh((sw as f32 - width) / 2.0, 0.0, width, sh as f32);
    }
    let height = (sw as f64 / dest_aspect) as f32;
    Rect::xywh(0.0, (sh as f32 - height) / 2.0, sw as f32, height)
}

/// `Math.Round` (to even), as an int.
fn round_even(v: f64) -> i32 {
    v.round_ties_even() as i32
}

/// `GetFitDestinationRectangle`: the source scaled into the target, centred.
fn fit_dest_rect(sw: i32, sh: i32, dw: i32, dh: i32) -> (i32, i32, i32, i32) {
    let scale = (dw as f64 / sw as f64).min(dh as f64 / sh as f64);
    let width = round_even(sw as f64 * scale).max(1);
    let height = round_even(sh as f64 * scale).max(1);
    ((dw - width) / 2, (dh - height) / 2, width, height)
}

/// `ImageResizerService.Resize`.
pub fn resize(source: &Bitmap, width: i32, height: i32, mode: ResizeMode) -> Result<Bitmap, String> {
    if width < 1 || height < 1 {
        return Err("width and height must be at least 1".into());
    }
    let mut out = imaging::create_bitmap(width as u32, height as u32);
    let (sw, sh) = (source.width() as i32, source.height() as i32);
    let mut src = Rect::xywh(0.0, 0.0, sw as f32, sh as f32);
    let mut dst = (0, 0, width, height);
    match mode {
        ResizeMode::Fill => src = fill_source_rect(sw, sh, width, height),
        ResizeMode::Fit => dst = fit_dest_rect(sw, sh, width, height),
        ResizeMode::Stretch => {}
    }
    {
        let mut c = Canvas::new(&mut out);
        let d = Rect::xywh(dst.0 as f32, dst.1 as f32, dst.2 as f32, dst.3 as f32);
        imaging::draw_image(&mut c, source, src, d, None);
    }
    Ok(out)
}

/// `ImageResizerService.Save`: PNG, or JPEG over `background`.
pub fn encode(image: &Bitmap, format: OutputFormat, jpeg_quality: i32, background: Rgba) -> Vec<u8> {
    match format {
        OutputFormat::Jpeg => {
            let flattened = imaging::fill_background(image, background);
            imaging::encode(&flattened, ImageFileFormat::Jpeg, jpeg_quality)
        }
        OutputFormat::Png => imaging::encode(image, ImageFileFormat::Png, 100),
    }
}

/// `GetPreviewSize`: at most 1600 pixels on the longer side.
pub fn preview_size(width: i32, height: i32) -> (i32, i32) {
    let largest = width.max(height);
    if largest <= MAX_PREVIEW_DIMENSION {
        return (width, height);
    }
    let scale = MAX_PREVIEW_DIMENSION as f64 / largest as f64;
    (round_even(width as f64 * scale).max(1), round_even(height as f64 * scale).max(1))
}

/// `ImageResizerService.CreatePreview`: the encoded preview (empty when the
/// image doesn't load).
pub fn create_preview(
    file: &str,
    width: i32,
    height: i32,
    mode: ResizeMode,
    format: OutputFormat,
    jpeg_quality: i32,
    background: Rgba,
) -> Result<Vec<u8>, String> {
    let Some(source) = imaging::load_image(file) else { return Ok(Vec::new()) };
    let (pw, ph) = preview_size(width, height);
    let output = resize(&source, pw, ph, mode)?;
    Ok(encode(&output, format, jpeg_quality, background))
}

/// `ResizeImages`: the output path for one image.
pub fn output_path(output_folder: &str, output_file_name: &str, source: &str, format: OutputFormat) -> String {
    let name = dotnet::file_name_without_extension(source);
    let path = dotnet::combine(output_folder, &output_file_name.replace("$filename", name));
    dotnet::change_extension(&path, if format == OutputFormat::Jpeg { "jpg" } else { "png" })
}

/// `ImageResizerViewModel.ResizeImages`.
#[allow(clippy::too_many_arguments)]
pub fn resize_images(
    files: &[String],
    width: i32,
    height: i32,
    mode: ResizeMode,
    format: OutputFormat,
    quality: i32,
    background: Rgba,
    output_folder: &str,
    output_file_name: &str,
) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for file in files {
        if !std::path::Path::new(file).is_file() {
            continue;
        }
        let Some(source) = imaging::load_image(file) else { continue };
        let output = resize(&source, width, height, mode)?;
        let path = output_path(output_folder, output_file_name, file, format);
        if let Some(dir) = std::path::Path::new(&path).parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        std::fs::write(&path, encode(&output, format, quality, background)).map_err(|e| e.to_string())?;
        out.push(path);
    }
    Ok(out)
}
