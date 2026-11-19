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
    parser.custom_time_zone = if upload.UseCustomTimeZone { sharex_helpers::time_zone::from_settings(&upload.CustomTimeZone) } else { None };
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

/// `TaskHelpers.PrepareImage`: the image in the task's format (PNG, JPEG,
/// WebP, GIF or BMP with its options), or a JPEG on white over
/// `ImageAutoUseJPEGSize` KB (`ImageAutoUseJPEG`); the extension with it.
/// `strip_png_color_space`: `PNGStripColorSpaceInformation`.
pub fn prepare_image(image: &RgbaImage, task: &TaskSettings, strip_png_color_space: bool) -> Result<(Vec<u8>, &'static str), String> {
    let settings = task.ImageSettings.clone().unwrap_or_default();
    let bmp = sharex_tools::imaging::Bitmap::from_straight(image.width(), image.height(), image.as_raw());
    let d = sharex_tools::capture_image::prepare_image(&bmp, &settings, strip_png_color_space);
    if d.data.is_empty() {
        // `SaveImageAsStream` catches the encoder's failure and shows it.
        return Err(format!("Image encoding failed: {:?}.", d.format));
    }
    Ok((d.data, sharex_tools::capture_image::extension(&d.format)))
}

/// What asks the user about a file that exists (`FileExistWindowIntegration.Show`):
/// the path to write, or `None` when cancelled. The app installs one that
/// opens the file exists window and waits for it.
pub type FileExistsAsker = std::sync::Arc<dyn Fn(&Path) -> Option<PathBuf> + Send + Sync>;

static FILE_EXISTS_ASKER: std::sync::Mutex<Option<(FileExistsAsker, std::thread::ThreadId)>> = std::sync::Mutex::new(None);

/// Installs the asker; called on the event loop's thread, which is never
/// made to wait for it.
pub fn set_file_exists_asker(asker: Option<FileExistsAsker>) {
    let ui_thread = std::thread::current().id();
    *FILE_EXISTS_ASKER.lock().expect("file exists asker") = asker.map(|a| (a, ui_thread));
}

/// `TaskHelpers.HandleExistsFile`: the path to write `path` to, or `None`
/// (an empty path in ShareX) when the task shouldn't write it. Asking (the
/// default) opens the file exists window and waits; without one (tests,
/// or on the event loop's own thread) a unique name is used.
pub fn handle_exists_file(path: &Path, task: &TaskSettings) -> Option<PathBuf> {
    if !path.is_file() {
        return Some(path.to_path_buf());
    }
    match task.ImageSettings.as_ref().map(|s| s.FileExistAction.clone()).unwrap_or(FileExistAction::Ask) {
        // No case of ShareX's switch: the path as it is.
        FileExistAction::Overwrite | FileExistAction::Unknown(_) => Some(path.to_path_buf()),
        FileExistAction::Cancel => None,
        FileExistAction::UniqueName => Some(unique_file_path(path)),
        FileExistAction::Ask => {
            let asker = FILE_EXISTS_ASKER.lock().expect("file exists asker").clone();
            match asker {
                Some((ask, ui_thread)) if ui_thread != std::thread::current().id() => ask(path),
                _ => Some(unique_file_path(path)),
            }
        }
    }
}

/// `HelpersOptions.LastSaveDirectory`: where the last Save as... went.
static LAST_SAVE_DIRECTORY: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

/// `SkiaImageHelpers.TryGetImageFormat`: the format a file's extension
/// names.
pub fn try_get_image_format(path: &Path) -> Option<sharex_settings::EImageFormat> {
    use sharex_settings::EImageFormat;
    let ext = path.extension()?.to_string_lossy().to_lowercase();
    Some(match ext.as_str() {
        "png" => EImageFormat::PNG,
        "jpg" | "jpeg" | "jpe" | "jfif" => EImageFormat::JPEG,
        "webp" => EImageFormat::WEBP,
        "gif" => EImageFormat::GIF,
        "bmp" => EImageFormat::BMP,
        _ => return None,
    })
}

