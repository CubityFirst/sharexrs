//! The image editor window: ShareX.ImageEditor's `EditorWindow` (an
//! `EditorView` with `UseBuiltInToolbars`), opened by
//! `ImageEditorIntegration.ShowEditorDialog` from Tools › Image editor,
//! the after capture "Annotate image" task, the task list's and history's
//! Edit image and `-ImageEditor`.
//!
//! The workspace is `sharex_image_editor::editor::Workspace` (shared with
//! region capture), drawn at the zoom in the canvas host (`Margin="8"`),
//! with the editor's overlay (`sharex_capture::editor_chrome`). Over it
//! float `EditorBuiltInToolbars`: the `AnnotationToolbar` at the top (the
//! File menu, the tools, Crop, Cut out, Canvas size, Background, Image
//! effects; `ShowEditingActions="False"`) with its tool options row and
//! popups (`sharex_capture::editor_popups`), and the bottom toolbar (zoom,
//! the task buttons, copy, save, save as, pin, print, upload, new from a
//! screenshot). The context menu is `EditorView.EditorContextMenu`.

use std::path::PathBuf;
use std::sync::mpsc;

use egui::{Color32, CornerRadius, Id, Pos2, Rect, Sense, Stroke, StrokeKind, TextureHandle, Ui, Vec2, pos2, vec2};
use image::RgbaImage;
use sharex_capture::editor_chrome::{self, Mapper};
use sharex_capture::editor_popups::Popups;
use sharex_capture::region::options_row::{self, OptionItem};
use sharex_capture::region::toolbar as captb;
use sharex_image_editor::editor::{self as ed, Command, RightClick, Visible, Workspace};
use sharex_image_editor::model::EditorTool;
use sharex_image_editor::options::ImageEditorOptions;
use sharex_settings::TaskSettings;
use sharex_skia::Pixmap;
use sharex_strings::image_editor as ie;
use sharex_ui::keys::{app_icons, image_editor_icons as tool_icons};
use sharex_ui::menu::{Flyout, MenuEntry, Placement};
use sharex_ui::{Palette, icons, palette, widgets};

use crate::dialogs::{Cx, Dialog, Effect, Spec, UploadRequest};

mod canvas_size;
mod emoji_picker;
mod insert_image;
mod modal;
mod new_image;
mod notify;
mod start_screen;

use sharex_ui::keys::app_icons as icon;

/// The dialog in `EditorView`'s modal host (`ModalContent`).
enum Modal {
    /// `ConfirmationDialogView` (unsaved changes on exit).
    Confirm,
    Start(start_screen::StartScreen),
    NewImage(Box<new_image::NewImage>),
    InsertImage(Box<insert_image::InsertImage>),
    Emoji(Box<emoji_picker::EmojiPicker>),
}

/// `Button.toolbar-button` and the other toolbar metrics (see
/// `sharex_capture::region::toolbar`).
const BUTTON: f32 = 40.0;
const SPACING: f32 = 4.0;
const PADDING: f32 = 4.0;
/// The File menu button's `MinWidth`.
const FILE_BUTTON: f32 = 54.0;

/// An item of the editor's toolbar (`ToolbarCustomizationItemViewModel`'s
/// default items).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Item {
    File,
    Tool(usize),
    Crop,
    CutOut,
    CanvasSize,
    Background,
    ImageEffects,
}

impl Item {
    fn all() -> Vec<Item> {
        let mut v = vec![Item::File];
        v.extend((0..captb::TOOLS.len()).map(Item::Tool));
        v.extend([Item::Crop, Item::CutOut, Item::CanvasSize, Item::Background, Item::ImageEffects]);
        v
    }

    fn tool(self) -> Option<EditorTool> {
        match self {
            Item::Tool(i) => Some(captb::TOOLS[i].tool.clone()),
            Item::Crop => Some(EditorTool::Crop),
            Item::CutOut => Some(EditorTool::CutOut),
            _ => None,
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Item::File => app_icons::File,
            Item::Tool(i) => captb::TOOLS[i].icon,
            Item::Crop => tool_icons::Crop,
            Item::CutOut => tool_icons::CutOut,
            Item::CanvasSize => app_icons::Expand,
            Item::Background => app_icons::Wallpaper,
            Item::ImageEffects => app_icons::Sparkles,
        }
    }

    /// `BeginGroupByDefault`.
    fn begin_group(self) -> bool {
        matches!(self, Item::Crop) || matches!(self, Item::Tool(i) if captb::TOOLS[i].begin_group)
    }

    /// The item's hotkeys (`DefaultHotkey`, `DefaultSecondaryHotkey`).
    fn hotkeys(self) -> [&'static str; 2] {
        match self {
            Item::Tool(i) => [captb::TOOLS[i].hotkey, captb::TOOLS[i].secondary_hotkey],
            Item::Crop => ["C", ""],
            Item::CutOut => ["U", ""],
            Item::CanvasSize => ["X", ""],
            _ => ["", ""],
        }
    }

    fn width(self) -> f32 {
        if self == Item::File { FILE_BUTTON } else { BUTTON }
    }
}

/// The bottom toolbar's buttons.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Bottom {
    Zoom,
    Continue,
    Cancel,
    Copy,
    Save,
    SaveAs,
    Pin,
    Print,
    Upload,
    CaptureRegion,
    CaptureFullscreen,
}

impl Bottom {
    fn icon(self) -> &'static str {
        match self {
            Bottom::Zoom => app_icons::ZoomIn,
            Bottom::Continue => app_icons::Play,
            Bottom::Cancel => app_icons::Close,
            Bottom::Copy => app_icons::Clipboard,
            Bottom::Save => app_icons::Save,
            Bottom::SaveAs => app_icons::SaveAs,
            Bottom::Pin => app_icons::Pin,
            Bottom::Print => app_icons::Printer,
            Bottom::Upload => app_icons::CloudUpload,
            Bottom::CaptureRegion => app_icons::Region,
            Bottom::CaptureFullscreen => app_icons::Fullscreen,
        }
    }
}

/// The context menu's and File menu's commands.
#[derive(Clone, Copy, Debug, PartialEq)]
enum MenuCmd {
    New,
    Open,
    NewRegion,
    NewFullscreen,
    Save,
    SaveAs,
    Exit,
    Undo,
    Redo,
    Delete,
    DeleteAll,
    Flatten,
    Cut,
    Copy,
    Paste,
    Duplicate,
    BringToFront,
    BringForward,
    SendBackward,
    SendToBack,
    ToggleToolbars,
    Zoom(u8),
    ZoomFit,
    /// `OpenRecentImageCommand` with the recent file at this index.
    OpenRecent(u8),
}

/// `EditorZoomController.ZoomLevels` (`ZoomPickerDropdown`'s too).
const ZOOM_LEVELS: [f64; 9] = [0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 2.0, 3.0, 4.0];
const MIN_ZOOM: f64 = 0.25;
const MAX_ZOOM: f64 = 4.0;

/// The image editor window (`EditorWindow` with its `MainViewModel`).
pub struct EditorWindow {
    ws: Workspace,
    /// `HasPreviewImage`.
    has_image: bool,
    task: TaskSettings,
    /// `UseContinueWorkflow` (`taskMode`): Continue hands the image back.
    task_mode: bool,
    /// `ImageFilePath`.
    file_path: Option<PathBuf>,
    /// `GetScreenshotsFolder(taskSettings)`, for Save without a file.
    folder: PathBuf,
    /// The waiting task's answer (`ShowEditorDialog` returns).
    pub reply: Option<mpsc::Sender<Option<RgbaImage>>>,
    /// `TaskResult`: Continue's image.
    result: Option<RgbaImage>,
    /// `EffectiveZoom`.
    zoom: f64,
    zoom_to_fit_pending: bool,
    /// The scroll offset when the image is larger than the viewport.
    scroll: Vec2,
    tex: Option<(u64, TextureHandle)>,
    popups: Popups,
    flyout: Option<Flyout<MenuCmd>>,
    zoom_open: bool,
    /// `AreToolbarsHidden`.
    toolbars_hidden: bool,
    /// The top toolbar's horizontal scroll (its `ScrollViewer`).
    top_scroll: f32,
    effects: Vec<Effect>,
    title: String,
    /// A canvas press is being dragged (the pointer captured).
    capturing: bool,
    /// The middle button pans (`EditorZoomController`).
    pan: Option<Pos2>,
    /// The last press (time, place), for the Canvas size tool's double click.
    last_click: Option<(f64, Pos2)>,
    /// `ModalContent` (`IsModalOpen` when set).
    modal: Option<Modal>,
    /// The editor's notifications.
    notify: notify::Notifications,
    /// The start screen's URL download (`OnLoadFromUrlRequested`).
    url_job: Option<mpsc::Receiver<Result<RgbaImage, String>>>,
    /// Where things were drawn last frame.
    layout: Layout,
    /// Open the File menu once the toolbar is laid out (`OpenFileMenu`).
    open_file_menu: bool,
    /// The window's client area on the desktop (points) and its scale.
    window_inner: Option<(Rect, f32)>,
    /// The Canvas size tool, while open (`IsCanvasSizeModeActive`).
    canvas_resize: Option<canvas_size::CanvasResize>,
    /// The confirmation's warning sign, drawn once.
    sign_texture: Option<Option<TextureHandle>>,
    /// `--render`: the Canvas size tool opened on the first frame.
    canvas_resize_on_open: bool,
    /// The room round the image in the scroll viewer (the scroll viewer's
    /// content `Margin`, the Canvas size tool's).
    room: Vec2,
}

struct Layout {
    top: Option<Rect>,
    items: Vec<(Item, Rect)>,
    options: Option<options_row::OptionsLayout>,
    bottom: Option<Rect>,
    bottom_items: Vec<(Bottom, Rect)>,
    canvas: Rect,
    image: Rect,
    crop_button: Option<Rect>,
    /// The whole view (`EditorView`'s bounds).
    view: Rect,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            top: None,
            items: Vec::new(),
            options: None,
            bottom: None,
            bottom_items: Vec::new(),
            canvas: Rect::NOTHING,
            image: Rect::NOTHING,
            crop_button: None,
            view: Rect::NOTHING,
        }
    }
}

/// A straight RGBA image as a premultiplied pixmap.
pub fn pixmap_of(image: &RgbaImage) -> Pixmap {
    Pixmap::from_rgba(image.width(), image.height(), image.as_raw())
}

/// A premultiplied pixmap as straight RGBA (`SKBitmap.Copy` to a
/// `Bitmap`).
pub fn rgba_of(pm: &Pixmap) -> RgbaImage {
    let mut out = RgbaImage::new(pm.width(), pm.height());
    for (px, c) in out.pixels_mut().zip(pm.pixels()) {
        let a = c >> 24;
        let un = |v: u32| (v * 255 + a / 2).checked_div(a).map_or(0, |x| x.min(255) as u8);
        *px = image::Rgba([un((c >> 16) & 0xFF), un((c >> 8) & 0xFF), un(c & 0xFF), a as u8]);
    }
    out
}

impl EditorWindow {
    /// `new EditorWindow(options)` with `LoadImage(bitmap)`: the task's
    /// `ImageEditorOptions`, `ShowTaskButtons`, `UseContinueWorkflow =
    /// taskMode`.
    pub fn new(image: Option<RgbaImage>, file_path: Option<PathBuf>, task: TaskSettings, task_mode: bool, folder: PathBuf) -> Self {
        let options = crate::capture_job::image_editor_options(&task);
        let has_image = image.is_some();
        let source = image.as_ref().map(pixmap_of).unwrap_or_else(|| Pixmap::new(1, 1));
        let mut ws = Workspace::new(source, options.clone());
        ws.task_buttons = true;
        ws.continue_workflow = task_mode;
        let mut w = Self {
            ws,
            has_image,
            task,
            task_mode,
            file_path,
            folder,
            reply: None,
            result: None,
            zoom: 1.0,
            zoom_to_fit_pending: options.ZoomToFitOnOpen,
            scroll: Vec2::ZERO,
            tex: None,
            popups: Popups::default(),
            flyout: None,
            zoom_open: false,
            toolbars_hidden: false,
            top_scroll: 0.0,
            effects: Vec::new(),
            title: String::new(),
            capturing: false,
            pan: None,
            last_click: None,
            modal: None,
            notify: notify::Notifications::default(),
            url_job: None,
            layout: Layout::default(),
            open_file_menu: false,
            window_inner: None,
            canvas_resize: None,
            canvas_resize_on_open: false,
            sign_texture: None,
            room: Vec2::ZERO,
        };
        w.title = w.window_title();
        // `OnLoaded`: no image (and not a task) shows the start screen.
        if !has_image && !task_mode {
            w.modal = Some(Modal::Start(start_screen::StartScreen::new(w.ws.tools.options.RecentImageFiles.clone())));
        }
        w
    }

