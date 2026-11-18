//! Capture › Region as ShareX runs it: `ShareX/CaptureHelpers/CaptureRegion
//! .cs` (freeze the screen, draw the pointer in, list the windows, open the
//! overlay) and what runs after it with a fresh profile's after capture
//! tasks (`CaptureBase.AfterCapture`, `UploadManager.RunImageTask`,
//! `WorkerTask.DoAfterCaptureJobs`): the capture sound, add image effects,
//! copy the image to the clipboard, save it to the screenshots folder under
//! ShareX's name pattern.

use std::path::{Path, PathBuf};

use image::RgbaImage;
use sharex_capture::region::{CaptureResult, Request};
use sharex_capture::screenshot::{self, Screenshot};
use sharex_capture::{Rect, windows_list};
use sharex_helpers::file_helpers::unique_file_path;
use sharex_helpers::{NameParser, NameParserType};
use sharex_settings::{AfterCaptureTasks, ApplicationConfig, FileExistAction, TaskSettings};

/// `CaptureRegion.ExecuteRegionCaptureAvaloniaAsync` up to the overlay:
/// the frozen screen (every monitor, or the active one), the pointer drawn
/// in (`ShowCursor`), the windows to snap to.
pub fn prepare(task: &TaskSettings, last_regions: Vec<Rect>) -> Result<Request, String> {
    let screens = screenshot::screens();
    if screens.is_empty() {
        return Err("no monitors found".into());
    }
    let cursor = screenshot::cursor_position();
    let capture = task.CaptureSettings.clone().unwrap_or_default();
    let options = capture.RegionCaptureOptions.clone();
    let shot = Screenshot { capture_cursor: false, ..Default::default() };
    let at = cursor.unwrap_or((screens[0].bounds.x, screens[0].bounds.y));
    let screen_bounds = if options.ActiveMonitorMode {
        screenshot::active_screen_bounds(&screens, at)
    } else {
        screenshot::screen_bounds(&screens)
    };
    let mut frozen = if options.ActiveMonitorMode {
        shot.capture_active_monitor(&screens, at)
    } else {
        shot.capture_fullscreen(&screens)
    }
    .ok_or("couldn't capture the screen")?;
    let enable_annotations = !task.AdvancedSettings.as_ref().is_some_and(|a| a.RegionCaptureDisableAnnotation);
    // The pointer becomes an image annotation over the frozen screen; with
    // annotations off (no workspace here) it's drawn in, which renders the
    // same.
    let mut cursor_image = None;
    if capture.ShowCursor
        && let Some(c) = screenshot::cursor_image(&frozen, (screen_bounds.x, screen_bounds.y))
    {
        if enable_annotations {
            cursor_image = Some(c);
        } else {
            screenshot::draw_cursor(&mut frozen, &c);
        }
    }
    let windows = if options.DetectWindows {
        windows_list::window_info_list(options.DetectControls, &[])
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
        enable_annotations,
        editor_options: image_editor_options(task),
        cursor_image,
        allow_multiple_regions: true,
        pixel_info: Some(pixel_info),
    })
}

/// `CodeMenuEntryPixelInfo.Parse` for the overlay's custom info text.
pub(crate) fn pixel_info(text: &str, c: [u8; 4], position: (i32, i32)) -> String {
    let color = sharex_tools::color::Color::argb(c[3], c[0], c[1], c[2]);
    sharex_tools::color::pixel_info_parse(text, &color, position)
}

/// `RegionCaptureTasks.CaptureAsync` for `GetRectangleRegionAsync` (screen
/// recording's region): the frozen screen without the pointer, and no
/// annotations (`EnableAnnotations = false`).
pub fn prepare_selection(task: &TaskSettings, last_regions: Vec<Rect>) -> Result<Request, String> {
    let mut task = task.clone();
    task.CaptureSettings.get_or_insert_default().ShowCursor = false;
    let mut request = prepare(&task, last_regions)?;
    request.enable_annotations = false;
    request.allow_multiple_regions = false;
    request.editor_options = sharex_image_editor::ImageEditorOptions::default();
    Ok(request)
}

