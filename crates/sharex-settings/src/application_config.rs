//! `ShareX/ApplicationConfig.cs` (`ApplicationConfig.json`), with
//! `ApplicationThemeOptions` (ShareX.Avalonia/Theming) and `QuickTaskInfo`.

use serde_json::Value;

use crate::defaults;
use crate::enums::*;
use crate::settings;
use crate::task_settings::TaskSettings;
use crate::types::{DateTime, Point, Rectangle, Size};

settings! {
    /// `ShareX.ApplicationConfig`: fields first, then the properties (the
    /// "Advanced" ones with `[DefaultValue]`s), then `SettingsBase`'s
    /// `ApplicationVersion`, which is the order Newtonsoft writes them in.
    pub struct ApplicationConfig {
        DefaultTaskSettings: TaskSettings = TaskSettings::default(),
        FirstTimeRunDate: DateTime = DateTime::now(),
        ShowStartScreen: bool = true,
        ShowIconPackPrompt: bool = true,
        FileUploadDefaultDirectory: String = String::new(),
        NameParserAutoIncrementNumber: i32 = 0,
        QuickTaskPresets: Vec<QuickTaskInfo> = QuickTaskInfo::default_presets(),

        // Main window
        FirstTimeMinimizeToTray: bool = true,
        ScreenshotDelayJustOnce: bool = false,

        // General
        Language: SupportedLanguage = SupportedLanguage::Automatic,
        ShowTray: bool = true,
        SilentRun: bool = false,
        TrayIconProgressEnabled: bool = true,
        TaskbarProgressEnabled: bool = true,
        UseWhiteShareXIcon: bool = false,
        RememberMainFormPosition: bool = false,
        MainFormPosition: Point = Point::default(),
        RememberMainFormSize: bool = false,
        MainFormSize: Size = Size::default(),
        TrayLeftClickAction: HotkeyType = HotkeyType::RectangleRegion,
        TrayLeftDoubleClickAction: HotkeyType = HotkeyType::OpenMainWindow,
        TrayMiddleClickAction: HotkeyType = HotkeyType::ClipboardUploadWithContentViewer,
        AutoCheckUpdate: bool = true,
        UpdateChannel: UpdateChannel = UpdateChannel::Release,
        /// Kept for backward compatibility (see `ApplicationConfigBackwardCompatibilityTasks`).
        CheckPreReleaseUpdates: bool = false,

        // Theme
        ThemeOptions: Option<ApplicationThemeOptions> = Some(ApplicationThemeOptions::default()),

        // Paths
        UseCustomScreenshotsPath: bool = false,
        CustomScreenshotsPath: String = String::new(),
        SaveImageSubFolderPattern: String = "%y-%mo".into(),
        SaveImageSubFolderPatternWindow: String = String::new(),

        // Settings
        AutoCleanupBackupFiles: bool = false,
        AutoCleanupLogFiles: bool = false,
        CleanupKeepFileCount: i32 = 10,

        // Main window
        TaskViewMode: TaskViewMode = TaskViewMode::ThumbnailView,
        ShowColumns: bool = true,
        TaskListViewColumnWidths: Vec<i32> = Vec::new(),
        ImagePreview: ImagePreviewVisibility = ImagePreviewVisibility::Automatic,
        ImagePreviewLocation: ImagePreviewLocation = ImagePreviewLocation::Side,
        ImagePreviewWidth: i32 = 300,
        ImagePreviewHeight: i32 = 250,
        ShowThumbnailTitle: bool = true,
        ThumbnailTitleLocation: ThumbnailTitleLocation = ThumbnailTitleLocation::Top,
        ThumbnailSize: Size = Size::new(200, 150),
        ThumbnailClickAction: ThumbnailViewClickAction = ThumbnailViewClickAction::Default,
        HiddenTools: Vec<String> = Vec::new(),

        // Clipboard formats: `List<ClipboardFormat>` (not ported yet).
        ClipboardContentFormats: Vec<Value> = Vec::new(),

        // Upload
        UploadLimit: i32 = 0,
        BufferSizePower: i32 = 5,
        MaxUploadFailRetry: i32 = 1,

        // History
        HistorySaveTasks: bool = true,
        HistoryCheckURL: bool = false,
        /// Reads each captured image's text in the background for history search.
        HistorySaveOCRText: bool = false,
        /// `RecentTask[]` (not ported yet).
        RecentTasks: Option<Vec<Value>> = None,
        RecentTasksSave: bool = false,
        RecentTasksMaxCount: i32 = 10,
        RecentTasksShowInMainWindow: bool = true,
        RecentTasksShowInTrayMenu: bool = true,
        RecentTasksTrayMenuMostRecentFirst: bool = false,
        /// `ShareX.HistoryLib.HistorySettings` (not ported yet).
        HistorySettings: Value = defaults::json("HistorySettings"),
        /// `ShareX.HistoryLib.ImageHistorySettings` (not ported yet).
        ImageHistorySettings: Value = defaults::json("ImageHistorySettings"),

        // Print
        DontShowPrintSettingsDialog: bool = false,
        /// `ShareX.HelpersLib.PrintSettings` (not ported yet).
        PrintSettings: Value = defaults::json("PrintSettings"),

        // Proxy: `ShareX.HelpersLib.ProxyInfo` (not ported yet).
        ProxySettings: Value = defaults::json("ProxyInfo"),

        // AutoCapture
        AutoCaptureRegion: Rectangle = Rectangle::default(),
        /// `decimal`.
        AutoCaptureRepeatTime: f64 = 60.0,
        AutoCaptureMinimizeToTray: bool = true,
        AutoCaptureWaitUpload: bool = true,

        // ScreenRecord
        ScreenRecordRegion: Rectangle = Rectangle::default(),

        // Actions toolbar
        ActionsToolbarList: Vec<HotkeyType> = vec![
            HotkeyType::RectangleRegion,
            HotkeyType::PrintScreen,
            HotkeyType::ScreenRecorder,
            HotkeyType::None,
            HotkeyType::FileUpload,
            HotkeyType::ClipboardUploadWithContentViewer,
        ],
        ActionsToolbarRunAtStartup: bool = false,
        ActionsToolbarPosition: Point = Point::default(),
        ActionsToolbarLockPosition: bool = false,
        ActionsToolbarStayTopMost: bool = true,

        // Color picker: `List<System.Drawing.Color>`, as their converter writes them.
        RecentColors: Vec<String> = Vec::new(),

        // Advanced (properties).
        /// Calculate and show file sizes in binary units (KiB, MiB etc.).
        BinaryUnits: bool = false,
        /// Show most recent task first in main window.
        ShowMostRecentTaskFirst: bool = false,
        /// Show only customized tasks in main window workflows.
        WorkflowsOnlyShowEdited: bool = false,
        /// Automatically expand capture menu when you open the tray menu.
        TrayAutoExpandCaptureMenu: bool = false,
        /// URLs will open using this path instead of default browser.
        BrowserPath: String = String::new(),
        /// Save settings after task completed but only if there are no other active tasks.
        SaveSettingsAfterTaskCompleted: bool = false,
        DevMode: bool = false,
        /// Disables hotkeys.
        DisableHotkeys: bool = false,
        /// If active window is fullscreen then hotkeys won't be executed.
        DisableHotkeysOnFullscreen: bool = false,
        /// If you hold hotkeys then it will only trigger every this milliseconds
        /// (at least 200: see [`ApplicationConfig::hotkey_repeat_limit`]).
        HotkeyRepeatLimit: i32 = 500,
        /// Show clipboard content viewer when using clipboard upload in main window.
        ShowClipboardContentViewer: bool = true,
        DefaultClipboardCopyImageFillBackground: bool = true,
        UseAlternativeClipboardCopyImage: bool = false,
        UseAlternativeClipboardGetImage: bool = false,
        /// If JPEG exif contains orientation data then rotate image accordingly.
        RotateImageByExifOrientationData: bool = true,
        /// Strip color space information chunks from PNG image.
        PNGStripColorSpaceInformation: bool = false,
        /// Ask for confirmation before deleting a file locally.
        ShowFileDeleteConfirmation: bool = true,
        /// Can be used to disable uploading application wide.
        DisableUpload: bool = false,
        /// Ignore emojis while URL encoding upload results.
        URLEncodeIgnoreEmoji: bool = true,
        /// Show more than 10 files upload warning.
        ShowMultiUploadWarning: bool = true,
        /// Large file size defined in MB; 0 disables the warning.
        ShowLargeFileSizeWarning: i32 = 100,
        /// Store Uploaders configuration per machine (UploadersConfig-MYPC.json).
        UseMachineSpecificUploadersConfig: bool = false,
        CustomUploadersConfigPath: Option<String> = None,
        CustomHotkeysConfigPath: Option<String> = None,
        /// Secondary custom screenshot path, used when the first is unavailable.
        CustomScreenshotsPath2: Option<String> = None,
        /// Drag and drop window.
        DropSize: i32 = 150,
        DropOffset: i32 = 5,
        DropAlignment: ImageContentAlignment = ImageContentAlignment::BottomRight,
        DropOpacity: i32 = 100,
        DropHoverOpacity: i32 = 255,

        /// `SettingsBase.ApplicationVersion`: the version that last saved the
        /// file (null in a new one: `IsFirstTimeRun`).
        ApplicationVersion: Option<String> = None,
    }
}

