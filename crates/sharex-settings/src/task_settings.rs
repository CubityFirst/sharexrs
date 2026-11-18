//! `ShareX/TaskSettings.cs`: what a task (a hotkey, a workflow, the
//! defaults) does, and the settings sections each task can override.

use serde_json::Value;

use crate::enums::*;
use crate::external_program::ExternalProgram;
use crate::watch_folder::WatchFolderSettings;
use crate::ffmpeg_options::FFmpegOptions;
use crate::region_capture::RegionCaptureOptions;
use crate::settings;
use crate::types::{Rectangle, Size};
use crate::defaults;

settings! {
    /// `ShareX.TaskSettings`.
    ///
    /// The sections are `Option`s because ShareX nulls the ones a hotkey
    /// takes from the defaults before saving `HotkeysConfig.json`
    /// ([`TaskSettings::cleanup`]).
    pub struct TaskSettings {
        Description: String = String::new(),
        Job: HotkeyType = HotkeyType::None,
        UseDefaultAfterCaptureJob: bool = true,
        AfterCaptureJob: AfterCaptureTasks = AfterCaptureTasks::CopyImageToClipboard | AfterCaptureTasks::SaveImageToFile,
        UseDefaultAfterUploadJob: bool = true,
        AfterUploadJob: AfterUploadTasks = AfterUploadTasks::CopyURLToClipboard,
        UseDefaultDestinations: bool = true,
        ImageDestination: ImageDestination = ImageDestination::Imgur,
        ImageFileDestination: FileDestination = FileDestination::Dropbox,
        TextDestination: TextDestination = TextDestination::Pastebin,
        TextFileDestination: FileDestination = FileDestination::Dropbox,
        FileDestination: FileDestination = FileDestination::Dropbox,
        URLShortenerDestination: UrlShortenerType = UrlShortenerType::BITLY,
        URLSharingServiceDestination: URLSharingServices = URLSharingServices::Email,
        OverrideFTP: bool = false,
        FTPIndex: i32 = 0,
        OverrideCustomUploader: bool = false,
        CustomUploaderIndex: i32 = 0,
        OverrideScreenshotsFolder: bool = false,
        ScreenshotsFolder: String = String::new(),
        UseDefaultGeneralSettings: bool = true,
        GeneralSettings: Option<TaskSettingsGeneral> = Some(Default::default()),
        UseDefaultImageSettings: bool = true,
        ImageSettings: Option<TaskSettingsImage> = Some(Default::default()),
        UseDefaultCaptureSettings: bool = true,
        CaptureSettings: Option<TaskSettingsCapture> = Some(Default::default()),
        UseDefaultUploadSettings: bool = true,
        UploadSettings: Option<TaskSettingsUpload> = Some(Default::default()),
        UseDefaultActions: bool = true,
        ExternalPrograms: Option<Vec<ExternalProgram>> = Some(Vec::new()),
        UseDefaultToolsSettings: bool = true,
        ToolsSettings: Option<TaskSettingsTools> = Some(Default::default()),
        UseDefaultAdvancedSettings: bool = true,
        AdvancedSettings: Option<TaskSettingsAdvanced> = Some(Default::default()),
        WatchFolderEnabled: bool = false,
        WatchFolderList: Vec<WatchFolderSettings> = Vec::new(),
    }
}

impl TaskSettings {
    /// `TaskSettings.IsUsingDefaultSettings`.
    pub fn is_using_default_settings(&self) -> bool {
        self.UseDefaultAfterCaptureJob
            && self.UseDefaultAfterUploadJob
            && self.UseDefaultDestinations
            && !self.OverrideFTP
            && !self.OverrideCustomUploader
            && !self.OverrideScreenshotsFolder
            && self.UseDefaultGeneralSettings
            && self.UseDefaultImageSettings
            && self.UseDefaultCaptureSettings
            && self.UseDefaultUploadSettings
            && self.UseDefaultActions
            && self.UseDefaultToolsSettings
            && self.UseDefaultAdvancedSettings
            && !self.WatchFolderEnabled
    }

    /// `TaskSettings.GetDefaultTaskSettings()`: a new task with the
    /// application's defaults copied in (`SetDefaultSettings`).
    pub fn default_task_settings(defaults: &TaskSettings) -> TaskSettings {
        let mut t = TaskSettings::default();
        t.set_default_settings(defaults);
        t
    }

