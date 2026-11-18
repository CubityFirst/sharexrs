//! The main window's task area (`ThumbnailPanel` in `MainWindow.axaml`):
//! the finished and running tasks as thumbnails (`TaskTile.thumbnail-card`)
//! or as a list (`TaskTile.task-row`, with its header, column resizers and
//! image preview pane), with `MainWindow.axaml.cs`'s selection, clicks,
//! keyboard and context menus (`BuildTaskContextMenu`).
//!
//! A row or card's text comes from `ThumbnailItemViewModel`
//! (`RefreshMetadata`, `RefreshListColumns`); its picture from
//! [`crate::thumbnail`], decoded off the UI thread ([`ImageRequest`]).

use std::path::PathBuf;

use egui::{Color32, CornerRadius, Event, Key, Modifiers, PointerButton, Pos2, Rect, Stroke, TextureHandle, Ui, Vec2, pos2, vec2};
use sharex_helpers::file_helpers::{is_image_file, is_text_file, is_video_file};
use sharex_settings::{ImagePreviewLocation, ImagePreviewVisibility, TaskViewMode, ThumbnailTitleLocation, ThumbnailViewClickAction, URLSharingServices, UrlShortenerType};
use sharex_strings::sharex as s;
use sharex_ui::icons;
use sharex_ui::keys::app_icons as icon;
use sharex_ui::menu::{Flyout, FlyoutOutput, MenuEntry, Placement};
use sharex_ui::theme::{FAILED, FONT_SIZE, palette};
use sharex_ui::widgets::{self, font};

use crate::tasks::{ImageKind, TaskManager, TaskStatus, UploadInfoStatus, WorkerTask};

/// `TaskListColumnLayout`: `MinColumnWidth`, `StatusIconWidth`, `DefaultWidths`.
const MIN_COLUMN_WIDTH: f32 = 24.0;
const STATUS_ICON_WIDTH: f32 = 22.0;
const DEFAULT_WIDTHS: [f32; 6] = [150.0, 65.0, 125.0, 80.0, 55.0, 70.0];
/// `Border.task-list-header` and `Border.task-row` heights.
const HEADER_HEIGHT: f32 = 24.0;
const ROW_HEIGHT: f32 = 22.0;
/// `MinImagePreviewWidth/Height`.
const MIN_PREVIEW: i32 = 100;
/// The thumbnail list's `Margin="6"`; a card's `Margin`, `BorderThickness`
/// and `Padding`; the `StackPanel`'s `Spacing`.
const LIST_MARGIN: f32 = 6.0;
const CARD_MARGIN: f32 = 4.0;
const CARD_BORDER: f32 = 1.0;
const CARD_PADDING: f32 = 4.0;
const CARD_SPACING: f32 = 3.0;
/// Windows' double-click time and size (`GetTapSize`, 4x4).
const DOUBLE_CLICK_SECONDS: f64 = 0.5;
const DOUBLE_CLICK_DISTANCE: f32 = 4.0;

/// The settings the task area reads and changes (`ApplicationConfig`).
#[derive(Clone, Debug, PartialEq)]
pub struct ViewSettings {
    pub mode: TaskViewMode,
    pub show_columns: bool,
    pub column_widths: Vec<i32>,
    pub image_preview: ImagePreviewVisibility,
    pub preview_location: ImagePreviewLocation,
    pub preview_width: i32,
    pub preview_height: i32,
    pub show_title: bool,
    pub title_location: ThumbnailTitleLocation,
    pub thumbnail_size: (i32, i32),
    pub click_action: ThumbnailViewClickAction,
    pub uploads_disabled: bool,
    /// `ClipboardContentFormats`' descriptions.
    pub clipboard_formats: Vec<String>,
    /// Whether any external program is set up (`ExternalPrograms`).
    pub external_programs: bool,
}

impl Default for ViewSettings {
    fn default() -> Self {
        Self {
            mode: TaskViewMode::ThumbnailView,
            show_columns: true,
            column_widths: Vec::new(),
            image_preview: ImagePreviewVisibility::Automatic,
            preview_location: ImagePreviewLocation::Side,
            preview_width: 300,
            preview_height: 250,
            show_title: true,
            title_location: ThumbnailTitleLocation::Top,
            thumbnail_size: (200, 150),
            click_action: ThumbnailViewClickAction::Default,
            uploads_disabled: false,
            clipboard_formats: Vec::new(),
            external_programs: false,
        }
    }
}

impl ViewSettings {
    fn is_list(&self) -> bool {
        self.mode == TaskViewMode::ListView
    }

    /// `ThumbnailItemViewModel.Width/Height`.
    fn thumb_w(&self) -> f32 {
        self.thumbnail_size.0.max(96) as f32
    }
    fn thumb_h(&self) -> f32 {
        self.thumbnail_size.1.max(72) as f32
    }

    /// `TaskListColumnLayout.Load`.
    fn columns(&self) -> [f32; 6] {
        let mut w = DEFAULT_WIDTHS;
        for (i, c) in w.iter_mut().enumerate() {
            if let Some(&v) = self.column_widths.get(i).filter(|v| **v > 0) {
                *c = (v as f32).max(MIN_COLUMN_WIDTH);
            }
        }
        w
    }
}

/// A context menu command (`BuildTaskContextMenu` and its submenus).
#[derive(Clone, Debug, PartialEq)]
pub enum TaskCommand {
    /// `UploadInfoManager.OCRImage`.
    OcrImage,
    OpenFile,
    OpenFolder,
    CopyFile,
    CopyImage,
    CopyImageDimensions,
    CopyText,
    CopyFilePath,
    CopyFileName,
    CopyFileNameWithExtension,
    CopyFolder,
    /// `TryOpen` (Enter, a row's double click).
    TryOpen,
    /// `TryCopy` (Ctrl+C).
    TryCopy,
    RemoveSelected,
    DeleteSelectedFiles,
    /// `StopUpload`: `WorkerTask.Stop` on the selected tasks.
    StopUpload,
    /// `OpenURL`, `OpenShortenedURL`, `OpenThumbnailURL`, `OpenDeletionURL`.
    OpenUrl(UrlField),
    /// `CopyURL` and the like: every selected task's.
    CopyUrl(UrlField),
    /// `CopyHTMLLink` and the like: an `UploadInfoParser` format of the
    /// selected tasks that pass the filter.
    CopyFormat(&'static str, FormatFilter),
    /// `CopyCustomFormat`: the custom clipboard format at this index.
    CopyCustomFormat(usize),
    OpenThumbnailFile,
    CopyThumbnailFile,
    CopyThumbnailImage,
    /// `UploadInfoManager.Upload`: the file again.
    Upload,
    /// `UploadInfoManager.Download`.
    Download,
    /// `ShortenURL(urlShortener)`.
    ShortenUrl(UrlShortenerType),
    /// `ShareURL(urlSharingService)`.
    ShareUrl(URLSharingServices),
    /// `SearchImageUsingGoogleLens` / `SearchImageUsingBing`.
    SearchImage(URLSharingServices),
    /// `ShowQRCode`.
    ShowQrCode,
    /// `ShowResponse`: the response window.
    ShowResponse,
    /// `ShowErrors`: the error window.
    ShowErrors,
    Clear,
    SwitchView,
    /// An entry whose tool isn't ported yet (named for the log).
    NotPorted(&'static str),
}

/// Which of an upload's URLs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UrlField {
    Url,
    Shortened,
    Thumbnail,
    Deletion,
}

/// Which selected tasks a copied format takes (`IsURLExist`,
/// `IsImageURL`, `IsImageURL && IsThumbnailURLExist`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormatFilter {
    Url,
    Image,
    LinkedImage,
}

/// `UploadInfoParser`'s formats.
pub mod link_formats {
    pub const HTML_LINK: &str = "<a href=\"$url\">$url</a>";
    pub const HTML_IMAGE: &str = "<img src=\"$url\">";
    pub const HTML_LINKED_IMAGE: &str = "<a href=\"$url\"><img src=\"$thumbnailurl\"></a>";
    pub const FORUM_LINK: &str = "[url]$url[/url]";
    pub const FORUM_IMAGE: &str = "[img]$url[/img]";
    pub const FORUM_LINKED_IMAGE: &str = "[url=$url][img]$thumbnailurl[/img][/url]";
    pub const MARKDOWN_LINK: &str = "[$url]($url)";
    pub const MARKDOWN_IMAGE: &str = "![]($url)";
    pub const MARKDOWN_LINKED_IMAGE: &str = "[![]($thumbnailurl)]($url)";
}

/// What the task area asks the app to do.
#[derive(Clone, Debug, PartialEq)]
pub enum TaskAction {
    /// Run a command on these tasks (the selection, in the order it was made).
    Command(TaskCommand, Vec<u64>),
    /// `ShowImagesInViewer`: the image viewer isn't ported yet.
    OpenImageViewer(PathBuf),
    /// Open a file (a thumbnail title's click, `ExecuteThumbnailClick`).
    OpenFile(PathBuf),
    /// Escape: `Close()`.
    CloseWindow,
    /// The view settings changed; save them.
    SettingsChanged,
}

/// A picture to decode off the UI thread.
#[derive(Clone, Debug)]
pub struct ImageRequest {
    pub id: u64,
    pub version: u64,
    pub kind: ImageKind,
    pub path: PathBuf,
    /// The box, in physical pixels.
    pub size: [u32; 2],
    /// The box's corner radius, in physical pixels.
    pub radius: f32,
}

