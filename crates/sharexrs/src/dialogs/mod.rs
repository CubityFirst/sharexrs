//! ShareX's upload windows, each in an OS window of its own: the text,
//! URL and shorten URL windows, the clipboard content viewer, the drag and
//! drop window, the multi upload and large file confirmations, the before
//! and after upload windows and the QR code window.
//!
//! Every one is a [`Dialog`]: it describes its window ([`Spec`]), draws its
//! client area and answers with [`Effect`]s, which the app carries out
//! (`App::dialog_effects`). ShareX's windows that block a task's thread
//! (`LargeFileUploadWarningWindowIntegration.Show`,
//! `BeforeUploadWindowIntegration.Show`) answer through a channel the
//! task's thread waits on.

pub mod about;
pub mod about_clicker;
pub mod actions_toolbar;
pub mod after_capture;
pub mod after_upload;
pub mod before_upload;
pub mod clipboard_upload;
pub mod confirm;
pub mod custom_uploaders;
pub mod debug_log;
pub mod display_diagnostics;
pub mod destination_settings;
pub mod drop_window;
pub mod email_window;
pub mod file_exist;
pub mod forms;
pub mod icon_pack_prompt;
pub mod image_viewer;
pub mod input_box;
pub mod layout;
pub mod message_dialog;
pub mod notification;
pub mod oauth_listener;
pub mod output_box;
pub mod print_window;
pub mod qr_code;
pub mod quick_task_editor;
pub mod render;
pub mod response;
pub mod text_upload;
pub mod tooltip_window;
pub mod update;
pub mod url_input;
pub mod youtube_options;

use std::path::PathBuf;

use image::RgbaImage;
use sharex_settings::{ApplicationConfig, TaskSettings};
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Icon, WindowAttributes, WindowButtons, WindowId, WindowLevel};

use crate::upload::UploadersConfigStore;
use crate::upload::manager::ClipboardContent;
use crate::window::EguiWindow;
use crate::{App, app_icon, center, set_theme};

/// How a dialog's window is made (its `Window` attributes in the `.axaml`).
#[derive(Clone, Debug)]
pub struct Spec {
    pub title: String,
    pub width: f32,
    pub height: f32,
    pub min_width: f32,
    pub min_height: f32,
    /// `CanResize`.
    pub resizable: bool,
    /// `SizeToContent="Height"`: the height comes from [`Dialog::content_height`].
    pub size_to_content: bool,
    /// `Topmost`.
    pub topmost: bool,
    /// `ShowInTaskbar="False"`.
    pub skip_taskbar: bool,
    /// `CanMaximize="False"`.
    pub maximizable: bool,
    /// `ShowActivated`.
    pub activate: bool,
    /// `WindowDecorations="None"` with a transparent background.
    pub borderless: bool,
    /// `WindowState="Maximized"`.
    pub maximized: bool,
    /// `MaxWidth` / `MaxHeight` (0: none).
    pub max_width: f32,
    pub max_height: f32,
    /// The pointer passes through it (a tooltip's popup).
    pub click_through: bool,
}

impl Spec {
    /// A resizable window `width` x `height` with these minimums.
    pub fn sized(title: impl Into<String>, width: f32, height: f32, min_width: f32, min_height: f32) -> Self {
        Self {
            title: title.into(),
            width,
            height,
            min_width,
            min_height,
            resizable: true,
            size_to_content: false,
            topmost: false,
            skip_taskbar: false,
            maximizable: true,
            activate: true,
            borderless: false,
            maximized: false,
            max_width: 0.0,
            max_height: 0.0,
            click_through: false,
        }
    }

    /// A fixed-size window as tall as its content.
    pub fn fit_height(title: impl Into<String>, width: f32, min_width: f32) -> Self {
        Self { resizable: false, size_to_content: true, ..Self::sized(title, width, 200.0, min_width, 0.0) }
    }
}