    /// `TaskSettings.SetDefaultSettings()`: takes every section this task
    /// uses the defaults for from `defaults`.
    pub fn set_default_settings(&mut self, defaults: &TaskSettings) {
        let d = defaults.clone();
        if self.UseDefaultAfterCaptureJob {
            self.AfterCaptureJob = d.AfterCaptureJob;
        }
        if self.UseDefaultAfterUploadJob {
            self.AfterUploadJob = d.AfterUploadJob;
        }
        if self.UseDefaultDestinations {
            self.ImageDestination = d.ImageDestination;
            self.ImageFileDestination = d.ImageFileDestination;
            self.TextDestination = d.TextDestination;
            self.TextFileDestination = d.TextFileDestination;
            self.FileDestination = d.FileDestination;
            self.URLShortenerDestination = d.URLShortenerDestination;
            self.URLSharingServiceDestination = d.URLSharingServiceDestination;
        }
        if self.UseDefaultGeneralSettings {
            self.GeneralSettings = d.GeneralSettings;
        }
        if self.UseDefaultImageSettings {
            self.ImageSettings = d.ImageSettings;
        }
        if self.UseDefaultCaptureSettings {
            self.CaptureSettings = d.CaptureSettings;
        }
        if self.UseDefaultUploadSettings {
            self.UploadSettings = d.UploadSettings;
        }
        if self.UseDefaultActions {
            self.ExternalPrograms = d.ExternalPrograms;
        }
        if self.UseDefaultToolsSettings {
            self.ToolsSettings = d.ToolsSettings;
        }
        if self.UseDefaultAdvancedSettings {
            self.AdvancedSettings = d.AdvancedSettings;
        }
    }

    /// `TaskSettings.GetSafeTaskSettings()`: the settings a task runs with:
    /// the defaults where it uses them, its own elsewhere.
    pub fn safe_task_settings(&self, defaults: &TaskSettings) -> TaskSettings {
        if self.is_using_default_settings() {
            let mut safe = defaults.clone();
            safe.Description = self.Description.clone();
            safe.Job = self.Job.clone();
            safe
        } else {
            let mut safe = self.clone();
            safe.set_default_settings(defaults);
            safe
        }
    }

    /// `TaskSettings.Cleanup()`: drops the sections taken from the
    /// defaults, before `HotkeysConfig.json` is saved.
    pub fn cleanup(&mut self) {
        if self.UseDefaultGeneralSettings {
            self.GeneralSettings = None;
        }
        if self.UseDefaultImageSettings {
            self.ImageSettings = None;
        }
        if self.UseDefaultCaptureSettings {
            self.CaptureSettings = None;
        }
        if self.UseDefaultUploadSettings {
            self.UploadSettings = None;
        }
        if self.UseDefaultActions {
            self.ExternalPrograms = None;
        }
        if self.UseDefaultToolsSettings {
            self.ToolsSettings = None;
        }
        if self.UseDefaultAdvancedSettings {
            self.AdvancedSettings = None;
        }
    }

    /// `TaskSettings.ToString()`: the description, or the job's name.
    pub fn display_name(&self) -> String {
        if self.Description.is_empty() { self.Job.localized_description() } else { self.Description.clone() }
    }
}

settings! {
    /// `ShareX.TaskSettingsGeneral`.
    pub struct TaskSettingsGeneral {
        PlaySoundAfterCapture: bool = true,
        PlaySoundAfterUpload: bool = true,
        PlaySoundAfterAction: bool = true,
        ShowToastNotificationAfterTaskCompleted: bool = true,
        ToastWindowDuration: f32 = 3.0,
        ToastWindowFadeDuration: f32 = 1.0,
        ToastWindowPlacement: ImageContentAlignment = ImageContentAlignment::BottomRight,
        ToastWindowMonitor: ToastWindowMonitor = ToastWindowMonitor::ActiveMonitor,
        ToastWindowMonitorDeviceName: String = String::new(),
        ToastWindowSize: Size = Size::new(400, 300),
        ToastWindowLeftClickAction: ToastClickAction = ToastClickAction::OpenUrl,
        ToastWindowRightClickAction: ToastClickAction = ToastClickAction::CloseNotification,
        ToastWindowMiddleClickAction: ToastClickAction = ToastClickAction::AnnotateImage,
        ToastWindowButtonSize: i32 = 40,
        /// `List<NotificationActionButton>` (not ported yet).
        ToastWindowButtons: Value = defaults::json("NotificationActionButton.CreateDefaultButtons"),
        ToastWindowAutoHide: bool = true,
        DisableNotificationsOnFullscreen: bool = false,
        UseCustomCaptureSound: bool = false,
        CustomCaptureSoundPath: String = String::new(),
        UseCustomTaskCompletedSound: bool = false,
        CustomTaskCompletedSoundPath: String = String::new(),
        UseCustomActionCompletedSound: bool = false,
        CustomActionCompletedSoundPath: String = String::new(),
        UseCustomErrorSound: bool = false,
        CustomErrorSoundPath: String = String::new(),
    }
}