/// `TaskSettings.ToolsSettingsReference.ImageEditorOptions`.
pub fn image_editor_options(task: &TaskSettings) -> sharex_image_editor::ImageEditorOptions {
    task.ToolsSettings
        .as_ref()
        .map(|t| sharex_image_editor::ImageEditorOptions::from_value(&t.ImageEditorOptions))
        .unwrap_or_default()
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

/// `TaskHelpers.GetFileName`. `auto_increment` is
/// `ApplicationConfig.NameParserAutoIncrementNumber`, counted up when the
/// pattern uses it.
pub fn file_name(task: &TaskSettings, auto_increment: &mut i32, extension: &str, metadata: &Metadata) -> String {
    let upload = task.UploadSettings.clone().unwrap_or_default();
    let advanced = task.AdvancedSettings.clone().unwrap_or_default();
    let mut parser = NameParser::new(NameParserType::FileName);
    parser.auto_increment_number = *auto_increment as i64;
    parser.max_name_length = advanced.NamePatternMaxLength.max(0) as usize;
    parser.max_title_length = advanced.NamePatternMaxTitleLength.max(0) as usize;
    parser.image_width = metadata.image.width();
    parser.image_height = metadata.image.height();
    parser.window_text = metadata.window_title.clone();
    parser.process_name = metadata.process_name.clone();
    let has_title = parser.window_text.as_deref().is_some_and(|t| !t.is_empty());
    let pattern = if !upload.NameFormatPatternActiveWindow.is_empty() && has_title {
        &upload.NameFormatPatternActiveWindow
    } else {
        &upload.NameFormatPattern
    };
    let mut name = parser.parse(pattern);
    *auto_increment = parser.auto_increment_number as i32;
    if !extension.is_empty() {
        name = format!("{name}.{}", extension.trim_start_matches('.'));
    }
    name
}

/// `TaskHelpers.GetScreenshotsFolder`: the task's own folder, else
/// `parent` (`AppPaths.ScreenshotsParentFolder`) and the sub folder pattern.
pub fn screenshots_folder(task: &TaskSettings, app: &ApplicationConfig, parent: &Path, metadata: &Metadata) -> PathBuf {
    let mut parser = NameParser::new(NameParserType::FilePath);
    parser.image_width = metadata.image.width();
    parser.image_height = metadata.image.height();
    parser.window_text = metadata.window_title.clone();
    parser.process_name = metadata.process_name.clone();
    if task.OverrideScreenshotsFolder && !task.ScreenshotsFolder.is_empty() {
        return PathBuf::from(parser.parse(&task.ScreenshotsFolder));
    }
    let has_title = parser.window_text.as_deref().is_some_and(|t| !t.is_empty());
    let pattern = if !app.SaveImageSubFolderPatternWindow.is_empty() && has_title {
        &app.SaveImageSubFolderPatternWindow
    } else {
        &app.SaveImageSubFolderPattern
    };
    parent.join(parser.parse(pattern))
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
pub fn prepare_image(image: &RgbaImage, task: &TaskSettings) -> Result<(Vec<u8>, &'static str), String> {
    let settings = task.ImageSettings.clone().unwrap_or_default();
    let png = encode_png(image)?;
    let limit = settings.ImageAutoUseJPEGSize.max(0) as u64 * 1000;
    if settings.ImageAutoUseJPEG && png.len() as u64 > limit {
        // `FillBackground(img, Color.White)` then `SaveJPEG`.
        let rgb = image::RgbImage::from_fn(image.width(), image.height(), |x, y| {
            let p = image.get_pixel(x, y);
            let a = p[3] as u32;
            let c = |v: u8| ((v as u32 * a + 255 * (255 - a) + 127) / 255) as u8;
            image::Rgb([c(p[0]), c(p[1]), c(p[2])])
        });
        let mut out = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, settings.ImageJPEGQuality.clamp(1, 100) as u8)
            .encode_image(&rgb)
            .map_err(|e| e.to_string())?;
        return Ok((out, "jpg"));
    }
    Ok((png, "png"))
}

/// `SkiaImageHelpers.SavePNG` with ShareX's encoder options (compression
/// level 1, all filters) and the default bit depth: the bitmap as it is,
/// which for screenshots is 32 bpp RGBA even when opaque.
fn encode_png(image: &RgbaImage) -> Result<Vec<u8>, String> {
    use image::ImageEncoder;
    use image::codecs::png::{CompressionType, FilterType, PngEncoder};
    let mut out = Vec::new();
    PngEncoder::new_with_quality(&mut out, CompressionType::Fast, FilterType::Adaptive)
        .write_image(image.as_raw(), image.width(), image.height(), image::ExtendedColorType::Rgba8)
        .map_err(|e| e.to_string())?;
    Ok(out)
}

/// `TaskHelpers.HandleExistsFile`. Asking (the default) needs ShareX's
/// file exists window, not ported yet: a unique name is used instead.
pub fn handle_exists_file(path: &Path, task: &TaskSettings) -> Option<PathBuf> {
    if !path.exists() {
        return Some(path.to_path_buf());
    }
    match task.ImageSettings.as_ref().map(|s| &s.FileExistAction) {
        Some(FileExistAction::Overwrite) => Some(path.to_path_buf()),
        Some(FileExistAction::Cancel) => None,
        _ => Some(unique_file_path(path)),
    }
}

/// What the after capture tasks did.
#[derive(Debug, Default)]
pub struct TaskOutcome {
    pub file_path: Option<PathBuf>,
    pub copied: bool,
    /// The encoded image (`WorkerTask.Data`) and its file name, when it was
    /// prepared for saving or uploading.
    pub data: Option<Vec<u8>>,
    pub file_name: Option<String>,
    /// The image after the after capture tasks (`WorkerTask.Image`,
    /// `Info.Metadata.Image`): for the upload windows and OCR.
    pub image: Option<RgbaImage>,
}

/// An image task ready to run off the event loop, with what
/// `WorkerTask.CreateImageUploaderTask` sets up on the UI thread: the file
/// name (with a "bmp" extension first, as ShareX) and the folder.
pub struct ImageTask {
    pub metadata: Metadata,
    pub task: TaskSettings,
    pub file_name: String,
    pub folder: PathBuf,
    /// `AppPaths.ImageEffectsFolder` (`%ShareXImageEffects%` in effect paths).
    pub image_effects_folder: PathBuf,
}

impl ImageTask {
    pub fn new(
        metadata: Metadata,
        task: TaskSettings,
        app: &mut ApplicationConfig,
        screenshots_parent: &Path,
        image_effects_folder: PathBuf,
    ) -> Self {
        let mut counter = app.NameParserAutoIncrementNumber;
        let file_name = file_name(&task, &mut counter, "bmp", &metadata);
        app.NameParserAutoIncrementNumber = counter;
        let folder = screenshots_folder(&task, app, screenshots_parent, &metadata);
        Self { metadata, task, file_name, folder, image_effects_folder }
    }

    /// `WorkerTask.DoAfterCaptureJobs` for the tasks ported so far: add image
    /// effects, copy the image to the clipboard (owned by `owner`), then save it.
    pub fn run(mut self, owner: Option<isize>, ui: Option<&dyn crate::upload::TaskUi>) -> Result<TaskOutcome, String> {
        let mut outcome = TaskOutcome::default();
        let job = self.task.AfterCaptureJob;
        if job.contains(AfterCaptureTasks::AddImageEffects) {
            self.metadata.image =
                self.apply_image_effects(ui).ok_or("Error: Applying image effects resulted empty image.")?;
        }
        // `TaskHelpers.AnnotateImage(Image, null, Info.TaskSettings, true)`:
        // cancelled, the task stops.
        if job.contains(AfterCaptureTasks::AnnotateImage)
            && let Some(ui) = ui
        {
            match ui.annotate_image(&self.metadata.image, &self.task) {
                Some(image) => self.metadata.image = image,
                None => {
                    eprintln!("Annotate image: cancelled, the task stops.");
                    return Ok(outcome);
                }
            }
        }
        if job.contains(AfterCaptureTasks::CopyImageToClipboard) {
            match sharex_capture::clipboard::copy_image(&self.metadata.image, owner) {
                Ok(()) => {
                    outcome.copied = true;
                    eprintln!("Image copied to clipboard.");
                }
                Err(e) => eprintln!("Clipboard copy image failed: {e}"),
            }
        }
        if job.contains(AfterCaptureTasks::PinToScreen)
            && let Some(ui) = ui
        {
            ui.pin_to_screen(&self.metadata.image, &self.task);
        }
        if job.contains(AfterCaptureTasks::SaveImageToFile) || job.contains(AfterCaptureTasks::UploadImageToHost) {
            let (data, ext) = prepare_image(&self.metadata.image, &self.task)?;
            let name = change_extension(&self.file_name, ext);
            if job.contains(AfterCaptureTasks::SaveImageToFile)
                && let Some(path) = handle_exists_file(&self.folder.join(&name), &self.task)
            {
                std::fs::create_dir_all(&self.folder).map_err(|e| format!("{}: {e}", self.folder.display()))?;
                std::fs::write(&path, &data).map_err(|e| format!("{}: {e}", path.display()))?;
                eprintln!("Image saved to file: {}", path.display());
                outcome.file_path = Some(path);
            }
            outcome.data = Some(data);
            outcome.file_name = Some(name);
        }
        outcome.image = Some(self.metadata.image);
        Ok(outcome)
    }
}

impl ImageTask {
    /// `TaskHelpers.ApplyImageEffects`: the selected (or a random) preset of the
    /// task's image settings; `None` when ShareX's result would be null.
    fn apply_image_effects(&self, ui: Option<&dyn crate::upload::TaskUi>) -> Option<RgbaImage> {
        use sharex_image_effects::{Bitmap, Context, ImageEffectPreset, ImageEffectsSettings};
        let mut settings = self.task.ImageSettings.clone().unwrap_or_default();
        if settings.ShowImageEffectsWindowAfterCapture
            && let Some((presets, index)) = ui.and_then(|ui| ui.image_effects_window(&self.metadata.image, &self.task))
        {
            // The window edits the task's presets; the selected one is applied
            // whether it was closed with Continue or Cancel.
            settings.ImageEffectPresets = presets;
            settings.SelectedImageEffectPreset = index;
        }
        // One preset per entry, so the selected index stays ShareX's.
        let presets: Vec<ImageEffectPreset> =
            settings.ImageEffectPresets.iter().map(|v| ImageEffectPreset::from_json(v).unwrap_or_default()).collect();
        let effects = ImageEffectsSettings {
            presets: &presets,
            selected: settings.SelectedImageEffectPreset,
            use_random: settings.UseRandomImageEffect,
        };
        let mut ctx = Context {
            special_folders: vec![("ShareXImageEffects".into(), self.image_effects_folder.clone())],
            ..Default::default()
        };
        let image = &self.metadata.image;
        let bmp = Bitmap::from_straight(image.width(), image.height(), image.as_raw());
        let out = sharex_image_effects::apply_image_effects(bmp, &effects, &mut ctx)?;
        if out.is_empty() {
            return None;
        }
        RgbaImage::from_raw(out.width(), out.height(), out.to_straight())
    }
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
    fn add_image_effects_applies_the_selected_preset() {
        let mut task = TaskSettings::default();
        let s = task.ImageSettings.get_or_insert_with(Default::default);
        let inverse = serde_json::json!({ "Name": "", "Effects": [{ "$type": "ShareX.ImageEffectsLib.Inverse, ShareX.ImageEffectsLib", "Enabled": true }] });
        s.ImageEffectPresets = vec![serde_json::json!({ "Name": "", "Effects": [] }), inverse];
        s.SelectedImageEffectPreset = 1;
        let job = ImageTask {
            metadata: metadata(None, None),
            task,
            file_name: String::new(),
            folder: PathBuf::new(),
            image_effects_folder: PathBuf::new(),
        };
        let out = job.apply_image_effects(None).unwrap();
        assert_eq!(out.get_pixel(0, 0).0, [254, 253, 252, 255]);
    }

    /// `ShowImageEffectsWindowAfterCapture`: the window's presets and
    /// selected index decide what's applied (here it switches to Inverse).
    #[test]
    fn the_image_effects_window_picks_the_preset() {
        struct Ui(Vec<serde_json::Value>);
        impl crate::upload::TaskUi for Ui {
            fn copy_text(&self, _: &str) {}
            fn clipboard_contains_text(&self) -> bool {
                false
            }
            fn clear_clipboard(&self) {}
            fn open_url(&self, _: &str) {}
            fn show_qr_code(&self, _: &str) {}
            fn copy_file(&self, _: &std::path::Path) {}
            fn open_folder_with_file(&self, _: &std::path::Path) {}
            fn image_effects_window(&self, _: &RgbaImage, _: &TaskSettings) -> Option<(Vec<serde_json::Value>, i32)> {
                Some((self.0.clone(), 1))
            }
        }
        let mut task = TaskSettings::default();
        let s = task.ImageSettings.get_or_insert_with(Default::default);
        s.ShowImageEffectsWindowAfterCapture = true;
        s.ImageEffectPresets = vec![serde_json::json!({ "Name": "", "Effects": [] })];
        s.SelectedImageEffectPreset = 0;
        let inverse = serde_json::json!({ "Name": "", "Effects": [{ "$type": "ShareX.ImageEffectsLib.Inverse, ShareX.ImageEffectsLib", "Enabled": true }] });
        let edited = vec![serde_json::json!({ "Name": "", "Effects": [] }), inverse];
        let job = ImageTask { metadata: metadata(None, None), task, file_name: String::new(), folder: PathBuf::new(), image_effects_folder: PathBuf::new() };
        let out = job.apply_image_effects(Some(&Ui(edited))).unwrap();
        assert_eq!(out.get_pixel(0, 0).0, [254, 253, 252, 255]);
        // Without a window (no UI), the task's own selection.
        let out = job.apply_image_effects(None).unwrap();
        assert_eq!(out.get_pixel(0, 0).0, [1, 2, 3, 255]);
    }

    #[test]
    fn names_follow_the_default_patterns() {
        let task = TaskSettings::default();
        let mut n = 0;
        let name = file_name(&task, &mut n, "bmp", &metadata(None, None));
        assert_eq!(name.len(), 10 + 4, "{name}");
        assert!(name.ends_with(".bmp"));
        let name = file_name(&task, &mut n, "bmp", &metadata(Some("ShareXrs test pattern"), Some("sharexrs")));
        assert!(name.starts_with("sharexrs_") && name.len() == "sharexrs_".len() + 10 + 4, "{name}");
        assert_eq!(change_extension(&name, "png"), name.replace(".bmp", ".png"));
    }

    #[test]
    fn screenshots_go_in_a_year_month_folder() {
        let parent = Path::new("profile").join("Screenshots");
        let app = ApplicationConfig::default();
        let folder = screenshots_folder(&TaskSettings::default(), &app, &parent, &metadata(Some("t"), Some("p")));
        assert_eq!(folder, parent.join(NameParser::new(NameParserType::FilePath).parse("%y-%mo")));
    }

    #[test]
    fn small_images_are_rgba_png_and_big_noisy_ones_jpeg() {
        let task = TaskSettings::default();
        let (png, ext) = prepare_image(&RgbaImage::from_pixel(10, 10, image::Rgba([9, 9, 9, 255])), &task).unwrap();
        assert_eq!(ext, "png");
        assert_eq!(image::load_from_memory(&png).unwrap().color(), image::ColorType::Rgba8);
        let mut noisy = RgbaImage::new(1400, 1400);
        let mut v = 1u32;
        for p in noisy.pixels_mut() {
            v = v.wrapping_mul(1_103_515_245).wrapping_add(12345);
            *p = image::Rgba([(v >> 8) as u8, (v >> 16) as u8, (v >> 24) as u8, 255]);
        }
        assert_eq!(prepare_image(&noisy, &task).unwrap().1, "jpg");
    }
}