/// What the app does for a dialog.
pub enum Effect {
    /// The window closes (`Close()`).
    Close,
    /// Tasks to start (`UploadManager`).
    Upload(Box<UploadRequest>),
    /// `ClipboardHelpers.CopyText`.
    CopyText(String),
    /// `ClipboardHelpers.CopyImage`.
    CopyImage(RgbaImage),
    /// `URLHelpers.OpenURL`.
    OpenUrl(String),
    /// `FileHelpers.OpenFile`.
    OpenFile(PathBuf),
    /// `FileHelpers.OpenFolderWithFile`.
    OpenFolderWithFile(PathBuf),
    /// `BeginMoveDrag`.
    DragWindow,
    /// The window's tooltip for this element of its client area (points),
    /// in a popup window of its own above it; `None` closes it.
    Tooltip(Option<(String, egui::Rect)>),
    /// `DragDrop.DoDragDropAsync` with these files (`DragDropEffects.Copy`).
    DragFiles(Vec<PathBuf>),
    /// The same with `DragDropEffects.Copy | Move` (the toast's file).
    DragFilesMovable(Vec<PathBuf>),
    /// `SizeToContent="Height"` after the content changed.
    SetHeight(f32),
    /// The window restored at this client size (points), centred in its
    /// screen's work area (`EditorWindow.TryAutoSize`).
    SetSizeCentered(f32, f32),
    /// The window maximized.
    Maximize,
    /// The QR code window's screen scan: hidden for 250 ms, the screen
    /// grabbed and scanned, shown again ([`Dialog::scanned`]).
    ScanScreen,
    /// `SettingManager.SaveUploadersConfigAsync` and `RefreshMenus`.
    SaveUploadersConfig,
    /// `ResponseWindow.ShowInstance(result)`.
    ShowResponse(Box<response::ResponseData>),
    /// Custom uploader settings' "Clear all..." confirmation.
    ConfirmClearCustomUploaders,
    /// `TextUploadWindow.ShowAsync(text)` for an answer, not an upload.
    AskText(String, std::sync::mpsc::Sender<Option<String>>),
    /// `Window.Title` changed (the history windows' item counts).
    SetTitle(String),
    /// `ClipboardHelpers.CopyFile(files)`.
    CopyFiles(Vec<PathBuf>),
    /// `ClipboardHelpers.CopyImageFromFile`.
    CopyImageFile(PathBuf),
    /// The OCR window's Select region...: minimized, a region captured
    /// (`RegionCaptureTasks.GetRegionImageAsync`), restored.
    OcrSelectRegion,
    /// `TaskHelpers` entry points of the history windows' services
    /// (`HistoryWindowServices`), on a file.
    FileService(FileService, PathBuf),
    /// Another window of this one (`ShowDialog(this)`: a confirmation, the
    /// packager).
    OpenDialog(Box<dyn Dialog>),
    /// `ClipboardHelpers.GetImage()` handed back through [`Dialog::replace_image`]
    /// (the Image effects window's Load from clipboard).
    PasteImage,
    /// `UploadManager.RunImageTask(image, taskSettings)`: the image through
    /// the task's after capture tasks.
    RunImageTask(RgbaImage, Box<TaskSettings>),
    /// [`Effect::RunImageTask`] with `TaskMetadata.ProcessName` set.
    RunImageTaskAs(RgbaImage, Box<TaskSettings>, &'static str),
    /// What isn't ported yet (logged).
    NotPorted(&'static str),
    /// `TaskHelpers.OpenRemoteStorageBrowser` (Destination settings' button).
    OpenRemoteStorageBrowser,
    /// The image editor's colour popups' screen colour picker
    /// (`ScreenColorPickerWindow.PickAsync(owner)`): over the window, its
    /// colour handed back through [`Dialog::screen_color`].
    PickScreenColorOverWindow,
    /// The image editor's New from screenshot (`OnNewFromScreenshotRequested`):
    /// the window hidden for 250 ms, a region (`RegionCaptureTasks.
    /// GetRegionImageAsync`) or the full screen (`CaptureFullscreen`) with
    /// these task settings, shown again, the image handed back through
    /// [`Dialog::replace_image`].
    EditorCapture { fullscreen: bool, task: Box<TaskSettings> },
    /// The colour picker's Pick color from screen: minimized, the screen
    /// colour picker, restored, its colour handed back through
    /// [`Dialog::screen_color`].
    PickScreenColor,
    /// The mouse highlighter window's Start/Stop (`SetManualActive`).
    HighlighterToggle(Box<sharex_tools::mouse_highlighter::MouseHighlighterOptions>),
    /// The mouse highlighter's settings changed (`settingsChanged`).
    HighlighterOptions(Box<sharex_tools::mouse_highlighter::MouseHighlighterOptions>),
    /// A source of the Pin to screen startup window.
    PinFrom(crate::screen_tools::pin::Source),
    /// `TaskHelpers.PinToScreen(image, taskSettings)` (the image editor's
    /// Pin to screen).
    PinImage(RgbaImage, Box<TaskSettings>),
    /// `TaskHelpers.PrintImage`: the Print window, or straight to the
    /// printer (`DontShowPrintSettingsDialog`).
    PrintImage(RgbaImage),

    /// The pin options window saved (`ApplyOptions` on that pin).
    PinOptionsSaved(WindowId, Box<sharex_tools::pin::PinToScreenOptions>),
    /// `UpdateChecker.DownloadUpdateAsync` after Yes: the browser
    /// (portable) or the downloader window.
    UpdateDownload(Box<sharex_update::GitHubUpdateChecker>, update::UpdateOrigin),
    /// The update message, and the download it started, are done
    /// (`StartAsync` returns): the manager hears the answer, and ShareXrs
    /// exits when the installer (or the replaced program) takes over.
    UpdateFinished { result: sharex_update::DialogResult, origin: update::UpdateOrigin, shutdown: bool },
    /// The scrolling capture window's `SelectWindowAsync`.
    ScrollingSelect,
    /// `ScrollingCaptureService.StopCapture`.
    ScrollingStop,
    /// The scrolling capture's notification sound (`ActionCompleted`).
    ScrollingSound,
    /// The scrolling capture options overlay's OK.
    ScrollingOptionsSaved(Box<sharex_capture::scrolling::ScrollingCaptureOptions>),
    /// `RestoreAndActivate` / `ShowAndActivate`: shown, restored, activated.
    RestoreAndActivate,
    /// The auto capture window's Select region...
    AutoCaptureSelectRegion,
    /// The auto capture window hides to the tray, or comes back from it.
    AutoCaptureTray(bool),
    /// `DownloaderWindow.Install`'s question about an unsigned installer.
    AskInvalidSignature,
    /// `UpdateMessageWindow.OnOpened` for a later automatic check: minimised,
    /// flashing in the taskbar (`FlashWindowEx`).
    MinimizeAndFlash,
    /// `MessageBox.Show(owner, text, caption, OK, icon)`: a message that
    /// only needs reading.
    MessageBox(Box<crate::message_box::MessageBox>),
    /// `WindowState = WindowState.FullScreen` (true) or back.
    SetFullscreen(bool),
    /// The video trimmer's Save: `resolveFFmpegPath` (`CheckFFmpeg` with
    /// the window's task settings, its message when FFmpeg is missing),
    /// then [`Dialog::trimmer_export`] with the path.
    TrimmerExport,
    /// The after capture window's Continue (or copy): the task starts with
    /// the settings it chose.
    AfterCaptureAccepted(Box<after_capture::Accepted>),
    /// `ImageViewerWindowIntegration.ShowImage(image)`.
    ShowImage(RgbaImage),
    /// `ImageViewerWindowIntegration.ShowImage(imageData, displayName)`.
    ShowImageNamed(RgbaImage, String),
    /// The window's client size (points), where it is.
    SetInnerSize(f32, f32),
    /// The Actions toolbar's title context menu, at the pointer.
    ActionsToolbarMenu,
    /// `TaskHelpers.ExecuteJob(job)` with the default task settings.
    RunJob(sharex_settings::HotkeyType),
    /// The Actions toolbar shows the list as it is now.
    RefreshActionsToolbar,
    /// Delete a file to the recycle bin after `TaskHelpers.ConfirmFileDelete`
    /// (the notification's Delete file).
    DeleteFile(PathBuf),
    /// `TaskHelpers.AnnotateImageAsync(image, null, defaults)`: the image
    /// editor on an image that was never saved.
    AnnotateImage(RgbaImage),
    /// `ThemeManager.ApplyConfiguredSettings` after the settings' theme
    /// options changed (the icon pack prompt).
    ApplyAppearance,
    /// `SettingManager.SaveApplicationConfigAsync`.
    SaveSettings,
}

/// `HistoryWindowServices`: what the history windows hand a file to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileService {
    /// `UploadManager.UploadFile`.
    UploadFile,
    /// `AnnotateImageFromFile`.
    EditImage,
    /// `OpenPhotoEditor` (deferred).
    EditImageInPhotoEditor,
    /// `OpenVideoTrimmer`.
    TrimVideo,
    /// `OpenAnimatedGifTrimmer`.
    TrimAnimatedGif,
    /// `PinToScreen`.
    PinToScreen,
    /// `AnalyzeImage`.
    AnalyzeImage,
    /// `ImageViewerWindowIntegration.ShowImage`.
    ShowImage,
}

/// `UploadManager`'s entry points a dialog starts.
pub enum UploadRequest {
    /// `UploadText(text, taskSettings, allowCustomText)`.
    Text { text: String, task: TaskSettings, allow_custom_text: bool },
    /// `ProcessTextUpload`.
    ProcessText { text: String, task: TaskSettings },
    /// `DownloadAndUploadFile`.
    DownloadAndUpload { url: String, task: TaskSettings },
    /// `ShortenURL`.
    ShortenUrl { url: String, task: TaskSettings },
    /// `ProcessImageUpload`.
    ProcessImage { image: RgbaImage, task: TaskSettings },
    /// `UploadFile(files)`: asks first when there are more than 10, unless
    /// already `confirmed`.
    Files { files: Vec<PathBuf>, task: TaskSettings, confirmed: bool },
    /// `UploadImage(image)` of a generated QR code (or of the notification's
    /// image).
    QrCodeImage(RgbaImage),
}

impl UploadRequest {
    /// `ClipboardUploadWindow.UploadClipboardContent`.
    pub fn clipboard(content: ClipboardContent, task: TaskSettings) -> Self {
        match content {
            ClipboardContent::Image(image) => Self::ProcessImage { image, task },
            ClipboardContent::Text(text) => Self::ProcessText { text, task },
            ClipboardContent::Files(files) => Self::Files { files, task, confirmed: false },
        }
    }
}

/// What a dialog may read and change while it's drawn.
pub struct Cx<'a> {
    /// `ApplicationState.Settings`.
    pub settings: &'a mut ApplicationConfig,
    /// `ApplicationState.UploadersConfig`.
    #[allow(dead_code)]
    pub uploaders: &'a UploadersConfigStore,
}