/// `TaskHelpers.CreateThumbnail`: with a thumbnail width or height (and,
/// with `ThumbnailCheckSize`, an image bigger than both), the image
/// resized (`Resize(width, height)`, a 0 keeping the aspect ratio) on white
/// as a JPEG at quality 90, named `<name><ThumbnailName>.jpg` in `folder`
/// (`HandleExistsFile`); its path, or `None`.
pub fn create_thumbnail(image: &RgbaImage, folder: &Path, file_name: &str, task: &TaskSettings) -> Option<PathBuf> {
    let s = task.ImageSettings.clone().unwrap_or_default();
    let (w, h) = (s.ThumbnailWidth, s.ThumbnailHeight);
    let big = image.width() as i64 > i64::from(w) && image.height() as i64 > i64::from(h);
    if !((w > 0 || h > 0) && (!s.ThumbnailCheckSize || big)) {
        return None;
    }
    let stem = Path::new(file_name).file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let path = handle_exists_file(&folder.join(format!("{stem}{}.jpg", s.ThumbnailName)), task)?;
    let bmp = sharex_tools::imaging::Bitmap::from_straight(image.width(), image.height(), image.as_raw());
    // `new Resize(width, height).Apply(...)` (`ResizeAll`).
    let (rw, rh) = sharex_image_effects::ops::apply_aspect_ratio(w, h, bmp.width(), bmp.height());
    let resized = sharex_image_effects::ops::resize_image(bmp, rw, rh);
    let filled = sharex_tools::imaging::fill_background(&resized, sharex_tools::imaging::WHITE);
    // `Save(path, SKEncodedImageFormat.Jpeg, 90)`: Skia's JPEG encoder's
    // defaults (4:2:0).
    let jpeg = sharex_tools::imaging::save_jpeg(&filled, 90, sharex_tools::imaging::JpegDownsample::Downsample420);
    if let Err(e) = std::fs::create_dir_all(folder).and_then(|()| std::fs::write(&path, jpeg)) {
        eprintln!("Thumbnail {}: {e}", path.display());
        return None;
    }
    Some(path)
}

