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
    /// The exit confirmation (`ConfirmationDialogView`) is open.
    confirm_exit: bool,
    /// Where things were drawn last frame.
    layout: Layout,
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
            confirm_exit: false,
            layout: Layout::default(),
        };
        w.title = w.window_title();
        w
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
                let img = self.snapshot();
                self.effects.push(Effect::CopyImage(img));
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
            Command::NewImage => self.effects.push(Effect::NotPorted("Image editor: New image dialog")),
            Command::OpenImage => self.open_image(),
            Command::PinToScreen => {
                let img = self.snapshot();
                self.effects.push(Effect::PinImage(img, Box::new(self.task.clone())));
            }
            Command::Upload => {
                let img = self.snapshot();
                self.effects.push(Effect::Upload(Box::new(UploadRequest::ProcessImage { image: img, task: self.task.clone() })));
            }
            Command::Print => self.effects.push(Effect::NotPorted("Image editor: Print")),
            Command::ToggleToolbars => self.toolbars_hidden = !self.toolbars_hidden,
            Command::Continue => self.finish(true),
            Command::Exit => {
                if self.task_mode {
                    self.finish(false);
                } else {
                    self.request_close();
                }
            }
        }
    }

    /// `RequestClose`: the confirmation when there are unsaved changes.
    fn request_close(&mut self) {
        if self.ws.tools.options.ShowExitConfirmation && self.ws.is_dirty() && self.has_image {
            self.confirm_exit = true;
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
                self.file_path = Some(path);
                self.ws.set_clean();
                self.update_title();
            }
            Err(e) => eprintln!("Image editor: couldn't save {}: {e}", path.display()),
        }
    }

    /// `OpenImageCommand`: an image file replaces the image.
    fn open_image(&mut self) {
        let Some(path) = rfd::FileDialog::new().add_filter("Images", &["png", "jpg", "jpeg", "bmp", "gif", "webp", "ico", "tif", "tiff"]).pick_file()
        else {
            return;
        };
        match crate::thumbnail::load(&path) {
            Some(img) => {
                self.load(img);
                self.file_path = Some(path);
                self.update_title();
            }
            None => eprintln!("Image editor: couldn't load {}", path.display()),
        }
    }

    /// `UpdatePreview`: a new image, no annotations, a new history.
    fn load(&mut self, img: RgbaImage) {
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
            MenuCmd::Undo => self.ws.undo(),
            MenuCmd::Redo => self.ws.redo(),
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
            Item::CanvasSize => self.effects.push(Effect::NotPorted("Image editor: Canvas size")),
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
                        _ => {
                            let c = self.visible_center();
                            self.ws.insert_image_centered(pixmap_of(&img), c);
                        }
                    }
                }
                ed::Request::EmojiPicker | ed::Request::ReplaceEmoji { .. } => {
                    self.effects.push(Effect::NotPorted("Image editor: emoji picker"));
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

    fn file_menu(&self) -> Vec<MenuEntry<MenuCmd>> {
        vec![
            MenuEntry::item(ie::AnnotationToolbar_NewEllipsis(), app_icons::FileAdd, MenuCmd::New).gesture("Ctrl+N"),
            MenuEntry::item(ie::AnnotationToolbar_OpenEllipsis(), app_icons::FolderOpen, MenuCmd::Open).gesture("Ctrl+O"),
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
        let left = (full.center().x - width / 2.0).round();
        let rect = Rect::from_min_size(pos2(left, full.min.y), vec2(width, BUTTON + 2.0 * (PADDING + 1.0)));
        let max_scroll = (natural - width).max(0.0);
        if ui.rect_contains_pointer(rect) {
            let d = ui.ctx().input(|i| i.smooth_scroll_delta);
            self.top_scroll = (self.top_scroll - d.x - d.y).clamp(0.0, max_scroll);
        }
        self.top_scroll = self.top_scroll.clamp(0.0, max_scroll);
        let painter = ui.painter();
        // `MainToolbarCornerRadius="0,0,4,4"`, a 1px border.
        let radius = CornerRadius { nw: 0, ne: 0, sw: 4, se: 4 };
        painter.rect_filled(rect, radius, p.background_toolbar);
        painter.rect_stroke(rect, radius, Stroke::new(1.0, p.border), StrokeKind::Inside);
        let clip = painter.with_clip_rect(rect.shrink(1.0));
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
            let hovered = ui.rect_contains_pointer(r.intersect(rect));
            if it == Item::File {
                captb::button(&clip, p, r, "", false, hovered, 1.0);
                // The 24px icon left, the 10px chevron right (padding 6).
                icons::paint(&clip, Rect::from_min_size(pos2(r.min.x + 7.0, r.center().y - 12.0), vec2(24.0, 24.0)), it.icon(), p.text);
                let open = self.flyout.is_some() && self.layout.top.is_some();
                let angle = if open { std::f32::consts::PI } else { 0.0 };
                icons::paint_transformed(&clip, Rect::from_min_size(pos2(r.max.x - 17.0, r.center().y - 5.0), vec2(10.0, 10.0)), app_icons::ChevronDown, p.text_secondary, Vec2::ZERO, angle);
            } else {
                let on = it.tool().is_some_and(|t| t == active);
                captb::button(&clip, p, r, it.icon(), on, hovered, 1.0);
            }
            self.layout.items.push((it, r.intersect(rect)));
        }
        // The `ScrollViewer`'s collapsed scroll bar when the items overflow.
        if max_scroll > 0.0 {
            let track = rect.shrink2(vec2(1.0, 0.0));
            let frac = width / natural;
            let tw = track.width() * frac;
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
        let zoom_w = (2.0 + 20.0 + 20.0 + 6.0 + widgets::text_width(ui, &zoom_text, font.clone()).ceil() + 10.0 + 10.0).max(80.0);
        // Items: zoom, separator, task grid (90 + margins 4/10), six
        // buttons, separator, two capture buttons.
        let sep = 1.0 + 8.0;
        let content = zoom_w + SPACING + sep + SPACING + 4.0 + 90.0 + 10.0 + SPACING + 6.0 * BUTTON + 5.0 * SPACING + SPACING + sep + SPACING + 2.0 * BUTTON + SPACING;
        let width = content + 2.0 * (PADDING + 1.0);
        let height = BUTTON + 2.0 * (PADDING + 1.0);
        let rect = Rect::from_min_size(pos2((full.center().x - width / 2.0).round(), full.max.y - height), vec2(width, height));
        let painter = ui.painter();
        let radius = CornerRadius { nw: 4, ne: 4, sw: 0, se: 0 };
        painter.rect_filled(rect, radius, p.background_toolbar);
        painter.rect_stroke(rect, radius, Stroke::new(1.0, p.border), StrokeKind::Inside);
        let top = rect.min.y + 1.0 + PADDING;
        let mut x = rect.min.x + 1.0 + PADDING;
        let mut items = Vec::new();
        let hover = |r: Rect| ui.rect_contains_pointer(r);
        // Zoom.
        let zr = Rect::from_min_size(pos2(x, top), vec2(zoom_w, BUTTON));
        captb::button(painter, p, zr, "", false, hover(zr), 1.0);
        icons::paint(painter, Rect::from_min_size(pos2(zr.min.x + 11.0, zr.center().y - 10.0), vec2(20.0, 20.0)), app_icons::ZoomIn, p.text_secondary);
        let g = widgets::galley(ui, &zoom_text, font.clone(), p.text, f32::INFINITY);
        let tw = widgets::text_width(ui, &zoom_text, font.clone());
        let mid = (zr.min.x + 11.0 + 26.0 + zr.max.x - 21.0) / 2.0;
        painter.galley(pos2((mid - tw / 2.0).round(), widgets::center_y(zr, widgets::line_height(ui, &font))), g, p.text);
        let angle = if self.zoom_open { std::f32::consts::PI } else { 0.0 };
        icons::paint_transformed(painter, Rect::from_min_size(pos2(zr.max.x - 21.0, zr.center().y - 5.0), vec2(10.0, 10.0)), app_icons::ChevronDown, p.text_secondary, Vec2::ZERO, angle);
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
            captb::button(painter, p, r, b.icon(), false, hover(r), 1.0);
            items.push((b, r));
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
        // Centred when it fits, scrolled when it doesn't.
        let max_scroll = vec2((size.x - canvas.width()).max(0.0), (size.y - canvas.height()).max(0.0));
        self.scroll = self.scroll.clamp(Vec2::ZERO, max_scroll);
        let min = pos2(
            if size.x <= canvas.width() { canvas.center().x - size.x / 2.0 } else { canvas.min.x - self.scroll.x },
            if size.y <= canvas.height() { canvas.center().y - size.y / 2.0 } else { canvas.min.y - self.scroll.y },
        );
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
        // `ShareX.Brush.Checkered` under the image (transparent pixels).
        painter.rect_filled(image, 0.0, p.checkered[0]);
        if let Some((_, t)) = &self.tex {
            painter.image(t.id(), image, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        }
        let m = self.mapper_ppp(ppp);
        let hovered = ctx.pointer_hover_pos().is_some_and(|p| self.layout.crop_button.is_some_and(|r| r.contains(p)));
        let chrome = editor_chrome::paint(ui, &painter, p, &m, &self.ws, hovered);
        self.layout.crop_button = chrome.crop_button;
    }

    /// The part of the window the toolbars, menus and popups cover.
    fn over_ui(&self, p: Pos2) -> bool {
        if self.popups.contains(p) || self.flyout.as_ref().is_some_and(|f| f.contains(p)) {
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
        let modal = self.confirm_exit;
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
                                self.flyout = Some(Flyout::menu(Id::new("editor-context"), self.context_menu(), Rect::from_min_size(pos, Vec2::ZERO), Placement::Pointer(pos)));
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
                        if pressed && key == egui::Key::Escape {
                            self.confirm_exit = false;
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
        let r = self.ws.key_down(k, mods, &Self::hotkey_tool);
        if let Some(c) = r.command {
            self.run(c);
        }
        self.handle_requests();
    }

    /// The toolbars' buttons (their `Click`s).
    fn toolbar_clicks(&mut self, ui: &Ui) {
        if self.toolbars_hidden {
            return;
        }
        for (it, r) in self.layout.items.clone() {
            if ui.interact(r, Id::new(("editor-item", format!("{it:?}"))), Sense::click()).clicked() {
                if it == Item::File {
                    let entries = self.file_menu();
                    self.flyout = Some(Flyout::menu(Id::new("editor-file"), entries, r, Placement::BottomEdgeAlignedLeft));
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
                    Bottom::Cancel => self.finish(false),
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

    /// `ConfirmationDialogView` over the `modal-scrim`: 400 wide, a title
    /// bar (the toolbar background, 15px), the message beside a 40px
    /// warning sign (25px padding), and Yes, No and Cancel at the right of
    /// a button bar.
    fn confirm_dialog(&mut self, ui: &Ui, p: &Palette, full: Rect) {
        let painter = ui.painter();
        painter.rect_filled(full, 0.0, p.overlay_modal);
        let w = 400.0;
        let title_font = widgets::font(15.0, 600);
        let font = widgets::font(14.0, 400);
        let title_h = 8.0 + widgets::line_height(ui, &title_font) + 8.0 + 1.0;
        let msg_w = w - 2.0 - 50.0 - 40.0 - 25.0;
        let lines = sharex_ui::controls::wrap_lines(ui, ie::ConfirmationDialogView_Message(), &font, msg_w);
        let msg_h = (widgets::line_height(ui, &font) * lines.len() as f32).max(54.0);
        let body_h = 25.0 + msg_h + 25.0;
        let bar_h = 1.0 + 10.0 + 32.0 + 10.0;
        let r = Rect::from_center_size(full.center(), vec2(w, title_h + body_h + bar_h + 2.0));
        let shadow = egui::epaint::Shadow { offset: [0, 4], blur: 10, spread: 0, color: Color32::from_black_alpha(0x40) };
        painter.add(shadow.as_shape(r, CornerRadius::same(4)));
        sharex_ui::border::border(painter, r, 4.0, 1.0, Some(sharex_ui::border::Brush::Solid(p.background_panel)), Some(p.border));
        let inner = r.shrink(1.0);
        let title = Rect::from_min_size(inner.min, vec2(inner.width(), title_h));
        painter.rect_filled(Rect::from_min_max(title.min, pos2(title.max.x, title.max.y - 1.0)), CornerRadius { nw: 3, ne: 3, sw: 0, se: 0 }, p.background_toolbar);
        painter.rect_filled(Rect::from_min_max(pos2(title.min.x, title.max.y - 1.0), title.max), 0.0, p.border);
        let g = widgets::galley(ui, ie::ConfirmationDialogView_Title(), title_font.clone(), p.text, f32::INFINITY);
        painter.galley(pos2(title.min.x + 14.0, title.min.y + 8.0), g, p.text);
        let body = Rect::from_min_size(pos2(inner.min.x, title.max.y), vec2(inner.width(), body_h));
        let sign = egui::FontId::proportional(40.0);
        let g = painter.layout_no_wrap("\u{26A0}\u{FE0F}".to_string(), sign, p.text);
        painter.galley(pos2(body.min.x + 25.0, body.center().y - g.size().y / 2.0), g, p.text);
        let mut y = body.center().y - widgets::line_height(ui, &font) * lines.len() as f32 / 2.0;
        for line in &lines {
            let g = widgets::galley(ui, line, font.clone(), p.text, f32::INFINITY);
            painter.galley(pos2(body.min.x + 25.0 + 40.0 + 25.0, y), g, p.text);
            y += widgets::line_height(ui, &font);
        }
        let bar = Rect::from_min_max(pos2(inner.min.x, body.max.y), inner.max);
        painter.rect_filled(bar, CornerRadius { nw: 0, ne: 0, sw: 3, se: 3 }, p.background_toolbar);
        painter.rect_filled(Rect::from_min_size(bar.min, vec2(bar.width(), 1.0)), 0.0, p.border);
        let labels = [(ie::ConfirmationDialogView_Yes(), 2u8), (ie::ConfirmationDialogView_No(), 1), (ie::ConfirmationDialogView_Cancel(), 0)];
        let b = sharex_ui::controls::Button { padding: sharex_ui::theme::Thickness::new(20.0, 0.0, 20.0, 0.0), ..Default::default() };
        let total: f32 = labels.iter().map(|(l, _)| b.width(ui, l)).sum::<f32>() + 10.0;
        let mut x = bar.max.x - 10.0 - total;
        for (label, k) in labels {
            let bw = b.width(ui, label);
            let br = Rect::from_min_size(pos2(x, bar.min.y + 11.0), vec2(bw, 32.0));
            x += bw + 5.0;
            if b.show(ui, br, Id::new(("confirm-exit", k)), label, true).clicked() {
                self.confirm_exit = false;
                match k {
                    2 => {
                        self.save(false);
                        if !self.ws.is_dirty() {
                            self.finish(false);
                        }
                    }
                    1 => self.finish(false),
                    _ => {}
                }
            }
        }
    }

    /// The options the tools changed, for the task settings.
    pub fn options(&self) -> ImageEditorOptions {
        self.ws.tools.options.clone()
    }
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
        ui.painter().rect_filled(full, 0.0, p.background_main);
        self.input(ui);
        self.paint_canvas(ui, &p, full);
        if !self.toolbars_hidden {
            self.paint_top(ui, &p, full);
            self.paint_bottom(ui, &p, full);
            self.toolbar_clicks(ui);
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
        if self.confirm_exit {
            self.confirm_dialog(ui, &p, full);
        }
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
        if self.ws.is_interacting() || self.pan.is_some() {
            ui.ctx().request_repaint();
        }
        std::mem::take(&mut self.effects)
    }

    fn close_requested(&mut self) -> Vec<Effect> {
        // `OnClosing`: `RequestClose(ignoreModal: true)`.
        self.request_close();
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

pub fn render_dialog(_name: &str, o: &crate::dialogs::render::Options) -> Result<Box<dyn Dialog>, String> {
    let path = o.image.clone().ok_or("--image <png> is required")?;
    let image = image::open(&path).map_err(|e| format!("{}: {e}", path.display()))?.to_rgba8();
    let w = EditorWindow::new(Some(image), Some(path), TaskSettings::default(), false, PathBuf::new());
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