/// One of ShareX's upload windows.
pub trait Dialog {
    fn spec(&self) -> Spec;

    /// The width of the content (`SizeToContent="WidthAndHeight"`), when
    /// it has one.
    fn content_width(&mut self, ui: &mut egui::Ui) -> Option<f32> {
        let _ = ui;
        None
    }

    /// The window moved (`PositionChanged`), to this outer position.
    fn moved(&mut self, _x: i32, _y: i32, _cx: &mut Cx) {}

    /// The window was resized (`Resized`), maximized or not.
    fn resized(&mut self, _maximized: bool, _cx: &mut Cx) {}

    /// The height of the content for [`Spec::size_to_content`].
    fn content_height(&mut self, ui: &mut egui::Ui) -> f32 {
        let _ = ui;
        self.spec().height
    }

    /// Draws the client area.
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Cx) -> Vec<Effect>;

    /// The window closed (`Closed`), whichever way: the last answer.
    fn closed(&mut self, cx: &mut Cx) -> Vec<Effect> {
        let _ = cx;
        Vec::new()
    }

    /// A confirmation the window asked for was answered Yes.
    fn confirmed(&mut self) {}

    /// A Yes/No question the window asked (`AskInvalidSignature`) was answered.
    fn answered(&mut self, _yes: bool) {}

    /// `ResponseWindow.ShowInstance` on the open response window.
    fn show_response(&mut self, _data: response::ResponseData) {}

    /// What a screen scan found (`ScanScreen`).
    fn scanned(&mut self, _results: Option<Vec<String>>) {}

    /// Files dragged over the window (`DragEnter`/`DragLeave`) and dropped.
    fn drag(&mut self, _hover: bool) {}
    fn dropped(&mut self, _files: Vec<PathBuf>, _cx: &mut Cx) -> Vec<Effect> {
        Vec::new()
    }

    /// The window's `Opacity` (premultiplied into the clear colour).
    fn opacity(&self) -> f32 {
        1.0
    }

    /// `DragDropUploadWindow.TaskSettings`: the drop window's tasks run
    /// with the settings it was last opened with.
    fn set_task(&mut self, _task: TaskSettings) {}

    /// Windows ShareX keeps one of (`ClipboardUploadWindowIntegration._window`):
    /// asking again brings that one to the front.
    fn single(&self) -> Option<&'static str> {
        None
    }

    /// The service whose page a window opened for shows
    /// (`DestinationSettingsIntegration.Show(config, service)`).
    fn service(&self) -> Option<&str> {
        None
    }

    /// `NavigateToService` on the open window.
    fn navigate(&mut self, _service: &str) {}

    /// The OCR window's Select region... captured this image (the Image
    /// effects window: pasted).
    fn replace_image(&mut self, _image: RgbaImage, _ctx: &egui::Context) {}

    /// `ImageEffectsWindow.ImportPreset(json)` on the open presets window.
    fn import(&mut self, _json: &str) {}

    /// The screen colour picker's colour after [`Effect::PickScreenColor`].
    fn screen_color(&mut self, _color: Option<sharex_tools::color::Color>, _cx: &mut Cx) {}

    /// A Pin to screen source gave nothing.
    fn pin_failed(&mut self, _source: crate::screen_tools::pin::Source) {}

    /// `Closing`: closes, unless the window cancels it (a scrolling capture
    /// stops first).
    fn close_requested(&mut self) -> Vec<Effect> {
        vec![Effect::Close]
    }

    /// `Activated`.
    fn activated(&mut self) -> Vec<Effect> {
        Vec::new()
    }

    /// The task settings the window was opened with (the video trimmer's
    /// `CheckFFmpeg`).
    fn task(&self) -> Option<&TaskSettings> {
        None
    }

    /// The video trimmer's export, once FFmpeg is resolved (`None`: missing).
    fn trimmer_export(&mut self, _ffmpeg: Option<std::path::PathBuf>) {}

    /// `Deactivated`.
    fn deactivated(&mut self) -> Vec<Effect> {
        Vec::new()
    }

    /// The scrolling capture window, for the app to report to.
    fn as_scrolling(&mut self) -> Option<&mut crate::scrolling_capture::window::ScrollingCaptureWindow> {
        None
    }

    /// The auto capture window, for the app to drive.
    fn as_auto_capture(&mut self) -> Option<&mut crate::auto_capture::window::AutoCaptureWindow> {
        None
    }

    /// A theme of its own whatever the app's (`RequestedThemeVariant =
    /// ThemeManager.ShareXDark`: the notification window).
    fn theme(&self) -> Option<egui::Theme> {
        None
    }

    /// Where the window goes once it's sized; centred on the screen when
    /// `None` (`WindowStartupLocation="CenterScreen"`).
    fn position(&self, work_area: sharex_capture::Rect, size: (u32, u32)) -> Option<(i32, i32)> {
        let _ = (work_area, size);
        None
    }
}