/// What the after capture tasks did.
#[derive(Debug, Default)]
pub struct TaskOutcome {
    pub file_path: Option<PathBuf>,
    /// `Info.ThumbnailFilePath` (Save thumbnail image to file).
    pub thumbnail_file_path: Option<PathBuf>,
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
    /// `ApplicationConfig.PNGStripColorSpaceInformation`.
    pub png_strip_color_space: bool,
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
        let png_strip_color_space = app.PNGStripColorSpaceInformation;
        Self { metadata, task, file_name, folder, image_effects_folder, png_strip_color_space }
    }

    /// `WorkerTask.DoAfterCaptureJobs`: beautify, add image effects,
    /// annotate, copy the image to the clipboard (owned by `owner`), pin,
    /// print, then encode it and save it (to the folder, with the dialog,
    /// and its thumbnail). (Open in Photo editor is deferred.)
    pub fn run(mut self, owner: Option<isize>, ui: Option<&dyn crate::upload::TaskUi>) -> Result<TaskOutcome, String> {
        let mut outcome = TaskOutcome::default();
        let job = self.task.AfterCaptureJob;
        // `TaskHelpers.BeautifyImage(Image, Info.TaskSettings)`: cancelled,
        // the task stops.
        if job.contains(AfterCaptureTasks::BeautifyImage)
            && let Some(ui) = ui
        {
            match ui.beautify_image(&self.metadata.image, &self.task) {
                Some(image) => self.metadata.image = image,
                None => {
                    eprintln!("Beautify image: cancelled, the task stops.");
                    return Ok(outcome);
                }
            }
        }
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
        if job.contains(AfterCaptureTasks::SendImageToPrinter)
            && let Some(ui) = ui
        {
            ui.print_image(&self.metadata.image);
        }
        let prepare = [
            AfterCaptureTasks::SaveImageToFile,
            AfterCaptureTasks::SaveImageToFileWithDialog,
            AfterCaptureTasks::DoOCR,
            AfterCaptureTasks::UploadImageToHost,
            AfterCaptureTasks::AnalyzeImage,
        ]
        .iter()
        .any(|t| job.contains(*t));
        if prepare {
            let (mut data, ext) = prepare_image(&self.metadata.image, &self.task, self.png_strip_color_space)?;
            // `Info.FileName`; setting `Info.FilePath` sets it to the path's name.
            let mut name = change_extension(&self.file_name, ext);
            let file_name_of = |p: &Path| p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            if (job.contains(AfterCaptureTasks::SaveImageToFile) || job.contains(AfterCaptureTasks::AnalyzeImage))
                && let Some(path) = handle_exists_file(&self.folder.join(&name), &self.task)
            {
                write_image_data(&path, &data, ui);
                eprintln!("Image saved to file: {}", path.display());
                name = file_name_of(&path);
                outcome.file_path = Some(path);
            }
            if job.contains(AfterCaptureTasks::SaveImageToFileWithDialog)
                && let Some(ui) = ui
            {
                let last = LAST_SAVE_DIRECTORY.lock().expect("last save directory").clone();
                let initial = if !last.is_empty() && Path::new(&last).is_dir() { PathBuf::from(last) } else { self.folder.clone() };
                let title = format!("{} {}", sharex_strings::sharex::UploadTask_DoAfterCaptureJobs_Choose_a_folder_to_save(), name);
                let mut format = try_get_image_format(Path::new(&name));
                loop {
                    let current_ext = Path::new(&name).extension().map(|e| e.to_string_lossy().into_owned()).unwrap_or_default();
                    let Some(selected) = ui.save_image_file_dialog(&title, &name, &initial, &current_ext) else { break };
                    // Another format picked: the image in that format, which
                    // later steps (uploading) use too.
                    if let Some(f) = try_get_image_format(&selected)
                        && Some(f.clone()) != format
                    {
                        let settings = self.task.ImageSettings.clone().unwrap_or_default();
                        let image = &self.metadata.image;
                        let bmp = sharex_tools::imaging::Bitmap::from_straight(image.width(), image.height(), image.as_raw());
                        data = sharex_tools::capture_image::save_image_as_stream(&bmp, &f, &settings, self.png_strip_color_space);
                        format = Some(f);
                    }
                    if let Some(dir) = selected.parent() {
                        *LAST_SAVE_DIRECTORY.lock().expect("last save directory") = dir.to_string_lossy().into_owned();
                    }
                    name = file_name_of(&selected);
                    outcome.file_path = Some(selected.clone());
                    if write_image_data(&selected, &data, Some(ui)) {
                        eprintln!("Image saved to file with dialog: {}", selected.display());
                        break;
                    }
                }
            }
            if job.contains(AfterCaptureTasks::SaveThumbnailImageToFile) {
                let (thumb_name, thumb_folder) = match &outcome.file_path {
                    Some(p) => (file_name_of(p), p.parent().map(Path::to_path_buf).unwrap_or_default()),
                    None => (name.clone(), self.folder.clone()),
                };
                outcome.thumbnail_file_path = create_thumbnail(&self.metadata.image, &thumb_folder, &thumb_name, &self.task);
                if let Some(p) = &outcome.thumbnail_file_path {
                    eprintln!("Thumbnail saved to file: {}", p.display());
                }
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

/// `ImageData.Write(filePath)`: the folder made (`WriteToFile`'s
/// `CreateDirectoryFromFilePath`), then the image; a failure is
/// `MessageBox.Show(ImageData_Write_Error_Message + "\r\n\"path\"",
/// "ShareX - Error", OK, Error)`, with the antivirus hint when access was
/// blocked (`FileHelpers.IsAccessBlocked`: `UnauthorizedAccessException` or
/// `FileNotFoundException`), and the task goes on.
pub fn write_image_data(path: &Path, data: &[u8], ui: Option<&dyn crate::upload::TaskUi>) -> bool {
    use sharex_strings::sharex as s;
    if data.is_empty() || path.as_os_str().is_empty() {
        return false;
    }
    if let Some(parent) = path.parent() {
        crate::shell::create_directory(parent, ui);
    }
    match std::fs::write(path, data) {
        Ok(()) => true,
        Err(e) => {
            eprintln!("{}: {e}", path.display());
            let mut text = format!("{}\r\n\"{}\"", s::ImageData_Write_Error_Message(), path.display());
            if matches!(e.kind(), std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::NotFound) {
                text.push_str("\r\n\r\n");
                text.push_str(s::YourAntiVirusSoftwareOrTheControlledFolderAccessFeatureInWindowsCouldBeBlockingShareX());
            }
            let caption = format!("ShareXrs - {}", s::Error());
            let message = crate::message_box::MessageBox::ok(caption, text, crate::message_box::MessageBoxIcon::Error);
            match ui {
                Some(ui) => {
                    eprintln!("{}", message.text);
                    ui.message_box(message);
                }
                None => crate::message_box::show(message),
            }
            false
        }
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
            png_strip_color_space: false,
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
        let job = ImageTask { metadata: metadata(None, None), task, file_name: String::new(), folder: PathBuf::new(), image_effects_folder: PathBuf::new(), png_strip_color_space: false };
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
        let (png, ext) = prepare_image(&RgbaImage::from_pixel(10, 10, image::Rgba([9, 9, 9, 255])), &task, false).unwrap();
        assert_eq!(ext, "png");
        assert_eq!(image::load_from_memory(&png).unwrap().color(), image::ColorType::Rgba8);
        let mut noisy = RgbaImage::new(1400, 1400);
        let mut v = 1u32;
        for p in noisy.pixels_mut() {
            v = v.wrapping_mul(1_103_515_245).wrapping_add(12345);
            *p = image::Rgba([(v >> 8) as u8, (v >> 16) as u8, (v >> 24) as u8, 255]);
        }
        assert_eq!(prepare_image(&noisy, &task, false).unwrap().1, "jpg");
    }
}