settings! {
    /// `ShareX.TaskSettingsImage`.
    pub struct TaskSettingsImage {
        ImageFormat: EImageFormat = EImageFormat::PNG,
        ImagePNGBitDepth: PNGBitDepth = PNGBitDepth::Default,
        ImageBMPBitDepth: BMPBitDepth = BMPBitDepth::Bit24,
        ImagePNGCompressionLevel: i32 = 1,
        /// SkiaSharp's `SKPngEncoderFilterFlags`.
        ImagePNGFilter: RawEnum = RawEnum::name("AllFilters"),
        ImageJPEGQuality: i32 = 90,
        /// SkiaSharp's `SKJpegEncoderDownsample`.
        ImageJPEGSubsampling: RawEnum = RawEnum::name("Downsample420"),
        ImageGIFQuality: GIFQuality = GIFQuality::Adaptive,
        ImageWebPQuality: i32 = 90,
        ImageWebPLossless: bool = true,
        ImageWebPCompressionEffort: i32 = 100,
        ImageAutoUseJPEG: bool = true,
        ImageAutoUseJPEGSize: i32 = 2048,
        ImageAutoJPEGQuality: bool = false,
        FileExistAction: FileExistAction = FileExistAction::Ask,
        /// `List<ImageEffectPreset>`, kept as ShareX wrote it; read with
        /// `sharex_image_effects::ImageEffectPreset::from_json`.
        ImageEffectPresets: Vec<Value> = vec![defaults::json("ImageEffectPreset.GetDefaultPreset")],
        SelectedImageEffectPreset: i32 = 0,
        ShowImageEffectsWindowAfterCapture: bool = false,
        ImageEffectOnlyRegionCapture: bool = false,
        UseRandomImageEffect: bool = false,
        ThumbnailWidth: i32 = 200,
        ThumbnailHeight: i32 = 0,
        ThumbnailName: String = "-thumbnail".into(),
        ThumbnailCheckSize: bool = false,
    }
}

settings! {
    /// `ShareX.TaskSettingsCapture`.
    pub struct TaskSettingsCapture {
        ShowCursor: bool = true,
        /// `decimal`.
        ScreenshotDelay: f64 = 0.0,
        CaptureTransparent: bool = false,
        CaptureShadow: bool = true,
        CaptureShadowOffset: i32 = 100,
        CaptureClientArea: bool = false,
        CaptureAutoHideTaskbar: bool = false,
        CaptureAutoHideDesktopIcons: bool = false,
        HDRScreenshotColorCorrection: bool = false,
        CaptureCustomRegion: Rectangle = Rectangle::new(0, 0, 0, 0),
        CaptureCustomWindow: String = String::new(),
        RegionCaptureOptions: RegionCaptureOptions = RegionCaptureOptions::default(),
        FFmpegOptions: FFmpegOptions = FFmpegOptions::default(),
        ScreenRecordUseNative: bool = true,
        ScreenRecordShowBar: bool = true,
        ScreenRecordSystemAudio: bool = true,
        ScreenRecordSystemAudioDeviceId: String = String::new(),
        ScreenRecordSystemAudioGain: f32 = 1.0,
        ScreenRecordMicrophone: bool = false,
        ScreenRecordMicrophoneDeviceId: String = String::new(),
        ScreenRecordMicrophoneGain: f32 = 1.0,
        ScreenRecordCamera: bool = false,
        ScreenRecordCameraDeviceId: String = String::new(),
        ScreenRecordCameraResolution: CameraCaptureResolution = CameraCaptureResolution::Size1280x720,
        ScreenRecordCameraFPS: i32 = 30,
        ScreenRecordCameraPosition: CameraOverlayPosition = CameraOverlayPosition::BottomRight,
        ScreenRecordCameraShape: CameraOverlayShape = CameraOverlayShape::Rectangle,
        ScreenRecordCameraWidthPercent: i32 = 20,
        ScreenRecordCameraMargin: i32 = 20,
        ScreenRecordRequireHardwareEncoder: bool = true,
        /// kbps.
        ScreenRecordVideoBitrate: i32 = 10000,
        ScreenRecordHDRToneMapping: bool = false,
        ScreenRecordFPS: i32 = 30,
        GIFFPS: i32 = 15,
        ScreenRecordShowCursor: bool = true,
        ScreenRecordMouseHighlighter: bool = false,
        ScreenRecordShowTimer: bool = true,
        ScreenRecordShowButtonLabels: bool = false,
        ScreenRecordStartDelayEnabled: bool = false,
        ScreenRecordStartDelay: f32 = 3.0,
        ScreenRecordFixedDuration: bool = false,
        ScreenRecordDuration: f32 = 3.0,
        ScreenRecordTwoPassEncoding: bool = false,
        ScreenRecordAskConfirmationOnAbort: bool = false,
        /// `ShareX.ScreenCaptureLib.ScrollingCaptureOptions` (not ported yet).
        ScrollingCaptureOptions: Value = defaults::json("ScrollingCaptureOptions"),
        /// `ShareX.Tools.OCROptions` (not ported yet).
        OCROptions: Value = defaults::json("OCROptions"),
    }
}