/// An open dialog.
pub struct DialogWindow {
    pub window: EguiWindow,
    pub dialog: Box<dyn Dialog>,
}

impl App {
    /// `TaskHelpers.PrintImage`.
    pub(crate) fn print_image(&mut self, event_loop: &ActiveEventLoop, image: RgbaImage) {
        let s = &self.settings.settings;
        if s.DontShowPrintSettingsDialog {
            print_window::print_in_background(image, crate::print::PrintSettings::from_json(&s.PrintSettings));
        } else {
            self.open_dialog_window(event_loop, Box::new(print_window::PrintWindow::new(image, false)));
        }
    }

    /// Opens `dialog` in a window of its own (`Window.Show()`).
    pub(crate) fn open_dialog_window(&mut self, event_loop: &ActiveEventLoop, dialog: Box<dyn Dialog>) {
        if let Some(key) = dialog.single()
            && let Some(open) = self.dialogs.iter_mut().find(|d| d.dialog.single() == Some(key))
        {
            if let Some(service) = dialog.service() {
                open.dialog.navigate(service);
                open.window.window.request_redraw();
            }
            open.window.window.set_minimized(false);
            open.window.window.focus_window();
            return;
        }
        if let Err(e) = self.try_open_dialog_window(event_loop, dialog) {
            eprintln!("couldn't open the window: {e}");
        }
    }

    fn try_open_dialog_window(&mut self, event_loop: &ActiveEventLoop, mut dialog: Box<dyn Dialog>) -> Result<(), String> {
        let theme = dialog.theme().unwrap_or(self.theme());
        let gpu = match &self.gpu {
            Some(g) => g,
            None => self.gpu.insert(sharex_ui::gpu::Gpu::new()?),
        };
        let spec = dialog.spec();
        let attrs = attributes(&spec, theme);
        let window = event_loop.create_window(attrs).map_err(|e| e.to_string())?;
        let mut w = EguiWindow::new(window, gpu, None)?;
        set_theme(&w, theme);
        if spec.borderless {
            w.clear_color = Some([0.0; 4]);
        }
        if spec.size_to_content {
            let (width, height) = w.paint(|ui| (dialog.content_width(ui), dialog.content_height(ui)));
            let _ = w.window.request_inner_size(LogicalSize::new(width.unwrap_or(spec.width), height));
        }
        clamp_to_max_track_size(&w.window, &spec);
        let size = w.window.outer_size();
        let screens = sharex_capture::screenshot::screens();
        let placed = screens
            .iter()
            .find(|s| s.primary)
            .or(screens.first())
            .and_then(|s| dialog.position(s.work_area, (size.width, size.height)));
        match placed {
            Some((x, y)) => w.window.set_outer_position(winit::dpi::PhysicalPosition::new(x, y)),
            None => center(&w.window, event_loop),
        }
        if spec.click_through {
            let _ = w.window.set_cursor_hittest(false);
        }
        w.window.set_visible(true);
        // Maximized once it's sized, placed and shown: a window created
        // maximized and then resized or moved (the clamp above, centring)
        // is restored by winit later, leaving it at its normal size.
        if spec.maximized {
            w.window.set_maximized(true);
        }
        if spec.activate {
            w.window.focus_window();
        }
        w.window.request_redraw();
        self.dialogs.push(DialogWindow { window: w, dialog });
        Ok(())
    }

