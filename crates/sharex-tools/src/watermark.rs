//! Image watermark: `ImageWatermarkService` and `ImageWatermarkViewModel.WatermarkImages`.
//!
//! The image watermark matches ShareX pixel for pixel. The text watermark
//! measures and places text as ShareX does (SKTypeface.Default, Segoe UI under
//! DirectWrite) but fills glyph outlines where Skia rasterises DirectWrite glyph
//! masks, so glyph edges differ (as the Text watermark image effect, see
//! docs/IMAGE-EFFECTS.md); characters the default font lacks aren't drawn with
//! a fallback font (`FallbackFonts`).

use sharex_image_effects::dotnet::{FontUnit, ImageFont};
use sharex_image_effects::text::{Fonts, SkFont};
use sharex_skia::raster::{Canvas, Paint, Rect, Shader};

use crate::dotnet;
use crate::imaging::{self, Bitmap, ImageFileFormat, Rgba};

/// `ImageWatermarkType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatermarkType {
    Text,
    Image,
}

/// `ImageWatermarkPosition`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Position {
    TopLeft,
    TopCenter,
    TopRight,
    MiddleLeft,
    Center,
    MiddleRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

impl Position {
    pub const ALL: [Position; 9] = [
        Position::TopLeft,
        Position::TopCenter,
        Position::TopRight,
        Position::MiddleLeft,
        Position::Center,
        Position::MiddleRight,
        Position::BottomLeft,
        Position::BottomCenter,
        Position::BottomRight,
    ];
}

/// `ImageWatermarkOutputFormat`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Png,
    Jpeg,
}

/// `ImageWatermarkOptions`.
#[derive(Debug, Clone, PartialEq)]
pub struct WatermarkOptions {
    pub kind: WatermarkType,
    pub text: String,
    pub position: Position,
    pub margin: i32,
    pub opacity: i32,
    pub text_size: f32,
    pub text_color: Rgba,
    pub image_scale: i32,
    pub rotation: f32,
}

impl Default for WatermarkOptions {
    /// The window's defaults.
    fn default() -> Self {
        WatermarkOptions {
            kind: WatermarkType::Text,
            text: sharex_strings::tools::ImageWatermarkWindow_Watermark().to_string(),
            position: Position::BottomRight,
            margin: 24,
            opacity: 70,
            text_size: 48.0,
            text_color: imaging::WHITE,
            image_scale: 25,
            rotation: 0.0,
        }
    }
}

pub const DEFAULT_FILE_NAME: &str = "$filename_watermarked";
const MAX_PREVIEW_DIMENSION: i32 = 1600;

/// `SKFont(SKTypeface.Default, size)`.
fn default_font(fonts: &mut Fonts, size: f32) -> Option<SkFont> {
    fonts.font(&ImageFont { name: "Segoe UI".into(), size, style: 0, unit: FontUnit::Pixel })
}

/// `CreateTextWatermark`.
pub fn create_text_watermark(o: &WatermarkOptions) -> Option<Bitmap> {
    if o.text.trim().is_empty() {
        return None;
    }
    let lines: Vec<String> = o.text.replace('\r', "").split('\n').map(str::to_string).collect();
    let mut fonts = Fonts::default();
    let font = default_font(&mut fonts, o.text_size.max(1.0))?;
    let width = lines.iter().map(|l| font.measure(l)).fold(f32::MIN, f32::max);
    let m = font.metrics();
    let ascent = m.ascent;
    let line_height = m.descent + m.leading - ascent;
    let w = ((width.ceil() as i32) + 4).max(1);
    let h = (((line_height * lines.len() as f32).ceil() as i32) + 4).max(1);
    let mut wm = imaging::create_bitmap(w as u32, h as u32);
    let alpha = (o.opacity.clamp(0, 100) as f64 / 100.0 * 255.0).round_ties_even() as u8;
    let color = [o.text_color[0], o.text_color[1], o.text_color[2], alpha];
    {
        let mut c = Canvas::new(&mut wm);
        let paint = Paint::fill(color);
        for (i, line) in lines.iter().enumerate() {
            let mut pb = tiny_skia::PathBuilder::new();
            font.line_path(&mut pb, line, 2.0, 2.0 - ascent + i as f32 * line_height, true);
            if let Some(path) = pb.finish() {
                c.fill_path(&path, &paint);
            }
        }
    }
    Some(wm)
}

