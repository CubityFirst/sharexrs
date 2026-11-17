//! The settings the capture jobs read, with a fresh profile's defaults
//! (`ShareX/TaskSettings.cs`, `ShareX/ApplicationConfig.cs`,
//! `ShareX/Infrastructure/AppPaths.cs`), under ShareX's names.
//!
//! A stand-in until ShareX's settings files are ported (`ApplicationConfig
//! .json`'s `DefaultTaskSettings`): that work replaces this struct with the
//! real, loaded settings; the capture code only reads these fields.

use std::path::PathBuf;

use sharex_capture::RegionCaptureOptions;

/// `AfterCaptureTasks` flags (ShareX's values) of the tasks ported so far.
pub mod after_capture {
    pub const COPY_IMAGE_TO_CLIPBOARD: u32 = 1 << 5;
    pub const SAVE_IMAGE_TO_FILE: u32 = 1 << 8;
}

/// `FileExistAction`.
#[allow(dead_code)] // Chosen in task settings, which aren't ported yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileExistAction {
    Ask,
    Overwrite,
    UniqueName,
    Cancel,
}

#[derive(Clone, Debug)]
pub struct TaskSettings {
    /// `AfterCaptureJob`: copy to clipboard and save to file.
    pub after_capture_job: u32,
    /// `CaptureSettings.ShowCursor`.
    pub show_cursor: bool,
    /// `CaptureSettings.ScreenshotDelay`, seconds.
    pub screenshot_delay: f64,
    /// `CaptureSettings.RegionCaptureOptions`.
    pub region_capture_options: RegionCaptureOptions,
    /// `AdvancedSettings.RegionCaptureDisableAnnotation`.
    pub region_capture_disable_annotation: bool,
    /// `GeneralSettings.PlaySoundAfterCapture`.
    pub play_sound_after_capture: bool,
    /// `UploadSettings.NameFormatPattern`.
    pub name_format_pattern: String,
    /// `UploadSettings.NameFormatPatternActiveWindow`.
    pub name_format_pattern_active_window: String,
    /// `AdvancedSettings.NamePatternMaxLength` / `MaxTitleLength`.
    pub name_pattern_max_length: usize,
    pub name_pattern_max_title_length: usize,
    /// `ImageSettings.ImageAutoUseJPEG` and its size in KB.
    pub image_auto_use_jpeg: bool,
    pub image_auto_use_jpeg_size: u64,
    /// `ImageSettings.ImageJPEGQuality`.
    pub image_jpeg_quality: u8,
    /// `ImageSettings.FileExistAction`.
    pub file_exist_action: FileExistAction,
    /// `ApplicationConfig.SaveImageSubFolderPattern` and `...Window`.
    pub save_image_sub_folder_pattern: String,
    pub save_image_sub_folder_pattern_window: String,
    /// `ApplicationConfig.NameParserAutoIncrementNumber`.
    pub name_parser_auto_increment_number: i64,
}

impl Default for TaskSettings {
    fn default() -> Self {
        use after_capture::*;
        Self {
            after_capture_job: COPY_IMAGE_TO_CLIPBOARD | SAVE_IMAGE_TO_FILE,
            show_cursor: true,
            screenshot_delay: 0.0,
            region_capture_options: RegionCaptureOptions::default(),
            region_capture_disable_annotation: false,
            play_sound_after_capture: true,
            name_format_pattern: "%ra{10}".into(),
            name_format_pattern_active_window: "%pn_%ra{10}".into(),
            name_pattern_max_length: 100,
            name_pattern_max_title_length: 50,
            image_auto_use_jpeg: true,
            image_auto_use_jpeg_size: 2048,
            image_jpeg_quality: 90,
            file_exist_action: FileExistAction::Ask,
            save_image_sub_folder_pattern: "%y-%mo".into(),
            save_image_sub_folder_pattern_window: String::new(),
            name_parser_auto_increment_number: 0,
        }
    }
}

/// `AppPaths.PersonalFolder`: `ShareX` beside the executable when a
/// `Portable` file is there (ShareX's portable mode), else
/// `Documents/ShareX`.
pub fn personal_folder() -> PathBuf {
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
        && dir.join("Portable").exists()
    {
        return dir.join("ShareX");
    }
    dirs::document_dir().unwrap_or_else(|| PathBuf::from(".")).join("ShareX")
}

/// `AppPaths.ScreenshotsParentFolder` (without a custom screenshots path).
pub fn screenshots_parent_folder() -> PathBuf {
    personal_folder().join("Screenshots")
}