    /// Routes an event to a dialog's window; false when it's none of theirs.
    pub(crate) fn dialog_window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: &WindowEvent) -> bool {
        let Some(i) = self.dialogs.iter().position(|d| d.window.window.id() == id) else { return false };
        let mut cx = Cx { settings: &mut self.settings.settings, uploaders: &self.uploads.config };
        let d = &mut self.dialogs[i];
        let effects = match event {
            WindowEvent::CloseRequested => d.dialog.close_requested(),
            WindowEvent::Focused(true) => {
                d.window.on_event(event);
                d.window.window.request_redraw();
                d.dialog.activated()
            }
            WindowEvent::Focused(false) => {
                d.window.on_event(event);
                d.dialog.deactivated()
            }
            WindowEvent::Moved(p) => {
                d.dialog.moved(p.x, p.y, &mut cx);
                Vec::new()
            }
            WindowEvent::Resized(_) => {
                d.dialog.resized(d.window.window.is_maximized(), &mut cx);
                d.window.on_event(event);
                Vec::new()
            }
            WindowEvent::RedrawRequested => {
                let dialog = &mut d.dialog;
                d.window.paint(|ui| {
                    ui.multiply_opacity(dialog.opacity());
                    dialog.ui(ui, &mut cx)
                })
            }
            WindowEvent::HoveredFile(_) => {
                d.dialog.drag(true);
                d.window.window.request_redraw();
                Vec::new()
            }
            WindowEvent::HoveredFileCancelled => {
                d.dialog.drag(false);
                d.window.window.request_redraw();
                Vec::new()
            }
            WindowEvent::DroppedFile(path) => {
                // winit gives one event per file; ShareX gets them together.
                d.dialog.drag(false);
                d.window.window.request_redraw();
                d.dialog.dropped(vec![path.clone()], &mut cx)
            }
            other => {
                d.window.on_event(other);
                Vec::new()
            }
        };
        self.dialog_effects(event_loop, id, effects);
        true
    }

    /// [`Effect::DragFiles`]: OLE's drag (it has the pointer until the
    /// drop), then the button's release egui never saw.
    fn dialog_drag_files(&mut self, id: WindowId, files: &[PathBuf], allow_move: bool) {
        let _ = self.drag_source.drag_files(files, allow_move, &mut |_| crate::drag_out::Cursor::Default);
        if let Some(d) = self.dialogs.iter_mut().find(|d| d.window.window.id() == id) {
            d.window.on_event(&WindowEvent::MouseInput {
                device_id: winit::event::DeviceId::dummy(),
                state: winit::event::ElementState::Released,
                button: winit::event::MouseButton::Left,
            });
            d.window.window.request_redraw();
        }
    }

    /// [`Effect::Tooltip`]: the owner's tooltip popup opened, moved or closed.
    fn dialog_tooltip(&mut self, event_loop: &ActiveEventLoop, owner: WindowId, tip: Option<(String, egui::Rect)>) {
        let theme = self.theme();
        let wanted = tip.and_then(|(text, r)| {
            let d = self.dialogs.iter().find(|d| d.window.window.id() == owner)?;
            let origin = d.window.window.inner_position().ok()?;
            let s = d.window.window.scale_factor() as f32;
            let anchor = sharex_capture::Rect::new(
                origin.x + (r.min.x * s).round() as i32,
                origin.y + (r.min.y * s).round() as i32,
                (r.width() * s).round() as i32,
                (r.height() * s).round() as i32,
            );
            let dark = d.dialog.theme().unwrap_or(theme) == egui::Theme::Dark;
            Some((text, anchor, s, dark))
        });
        if let (Some((o, _, text, anchor)), Some((t, a, ..))) = (&self.tooltip_popup, &wanted)
            && *o == owner
            && text == t
            && anchor == a
        {
            return;
        }
        if let Some((_, popup, ..)) = self.tooltip_popup.take()
            && let Some(i) = self.dialogs.iter().position(|d| d.window.window.id() == popup)
        {
            self.dialogs.remove(i);
        }
        if let Some((text, anchor, scale, dark)) = wanted {
            let popup = tooltip_window::TooltipWindow::new(text.clone(), anchor, scale, dark);
            self.open_dialog_window(event_loop, Box::new(popup));
            if let Some(d) = self.dialogs.last() {
                self.tooltip_popup = Some((owner, d.window.window.id(), text, anchor));
            }
        }
    }

    /// Carries out what the dialog `id` asked for.
    pub(crate) fn dialog_effects(&mut self, event_loop: &ActiveEventLoop, id: WindowId, effects: Vec<Effect>) {
        for e in effects {
            match e {
                Effect::Tooltip(tip) => self.dialog_tooltip(event_loop, id, tip),
                Effect::Close => {
                    if self.tooltip_popup.as_ref().is_some_and(|t| t.0 == id) {
                        self.dialog_tooltip(event_loop, id, None);
                    }
                    let Some(i) = self.dialogs.iter().position(|d| d.window.window.id() == id) else { continue };
                    let mut d = self.dialogs.remove(i);
                    let mut cx = Cx { settings: &mut self.settings.settings, uploaders: &self.uploads.config };
                    let last = d.dialog.closed(&mut cx);
                    drop(d);
                    self.scrolling_window_closed(id);
                    self.auto_capture_window_closed(id);
                    self.dialog_effects(event_loop, id, last);
                }
                Effect::Upload(request) => self.upload_request(event_loop, *request),
                Effect::CopyText(text) => {
                    if let Err(e) = sharex_capture::clipboard::copy_text(&text, self.clipboard_owner()) {
                        eprintln!("Clipboard copy text failed: {e}");
                    }
                }
                Effect::CopyImage(image) => {
                    if let Err(e) = sharex_capture::clipboard::copy_image(&image, self.clipboard_owner()) {
                        eprintln!("Clipboard copy image failed: {e}");
                    }
                }
                Effect::OpenUrl(url) => crate::upload::open_url(&url),
                Effect::OpenFile(path) => {
                    crate::shell::open_file(&path);
                }
                Effect::OpenFolderWithFile(path) => {
                    crate::shell::open_folder_with_file(&path);
                }
                Effect::DragWindow => {
                    if let Some(d) = self.dialogs.iter().find(|d| d.window.window.id() == id) {
                        let _ = d.window.window.drag_window();
                    }
                }
                Effect::DragFiles(files) => self.dialog_drag_files(id, &files, false),
                Effect::DragFilesMovable(files) => self.dialog_drag_files(id, &files, true),
                Effect::SetSizeCentered(w, h) => {
                    if let Some(d) = self.dialogs.iter().find(|d| d.window.window.id() == id) {
                        let win = &d.window.window;
                        win.set_maximized(false);
                        let _ = win.request_inner_size(LogicalSize::new(w, h));
                        center(win, event_loop);
                    }
                }
                Effect::Maximize => {
                    if let Some(d) = self.dialogs.iter().find(|d| d.window.window.id() == id) {
                        d.window.window.set_maximized(true);
                    }
                }
                Effect::SetHeight(h) => {
                    if let Some(d) = self.dialogs.iter().find(|d| d.window.window.id() == id) {
                        let w = d.window.window.inner_size().to_logical::<f32>(d.window.window.scale_factor()).width;
                        let _ = d.window.window.request_inner_size(LogicalSize::new(w, h));
                    }
                }
                Effect::ScanScreen => {
                    let Some(d) = self.dialogs.iter_mut().find(|d| d.window.window.id() == id) else { continue };
                    d.window.window.set_visible(false);
                    std::thread::sleep(std::time::Duration::from_millis(250));
                    let screens = sharex_capture::screenshot::screens();
                    let grab = sharex_capture::screenshot::Screenshot::default().capture_fullscreen(&screens);
                    d.window.window.set_visible(true);
                    d.window.window.focus_window();
                    d.dialog.scanned(grab.map(|g| qr_code::scan(&g)));
                    d.window.window.request_redraw();
                }
                Effect::SaveUploadersConfig => {
                    self.save_uploaders_config(false);
                    self.refresh_menus();
                }
                Effect::ShowResponse(data) => self.show_response(event_loop, *data),
                Effect::ConfirmClearCustomUploaders => {
                    let message = crate::message_box::MessageBox::yes_no(
                        crate::rs_title(sharex_strings::sharex::CustomUploaderSettingsWindow_ConfirmationTitle()),
                        sharex_strings::sharex::CustomUploaderSettingsWindow_ClearConfirmation(),
                        crate::message_box::MessageBoxIcon::Question,
                    );
                    if let Err(e) = self.open_dialog(event_loop, message, crate::DialogPurpose::ClearCustomUploaders) {
                        eprintln!("couldn't open the message box: {e}");
                    }
                }
                Effect::AskText(text, reply) => {
                    let task = sharex_settings::TaskSettings::default_task_settings(&self.settings.settings.DefaultTaskSettings);
                    let mut w = text_upload::TextUploadWindow::new(Some(text), task);
                    w.reply = Some(reply);
                    self.open_dialog_window(event_loop, Box::new(w));
                }
                Effect::SetTitle(title) => {
                    if let Some(d) = self.dialogs.iter().find(|d| d.window.window.id() == id) {
                        d.window.window.set_title(&title);
                    }
                }
                Effect::CopyFiles(files) => {
                    if let Err(e) = sharex_capture::clipboard::copy_files(&files, self.clipboard_owner()) {
                        eprintln!("Clipboard copy file failed: {e}");
                    }
                }
                Effect::CopyImageFile(path) => match crate::thumbnail::load(&path) {
                    Some(image) => {
                        if let Err(e) = sharex_capture::clipboard::copy_image(&image, self.clipboard_owner()) {
                            eprintln!("Clipboard copy image from file failed: {e}");
                        }
                    }
                    None => eprintln!("Clipboard copy image from file failed: {}", path.display()),
                },
                Effect::FileService(service, path) => self.file_service(event_loop, service, path),
                Effect::OcrSelectRegion => self.ocr_select_region(id),
                Effect::ScrollingSelect => self.scrolling_select(id),
                Effect::ScrollingStop => self.scrolling_stop(),
                Effect::ScrollingSound => self.scrolling_sound(),
                Effect::ScrollingOptionsSaved(o) => self.scrolling_options_saved(*o),
                Effect::RestoreAndActivate => self.restore_and_activate(id),
                Effect::AutoCaptureSelectRegion => self.auto_capture_select_region(event_loop),
                Effect::AutoCaptureTray(hide) => self.auto_capture_tray(event_loop, id, hide),
                Effect::NotPorted(what) => eprintln!("{what}: not ported yet"),
                Effect::OpenRemoteStorageBrowser => self.open_remote_storage_browser(event_loop),
                Effect::PickScreenColorOverWindow => self.open_screen_color_picker(event_loop, crate::screen_tools::PickPurpose::Dialog(id)),
                Effect::EditorCapture { fullscreen, task } => self.editor_capture(id, fullscreen, *task),
                Effect::PickScreenColor => {
                    if let Some(d) = self.dialogs.iter().find(|d| d.window.window.id() == id) {
                        d.window.window.set_minimized(true);
                    }
                    self.open_screen_color_picker(event_loop, crate::screen_tools::PickPurpose::Dialog(id));
                }
                Effect::HighlighterToggle(o) => self.highlighter_toggle(event_loop, *o),
                Effect::HighlighterOptions(o) => self.highlighter_options_changed(*o),
                Effect::PinFrom(source) => self.pin_from(event_loop, id, source),
                Effect::PinImage(image, task) => self.pin_image(event_loop, image, None, &task),
                Effect::PrintImage(image) => self.print_image(event_loop, image),

                Effect::PinOptionsSaved(pin, options) => self.pin_options_saved(pin, *options),
                Effect::RunImageTask(image, task) => self.run_image_task_for(image, *task),
                Effect::RunImageTaskAs(image, task, process) => self.run_image_task_with(image, *task, Some(process.to_owned())),
                Effect::OpenDialog(dialog) => self.open_dialog_window(event_loop, dialog),
                Effect::TrimmerExport => {
                    let task = self.dialogs.iter().find(|d| d.window.window.id() == id).and_then(|d| d.dialog.task().cloned());
                    if let Some(task) = task {
                        let ffmpeg = self.check_ffmpeg(event_loop, &task);
                        if let Some(d) = self.dialogs.iter_mut().find(|d| d.window.window.id() == id) {
                            d.dialog.trimmer_export(ffmpeg);
                            d.window.window.request_redraw();
                        }
                    }
                }
                Effect::SetFullscreen(on) => {
                    if let Some(d) = self.dialogs.iter().find(|d| d.window.window.id() == id) {
                        d.window.window.set_fullscreen(on.then_some(winit::window::Fullscreen::Borderless(None)));
                    }
                }
                Effect::MessageBox(message) => {
                    if let Err(e) = self.open_dialog(event_loop, *message, crate::DialogPurpose::Owned(id)) {
                        eprintln!("couldn't open the message box: {e}");
                    }
                }
                Effect::PasteImage => {
                    if let Some(image) = sharex_capture::clipboard::get_image()
                        && let Some(d) = self.dialogs.iter_mut().find(|d| d.window.window.id() == id)
                    {
                        let ctx = d.window.ctx().clone();
                        d.dialog.replace_image(image, &ctx);
                        d.window.window.request_redraw();
                    }
                }
                Effect::UpdateDownload(checker, origin) => self.download_update(event_loop, *checker, origin),
                Effect::UpdateFinished { result, origin, shutdown } => self.update_finished(event_loop, result, origin, shutdown),
                Effect::AskInvalidSignature => {
                    let (text, caption) = sharex_update::DownloaderWindow::signature_question();
                    let mut message = crate::message_box::MessageBox::yes_no(caption, text, crate::message_box::MessageBoxIcon::Warning);
                    // `MessageBoxDefaultButton.Button2`.
                    message.default_button = 1;
                    if let Err(e) = self.open_dialog(event_loop, message, crate::DialogPurpose::UpdateSignature) {
                        eprintln!("couldn't open the message box: {e}");
                    }
                }
                Effect::ApplyAppearance => self.apply_appearance(),
                Effect::DeleteFile(path) => self.confirm_delete_path(event_loop, path),
                Effect::SetInnerSize(w, h) => {
                    if let Some(d) = self.dialogs.iter().find(|d| d.window.window.id() == id) {
                        let _ = d.window.window.request_inner_size(LogicalSize::new(w, h));
                    }
                }
                Effect::ActionsToolbarMenu => self.open_actions_toolbar_menu(event_loop),
                Effect::RunJob(job) => {
                    let task = TaskSettings::default_task_settings(&self.settings.settings.DefaultTaskSettings);
                    self.run_job_now(event_loop, job, task);
                }
                Effect::RefreshActionsToolbar => {
                    if let Some(d) = self.dialogs.iter().find(|d| d.dialog.single() == Some("actions-toolbar")) {
                        d.window.window.request_redraw();
                    }
                }
                Effect::AfterCaptureAccepted(accepted) => self.after_capture_accepted(*accepted),
                Effect::ShowImage(image) => self.show_image(event_loop, image, None),
                Effect::ShowImageNamed(image, name) => self.show_image(event_loop, image, Some(name)),
                Effect::AnnotateImage(image) => {
                    let task = TaskSettings::default_task_settings(&self.settings.settings.DefaultTaskSettings);
                    self.open_image_editor(event_loop, Some(image), None, task, None);
                }
                Effect::SaveSettings => {
                    if let Err(e) = self.settings.save_application_config(false) {
                        eprintln!("couldn't save the settings: {e}");
                    }
                }
                Effect::MinimizeAndFlash => {
                    if let Some(d) = self.dialogs.iter().find(|d| d.window.window.id() == id) {
                        d.window.window.set_minimized(true);
                        d.window.window.request_user_attention(Some(winit::window::UserAttentionType::Informational));
                    }
                }
            }
        }
    }

    /// A history window's service on a file (`HistoryWindowServices`, as
    /// `TaskHelpers.OpenHistory` fills them in).
    pub(crate) fn file_service(&mut self, event_loop: &ActiveEventLoop, service: FileService, path: PathBuf) {
        match service {
            FileService::UploadFile => {
                let task = TaskSettings::default_task_settings(&self.settings.settings.DefaultTaskSettings);
                self.upload_request(event_loop, UploadRequest::Files { files: vec![path], task, confirmed: false });
            }
            FileService::EditImage => self.annotate_file(event_loop, &path, None),
            FileService::EditImageInPhotoEditor => eprintln!("Photo editor: deferred"),
            // `OpenVideoTrimmer(inputFilePath: filePath)`, `OpenAnimatedGifTrimmer(filePath)`.
            FileService::TrimVideo => {
                self.open_video_tool(event_loop, "MainMenuBuilder_VideoTrimmer", None, Some(path.to_string_lossy().into_owned()));
            }
            FileService::TrimAnimatedGif => {
                self.open_video_tool(event_loop, "MainMenuBuilder_AnimatedGifTrimmer", None, Some(path.to_string_lossy().into_owned()));
            }
            FileService::PinToScreen => self.pin_file(event_loop, &path, None),
            FileService::AnalyzeImage => eprintln!("Analyze image: not ported yet"),
            FileService::ShowImage => self.show_image_file(event_loop, &path),
        }
    }

    /// `ResponseWindow.ShowInstance(result)`: the open window shows it.
    pub(crate) fn show_response(&mut self, event_loop: &ActiveEventLoop, data: response::ResponseData) {
        if let Some(d) = self.dialogs.iter_mut().find(|d| d.dialog.single() == Some("response")) {
            d.dialog.show_response(data);
            d.window.window.set_minimized(false);
            d.window.window.focus_window();
            d.window.window.request_redraw();
        } else {
            self.open_dialog_window(event_loop, Box::new(response::ResponseWindow::new(data)));
        }
    }

    /// `CustomUploaderSettingsIntegration.Show`: one window, bound to the
    /// config once it's loaded.
    pub(crate) fn show_custom_uploader_settings(&mut self, event_loop: &ActiveEventLoop) {
        let w = self.uploads.config.with(|c| custom_uploaders::CustomUploaderSettingsWindow::new(c));
        self.open_dialog_window(event_loop, Box::new(w));
    }

    /// `TaskHelpers.OpenUploadersConfigWindow(service)`: one window, on
    /// the service's page when one is given.
    pub(crate) fn show_destination_settings(&mut self, event_loop: &ActiveEventLoop, service: Option<&str>) {
        self.open_dialog_window(event_loop, Box::new(destination_settings::DestinationSettingsWindow::new(service)));
    }

    /// `TaskHelpers.OpenRemoteStorageBrowser`: Amazon S3 and each usable
    /// FTP/SFTP account; without any, the message saying how to set one up.
    pub(crate) fn open_remote_storage_browser(&mut self, event_loop: &ActiveEventLoop) {
        let allow_upload = !self.settings.system.disable_upload;
        let providers = self.uploads.config.with(|c| sharex_uploaders::remote_storage::providers(c, allow_upload));
        let dialog: Box<dyn Dialog> = if providers.is_empty() {
            use crate::message_box::{MessageBox, MessageBoxIcon};
            let title = sharex_strings::sharex::MainMenuBuilder_RemoteStorageBrowser().trim_end_matches(['.', '…']);
            let text = sharex_strings::sharex::TaskHelpers_RemoteStorageNotConfigured();
            Box::new(message_dialog::MessageDialog::new(MessageBox::ok(format!("ShareXrs - {title}"), text, MessageBoxIcon::Information), None))
        } else {
            Box::new(crate::remote_storage_browser::RemoteStorageBrowserWindow::new(providers, std::sync::Arc::new(crate::upload::SystemBrowser)))
        };
        self.open_dialog_window(event_loop, dialog);
    }

    /// A window of ours to own the clipboard's data.
    pub(crate) fn clipboard_owner(&self) -> Option<isize> {
        self.main
            .as_ref()
            .or(self.dialogs.first().map(|d| &d.window))
            .map(|w| &*w.window)
            .or(self.screen_tool_window())
            .and_then(sharex_capture::platform::window_handle)
    }

    /// Repaints dialogs that asked to be; the next time one wants to be.
    pub(crate) fn dialogs_due(&mut self, now: std::time::Instant) -> Option<std::time::Instant> {
        let mut next: Option<std::time::Instant> = None;
        for d in &mut self.dialogs {
            match d.window.repaint_at {
                Some(at) if at <= now => {
                    d.window.window.request_redraw();
                    d.window.repaint_at = None;
                }
                Some(at) => next = Some(next.map_or(at, |n| n.min(at))),
                None => {}
            }
        }
        next
    }
}