    fn notifications_enabled(&self) -> bool {
        self.ws.tools.options.ShowNotifications
    }

    /// `ShowNotification`.
    fn notify(&mut self, message: impl Into<String>, icon: &'static str) {
        let enabled = self.notifications_enabled();
        self.notify.show(enabled, message, Some(icon));
    }

    /// `ShowTaskActionNotification`: none when the editor closes after it.
    fn notify_task(&mut self, message: impl Into<String>, icon: &'static str) {
        if !self.ws.tools.options.AutoCloseEditorOnTask {
            self.notify(message, icon);
        }
    }

    /// `BuildFilePathNotification`.
    fn file_message(headline: &str, path: &std::path::Path) -> String {
        format!("{headline}\n{}", ie::MainViewModel_FilePathFormat().replace("{0}", &path.display().to_string()))
    }

    /// `BuildImageSizeNotification`.
    fn size_message(headline: &str, w: u32, h: u32) -> String {
        if w > 0 && h > 0 {
            format!("{headline}\n{}", ie::MainViewModel_SizeFormat().replace("{0}", &w.to_string()).replace("{1}", &h.to_string()))
        } else {
            headline.to_string()
        }
    }

    /// `CloseAfterTaskActionIfEnabled`.
    fn close_after_task_action(&mut self) {
        if self.ws.tools.options.AutoCloseEditorOnTask {
            self.result = None;
            self.effects.push(Effect::Close);
        }
    }

    /// `MainViewModel.AddRecentImageFile` (`ImageEditorOptions.AddRecentImageFile`).
    fn add_recent(&mut self, path: &std::path::Path) {
        let path = path.display().to_string();
        let o = &mut self.ws.tools.options;
        o.RecentImageFiles.retain(|p| *p != path);
        o.RecentImageFiles.insert(0, path);
        let max = o.MaxRecentImageFiles.max(0) as usize;
        o.RecentImageFiles.truncate(max);
    }

    fn remove_recent(&mut self, path: &str) {
        self.ws.tools.options.RecentImageFiles.retain(|p| p != path);
        if let Some(Modal::Start(s)) = &mut self.modal {
            s.recent.retain(|p| p != path);
        }
    }

    /// `ShowStartScreenStatus`: the start screen (opened again if need
    /// be) with `message`.
    fn start_status(&mut self, message: String) {
        if !matches!(self.modal, Some(Modal::Start(_))) {
            self.modal = Some(Modal::Start(start_screen::StartScreen::new(self.ws.tools.options.RecentImageFiles.clone())));
        }
        if let Some(Modal::Start(s)) = &mut self.modal {
            s.url_loading = false;
            s.status = Some(message);
        }
    }

    /// `LoadBitmapIntoEditor`: a new image, its file (in the recent files).
    fn load_bitmap(&mut self, img: RgbaImage, path: Option<PathBuf>) {
        self.load(img);
        self.file_path = path.clone();
        self.ws.set_clean();
        if let Some(p) = &path {
            self.add_recent(p);
        }
        self.update_title();
    }

    /// `BuildWindowTitle`: "ShareXrs - Image Editor - 640x400 - name.png".
    fn window_title(&self) -> String {
        let mut t = crate::rs_title(ie::MainViewModel_WindowTitle());
        if self.has_image {
            let (w, h) = self.ws.size();
            t.push_str(&format!(" - {w}x{h}"));
        }
        if let Some(name) = self.file_path.as_ref().and_then(|p| p.file_name()).and_then(|n| n.to_str()) {
            t.push_str(&format!(" - {name}"));
        }
        t
    }

    fn update_title(&mut self) {
        let t = self.window_title();
        if t != self.title {
            self.title = t.clone();
            self.effects.push(Effect::SetTitle(t));
        }
    }

    /// `GetSnapshot()`: the image with its annotations.
    fn snapshot(&mut self) -> RgbaImage {
        rgba_of(&self.ws.export())
    }

    // Commands.

    fn run(&mut self, c: Command) {
        match c {
            Command::CopyImage => {
                if !self.has_image {
                    return;
                }
                let img = self.snapshot();
                self.effects.push(Effect::CopyImage(img));
                self.notify_task(ie::MainViewModel_ImageCopiedToClipboard(), icon::Clipboard);
                self.close_after_task_action();
            }
            Command::Paste => {
                // The system clipboard first (an image or image files),
                // then the copied annotation.
                if let Some(img) = sharex_capture::clipboard::get_image() {
                    let c = self.visible_center();
                    self.ws.insert_image_centered(pixmap_of(&img), c);
                } else {
                    self.ws.paste_annotation();
                }
            }
            Command::Save => self.save(false),
            Command::SaveAs => self.save(true),
            Command::NewImage => self.modal = Some(Modal::NewImage(Box::default())),
            Command::OpenImage => self.open_image(),
            Command::PinToScreen => {
                if !self.has_image {
                    return;
                }
                let img = self.snapshot();
                self.effects.push(Effect::PinImage(img, Box::new(self.task.clone())));
                self.notify_task(ie::MainViewModel_ImagePinnedToScreen(), icon::Pin);
                self.close_after_task_action();
            }
            Command::Upload => {
                if !self.has_image {
                    return;
                }
                let img = self.snapshot();
                self.effects.push(Effect::Upload(Box::new(UploadRequest::ProcessImage { image: img, task: self.task.clone() })));
                self.notify_task(ie::MainViewModel_ImageIsUploading(), icon::CloudUpload);
                self.close_after_task_action();
            }
            Command::Print => self.effects.push(Effect::NotPorted("Image editor: Print")),
            Command::ToggleToolbars => self.toolbars_hidden = !self.toolbars_hidden,
            Command::Continue => self.finish(true),
            Command::Exit => {
                if self.task_mode {
                    self.finish(false);
                } else {
                    self.request_close(false);
                }
            }
        }
    }

    /// `RequestClose(ignoreModal)`: nothing while a dialog is open (unless
    /// the window is closing: the dialog goes), the confirmation when there
    /// are unsaved changes.
    fn request_close(&mut self, ignore_modal: bool) {
        if self.modal.is_some() {
            if !ignore_modal {
                return;
            }
            self.modal = None;
        }
        if self.ws.tools.options.ShowExitConfirmation && self.ws.is_dirty() && self.has_image {
            self.modal = Some(Modal::Confirm);
        } else {
            self.finish(false);
        }
    }

    /// `ContinueCommand` / `CancelCommand`: closes, with the image or not.
    fn finish(&mut self, continued: bool) {
        self.ws.commit_text();
        if continued && self.has_image {
            self.result = Some(self.snapshot());
        }
        self.effects.push(Effect::Close);
    }

    /// `SaveImageRequested` / `SaveImageAsRequested`: the image file (PNG
    /// or the task's format), or a new file in the screenshots folder.
    fn save(&mut self, ask: bool) {
        if !self.has_image {
            return;
        }
        let img = self.snapshot();
        let mut path = self.file_path.clone();
        if path.is_none() {
            let meta = crate::capture_job::Metadata { image: img.clone(), window_title: None, process_name: None };
            let mut counter = 0;
            let ext = image_extension(&self.task);
            let name = crate::capture_job::file_name(&self.task, &mut counter, ext, &meta);
            path = Some(self.folder.join(name));
        }
        if ask {
            let mut d = rfd::FileDialog::new().add_filter("PNG", &["png"]).add_filter("JPEG", &["jpg", "jpeg"]).add_filter("BMP", &["bmp"]);
            if let Some(p) = &path {
                if let Some(dir) = p.parent() {
                    d = d.set_directory(dir);
                }
                if let Some(n) = p.file_name().and_then(|n| n.to_str()) {
                    d = d.set_file_name(n);
                }
            }
            match d.save_file() {
                Some(p) => path = Some(p),
                None => return,
            }
        }
        let Some(path) = path else { return };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        match img.save(&path) {
            Ok(()) => {
                eprintln!("Image editor: saved {}", path.display());
                let message = Self::file_message(ie::MainViewModel_ImageSavedToFile(), &path);
                self.file_path = Some(path);
                self.ws.set_clean();
                self.update_title();
                self.notify_task(message, if ask { icon::SaveAs } else { icon::Save });
                self.close_after_task_action();
            }
            Err(e) => eprintln!("Image editor: couldn't save {}: {e}", path.display()),
        }
    }

    /// `OpenImageCommand`: an image file replaces the image.
    fn open_image(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title(ie::EditorView_OpenImage())
            .add_filter("Images", &["png", "jpg", "jpeg", "bmp", "gif", "webp", "ico", "tif", "tiff"])
            .pick_file()
        else {
            return;
        };
        match crate::thumbnail::load(&path) {
            Some(img) => {
                self.load_bitmap(img, Some(path.clone()));
                // `ShowOpenImageNotification`.
                self.notify(Self::file_message(ie::MainViewModel_ImageOpened(), &path), icon::FolderOpen);
            }
            None => eprintln!("Image editor: couldn't load {}", path.display()),
        }
    }

    /// `UpdatePreview`: a new image, no annotations, a new history.
    fn load(&mut self, img: RgbaImage) {
        self.image_changed_elsewhere();
        self.ws.load_image(pixmap_of(&img));
        self.has_image = true;
        self.tex = None;
        self.scroll = Vec2::ZERO;
        self.zoom_to_fit_pending = self.ws.tools.options.ZoomToFitOnOpen;
        self.update_title();
    }

    fn menu_command(&mut self, c: MenuCmd) {
        match c {
            MenuCmd::New => self.run(Command::NewImage),
            MenuCmd::Open => self.run(Command::OpenImage),
            MenuCmd::NewRegion | MenuCmd::NewFullscreen => self.effects.push(Effect::NotPorted("Image editor: New from screenshot")),
            MenuCmd::Save => self.run(Command::Save),
            MenuCmd::SaveAs => self.run(Command::SaveAs),
            MenuCmd::Exit => self.run(Command::Exit),
            MenuCmd::Undo => {
                self.image_changed_elsewhere();
                self.ws.undo();
            }
            MenuCmd::Redo => {
                self.image_changed_elsewhere();
                self.ws.redo();
            }
            MenuCmd::Delete => self.ws.delete_selected(),
            MenuCmd::DeleteAll => self.ws.clear_annotations(),
            MenuCmd::Flatten => {
                self.ws.flatten();
            }
            MenuCmd::Cut => {
                self.ws.cut_annotation();
            }
            MenuCmd::Copy => {
                self.ws.copy_annotation();
            }
            MenuCmd::Paste => self.run(Command::Paste),
            MenuCmd::Duplicate => {
                self.ws.duplicate_selected();
            }
            MenuCmd::BringToFront => self.ws.bring_to_front(),
            MenuCmd::BringForward => self.ws.bring_forward(),
            MenuCmd::SendBackward => self.ws.send_backward(),
            MenuCmd::SendToBack => self.ws.send_to_back(),
            MenuCmd::ToggleToolbars => self.toolbars_hidden = !self.toolbars_hidden,
            MenuCmd::Zoom(i) => self.set_zoom(ZOOM_LEVELS[i as usize]),
            MenuCmd::ZoomFit => self.zoom_to_fit(),
            MenuCmd::OpenRecent(i) => {
                if let Some(path) = self.ws.tools.options.RecentImageFiles.get(i as usize).cloned() {
                    self.load_recent(&path);
                }
            }
        }
    }

