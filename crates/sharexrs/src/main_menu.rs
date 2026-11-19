//! The main window's menus: `ShareX/Presentation/MainWindow/MainMenuBuilder.cs`.
//!
//! Every sidebar entry with a chevron opens one of these. The actions
//! behind the entries aren't ported yet: each entry carries a
//! [`MenuCommand`] for the app to carry out once they are.

use sharex_strings::{helpers_lib as hs, sharex as s};
use sharex_ui::keys::app_icons as icon;
use std::sync::Arc;

use sharex_ui::menu::{MenuCategory, MenuEntry, MenuIcon, Toggle};

/// What a menu entry does (`MainMenuBuilder`'s lambdas, by name).
#[derive(Clone, Debug, PartialEq)]
pub enum MenuCommand {
    // Capture
    CaptureFullscreen,
    /// Capture › Window › a window (its handle).
    CaptureWindow(isize),
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
    /// `TaskHelpers.ExecuteJob(workflow.TaskSettings)`: a hotkey's task, by
    /// its index in `HotkeysConfig.Hotkeys`.
    RunWorkflow(usize),
    /// The tray's Recent items › Copy / Open (`RecentTask.Copy`, `Open`), by
    /// the recent task's index in `TaskManager.RecentManager.Tasks`.
    RecentCopy(usize),
    RecentOpen(usize),
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
    // The tray menu (`BuildTrayMenu`, `MainFormCommand`)
    ApplicationSettings,
    TaskSettings,
    /// `TaskHelpers.ToggleHotkeys`.
    ToggleHotkeys,
    DestinationSettings,
    CustomUploaderSettings,
    ScreenshotsFolder,
    History,
    ImageHistory,
    RestartAsAdministrator,
    ActionsToolbar,
    /// `MainWindowIntegration.Activate`.
    ShowMainWindow,
    /// `ApplicationLifecycle.ForceClose`.
    Exit,
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
    /// Its index in `HotkeysConfig.Hotkeys`.
    pub hotkey_index: usize,
}

/// `BuildWorkflowsMenu`'s hotkeys: every one with a job, as the menu shows it.
pub fn workflows_from_hotkeys(hotkeys: &[sharex_settings::HotkeySettings]) -> Vec<Workflow> {
    hotkeys
        .iter()
        .enumerate()
        .filter(|(_, h)| h.TaskSettings.Job != sharex_settings::HotkeyType::None)
        .map(|(i, h)| Workflow {
            name: h.TaskSettings.display_name(),
            hotkey: h.HotkeyInfo.is_valid_hotkey().then(|| h.HotkeyInfo.to_string()),
            icon: find_menu_icon(&h.TaskSettings.Job),
            uses_default_settings: h.TaskSettings.is_using_default_settings(),
            hotkey_index: i,
        })
        .collect()
}

/// A recent task as the tray's Recent items lists it.
#[derive(Clone, Debug)]
pub struct RecentItem {
    /// `RecentTask.TrayMenuText`.
    pub text: String,
    /// `GetRecentTaskIcon`.
    pub icon: &'static str,
    /// Its index in `TaskManager.RecentManager.Tasks`.
    pub index: usize,
}

/// `BuildRecentItemsMenu`'s items, most recent first when
/// `RecentTasksTrayMenuMostRecentFirst`.
pub fn recent_items(tasks: &[crate::tasks::RecentTask], most_recent_first: bool) -> Vec<RecentItem> {
    let mut items: Vec<RecentItem> = tasks.iter().enumerate().map(|(i, t)| RecentItem { text: t.tray_menu_text(), icon: t.icon(), index: i }).collect();
    if most_recent_first {
        items.reverse();
    }
    items
}