/// The window's attributes from its spec.
fn attributes(spec: &Spec, theme: egui::Theme) -> WindowAttributes {
    let mut attrs = WindowAttributes::default()
        .with_title(&spec.title)
        .with_window_icon(Icon::from_rgba(app_icon::rgba64(), 64, 64).ok())
        .with_inner_size(LogicalSize::new(spec.width, spec.height))
        .with_resizable(spec.resizable)
        .with_theme(Some(match theme {
            egui::Theme::Dark => winit::window::Theme::Dark,
            egui::Theme::Light => winit::window::Theme::Light,
        }))
        .with_visible(false)
        .with_active(spec.activate);
    if spec.min_width > 0.0 || spec.min_height > 0.0 {
        attrs = attrs.with_min_inner_size(LogicalSize::new(spec.min_width, spec.min_height));
    }
    if spec.max_width > 0.0 || spec.max_height > 0.0 {
        let w = if spec.max_width > 0.0 { spec.max_width } else { f32::MAX };
        let h = if spec.max_height > 0.0 { spec.max_height } else { f32::MAX };
        attrs = attrs.with_max_inner_size(LogicalSize::new(w, h));
    }
    if !spec.maximizable {
        attrs = attrs.with_enabled_buttons(WindowButtons::CLOSE | WindowButtons::MINIMIZE);
    }
    if spec.topmost {
        attrs = attrs.with_window_level(WindowLevel::AlwaysOnTop);
    }
    if spec.borderless {
        attrs = attrs.with_decorations(false).with_transparent(true);
    }
    // `ShowInTaskbar="False"` (Windows; elsewhere every window shows).
    #[cfg(windows)]
    if spec.skip_taskbar {
        use winit::platform::windows::WindowAttributesExtWindows;
        attrs = attrs.with_skip_taskbar(true);
    }
    #[cfg(not(windows))]
    let _ = spec.skip_taskbar;
    attrs
}