    fn set_zoom(&mut self, z: f64) {
        self.zoom = z.clamp(MIN_ZOOM, MAX_ZOOM);
    }

    /// `ZoomToFit`: the image within the viewport, at most 100%... up to
    /// `MaxZoom`, as ShareX fits it.
    fn zoom_to_fit(&mut self) {
        let (w, h) = self.ws.size();
        let vp = self.layout.canvas;
        if vp.width() <= 0.0 || w == 0 || h == 0 {
            return;
        }
        let fit = (vp.width() as f64 / w as f64).min(vp.height() as f64 / h as f64);
        self.zoom = fit.clamp(MIN_ZOOM, MAX_ZOOM);
        self.scroll = Vec2::ZERO;
    }

    /// The zoom step for the wheel (`EditorZoomController`): the next level.
    fn zoom_step(&mut self, up: bool) {
        let i = ZOOM_LEVELS.iter().enumerate().min_by(|a, b| (a.1 - self.zoom).abs().total_cmp(&(b.1 - self.zoom).abs())).map(|(i, _)| i).unwrap_or(3);
        let n = if up { (i + 1).min(ZOOM_LEVELS.len() - 1) } else { i.saturating_sub(1) };
        self.zoom = ZOOM_LEVELS[n];
    }

    /// `GetVisibleCanvasCenter` in image pixels.
    fn visible_center(&self) -> (f64, f64) {
        let (w, h) = self.ws.size();
        let m = self.mapper_ppp(1.0);
        let c = m.image_pos(self.layout.canvas.center());
        (c.0.clamp(0.0, w as f64), c.1.clamp(0.0, h as f64))
    }

    fn mapper_ppp(&self, ppp: f32) -> Mapper {
        Mapper { origin: (0.0, 0.0), ppp, scale: self.zoom * ppp as f64, offset: self.layout.image.min }
    }

    /// A toolbar item that isn't a tool whose hotkey this is.
    fn hotkey_item(key: ed::Key, m: ed::Mods) -> Option<Item> {
        let name = match key {
            ed::Key::Letter(c) => c.to_string(),
            _ => return None,
        };
        Item::all().into_iter().filter(|it| it.tool().is_none()).find(|it| {
            it.hotkeys().iter().any(|h| !h.is_empty() && h.eq_ignore_ascii_case(&name) && m == ed::Mods::NONE)
        })
    }

    /// `TrySelectToolForToolbarHotkey`.
    fn hotkey_tool(key: ed::Key, m: ed::Mods) -> Option<EditorTool> {
        let name = match key {
            ed::Key::Letter(c) => c.to_string(),
            ed::Key::Digit(d) => format!("D{d}"),
            _ => return None,
        };
        Item::all().into_iter().filter_map(|it| it.tool().map(|t| (it, t))).find_map(|(it, t)| {
            it.hotkeys()
                .iter()
                .any(|h| {
                    let mut parts: Vec<&str> = h.split('+').map(str::trim).filter(|s| !s.is_empty()).collect();
                    let Some(k) = parts.pop() else { return false };
                    let has = |x: &str| parts.iter().any(|p| p.eq_ignore_ascii_case(x));
                    k.eq_ignore_ascii_case(&name) && has("Ctrl") == m.ctrl && has("Shift") == m.shift && has("Alt") == m.alt
                })
                .then_some(t)
        })
    }

    fn select_item(&mut self, item: Item) {
        match item {
            Item::File => {}
            Item::CanvasSize => self.toggle_canvas_resize(),
            Item::Background => self.effects.push(Effect::NotPorted("Image editor: Background panel")),
            Item::ImageEffects => self.effects.push(Effect::NotPorted("Image editor: Image effects panel")),
            _ => {
                if let Some(t) = item.tool() {
                    self.popups.close();
                    self.ws.select_tool(t);
                }
            }
        }
        self.handle_requests();
    }

    /// The workspace's dialog and picker requests.
    fn handle_requests(&mut self) {
        for r in self.ws.take_requests() {
            match r {
                ed::Request::InsertImage | ed::Request::PickImage { .. } | ed::Request::ReplaceImage { .. } => {
                    // `RequestImageInsertion`: nothing while a dialog is open.
                    if self.modal.is_some() {
                        continue;
                    }
                    let Some(path) = rfd::FileDialog::new()
                        .set_title(ie::EditorView_SelectImage())
                        .add_filter("Images", &["png", "jpg", "jpeg", "bmp", "gif", "webp", "ico"])
                        .pick_file()
                    else {
                        continue;
                    };
                    let Some(img) = crate::thumbnail::load(&path) else { continue };
                    match r {
                        ed::Request::PickImage { at } => {
                            self.ws.insert_picked_image(pixmap_of(&img), at);
                        }
                        ed::Request::ReplaceImage { id } => self.ws.replace_image(&id, pixmap_of(&img)),
                        _ => self.insert_external_image(img, path),
                    }
                }
                ed::Request::EmojiPicker | ed::Request::ReplaceEmoji { .. } => {
                    // `ShowEmojiPickerDialog`: nothing while a dialog is open.
                    if self.modal.is_some() {
                        continue;
                    }
                    let replace = match r {
                        ed::Request::ReplaceEmoji { id } => Some(id),
                        _ => None,
                    };
                    self.modal = Some(Modal::Emoji(Box::new(emoji_picker::EmojiPicker::new(replace))));
                }
                ed::Request::Cropped { width, height } => {
                    self.notify(Self::size_message(ie::MainViewModel_ImageCropped(), width, height), tool_icons::Crop);
                }
                ed::Request::CutOut { width, height } => {
                    self.notify(Self::size_message(ie::MainViewModel_ImageCutOut(), width, height), tool_icons::CutOut);
                }
            }
        }
    }

    fn context_menu(&self) -> Vec<MenuEntry<MenuCmd>> {
        let sel = self.ws.selected().is_some();
        let any = !self.ws.annotations().is_empty();
        let toggle = if self.toolbars_hidden { ie::MainViewModel_ShowToolbars() } else { ie::MainViewModel_HideToolbars() };
        vec![
            MenuEntry::item(ie::EditorView_Undo(), app_icons::Undo, MenuCmd::Undo).gesture("Ctrl+Z").enabled(self.ws.can_undo()),
            MenuEntry::item(ie::EditorView_Redo(), app_icons::Redo, MenuCmd::Redo).gesture("Ctrl+Y").enabled(self.ws.can_redo()),
            MenuEntry::separator(),
            MenuEntry::item(ie::EditorView_Delete(), app_icons::Trash, MenuCmd::Delete).gesture("Delete").enabled(sel),
            MenuEntry::item(ie::EditorView_Deleteall(), app_icons::ClearAll, MenuCmd::DeleteAll).gesture("Shift+Delete").enabled(any),
            MenuEntry::item(ie::EditorView_Flatten(), app_icons::Layers, MenuCmd::Flatten).gesture("Ctrl+Shift+F").enabled(any),
            MenuEntry::separator(),
            MenuEntry::item(ie::EditorView_Cut(), app_icons::Scissors, MenuCmd::Cut).gesture("Ctrl+X").enabled(sel),
            MenuEntry::item(ie::EditorView_Copy(), app_icons::Clipboard, MenuCmd::Copy).gesture("Ctrl+Shift+C").enabled(sel),
            MenuEntry::item(ie::EditorView_Paste(), app_icons::ClipboardPaste, MenuCmd::Paste).gesture("Ctrl+V"),
            MenuEntry::item(ie::EditorView_Duplicate(), app_icons::Duplicate, MenuCmd::Duplicate).gesture("Ctrl+D").enabled(sel),
            MenuEntry::separator(),
            MenuEntry::item(ie::EditorView_Bringtofront(), app_icons::BringToFront, MenuCmd::BringToFront).gesture("Home").enabled(sel),
            MenuEntry::item(ie::EditorView_Bringforward(), app_icons::BringForward, MenuCmd::BringForward).gesture("PageUp").enabled(sel),
            MenuEntry::item(ie::EditorView_Sendbackward(), app_icons::SendBackward, MenuCmd::SendBackward).gesture("PageDown").enabled(sel),
            MenuEntry::item(ie::EditorView_Sendtoback(), app_icons::SendToBack, MenuCmd::SendToBack).gesture("End").enabled(sel),
            MenuEntry::separator(),
            MenuEntry::item(toggle, app_icons::ChevronsUpDown, MenuCmd::ToggleToolbars).gesture("Ctrl+H"),
        ]
    }

    /// `EditorContextMenu`: a plain `ContextMenu` (`MenuItem`s 12,8 padded)
    /// at the pointer.
    fn context_flyout(entries: Vec<MenuEntry<MenuCmd>>, at: Pos2) -> Flyout<MenuCmd> {
        let entries = Self::plain_icons(entries);
        let mut f = Flyout::menu(Id::new("editor-context"), entries, Rect::from_min_size(at, Vec2::ZERO), Placement::Pointer(at));
        f.style = sharex_ui::menu::MenuStyle::Standard;
        f
    }

    /// The File button's `MenuFlyout` (`BottomEdgeAlignedLeft`, 4 down).
    fn file_flyout(&self, button: Rect) -> Flyout<MenuCmd> {
        let entries = Self::plain_icons(self.file_menu());
        let mut f = Flyout::menu(Id::new("editor-file"), entries, button.translate(vec2(0.0, 4.0)), Placement::BottomEdgeAlignedLeft);
        f.style = sharex_ui::menu::MenuStyle::Standard;
        f
    }

    /// Plain `MenuItem.Icon` `IconPresenter`s: the item's foreground.
    fn plain_icons(entries: Vec<MenuEntry<MenuCmd>>) -> Vec<MenuEntry<MenuCmd>> {
        entries
            .into_iter()
            .map(|e| match e.icon {
                sharex_ui::menu::MenuIcon::Key(k) => e.icon(sharex_ui::menu::MenuIcon::Plain(k, 14.0)),
                _ => e,
            })
            .collect()
    }

    fn file_menu(&self) -> Vec<MenuEntry<MenuCmd>> {
        // `RecentImageMenuItems`: each path in a 300px `TextBlock` trimmed
        // at its start.
        let recent: Vec<MenuEntry<MenuCmd>> = self
            .ws
            .tools
            .options
            .RecentImageFiles
            .iter()
            .enumerate()
            .map(|(i, p)| MenuEntry::item(p.clone(), "", MenuCmd::OpenRecent(i as u8)).id("recent-file"))
            .collect();
        let has_recent = !recent.is_empty();
        vec![
            MenuEntry::item(ie::AnnotationToolbar_NewEllipsis(), app_icons::FileAdd, MenuCmd::New).gesture("Ctrl+N"),
            MenuEntry::item(ie::AnnotationToolbar_OpenEllipsis(), app_icons::FolderOpen, MenuCmd::Open).gesture("Ctrl+O"),
            MenuEntry::parent(ie::AnnotationToolbar_Openrecent(), app_icons::History, recent).visible(has_recent),
            MenuEntry::item(ie::EditorView_NewFromScreenshotRegionEllipsis(), app_icons::Region, MenuCmd::NewRegion),
            MenuEntry::item(ie::EditorView_NewFromScreenshotFullScreen(), app_icons::Fullscreen, MenuCmd::NewFullscreen),
            MenuEntry::separator(),
            MenuEntry::item(ie::AnnotationToolbar_Save(), app_icons::Save, MenuCmd::Save).gesture("Ctrl+S"),
            MenuEntry::item(ie::AnnotationToolbar_SaveasEllipsis(), app_icons::SaveAs, MenuCmd::SaveAs).gesture("Ctrl+Shift+S"),
            MenuEntry::separator(),
            MenuEntry::item(ie::AnnotationToolbar_Exit(), app_icons::Exit, MenuCmd::Exit).gesture("Escape"),
        ]
    }

    // Layout and drawing.

