//! Capture › Region as ShareX runs it: `ShareX/CaptureHelpers/CaptureRegion
//! .cs` (freeze the screen, draw the pointer in, list the windows, open the
//! overlay) and what runs after it with a fresh profile's after capture
//! tasks (`CaptureBase.AfterCapture`, `UploadManager.RunImageTask`,
//! `WorkerTask.DoAfterCaptureJobs`): the capture sound, copy the image to
//! the clipboard, save it to the screenshots folder under ShareX's name
//! pattern.

use std::path::{Path, PathBuf};

use image::RgbaImage;
use sharex_capture::region::{CaptureResult, Request};
use sharex_capture::screenshot::{self, Screenshot};
use sharex_capture::{Rect, windows_list};
use sharex_helpers::file_helpers::unique_file_path;
use sharex_helpers::{NameParser, NameParserType};

use crate::task_settings::{FileExistAction, TaskSettings, after_capture, screenshots_parent_folder};

/// `CaptureRegion.ExecuteRegionCaptureAvaloniaAsync` up to the overlay:
/// the frozen screen (every monitor, or the active one), the pointer drawn
/// in (`ShowCursor`), the windows to snap to.
pub fn prepare(settings: &TaskSettings, last_regions: Vec<Rect>) -> Result<Request, String> {
    let screens = screenshot::screens();
    if screens.is_empty() {
        return Err("no monitors found".into());
    }
    let cursor = screenshot::cursor_position();
    let options = settings.region_capture_options.clone();
    let shot = Screenshot { capture_cursor: false, ..Default::default() };
    let at = cursor.unwrap_or((screens[0].bounds.x, screens[0].bounds.y));
    let screen_bounds = if options.active_monitor_mode {
        screenshot::active_screen_bounds(&screens, at)
    } else {
        screenshot::screen_bounds(&screens)
    };
    let mut frozen = if options.active_monitor_mode {
        shot.capture_active_monitor(&screens, at)
    } else {
        shot.capture_fullscreen(&screens)
    }
    .ok_or("couldn't capture the screen")?;
    if settings.show_cursor
        && let Some(c) = screenshot::cursor_image(&frozen, (screen_bounds.x, screen_bounds.y))
    {
        screenshot::draw_cursor(&mut frozen, &c);
    }
    let windows = if options.detect_windows {
        windows_list::window_info_list(options.detect_controls, &[])
    } else {
        Vec::new()
    };
    Ok(Request {
        screenshot: frozen,
        screen_bounds,
        screens,
        options,
        windows,
        cursor,
        last_regions,
        enable_annotations: !settings.region_capture_disable_annotation,
    })
}

/// `TaskMetadata`.
#[derive(Clone, Debug)]
pub struct Metadata {
    pub image: RgbaImage,
    pub window_title: Option<String>,
    pub process_name: Option<String>,
}

impl Metadata {
    /// `new TaskMetadata(output)` plus `UpdateInfo(result.WindowInfo)`.
    pub fn from_result(result: &CaptureResult) -> Self {
        let info = result.window_info.as_ref();
        Self {
            image: result.image.clone(),
            window_title: info.map(|i| i.title.clone()),
            process_name: info.and_then(|i| i.process_name.clone()),
        }
    }
}

/// `TaskHelpers.GetFileName`.
pub fn file_name(settings: &mut TaskSettings, extension: &str, metadata: &Metadata) -> String {
    let mut parser = NameParser::new(NameParserType::FileName);
    parser.auto_increment_number = settings.name_parser_auto_increment_number;
    parser.max_name_length = settings.name_pattern_max_length;
    parser.max_title_length = settings.name_pattern_max_title_length;
    parser.image_width = metadata.image.width();
    parser.image_height = metadata.image.height();
    parser.window_text = metadata.window_title.clone();
    parser.process_name = metadata.process_name.clone();
    let has_title = parser.window_text.as_deref().is_some_and(|t| !t.is_empty());
    let pattern = if !settings.name_format_pattern_active_window.is_empty() && has_title {
        settings.name_format_pattern_active_window.clone()
    } else {
        settings.name_format_pattern.clone()
    };
    let mut name = parser.parse(&pattern);
    settings.name_parser_auto_increment_number = parser.auto_increment_number;
    if !extension.is_empty() {
        name = format!("{name}.{}", extension.trim_start_matches('.'));
    }
    name
}