/// `CreateImageWatermark`: the image scaled to `ImageScale`% of the canvas
/// (keeping its aspect), drawn with the opacity as the paint's alpha.
pub fn create_image_watermark(image: Option<&Bitmap>, canvas_w: i32, canvas_h: i32, o: &WatermarkOptions) -> Option<Bitmap> {
    let image = image.filter(|i| i.width() >= 1 && i.height() >= 1)?;
    let percentage = o.image_scale.clamp(1, 100) as f64 / 100.0;
    let scale = (canvas_w as f64 * percentage / image.width() as f64)
        .min(canvas_h as f64 * percentage / image.height() as f64);
    let width = ((image.width() as f64 * scale).round_ties_even() as i32).max(1);
    let height = ((image.height() as f64 * scale).round_ties_even() as i32).max(1);
    let mut wm = imaging::create_bitmap(width as u32, height as u32);
    let mut paint = Paint::shader(Shader::Color([255, 255, 255, 255]));
    paint.alpha = (o.opacity.clamp(0, 100) as f64 * 2.55).round_ties_even() as u8;
    {
        let mut c = Canvas::new(&mut wm);
        let src = Rect::xywh(0.0, 0.0, image.width() as f32, image.height() as f32);
        imaging::draw_image(&mut c, image, src, Rect::xywh(0.0, 0.0, width as f32, height as f32), Some(&paint));
    }
    Some(wm)
}

/// `Rotate`: the bounding box of the rotated watermark, drawn rotated about
/// its centre (a copy when the angle is under 0.01°).
pub fn rotate(source: &Bitmap, angle: f32) -> Bitmap {
    let angle = angle % 360.0;
    if angle.abs() < 0.01 {
        return source.clone();
    }
    let radians = angle as f64 * std::f64::consts::PI / 180.0;
    let (sin, cos) = (radians.sin().abs(), radians.cos().abs());
    let (sw, sh) = (source.width() as f64, source.height() as f64);
    let width = ((sw * cos + sh * sin).ceil() as i32).max(1);
    let height = ((sw * sin + sh * cos).ceil() as i32).max(1);
    let mut out = imaging::create_bitmap(width as u32, height as u32);
    {
        let mut c = Canvas::new(&mut out);
        c.translate(width as f32 / 2.0, height as f32 / 2.0);
        c.rotate(angle);
        c.translate(-(source.width() as f32) / 2.0, -(source.height() as f32) / 2.0);
        let r = Rect::xywh(0.0, 0.0, source.width() as f32, source.height() as f32);
        imaging::draw_image(&mut c, source, r, r, None);
    }
    out
}

/// `GetLocation`.
pub fn location(cw: i32, ch: i32, ww: i32, wh: i32, position: Position, margin: i32) -> (i32, i32) {
    let (left, cx, right) = (margin, (cw - ww) / 2, cw - ww - margin);
    let (top, cy, bottom) = (margin, (ch - wh) / 2, ch - wh - margin);
    match position {
        Position::TopLeft => (left, top),
        Position::TopCenter => (cx, top),
        Position::TopRight => (right, top),
        Position::MiddleLeft => (left, cy),
        Position::Center => (cx, cy),
        Position::MiddleRight => (right, cy),
        Position::BottomLeft => (left, bottom),
        Position::BottomCenter => (cx, bottom),
        Position::BottomRight => (right, bottom),
    }
}