impl ApplicationConfig {
    /// The `HotkeyRepeatLimit` setter's clamp (`Math.Max(value, 200)`).
    pub fn hotkey_repeat_limit(&self) -> i32 {
        self.HotkeyRepeatLimit.max(200)
    }

    /// `Settings.ThemeOptions ??= new ApplicationThemeOptions()`.
    pub fn theme_options(&mut self) -> &mut ApplicationThemeOptions {
        self.ThemeOptions.get_or_insert_with(Default::default)
    }
}

settings! {
    /// `ShareX.AvaloniaUI.Theming.ApplicationThemeOptions`.
    pub struct ApplicationThemeOptions {
        Theme: String = ApplicationThemeOptions::DEFAULT_THEME.into(),
        UseSystemTheme: bool = false,
        AccentColorHex: String = ApplicationThemeOptions::DEFAULT_ACCENT_COLOR_HEX.into(),
        UseSystemAccentColor: bool = false,
        IconPack: IconPack = IconPack::Modern,
        CompactMode: bool = false,
        /// Keeps scroll bars at full width instead of a thin line that widens on hover.
        AlwaysExpandScrollBars: bool = false,
        /// Spacing is a percentage between compact (0) and default (100), up to
        /// 200; null follows `CompactMode`.
        SidebarSpacing: Option<i32> = None,
        MenuSpacing: Option<i32> = None,
        ToolMenuSpacing: Option<i32> = None,
        SettingsSpacing: Option<i32> = None,
    }
}