    /// The top toolbar's content width (items, spacing, separators).
    fn top_content_width() -> f32 {
        let items = Item::all();
        let mut w = 0.0;
        for (i, it) in items.iter().enumerate() {
            if i > 0 {
                w += SPACING;
            }
            if it.begin_group() {
                w += 1.0 + SPACING;
            }
            w += it.width();
        }
        w
    }

    fn paint_top(&mut self, ui: &Ui, p: &Palette, full: Rect) {
        let content = Self::top_content_width();
        let natural = content + 2.0 * (PADDING + 1.0);
        let width = natural.min(full.width());
        let left = (full.center().x - width / 2.0).round_ties_even();
        let rect = Rect::from_min_size(pos2(left, full.min.y), vec2(width, BUTTON + 2.0 * (PADDING + 1.0)));
        let max_scroll = (natural - width).max(0.0);
        let modal_open = self.modal.is_some();
        if ui.rect_contains_pointer(rect) && !modal_open {
            let d = ui.ctx().input(|i| i.smooth_scroll_delta);
            self.top_scroll = (self.top_scroll - d.x - d.y).clamp(0.0, max_scroll);
        }
        self.top_scroll = self.top_scroll.clamp(0.0, max_scroll);
        let painter = ui.painter();
        // `MainToolbarCornerRadius="0,0,4,4"`, a 1px border.
        let radius = CornerRadius { nw: 0, ne: 0, sw: 4, se: 4 };
        painter.rect_filled(rect, radius, p.background_toolbar);
        painter.rect_stroke(rect, radius, Stroke::new(1.0, p.border), StrokeKind::Inside);
        // The `ScrollViewer` sits inside the border and `Padding="4"`, and
        // clips there.
        let viewport = Rect::from_min_max(pos2(rect.min.x + 1.0 + PADDING, rect.min.y + 1.0), pos2(rect.max.x - 1.0 - PADDING, rect.max.y - 1.0));
        let clip = painter.with_clip_rect(viewport);
        let top = rect.min.y + 1.0 + PADDING;
        let mut x = rect.min.x + 1.0 + PADDING - self.top_scroll;
        self.layout.items.clear();
        let active = self.ws.active_tool();
        for (i, it) in Item::all().into_iter().enumerate() {
            if i > 0 {
                x += SPACING;
            }
            if it.begin_group() {
                clip.rect_filled(Rect::from_min_size(pos2(x, top), vec2(1.0, BUTTON)), 0.0, p.separator);
                x += 1.0 + SPACING;
            }
            let r = Rect::from_min_size(pos2(x, top), vec2(it.width(), BUTTON));
            x += it.width();
            let hovered = !modal_open && ui.rect_contains_pointer(r.intersect(rect));
            if it == Item::File {
                captb::button(&clip, p, r, "", false, hovered, 1.0);
                // The 24px icon left, the 10px chevron right (padding 6).
                icons::paint(&clip, Rect::from_min_size(pos2(r.min.x + 7.0, r.center().y - 12.0), vec2(24.0, 24.0)), it.icon(), p.text);
                let open = self.flyout.is_some() && self.layout.top.is_some();
                let angle = if open { std::f32::consts::PI } else { 0.0 };
                icons::paint_transformed(&clip, Rect::from_min_size(pos2(r.max.x - 17.0, r.center().y - 5.0), vec2(10.0, 10.0)), app_icons::ChevronDown, p.text_secondary, Vec2::ZERO, angle);
            } else {
                let on = it.tool().is_some_and(|t| t == active) || (it == Item::CanvasSize && self.canvas_resize.is_some());
                captb::button(&clip, p, r, it.icon(), on, hovered, 1.0);
            }
            self.layout.items.push((it, r.intersect(rect)));
        }
        // The `ScrollViewer`'s collapsed scroll bar when the items overflow.
        if max_scroll > 0.0 {
            // Fluent's horizontal `ScrollBar` across the viewport: the
            // track between the two 16px line buttons, the thumb its share
            // of the content.
            let track = viewport.shrink2(vec2(16.0, 0.0));
            let tw = track.width() * viewport.width() / content;
            let tx = track.min.x + (track.width() - tw) * (self.top_scroll / max_scroll);
            let bar = Rect::from_min_size(pos2(tx, rect.max.y - 9.0), vec2(tw, 2.0));
            painter.rect_filled(bar, CornerRadius::same(1), sharex_ui::theme::fluent(ui.ctx()).base_medium);
        }
        self.layout.top = Some(rect);
        // The tool options row (`ShowToolOptions`), and its popup.
        let tool = self.ws.active_tool();
        let visible = Visible::of(tool.clone(), self.ws.selected());
        let row = options_row::layout(ui, rect, 1.0, &visible, &self.ws.tools);
        if let Some(row) = &row {
            let hovered = row.hit(ui.ctx().pointer_hover_pos().unwrap_or(Pos2::new(-1.0, -1.0)));
            options_row::paint(ui, ui.painter(), p, row, &tool, &self.ws.tools, hovered, self.popups.open, 1.0);
        }
        self.layout.options = row;
    }

    fn paint_bottom(&mut self, ui: &Ui, p: &Palette, full: Rect) {
        let font = widgets::font(12.0, 600);
        let zoom_text = format!("{}%", (self.zoom * 100.0).round() as i64);
        // `ZoomPickerDropdown`: the 20px icon (6 after it), the text, the
        // chevron (`Margin="8,0,4,0"`), in the button's 10px padding.
        let zoom_w = (2.0 + 20.0 + 20.0 + 6.0 + widgets::text_width(ui, &zoom_text, font.clone()).ceil() + 8.0 + 10.0 + 4.0).max(80.0);
        // Items: zoom, separator, task grid (90 + margins 4/10), six
        // buttons, separator, two capture buttons.
        let sep = 1.0 + 8.0;
        let content = zoom_w + SPACING + sep + SPACING + 4.0 + 90.0 + 10.0 + SPACING + 6.0 * BUTTON + 5.0 * SPACING + SPACING + sep + SPACING + 2.0 * BUTTON + SPACING;
        let width = content + 2.0 * (PADDING + 1.0);
        let height = BUTTON + 2.0 * (PADDING + 1.0);
        let rect = Rect::from_min_size(pos2((full.center().x - width / 2.0).round_ties_even(), full.max.y - height), vec2(width, height));
        let painter = ui.painter();
        let radius = CornerRadius { nw: 4, ne: 4, sw: 0, se: 0 };
        painter.rect_filled(rect, radius, p.background_toolbar);
        painter.rect_stroke(rect, radius, Stroke::new(1.0, p.border), StrokeKind::Inside);
        let top = rect.min.y + 1.0 + PADDING;
        let mut x = rect.min.x + 1.0 + PADDING;
        let mut items = Vec::new();
        let modal_open = self.modal.is_some();
        let hover = |r: Rect| !modal_open && ui.rect_contains_pointer(r);
        // Zoom.
        let zr = Rect::from_min_size(pos2(x, top), vec2(zoom_w, BUTTON));
        captb::button(painter, p, zr, "", false, hover(zr), 1.0);
        icons::paint(painter, Rect::from_min_size(pos2(zr.min.x + 11.0, zr.center().y - 10.0), vec2(20.0, 20.0)), app_icons::ZoomIn, p.text_secondary);
        let g = widgets::galley(ui, &zoom_text, font.clone(), p.text, f32::INFINITY);
        let tw = widgets::text_width(ui, &zoom_text, font.clone());
        let mid = (zr.min.x + 11.0 + 26.0 + zr.max.x - 11.0 - 4.0 - 10.0 - 8.0) / 2.0;
        painter.galley(pos2((mid - tw / 2.0).round(), widgets::center_y(zr, widgets::line_height(ui, &font))), g, p.text);
        let angle = if self.zoom_open { std::f32::consts::PI } else { 0.0 };
        icons::paint_transformed(painter, Rect::from_min_size(pos2(zr.max.x - 25.0, zr.center().y - 5.0), vec2(10.0, 10.0)), app_icons::ChevronDown, p.text_secondary, Vec2::ZERO, angle);
        items.push((Bottom::Zoom, zr));
        x += zoom_w + SPACING + 4.0;
        painter.rect_filled(Rect::from_min_size(pos2(x, top + 1.0), vec2(1.0, 38.0)), 0.0, p.separator);
        x += 1.0 + 4.0 + SPACING + 4.0;
        // The task buttons: cancel (55x40, right), continue (56 round, left,
        // 8px below the bar).
        let grid = Rect::from_min_size(pos2(x, top), vec2(90.0, BUTTON));
        let cancel = Rect::from_min_max(pos2(grid.max.x - 55.0, grid.min.y), grid.max);
        let cr = CornerRadius { nw: 0, sw: 0, ne: 4, se: 4 };
        let (cbg, cbd) = (p.cancel_background, p.cancel_border);
        painter.rect_filled(cancel, cr, if hover(cancel) { cbd } else { cbg });
        painter.rect_stroke(cancel, cr, Stroke::new(1.0, cbd), StrokeKind::Inside);
        // `Padding="27,0,10,0"`: the 20px icon in the right part.
        let ic = Rect::from_min_size(pos2(cancel.min.x + 27.0 + (cancel.width() - 37.0 - 20.0) / 2.0, cancel.center().y - 10.0), vec2(20.0, 20.0));
        icons::paint(painter, ic, app_icons::Close, p.task_mode_foreground);
        items.push((Bottom::Cancel, cancel));
        let cont = Rect::from_min_size(pos2(grid.min.x, grid.max.y + 8.0 - 56.0), vec2(56.0, 56.0));
        let (gbg, gbd) = (p.continue_background, p.continue_border);
        painter.circle_filled(cont.center(), 28.0, if hover(cont) { gbd } else { gbg });
        painter.circle_stroke(cont.center(), 27.5, Stroke::new(1.0, gbd));
        icons::paint(painter, Rect::from_center_size(cont.center() + vec2(2.0, 0.0), vec2(20.0, 20.0)), app_icons::Play, p.task_mode_foreground);
        items.push((Bottom::Continue, cont));
        x = grid.max.x + 10.0 + SPACING;
        for b in [Bottom::Copy, Bottom::Save, Bottom::SaveAs, Bottom::Pin, Bottom::Print, Bottom::Upload] {
            let r = Rect::from_min_size(pos2(x, top), vec2(BUTTON, BUTTON));
            // `CanCopy`... `HasPreviewImage`: disabled without an image.
            if self.has_image {
                captb::button(painter, p, r, b.icon(), false, hover(r), 1.0);
                items.push((b, r));
            } else {
                disabled_button(ui, painter, p, r, b.icon());
            }
            x += BUTTON + SPACING;
        }
        x += 4.0;
        painter.rect_filled(Rect::from_min_size(pos2(x, top + 1.0), vec2(1.0, 38.0)), 0.0, p.separator);
        x += 1.0 + 4.0 + SPACING;
        for b in [Bottom::CaptureRegion, Bottom::CaptureFullscreen] {
            let r = Rect::from_min_size(pos2(x, top), vec2(BUTTON, BUTTON));
            captb::button(painter, p, r, b.icon(), false, hover(r), 1.0);
            items.push((b, r));
            x += BUTTON + SPACING;
        }
        self.layout.bottom = Some(rect);
        self.layout.bottom_items = items;
    }