/// `HotkeyManager.GetDefaultHotkeyList`, until hotkeys are ported.
pub fn default_workflows() -> Vec<Workflow> {
    let w = |i: usize, name: &str, hotkey: &str, icon| Workflow {
        name: name.into(),
        hotkey: Some(hotkey.into()),
        icon,
        uses_default_settings: true,
        hotkey_index: i,
    };
    vec![
        w(0, hs::HotkeyType_RectangleRegion(), "Ctrl + Print Screen", icon::Region),
        w(1, hs::HotkeyType_PrintScreen(), "Print Screen", icon::Fullscreen),
        w(2, hs::HotkeyType_ActiveWindow(), "Alt + Print Screen", icon::Window),
        w(3, hs::HotkeyType_ScreenRecorder(), "Shift + Print Screen", icon::ScreenRecording),
        w(4, hs::HotkeyType_ScreenRecorderGIF(), "Ctrl + Shift + Print Screen", icon::Film),
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
    /// Capture › Window's windows, when not listed live.
    pub windows: Vec<WindowItem>,
    /// Capture › Window lists the windows on screen when the menu is built
    /// (`WindowsList.GetVisibleWindowsList`); off for `--render`, which
    /// shows [`windows`](Self::windows) instead.
    pub list_windows: bool,
    /// `ApplicationConfig.DisableHotkeys`, for the tray's toggle.
    pub hotkeys_disabled: bool,
    /// The tray's Recent items entry is shown: `RecentTasksSave` and
    /// `RecentTasksShowInTrayMenu` (and some recent task).
    pub show_recent_items: bool,
    /// Recent items › the tasks.
    pub recent_items: Vec<RecentItem>,
    /// `ApplicationConfig.HiddenTools`: the tool grid leaves these out.
    pub hidden_tools: Vec<String>,
}

/// A window in Capture › Window.
#[derive(Clone, Debug)]
pub struct WindowItem {
    pub handle: isize,
    pub title: String,
    /// `GetWindowIcon`: the window's icon at its own size.
    pub icon: Option<Arc<egui::ColorImage>>,
}

impl WindowItem {
    /// The windows on screen, with their icons. An icon that hasn't
    /// changed since the last listing is the same `Arc`, so menus draw it
    /// from the texture they made for it then.
    pub fn list() -> Vec<Self> {
        use std::collections::HashMap;
        use std::sync::Mutex;
        static ICONS: Mutex<Option<HashMap<isize, Arc<egui::ColorImage>>>> = Mutex::new(None);
        let mut cache = ICONS.lock().unwrap_or_else(|e| e.into_inner());
        let previous = cache.take().unwrap_or_default();
        let mut icons = HashMap::new();
        let items = sharex_capture::platform::visible_windows()
            .into_iter()
            .map(|w| {
                let icon = sharex_capture::platform::window_icon(w.handle).map(|i| {
                    let size = [i.width() as usize, i.height() as usize];
                    let image = egui::ColorImage::from_rgba_unmultiplied(size, i.as_raw());
                    match previous.get(&w.handle) {
                        Some(same) if **same == image => same.clone(),
                        _ => Arc::new(image),
                    }
                });
                if let Some(icon) = &icon {
                    icons.insert(w.handle, icon.clone());
                }
                WindowItem { handle: w.handle, icon, title: w.title }
            })
            .collect();
        *cache = Some(icons);
        items
    }
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
            list_windows: false,
            hotkeys_disabled: false,
            show_recent_items: false,
            recent_items: Vec::new(),
            hidden_tools: Vec::new(),
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
    let listed;
    let windows = if state.list_windows {
        listed = WindowItem::list();
        &listed
    } else {
        &state.windows
    };
    let mut items: Vec<Entry> = windows
        .iter()
        .map(|w| {
            // `Truncate(50, "...")`.
            let shown = if w.title.chars().count() > 50 {
                format!("{}...", w.title.chars().take(50).collect::<String>())
            } else {
                w.title.clone()
            };
            let icon = w.icon.clone().map_or(MenuIcon::None, MenuIcon::Bitmap);
            Entry::item(shown, "", C::CaptureWindow(w.handle)).icon(icon)
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

/// `BuildTrayMenu`.
pub fn tray_menu(state: &MenuState) -> Vec<Entry> {
    let up = state.uploads_enabled;
    let (hotkeys_text, hotkeys_icon) = if state.hotkeys_disabled {
        (s::MainMenuBuilder_EnableHotkeys(), icon::Keyboard)
    } else {
        (s::MainMenuBuilder_DisableHotkeys(), icon::KeyboardOff)
    };
    let tools = tool_categories();
    vec![
        parent(s::MainMenuBuilder_Capture(), icon::Camera, capture_menu(state)),
        parent(s::MainMenuBuilder_Upload(), icon::Upload, upload_menu()).visible(up),
        parent(s::MainMenuBuilder_Workflows(), icon::Workflows, workflows_menu(state, true)),
        Entry::label(s::MainMenuBuilder_Tools(), icon::Wrench).categories(tools),
        Entry::separator(),
        parent(s::MainMenuBuilder_AfterCaptureTasks(), icon::AfterCaptureTasks, after_capture_menu(state)),
        parent(s::MainMenuBuilder_AfterUploadTasks(), icon::AfterUploadTasks, after_upload_menu(state)).visible(up),
        parent(s::MainMenuBuilder_Destinations(), icon::Destinations, destinations_menu(state)).visible(up),
        Entry::separator(),
        item(s::MainMenuBuilder_ApplicationSettings(), icon::Settings, C::ApplicationSettings),
        item(s::MainMenuBuilder_TaskSettings(), icon::Sliders, C::TaskSettings),
        item(s::MainMenuBuilder_HotkeySettings(), icon::Keyboard, C::HotkeySettings),
        item(hotkeys_text, hotkeys_icon, C::ToggleHotkeys),
        item(s::MainMenuBuilder_DestinationSettings(), icon::DestinationSettings, C::DestinationSettings).visible(up),
        item(s::MainMenuBuilder_CustomUploaderSettings(), icon::CustomUploaderSettings, C::CustomUploaderSettings)
            .visible(up),
        Entry::separator(),
        item(s::MainMenuBuilder_ScreenshotsFolder(), icon::ScreenshotsFolder, C::ScreenshotsFolder),
        item(s::MainMenuBuilder_History(), icon::History, C::History),
        item(s::MainMenuBuilder_ImageHistory(), icon::ImageHistory, C::ImageHistory),
        Entry::separator(),
        item(s::MainMenuBuilder_RestartAsAdministrator(), icon::RestartAsAdmin, C::RestartAsAdministrator),
        parent(s::MainMenuBuilder_RecentItems(), icon::RecentItems, recent_items_menu(state))
            .visible(state.show_recent_items && !state.recent_items.is_empty()),
        item(s::MainMenuBuilder_ActionsToolbar(), icon::Toolbar, C::ActionsToolbar),
        item(s::MainMenuBuilder_ShowShareX(), icon::Window, C::ShowMainWindow),
        item(s::MainMenuBuilder_Exit(), icon::Exit, C::Exit),
    ]
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

/// `BuildRecentItemsMenu`: each task with Copy and Open.
pub fn recent_items_menu(state: &MenuState) -> Vec<Entry> {
    state
        .recent_items
        .iter()
        .map(|r| {
            parent(&r.text, r.icon, vec![
                item(s::MainMenuBuilder_Copy(), icon::Copy, C::RecentCopy(r.index)),
                item(s::MainMenuBuilder_Open(), icon::ExternalLink, C::RecentOpen(r.index)),
            ])
        })
        .collect()
}

/// `BuildWorkflowsMenu`: the main window's ends with "Add workflows…",
/// the tray's (`_trayMenu`) doesn't.
pub fn workflows_menu(state: &MenuState, tray: bool) -> Vec<Entry> {
    let mut items: Vec<Entry> = state
        .workflows
        .iter()
        .filter(|w| !(state.workflows_only_show_edited && w.uses_default_settings))
        .map(|w| {
            let mut title = w.name.clone();
            if !w.uses_default_settings {
                title.push('*');
            }
            if let Some(hotkey) = &w.hotkey {
                title.push_str("    ");
                title.push_str(hotkey);
            }
            item(&title, w.icon, C::RunWorkflow(w.hotkey_index))
        })
        .collect();
    if !tray {
        if !items.is_empty() {
            items.push(Entry::separator());
        }
        items.push(item(s::MainMenuBuilder_AddWorkflowsFromHotkeySettings(), icon::Keyboard, C::HotkeySettings));
    }
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
pub const AFTER_CAPTURE_TASKS: [(&str, &str); 23] = [
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
pub const AFTER_UPLOAD_TASKS: [(&str, &str); 6] = [
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

/// `TaskHelpers.FindMenuIcon(hotkeyType)`.
pub fn find_menu_icon(job: &sharex_settings::HotkeyType) -> &'static str {
    use sharex_settings::HotkeyType as H;
    match job {
        H::None => icon::CircleDashed,
        // Upload
        H::FileUpload => icon::FileUpload,
        H::FolderUpload => icon::FolderUpload,
        H::ClipboardUpload => icon::UploadClipboard,
        H::ClipboardUploadWithContentViewer => icon::ClipboardViewer,
        H::UploadText => icon::FileText,
        H::UploadURL => icon::Link,
        H::DragDropUpload => icon::DragDropUpload,
        H::ShortenURL => icon::ShortLink,
        H::StopUploads => icon::CircleStop,
        // Screen capture
        H::PrintScreen => icon::Fullscreen,
        H::ActiveWindow | H::CustomWindow => icon::Window,
        H::ActiveMonitor => icon::Monitor,
        H::RectangleRegion => icon::Region,
        H::RectangleTransparent => icon::RegionTransparent,
        H::CustomRegion => icon::Crop,
        H::LastRegion => icon::LastRegion,
        H::ScrollingCapture => icon::ScrollingCapture,
        H::AutoCapture => icon::AutoCapture,
        H::StartAutoCapture => icon::CirclePlay,
        H::StopAutoCapture => icon::TimerOff,
        // Screen record
        H::ScreenRecorder => icon::ScreenRecording,
        H::ScreenRecorderActiveWindow => icon::Window,
        H::ScreenRecorderCustomRegion => icon::Crop,
        H::StartScreenRecorder => icon::CirclePlay,
        H::ScreenRecorderGIF | H::ScreenRecorderGIFActiveWindow => icon::Film,
        H::ScreenRecorderGIFCustomRegion => icon::Crop,
        H::StartScreenRecorderGIF => icon::CirclePlay,
        H::StopScreenRecording => icon::CircleStop,
        H::PauseScreenRecording => icon::CirclePause,
        H::AbortScreenRecording => icon::CircleX,
        // Tools
        H::ColorPicker => icon::Palette,
        H::ScreenColorPicker => icon::ScreenColorPicker,
        H::Ruler => icon::Measure,
        H::ClassicRuler => icon::Ruler,
        H::MouseHighlighter => icon::MouseHighlighter,
        H::PinToScreen => icon::Pin,
        H::PinToScreenFromScreen => icon::PictureInPicture,
        H::PinToScreenFromClipboard => icon::Clipboard,
        H::PinToScreenFromFile => icon::FileImage,
        H::PinToScreenCloseAll => icon::Unpin,
        H::ImageEditor => icon::ImageEditor,
        H::PhotoEditor => icon::Layers,
        H::PixelEditor => icon::PixelEditor,
        H::ImageBeautifier => icon::ImageBeautifier,
        H::ImageEffects => icon::MagicWand,
        H::ImageViewer => icon::Eye,
        H::BackgroundRemover => icon::BackgroundRemover,
        H::ImageComparer => icon::ImageComparer,
        H::IconConverter => icon::IconConverter,
        H::ImageCombiner => icon::Combine,
        H::ImageSplitter => icon::Split,
        H::ImageResizer => icon::Resize,
        H::ImageConverter => icon::ImageConverter,
        H::ImageWatermark => icon::Stamp,
        H::ImageThumbnailer => icon::ImageThumbnailer,
        H::VideoConverter => icon::VideoConverter,
        H::VideoTrimmer => icon::Scissors,
        H::AnimatedGifTrimmer => icon::ScissorsFrame,
        H::VideoThumbnailer => icon::Clapperboard,
        H::AnalyzeImage => icon::Robot,
        H::OCR => icon::TextRecognition,
        H::QRCode => icon::QrCode,
        H::QRCodeDecodeFromScreen => icon::ScanEye,
        H::QRCodeScanRegion => icon::QrScan,
        H::HashCheck => icon::Hash,
        H::Metadata => icon::Tag,
        H::StripMetadata => icon::FileDelete,
        H::IndexFolder => icon::FolderTree,
        H::ClipboardViewer => icon::ClipboardViewer,
        H::BorderlessWindow | H::ActiveWindowBorderless => icon::BorderlessWindow,
        H::ActiveWindowTopMost => icon::Pin,
        H::InspectWindow => icon::InspectWindow,
        H::NetworkMonitor => icon::Activity,
        H::MonitorTest => icon::MonitorTest,
        // Other
        H::DisableHotkeys => icon::KeyboardOff,
        H::OpenMainWindow => icon::Window,
        H::OpenScreenshotsFolder => icon::ScreenshotsFolder,
        H::OpenHistory => icon::History,
        H::OpenImageHistory => icon::ImageHistory,
        H::ToggleActionsToolbar => icon::Toolbar,
        H::ToggleTrayMenu => icon::Menu,
        H::ExitShareX => icon::Exit,
        _ => icon::Circle,
    }
}

/// The task picker (`HotkeySettingsWindow.OnTaskClick`,
/// `TaskSettingsPageBuilder.TaskMenu`): `None`, then a submenu per
/// category (`GroupBy(Category)`, in enum order), each task a radio item.
pub fn task_menu(selected: &sharex_settings::HotkeyType) -> Vec<MenuEntry<sharex_settings::HotkeyType>> {
    use sharex_settings::HotkeyType as H;
    let item = |h: &H| {
        MenuEntry::item(h.localized_description(), find_menu_icon(h), h.clone()).toggle(Toggle::Radio, h == selected)
    };
    let mut values: Vec<&H> = H::ALL.iter().collect();
    // `Helpers.GetEnums` is `Enum.GetValues`: sorted by value.
    values.sort_by_key(|h| h.value().unwrap_or(i64::MAX));
    let mut root = Vec::new();
    let mut groups: Vec<(String, Vec<MenuEntry<H>>)> = Vec::new();
    for h in values {
        match h.localized_category() {
            None => root.push(item(h)),
            Some(c) => match groups.iter_mut().find(|(k, _)| *k == c) {
                Some((_, items)) => items.push(item(h)),
                None => groups.push((c, vec![item(h)])),
            },
        }
    }
    root.extend(groups.into_iter().map(|(c, items)| MenuEntry::parent(c, "", items)));
    root
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
        assert_eq!(workflows_menu(&state, false)[0].header, "Capture region    Ctrl + Print Screen");
        assert_eq!(workflows_menu(&state, false).last().unwrap().header, s::MainMenuBuilder_AddWorkflowsFromHotkeySettings());
        assert_eq!(workflows_menu(&state, true).len(), 5);
    }

    #[test]
    fn delay_format() {
        assert_eq!(format_delay(0.0), "0");
        assert_eq!(format_delay(1.5), "1.5");
    }
}
