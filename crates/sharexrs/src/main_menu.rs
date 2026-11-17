//! The main window's menus: `ShareX/Presentation/MainWindow/MainMenuBuilder.cs`.
//!
//! Every sidebar entry with a chevron opens one of these. The actions
//! behind the entries aren't ported yet: each entry carries a
//! [`MenuCommand`] for the app to carry out once they are.

use sharex_strings::{helpers_lib as hs, sharex as s};
use sharex_ui::keys::app_icons as icon;
use sharex_ui::menu::{MenuCategory, MenuEntry, Toggle};

/// What a menu entry does (`MainMenuBuilder`'s lambdas, by name).
#[derive(Clone, Debug, PartialEq)]
pub enum MenuCommand {
    // Capture
    CaptureFullscreen,
    /// Capture › Window › a window (by its title until windows are listed).
    CaptureWindow(String),
    /// Capture › Monitor › one monitor's bounds (x, y, width, height).
    CaptureMonitor(i32, i32, u32, u32),
    CaptureRegion,
    CaptureRegionTransparent,
    CaptureLastRegion,
    ScreenRecording,
    ScreenRecordingGif,
    ScrollingCapture,
    AutoCapture,
    ToggleShowCursor,
    SetScreenshotDelay(u32),
    ToggleScreenshotDelayJustOnce,
    // Upload
    UploadFile,
    UploadFolder,
    UploadClipboard,
    UploadText,
    UploadUrl,
    DragAndDropUpload,
    ShortenUrl,
    // Workflows
    RunWorkflow(usize),
    /// `MainFormCommand.HotkeySettings`.
    HotkeySettings,
    // Tools (`BuildToolCategories`, by `nameof(Strings.MainMenuBuilder_*)`)
    Tool(&'static str),
    // After capture / after upload tasks, by enum name
    ToggleAfterCaptureTask(&'static str),
    SelectImageEffectPreset(usize),
    ToggleAfterUploadTask(&'static str),
    // Destinations, by enum name
    SetImageDestination(&'static str),
    SetImageFileDestination(&'static str),
    SetTextDestination(&'static str),
    SetTextFileDestination(&'static str),
    SetFileDestination(&'static str),
    SetUrlShortener(&'static str),
    SetUrlSharingService(&'static str),
    // Debug (`MainFormCommand`)
    DebugLog,
    DisplayDiagnostics,
    TestImageUpload,
    TestTextUpload,
    TestFileUpload,
    TestUrlShortener,
    TestUrlSharing,
}

use MenuCommand as C;
type Entry = MenuEntry<MenuCommand>;

/// A hotkey's workflow, as the Workflows menu lists it (`HotkeySettings`).
#[derive(Clone, Debug)]
pub struct Workflow {
    /// `TaskSettings.ToString()`: its description, or its job's.
    pub name: String,
    /// `HotkeyInfo.ToString()`, when it's a valid hotkey.
    pub hotkey: Option<String>,
    /// `TaskHelpers.FindMenuIcon(job)`.
    pub icon: &'static str,
    /// `TaskSettings.IsUsingDefaultSettings`.
    pub uses_default_settings: bool,
}

/// `HotkeyManager.GetDefaultHotkeyList`, until hotkeys are ported.
pub fn default_workflows() -> Vec<Workflow> {
    let w = |name: &str, hotkey: &str, icon| Workflow {
        name: name.into(),
        hotkey: Some(hotkey.into()),
        icon,
        uses_default_settings: true,
    };
    vec![
        w(hs::HotkeyType_RectangleRegion(), "Ctrl + Print Screen", icon::Region),
        w(hs::HotkeyType_PrintScreen(), "Print Screen", icon::Fullscreen),
        w(hs::HotkeyType_ActiveWindow(), "Alt + Print Screen", icon::Window),
        w(hs::HotkeyType_ScreenRecorder(), "Shift + Print Screen", icon::ScreenRecording),
        w(hs::HotkeyType_ScreenRecorderGIF(), "Ctrl + Shift + Print Screen", icon::Film),
    ]
}

/// The settings the menus show, until `ApplicationConfig` and
/// `TaskSettings` are ported: ShareX's defaults for a fresh profile.
#[derive(Clone, Debug)]
pub struct MenuState {
    pub uploads_enabled: bool,
    pub show_cursor: bool,
    pub screenshot_delay: f32,
    pub screenshot_delay_just_once: bool,
    /// `AfterCaptureTasks` flags, by enum name.
    pub after_capture: Vec<&'static str>,
    pub after_upload: Vec<&'static str>,
    pub image_effect_presets: Vec<String>,
    pub selected_image_effect_preset: usize,
    pub image_destination: &'static str,
    pub image_file_destination: &'static str,
    pub text_destination: &'static str,
    pub text_file_destination: &'static str,
    pub file_destination: &'static str,
    pub url_shortener: &'static str,
    pub url_sharing_service: &'static str,
    pub workflows: Vec<Workflow>,
    /// `ApplicationConfig.WorkflowsOnlyShowEdited`.
    pub workflows_only_show_edited: bool,
    /// `DesktopScreen.AllScreens`' bounds, in physical pixels.
    pub monitors: Vec<(i32, i32, u32, u32)>,
    /// Visible windows' titles for Capture › Window. Listing windows isn't
    /// ported yet (`WindowsList.GetVisibleWindowsList`), so this stays empty
    /// and the menu shows "No windows found".
    pub windows: Vec<String>,
}

impl Default for MenuState {
    fn default() -> Self {
        Self {
            uploads_enabled: true,
            show_cursor: true,
            screenshot_delay: 0.0,
            screenshot_delay_just_once: false,
            // `TaskSettings`' field initialisers.
            after_capture: vec!["CopyImageToClipboard", "SaveImageToFile"],
            after_upload: vec!["CopyURLToClipboard"],
            // `ImageEffectPreset.GetDefaultPreset()`, unnamed, shows as "Name".
            image_effect_presets: vec!["Name".into()],
            selected_image_effect_preset: 0,
            image_destination: "Imgur",
            image_file_destination: "Dropbox",
            text_destination: "Pastebin",
            text_file_destination: "Dropbox",
            file_destination: "Dropbox",
            url_shortener: "BITLY",
            url_sharing_service: "Email",
            workflows: default_workflows(),
            workflows_only_show_edited: false,
            monitors: Vec::new(),
            windows: Vec::new(),
        }
    }
}

fn item(header: &str, icon: &'static str, command: MenuCommand) -> Entry {
    Entry::item(header, icon, command)
}

fn parent(header: &str, icon: &'static str, children: Vec<Entry>) -> Entry {
    Entry::parent(header, icon, children)
}

/// `string.Format(Strings.ScreenshotDelay0S, …)`.
fn screenshot_delay(delay: &str) -> String {
    s::ScreenshotDelay0S().replace("{0}", delay)
}

/// `decimal.ToString("0.#")`.
fn format_delay(delay: f32) -> String {
    let rounded = (delay * 10.0).round() / 10.0;
    if rounded.fract() == 0.0 { format!("{}", rounded as i64) } else { format!("{rounded:.1}") }
}

/// `BuildCaptureMenu`.
pub fn capture_menu(state: &MenuState) -> Vec<Entry> {
    vec![
        item(s::MainMenuBuilder_Fullscreen(), icon::Fullscreen, C::CaptureFullscreen),
        parent(s::MainMenuBuilder_Window(), icon::Window, window_menu(state)),
        parent(s::MainMenuBuilder_Monitor(), icon::Monitor, monitor_menu(state)),
        item(s::MainMenuBuilder_Region(), icon::Region, C::CaptureRegion),
        item(s::MainMenuBuilder_RegionTransparent(), icon::RegionTransparent, C::CaptureRegionTransparent),
        item(s::MainMenuBuilder_LastRegion(), icon::LastRegion, C::CaptureLastRegion),
        item(s::MainMenuBuilder_ScreenRecording(), icon::ScreenRecording, C::ScreenRecording),
        item(s::MainMenuBuilder_ScreenRecordingGif(), icon::Film, C::ScreenRecordingGif),
        item(s::MainMenuBuilder_ScrollingCapture(), icon::ScrollingCapture, C::ScrollingCapture),
        item(s::MainMenuBuilder_AutoCapture(), icon::AutoCapture, C::AutoCapture),
        Entry::separator(),
        item(s::MainMenuBuilder_ShowCursor(), icon::Pointer, C::ToggleShowCursor)
            .toggle(Toggle::CheckBox, state.show_cursor),
        parent(&screenshot_delay(&format_delay(state.screenshot_delay)), icon::Timer, screenshot_delay_menu(state)),
    ]
}

/// `BuildWindowMenu`.
fn window_menu(state: &MenuState) -> Vec<Entry> {
    let mut items: Vec<Entry> = state
        .windows
        .iter()
        .map(|title| {
            // `Truncate(50, "...")`.
            let shown = if title.chars().count() > 50 {
                format!("{}...", title.chars().take(50).collect::<String>())
            } else {
                title.clone()
            };
            Entry::item(shown, "", C::CaptureWindow(title.clone()))
        })
        .collect();
    if items.is_empty() {
        items.push(Entry::label(s::MainMenuBuilder_NoWindowsFound(), icon::Window).enabled(false));
    }
    items
}

/// `BuildMonitorMenu`.
fn monitor_menu(state: &MenuState) -> Vec<Entry> {
    state
        .monitors
        .iter()
        .enumerate()
        .map(|(i, &(x, y, w, h))| item(&format!("{}. {w}x{h}", i + 1), icon::Monitor, C::CaptureMonitor(x, y, w, h)))
        .collect()
}

/// `BuildScreenshotDelayMenu`.
fn screenshot_delay_menu(state: &MenuState) -> Vec<Entry> {
    let mut items: Vec<Entry> = (0..6)
        .map(|delay| {
            let icon = if delay == 0 { icon::TimerOff } else { icon::Timer };
            item(&screenshot_delay(&delay.to_string()), icon, C::SetScreenshotDelay(delay))
                .toggle(Toggle::Radio, (state.screenshot_delay - delay as f32).abs() < 0.01)
        })
        .collect();
    items.push(Entry::separator());
    items.push(
        item(s::MainMenuBuilder_ScreenshotDelayJustOnce(), icon::Timer, C::ToggleScreenshotDelayJustOnce)
            .toggle(Toggle::CheckBox, state.screenshot_delay_just_once),
    );
    items
}

/// `BuildUploadMenu`.
pub fn upload_menu() -> Vec<Entry> {
    vec![
        item(s::MainMenuBuilder_UploadFile(), icon::FileUpload, C::UploadFile),
        item(s::MainMenuBuilder_UploadFolder(), icon::FolderUpload, C::UploadFolder),
        item(s::MainMenuBuilder_UploadClipboard(), icon::UploadClipboard, C::UploadClipboard),
        item(s::MainMenuBuilder_UploadText(), icon::FileText, C::UploadText),
        item(s::MainMenuBuilder_UploadUrl(), icon::Link, C::UploadUrl),
        item(s::MainMenuBuilder_DragAndDropUpload(), icon::DragDropUpload, C::DragAndDropUpload),
        item(s::MainMenuBuilder_ShortenUrl(), icon::ShortLink, C::ShortenUrl),
    ]
}

/// `BuildWorkflowsMenu` (the main window's: with "Add workflows…").
pub fn workflows_menu(state: &MenuState) -> Vec<Entry> {
    let mut items: Vec<Entry> = state
        .workflows
        .iter()
        .enumerate()
        .filter(|(_, w)| !(state.workflows_only_show_edited && w.uses_default_settings))
        .map(|(i, w)| {
            let mut title = w.name.clone();
            if !w.uses_default_settings {
                title.push('*');
            }
            if let Some(hotkey) = &w.hotkey {
                title.push_str("    ");
                title.push_str(hotkey);
            }
            item(&title, w.icon, C::RunWorkflow(i))
        })
        .collect();
    if !items.is_empty() {
        items.push(Entry::separator());
    }
    items.push(item(s::MainMenuBuilder_AddWorkflowsFromHotkeySettings(), icon::Keyboard, C::HotkeySettings));
    items
}

/// `BuildToolCategories`.
pub fn tool_categories() -> Vec<MenuCategory<MenuCommand>> {
    macro_rules! tool {
        ($key:ident, $icon:expr) => {
            Entry::item(s::$key(), $icon, C::Tool(stringify!($key))).id(stringify!($key))
        };
        ($key:ident, $icon:expr, new) => {
            tool!($key, $icon).badge(s::MainMenuBuilder_NewBadge())
        };
    }
    vec![
        MenuCategory::new(s::MainMenuBuilder_ToolCategoryScreen(), vec![
            tool!(MainMenuBuilder_ColorPicker, icon::Palette),
            tool!(MainMenuBuilder_Ruler, icon::Measure),
            tool!(MainMenuBuilder_MouseHighlighter, icon::MouseHighlighter),
            tool!(MainMenuBuilder_PinToScreenDialog, icon::Pin),
            tool!(MainMenuBuilder_ClassicRuler, icon::Ruler),
            tool!(MainMenuBuilder_ScreenColorPicker, icon::ScreenColorPicker),
        ]),
        MenuCategory::new(s::MainMenuBuilder_ToolCategoryImages(), vec![
            tool!(MainMenuBuilder_BackgroundRemover, icon::BackgroundRemover),
            tool!(MainMenuBuilder_IconConverter, icon::IconConverter),
            tool!(MainMenuBuilder_ImageBeautifier, icon::ImageBeautifier),
            tool!(MainMenuBuilder_ImageCombiner, icon::Combine),
            tool!(MainMenuBuilder_ImageComparer, icon::ImageComparer),
            tool!(MainMenuBuilder_ImageConverter, icon::ImageConverter),
            tool!(MainMenuBuilder_ImageEditor, icon::ImageEditor),
            tool!(MainMenuBuilder_ImageEffects, icon::MagicWand),
            tool!(MainMenuBuilder_ImageResizer, icon::Resize),
            tool!(MainMenuBuilder_ImageSplitter, icon::Split),
            tool!(MainMenuBuilder_ImageThumbnailer, icon::ImageThumbnailer),
            tool!(MainMenuBuilder_PhotoEditor, icon::Layers, new),
            tool!(MainMenuBuilder_PixelEditor, icon::PixelEditor, new),
            tool!(MainMenuBuilder_ImageViewer, icon::Eye),
            tool!(MainMenuBuilder_ImageWatermark, icon::Stamp),
        ]),
        MenuCategory::new(s::MainMenuBuilder_ToolCategoryVideo(), vec![
            tool!(MainMenuBuilder_AnimatedGifMaker, icon::AnimatedGifMaker),
            tool!(MainMenuBuilder_AnimatedGifTrimmer, icon::ScissorsFrame),
            tool!(MainMenuBuilder_VideoConverter, icon::VideoConverter),
            tool!(MainMenuBuilder_VideoPlayer, icon::CirclePlay),
            tool!(MainMenuBuilder_VideoThumbnailer, icon::Clapperboard),
            tool!(MainMenuBuilder_VideoTrimmer, icon::Scissors),
        ]),
        MenuCategory::new(s::MainMenuBuilder_ToolCategoryData(), vec![
            tool!(MainMenuBuilder_AnalyzeImage, icon::Robot),
            tool!(MainMenuBuilder_HashChecker, icon::Hash),
            tool!(MainMenuBuilder_IndexFolder, icon::FolderTree),
            tool!(MainMenuBuilder_Metadata, icon::Tag),
            tool!(MainMenuBuilder_OCR, icon::TextRecognition),
            tool!(MainMenuBuilder_QRCode, icon::QrCode),
            tool!(MainMenuBuilder_RemoteStorageBrowser, icon::RemoteStorageBrowser),
        ]),
        MenuCategory::new(s::MainMenuBuilder_ToolCategorySystem(), vec![
            tool!(MainMenuBuilder_BorderlessWindow, icon::BorderlessWindow),
            tool!(MainMenuBuilder_ClipboardViewer, icon::ClipboardViewer),
            tool!(MainMenuBuilder_InspectWindow, icon::InspectWindow),
            tool!(MainMenuBuilder_MonitorTest, icon::MonitorTest),
            tool!(MainMenuBuilder_NetworkMonitor, icon::Activity),
        ]),
    ]
}

/// `AfterCaptureTasks`, in declaration order, with
/// `GetAfterCaptureTaskIcon`'s icons.
const AFTER_CAPTURE_TASKS: [(&str, &str); 23] = [
    ("ShowQuickTaskMenu", icon::Menu),
    ("ShowAfterCaptureWindow", icon::Window),
    ("BeautifyImage", icon::ImageBeautifier),
    ("AddImageEffects", icon::MagicWand),
    ("AnnotateImage", icon::ImageEditor),
    ("OpenInPhotoEditor", icon::Layers),
    ("CopyImageToClipboard", icon::ClipboardImage),
    ("PinToScreen", icon::Pin),
    ("SendImageToPrinter", icon::Printer),
    ("SaveImageToFile", icon::Save),
    ("SaveImageToFileWithDialog", icon::SaveAs),
    ("SaveThumbnailImageToFile", icon::ImageDownload),
    ("PerformActions", icon::Terminal),
    ("CopyFileToClipboard", icon::ClipboardCopy),
    ("CopyFilePathToClipboard", icon::ClipboardList),
    ("CopyFolderPathToClipboard", icon::FolderBookmark),
    ("ShowInExplorer", icon::FolderOpen),
    ("AnalyzeImage", icon::Robot),
    ("ScanQRCode", icon::QrCode),
    ("DoOCR", icon::TextRecognition),
    ("ShowBeforeUploadWindow", icon::Window),
    ("UploadImageToHost", icon::CloudUpload),
    ("DeleteFile", icon::Trash),
];

/// `AfterUploadTasks`, with `GetAfterUploadTaskIcon`'s icons.
const AFTER_UPLOAD_TASKS: [(&str, &str); 6] = [
    ("ShowAfterUploadWindow", icon::Window),
    ("UseURLShortener", icon::ShortLink),
    ("ShareURL", icon::Share),
    ("CopyURLToClipboard", icon::ClipboardLink),
    ("OpenURL", icon::ExternalLink),
    ("ShowQRCode", icon::QrCode),
];

/// `Enum.GetLocalizedDescription()`: `<EnumType>_<Value>` in HelpersLib's
/// strings, else the `[Description]` (passed as `fallback`).
fn describe(enum_type: &str, value: &str, fallback: &str) -> String {
    hs::lookup(&format!("{enum_type}_{value}")).unwrap_or(fallback).to_string()
}

/// `BuildAfterCaptureMenu`.
pub fn after_capture_menu(state: &MenuState) -> Vec<Entry> {
    AFTER_CAPTURE_TASKS
        .iter()
        .filter(|(task, _)| state.uploads_enabled || !matches!(*task, "ShowBeforeUploadWindow" | "UploadImageToHost"))
        .map(|&(task, icon)| {
            let mut e = Entry::item(describe("AfterCaptureTasks", task, task), icon, C::ToggleAfterCaptureTask(task))
                .toggle(Toggle::CheckBox, state.after_capture.contains(&task))
                .stays_open()
                .bold_when_checked();
            if task == "AddImageEffects" {
                e = e.children(image_effect_preset_menu(state));
            }
            e
        })
        .collect()
}

/// `BuildImageEffectPresetMenu`.
fn image_effect_preset_menu(state: &MenuState) -> Vec<Entry> {
    let mut items: Vec<Entry> = state
        .image_effect_presets
        .iter()
        .enumerate()
        .map(|(i, name)| {
            Entry::item(name.as_str(), "", C::SelectImageEffectPreset(i))
                .toggle(Toggle::Radio, i == state.selected_image_effect_preset)
                .stays_open()
                .bold_when_checked()
        })
        .collect();
    if items.is_empty() {
        items.push(Entry::label(s::MainMenuBuilder_NoImageEffectPresets(), "").enabled(false));
    }
    items
}

/// `BuildAfterUploadMenu`.
pub fn after_upload_menu(state: &MenuState) -> Vec<Entry> {
    AFTER_UPLOAD_TASKS
        .iter()
        .map(|&(task, icon)| {
            Entry::item(describe("AfterUploadTasks", task, task), icon, C::ToggleAfterUploadTask(task))
                .toggle(Toggle::CheckBox, state.after_upload.contains(&task))
                .stays_open()
                .bold_when_checked()
        })
        .collect()
}

/// `ShareX.UploadersLib` enums: (value, `[Description]`).
const IMAGE_DESTINATIONS: [(&str, &str); 8] = [
    ("Imgur", "Imgur"),
    ("ImageShack", "ImageShack"),
    ("Flickr", "Flickr"),
    ("Photobucket", "Photobucket"),
    ("Chevereto", "Chevereto"),
    ("Vgyme", "vgy.me"),
    ("CustomImageUploader", "CustomImageUploader"),
    ("FileUploader", "FileUploader"),
];

const TEXT_DESTINATIONS: [(&str, &str); 12] = [
    ("Pastebin", "Pastebin"),
    ("Paste2", "Paste2"),
    ("Slexy", "Slexy"),
    ("Paste_ee", "Paste.ee"),
    ("Gist", "GitHub Gist"),
    ("Upaste", "uPaste"),
    ("Hastebin", "Hastebin"),
    ("OneTimeSecret", "OneTimeSecret"),
    ("Pastie", "Pastie"),
    ("PrivateBin", "PrivateBin"),
    ("CustomTextUploader", "CustomTextUploader"),
    ("FileUploader", "FileUploader"),
];

const FILE_DESTINATIONS: [(&str, &str); 31] = [
    ("Dropbox", "Dropbox"),
    ("FTP", "FTP"),
    ("OneDrive", "OneDrive"),
    ("GoogleDrive", "Google Drive"),
    ("Puush", "puush"),
    ("Box", "Box"),
    ("Mega", "MEGA"),
    ("AmazonS3", "Amazon S3"),
    ("GoogleCloudStorage", "Google Cloud Storage"),
    ("AzureStorage", "Azure Storage"),
    ("BackblazeB2", "Backblaze B2"),
    ("OwnCloud", "ownCloud / Nextcloud"),
    ("Immich", "Immich"),
    ("MediaFire", "MediaFire"),
    ("Pushbullet", "Pushbullet"),
    ("SendSpace", "SendSpace"),
    ("Localhostr", "Hostr"),
    ("Lambda", "Lambda"),
    ("ImgFish", "img.fish"),
    ("Pomf", "Pomf"),
    ("Uguu", "Uguu"),
    ("Seafile", "Seafile"),
    ("Streamable", "Streamable"),
    ("Sul", "s-ul"),
    ("Lithiio", "LobFile"),
    ("Transfersh", "transfer.sh"),
    ("Plik", "Plik"),
    ("YouTube", "YouTube"),
    ("SharedFolder", "SharedFolder"),
    ("Email", "Email"),
    ("CustomFileUploader", "CustomFileUploader"),
];

const URL_SHORTENERS: [(&str, &str); 14] = [
    ("BITLY", "bit.ly"),
    ("ISGD", "is.gd"),
    ("VGD", "v.gd"),
    ("TINYURL", "tinyurl.com"),
    ("TURL", "turl.ca"),
    ("YOURLS", "yourls.org"),
    ("QRnet", "qr.net"),
    ("VURL", "vurl.com"),
    ("TwoGP", "2.gp"),
    ("Polr", "Polr"),
    ("FirebaseDynamicLinks", "Firebase Dynamic Links"),
    ("Kutt", "Kutt"),
    ("ZeroWidthShortener", "Zero Width Shortener"),
    ("CustomURLShortener", "CustomURLShortener"),
];

const URL_SHARING_SERVICES: [(&str, &str); 13] = [
    ("Email", "Email"),
    ("Facebook", "Facebook"),
    ("Reddit", "Reddit"),
    ("Pinterest", "Pinterest"),
    ("Tumblr", "Tumblr"),
    ("LinkedIn", "LinkedIn"),
    ("StumbleUpon", "StumbleUpon"),
    ("Delicious", "Delicious"),
    ("VK", "VK"),
    ("Pushbullet", "Pushbullet"),
    ("GoogleImageSearch", "GoogleImageSearch"),
    ("BingVisualSearch", "BingVisualSearch"),
    ("CustomURLSharingService", "CustomURLSharingService"),
];

fn description(enum_type: &str, values: &[(&str, &str)], value: &str) -> String {
    let fallback = values.iter().find(|(v, _)| *v == value).map_or(value, |(_, d)| *d);
    describe(enum_type, value, fallback)
}

/// `BuildEnumDestinations`.
fn enum_destinations(
    enum_type: &str,
    values: &[(&'static str, &str)],
    selected: &str,
    command: fn(&'static str) -> MenuCommand,
) -> Vec<Entry> {
    values
        .iter()
        .map(|&(value, fallback)| {
            Entry::item(describe(enum_type, value, fallback), "", command(value))
                .toggle(Toggle::Radio, value == selected)
                .stays_open()
                .bold_when_checked()
        })
        .collect()
}

/// `Parent(headerFormat, createAccentText, …)`: the header with the chosen
/// destination in the accent colour.
fn accent_parent(format: &str, accent: String, icon: &'static str, children: Vec<Entry>) -> Entry {
    Entry::parent(format.replace("{0}", &accent), icon, children).accent(accent)
}

/// `BuildDestinationsMenu`.
pub fn destinations_menu(state: &MenuState) -> Vec<Entry> {
    let image_name = if state.image_destination == "FileUploader" {
        description("FileDestination", &FILE_DESTINATIONS, state.image_file_destination)
    } else {
        description("ImageDestination", &IMAGE_DESTINATIONS, state.image_destination)
    };
    let text_name = if state.text_destination == "FileUploader" {
        description("FileDestination", &FILE_DESTINATIONS, state.text_file_destination)
    } else {
        description("TextDestination", &TEXT_DESTINATIONS, state.text_destination)
    };
    // `BuildImageDestinations` / `BuildTextDestinations`: "File uploader" has
    // its own submenu of file destinations.
    let with_file_uploader = |enum_type: &str,
                              values: &[(&'static str, &str)],
                              selected: &str,
                              file_selected: &str,
                              set: fn(&'static str) -> MenuCommand,
                              set_file: fn(&'static str) -> MenuCommand| {
        values
            .iter()
            .map(|&(value, fallback)| {
                let mut e = Entry::item(describe(enum_type, value, fallback), "", set(value))
                    .toggle(Toggle::Radio, value == selected)
                    .stays_open()
                    .bold_when_checked();
                if value == "FileUploader" {
                    e = e.children(enum_destinations("FileDestination", &FILE_DESTINATIONS, file_selected, set_file));
                }
                e
            })
            .collect::<Vec<_>>()
    };
    vec![
        accent_parent(
            s::TaskSettingsForm_UpdateUploaderMenuNames_Image_uploader___0_(),
            image_name,
            icon::Image,
            with_file_uploader(
                "ImageDestination",
                &IMAGE_DESTINATIONS,
                state.image_destination,
                state.image_file_destination,
                C::SetImageDestination,
                C::SetImageFileDestination,
            ),
        ),
        accent_parent(
            s::TaskSettingsForm_UpdateUploaderMenuNames_Text_uploader___0_(),
            text_name,
            icon::FileText,
            with_file_uploader(
                "TextDestination",
                &TEXT_DESTINATIONS,
                state.text_destination,
                state.text_file_destination,
                C::SetTextDestination,
                C::SetTextFileDestination,
            ),
        ),
        accent_parent(
            s::TaskSettingsForm_UpdateUploaderMenuNames_File_uploader___0_(),
            description("FileDestination", &FILE_DESTINATIONS, state.file_destination),
            icon::FileUpload,
            enum_destinations("FileDestination", &FILE_DESTINATIONS, state.file_destination, C::SetFileDestination),
        ),
        accent_parent(
            s::TaskSettingsForm_UpdateUploaderMenuNames_URL_shortener___0_(),
            description("UrlShortenerType", &URL_SHORTENERS, state.url_shortener),
            icon::ShortLink,
            enum_destinations("UrlShortenerType", &URL_SHORTENERS, state.url_shortener, C::SetUrlShortener),
        ),
        accent_parent(
            s::TaskSettingsForm_UpdateUploaderMenuNames_URL_sharing_service___0_(),
            description("URLSharingServices", &URL_SHARING_SERVICES, state.url_sharing_service),
            icon::Share,
            enum_destinations(
                "URLSharingServices",
                &URL_SHARING_SERVICES,
                state.url_sharing_service,
                C::SetUrlSharingService,
            ),
        ),
    ]
}

/// `BuildDebugMenu`.
pub fn debug_menu(state: &MenuState) -> Vec<Entry> {
    let up = state.uploads_enabled;
    vec![
        item(s::MainMenuBuilder_ShowDebugLog(), icon::FileText, C::DebugLog),
        item(s::MainMenuBuilder_DisplayDiagnostics(), icon::Monitor, C::DisplayDiagnostics),
        item(s::MainMenuBuilder_TestImageUpload(), icon::ImageUpload, C::TestImageUpload).visible(up),
        item(s::MainMenuBuilder_TestTextUpload(), icon::FileUpload, C::TestTextUpload).visible(up),
        item(s::MainMenuBuilder_TestFileUpload(), icon::Upload, C::TestFileUpload).visible(up),
        item(s::MainMenuBuilder_TestUrlShortener(), icon::ShortLink, C::TestUrlShortener).visible(up),
        item(s::MainMenuBuilder_TestUrlSharing(), icon::Share, C::TestUrlSharing).visible(up),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menus_use_sharex_strings() {
        let state = MenuState::default();
        assert_eq!(capture_menu(&state)[0].header, "Fullscreen");
        assert_eq!(capture_menu(&state).last().unwrap().header, "Screenshot delay: 0s");
        let dest = destinations_menu(&state);
        assert_eq!(dest[0].header, "Image uploader: Imgur");
        assert_eq!(dest[0].accent.as_deref(), Some("Imgur"));
        assert_eq!(after_capture_menu(&state)[0].header, "Show quick task menu");
        let tools = tool_categories();
        assert_eq!(tools[0].entries[0].header, "Color picker...");
        assert_eq!(workflows_menu(&state)[0].header, "Capture region    Ctrl + Print Screen");
    }

    #[test]
    fn delay_format() {
        assert_eq!(format_delay(0.0), "0");
        assert_eq!(format_delay(1.5), "1.5");
    }
}