impl ImageRequest {
    /// Decodes and draws the picture into its box ([`crate::thumbnail`]).
    pub fn run(&self) -> Option<image::RgbaImage> {
        let img = crate::thumbnail::load(&self.path)?;
        let [w, h] = self.size;
        Some(match self.kind {
            ImageKind::Thumbnail => {
                let fit = crate::thumbnail::decode_to_fit(&img, w, h);
                crate::thumbnail::fit(&fit, w, h, crate::thumbnail::Stretch::Uniform, self.radius)
            }
            ImageKind::Preview => crate::thumbnail::fit(&img, w, h, crate::thumbnail::Stretch::UniformDownOnly, self.radius),
        })
    }
}

/// One `ThumbnailItemViewModel`.
struct Item {
    id: u64,
    selected: bool,
    /// The decoded thumbnail and the request it answers.
    thumb: Option<TextureHandle>,
    thumb_key: Option<(PathBuf, [u32; 2])>,
    version: u64,
}

/// The list view's preview pane picture.
#[derive(Default)]
struct Preview {
    key: Option<(PathBuf, [u32; 2])>,
    version: u64,
    texture: Option<TextureHandle>,
}

/// A drag in progress on a column or the preview's resizer.
#[derive(Clone, Copy)]
enum Resize {
    Column { index: usize, start_x: f32, start_width: f32 },
    Preview { start: f32, start_size: i32 },
}

pub struct TaskView {
    pub manager: TaskManager,
    pub settings: ViewSettings,
    items: Vec<Item>,
    /// `_selectionOrder` and `_selectionAnchor`.
    selection_order: Vec<u64>,
    anchor: Option<u64>,
    suppress_click: bool,
    /// `TaskListColumnLayout`'s widths, while they're dragged.
    columns: [f32; 6],
    preview: Preview,
    /// The task context menu (or the empty area's), if open.
    pub flyout: Option<Flyout<TaskCommand>>,
    /// Where menus must stay (the work area); `None` keeps them in the window.
    pub work_area: Option<Rect>,
    /// Keyboard focus is in the task area (`ThumbnailPanel.IsKeyboardFocusWithin`).
    focused: bool,
    scroll: f32,
    resize: Option<Resize>,
    /// The last left press: when, where, on which task (double clicks).
    last_press: Option<(f64, Pos2, Option<u64>)>,
    /// A right press on a task or the empty area, waiting for its release.
    right_pressed: Option<Option<u64>>,
    /// Pictures to decode, taken by the app.
    pub requests: Vec<ImageRequest>,
    /// Key presses egui turns into clipboard events (C and V with Ctrl,
    /// Delete with Shift), with their modifiers, given by the app.
    pub clipboard_keys: Vec<(Key, Modifiers)>,
    next_version: u64,
    /// Rects laid out last frame, for hit testing.
    layout: Layout,
}

#[derive(Clone)]
struct Layout {
    area: Rect,
    /// Each task's card or row and, for cards, its title and preview.
    hits: Vec<(u64, Rect, Option<Rect>)>,
    list: Rect,
    header: Option<Rect>,
    preview_pane: Option<Rect>,
    preview_image: Option<Rect>,
    preview_resizer: Option<Rect>,
    column_resizers: Vec<Rect>,
    content_height: f32,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            area: Rect::NOTHING,
            hits: Vec::new(),
            list: Rect::NOTHING,
            header: None,
            preview_pane: None,
            preview_image: None,
            preview_resizer: None,
            column_resizers: Vec::new(),
            content_height: 0.0,
        }
    }
}

impl Default for TaskView {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskView {
    pub fn new() -> Self {
        Self {
            manager: TaskManager::default(),
            settings: ViewSettings::default(),
            items: Vec::new(),
            selection_order: Vec::new(),
            anchor: None,
            suppress_click: false,
            columns: DEFAULT_WIDTHS,
            preview: Preview::default(),
            flyout: None,
            work_area: None,
            focused: false,
            scroll: 0.0,
            resize: None,
            last_press: None,
            right_pressed: None,
            requests: Vec::new(),
            clipboard_keys: Vec::new(),
            next_version: 0,
            layout: Layout::default(),
        }
    }