/// `TaskHelpers.GetScreenshotsFolder`.
pub fn screenshots_folder(settings: &TaskSettings, metadata: &Metadata) -> PathBuf {
    let mut parser = NameParser::new(NameParserType::FilePath);
    parser.image_width = metadata.image.width();
    parser.image_height = metadata.image.height();
    parser.window_text = metadata.window_title.clone();
    parser.process_name = metadata.process_name.clone();
    let has_title = parser.window_text.as_deref().is_some_and(|t| !t.is_empty());
    let pattern = if !settings.save_image_sub_folder_pattern_window.is_empty() && has_title {
        &settings.save_image_sub_folder_pattern_window
    } else {
        &settings.save_image_sub_folder_pattern
    };
    screenshots_parent_folder().join(parser.parse(pattern))
}

/// `Path.ChangeExtension`.
fn change_extension(name: &str, ext: &str) -> String {
    let stem = match name.rfind('.') {
        Some(i) if !name[i..].contains(['/', '\\']) => &name[..i],
        _ => name,
    };
    format!("{stem}.{ext}")
}

/// `TaskHelpers.PrepareImage`: PNG, or JPEG on white when the PNG is over
/// `ImageAutoUseJPEGSize` KB (`ImageAutoUseJPEG`).
pub fn prepare_image(image: &RgbaImage, settings: &TaskSettings) -> Result<(Vec<u8>, &'static str), String> {
    let png = encode_png(image)?;
    let limit = settings.image_auto_use_jpeg_size * 1000;
    if settings.image_auto_use_jpeg && png.len() as u64 > limit {
        // `FillBackground(img, Color.White)` then `SaveJPEG`.
        let rgb = image::RgbImage::from_fn(image.width(), image.height(), |x, y| {
            let p = image.get_pixel(x, y);
            let a = p[3] as u32;
            let c = |v: u8| ((v as u32 * a + 255 * (255 - a) + 127) / 255) as u8;
            image::Rgb([c(p[0]), c(p[1]), c(p[2])])
        });
        let mut out = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, settings.image_jpeg_quality)
            .encode_image(&rgb)
            .map_err(|e| e.to_string())?;
        return Ok((out, "jpg"));
    }
    Ok((png, "png"))
}

/// `SkiaImageHelpers.SavePNG` with ShareX's encoder options (compression
/// level 1, all filters), opaque images as RGB.
fn encode_png(image: &RgbaImage) -> Result<Vec<u8>, String> {
    use image::ImageEncoder;
    use image::codecs::png::{CompressionType, FilterType, PngEncoder};
    let mut out = Vec::new();
    let encoder = PngEncoder::new_with_quality(&mut out, CompressionType::Fast, FilterType::Adaptive);
    if image.pixels().all(|p| p[3] == 255) {
        let rgb: Vec<u8> = image.pixels().flat_map(|p| [p[0], p[1], p[2]]).collect();
        encoder.write_image(&rgb, image.width(), image.height(), image::ExtendedColorType::Rgb8)
    } else {
        encoder.write_image(image.as_raw(), image.width(), image.height(), image::ExtendedColorType::Rgba8)
    }
    .map_err(|e| e.to_string())?;
    Ok(out)
}

/// `TaskHelpers.HandleExistsFile`. Asking (the default) needs ShareX's
/// file exists window, not ported yet: a unique name is used instead.
fn handle_exists_file(path: &Path, settings: &TaskSettings) -> Option<PathBuf> {
    if !path.exists() {
        return Some(path.to_path_buf());
    }
    match settings.file_exist_action {
        FileExistAction::Overwrite => Some(path.to_path_buf()),
        FileExistAction::Cancel => None,
        FileExistAction::Ask | FileExistAction::UniqueName => Some(unique_file_path(path)),
    }
}