settings! {
    /// `ShareX.TaskSettingsUpload`.
    pub struct TaskSettingsUpload {
        UseCustomTimeZone: bool = false,
        /// `TimeZoneInfo`, as Newtonsoft writes `TimeZoneInfo.Utc`.
        CustomTimeZone: Value = defaults::json("TimeZoneInfo.Utc"),
        NameFormatPattern: String = "%ra{10}".into(),
        NameFormatPatternActiveWindow: String = "%pn_%ra{10}".into(),
        FileUploadUseNamePattern: bool = false,
        FileUploadReplaceProblematicCharacters: bool = false,
        URLRegexReplace: bool = false,
        URLRegexReplacePattern: String = "^https?://(.+)$".into(),
        URLRegexReplaceReplacement: String = "https://$1".into(),
        ClipboardUploadURLContents: bool = false,
        ClipboardUploadShortenURL: bool = false,
        ClipboardUploadShareURL: bool = false,
        ClipboardUploadAutoIndexFolder: bool = false,
        /// `List<UploaderFilter>` (not ported yet).
        UploaderFilters: Vec<Value> = Vec::new(),
    }
}

settings! {
    /// `ShareX.TaskSettingsTools`: each tool's options, kept as ShareX
    /// writes them until the tool is ported.
    pub struct TaskSettingsTools {
        ColorPickerOptions: Value = defaults::json("ColorPickerOptions"),
        ScreenColorPickerOptions: Value = defaults::json("ScreenColorPickerOptions"),
        PinToScreenOptions: Value = defaults::json("PinToScreenOptions"),
        MouseHighlighterOptions: Value = defaults::json("MouseHighlighterOptions"),
        IndexerSettings: Value = defaults::json("IndexerSettings"),
        ImageCombinerOptions: Value = defaults::json("ImageCombinerOptions"),
        VideoConverterOptions: Value = defaults::json("VideoConverterOptions"),
        VideoThumbnailOptions: Value = defaults::json("VideoThumbnailOptions"),
        BorderlessWindowSettings: Value = defaults::json("BorderlessWindowSettings"),
        AIOptions: Value = defaults::json("AIOptions"),
        ImageEditorOptions: Value = defaults::json("ImageEditorOptions"),
        BackgroundRemoverOptions: Value = defaults::json("BackgroundRemoverOptions"),
    }
}

/// `FileHelpers.ImageFileExtensions`.
pub const IMAGE_FILE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "gif", "bmp", "ico", "webp"];

/// `FileHelpers.TextFileExtensions`.
pub const TEXT_FILE_EXTENSIONS: &[&str] = &[
    "txt", "log", "nfo", "c", "cpp", "cc", "cxx", "h", "hpp", "hxx", "cs", "vb", "html", "htm", "xhtml", "xht", "xml",
    "css", "js", "php", "bat", "java", "lua", "py", "pl", "cfg", "ini", "dart", "go", "gohtml",
];

settings! {
    /// `ShareX.TaskSettingsAdvanced` (properties with `[DefaultValue]`s,
    /// applied by `ApplyDefaultPropertyValues`).
    pub struct TaskSettingsAdvanced {
        ProcessImagesDuringFileUpload: bool = false,
        ProcessImagesDuringClipboardUpload: bool = false,
        ProcessImagesDuringExtensionUpload: bool = false,
        UseAfterCaptureTasksDuringFileUpload: bool = true,
        TextTaskSaveAsFile: bool = true,
        AutoClearClipboard: bool = false,
        RegionCaptureDisableAnnotation: bool = false,
        ImageExtensions: Vec<String> = IMAGE_FILE_EXTENSIONS.iter().map(|s| s.to_string()).collect(),
        TextExtensions: Vec<String> = TEXT_FILE_EXTENSIONS.iter().map(|s| s.to_string()).collect(),
        EarlyCopyURL: bool = false,
        TextFileExtension: String = "txt".into(),
        TextFormat: String = "text".into(),
        TextCustom: String = String::new(),
        TextCustomEncodeInput: bool = true,
        ResultForceHTTPS: bool = false,
        ClipboardContentFormat: String = "$result".into(),
        BalloonTipContentFormat: String = "$result".into(),
        OpenURLFormat: String = "$result".into(),
        AutoShortenURLLength: i32 = 0,
        AutoCloseAfterUploadForm: bool = false,
        NamePatternMaxLength: i32 = 100,
        NamePatternMaxTitleLength: i32 = 50,
    }
}