/// Windows keeps a window within its maximum tracking size (the screen plus
/// the frame, `SM_CXMAXTRACK`/`SM_CYMAXTRACK`): Avalonia's windows that ask
/// for more than the screen open at that size (ShareX's 1180-wide tool
/// windows open 1028 wide on a 1024-pixel screen). winit's aren't held to
/// it, so the window is shrunk the same way (never below its `MinWidth` /
/// `MinHeight`, which win).
#[cfg(windows)]
fn clamp_to_max_track_size(window: &winit::window::Window, spec: &Spec) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXMAXTRACK, SM_CYMAXTRACK};
    let (max_w, max_h) = unsafe { (GetSystemMetrics(SM_CXMAXTRACK), GetSystemMetrics(SM_CYMAXTRACK)) };
    let outer = window.outer_size();
    let inner = window.inner_size();
    let over_w = (outer.width as i32 - max_w).max(0) as u32;
    let over_h = (outer.height as i32 - max_h).max(0) as u32;
    if over_w > 0 || over_h > 0 {
        let scale = window.scale_factor();
        let min_w = (spec.min_width as f64 * scale).round() as u32;
        let min_h = (spec.min_height as f64 * scale).round() as u32;
        let size = winit::dpi::PhysicalSize::new(inner.width.saturating_sub(over_w).max(min_w), inner.height.saturating_sub(over_h).max(min_h));
        let _ = window.request_inner_size(size);
    }
}

#[cfg(not(windows))]
fn clamp_to_max_track_size(_window: &winit::window::Window, _spec: &Spec) {}