/// What the after capture tasks did.
#[derive(Debug, Default)]
pub struct TaskOutcome {
    pub file_path: Option<PathBuf>,
    pub copied: bool,
}

/// `WorkerTask.DoAfterCaptureJobs` for the tasks ported: copy the image
/// to the clipboard (owned by `owner`), then save it.
pub fn run_image_task(metadata: Metadata, mut settings: TaskSettings, owner: Option<isize>) -> Result<TaskOutcome, String> {
    let mut outcome = TaskOutcome::default();
    // `CreateImageUploaderTask`: named with a "bmp" extension first.
    let mut name = file_name(&mut settings, "bmp", &metadata);
    if settings.after_capture_job & after_capture::COPY_IMAGE_TO_CLIPBOARD != 0 {
        match sharex_capture::clipboard::copy_image(&metadata.image, owner) {
            Ok(()) => {
                outcome.copied = true;
                eprintln!("Image copied to clipboard.");
            }
            Err(e) => eprintln!("Clipboard copy image failed: {e}"),
        }
    }
    if settings.after_capture_job & after_capture::SAVE_IMAGE_TO_FILE != 0 {
        let (data, ext) = prepare_image(&metadata.image, &settings)?;
        name = change_extension(&name, ext);
        let folder = screenshots_folder(&settings, &metadata);
        if let Some(path) = handle_exists_file(&folder.join(&name), &settings) {
            std::fs::create_dir_all(&folder).map_err(|e| format!("{}: {e}", folder.display()))?;
            std::fs::write(&path, data).map_err(|e| format!("{}: {e}", path.display()))?;
            eprintln!("Image saved to file: {}", path.display());
            outcome.file_path = Some(path);
        }
    }
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metadata(title: Option<&str>, process: Option<&str>) -> Metadata {
        Metadata {
            image: RgbaImage::from_pixel(4, 3, image::Rgba([1, 2, 3, 255])),
            window_title: title.map(str::to_string),
            process_name: process.map(str::to_string),
        }
    }

    #[test]
    fn names_follow_the_default_patterns() {
        let mut s = TaskSettings::default();
        let name = file_name(&mut s, "bmp", &metadata(None, None));
        assert_eq!(name.len(), 10 + 4, "{name}");
        assert!(name.ends_with(".bmp"));
        let name = file_name(&mut s, "bmp", &metadata(Some("ShareXrs test pattern"), Some("sharexrs")));
        assert!(name.starts_with("sharexrs_") && name.len() == "sharexrs_".len() + 10 + 4, "{name}");
        assert_eq!(change_extension(&name, "png"), name.replace(".bmp", ".png"));
    }

    #[test]
    fn screenshots_go_in_a_year_month_folder() {
        let s = TaskSettings::default();
        let folder = screenshots_folder(&s, &metadata(Some("t"), Some("p")));
        let now = chrono_like_year_month();
        assert!(folder.ends_with(now), "{}", folder.display());
        assert!(folder.parent().unwrap().ends_with("Screenshots"));
    }

    fn chrono_like_year_month() -> String {
        NameParser::new(NameParserType::FilePath).parse("%y-%mo")
    }

    #[test]
    fn small_images_are_png_and_big_noisy_ones_jpeg() {
        let s = TaskSettings::default();
        let (png, ext) = prepare_image(&RgbaImage::from_pixel(10, 10, image::Rgba([9, 9, 9, 255])), &s).unwrap();
        assert_eq!(ext, "png");
        let decoded = image::load_from_memory(&png).unwrap();
        assert_eq!(decoded.color(), image::ColorType::Rgb8);
        let mut noisy = RgbaImage::new(1400, 1400);
        let mut v = 1u32;
        for p in noisy.pixels_mut() {
            v = v.wrapping_mul(1_103_515_245).wrapping_add(12345);
            *p = image::Rgba([(v >> 8) as u8, (v >> 16) as u8, (v >> 24) as u8, 255]);
        }
        assert_eq!(prepare_image(&noisy, &s).unwrap().1, "jpg");
    }
}