impl ApplicationThemeOptions {
    pub const DEFAULT_THEME: &'static str = "Dark";
    pub const DEFAULT_ACCENT_COLOR_HEX: &'static str = "#09AAFF";
}

settings! {
    /// `ShareX.QuickTaskInfo`: a quick task menu entry.
    pub struct QuickTaskInfo {
        Name: Option<String> = None,
        AfterCaptureTasks: AfterCaptureTasks = AfterCaptureTasks::None,
        AfterUploadTasks: AfterUploadTasks = AfterUploadTasks::None,
    }
}

impl QuickTaskInfo {
    pub fn new(name: Option<&str>, after_capture: AfterCaptureTasks, after_upload: AfterUploadTasks) -> Self {
        Self {
            Name: name.map(str::to_owned),
            AfterCaptureTasks: after_capture,
            AfterUploadTasks: after_upload,
            ..Default::default()
        }
    }

    /// `QuickTaskInfo.GetDefaultPresets()`.
    pub fn default_presets() -> Vec<QuickTaskInfo> {
        use AfterCaptureTasks as C;
        use AfterUploadTasks as U;
        let q = |name, c, u| QuickTaskInfo::new(name, c, u);
        vec![
            q(Some("Save, Upload, Copy URL"), C::SaveImageToFile | C::UploadImageToHost, U::CopyURLToClipboard),
            q(Some("Save, Copy image"), C::SaveImageToFile | C::CopyImageToClipboard, U::None),
            q(Some("Save, Copy image file"), C::SaveImageToFile | C::CopyFileToClipboard, U::None),
            q(
                Some("Annotate, Save, Upload, Copy URL"),
                C::AnnotateImage | C::SaveImageToFile | C::UploadImageToHost,
                U::CopyURLToClipboard,
            ),
            q(None, C::None, U::None),
            q(Some("Upload, Copy URL"), C::UploadImageToHost, U::CopyURLToClipboard),
            q(Some("Save"), C::SaveImageToFile, U::None),
            q(Some("Copy image"), C::CopyImageToClipboard, U::None),
            q(Some("Annotate"), C::AnnotateImage, U::None),
        ]
    }

    /// `QuickTaskInfo.IsValid`.
    pub fn is_valid(&self) -> bool {
        self.AfterCaptureTasks != AfterCaptureTasks::None
    }
}