    /// The canvas: the image (with its annotations) at the zoom, centred
    /// in the canvas host, and the editor's overlay.
    fn paint_canvas(&mut self, ui: &Ui, p: &Palette, full: Rect) {
        let ctx = ui.ctx().clone();
        let ppp = ctx.pixels_per_point();
        let canvas = full.shrink(8.0);
        self.layout.canvas = canvas;
        if !self.has_image {
            self.layout.image = Rect::NOTHING;
            return;
        }
        if self.zoom_to_fit_pending {
            self.zoom_to_fit_pending = false;
            self.zoom_to_fit();
        }
        let (w, h) = self.ws.size();
        // One image pixel is one DIP at 100%.
        let size = vec2(w as f32, h as f32) * self.zoom as f32;
        // Centred when it fits (with its room), scrolled when it doesn't.
        let content = size + self.room * 2.0;
        let max_scroll = vec2((content.x - canvas.width()).max(0.0), (content.y - canvas.height()).max(0.0));
        self.scroll = self.scroll.clamp(Vec2::ZERO, max_scroll);
        let min = self.image_origin(self.room, self.scroll);
        let min = pos2((min.x * ppp).round() / ppp, (min.y * ppp).round() / ppp);
        let image = Rect::from_min_size(min, size);
        self.layout.image = image;
        let version = self.ws.version();
        if self.tex.as_ref().is_none_or(|(v, _)| *v != version) {
            let pm = self.ws.composite().clone();
            let mut rgba = Vec::with_capacity(pm.pixels().len() * 4);
            for c in pm.pixels() {
                rgba.extend_from_slice(&[(c >> 16) as u8, (c >> 8) as u8, *c as u8, (c >> 24) as u8]);
            }
            let img = egui::ColorImage::from_rgba_premultiplied([pm.width() as usize, pm.height() as usize], &rgba);
            let filter = if self.zoom >= 1.0 { egui::TextureOptions::NEAREST } else { egui::TextureOptions::LINEAR };
            match &mut self.tex {
                Some((v, t)) => {
                    t.set(img, filter);
                    *v = version;
                }
                None => self.tex = Some((version, ctx.load_texture("image-editor", img, filter))),
            }
        }
        let painter = ui.painter().with_clip_rect(canvas.intersect(ui.clip_rect()));
        // The `Border` with `ShareX.Brush.Checkered` (15px squares, scaled
        // with the zoom) and its 1px border, under the image.
        paint_checkered(&painter, image, image.min, self.zoom as f32, p);
        painter.rect_stroke(image, 0.0, Stroke::new(self.zoom as f32, p.border), StrokeKind::Inside);
        if let Some((_, t)) = &self.tex {
            painter.image(t.id(), image, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        }
        let m = self.mapper_ppp(ppp);
        let hovered = ctx.pointer_hover_pos().is_some_and(|p| self.layout.crop_button.is_some_and(|r| r.contains(p)));
        let chrome = editor_chrome::paint(ui, &painter, p, &m, &self.ws, hovered);
        self.layout.crop_button = chrome.crop_button;
        // The canvas size tool's overlay (`OverlayHost`, unclipped).
        if self.canvas_resize.is_some() {
            self.paint_canvas_resize(ui, p);
        }
        // `CanvasScrollViewer`'s collapsed scroll bars: 2px thumbs on the
        // bottom and right edges (overlaid), each track between Fluent's
        // 16px line buttons, the horizontal one short of the vertical
        // bar's column.
        let thumb = sharex_ui::theme::fluent(ui.ctx()).base_medium;
        let (hx, vy) = (max_scroll.x > 0.0, max_scroll.y > 0.0);
        if hx {
            let right = canvas.max.x - if vy { 16.0 } else { 0.0 };
            let track = (canvas.min.x + 16.0, right - 16.0);
            let len = track.1 - track.0;
            let tw = (len * canvas.width() / content.x).max(16.0);
            let x = track.0 + (len - tw) * self.scroll.x / max_scroll.x;
            ui.painter().rect_filled(Rect::from_min_size(pos2(x, canvas.max.y - 4.0), vec2(tw, 2.0)), CornerRadius::same(1), thumb);
        }
        if vy {
            let bottom = canvas.max.y - if hx { 16.0 } else { 0.0 };
            let track = (canvas.min.y + 16.0, bottom - 16.0);
            let len = track.1 - track.0;
            let th = (len * canvas.height() / content.y).max(16.0);
            let y = track.0 + (len - th) * self.scroll.y / max_scroll.y;
            ui.painter().rect_filled(Rect::from_min_size(pos2(canvas.max.x - 4.0, y), vec2(2.0, th)), CornerRadius::same(1), thumb);
        }
    }

    /// The part of the window the toolbars, menus and popups cover.
    fn over_ui(&self, p: Pos2) -> bool {
        if self.popups.contains(p) || self.flyout.as_ref().is_some_and(|f| f.contains(p)) || self.notify.contains(p) {
            return true;
        }
        if self.toolbars_hidden {
            return false;
        }
        self.layout.top.is_some_and(|r| r.contains(p))
            || self.layout.options.as_ref().is_some_and(|o| o.rect.contains(p))
            || self.layout.bottom.is_some_and(|r| r.contains(p))
            || self.layout.bottom_items.iter().any(|(_, r)| r.contains(p))
    }

    fn mods(m: egui::Modifiers) -> ed::Mods {
        ed::Mods { shift: m.shift, ctrl: m.command || m.ctrl, alt: m.alt }
    }

    fn key(k: egui::Key) -> Option<ed::Key> {
        use egui::Key as K;
        Some(match k {
            K::Escape => ed::Key::Escape,
            K::Enter => ed::Key::Enter,
            K::Delete => ed::Key::Delete,
            K::Backspace => ed::Key::Backspace,
            K::ArrowLeft => ed::Key::Left,
            K::ArrowRight => ed::Key::Right,
            K::ArrowUp => ed::Key::Up,
            K::ArrowDown => ed::Key::Down,
            K::Home => ed::Key::Home,
            K::End => ed::Key::End,
            K::PageUp => ed::Key::PageUp,
            K::PageDown => ed::Key::PageDown,
            _ => {
                let name = k.name();
                if name.len() == 1 {
                    let c = name.chars().next()?;
                    if c.is_ascii_alphabetic() {
                        return Some(ed::Key::Letter(c.to_ascii_uppercase()));
                    }
                    if let Some(d) = c.to_digit(10) {
                        return Some(ed::Key::Digit(d as u8));
                    }
                }
                return None;
            }
        })
    }

    /// The pointer and keys (`EditorView`'s handlers).
    fn input(&mut self, ui: &Ui) {
        let ctx = ui.ctx().clone();
        let ppp = ctx.pixels_per_point();
        let m = self.mapper_ppp(ppp);
        let events = ctx.input(|i| i.events.clone());
        let modal = self.modal.is_some();
        for ev in events {
            match ev {
                egui::Event::PointerMoved(pos) => {
                    if let Some(start) = self.pan {
                        self.scroll -= pos - start;
                        self.pan = Some(pos);
                        continue;
                    }
                    if modal {
                        continue;
                    }
                    if self.canvas_resize.is_some() {
                        self.canvas_resize_moved(pos);
                        continue;
                    }
                    let mods = Self::mods(ctx.input(|i| i.modifiers));
                    if self.capturing || (!self.over_ui(pos) && self.layout.canvas.contains(pos)) {
                        self.ws.pointer_moved(m.image_pos(pos), mods);
                    }
                }
                egui::Event::PointerButton { pos, button, pressed, modifiers } => {
                    let mods = Self::mods(modifiers);
                    if button == egui::PointerButton::Middle {
                        self.pan = pressed.then_some(pos);
                        continue;
                    }
                    if modal || !self.has_image {
                        continue;
                    }
                    let b = match button {
                        egui::PointerButton::Primary => ed::Button::Left,
                        egui::PointerButton::Secondary => ed::Button::Right,
                        _ => continue,
                    };
                    // The Canvas size tool has the canvas host's presses
                    // (but for its panel and popup).
                    if let Some(cr) = &self.canvas_resize {
                        if pressed {
                            if self.over_ui(pos) || cr.passes_through(pos) || !self.layout.view.contains(pos) {
                                continue;
                            }
                            if b == ed::Button::Right {
                                self.cancel_canvas_resize();
                            } else {
                                let now = ctx.input(|i| i.time);
                                let double = self.last_click.is_some_and(|(t, q)| now - t < 0.5 && q.distance(pos) < 4.0);
                                self.last_click = Some((now, pos));
                                self.canvas_resize_pressed(pos, double);
                            }
                        } else if b == ed::Button::Left {
                            self.canvas_resize_released();
                        }
                        continue;
                    }
                    if pressed {
                        if self.over_ui(pos) || !self.layout.canvas.contains(pos) {
                            continue;
                        }
                        if self.layout.crop_button.is_some_and(|r| r.contains(pos)) {
                            self.ws.confirm_crop();
                            continue;
                        }
                        // A press on the canvas closes an open popup first.
                        if self.popups.open.is_some() {
                            self.popups.close();
                            continue;
                        }
                        let at = m.image_pos(pos);
                        if b == ed::Button::Right {
                            if self.ws.is_drawing() {
                                self.ws.pointer_pressed(at, b, mods);
                            } else if self.ws.right_pressed(at) == RightClick::ContextMenu {
                                self.flyout = Some(Self::context_flyout(self.context_menu(), pos));
                            }
                            continue;
                        }
                        self.capturing = true;
                        self.ws.pointer_pressed(at, b, mods);
                        self.handle_requests();
                    } else if self.capturing && b == ed::Button::Left {
                        self.capturing = false;
                        self.ws.pointer_released(m.image_pos(pos), b, mods);
                    }
                }
                egui::Event::MouseWheel { delta, modifiers, .. } => {
                    let pos = ctx.pointer_hover_pos().unwrap_or_default();
                    if modifiers.command || modifiers.ctrl {
                        if delta.y != 0.0 {
                            self.zoom_step(delta.y > 0.0);
                        }
                    } else if !self.over_ui(pos) && self.layout.canvas.contains(pos) {
                        let step = 40.0;
                        if modifiers.shift {
                            self.scroll.x -= delta.y.signum() * step;
                        } else {
                            self.scroll.y -= delta.y.signum() * step;
                            self.scroll.x -= delta.x.signum() * step;
                        }
                    }
                }
                egui::Event::Text(t) => {
                    if !modal && self.ws.text_edit().is_some() {
                        self.ws.text_input(&t);
                    }
                }
                egui::Event::Copy => self.key_down(ed::Key::Letter('C'), ed::Mods { ctrl: true, ..ed::Mods::NONE }),
                egui::Event::Cut => self.key_down(ed::Key::Letter('X'), ed::Mods { ctrl: true, ..ed::Mods::NONE }),
                egui::Event::Paste(_) => self.key_down(ed::Key::Letter('V'), ed::Mods { ctrl: true, ..ed::Mods::NONE }),
                egui::Event::Key { key, pressed, modifiers, repeat, .. } => {
                    let mods = Self::mods(modifiers);
                    if modal {
                        // `OnKeyUp`: Escape closes the dialog.
                        if !pressed && key == egui::Key::Escape && !modifiers.any() {
                            self.modal = None;
                        }
                        continue;
                    }
                    // Popups and menus have the keys while open.
                    if self.popups.open.is_some() || self.flyout.is_some() || self.zoom_open {
                        continue;
                    }
                    let Some(k) = Self::key(key) else { continue };
                    if pressed {
                        if key == egui::Key::Escape {
                            continue;
                        }
                        let _ = repeat;
                        self.key_down(k, mods);
                    } else {
                        self.ws.key_up(k, mods);
                        // `OnKeyUp`: Escape closes the Canvas size tool first.
                        if key == egui::Key::Escape && mods == ed::Mods::NONE && self.cancel_canvas_resize() {
                            continue;
                        }
                        if key == egui::Key::Escape && !mods.ctrl && !mods.shift && !mods.alt {
                            let r = self.ws.escape();
                            if let Some(c) = r.command {
                                self.run(c);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn key_down(&mut self, k: ed::Key, mods: ed::Mods) {
        // Enter applies the canvas size first (`TryConfirmCanvasResize`).
        if k == ed::Key::Enter && mods == ed::Mods::NONE && self.confirm_canvas_resize() {
            return;
        }
        // `TrySelectToolForToolbarHotkey` for the items that aren't tools
        // (Canvas size: X), outside text editing.
        if self.ws.text_edit().is_none()
            && let Some(it) = Self::hotkey_item(k, mods)
        {
            self.select_item(it);
            return;
        }
        let r = self.ws.key_down(k, mods, &Self::hotkey_tool);
        if let Some(c) = r.command {
            self.run(c);
        }
        self.handle_requests();
    }

    /// The toolbars' buttons (their `Click`s); the modal scrim covers them.
    fn toolbar_clicks(&mut self, ui: &Ui) {
        if self.toolbars_hidden || self.modal.is_some() {
            return;
        }
        for (it, r) in self.layout.items.clone() {
            if ui.interact(r, Id::new(("editor-item", format!("{it:?}"))), Sense::click()).clicked() {
                if it == Item::File {
                    self.flyout = Some(self.file_flyout(r));
                } else {
                    self.select_item(it);
                }
            }
        }
        if let Some(row) = self.layout.options.clone() {
            for (item, r) in &row.items {
                let resp = ui.interact(*r, Id::new(("editor-option", *item as u8)), Sense::click());
                if resp.clicked() {
                    self.option_clicked(*item);
                }
                if resp.secondary_clicked() && *item == OptionItem::Shadow {
                    self.popups.toggle(OptionItem::Shadow, &self.ws.tools);
                }
                if item.is_slider()
                    && let Some(pos) = resp.interact_pointer_pos()
                    && (resp.dragged() || resp.clicked() || resp.drag_started())
                {
                    let left = r.min.x + 29.0 + 10.0;
                    let t = ((pos.x - left) / 100.0).clamp(0.0, 1.0);
                    let tool = self.ws.active_tool();
                    let item = *item;
                    self.ws.set_option(|s, target| {
                        if item == OptionItem::Strength {
                            let max = ed::max_effect_strength(tool);
                            s.set_effect_strength((1.0 + t * (max - 1.0)).round(), target);
                        } else {
                            s.set_spotlight_blur((t * 200.0).round(), target);
                        }
                    });
                }
            }
        }
        for (b, r) in self.layout.bottom_items.clone() {
            if ui.interact(r, Id::new(("editor-bottom", b as u8)), Sense::click()).clicked() {
                match b {
                    Bottom::Zoom => self.zoom_open = !self.zoom_open,
                    Bottom::Continue => self.run(Command::Continue),
                    // `CancelCommand`: `RequestClose`.
                    Bottom::Cancel => self.request_close(false),
                    Bottom::Copy => self.run(Command::CopyImage),
                    Bottom::Save => self.run(Command::Save),
                    Bottom::SaveAs => self.run(Command::SaveAs),
                    Bottom::Pin => self.run(Command::PinToScreen),
                    Bottom::Print => self.run(Command::Print),
                    Bottom::Upload => self.run(Command::Upload),
                    Bottom::CaptureRegion | Bottom::CaptureFullscreen => self.menu_command(MenuCmd::NewRegion),
                }
            }
        }
    }

    /// An options row control: a toggle flips, a dropdown opens its popup.
    fn option_clicked(&mut self, item: OptionItem) {
        if Popups::has_popup(item) {
            self.popups.toggle(item, &self.ws.tools);
            return;
        }
        self.ws.set_option(|s, t| match item {
            OptionItem::Bold => s.set_text_bold(!s.text_bold, t),
            OptionItem::Italic => s.set_text_italic(!s.text_italic, t),
            OptionItem::EffectEllipse => s.set_effect_ellipse(!s.effect_ellipse, t),
            OptionItem::ArrowRoundedCorners => s.set_arrow_rounded_corners(!s.arrow_rounded_corners, t),
            OptionItem::FreehandArrow => s.set_freehand_arrow(!s.freehand_arrow, t),
            OptionItem::Tail => s.set_speech_balloon_tail(!s.speech_balloon_tail, t),
            OptionItem::AutoSize => s.set_speech_balloon_auto_size(!s.speech_balloon_auto_size, t),
            OptionItem::Shadow => s.set_shadow_enabled(!s.shadow_enabled),
            _ => {}
        });
    }

    /// `ZoomPickerDropdown`'s popup: the levels, a separator, Zoom to fit
    /// (opens upward, `VerticalOffset="-4"`).
    fn zoom_popup(&mut self, ui: &Ui, p: &Palette) {
        let Some(anchor) = self.layout.bottom_items.iter().find(|(b, _)| *b == Bottom::Zoom).map(|(_, r)| *r) else { return };
        let font = widgets::font(12.0, 600);
        let w = anchor.width().max(60.0) + 18.0;
        let h = 18.0 + ZOOM_LEVELS.len() as f32 * 32.0 + 9.0 + 32.0;
        let rect = Rect::from_min_size(pos2(anchor.center().x - w / 2.0, anchor.min.y - 4.0 - h), vec2(w, h));
        let mut choice = None;
        let area = egui::Area::new(Id::new("editor-zoom-popup")).order(egui::Order::Foreground).fixed_pos(rect.min).fade_in(false).show(ui.ctx(), |ui| {
            let (r, _) = ui.allocate_exact_size(rect.size(), Sense::hover());
            sharex_ui::border::border(ui.painter(), r, 4.0, 1.0, Some(sharex_ui::border::Brush::Solid(p.control_background)), Some(p.border));
            let mut y = r.min.y + 9.0;
            for (i, z) in ZOOM_LEVELS.iter().enumerate() {
                let ir = Rect::from_min_size(pos2(r.min.x + 9.0, y), vec2(r.width() - 18.0, 32.0));
                let resp = ui.interact(ir, Id::new(("zoom-level", i)), Sense::click());
                if resp.hovered() {
                    ui.painter().rect_filled(ir, CornerRadius::same(4), p.control_background_hover);
                }
                let t = format!("{}%", (z * 100.0).round() as i64);
                let tw = widgets::text_width(ui, &t, font.clone());
                let g = widgets::galley(ui, &t, font.clone(), p.text, f32::INFINITY);
                ui.painter().galley(pos2((ir.center().x - tw / 2.0).round(), widgets::center_y(ir, widgets::line_height(ui, &font))), g, p.text);
                if resp.clicked() {
                    choice = Some(MenuCmd::Zoom(i as u8));
                }
                y += 32.0;
            }
            ui.painter().rect_filled(Rect::from_min_size(pos2(r.min.x + 13.0, y + 4.0), vec2(r.width() - 26.0, 1.0)), 0.0, p.separator);
            y += 9.0;
            let ir = Rect::from_min_size(pos2(r.min.x + 9.0, y), vec2(r.width() - 18.0, 32.0));
            let resp = ui.interact(ir, Id::new("zoom-fit"), Sense::click());
            if resp.hovered() {
                ui.painter().rect_filled(ir, CornerRadius::same(4), p.control_background_hover);
            }
            let t = ie::ZoomPickerDropdown_ZoomtoFit();
            let tw = widgets::text_width(ui, t, font.clone());
            let g = widgets::galley(ui, t, font.clone(), p.text, f32::INFINITY);
            ui.painter().galley(pos2((ir.center().x - tw / 2.0).round(), widgets::center_y(ir, widgets::line_height(ui, &font))), g, p.text);
            if resp.clicked() {
                choice = Some(MenuCmd::ZoomFit);
            }
        });
        if let Some(c) = choice {
            self.menu_command(c);
            self.zoom_open = false;
        } else if ui.ctx().input(|i| i.pointer.any_pressed())
            && let Some(at) = ui.ctx().input(|i| i.pointer.interact_pos())
            && !area.response.rect.contains(at)
            && !anchor.contains(at)
        {
            self.zoom_open = false;
        }
    }

    /// `ConfirmationDialogView` over the `modal-scrim`: 400 wide, the
    /// title, the warning sign (40px, the colour emoji font) and the
    /// message (wrapped) 25 apart in `Padding="25"`, Yes, No and Cancel at
    /// the right of the button bar.
    fn confirm_dialog(&mut self, ui: &Ui, p: &Palette, full: Rect) {
        const SIGN: &str = "\u{26A0}\u{FE0F}";
        let font = modal::text_font(13.0, 400);
        let lh = ui.fonts_mut(|f| f.row_height(&font));
        // The line's metrics are the `TextBlock`'s font's (Segoe UI); the
        // glyph comes from the emoji font.
        let (adv, _, _) = sharex_image_editor::render::emoji::line_metrics(SIGN, 40.0).unwrap_or((40.0, 30.0, 43.0));
        let primary = modal::text_font(40.0, 400);
        let sign_lh = ui.fonts_mut(|f| f.row_height(&primary));
        let ascent = widgets::galley(ui, "A", primary, p.text, f32::INFINITY).rows.first().and_then(|r| r.glyphs.first()).map_or(sign_lh * 0.8, |g| g.font_face_ascent);
        let sign_w = adv.ceil();
        let msg_w = 400.0 - 2.0 - 50.0 - sign_w - 25.0;
        let lines: Vec<String> = ie::ConfirmationDialogView_Message()
            .split('\n')
            .flat_map(|l| if l.is_empty() { vec![String::new()] } else { sharex_ui::controls::wrap_lines(ui, l, &font, msg_w) })
            .collect();
        let msg_h = (lh * lines.len() as f32).ceil();
        let content_h = msg_h.max(sign_lh.ceil());
        let size = vec2(400.0, 2.0 + modal::title_height(ui, false) + 25.0 + content_h + 25.0 + modal::BUTTON_BAR);
        let r = modal::centered(full, size);
        if modal::scrim(ui, p, full, r) {
            self.modal = None;
        }
        let inner = modal::frame(ui, p, r, modal::SHADOW_SMALL);
        let (title, _) = modal::title_bar(ui, p, inner, ie::ConfirmationDialogView_Title(), p.text_secondary, None);
        let content = Rect::from_min_size(pos2(inner.min.x + 25.0, title.max.y + 25.0), vec2(inner.width() - 50.0, content_h));
        let painter = ui.painter();
        // The sign, centred in the row.
        let sign_top = content.min.y + ((content_h - sign_lh.ceil()) / 2.0).round_ties_even();
        let tex = self.sign_texture.get_or_insert_with(|| {
            let (w, h) = (sign_w as u32 + 4, sign_lh.ceil() as u32 + 4);
            sharex_image_editor::render::emoji::render_line(SIGN, w, h, 40.0, ascent).map(|pm| {
                let mut rgba = Vec::with_capacity(pm.pixels().len() * 4);
                for c in pm.pixels() {
                    rgba.extend_from_slice(&[(c >> 16) as u8, (c >> 8) as u8, *c as u8, (c >> 24) as u8]);
                }
                let img = egui::ColorImage::from_rgba_premultiplied([pm.width() as usize, pm.height() as usize], &rgba);
                ui.ctx().load_texture("confirm-sign", img, egui::TextureOptions::NEAREST)
            })
        });
        if let Some(t) = tex {
            let s = t.size_vec2();
            painter.image(t.id(), Rect::from_min_size(pos2(content.min.x, sign_top), s), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        }
        let mut y = content.min.y + ((content_h - msg_h) / 2.0).round_ties_even();
        let x = content.min.x + sign_w + 25.0;
        for line in &lines {
            let g = widgets::galley(ui, line, font.clone(), p.text_secondary, f32::INFINITY);
            painter.galley(pos2(x, y.round()), g, p.text_secondary);
            y += lh;
        }
        let labels = [ie::ConfirmationDialogView_Yes(), ie::ConfirmationDialogView_No(), ie::ConfirmationDialogView_Cancel()];
        if let Some(k) = modal::button_bar(ui, p, inner, Id::new("confirm-exit"), &labels) {
            self.modal = None;
            match k {
                0 => {
                    self.save(false);
                    self.finish(false);
                }
                1 => self.finish(false),
                _ => {}
            }
        }
    }

    /// `OnImageChangedDuringCanvasResize` for the commands that change the
    /// image from elsewhere.
    fn image_changed_elsewhere(&mut self) {
        if let Some(cr) = self.canvas_resize.take() {
            self.room = Vec2::ZERO;
            if let Some(z) = cr.saved_zoom {
                self.zoom = z;
            }
        }
    }

    /// The open dialog over the view (`ModalContentHost`).
    fn modal_ui(&mut self, ui: &mut Ui, p: &Palette, full: Rect) {
        match self.modal.take() {
            None => {}
            Some(Modal::Confirm) => {
                self.modal = Some(Modal::Confirm);
                self.confirm_dialog(ui, p, full);
            }
            Some(Modal::Start(mut s)) => {
                let rect = modal::centered(full, s.size(ui, full.height()));
                let outside = modal::scrim(ui, p, full, rect);
                let action = s.show(ui, p, rect);
                self.modal = Some(Modal::Start(s));
                if outside {
                    self.modal = None;
                }
                if let Some(a) = action {
                    self.start_action(a);
                }
            }
            Some(Modal::NewImage(mut d)) => {
                let rect = modal::centered(full, d.size(ui));
                let pos = ui.ctx().input(|i| i.pointer.interact_pos());
                let outside = modal::scrim(ui, p, full, rect) && !pos.is_some_and(|q| d.popup_contains(q));
                let out = d.show(ui, p, rect);
                d.show_popup(ui, p, full);
                match out {
                    Some(new_image::Outcome::Ok) => self.create_new_image(&d),
                    Some(new_image::Outcome::Cancel) => {}
                    None if outside => {}
                    None => self.modal = Some(Modal::NewImage(d)),
                }
            }
            Some(Modal::InsertImage(mut d)) => {
                // `PositionModalOnCursorScreen`.
                let area = self.cursor_screen(true).unwrap_or(full);
                let size = d.size(ui);
                let rect = Rect::from_center_size(area.center(), size).translate(vec2(0.0, 0.0));
                let rect = Rect::from_min_size(pos2(rect.min.x.round(), rect.min.y.round()), size);
                let outside = modal::scrim(ui, p, full, rect);
                match d.show(ui, p, rect) {
                    Some(insert_image::Outcome::Place(pl)) => self.insert_with_placement(d.image, pl),
                    Some(insert_image::Outcome::Cancel) => {}
                    None if outside => {}
                    None => self.modal = Some(Modal::InsertImage(d)),
                }
            }
            Some(Modal::Emoji(mut d)) => {
                // `UpdateEmojiPickerPosition`: centred on the part of the
                // editor on the pointer's monitor, at most that big.
                let area = self.cursor_screen(true).unwrap_or(full);
                let size = d.size(area);
                let min = pos2((area.center().x - size.x / 2.0).round(), (area.center().y - size.y / 2.0).round());
                let rect = Rect::from_min_size(min, size);
                let combo_open = sharex_ui::controls::ComboBox::is_open(ui.ctx(), Id::new("emoji-group"));
                let outside = modal::scrim(ui, p, full, rect) && !combo_open;
                match d.show(ui, p, rect) {
                    Some(emoji_picker::Outcome::Pick(e)) => match d.replace.clone() {
                        Some(key) => self.ws.replace_emoji(&key, &e.unicode, e.display_name()),
                        None => self.insert_emoji(e),
                    },
                    Some(emoji_picker::Outcome::Cancel) => {}
                    None if outside => {}
                    None => self.modal = Some(Modal::Emoji(d)),
                }
            }
        }
    }

    /// `NewImageDialogViewModel`'s OK (`OnNewImageRequested`): an image of
    /// that size cleared to transparent or the colour, loaded with no file.
    fn create_new_image(&mut self, d: &new_image::NewImage) {
        let (w, h) = (d.width.clamp(1, 10000) as u32, d.height.clamp(1, 10000) as u32);
        let px = if d.transparent { [0, 0, 0, 0] } else { d.color.to_srgba_unmultiplied() };
        let img = RgbaImage::from_pixel(w, h, image::Rgba(px));
        self.load_bitmap(img, None);
        self.notify(Self::size_message(ie::MainViewModel_NewImageCreated(), w, h), app_icons::FileAdd);
    }

    /// The start screen's commands (`EnsureStartScreenDialog`'s callbacks).
    fn start_action(&mut self, a: start_screen::Action) {
        use start_screen::Action as A;
        match a {
            A::NewImage => {
                self.modal = None;
                self.run(Command::NewImage);
            }
            A::OpenFile => {
                self.modal = None;
                self.open_image();
            }
            A::LoadFromClipboard => self.load_from_clipboard(),
            A::ShowUrlInput => {
                // `PrepareStartScreenUrlInputAsync`: the clipboard's text when
                // it's an http(s) URL.
                let text = sharex_capture::clipboard::get_text().map(|t| t.trim().to_string());
                let url = text.filter(|t| start_screen::is_http_url(t));
                if let Some(Modal::Start(s)) = &mut self.modal {
                    s.show_url_input(url);
                }
            }
            A::SubmitUrl(url) => self.load_from_url(url),
            A::Close => self.modal = None,
            A::Exit => {
                self.modal = None;
                self.request_close(false);
            }
            A::OpenRecent(path) => self.load_recent(&path),
            A::CaptureRegion | A::CaptureFullscreen => self.menu_command(if a == A::CaptureRegion { MenuCmd::NewRegion } else { MenuCmd::NewFullscreen }),
        }
    }

    /// `OnLoadFromClipboardRequested`: an image, else the first image file,
    /// else the start screen says there's none.
    fn load_from_clipboard(&mut self) {
        if let Some(img) = sharex_capture::clipboard::get_image() {
            self.modal = None;
            self.load_bitmap(img, None);
            return;
        }
        for f in sharex_capture::clipboard::get_file_drop_list().unwrap_or_default() {
            if let Some(img) = crate::thumbnail::load(&f) {
                self.modal = None;
                self.load_bitmap(img, Some(f));
                return;
            }
        }
        self.start_status(ie::EditorView_ClipboardDoesNotContainImage().to_string());
    }

    /// `OnLoadFromUrlRequested`: downloaded (30 s timeout) on a thread.
    fn load_from_url(&mut self, url: String) {
        if let Some(Modal::Start(s)) = &mut self.modal {
            s.status = None;
            s.url_loading = true;
        }
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let request = sharex_uploaders::http::HttpRequest { method: sharex_uploaders::HttpMethod::GET, url, headers: Vec::new(), content: None };
            let options = sharex_uploaders::http::RequestOptions {
                timeout: Some(std::time::Duration::from_secs(30)),
                factory_client: false,
                ..Default::default()
            };
            let result = match sharex_uploaders::http::send(request, options) {
                Err(e) => Err(e.to_string()),
                // `EnsureSuccessStatusCode`.
                Ok(r) if !r.is_success() => Err(format!("Response status code does not indicate success: {} ({}).", r.status_code, r.status_description)),
                Ok(r) => match image::load_from_memory(r.response_data.as_deref().unwrap_or_default()) {
                    Ok(img) => Ok(img.to_rgba8()),
                    Err(_) => Err(String::new()),
                },
            };
            let _ = tx.send(result);
        });
        self.url_job = Some(rx);
    }

    fn poll_url_job(&mut self) {
        let Some(rx) = &self.url_job else { return };
        match rx.try_recv() {
            Ok(Ok(img)) => {
                self.url_job = None;
                if matches!(self.modal, Some(Modal::Start(_))) {
                    self.modal = None;
                }
                self.load_bitmap(img, None);
            }
            Ok(Err(e)) => {
                self.url_job = None;
                let message = if e.is_empty() {
                    ie::EditorView_UrlDoesNotPointToValidImage().to_string()
                } else {
                    ie::EditorView_FailedToDownloadImageFormat().replace("{0}", &e)
                };
                if let Some(Modal::Start(s)) = &mut self.modal {
                    s.url_loading = false;
                    s.status = Some(message);
                }
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => self.url_job = None,
        }
    }

    /// `OnLoadRecentFileRequested`: gone files leave the list.
    fn load_recent(&mut self, path: &str) {
        let p = std::path::Path::new(path);
        if !p.exists() {
            self.remove_recent(path);
            self.start_status(ie::EditorView_FileNoLongerExistsFormat().replace("{0}", path));
            return;
        }
        match crate::thumbnail::load(p) {
            Some(img) => {
                if matches!(self.modal, Some(Modal::Start(_))) {
                    self.modal = None;
                }
                self.load_bitmap(img, Some(p.to_path_buf()));
                self.notify(Self::file_message(ie::MainViewModel_ImageOpened(), p), icon::FolderOpen);
            }
            None => self.start_status(ie::EditorView_FailedToLoadImageFileFormat().replace("{0}", path)),
        }
    }

    /// `InsertExternalImageAsync`: with no image it becomes the image;
    /// otherwise the insert image dialog asks where (unless
    /// `ShowInsertImageDialog` is off: centred).
    fn insert_external_image(&mut self, img: RgbaImage, path: PathBuf) {
        if !self.has_image {
            self.load_bitmap(img, Some(path));
            return;
        }
        if self.ws.tools.options.ShowInsertImageDialog && !self.ws.host_mode {
            self.modal = Some(Modal::InsertImage(Box::new(insert_image::InsertImage { image: img })));
        } else {
            self.insert_with_placement(img, insert_image::Placement::Center);
        }
    }

    /// `InsertImageWithPlacementAsync`.
    fn insert_with_placement(&mut self, img: RgbaImage, placement: insert_image::Placement) {
        use insert_image::Placement as P;
        let (cw, ch) = self.ws.size();
        let (bw, bh) = (img.width(), img.height());
        let at = match placement {
            // The centre of the monitor under the pointer, clamped to the
            // canvas (or the visible canvas's centre).
            P::Center => {
                let (w, h) = (cw as f64, ch as f64);
                let c = self.cursor_screen_center_image().map(|(x, y)| (x.clamp(0.0, w), y.clamp(0.0, h))).unwrap_or_else(|| self.visible_center());
                (c.0 - bw as f64 / 2.0, c.1 - bh as f64 / 2.0)
            }
            P::CanvasExpandDown => {
                self.ws.resize_canvas(0, bw.saturating_sub(cw) as i32, bh as i32, 0, [0, 0, 0, 0]);
                (0.0, ch as f64)
            }
            P::CanvasExpandRight => {
                self.ws.resize_canvas(0, bw as i32, bh.saturating_sub(ch) as i32, 0, [0, 0, 0, 0]);
                (cw as f64, 0.0)
            }
        };
        self.ws.insert_image_annotation(pixmap_of(&img), at, EditorTool::Image, true);
        self.notify(ie::MainViewModel_ImageInserted(), tool_icons::Image);
        self.update_title();
    }

    /// `InsertEmojiAnnotation`: 160px, centred on the monitor under the
    /// pointer (clamped to the canvas), or on the canvas.
    fn insert_emoji(&mut self, entry: &sharex_image_editor::emoji_catalog::EmojiEntry) {
        let (w, h) = self.ws.size();
        let (w, h) = (w as f64, h as f64);
        let (cx, cy) = self.cursor_screen_center_image().map(|(x, y)| (x.clamp(0.0, w), y.clamp(0.0, h))).unwrap_or((w / 2.0, h / 2.0));
        let size = ed::EMOJI_SIZE;
        self.ws.insert_emoji(&entry.unicode, entry.display_name(), (cx - size / 2.0, cy - size / 2.0));
    }

    /// The pointer's monitor's centre in the view (`GetCursorScreenCenter`),
    /// as the window knows its place on the desktop.
    fn cursor_screen_center(&self) -> Option<Pos2> {
        let area = self.cursor_screen(false)?;
        Some(area.center())
    }

    /// The pointer's monitor in view coordinates; with `visible`, the part
    /// of the view on it (`GetCursorScreenCenterInView`'s area).
    fn cursor_screen(&self, visible: bool) -> Option<Rect> {
        // `--render`: `SHAREXRS_RENDER_SCREEN=x,y,w,h`, the monitor in view
        // coordinates (where ShareX's window sat when its shot was taken).
        if self.window_inner.is_none()
            && let Ok(v) = std::env::var("SHAREXRS_RENDER_SCREEN")
        {
            let n: Vec<f32> = v.split(',').filter_map(|x| x.trim().parse().ok()).collect();
            if n.len() == 4 {
                let r = Rect::from_min_size(pos2(n[0], n[1]), vec2(n[2], n[3]));
                return Some(if visible { r.intersect(self.layout.view) } else { r });
            }
        }
        let (inner, ppp) = self.window_inner?;
        let (cx, cy) = sharex_capture::screenshot::cursor_position()?;
        let screens = sharex_capture::screenshot::screens();
        let s = screens.iter().find(|s| s.bounds.contains(cx, cy))?;
        let b = s.bounds;
        let r = Rect::from_min_max(
            pos2(b.x as f32 / ppp - inner.min.x, b.y as f32 / ppp - inner.min.y),
            pos2((b.x + b.w) as f32 / ppp - inner.min.x, (b.y + b.h) as f32 / ppp - inner.min.y),
        );
        if visible {
            let v = r.intersect(self.layout.view);
            return (v.width() > 0.0 && v.height() > 0.0).then_some(v);
        }
        Some(r)
    }

    fn cursor_screen_center_image(&self) -> Option<(f64, f64)> {
        let c = self.cursor_screen_center()?;
        Some(self.mapper_ppp(1.0).image_pos(c))
    }

    /// The options the tools changed, for the task settings.
    pub fn options(&self) -> ImageEditorOptions {
        self.ws.tools.options.clone()
    }
}

/// `ShareX.Brush.Checkered`: 30px tiles of two 15px squares (scaled by
/// `zoom`) from `origin`, over `rect`.
pub(super) fn paint_checkered(painter: &egui::Painter, rect: Rect, origin: Pos2, zoom: f32, p: &Palette) {
    painter.rect_filled(rect, 0.0, p.checkered[0]);
    let cell = 15.0 * zoom;
    if cell < 0.5 {
        return;
    }
    let clip = painter.with_clip_rect(rect.intersect(painter.clip_rect()));
    let vis = rect.intersect(painter.clip_rect());
    if vis.width() <= 0.0 || vis.height() <= 0.0 {
        return;
    }
    let i0 = ((vis.min.x - origin.x) / cell).floor() as i64;
    let i1 = ((vis.max.x - origin.x) / cell).ceil() as i64;
    let j0 = ((vis.min.y - origin.y) / cell).floor() as i64;
    let j1 = ((vis.max.y - origin.y) / cell).ceil() as i64;
    for j in j0..j1 {
        for i in i0..i1 {
            if (i + j).rem_euclid(2) == 0 {
                let r = Rect::from_min_size(pos2(origin.x + i as f32 * cell, origin.y + j as f32 * cell), vec2(cell, cell));
                clip.rect_filled(r, 0.0, p.checkered[1]);
            }
        }
    }
}

/// A disabled `toolbar-button`: ShareX's `Button:disabled` (the control
/// background, no border, the base medium low foreground), at 35%.
fn disabled_button(ui: &Ui, painter: &egui::Painter, p: &Palette, r: Rect, icon: &str) {
    let mut faded = painter.clone();
    faded.set_opacity(0.35);
    faded.rect_filled(r, CornerRadius::same(4), p.control_background);
    icons::paint(&faded, Rect::from_center_size(r.center(), vec2(20.0, 20.0)), icon, sharex_ui::theme::fluent(ui.ctx()).base_medium_low);
}

/// The task's image format's extension (`ImageFormat.GetFileExtension`).
fn image_extension(task: &TaskSettings) -> &'static str {
    let format = task.ImageSettings.as_ref().map(|s| format!("{:?}", s.ImageFormat)).unwrap_or_default();
    match format.as_str() {
        "JPEG" => "jpg",
        "BMP" => "bmp",
        "GIF" => "gif",
        "TIFF" => "tif",
        "WebP" => "webp",
        _ => "png",
    }
}

impl Dialog for EditorWindow {
    fn spec(&self) -> Spec {
        let o = &self.ws.tools.options;
        let (w, h) = if o.RememberWindowState && o.WindowWidth > 0.0 { (o.WindowWidth as f32, o.WindowHeight as f32) } else { (1280.0, 720.0) };
        Spec { maximized: !o.RememberWindowState || o.IsWindowMaximized, ..Spec::sized(self.title.clone(), w, h, 640.0, 480.0) }
    }

    fn ui(&mut self, ui: &mut Ui, cx: &mut Cx) -> Vec<Effect> {
        let _ = cx;
        let p = palette(ui.ctx());
        let full = ui.max_rect();
        self.layout.view = full;
        let ppp = ui.ctx().pixels_per_point();
        self.window_inner = ui.ctx().input(|i| i.viewport().inner_rect).map(|r| (r, ppp));
        ui.painter().rect_filled(full, 0.0, p.background_main);
        self.input(ui);
        self.paint_canvas(ui, &p, full);
        if !self.toolbars_hidden {
            self.paint_top(ui, &p, full);
            self.paint_bottom(ui, &p, full);
            self.toolbar_clicks(ui);
            if std::mem::take(&mut self.open_file_menu)
                && let Some((_, r)) = self.layout.items.iter().find(|(it, _)| *it == Item::File)
            {
                self.flyout = Some(self.file_flyout(*r));
            }
        } else {
            self.layout.top = None;
            self.layout.options = None;
            self.layout.bottom = None;
            self.layout.bottom_items.clear();
        }
        // The open option popup, under its button.
        if let (Some(item), Some(row)) = (self.popups.open, self.layout.options.clone())
            && let Some(anchor) = row.item_rect(item)
        {
            let r = self.popups.show(ui, &p, anchor, full, &mut self.ws);
            if r.pick_screen_color {
                self.effects.push(Effect::NotPorted("Image editor: pick a colour from the screen"));
            }
        }
        if self.zoom_open {
            self.zoom_popup(ui, &p);
        }
        if let Some(f) = self.flyout.as_mut() {
            let out = f.show(ui, full);
            if out.closed {
                self.flyout = None;
            }
            if let Some(c) = out.command {
                self.flyout = None;
                self.menu_command(c);
            }
        }
        self.poll_url_job();
        if std::mem::take(&mut self.canvas_resize_on_open) {
            self.toggle_canvas_resize();
            ui.ctx().request_repaint();
        }
        // The notification (`ZIndex="28"`), under the modal host.
        self.notify.paint(ui, &p, full);
        self.modal_ui(ui, &p, full);
        // The cursor the workspace wants over the canvas.
        if let Some(pos) = ui.ctx().pointer_hover_pos()
            && self.layout.canvas.contains(pos)
            && !self.over_ui(pos)
        {
            let icon = match self.ws.cursor() {
                ed::Cursor::Crosshair => egui::CursorIcon::Crosshair,
                ed::Cursor::OpenHand => egui::CursorIcon::Grab,
                ed::Cursor::ClosedHand => egui::CursorIcon::Grabbing,
                ed::Cursor::IBeam => egui::CursorIcon::Text,
                ed::Cursor::Arrow => egui::CursorIcon::Default,
            };
            ui.ctx().set_cursor_icon(icon);
        }
        self.update_title();
        if self.ws.is_interacting() || self.pan.is_some() || self.canvas_resize.as_ref().is_some_and(|c| c.is_dragging()) {
            ui.ctx().request_repaint();
        }
        std::mem::take(&mut self.effects)
    }

    fn close_requested(&mut self) -> Vec<Effect> {
        // `OnClosing`: `RequestClose(ignoreModal: true)`.
        self.request_close(true);
        std::mem::take(&mut self.effects)
    }

    fn closed(&mut self, cx: &mut Cx) -> Vec<Effect> {
        // The tool options go back into the task settings
        // (`ToolsSettingsReference.ImageEditorOptions`).
        let options = self.options().to_value();
        cx.settings.DefaultTaskSettings.ToolsSettings.get_or_insert_default().ImageEditorOptions = options;
        let result = self.result.take();
        if let Some(reply) = self.reply.take() {
            let _ = reply.send(result);
            return Vec::new();
        }
        // `AnnotateImageAsync`: Continue runs the image's task.
        match result {
            Some(img) => vec![Effect::RunImageTask(img, Box::new(self.task.clone()))],
            None => Vec::new(),
        }
    }

    fn replace_image(&mut self, image: RgbaImage, _ctx: &egui::Context) {
        self.load(image);
    }
}

/// `--render image-editor --image <png>`: the window on an image.
pub fn is_render_screen(name: &str) -> bool {
    name.starts_with("image-editor")
}

pub fn render_dialog(name: &str, o: &crate::dialogs::render::Options) -> Result<Box<dyn Dialog>, String> {
    let image = match &o.image {
        Some(path) => Some(image::open(path).map_err(|e| format!("{}: {e}", path.display()))?.to_rgba8()),
        None => None,
    };
    let mut task = TaskSettings::default();
    // `--files`: the recent files.
    if !o.files.is_empty() {
        let mut opts = crate::capture_job::image_editor_options(&task);
        opts.RecentImageFiles = o.files.iter().map(|f| f.display().to_string()).collect();
        task.ToolsSettings.get_or_insert_default().ImageEditorOptions = opts.to_value();
    }
    let mut w = EditorWindow::new(image, o.image.clone(), task, false, PathBuf::new());
    // `image-editor-context`: the context menu opened at 300,200.
    if name == "image-editor-file" {
        w.open_file_menu = true;
    }
    if name == "image-editor-context" {
        let at = pos2(300.0, 200.0);
        w.flyout = Some(EditorWindow::context_flyout(w.context_menu(), at));
    }
    // `--state`: the editor's dialogs.
    match o.state.as_deref() {
        Some("new") => w.modal = Some(Modal::NewImage(Box::default())),
        Some("new-solid") => w.modal = Some(Modal::NewImage(Box::new(new_image::NewImage { transparent: false, ..Default::default() }))),
        Some("new-done") => {
            w.modal = None;
            w.create_new_image(&new_image::NewImage::default());
            w.notify.settle();
        }
        Some("url") => {
            if let Some(Modal::Start(s)) = &mut w.modal {
                s.show_url_input(None);
            }
        }
        Some("status") => w.start_status(ie::EditorView_ClipboardDoesNotContainImage().to_string()),
        Some("none") => w.modal = None,
        Some("canvas-size") => w.canvas_resize_on_open = true,
        Some("confirm") => w.modal = Some(Modal::Confirm),
        Some("insert") => {
            let img = RgbaImage::from_pixel(200, 120, image::Rgba([30, 200, 90, 255]));
            w.modal = Some(Modal::InsertImage(Box::new(insert_image::InsertImage { image: img })));
        }
        Some(st) if st.starts_with("emoji") => {
            let mut d = emoji_picker::EmojiPicker::new(None);
            d.render_all_previews();
            if let Some(q) = st.strip_prefix("emoji:") {
                d.set_search(q);
            }
            w.modal = Some(Modal::Emoji(Box::new(d)));
        }
        _ => {}
    }
    Ok(Box::new(w))
}

impl crate::App {
    /// `ImageEditorIntegration.ShowEditorDialog(image, options, events,
    /// taskMode, filePath)`: the editor window for `task`; with `reply`,
    /// the task waits for its image (`taskMode`).
    pub(crate) fn open_image_editor(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        image: Option<RgbaImage>,
        file_path: Option<PathBuf>,
        task: TaskSettings,
        reply: Option<mpsc::Sender<Option<RgbaImage>>>,
    ) {
        let task_mode = reply.is_some();
        let parent = self.settings.paths.screenshots_parent_folder(Some(&self.settings.settings));
        let meta = crate::capture_job::Metadata {
            image: image.clone().unwrap_or_else(|| RgbaImage::new(1, 1)),
            window_title: None,
            process_name: None,
        };
        let folder = crate::capture_job::screenshots_folder(&task, &self.settings.settings, &parent, &meta);
        let mut w = EditorWindow::new(image, file_path, task, task_mode, folder);
        w.reply = reply;
        self.open_dialog_window(event_loop, Box::new(w));
    }

    /// `TaskHelpers.AnnotateImageFromFile(filePath, taskSettings)`.
    pub(crate) fn annotate_file(&mut self, event_loop: &winit::event_loop::ActiveEventLoop, path: &std::path::Path, task: Option<TaskSettings>) {
        let task = task.unwrap_or_else(|| TaskSettings::default_task_settings(&self.settings.settings.DefaultTaskSettings));
        match crate::thumbnail::load(path) {
            Some(img) => self.open_image_editor(event_loop, Some(img), Some(path.to_path_buf()), task, None),
            None => eprintln!("Image editor: couldn't load {}", path.display()),
        }
    }
}