    /// Applies new settings (`TaskListColumns.Load` and the view mode).
    pub fn set_settings(&mut self, settings: ViewSettings) {
        self.columns = settings.columns();
        self.settings = settings;
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// `MainWindow.AddTask`: newest first.
    pub fn add_task(&mut self, id: u64) {
        if self.items.iter().any(|i| i.id == id) {
            return;
        }
        self.items.insert(0, Item { id, selected: false, thumb: None, thumb_key: None, version: 0 });
    }

    /// `OnTaskChanged`: the item refreshes (its thumbnail too, from the file).
    pub fn task_changed(&mut self, id: u64) {
        if let Some(item) = self.items.iter_mut().find(|i| i.id == id) {
            item.thumb_key = None;
        }
        self.preview.key = None;
    }

    /// `OnTaskRemoved`.
    fn remove_item(&mut self, id: u64) {
        self.selection_order.retain(|i| *i != id);
        if self.anchor == Some(id) {
            self.anchor = None;
        }
        self.items.retain(|i| i.id != id);
    }

    /// A decoded picture arrived.
    pub fn image_ready(&mut self, ctx: &egui::Context, id: u64, version: u64, kind: ImageKind, image: Option<image::RgbaImage>) {
        let texture = image.map(|img| {
            ctx.load_texture(format!("task-{kind:?}-{id}-{version}"), crate::thumbnail::to_color_image(&img), egui::TextureOptions::NEAREST)
        });
        match kind {
            ImageKind::Thumbnail => {
                if let Some(item) = self.items.iter_mut().find(|i| i.id == id && i.version == version) {
                    item.thumb = texture;
                }
            }
            ImageKind::Preview => {
                if self.preview.version == version {
                    self.preview.texture = texture;
                }
            }
        }
    }

    /// `GetSelectedItems`: in the order they were selected.
    pub fn selected(&mut self) -> Vec<u64> {
        let items = &self.items;
        self.selection_order.retain(|id| items.iter().any(|i| i.id == *id && i.selected));
        for i in &self.items {
            if i.selected && !self.selection_order.contains(&i.id) {
                self.selection_order.push(i.id);
            }
        }
        self.selection_order.clone()
    }

    fn set_selected(&mut self, id: u64, selected: bool) {
        if let Some(i) = self.items.iter_mut().find(|i| i.id == id) {
            i.selected = selected;
        }
        if selected {
            if !self.selection_order.contains(&id) {
                self.selection_order.push(id);
            }
        } else {
            self.selection_order.retain(|i| *i != id);
        }
    }

    fn clear_selection(&mut self) {
        for i in &mut self.items {
            i.selected = false;
        }
        self.selection_order.clear();
    }

    /// `SelectOnly`.
    pub fn select_only(&mut self, id: u64) {
        self.clear_selection();
        self.set_selected(id, true);
        self.anchor = Some(id);
    }

    /// Adds a task to the selection (as a Ctrl click does).
    pub fn add_to_selection(&mut self, id: u64) {
        self.set_selected(id, true);
        self.anchor = Some(id);
    }

    fn index_of(&self, id: u64) -> Option<usize> {
        self.items.iter().position(|i| i.id == id)
    }

    fn task(&self, id: u64) -> Option<&WorkerTask> {
        self.manager.get(id)
    }

    /// `RemoveSelectedTasks`.
    pub fn remove_tasks(&mut self, ids: &[u64]) {
        for id in ids {
            self.manager.remove(*id);
            self.remove_item(*id);
        }
    }

    /// `ClearTasks`.
    pub fn clear(&mut self) {
        self.manager.clear();
        self.items.clear();
        self.selection_order.clear();
        self.anchor = None;
        self.scroll = 0.0;
    }

    /// `SwitchTaskViewMode`.
    pub fn switch_view(&mut self) {
        self.settings.mode = if self.settings.is_list() { TaskViewMode::ThumbnailView } else { TaskViewMode::ListView };
        self.scroll = 0.0;
        // `RefreshSettings`: the list view drops decoded previews.
        if self.settings.is_list() {
            for i in &mut self.items {
                i.thumb = None;
                i.thumb_key = None;
            }
        }
    }

    // ---- drawing ------------------------------------------------------

    /// Draws the task area in `area` and handles its input. `menus_open`:
    /// a sidebar menu is open (it has the keyboard and the pointer).
    pub fn ui(&mut self, ui: &mut Ui, area: Rect, menus_open: bool) -> Vec<TaskAction> {
        let mut actions = Vec::new();
        let flyout_open = menus_open || self.flyout.is_some();
        self.layout = self.lay_out(ui, area);
        self.request_images(ui);

        if !flyout_open {
            self.pointer_input(ui, &mut actions);
            self.keyboard_input(ui, &mut actions);
        } else {
            // A menu has the keyboard.
            self.clipboard_keys.clear();
        }

        let painter = ui.painter().with_clip_rect(area);
        if self.settings.is_list() {
            self.paint_list(ui, &painter);
        } else {
            self.paint_thumbnails(ui, &painter);
        }

        if let Some(flyout) = &mut self.flyout {
            let bounds = self.work_area.unwrap_or(ui.max_rect());
            let FlyoutOutput { command, closed } = flyout.show(ui, bounds);
            if closed {
                self.flyout = None;
            }
            if let Some(command) = command {
                let selected = self.selected();
                actions.extend(self.run_command(command, selected));
            }
        }
        actions
    }

    /// Commands the task area carries out itself; the rest go to the app.
    pub fn run_command(&mut self, command: TaskCommand, selected: Vec<u64>) -> Vec<TaskAction> {
        match command {
            TaskCommand::RemoveSelected => {
                self.remove_tasks(&selected);
                vec![TaskAction::SettingsChanged]
            }
            TaskCommand::Clear => {
                self.clear();
                vec![TaskAction::SettingsChanged]
            }
            TaskCommand::SwitchView => {
                self.switch_view();
                vec![TaskAction::SettingsChanged]
            }
            other => vec![TaskAction::Command(other, selected)],
        }
    }

    fn line_h(ui: &Ui) -> f32 {
        widgets::line_height(ui, &font(FONT_SIZE, 400))
    }

    /// The card's size with its margin (`TaskTile.thumbnail-card`).
    fn card_size(&self, ui: &Ui) -> Vec2 {
        let s = &self.settings;
        let title = if s.show_title { Self::line_h(ui) + CARD_SPACING } else { 0.0 };
        let edge = 2.0 * (CARD_MARGIN + CARD_BORDER + CARD_PADDING);
        vec2(s.thumb_w() + edge, s.thumb_h() + title + edge)
    }

    fn preview_visible(&self) -> bool {
        if !self.settings.is_list() || self.items.is_empty() {
            return false;
        }
        match self.settings.image_preview {
            ImagePreviewVisibility::Show => true,
            ImagePreviewVisibility::Hide => false,
            _ => self.preview_path().is_some(),
        }
    }

    /// `UpdateImagePreview`: the first selected task's image, if it exists.
    fn preview_path(&self) -> Option<PathBuf> {
        if !self.settings.is_list() || self.settings.image_preview == ImagePreviewVisibility::Hide {
            return None;
        }
        let first = self.selection_order.iter().find(|id| self.items.iter().any(|i| i.id == **id && i.selected))?;
        let path = self.task(*first)?.info.file_path.clone()?;
        (is_image_file(&path.to_string_lossy()) && path.is_file()).then_some(path)
    }

    fn lay_out(&mut self, ui: &Ui, area: Rect) -> Layout {
        let mut l = Layout { area, ..Default::default() };
        if self.settings.is_list() {
            let mut list = area;
            if self.preview_visible() {
                if self.settings.preview_location == ImagePreviewLocation::Bottom {
                    let max = MIN_PREVIEW.max(area.height() as i32 - 100);
                    let h = self.settings.preview_height.clamp(MIN_PREVIEW, max) as f32;
                    let pane = Rect::from_min_max(pos2(area.min.x, area.max.y - h), area.max);
                    l.preview_resizer = Some(Rect::from_min_size(pos2(pane.min.x, pane.min.y - 3.0), vec2(pane.width(), 6.0)));
                    list.max.y = pane.min.y;
                    l.preview_pane = Some(pane);
                } else {
                    let max = MIN_PREVIEW.max(area.width() as i32 - 200);
                    let w = self.settings.preview_width.clamp(MIN_PREVIEW, max) as f32;
                    let pane = Rect::from_min_max(pos2(area.max.x - w, area.min.y), area.max);
                    l.preview_resizer = Some(Rect::from_min_size(pos2(pane.min.x - 3.0, pane.min.y), vec2(6.0, pane.height())));
                    list.max.x = pane.min.x;
                    l.preview_pane = Some(pane);
                }
                if let Some(pane) = l.preview_pane {
                    // `Border.image-preview-frame` (1px on the list's side), then a 6px margin.
                    let frame = if self.settings.preview_location == ImagePreviewLocation::Bottom {
                        Rect::from_min_max(pos2(pane.min.x, pane.min.y + 1.0), pane.max)
                    } else {
                        Rect::from_min_max(pos2(pane.min.x + 1.0, pane.min.y), pane.max)
                    };
                    l.preview_image = Some(frame.shrink(6.0));
                }
            }
            let mut top = list.min.y;
            if self.settings.show_columns {
                let header = Rect::from_min_size(list.min, vec2(list.width(), HEADER_HEIGHT));
                let mut x = list.min.x + STATUS_ICON_WIDTH;
                for w in self.columns {
                    x += w;
                    // `Thumb.column-resizer`: 7px, right-aligned with -4px margin.
                    l.column_resizers.push(Rect::from_min_max(pos2(x - 3.0, header.min.y), pos2(x + 4.0, header.max.y)));
                }
                l.header = Some(header);
                top += HEADER_HEIGHT;
            }
            l.list = Rect::from_min_max(pos2(list.min.x, top), list.max);
            l.content_height = self.items.len() as f32 * ROW_HEIGHT;
            let max_scroll = (l.content_height - l.list.height()).max(0.0);
            self.scroll = self.scroll.clamp(0.0, max_scroll);
            for (i, item) in self.items.iter().enumerate() {
                let y = l.list.min.y + i as f32 * ROW_HEIGHT - self.scroll;
                l.hits.push((item.id, Rect::from_min_size(pos2(l.list.min.x, y), vec2(l.list.width(), ROW_HEIGHT)), None));
            }
        } else {
            l.list = area;
            let card = self.card_size(ui);
            let inner_w = (area.width() - 2.0 * LIST_MARGIN).max(card.x);
            let per_row = ((inner_w / card.x).floor() as usize).max(1);
            let rows = self.items.len().div_ceil(per_row);
            l.content_height = 2.0 * LIST_MARGIN + rows as f32 * card.y;
            let max_scroll = (l.content_height - area.height()).max(0.0);
            self.scroll = self.scroll.clamp(0.0, max_scroll);
            let s = &self.settings;
            let title_h = Self::line_h(ui);
            for (i, item) in self.items.iter().enumerate() {
                let (row, col) = (i / per_row, i % per_row);
                let outer = Rect::from_min_size(
                    pos2(area.min.x + LIST_MARGIN + col as f32 * card.x, area.min.y + LIST_MARGIN + row as f32 * card.y - self.scroll),
                    card,
                );
                let border = outer.shrink(CARD_MARGIN);
                let content = border.shrink(CARD_BORDER + CARD_PADDING);
                let title_above = s.show_title && s.title_location == ThumbnailTitleLocation::Top;
                let preview_top = if title_above { content.min.y + title_h + CARD_SPACING } else { content.min.y };
                let preview = Rect::from_min_size(pos2(content.min.x, preview_top), vec2(s.thumb_w(), s.thumb_h()));
                l.hits.push((item.id, border, Some(preview)));
            }
        }
        l
    }

    /// The title's rect for a card (above or below its picture).
    fn title_rect(&self, ui: &Ui, border: Rect, preview: Rect) -> Option<Rect> {
        let s = &self.settings;
        if !s.show_title {
            return None;
        }
        let h = Self::line_h(ui);
        let content = border.shrink(CARD_BORDER + CARD_PADDING);
        Some(if s.title_location == ThumbnailTitleLocation::Top {
            Rect::from_min_size(content.min, vec2(s.thumb_w(), h))
        } else {
            Rect::from_min_size(pos2(content.min.x, preview.max.y + CARD_SPACING), vec2(s.thumb_w(), h))
        })
    }

    /// Asks for the pictures that aren't decoded yet (`Refresh`).
    fn request_images(&mut self, ui: &Ui) {
        let ppp = ui.ctx().pixels_per_point();
        if !self.settings.is_list() {
            let size = [(self.settings.thumb_w() * ppp).ceil() as u32, (self.settings.thumb_h() * ppp).ceil() as u32];
            for idx in 0..self.items.len() {
                let id = self.items[idx].id;
                let Some(path) = self.task(id).and_then(|t| t.info.file_path.clone()) else { continue };
                let p = path.to_string_lossy();
                if !(is_image_file(&p) || is_video_file(&p)) || !path.is_file() {
                    continue;
                }
                let key = Some((path.clone(), size));
                if self.items[idx].thumb_key == key {
                    continue;
                }
                self.next_version += 1;
                let item = &mut self.items[idx];
                item.thumb_key = key;
                item.version = self.next_version;
                if is_image_file(&p) {
                    self.requests.push(ImageRequest {
                        id,
                        version: item.version,
                        kind: ImageKind::Thumbnail,
                        path,
                        size,
                        radius: 4.0 * ppp,
                    });
                }
            }
        }
        let key = self.preview_path().zip(self.layout.preview_image).map(|(p, r)| {
            let inner = r.shrink(1.0);
            (p, [(inner.width() * ppp).round().max(1.0) as u32, (inner.height() * ppp).round().max(1.0) as u32])
        });
        if key != self.preview.key {
            if key.as_ref().map(|k| &k.0) != self.preview.key.as_ref().map(|k| &k.0) {
                self.preview.texture = None;
            }
            self.preview.key = key.clone();
            self.next_version += 1;
            self.preview.version = self.next_version;
            if let Some((path, size)) = key {
                self.requests.push(ImageRequest {
                    id: 0,
                    version: self.preview.version,
                    kind: ImageKind::Preview,
                    path,
                    size,
                    radius: 3.0 * ppp,
                });
            }
        }
    }

    /// Decodes every pending picture now (for `--render`).
    pub fn load_images_now(&mut self, ctx: &egui::Context) {
        for r in std::mem::take(&mut self.requests) {
            let img = r.run();
            self.image_ready(ctx, r.id, r.version, r.kind, img);
        }
    }

    fn hovered_task(&self, pos: Pos2) -> Option<u64> {
        if !self.layout.list.contains(pos) {
            return None;
        }
        self.layout.hits.iter().find(|(_, r, _)| r.contains(pos)).map(|(id, ..)| *id)
    }

    fn paint_thumbnails(&self, ui: &Ui, painter: &egui::Painter) {
        let p = palette(ui.ctx());
        let pointer = ui.ctx().pointer_latest_pos().filter(|pos| self.layout.area.contains(*pos));
        let hover_enabled = self.flyout.is_none();
        let title_font = font(FONT_SIZE, 400);
        for (id, border, preview) in &self.layout.hits {
            let Some(preview) = *preview else { continue };
            if !border.intersects(self.layout.area) {
                continue;
            }
            let Some(item) = self.items.iter().find(|i| i.id == *id) else { continue };
            let Some(task) = self.task(*id) else { continue };
            let hovered = hover_enabled && pointer.is_some_and(|pos| border.contains(pos));
            let (fill, stroke) = if hovered || item.selected {
                (p.control_background_hover, p.accent_border)
            } else {
                (p.control_background, p.border)
            };
            painter.rect(*border, CornerRadius::same(6), fill, Stroke::new(CARD_BORDER, stroke), egui::StrokeKind::Inside);

            if let Some(title) = self.title_rect(ui, *border, preview) {
                let text = title_text(task);
                let g = widgets::galley(ui, &text, title_font.clone(), p.text, title.width());
                let x = title.min.x + ((title.width() - g.size().x) / 2.0).round();
                painter.with_clip_rect(title.intersect(self.layout.area)).galley(pos2(x, title.min.y), g, p.text);
            }

            // `Border.thumbnail-preview`: the panel colour, rounded 4, clipped.
            painter.rect_filled(preview, CornerRadius::same(4), p.background_panel);
            let path = task.info.file_path_str();
            match &item.thumb {
                Some(tex) => {
                    painter.image(tex.id(), preview, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                    // `ShowPlayIcon`: a video's thumbnail.
                    if is_video_file(&path) {
                        let c = preview.center();
                        painter.circle_filled(c, 25.0, Color32::from_rgba_unmultiplied(0x18, 0x18, 0x18, 0xB8));
                        icons::paint(painter, Rect::from_center_size(c, Vec2::splat(18.0)), icon::Play, Color32::from_rgb(0xA9, 0xA9, 0xA9));
                    }
                }
                None => {
                    // `PlaceholderIcon` at 42px, 45% opaque.
                    let key = placeholder_icon(&if path.is_empty() { task.info.file_name.clone() } else { path });
                    let r = Rect::from_center_size(preview.center(), Vec2::splat(42.0));
                    icons::paint(painter, r, key, p.text.gamma_multiply(0.45));
                }
            }
            // `ProgressBar` (`Height="5"`, at the bottom) while an upload
            // reports progress: Fluent's track (base low) and the accent.
            if task.is_working()
                && let Some(progress) = task.info.progress
            {
                let f = sharex_ui::theme::fluent(ui.ctx());
                let bar = Rect::from_min_max(pos2(preview.min.x, preview.max.y - 5.0), preview.max);
                sharex_ui::border::border(painter, bar, 2.5, 0.0, Some(sharex_ui::border::Brush::Solid(f.base_low)), None);
                let done = (bar.width() * (progress.percentage() as f32 / 100.0).clamp(0.0, 1.0)).round();
                if done > 0.0 {
                    let fill = Rect::from_min_size(bar.min, vec2(done, bar.height()));
                    sharex_ui::border::border(painter, fill, 2.5, 0.0, Some(sharex_ui::border::Brush::Solid(p.accent_start)), None);
                }
            }
            if task.status == TaskStatus::Failed {
                // A red badge with a warning icon, top right (Margin 5, Padding 6,3).
                let size = vec2(6.0 + FONT_SIZE + 6.0, 3.0 + FONT_SIZE + 3.0);
                let badge = Rect::from_min_size(pos2(preview.max.x - 5.0 - size.x, preview.min.y + 5.0), size);
                painter.rect_filled(badge, CornerRadius::same(4), Color32::from_rgba_unmultiplied(0xB9, 0x1C, 0x1C, 0xC8));
                icons::paint(painter, Rect::from_center_size(badge.center(), Vec2::splat(FONT_SIZE)), icon::Warning, Color32::WHITE);
            }
        }
        self.paint_scroll_indicator(ui, painter);
    }

    fn paint_list(&self, ui: &Ui, painter: &egui::Painter) {
        let p = palette(ui.ctx());
        let l = &self.layout;
        let pointer = ui.ctx().pointer_latest_pos().filter(|pos| l.list.contains(*pos));
        let hover_enabled = self.flyout.is_none();
        let cell_font = font(12.0, 400);
        let cell_h = widgets::line_height(ui, &cell_font);
        let min_row = STATUS_ICON_WIDTH + self.columns.iter().sum::<f32>();

        if let Some(header) = l.header {
            // `Border.task-list-header`: toolbar colour, a separator line below.
            painter.rect_filled(header, 0.0, p.background_toolbar);
            painter.rect_filled(Rect::from_min_max(pos2(header.min.x, header.max.y - 1.0), header.max), 0.0, p.separator);
            let inner = Rect::from_min_max(header.min, pos2(header.max.x, header.max.y - 1.0));
            let hp = painter.with_clip_rect(header.intersect(painter.clip_rect()));
            let head_font = font(12.0, 600);
            let head_h = widgets::line_height(ui, &head_font);
            let titles = [
                s::TaskListView_Filename(),
                s::TaskListView_Status(),
                s::TaskListView_Progress(),
                s::TaskListView_Speed(),
                s::TaskListView_Elapsed(),
                s::TaskListView_Remaining(),
            ];
            let mut x = inner.min.x + STATUS_ICON_WIDTH;
            for (i, w) in self.columns.iter().enumerate() {
                cell_text(ui, &hp, titles[i], head_font.clone(), p.text_secondary, x, *w, inner, head_h);
                x += w;
            }
            let url_w = (inner.max.x - x).max(0.0);
            cell_text(ui, &hp, s::TaskListView_URL(), head_font, p.text_secondary, x, url_w, inner, head_h);
            // The resizers' 1px lines, inset 5px top and bottom.
            for r in &l.column_resizers {
                let line = Rect::from_min_max(pos2(r.min.x + 3.0, inner.min.y + 5.0), pos2(r.min.x + 4.0, inner.max.y - 5.0));
                hp.rect_filled(line, 0.0, p.separator);
            }
        }

        let rows = painter.with_clip_rect(l.list.intersect(painter.clip_rect()));
        for (id, row, _) in &l.hits {
            if !row.intersects(l.list) {
                continue;
            }
            let Some(item) = self.items.iter().find(|i| i.id == *id) else { continue };
            let Some(task) = self.task(*id) else { continue };
            let hovered = hover_enabled && pointer.is_some_and(|pos| row.contains(pos));
            if item.selected {
                rows.rect_filled(*row, 0.0, p.accent_start);
            } else if hovered {
                rows.rect_filled(*row, 0.0, p.control_background_hover);
            }
            let text_color = if item.selected { p.accent_foreground } else { p.text };
            let (status_icon, status_color) = list_status_icon(task);
            let icon_color = if item.selected {
                p.accent_foreground
            } else if task.status == TaskStatus::Failed {
                status_color
            } else {
                p.text_secondary
            };
            // `IconPresenter.task-status`: 13px in a 22px column.
            let ir = Rect::from_min_size(
                pos2(row.min.x + ((STATUS_ICON_WIDTH - 13.0) / 2.0).round_ties_even(), widgets::center_y(*row, 13.0)),
                Vec2::splat(13.0),
            );
            icons::paint(&rows, ir, status_icon, icon_color);
            let cells = list_cells(task);
            let mut x = row.min.x + STATUS_ICON_WIDTH;
            for (i, w) in self.columns.iter().enumerate() {
                cell_text(ui, &rows, &cells[i], cell_font.clone(), text_color, x, *w, *row, cell_h);
                x += w;
            }
            let url_w = (row.max.x.max(row.min.x + min_row) - x).max(0.0);
            cell_text(ui, &rows, &cells[6], cell_font.clone(), text_color, x, url_w, *row, cell_h);
        }

        if let (Some(pane), Some(image)) = (l.preview_pane, l.preview_image) {
            // `Border.image-preview-frame`.
            painter.rect_filled(pane, 0.0, p.background_panel);
            let line = if self.settings.preview_location == ImagePreviewLocation::Bottom {
                Rect::from_min_size(pane.min, vec2(pane.width(), 1.0))
            } else {
                Rect::from_min_size(pane.min, vec2(1.0, pane.height()))
            };
            painter.rect_filled(line, 0.0, p.separator);
            if self.preview.texture.is_some() || self.preview_path().is_some() {
                // The checkered box, rounded 4, with a 1px border.
                paint_checkered(painter, image, p.checkered);
                painter.rect_stroke(image, CornerRadius::same(4), Stroke::new(1.0, p.border), egui::StrokeKind::Inside);
                if let Some(tex) = &self.preview.texture {
                    let inner = image.shrink(1.0);
                    painter.image(tex.id(), inner, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                }
            }
        }
        self.paint_scroll_indicator(ui, painter);
    }

    /// The scroll bar's thumb while the content overflows (Fluent's
    /// collapsed `ScrollBar`: a thin line at the right edge).
    fn paint_scroll_indicator(&self, ui: &Ui, painter: &egui::Painter) {
        let l = &self.layout;
        let view = if self.settings.is_list() { l.list } else { l.area };
        if l.content_height <= view.height() + 0.5 {
            return;
        }
        let p = palette(ui.ctx());
        let track = Rect::from_min_max(pos2(view.max.x - 4.0, view.min.y + 2.0), pos2(view.max.x - 2.0, view.max.y - 2.0));
        let frac = view.height() / l.content_height;
        let h = (track.height() * frac).max(20.0);
        let t = self.scroll / (l.content_height - view.height());
        let y = track.min.y + (track.height() - h) * t;
        painter.rect_filled(Rect::from_min_size(pos2(track.min.x, y), vec2(2.0, h)), CornerRadius::same(1), p.text_secondary.gamma_multiply(0.5));
    }

    // ---- input ---------------------------------------------------------

    fn pointer_input(&mut self, ui: &Ui, actions: &mut Vec<TaskAction>) {
        let (events, time) = ui.input(|i| (i.events.clone(), i.time));
        let layer = ui.layer_id();
        let ctx = ui.ctx().clone();
        let area = self.layout.area;
        for event in events {
            match event {
                Event::PointerMoved(pos) => {
                    if let Some(r) = self.resize {
                        self.drag_resize(r, pos);
                    }
                }
                Event::MouseWheel { delta, unit, .. } => {
                    let over = ctx.pointer_latest_pos().is_some_and(|p| area.contains(p));
                    if over {
                        let step = match unit {
                            egui::MouseWheelUnit::Point => delta.y,
                            egui::MouseWheelUnit::Line => delta.y * 50.0,
                            egui::MouseWheelUnit::Page => delta.y * area.height(),
                        };
                        self.scroll = (self.scroll - step).max(0.0);
                    }
                }
                Event::PointerButton { pos, button, pressed, modifiers } => {
                    if self.resize.is_some() && !pressed && button == PointerButton::Primary {
                        self.resize = None;
                        actions.push(TaskAction::SettingsChanged);
                        continue;
                    }
                    // Only presses on the task area itself (not on a menu).
                    if ctx.layer_id_at(pos).is_some_and(|l| l != layer) {
                        continue;
                    }
                    if !area.contains(pos) {
                        if pressed {
                            self.focused = false;
                        }
                        continue;
                    }
                    if pressed {
                        self.press(pos, button, modifiers, time, actions);
                    } else {
                        self.release(pos, button, actions);
                    }
                }
                _ => {}
            }
        }
        // Resize cursors over the column and preview resizers.
        if let Some(pos) = ctx.pointer_latest_pos() {
            let l = &self.layout;
            if l.column_resizers.iter().any(|r| r.contains(pos)) || matches!(self.resize, Some(Resize::Column { .. })) {
                ctx.set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
            } else if l.preview_resizer.is_some_and(|r| r.contains(pos)) || matches!(self.resize, Some(Resize::Preview { .. })) {
                ctx.set_cursor_icon(if self.settings.preview_location == ImagePreviewLocation::Bottom {
                    egui::CursorIcon::ResizeVertical
                } else {
                    egui::CursorIcon::ResizeHorizontal
                });
            } else {
                // `Cursor="Hand"`: a clickable thumbnail, a title, the preview image.
                let on_card = !self.settings.is_list()
                    && self.layout.hits.iter().any(|(id, b, pr)| {
                        pr.is_some_and(|pr| {
                            let title = self.title_rect(ui, *b, pr);
                            (pr.contains(pos) && self.is_clickable(*id)) || title.is_some_and(|t| t.contains(pos))
                        })
                    });
                let on_preview = self.layout.preview_image.is_some_and(|r| r.contains(pos)) && self.preview.texture.is_some();
                if on_card || on_preview {
                    ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
                }
            }
        }
    }

    /// `ThumbnailItemViewModel.IsClickable`.
    fn is_clickable(&self, id: u64) -> bool {
        let Some(task) = self.task(id) else { return false };
        let path = task.info.file_path_str();
        let exists = !path.is_empty() && std::path::Path::new(&path).is_file();
        match self.settings.click_action {
            ThumbnailViewClickAction::Default | ThumbnailViewClickAction::OpenFile => exists,
            ThumbnailViewClickAction::OpenImageViewer | ThumbnailViewClickAction::EditImage => exists && is_image_file(&path),
            ThumbnailViewClickAction::OpenFolder => !path.is_empty(),
            ThumbnailViewClickAction::OpenURL => !task.info.result.text().trim().is_empty(),
            _ => false,
        }
    }

    fn drag_resize(&mut self, r: Resize, pos: Pos2) {
        match r {
            Resize::Column { index, start_x, start_width } => {
                self.columns[index] = (start_width + pos.x - start_x).max(MIN_COLUMN_WIDTH);
                self.settings.column_widths = self.columns.iter().map(|w| w.round() as i32).collect();
            }
            Resize::Preview { start, start_size } => {
                // The handle is on the pane's left or top edge: dragging left or up grows it.
                let area = self.layout.area;
                if self.settings.preview_location == ImagePreviewLocation::Bottom {
                    let max = MIN_PREVIEW.max(area.height() as i32 - 100);
                    self.settings.preview_height = (start_size - (pos.y - start).round() as i32).clamp(MIN_PREVIEW, max);
                } else {
                    let max = MIN_PREVIEW.max(area.width() as i32 - 200);
                    self.settings.preview_width = (start_size - (pos.x - start).round() as i32).clamp(MIN_PREVIEW, max);
                }
            }
        }
    }

    fn press(&mut self, pos: Pos2, button: PointerButton, modifiers: Modifiers, time: f64, actions: &mut Vec<TaskAction>) {
        self.focused = true;
        let l = self.layout.clone();
        // The resizers first (they're on top).
        if button == PointerButton::Primary {
            if let Some(i) = l.column_resizers.iter().position(|r| r.contains(pos)) {
                self.resize = Some(Resize::Column { index: i, start_x: pos.x, start_width: self.columns[i] });
                return;
            }
            if l.preview_resizer.is_some_and(|r| r.contains(pos)) {
                let bottom = self.settings.preview_location == ImagePreviewLocation::Bottom;
                let (start, size) = if bottom {
                    (pos.y, self.settings.preview_height.clamp(MIN_PREVIEW, MIN_PREVIEW.max(l.area.height() as i32 - 100)))
                } else {
                    (pos.x, self.settings.preview_width.clamp(MIN_PREVIEW, MIN_PREVIEW.max(l.area.width() as i32 - 200)))
                };
                self.resize = Some(Resize::Preview { start, start_size: size });
                return;
            }
        }
        // `OnImagePreviewPointerPressed`: keeps the selection; opens the viewer.
        if let Some(pane) = l.preview_pane
            && pane.contains(pos)
        {
            if button == PointerButton::Primary
                && self.preview.texture.is_some()
                && l.preview_image.is_some_and(|r| r.contains(pos))
                && let Some(path) = self.preview_path()
            {
                actions.push(TaskAction::OpenImageViewer(path));
            }
            return;
        }
        if l.header.is_some_and(|h| h.contains(pos)) {
            return;
        }
        let hit = self.hovered_task(pos);
        // A thumbnail's title opens the URL or file (`OnThumbnailTitlePointerPressed`
        // handles the press, so the card doesn't select).
        if let Some(id) = hit
            && !self.settings.is_list()
            && button == PointerButton::Primary
            && self.title_hit(pos).is_some()
        {
            self.last_press = Some((time, pos, Some(id)));
            return;
        }
        let double = button == PointerButton::Primary
            && self.last_press.is_some_and(|(t, p, last)| {
                time - t <= DOUBLE_CLICK_SECONDS && (p - pos).abs().max_elem() <= DOUBLE_CLICK_DISTANCE && last == hit
            });
        if button == PointerButton::Primary {
            self.last_press = if double { None } else { Some((time, pos, hit)) };
        }
        match (hit, button) {
            (Some(id), PointerButton::Secondary) => {
                // `OnThumbnailPointerPressed`, right button.
                if !self.items.iter().any(|i| i.id == id && i.selected) {
                    self.select_only(id);
                }
                self.right_pressed = Some(Some(id));
            }
            (Some(id), PointerButton::Primary) => {
                let ctrl = modifiers.command || modifiers.ctrl;
                let shift = modifiers.shift;
                self.suppress_click = ctrl || shift;
                let anchor = self.anchor.and_then(|a| self.index_of(a));
                let index = self.index_of(id).unwrap_or(0);
                if shift && let Some(anchor) = anchor {
                    if !ctrl {
                        self.clear_selection();
                    }
                    let ids: Vec<u64> = if index >= anchor {
                        (anchor..=index).map(|i| self.items[i].id).collect()
                    } else {
                        (index..=anchor).rev().map(|i| self.items[i].id).collect()
                    };
                    for i in ids {
                        self.set_selected(i, true);
                    }
                } else if ctrl {
                    let selected = self.items.iter().any(|i| i.id == id && i.selected);
                    self.set_selected(id, !selected);
                    self.anchor = Some(id);
                } else {
                    self.select_only(id);
                }
                if double && !self.suppress_click {
                    if self.settings.is_list() {
                        // `OnTaskRowDoubleTapped`: `TryOpen` on that task.
                        actions.push(TaskAction::Command(TaskCommand::TryOpen, vec![id]));
                    } else if self.settings.click_action == ThumbnailViewClickAction::Select
                        && let Some(path) = self.task(id).and_then(|t| t.info.file_path.clone())
                    {
                        actions.push(TaskAction::OpenFile(path));
                    }
                }
            }
            (None, PointerButton::Primary) => {
                // `OnMainContentPointerPressed`.
                self.clear_selection();
                self.anchor = None;
            }
            (None, PointerButton::Secondary) => self.right_pressed = Some(None),
            _ => {}
        }
    }

    /// The task whose thumbnail title is at `pos`.
    fn title_hit(&self, pos: Pos2) -> Option<u64> {
        // Laid out with the line height of the title's font; checked by rect.
        self.layout.hits.iter().find_map(|(id, border, preview)| {
            let preview = (*preview)?;
            let content = border.shrink(CARD_BORDER + CARD_PADDING);
            let s = &self.settings;
            if !s.show_title {
                return None;
            }
            let title = if s.title_location == ThumbnailTitleLocation::Top {
                Rect::from_min_max(content.min, pos2(content.max.x, preview.min.y - CARD_SPACING))
            } else {
                Rect::from_min_max(pos2(content.min.x, preview.max.y + CARD_SPACING), content.max)
            };
            title.contains(pos).then_some(*id)
        })
    }

    fn release(&mut self, pos: Pos2, button: PointerButton, actions: &mut Vec<TaskAction>) {
        let hit = self.hovered_task(pos);
        match button {
            PointerButton::Primary => {
                let Some(id) = hit else { return };
                if !self.settings.is_list() && self.title_hit(pos) == Some(id) {
                    // `OnThumbnailTitlePointerReleased`: the URL, else the file.
                    if let Some(task) = self.task(id) {
                        if !task.info.result.text().is_empty() {
                            actions.push(TaskAction::Command(TaskCommand::NotPorted("Open URL"), vec![id]));
                        } else if let Some(p) = task.info.file_path.clone() {
                            actions.push(TaskAction::OpenFile(p));
                        }
                    }
                    return;
                }
                // `HandleTaskPointerReleased`: a thumbnail runs the click action.
                if !self.settings.is_list() && !self.suppress_click && self.settings.click_action != ThumbnailViewClickAction::Select {
                    self.execute_click(id, actions);
                }
            }
            PointerButton::Secondary => {
                let Some(pressed) = self.right_pressed.take() else { return };
                let entries = match (pressed, hit) {
                    (Some(_), Some(_)) => self.task_menu(),
                    (None, None) => self.area_menu(),
                    // Released elsewhere than it was pressed.
                    _ => return,
                };
                let id = egui::Id::new("task-context-menu");
                self.flyout = Some(Flyout::menu(id, entries, Rect::from_min_size(pos, Vec2::ZERO), Placement::Pointer(pos)));
            }
            _ => {}
        }
    }

    /// `ExecuteThumbnailClick`.
    fn execute_click(&mut self, id: u64, actions: &mut Vec<TaskAction>) {
        let Some(task) = self.task(id) else { return };
        let Some(path) = task.info.file_path.clone() else { return };
        let p = path.to_string_lossy().into_owned();
        let exists = path.is_file();
        match self.settings.click_action {
            ThumbnailViewClickAction::Default if exists => {
                if is_image_file(&p) {
                    actions.push(TaskAction::OpenImageViewer(path));
                } else {
                    actions.push(TaskAction::OpenFile(path));
                }
            }
            ThumbnailViewClickAction::OpenImageViewer if exists && is_image_file(&p) => actions.push(TaskAction::OpenImageViewer(path)),
            ThumbnailViewClickAction::OpenFile => actions.push(TaskAction::OpenFile(path)),
            ThumbnailViewClickAction::OpenFolder => actions.push(TaskAction::Command(TaskCommand::OpenFolder, vec![id])),
            ThumbnailViewClickAction::OpenURL => actions.push(TaskAction::Command(TaskCommand::NotPorted("Open URL"), vec![id])),
            ThumbnailViewClickAction::EditImage if exists && is_image_file(&p) => {
                actions.push(TaskAction::Command(TaskCommand::NotPorted("Edit image"), vec![id]))
            }
            _ => {}
        }
    }

    fn keyboard_input(&mut self, ui: &Ui, actions: &mut Vec<TaskAction>) {
        // egui-winit turns Ctrl+C, Shift+Delete and Ctrl+V into clipboard
        // events without their modifiers, so the app hands those keys over
        // itself ([`Self::clipboard_keys`]).
        let mut keys: Vec<(Key, Modifiers)> = ui.input(|i| {
            i.events
                .iter()
                .filter_map(|e| match e {
                    Event::Key { key, pressed: true, modifiers, .. } => Some((*key, *modifiers)),
                    _ => None,
                })
                .collect()
        });
        keys.append(&mut self.clipboard_keys);
        for (key, m) in keys {
            // `OnWindowKeyDown`.
            if key == Key::Escape {
                actions.push(TaskAction::CloseWindow);
                continue;
            }
            let (ctrl, shift, alt) = (m.ctrl || m.command, m.shift, m.alt);
            // Arrows move the selection when the task area has focus
            // (`OnThumbnailPanelKeyDown` for thumbnails, the window's handler for the list).
            if !ctrl && !alt && self.move_selection(key, shift) {
                continue;
            }
            let command = match key {
                Key::Enter if ctrl => TaskCommand::OpenFile,
                Key::Enter if shift => TaskCommand::OpenFolder,
                Key::Enter => TaskCommand::TryOpen,
                Key::C if ctrl && shift => TaskCommand::CopyFilePath,
                Key::C if shift => TaskCommand::CopyFile,
                Key::C if alt => TaskCommand::CopyImage,
                Key::C if ctrl => TaskCommand::TryCopy,
                Key::V if ctrl => TaskCommand::NotPorted("Upload from clipboard"),
                Key::U if ctrl => TaskCommand::NotPorted("Upload"),
                Key::D if ctrl => TaskCommand::NotPorted("Download"),
                Key::E if ctrl => TaskCommand::NotPorted("Edit image"),
                Key::P if ctrl => TaskCommand::NotPorted("Pin to screen"),
                Key::Delete if shift => TaskCommand::DeleteSelectedFiles,
                Key::Delete => TaskCommand::RemoveSelected,
                _ => continue,
            };
            let selected = self.selected();
            actions.extend(self.run_command(command, selected));
        }
    }

    /// `TryMoveListSelection`.
    fn move_selection(&mut self, key: Key, extend: bool) -> bool {
        let count = self.items.len();
        if count == 0 || !self.focused {
            return false;
        }
        let thumbs = !self.settings.is_list();
        let current = self
            .selection_order
            .iter()
            .rev()
            .find(|id| self.items.iter().any(|i| i.id == **id && i.selected))
            .and_then(|id| self.index_of(*id))
            .map_or(-1, |i| i as i64);
        let count_i = count as i64;
        let target = match key {
            Key::ArrowUp => {
                if current < 0 {
                    count_i - 1
                } else {
                    current - 1
                }
            }
            Key::ArrowDown => current + 1,
            Key::ArrowLeft if thumbs => {
                if current < 0 {
                    count_i - 1
                } else {
                    current - 1
                }
            }
            Key::ArrowRight if thumbs => current + 1,
            Key::Home => 0,
            Key::End => count_i - 1,
            _ => return false,
        }
        .clamp(0, count_i - 1) as usize;
        let id = self.items[target].id;
        let anchor = self.anchor.and_then(|a| self.index_of(a));
        if extend && let Some(anchor) = anchor {
            self.clear_selection();
            let ids: Vec<u64> = if target >= anchor {
                (anchor..=target).map(|i| self.items[i].id).collect()
            } else {
                (target..=anchor).rev().map(|i| self.items[i].id).collect()
            };
            for i in ids {
                self.set_selected(i, true);
            }
            // The moving end last, so the next key continues from it.
            self.set_selected(id, false);
            self.set_selected(id, true);
        } else {
            self.select_only(id);
        }
        self.scroll_into_view(id);
        true
    }

    fn scroll_into_view(&mut self, id: u64) {
        let l = &self.layout;
        let view = if self.settings.is_list() { l.list } else { l.area };
        if let Some((_, r, _)) = l.hits.iter().find(|(i, ..)| *i == id) {
            if r.min.y < view.min.y {
                self.scroll -= view.min.y - r.min.y;
            } else if r.max.y > view.max.y {
                self.scroll += r.max.y - view.max.y;
            }
            self.scroll = self.scroll.max(0.0);
        }
    }

    // ---- context menus -----------------------------------------------

    fn clear_entry(&self) -> MenuEntry<TaskCommand> {
        let text = if self.settings.is_list() { s::MainWindow_ClearTaskList() } else { s::MainWindow_ClearThumbnailView() };
        MenuEntry::item(text, icon::ClearList, TaskCommand::Clear).visible(!self.items.is_empty())
    }

    /// `CreateSwitchTaskViewModeItem`.
    fn switch_entry(&self) -> MenuEntry<TaskCommand> {
        if self.settings.is_list() {
            MenuEntry::item(s::MainWindow_SwitchToThumbnailView(), icon::Grid, TaskCommand::SwitchView)
        } else {
            MenuEntry::item(s::MainWindow_SwitchToListView(), icon::ListView, TaskCommand::SwitchView)
        }
    }

    /// `OnMainContentPointerReleased`'s menu.
    pub fn area_menu(&self) -> Vec<MenuEntry<TaskCommand>> {
        vec![self.clear_entry(), self.switch_entry()]
    }

    /// `BuildTaskContextMenu`, for the selected tasks.
    pub fn task_menu(&mut self) -> Vec<MenuEntry<TaskCommand>> {
        let ids = self.selected();
        let tasks: Vec<&WorkerTask> = ids.iter().filter_map(|id| self.task(*id)).collect();
        let statuses: Vec<UploadInfoStatus> = tasks.iter().map(|t| UploadInfoStatus::new(&t.info)).collect();
        let first = statuses.first().cloned();
        let first_task = tasks.first();
        let has = first.is_some();
        let sel = first.clone().unwrap_or_default();
        let working = tasks.iter().any(|t| t.is_working());
        let ok = has && !working;
        let uploads = !self.settings.uploads_disabled;
        let images = statuses.iter().filter(|s| s.is_image_file).count();
        let path = first_task.map(|t| t.info.file_path_str()).unwrap_or_default();
        let is_gif = sel.is_file_exist && path.to_ascii_lowercase().ends_with(".gif");
        let np = TaskCommand::NotPorted;
        let item = |h: &str, i: &'static str, c: TaskCommand, v: bool| MenuEntry::item(h, i, c).visible(v);
        let sub = |h: &str, i: &'static str, c: Vec<MenuEntry<TaskCommand>>, v: bool| MenuEntry::parent(h, i, c).visible(v);
        let has_errors = first_task.is_some_and(|t| t.info.result.is_error());
        let has_response = first_task.is_some_and(|t| !t.info.result.response.is_empty());
        let entries = vec![
            item(s::MainWindow_ShowErrors(), icon::Warning, TaskCommand::ShowErrors, ok && has_errors),
            item(s::MainWindow_StopUpload(), icon::CircleStop, TaskCommand::StopUpload, working),
            sub(s::MainWindow_Open(), icon::ExternalLink, self.open_menu(&sel), has),
            sub(s::MainWindow_Copy(), icon::Copy, self.copy_menu(&sel, &statuses), ok),
            item(s::MainWindow_UploadSelectedFile(), icon::FileUpload, TaskCommand::Upload, uploads && ok && sel.is_file_exist).gesture("Ctrl+U"),
            item(s::MainWindow_DownloadSelectedUrl(), icon::Download, TaskCommand::Download, ok && sel.is_file_url).gesture("Ctrl+D"),
            item(s::MainWindow_EditImage(), icon::ImageEditor, np("Edit image"), ok && sel.is_image_file).gesture("Ctrl+E"),
            item(s::MainWindow_EditImageInPhotoEditor(), icon::Layers, np("Photo editor"), ok && images > 0),
            item(s::MainWindow_EditImageInPixelEditor(), icon::PixelEditor, np("Pixel editor"), ok && images > 0),
            item(s::MainWindow_ResizeImage(), icon::Resize, np("Image resizer"), ok && images > 0),
            item(s::MainWindow_TrimVideo(), icon::Scissors, np("Video trimmer"), ok && sel.is_video_file),
            item(s::MainWindow_TrimAnimatedGif(), icon::Scissors, np("GIF trimmer"), ok && is_gif),
            item(s::MainWindow_BeautifyImage(), icon::ImageBeautifier, np("Image beautifier"), ok && sel.is_image_file),
            item(s::MainWindow_AddImageEffects(), icon::MagicWand, np("Image effects"), ok && sel.is_image_file),
            item(s::MainWindow_PinToScreen(), icon::Pin, np("Pin to screen"), ok && sel.is_image_file).gesture("Ctrl+P"),
            sub(s::MainWindow_RunAction(), icon::Play, Vec::new(), ok && sel.is_file_exist && self.settings.external_programs),
            item(s::MainWindow_DeleteSelectedItem(), icon::Trash, TaskCommand::RemoveSelected, ok).gesture("Delete"),
            item(s::MainWindow_DeleteSelectedFile(), icon::FileDelete, TaskCommand::DeleteSelectedFiles, ok && sel.is_file_exist)
                .gesture("Shift+Delete"),
            // Remote deletion needs an Amazon S3 upload.
            item(s::MainWindow_DeleteItemRemotely(), icon::CloudOff, np("Delete remotely"), false),
            sub(s::MainWindow_ShortenSelectedUrl(), icon::ShortLink, shortener_menu(), uploads && ok && sel.is_url_exist),
            sub(s::MainWindow_ShareSelectedUrl(), icon::Share, sharing_menu(), uploads && ok && sel.is_url_exist),
            item(s::MainWindow_AnalyzeImage(), icon::Robot, np("Analyze image"), ok && sel.is_image_file),
            item(
                s::MainWindow_SearchWithGoogleLens(),
                icon::Search,
                TaskCommand::SearchImage(URLSharingServices::GoogleImageSearch),
                ok && sel.is_url_exist,
            ),
            item(
                s::MainWindow_SearchWithBingVisualSearch(),
                icon::InspectWindow,
                TaskCommand::SearchImage(URLSharingServices::BingVisualSearch),
                ok && sel.is_url_exist,
            ),
            item(s::MainWindow_ShowQRCode(), icon::QrCode, TaskCommand::ShowQrCode, ok && sel.is_url_exist),
            item(s::MainWindow_OCRImage(), icon::TextRecognition, TaskCommand::OcrImage, ok && sel.is_image_file),
            sub(
                s::MainWindow_CombineImagesDialog(),
                icon::Combine,
                vec![
                    MenuEntry::item(s::MainWindow_Horizontally(), icon::Columns, np("Combine images")),
                    MenuEntry::item(s::MainWindow_Vertically(), icon::Rows, np("Combine images")),
                ],
                ok && images > 1,
            ),
            item(s::MainWindow_ShowResponse(), icon::FileText, TaskCommand::ShowResponse, ok && has_response),
            MenuEntry::separator(),
            self.clear_entry(),
            self.switch_entry(),
        ];
        entries
    }

    /// `BuildOpenTaskMenu`.
    fn open_menu(&self, sel: &UploadInfoStatus) -> Vec<MenuEntry<TaskCommand>> {
        // Separators show even with nothing visible around them, as in ShareX.
        use TaskCommand::OpenUrl;
        vec![
            MenuEntry::item(s::MainWindow_Url(), icon::Link, OpenUrl(UrlField::Url)).visible(sel.is_url_exist).gesture("Return"),
            MenuEntry::item(s::MainWindow_ShortenedUrl(), icon::ShortLink, OpenUrl(UrlField::Shortened)).visible(sel.is_shortened_url_exist),
            MenuEntry::item(s::MainWindow_ThumbnailUrl(), icon::Image, OpenUrl(UrlField::Thumbnail)).visible(sel.is_thumbnail_url_exist),
            MenuEntry::item(s::MainWindow_DeletionUrl(), icon::Trash, OpenUrl(UrlField::Deletion)).visible(sel.is_deletion_url_exist),
            MenuEntry::separator(),
            MenuEntry::item(s::MainWindow_File(), icon::File, TaskCommand::OpenFile).visible(sel.is_file_exist).gesture("Ctrl+Return"),
            MenuEntry::item(s::MainWindow_Folder(), icon::FolderOpen, TaskCommand::OpenFolder)
                .visible(sel.is_file_exist)
                .gesture("Shift+Return"),
            MenuEntry::item(s::MainWindow_ThumbnailFile(), icon::FileImage, TaskCommand::OpenThumbnailFile).visible(sel.is_thumbnail_file_exist),
        ]
    }

    /// `BuildCopyTaskMenu`.
    fn copy_menu(&self, sel: &UploadInfoStatus, all: &[UploadInfoStatus]) -> Vec<MenuEntry<TaskCommand>> {
        use TaskCommand::{CopyFormat, CopyUrl};
        use link_formats as f;
        let any = |f: &dyn Fn(&UploadInfoStatus) -> bool| all.iter().any(f);
        let url = any(&|s| s.is_url_exist);
        let image_url = any(&|s| s.is_image_url);
        let linked = any(&|s| s.is_image_url && s.is_thumbnail_url_exist);
        let path = any(&|s| s.is_file_path_valid);
        let mut entries = vec![
            MenuEntry::item(s::MainWindow_Url(), icon::Link, CopyUrl(UrlField::Url)).visible(url).gesture("Ctrl+C"),
            MenuEntry::item(s::MainWindow_ShortenedUrl(), icon::ShortLink, CopyUrl(UrlField::Shortened)).visible(any(&|s| s.is_shortened_url_exist)),
            MenuEntry::item(s::MainWindow_ThumbnailUrl(), icon::Image, CopyUrl(UrlField::Thumbnail)).visible(any(&|s| s.is_thumbnail_url_exist)),
            MenuEntry::item(s::MainWindow_DeletionUrl(), icon::Trash, CopyUrl(UrlField::Deletion)).visible(any(&|s| s.is_deletion_url_exist)),
            MenuEntry::separator(),
            MenuEntry::item(s::MainWindow_File(), icon::File, TaskCommand::CopyFile).visible(sel.is_file_exist).gesture("Shift+C"),
            MenuEntry::item(s::MainWindow_Image(), icon::Image, TaskCommand::CopyImage).visible(sel.is_image_file).gesture("Alt+C"),
            MenuEntry::item(s::MainWindow_ImageDimensions(), icon::Ruler, TaskCommand::CopyImageDimensions).visible(sel.is_image_file),
            MenuEntry::item(s::MainWindow_Text(), icon::FileText, TaskCommand::CopyText).visible(sel.is_text_file),
            MenuEntry::item(s::MainWindow_ThumbnailFile(), icon::FileImage, TaskCommand::CopyThumbnailFile).visible(sel.is_thumbnail_file_exist),
            MenuEntry::item(s::MainWindow_ThumbnailImage(), icon::Images, TaskCommand::CopyThumbnailImage).visible(sel.is_thumbnail_file_exist),
            MenuEntry::separator(),
            MenuEntry::item(s::MainWindow_HtmlLink(), icon::Code, CopyFormat(f::HTML_LINK, FormatFilter::Url)).visible(url),
            MenuEntry::item(s::MainWindow_HtmlImage(), icon::FileCode, CopyFormat(f::HTML_IMAGE, FormatFilter::Image)).visible(image_url),
            MenuEntry::item(s::MainWindow_HtmlLinkedImage(), icon::Braces, CopyFormat(f::HTML_LINKED_IMAGE, FormatFilter::LinkedImage)).visible(linked),
            MenuEntry::item(s::MainWindow_ForumLink(), icon::Message, CopyFormat(f::FORUM_LINK, FormatFilter::Url)).visible(url),
            MenuEntry::item(s::MainWindow_ForumImage(), icon::Messages, CopyFormat(f::FORUM_IMAGE, FormatFilter::Image)).visible(image_url && url),
            MenuEntry::item(s::MainWindow_ForumLinkedImage(), icon::MessageShare, CopyFormat(f::FORUM_LINKED_IMAGE, FormatFilter::LinkedImage))
                .visible(linked),
            MenuEntry::item(s::MainWindow_MarkdownLink(), icon::Link, CopyFormat(f::MARKDOWN_LINK, FormatFilter::Url)).visible(url),
            MenuEntry::item(s::MainWindow_MarkdownImage(), icon::Image, CopyFormat(f::MARKDOWN_IMAGE, FormatFilter::Image)).visible(image_url),
            MenuEntry::item(s::MainWindow_MarkdownLinkedImage(), icon::Images, CopyFormat(f::MARKDOWN_LINKED_IMAGE, FormatFilter::LinkedImage))
                .visible(linked),
            MenuEntry::separator(),
            MenuEntry::item(s::MainWindow_FilePath(), icon::FilePath, TaskCommand::CopyFilePath).visible(path).gesture("Ctrl+Shift+C"),
            MenuEntry::item(s::MainWindow_FileName(), icon::File, TaskCommand::CopyFileName).visible(path),
            MenuEntry::item(s::MainWindow_FileNameWithExtension(), icon::Files, TaskCommand::CopyFileNameWithExtension).visible(path),
            MenuEntry::item(s::MainWindow_Folder(), icon::Folder, TaskCommand::CopyFolder).visible(path),
        ];
        if !self.settings.clipboard_formats.is_empty() {
            entries.push(MenuEntry::separator());
            for (i, f) in self.settings.clipboard_formats.iter().enumerate() {
                entries.push(MenuEntry::item(f.clone(), icon::ClipboardCopy, TaskCommand::CopyCustomFormat(i)));
            }
        }
        entries
    }
}

/// `BuildUrlShortenerMenu`: every shortener by its localized name.
fn shortener_menu() -> Vec<MenuEntry<TaskCommand>> {
    UrlShortenerType::ALL
        .iter()
        .map(|d| MenuEntry::item(sharex_uploaders::factory::service_name("UrlShortenerType", d.name()), icon::ShortLink, TaskCommand::ShortenUrl(d.clone())))
        .collect()
}

/// `BuildUrlSharingMenu`.
fn sharing_menu() -> Vec<MenuEntry<TaskCommand>> {
    URLSharingServices::ALL
        .iter()
        .map(|d| MenuEntry::item(sharex_uploaders::factory::service_name("URLSharingServices", d.name()), icon::Share, TaskCommand::ShareUrl(d.clone())))
        .collect()
}

/// `ThumbnailItemViewModel.Title`.
fn title_text(task: &WorkerTask) -> String {
    if !task.info.file_name.is_empty() {
        task.info.file_name.clone()
    } else if let Some(name) = task.info.file_path.as_deref().and_then(|p| p.file_name()) {
        name.to_string_lossy().into_owned()
    } else {
        s::ThumbnailItemViewModel_UntitledTask().to_string()
    }
}

/// `GetPlaceholderIcon`.
fn placeholder_icon(path: &str) -> &'static str {
    if path.is_empty() {
        icon::File
    } else if is_video_file(path) {
        icon::FileVideo
    } else if is_text_file(path) {
        icon::FileText
    } else if is_image_file(path) {
        icon::FileImage
    } else {
        icon::File
    }
}

/// `ListStatusIcon` and the failed colour.
fn list_status_icon(task: &WorkerTask) -> (&'static str, Color32) {
    let key = match task.status {
        TaskStatus::InQueue => icon::Clock,
        TaskStatus::Preparing | TaskStatus::Working | TaskStatus::Stopping => icon::Upload,
        TaskStatus::Stopped => icon::CircleStop,
        TaskStatus::Failed => icon::CircleX,
        TaskStatus::History => icon::History,
        TaskStatus::Completed => icon::CircleCheck,
    };
    (key, FAILED)
}

/// `RefreshListColumns`: file name, status, progress, speed, elapsed,
/// remaining, URL. An upload's transfer columns come from its progress
/// reports and keep their last values once it finishes.
fn list_cells(task: &WorkerTask) -> [String; 7] {
    let info = &task.info;
    let mut c: [String; 7] = Default::default();
    c[0] = title_text(task);
    match task.status {
        TaskStatus::History => {
            c[1] = s::TaskListView_History().to_string();
            c[2] = info.end_time.map(format_time).unwrap_or_default();
            c[6] = info.text();
        }
        TaskStatus::InQueue => c[1] = s::TaskListView_InQueue().to_string(),
        status => {
            c[2..6].clone_from_slice(&task.transfer);
            c[1] = match info.progress {
                // `string.Format("{0:0.0}%", progress.Percentage)`.
                Some(p) if status == TaskStatus::Working && p.length > 0 => format!("{:.1}%", p.percentage()),
                _ => info.status.clone(),
            };
            if matches!(status, TaskStatus::Completed | TaskStatus::Stopped) {
                c[6] = info.text();
            }
        }
    }
    c
}

/// `DateTime.ToString()` in the current culture (en-US: `M/d/yyyy h:mm:ss tt`).
fn format_time(t: std::time::SystemTime) -> String {
    let dt: chrono::DateTime<chrono::Local> = t.into();
    dt.format("%-m/%-d/%Y %-I:%M:%S %p").to_string()
}

/// A `TextBlock.task-cell` / `task-header`: `Padding="4,0"`, centred
/// vertically, trimmed with an ellipsis to its column.
#[allow(clippy::too_many_arguments)]
fn cell_text(ui: &Ui, painter: &egui::Painter, text: &str, f: egui::FontId, color: Color32, x: f32, w: f32, row: Rect, line_h: f32) {
    let inner = (w - 8.0).max(0.0);
    if inner <= 0.0 || text.is_empty() {
        return;
    }
    let g = widgets::galley(ui, text, f, color, inner);
    let clip = Rect::from_min_max(pos2(x, row.min.y), pos2(x + w, row.max.y)).intersect(painter.clip_rect());
    painter.with_clip_rect(clip).galley(pos2(x + 4.0, widgets::center_y(row, line_h)), g, color);
}

/// `ShareX.Brush.Checkered`: 30px tiles from `rect`'s corner, the
/// alternate colour in their top left and bottom right 15px squares.
fn paint_checkered(painter: &egui::Painter, rect: Rect, [base, alt]: [Color32; 2]) {
    let clip = painter.with_clip_rect(rect.intersect(painter.clip_rect()));
    clip.rect_filled(rect, CornerRadius::same(4), base);
    let origin = rect.min;
    let mut y = origin.y;
    let mut row = 0;
    while y < rect.max.y {
        let mut x = origin.x + if row % 2 == 0 { 0.0 } else { 15.0 };
        while x < rect.max.x {
            let cell = Rect::from_min_size(pos2(x, y), Vec2::splat(15.0)).intersect(rect.shrink(1.0));
            if cell.is_positive() {
                clip.rect_filled(cell, 0.0, alt);
            }
            x += 30.0;
        }
        y += 15.0;
        row += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view_with(n: usize) -> TaskView {
        let mut v = TaskView::new();
        for i in 0..n {
            let id = v.manager.start_image_task(format!("{i}.bmp"));
            v.add_task(id);
        }
        v
    }

    #[test]
    fn newest_tasks_come_first() {
        let v = view_with(3);
        assert_eq!(v.items.iter().map(|i| i.id).collect::<Vec<_>>(), [3, 2, 1]);
    }

    #[test]
    fn shift_selects_a_range_from_the_anchor() {
        let mut v = view_with(5);
        v.select_only(4);
        v.anchor = Some(4);
        let mut actions = Vec::new();
        v.layout.hits = v.items.iter().enumerate().map(|(i, it)| (it.id, Rect::from_min_size(pos2(0.0, i as f32 * 22.0), vec2(100.0, 22.0)), None)).collect();
        v.layout.list = Rect::from_min_size(Pos2::ZERO, vec2(100.0, 500.0));
        v.layout.area = v.layout.list;
        v.settings.mode = TaskViewMode::ListView;
        v.press(pos2(10.0, 3.0 * 22.0 + 5.0), PointerButton::Primary, Modifiers::SHIFT, 0.0, &mut actions);
        // Items are [5,4,3,2,1]: anchor 4 (index 1) to index 3 (id 2).
        assert_eq!(v.selected(), vec![4, 3, 2]);
        v.press(pos2(10.0, 5.0), PointerButton::Primary, Modifiers::CTRL, 1.0, &mut actions);
        assert_eq!(v.selected(), vec![4, 3, 2, 5]);
    }

    #[test]
    fn list_cells_of_an_upload() {
        use crate::tasks::{TaskInfo, UploadProgress};
        let mut v = TaskView::new();
        let id = v.manager.add_task(TaskInfo { file_name: "a.png".into(), ..Default::default() }, crate::upload::StopHandle::default());
        v.manager.prepare(id);
        v.manager.set_status(id, TaskStatus::Working, s::UploadTask_DoUploadJob_Uploading().into());
        let p = UploadProgress {
            position: 1_500_000,
            length: 6_000_000,
            speed: 500_000.0,
            elapsed: std::time::Duration::from_secs(3),
            remaining: std::time::Duration::from_secs(9),
        };
        v.manager.set_progress(id, p, false);
        let cells = list_cells(v.manager.get(id).unwrap());
        assert_eq!(cells[1..6], ["25.0%", "1.50 MB / 6.00 MB", "500.00 KB/s", "00:03", "00:09"]);
        let report = crate::tasks::TaskReport {
            file_name: "a.png".into(),
            result: crate::tasks::UploadResult { url: "https://i.example.com/a.png".into(), url_expected: true, ..Default::default() },
            ..Default::default()
        };
        v.manager.finish(id, &report, 10);
        let cells = list_cells(v.manager.get(id).unwrap());
        assert_eq!(cells[1..], [s::UploadTask_OnUploadCompleted_Done(), "1.50 MB / 6.00 MB", "500.00 KB/s", "00:03", "00:09", "https://i.example.com/a.png"]);
    }

    #[test]
    fn list_cells_of_a_done_task() {
        let mut v = view_with(1);
        v.manager.complete(1, Some(PathBuf::from("dir/a.png")), None, 10);
        let cells = list_cells(v.manager.get(1).unwrap());
        assert_eq!(cells[0], "a.png");
        assert_eq!(cells[1], s::UploadTask_OnUploadCompleted_Done());
        assert_eq!(cells[6], PathBuf::from("dir/a.png").to_string_lossy());
    }
}