/// `ImageWatermarkService.Apply`.
pub fn apply(source: &Bitmap, watermark_image: Option<&Bitmap>, o: &WatermarkOptions) -> Bitmap {
    let mut out = source.clone();
    let wm = match o.kind {
        WatermarkType::Text => create_text_watermark(o),
        WatermarkType::Image => create_image_watermark(watermark_image, source.width() as i32, source.height() as i32, o),
    };
    let Some(wm) = wm else { return out };
    let rotated = rotate(&wm, o.rotation);
    let (x, y) = location(
        source.width() as i32,
        source.height() as i32,
        rotated.width() as i32,
        rotated.height() as i32,
        o.position,
        o.margin,
    );
    Canvas::new(&mut out).draw_bitmap(&rotated, x as f32, y as f32, None);
    out
}

/// `ImageWatermarkService.Save`: PNG, or JPEG over `background`.
pub fn encode(image: &Bitmap, format: OutputFormat, jpeg_quality: i32, background: Rgba) -> Vec<u8> {
    match format {
        OutputFormat::Jpeg => imaging::encode(&imaging::fill_background(image, background), ImageFileFormat::Jpeg, jpeg_quality),
        OutputFormat::Png => imaging::encode(image, ImageFileFormat::Png, 100),
    }
}

/// `GetPreviewSize`.
pub fn preview_size(w: i32, h: i32) -> (i32, i32) {
    let largest = w.max(h);
    if largest <= MAX_PREVIEW_DIMENSION {
        return (w, h);
    }
    let scale = MAX_PREVIEW_DIMENSION as f64 / largest as f64;
    (
        ((w as f64 * scale).round_ties_even() as i32).max(1),
        ((h as f64 * scale).round_ties_even() as i32).max(1),
    )
}

/// `ImageWatermarkService.CreatePreview`: the watermark applied to the image
/// scaled to at most 1600 pixels (margin and text size scaled with it).
pub fn create_preview(
    path: &str,
    watermark_image_path: &str,
    o: &WatermarkOptions,
    format: OutputFormat,
    jpeg_quality: i32,
    background: Rgba,
) -> (Vec<u8>, u32, u32) {
    let Some(source) = imaging::load_image(path) else { return (Vec::new(), 0, 0) };
    let (pw, ph) = preview_size(source.width() as i32, source.height() as i32);
    let preview_scale = pw as f64 / source.width() as f64;
    let preview_source = imaging::resize(&source, pw as u32, ph as u32);
    let wm_image = if o.kind == WatermarkType::Image { imaging::load_image(watermark_image_path) } else { None };
    let po = WatermarkOptions {
        margin: ((o.margin as f64 * preview_scale).round_ties_even() as i32).max(0),
        text_size: ((o.text_size as f64 * preview_scale) as f32).max(1.0),
        ..o.clone()
    };
    let output = apply(&preview_source, wm_image.as_ref(), &po);
    (encode(&output, format, jpeg_quality, background), source.width(), source.height())
}

/// The output path for one image (`WatermarkImages`).
pub fn output_path(output_folder: &str, output_file_name: &str, source: &str, format: OutputFormat) -> String {
    let name = dotnet::file_name_without_extension(source);
    let path = dotnet::combine(output_folder, &output_file_name.replace("$filename", name));
    dotnet::change_extension(&path, if format == OutputFormat::Jpeg { "jpg" } else { "png" })
}

/// `ImageWatermarkViewModel.WatermarkImages`.
#[allow(clippy::too_many_arguments)]
pub fn watermark_images(
    files: &[String],
    watermark_image_path: &str,
    o: &WatermarkOptions,
    format: OutputFormat,
    quality: i32,
    background: Rgba,
    output_folder: &str,
    output_file_name: &str,
) -> std::io::Result<Vec<String>> {
    let wm_image = if o.kind == WatermarkType::Image { imaging::load_image(watermark_image_path) } else { None };
    let mut out = Vec::new();
    for file in files {
        if !std::path::Path::new(file).is_file() {
            continue;
        }
        let Some(source) = imaging::load_image(file) else { continue };
        let output = apply(&source, wm_image.as_ref(), o);
        let path = output_path(output_folder, output_file_name, file, format);
        if let Some(dir) = std::path::Path::new(&path).parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&path, encode(&output, format, quality, background))?;
        out.push(path);
    }
    Ok(out)
}
