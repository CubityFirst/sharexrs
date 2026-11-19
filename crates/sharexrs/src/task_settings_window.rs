//! Task settings: `ShareX/Presentation/TaskSettings` (`TaskSettingsWindow.axaml`,
//! `TaskSettingsViewModel.cs`, `TaskSettingsPageBuilder.cs`), for the default
//! task settings or one hotkey's, with the "Override ..." cards that make a
//! hotkey use its own section instead of the defaults.
//!
//! Pages are written once as calls on a [`Form`] (see `settings_form`), as
//! `TaskSettingsPageBuilder` builds them, and bound straight to the
//! `TaskSettings` they edit, so a change applies to the next task at once.

use egui::{CornerRadius, Id, Rect, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};
use serde_json::Value;
use sharex_settings::{AfterCaptureTasks, AfterUploadTasks, Described, HotkeyType, TaskSettings};
use sharex_strings::sharex as s;
use sharex_ui::controls::{self, Button, ColorButton, ComboItem, TextBox};
use sharex_ui::icons;
use sharex_ui::keys::app_icons as icon;
use sharex_ui::menu::{Flyout, MenuEntry, MenuStyle, Placement, Toggle};
use sharex_ui::theme::{density, palette};
use sharex_ui::widgets::{center_y, font, galley, line_height, text_width};

use crate::main_menu::{AFTER_CAPTURE_TASKS, AFTER_UPLOAD_TASKS, find_menu_icon, task_menu};
use crate::settings_form::{self, ComboSize, FormStyle, Mode, any_combo_open, matches, scroll_indicator};

/// `Height="720"`, `MinHeight="600"`; the width is `ShareX.Density.Settings.WindowWidth`.
pub const HEIGHT: f32 = 720.0;
pub const MIN_HEIGHT: f32 = 600.0;
/// The navigation column (`ColumnDefinitions="220,*"`).
const NAV_WIDTH: f32 = 220.0;
/// `Row`'s label column (`ColumnDefinitions="210,*"`).
const LABEL_W: f32 = 210.0;

/// What the window edits: the task settings and the application setting
/// one page shares (`NameParserAutoIncrementNumber`).
pub struct TaskCtx<'b> {
    pub task: &'b mut TaskSettings,
    pub auto_increment: &'b mut i32,
}

type Form<'u, 'a, 'b> = settings_form::Form<'u, 'a, TaskCtx<'b>, TaskSettingsWindow>;

/// What the window asks of the app.
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    /// Capture › Select region...: `RegionCaptureTasks.GetRectangleRegionAsync`
    /// with the task's region options, into `CaptureCustomRegion`.
    SelectRegion,
    /// A feature that isn't ported yet was asked for.
    NotPorted(&'static str),
    /// Screen recording options... (`ShowScreenRecordingOptions`).
    FFmpegOptions,
    /// The watch folders changed (`WatchFolderManager.AddWatchFolder`,
    /// `UpdateWatchFolderState`, `RemoveWatchFolder`).
    WatchFoldersChanged,
    /// Image effects configuration... (`TaskHelpers.OpenImageEffectsSingleton`).
    ImageEffects,
}

/// A page: `SettingsNavigationItem` (id, title, icon) and its parent.
struct Page {
    id: &'static str,
    parent: Option<usize>,
}

/// `TaskSettingsViewModel`'s navigation, in its order.
const PAGES: [Page; 20] = [
    Page { id: "task", parent: None },
    Page { id: "general", parent: None },
    Page { id: "general-notifications", parent: Some(1) },
    Page { id: "image", parent: None },
    Page { id: "image-effects", parent: Some(3) },
    Page { id: "image-thumbnail", parent: Some(3) },
    Page { id: "capture", parent: None },
    Page { id: "capture-region", parent: Some(6) },
    Page { id: "capture-screen-recorder", parent: Some(6) },
    Page { id: "capture-ocr", parent: Some(6) },
    Page { id: "upload", parent: None },
    Page { id: "upload-file-naming", parent: Some(10) },
    Page { id: "upload-clipboard", parent: Some(10) },
    Page { id: "upload-filters", parent: Some(10) },
    Page { id: "tools", parent: None },
    Page { id: "tools-image-editor", parent: Some(14) },
    Page { id: "tools-mouse-highlighter", parent: Some(14) },
    Page { id: "actions", parent: None },
    Page { id: "watch-folders", parent: None },
    Page { id: "advanced", parent: None },
];

/// The page with this id (`SettingsNavigationItem.Id`).
pub fn page_index(id: &str) -> Option<usize> {
    PAGES.iter().position(|p| p.id == id)
}

fn page_title(i: usize) -> (&'static str, &'static str) {
    match i {
        0 => (s::TaskSettingsWindow_Task(), icon::Keyboard),
        1 => (s::TaskSettingsWindow_General(), icon::Settings),
        2 => (s::TaskSettingsWindow_Notifications(), icon::Bell),
        3 => (s::TaskSettingsWindow_Image(), icon::Image),
        4 => (s::TaskSettingsWindow_Effects(), icon::MagicWand),
        5 => (s::TaskSettingsWindow_Thumbnail(), icon::Images),
        6 => (s::TaskSettingsWindow_Capture(), icon::Camera),
        7 => (s::TaskSettingsWindow_RegionCapture(), icon::Crop),
        8 => (s::TaskSettingsWindow_ScreenRecorder(), icon::Video),
        9 => (s::TaskSettingsWindow_OCR(), icon::TextRecognition),
        10 => (s::TaskSettingsWindow_Upload(), icon::Upload),
        11 => (s::TaskSettingsWindow_FileNaming(), icon::FileEdit),
        12 => (s::TaskSettingsWindow_ClipboardUpload(), icon::Clipboard),
        13 => (s::TaskSettingsWindow_UploaderFilters(), icon::Filter),
        14 => (s::TaskSettingsWindow_Tools(), icon::Wrench),
        15 => (s::TaskSettingsWindow_ImageEditor(), icon::Image),
        16 => (s::MouseHighlighter(), icon::PointerClick),
        17 => (s::TaskSettingsWindow_Actions(), icon::Zap),
        18 => (s::TaskSettingsWindow_WatchFolders(), icon::FolderSearch),
        _ => (s::TaskSettingsWindow_Advanced(), icon::Sliders),
    }
}

/// Fluent's `TreeViewItem` expander: a chevron in its 12x12 box, pointing
/// down when expanded (measured on ShareX: a 12x6 stroke about 1.4px wide).
pub(crate) fn paint_tree_chevron(ui: &Ui, rect: Rect, expanded: bool, color: egui::Color32) {
    let stroke = Stroke::new(1.4, color);
    let c = rect.center();
    let points = if expanded {
        [pos2(c.x - 5.4, c.y - 2.4), pos2(c.x, c.y + 2.6), pos2(c.x + 5.4, c.y - 2.4)]
    } else {
        [pos2(c.x - 2.4, c.y - 5.4), pos2(c.x + 2.6, c.y), pos2(c.x - 2.4, c.y + 5.4)]
    };
    ui.painter().add(egui::Shape::line(points.to_vec(), stroke));
}

/// What the window's popup menus choose.
#[derive(Clone, Debug, PartialEq)]
enum MenuCmd {
    Task(HotkeyType),
    AfterCapture(&'static str),
    AfterUpload(&'static str),
    Destination,
}

/// The window's own state (`TaskSettingsViewModel`, the page builder's
/// locals).
pub struct TaskSettingsWindow {
    /// `isDefault`: the application's default task settings, not a hotkey's.
    pub is_default: bool,
    pub page: usize,
    pub search: String,
    scroll: [f32; 20],
    nav_scroll: f32,
    /// The page whose item was last brought into view.
    nav_shown: Option<usize>,
    expanded: [bool; 20],
    menu: Option<Flyout<MenuCmd>>,
    /// The last after capture task chosen (`lastSelectedTask`): its icon
    /// shows on the button.
    last_after_capture: Option<&'static str>,
    last_after_upload: Option<&'static str>,
    filter_selected: Option<usize>,
    filter_extensions: String,
    pub action_selected: Option<usize>,
    pub watch_selected: Option<usize>,
    /// `ActionEditorOverlay`.
    pub action_editor: Option<crate::task_editors::ActionEditor>,
    /// `WatchFolderEditorOverlay`.
    pub watch_editor: Option<crate::task_editors::WatchFolderEditor>,
    /// The notification buttons editor (`ShowNotificationButtonsEditor`).
    pub notification_editor: Option<crate::task_editors::NotificationButtonsEditor>,
    /// `ToolbarEditorOverlay` (`ShowToolbarEditor`).
    pub toolbar_editor: Option<crate::toolbar_editor::ToolbarEditor>,
    color_open: bool,
    effects: Vec<Effect>,
}

impl TaskSettingsWindow {
    /// `TaskSettingsIntegration.Show`: a hotkey's settings take the
    /// defaults' sections first (`SetDefaultSettings`).
    pub fn new(task: &mut TaskSettings, defaults: Option<&TaskSettings>) -> Self {
        let is_default = defaults.is_none();
        if let Some(d) = defaults {
            task.set_default_settings(d);
        }
        Self::ensure_sections(task);
        let last = |names: &[(&'static str, &'static str)], has: &dyn Fn(&str) -> bool| names.iter().rev().find(|(n, _)| has(n)).map(|(n, _)| *n);
        let ac = task.AfterCaptureJob;
        let au = task.AfterUploadJob;
        Self {
            is_default,
            // The default window opens on Notifications, a hotkey's on Task.
            page: if is_default { 2 } else { 0 },
            search: String::new(),
            scroll: [0.0; 20],
            nav_scroll: 0.0,
            nav_shown: None,
            expanded: [true; 20],
            menu: None,
            last_after_capture: last(&AFTER_CAPTURE_TASKS, &|n| AfterCaptureTasks::parse(n).is_some_and(|f| ac.contains(f))),
            last_after_upload: last(&AFTER_UPLOAD_TASKS, &|n| AfterUploadTasks::parse(n).is_some_and(|f| au.contains(f))),
            filter_selected: None,
            filter_extensions: String::new(),
            action_selected: None,
            watch_selected: None,
            action_editor: None,
            watch_editor: None,
            notification_editor: None,
            toolbar_editor: None,
            color_open: false,
            effects: Vec::new(),
        }
    }

    /// `settings.XSettings ?? new()`: every section the pages bind to.
    fn ensure_sections(t: &mut TaskSettings) {
        t.GeneralSettings.get_or_insert_default();
        t.ImageSettings.get_or_insert_default();
        t.CaptureSettings.get_or_insert_default();
        t.UploadSettings.get_or_insert_default();
        t.ToolsSettings.get_or_insert_default();
        t.AdvancedSettings.get_or_insert_default();
        t.ExternalPrograms.get_or_insert_default();
    }

    /// The window's title: "ShareX - Task settings" or "... for <task>".
    pub fn title(&self, task: &TaskSettings) -> String {
        let t = if self.is_default {
            s::TaskSettingsWindow_ShareXTaskSettings().to_string()
        } else {
            s::TaskSettingsWindow_ShareXTaskSettingsFor0().replace("{0}", &task.display_name())
        };
        // ShareXrs's windows carry its own name.
        match t.strip_prefix("ShareX - ") {
            Some(rest) => format!("ShareXrs - {rest}"),
            None => t,
        }
    }

    fn page_shown(&self, i: usize) -> bool {
        i != 0 || !self.is_default
    }

    /// Scrolls page `page` to `y` (clamped when drawn; `f32::MAX` is the end).
    pub fn set_scroll(&mut self, page: usize, y: f32) {
        self.scroll[page] = y;
    }

    /// `SelectedNavigationItem`'s setter: in the default window General and
    /// Upload are only headings, so they select their first child.
    fn select_page(&mut self, i: usize) {
        let i = if self.is_default && (i == 1 || i == 10) { i + 1 } else { i };
        self.page = i;
    }

    /// The query's terms (`Split(' ', RemoveEmptyEntries | TrimEntries)`).
    fn terms(&self) -> Vec<String> {
        self.search.split(' ').map(|t| t.trim().to_lowercase()).filter(|t| !t.is_empty()).collect()
    }

    /// Draws the window over all of `ui`, changing the task settings.
    pub fn ui(&mut self, ui: &mut Ui, ctx: &mut TaskCtx) -> Vec<Effect> {
        let p = palette(ui.ctx());
        let full = ui.max_rect();
        ui.painter().rect_filled(full, 0.0, p.background_main);
        let open = ui.ctx().data_mut(|d| d.remove_temp::<bool>(Id::new("settings-any-combo-next"))).unwrap_or(false);
        ui.ctx().data_mut(|d| d.insert_temp(Id::new("settings-any-combo"), open));
        let terms = self.terms();
        // A menu opened this frame isn't dismissed by the press that opened it.
        let had_menu = self.menu.is_some();
        // `SettingsNavigationItem.ApplyFilter`: a page shows when it or a
        // child matches.
        let mut visible = [false; 20];
        for (i, shown) in visible.iter_mut().enumerate() {
            if !self.page_shown(i) {
                continue;
            }
            *shown = terms.is_empty() || {
                let mut texts = Vec::new();
                self.run_page(ui, ctx, i, Mode::Collect(&mut texts), Rect::NOTHING, &terms);
                matches(&texts.join(" "), &terms)
            };
        }
        for i in 0..PAGES.len() {
            if visible[i]
                && let Some(parent) = PAGES[i].parent
            {
                visible[parent] = true;
            }
        }
        if !visible[self.page]
            && let Some(first) = visible.iter().position(|v| *v)
        {
            self.select_page(first);
        }
        self.navigation(ui, full, &visible);
        let column = Rect::from_min_max(pos2(full.min.x + NAV_WIDTH, full.min.y), full.max);
        if visible.iter().any(|v| *v) {
            let page = self.page;
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(column));
            child.set_clip_rect(column.intersect(ui.clip_rect()));
            let height = self.run_page(&mut child, ctx, page, Mode::Paint, column, &terms);
            let max = (height - column.height()).max(0.0);
            let editing = self.action_editor.is_some() || self.watch_editor.is_some() || self.toolbar_editor.is_some() || self.notification_editor.is_some();
            if ui.rect_contains_pointer(column) && self.menu.is_none() && !self.color_open && !editing && !any_combo_open(ui.ctx()) {
                self.scroll[page] -= ui.input(|i| i.smooth_scroll_delta.y);
            }
            self.scroll[page] = self.scroll[page].clamp(0.0, max);
            if max > 0.0 {
                scroll_indicator(&child, column, self.scroll[page], height);
            }
        }
        self.show_menu(ui, full, ctx, had_menu);
        self.show_editors(ui, full, ctx);
        std::mem::take(&mut self.effects)
    }

    /// The action and watch folder editors over the window, and what their
    /// Save does (`ShowActionEditor`'s and `ShowWatchFolderEditor`'s callbacks
    /// in `TaskSettingsPageBuilder`).
    fn show_editors(&mut self, ui: &mut Ui, full: Rect, ctx: &mut TaskCtx) {
        use crate::task_editors::Outcome;
        if let Some(ed) = self.action_editor.as_mut() {
            match ed.show(ui, full) {
                Outcome::Open => {}
                Outcome::Cancelled => self.action_editor = None,
                Outcome::Saved(()) => {
                    if let Some(i) = ed.save(ctx.task.ExternalPrograms.get_or_insert_default()) {
                        self.action_selected = Some(i);
                        self.action_editor = None;
                    }
                }
            }
        }
        if let Some(ed) = self.toolbar_editor.as_mut() {
            use crate::toolbar_editor::Target;
            match ed.show(ui, full) {
                Outcome::Open => {}
                Outcome::Cancelled => self.toolbar_editor = None,
                Outcome::Saved(values) => {
                    // `options.ToolbarItems = items` (the image editor's
                    // options are kept as JSON).
                    match ed.target {
                        Target::ImageEditor => {
                            let o = &mut ctx.task.ToolsSettings.get_or_insert_default().ImageEditorOptions;
                            if !o.is_object() {
                                *o = serde_json::Value::Object(Default::default());
                            }
                            o["ToolbarItems"] = serde_json::Value::Array(values);
                        }
                        Target::RegionCapture => {
                            ctx.task.CaptureSettings.get_or_insert_default().RegionCaptureOptions.ToolbarItems = values;
                        }
                    }
                    self.toolbar_editor = None;
                }
            }
        }
        if let Some(ed) = self.notification_editor.as_mut() {
            match ed.show(ui, full) {
                Outcome::Open => {}
                Outcome::Cancelled => self.notification_editor = None,
                // `buttons => general.ToastWindowButtons = buttons`.
                Outcome::Saved(buttons) => {
                    ctx.task.GeneralSettings.get_or_insert_default().ToastWindowButtons = buttons;
                    self.notification_editor = None;
                }
            }
        }
        if let Some(ed) = self.watch_editor.as_mut() {
            match ed.show(ui, full) {
                Outcome::Open => {}
                Outcome::Cancelled => self.watch_editor = None,
                Outcome::Saved(()) => {
                    let i = ed.save(&mut ctx.task.WatchFolderList);
                    self.watch_selected = Some(i);
                    self.watch_editor = None;
                    self.effects.push(Effect::WatchFoldersChanged);
                }
            }
        }
    }

    /// The open popup menu, and what it chose.
    fn show_menu(&mut self, ui: &mut Ui, full: Rect, ctx: &mut TaskCtx, had_menu: bool) {
        let Some(flyout) = self.menu.as_mut() else { return };
        let out = flyout.show(ui, full);
        let outside = had_menu && ui.input(|i| i.pointer.any_pressed()) && ui.input(|i| i.pointer.interact_pos()).is_some_and(|pos| !flyout.contains(pos));
        if let Some(cmd) = out.command.clone() {
            let t = &mut *ctx.task;
            match cmd {
                MenuCmd::Task(job) => t.Job = job,
                MenuCmd::AfterCapture(name) => {
                    if let Some(flag) = AfterCaptureTasks::parse(name) {
                        let mut v = t.AfterCaptureJob;
                        if v.contains(flag) {
                            v.remove(flag);
                        } else {
                            v.insert(flag);
                            self.last_after_capture = Some(name);
                        }
                        t.AfterCaptureJob = v;
                    }
                }
                MenuCmd::AfterUpload(name) => {
                    if let Some(flag) = AfterUploadTasks::parse(name) {
                        let mut v = t.AfterUploadJob;
                        if v.contains(flag) {
                            v.remove(flag);
                        } else {
                            v.insert(flag);
                            self.last_after_upload = Some(name);
                        }
                        t.AfterUploadJob = v;
                    }
                }
                MenuCmd::Destination => self.effects.push(Effect::NotPorted("destinations")),
            }
        }
        if out.closed || outside || out.command.is_some() {
            self.menu = None;
        }
    }

    /// `SettingsNavigation`: the search box, then the pages as a tree
    /// (Fluent's `TreeViewItem`: a 12px chevron in a 12,0,12,0 margin,
    /// children indented 16).
    fn navigation(&mut self, ui: &mut Ui, full: Rect, visible: &[bool; 20]) {
        let p = palette(ui.ctx());
        let d = density(ui.ctx());
        let nav = Rect::from_min_size(full.min, vec2(NAV_WIDTH, full.height()));
        ui.painter().rect_filled(nav, 0.0, p.background_toolbar);
        ui.painter().rect_filled(Rect::from_min_max(pos2(nav.max.x - 1.0, nav.min.y), nav.max), 0.0, p.border);
        let inner = Rect::from_min_max(pos2(nav.min.x + 8.0, nav.min.y + 8.0), pos2(nav.max.x - 1.0 - 8.0, nav.max.y - 8.0));
        let font_h = line_height(ui, &TextBox::font());
        let search_h = (1.0 + d.settings_search_padding.top + font_h + d.settings_search_padding.bottom + 1.0).max(40.0);
        let search = Rect::from_min_size(inner.min, vec2(inner.width(), search_h));
        let tb = TextBox { padding: d.settings_search_padding, placeholder: sharex_strings::avalonia::SettingsNavigation_Search_settings(), ..Default::default() };
        let before = self.search.clone();
        tb.show(ui, search, Id::new("task-settings-search"), &mut self.search);
        if self.search != before && !self.search.trim().is_empty() {
            // A search expands what it finds (`IsExpanded = true`).
            self.expanded = [true; 20];
        }
        icons::paint(ui.painter(), Rect::from_min_size(pos2(search.min.x + 8.0, center_y(search, 16.0)), Vec2::splat(16.0)), icon::Search, p.text);
        let tree = Rect::from_min_max(pos2(inner.min.x, search.max.y + d.settings_nav_spacing), inner.max);
        let item_h = d.settings_nav_item_min_height.max(line_height(ui, &font(13.0, 400)) + d.settings_nav_item_margin.top + d.settings_nav_item_margin.bottom);
        // Each `TreeViewItem` has `Margin="0,1"` around its header and its
        // children: a parent's bottom margin comes after its last child.
        let mut rows: Vec<(usize, f32)> = Vec::new();
        let mut y = 0.0;
        for i in (0..PAGES.len()).filter(|&i| visible[i] && PAGES[i].parent.is_none()) {
            y += 1.0;
            rows.push((i, y));
            y += item_h;
            if self.expanded[i] {
                for c in (0..PAGES.len()).filter(|&c| visible[c] && PAGES[c].parent == Some(i)) {
                    y += 1.0;
                    rows.push((c, y));
                    y += item_h + 1.0;
                }
            }
            y += 1.0;
        }
        let content_h = y;
        let max_scroll = (content_h - tree.height()).max(0.0);
        if ui.rect_contains_pointer(tree) && self.menu.is_none() {
            self.nav_scroll -= ui.input(|i| i.smooth_scroll_delta.y);
        }
        // A newly selected page's item is brought into view (`BringIntoView`:
        // the least scrolling that shows it).
        if self.nav_shown != Some(self.page)
            && let Some(&(_, top)) = rows.iter().find(|(i, _)| *i == self.page)
        {
            if top < self.nav_scroll {
                self.nav_scroll = top;
            } else if top + item_h > self.nav_scroll + tree.height() {
                self.nav_scroll = top + item_h - tree.height();
            }
            self.nav_shown = Some(self.page);
        }
        self.nav_scroll = self.nav_scroll.clamp(0.0, max_scroll);
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(tree));
        child.set_clip_rect(tree.intersect(ui.clip_rect()));
        for &(i, top) in &rows {
            let r = Rect::from_min_size(pos2(tree.min.x, tree.min.y - self.nav_scroll + top), vec2(tree.width(), item_h));
            if !r.intersects(tree) {
                continue;
            }
            let resp = child.interact(r, Id::new(("task-nav", i)), egui::Sense::click());
            let selected = i == self.page;
            settings_form::nav_item_background(child.painter(), r, &resp, selected);
            let indent = if PAGES[i].parent.is_some() { 16.0 } else { 0.0 };
            let has_children = PAGES.iter().any(|c| c.parent == Some(i));
            let fg = if selected { p.accent_foreground } else { p.text };
            let mut toggled = false;
            if has_children {
                // The expander: Fluent's chevron, down when expanded.
                let cr = Rect::from_min_size(pos2(r.min.x + indent + 12.0, center_y(r, 12.0)), Vec2::splat(12.0));
                let hit = Rect::from_min_max(pos2(r.min.x + indent, r.min.y), pos2(r.min.x + indent + 36.0, r.max.y));
                let ex = child.interact(hit, Id::new(("task-nav-expand", i)), egui::Sense::click());
                paint_tree_chevron(&child, cr, self.expanded[i], fg);
                if ex.clicked() {
                    self.expanded[i] = !self.expanded[i];
                    toggled = true;
                }
            }
            let (title, key) = page_title(i);
            let content = Rect::from_min_max(pos2(r.min.x + indent + 36.0 + d.settings_nav_item_margin.left, r.min.y), pos2(r.max.x - d.settings_nav_item_margin.right, r.max.y));
            let icon_color = if selected { p.accent_foreground } else { p.accent_icon };
            icons::paint(child.painter(), Rect::from_min_size(pos2(content.min.x + 2.0, center_y(r, 16.0)), Vec2::splat(16.0)), key, icon_color);
            controls::label(&child, Rect::from_min_max(pos2(content.min.x + 26.0, r.min.y), pos2(content.max.x, r.max.y)), title, font(13.0, 400), fg);
            settings_form::nav_item_focus(child.painter(), r, &resp);
            if resp.clicked() && !toggled && !selected {
                self.select_page(i);
            }
        }
        if max_scroll > 0.0 {
            scroll_indicator(&child, tree, self.nav_scroll, content_h);
        }
    }

    /// Lays out page `i` (painting it in `view`, or only collecting its
    /// text) and returns its height.
    fn run_page(&mut self, ui: &mut Ui, ctx: &mut TaskCtx, i: usize, mode: Mode, view: Rect, terms: &[String]) -> f32 {
        let d = density(ui.ctx());
        let m = d.settings_page_margin;
        let top = view.min.y - self.scroll[i];
        let (title, key) = page_title(i);
        let mut f = Form {
            ui,
            s: ctx,
            w: self,
            paint: matches!(mode, Mode::Paint),
            collected: match mode {
                Mode::Collect(_) => Some(Vec::new()),
                Mode::Paint => None,
            },
            card_texts: None,
            terms: terms.to_vec(),
            page_title: title.to_string(),
            x: view.min.x + m.left,
            y: top + m.top,
            width: view.width() - m.left - m.right,
            first: true,
            spacing: 0.0,
            d: d.clone(),
            page: i,
            enabled: true,
            style: FormStyle::TASK,
        };
        f.page_title(key, title);
        match i {
            0 => page_task(&mut f),
            1 => parent_page(&mut f, Section::General, s::TaskSettingsWindow_OverrideGeneralSettings()),
            2 => page_notifications(&mut f),
            3 => page_image(&mut f),
            4 => page_effects(&mut f),
            5 => page_thumbnail(&mut f),
            6 => page_capture(&mut f),
            7 => page_region_capture(&mut f),
            8 => page_screen_recorder(&mut f),
            9 => page_ocr(&mut f),
            10 => parent_page(&mut f, Section::Upload, s::TaskSettingsWindow_OverrideUploadSettings()),
            11 => page_file_naming(&mut f),
            12 => page_clipboard_upload(&mut f),
            13 => page_uploader_filters(&mut f),
            14 => page_tools(&mut f),
            15 => page_image_editor(&mut f),
            16 => page_mouse_highlighter(&mut f),
            17 => page_actions(&mut f),
            18 => page_watch_folders(&mut f),
            _ => page_advanced(&mut f),
        }
        let height = f.y + m.bottom - top;
        if let (Mode::Collect(out), Some(texts)) = (mode, f.collected.take()) {
            out.push(title.to_string());
            out.extend(texts);
        }
        height
    }
}

impl Form<'_, '_, '_> {
    fn effect(&mut self, e: Effect) {
        if !self.w.effects.contains(&e) {
            self.w.effects.push(e);
        }
    }

    /// `OverrideValue`: always on in the default window.
    fn overriding(&self, section: Section) -> bool {
        self.w.is_default || !section.uses_default(self.s.task)
    }

    /// `OverrideCard`: only in a hotkey's window.
    fn override_section(&mut self, section: Section, text: &str) {
        if self.w.is_default {
            return;
        }
        let mut v = !section.uses_default(self.s.task);
        if self.override_card(text, &mut v) {
            section.set_uses_default(self.s.task, !v);
        }
    }

    /// `EnabledCard`: a card disabled unless `section` is overridden.
    fn enabled_card(&mut self, section: Section, build: &mut dyn FnMut(&mut Self)) {
        let on = self.overriding(section);
        let saved = self.enabled;
        self.enabled = saved && on;
        self.card(4.0, build);
        self.enabled = saved;
    }

    /// A button in a `Row` (or alone) that opens a menu: an icon, the text,
    /// a 14px chevron (`ColumnDefinitions="Auto,*,Auto" ColumnSpacing="7"`),
    /// `Button.form-control` 40 tall. Returns its rect when clicked.
    #[allow(clippy::too_many_arguments)]
    fn menu_button(&mut self, label: Option<&str>, key: &str, glyph: &str, text: &str, min_w: f32, stretch: bool, enabled: bool) -> Option<Rect> {
        if let Some(l) = label {
            self.text(l);
        }
        self.text(text);
        if self.collecting() {
            return None;
        }
        let h: f32 = 40.0;
        let lh = line_height(self.ui, &Self::label_font());
        let r = self.slot(h.max(lh));
        let p = palette(self.ui.ctx());
        let mut x = r.min.x;
        if let Some(l) = label {
            controls::label(self.ui, Rect::from_min_max(r.min, pos2(r.min.x + LABEL_W, r.max.y)), l, Self::label_font(), p.text_secondary);
            x += LABEL_W + 8.0;
        }
        let pad = self.d.settings_control_padding;
        let body = font(13.0, 400);
        let natural = 1.0 + pad.left + 18.0 + 7.0 + text_width(self.ui, text, body.clone()) + 7.0 + 14.0 + pad.right + 1.0;
        let w = if stretch { (r.max.x - x).max(min_w) } else { natural.max(min_w).min(r.max.x - x) };
        let br = Rect::from_min_size(pos2(x, center_y(r, h)), vec2(w, h));
        let enabled = enabled && self.enabled;
        let resp = Button::compact(pad).show(self.ui, br, self.id(key), "", enabled);
        let opacity = if enabled { 1.0 } else { 0.35 };
        let c = Rect::from_min_max(pos2(br.min.x + 1.0 + pad.left, br.min.y + 1.0), pos2(br.max.x - 1.0 - pad.right, br.max.y - 1.0));
        icons::paint(self.ui.painter(), Rect::from_min_size(pos2(c.min.x + 1.0, center_y(c, 16.0)), Vec2::splat(16.0)), glyph, p.accent_icon.gamma_multiply(opacity));
        let chevron = Rect::from_min_size(pos2(c.max.x - 14.0, center_y(c, 14.0)), Vec2::splat(14.0));
        icons::paint(self.ui.painter(), chevron, icon::ChevronDown, p.text.gamma_multiply(opacity));
        let tr = Rect::from_min_max(pos2(c.min.x + 25.0, c.min.y), pos2(chevron.min.x - 7.0, c.max.y));
        controls::label(self.ui, tr, text, body, p.text_secondary.gamma_multiply(opacity));
        resp.clicked().then_some(br)
    }

    /// A `Row` whose editor is a hint (`TextBlock.hint`): the preview rows.
    fn hint_row(&mut self, label: &str, text: &str) {
        self.text(label);
        self.text(text);
        if self.collecting() {
            return;
        }
        let p = palette(self.ui.ctx());
        let hf = font(12.0, 400);
        let m = self.style.hint_margin;
        let hh = m + line_height(self.ui, &hf) + m;
        let lh = line_height(self.ui, &Self::label_font());
        let r = self.slot(hh.max(lh));
        controls::label(self.ui, Rect::from_min_max(r.min, pos2(r.min.x + LABEL_W, r.max.y)), label, Self::label_font(), p.text_secondary);
        let tr = Rect::from_min_max(pos2(r.min.x + LABEL_W + 8.0, r.min.y), r.max);
        controls::label(self.ui, tr, text, hf, p.text_secondary);
    }
}

/// The sections a hotkey can take from the defaults or override.
#[derive(Clone, Copy, PartialEq)]
enum Section {
    General,
    Image,
    Capture,
    Upload,
    Tools,
    Actions,
    Advanced,
}

impl Section {
    fn uses_default(self, t: &TaskSettings) -> bool {
        match self {
            Section::General => t.UseDefaultGeneralSettings,
            Section::Image => t.UseDefaultImageSettings,
            Section::Capture => t.UseDefaultCaptureSettings,
            Section::Upload => t.UseDefaultUploadSettings,
            Section::Tools => t.UseDefaultToolsSettings,
            Section::Actions => t.UseDefaultActions,
            Section::Advanced => t.UseDefaultAdvancedSettings,
        }
    }

    fn set_uses_default(self, t: &mut TaskSettings, v: bool) {
        match self {
            Section::General => t.UseDefaultGeneralSettings = v,
            Section::Image => t.UseDefaultImageSettings = v,
            Section::Capture => t.UseDefaultCaptureSettings = v,
            Section::Upload => t.UseDefaultUploadSettings = v,
            Section::Tools => t.UseDefaultToolsSettings = v,
            Section::Actions => t.UseDefaultActions = v,
            Section::Advanced => t.UseDefaultAdvancedSettings = v,
        }
    }
}

/// A section of the edited task settings (always there: see `ensure_sections`).
macro_rules! sec {
    ($f:expr, $s:ident) => {
        $f.s.task.$s.get_or_insert_default()
    };
}

/// A `Check` bound to a field; evaluates to its value.
macro_rules! chk {
    ($f:expr, $text:expr, $s:ident . $($field:ident).+) => {{
        let mut v = sec!($f, $s).$($field).+;
        if $f.check($text, &mut v, true) {
            sec!($f, $s).$($field).+ = v;
        }
        v
    }};
    ($f:expr, $text:expr, $enabled:expr, $s:ident . $($field:ident).+) => {{
        let mut v = sec!($f, $s).$($field).+;
        if $f.check($text, &mut v, $enabled) {
            sec!($f, $s).$($field).+ = v;
        }
        v
    }};
}

/// A `Row` with an `int` `Number`.
macro_rules! num {
    ($f:expr, $label:expr, $min:expr, $max:expr, $enabled:expr, $s:ident . $($field:ident).+) => {{
        let mut v = sec!($f, $s).$($field).+ as i64;
        if $f.number($label, LABEL_W, $min as i64, $max as i64, &mut v, $enabled, None) {
            sec!($f, $s).$($field).+ = v as i32;
        }
    }};
}

/// A `Row` with an `EnumCombo`.
macro_rules! enum_combo {
    ($f:expr, $label:expr, $enabled:expr, $s:ident . $($field:ident).+) => {{
        let mut v = sec!($f, $s).$($field).+.clone();
        if $f.enum_combo($label, LABEL_W, &mut v, $enabled) {
            sec!($f, $s).$($field).+ = v;
        }
    }};
}

/// A `Row` with a `Text` box.
macro_rules! text {
    ($f:expr, $label:expr, $enabled:expr, $s:ident . $($field:ident).+) => {{
        let mut v = sec!($f, $s).$($field).+.clone();
        if $f.text_row(Some(($label, LABEL_W)), &mut v, $enabled, false) {
            sec!($f, $s).$($field).+ = v;
        }
    }};
}

/// A bool in a section kept as JSON (`OCROptions` and the tools' options).
fn jbool(v: &Value, key: &str, default: bool) -> bool {
    v.get(key).and_then(Value::as_bool).unwrap_or(default)
}

fn jstr(v: &Value, key: &str) -> String {
    v.get(key).and_then(Value::as_str).unwrap_or_default().to_string()
}

fn jset(v: &mut Value, key: &str, x: Value) {
    if !v.is_object() {
        *v = Value::Object(Default::default());
    }
    v[key] = x;
}

/// A `Check` bound to a bool in a JSON object.
fn jcheck(f: &mut Form, text: &str, v: &mut Value, key: &str, default: bool, enabled: bool) -> bool {
    let mut b = jbool(v, key, default);
    if f.check(text, &mut b, enabled) {
        jset(v, key, Value::Bool(b));
    }
    b
}

/// `ParentPage`: General and Upload only hold their override card.
fn parent_page(f: &mut Form, section: Section, text: &str) {
    f.override_section(section, text);
}

fn page_task(f: &mut Form) {
    f.card(4.0, &mut |f| {
        f.title(s::TaskSettingsWindow_Task());
        let job = f.s.task.Job.clone();
        if let Some(anchor) = f.menu_button(Some(s::TaskSettingsWindow_TaskLabel()), "task-menu", find_menu_icon(&job), &job.localized_description(), 250.0, false, true) {
            let mut fl = Flyout::menu(Id::new("task-menu-flyout"), map_entries(task_menu(&job), &MenuCmd::Task), anchor, Placement::BottomEdgeAlignedLeft);
            fl.style = MenuStyle::Standard;
            f.w.menu = Some(fl);
        }
        let mut v = f.s.task.Description.clone();
        if f.text_row(Some((s::TaskSettingsWindow_Description(), LABEL_W)), &mut v, true, false) {
            f.s.task.Description = v;
        }
    });
    // After capture tasks.
    f.card(4.0, &mut |f| {
        f.title(s::TaskSettingsWindow_AfterCaptureTasks());
        let mut o = !f.s.task.UseDefaultAfterCaptureJob;
        if f.check(s::TaskSettingsWindow_OverrideAfterCaptureTasks(), &mut o, true) {
            f.s.task.UseDefaultAfterCaptureJob = !o;
        }
        let value = f.s.task.AfterCaptureJob;
        let titles: Vec<String> = AFTER_CAPTURE_TASKS
            .iter()
            .filter(|(n, _)| AfterCaptureTasks::parse(n).is_some_and(|fl| value.contains(fl)))
            .map(|(n, _)| sharex_settings::localized_description("AfterCaptureTasks", n).unwrap_or_else(|| n.to_string()))
            .collect();
        let text = if titles.is_empty() { sharex_settings::localized_description("AfterCaptureTasks", "None").unwrap_or_else(|| "None".into()) } else { titles.join(", ") };
        let glyph = f.w.last_after_capture.and_then(|n| AFTER_CAPTURE_TASKS.iter().find(|(t, _)| *t == n)).map_or(icon::ImageUpload, |(_, i)| *i);
        if let Some(anchor) = f.menu_button(None, "after-capture", glyph, &text, 300.0, true, o) {
            let entries = AFTER_CAPTURE_TASKS
                .iter()
                .map(|(n, i)| {
                    let checked = AfterCaptureTasks::parse(n).is_some_and(|fl| value.contains(fl));
                    MenuEntry::item(sharex_settings::localized_description("AfterCaptureTasks", n).unwrap_or_else(|| n.to_string()), i, MenuCmd::AfterCapture(n)).toggle(Toggle::CheckBox, checked)
                })
                .collect();
            f.w.menu = Some(Flyout::menu(Id::new("after-capture-flyout"), entries, anchor, Placement::BottomEdgeAlignedLeft));
        }
    });
    f.card(4.0, &mut |f| {
        f.title(s::TaskSettingsWindow_AfterUploadTasks());
        let mut o = !f.s.task.UseDefaultAfterUploadJob;
        if f.check(s::TaskSettingsWindow_OverrideAfterUploadTasks(), &mut o, true) {
            f.s.task.UseDefaultAfterUploadJob = !o;
        }
        let value = f.s.task.AfterUploadJob;
        let titles: Vec<String> = AFTER_UPLOAD_TASKS
            .iter()
            .filter(|(n, _)| AfterUploadTasks::parse(n).is_some_and(|fl| value.contains(fl)))
            .map(|(n, _)| sharex_settings::localized_description("AfterUploadTasks", n).unwrap_or_else(|| n.to_string()))
            .collect();
        let text = if titles.is_empty() { sharex_settings::localized_description("AfterUploadTasks", "None").unwrap_or_else(|| "None".into()) } else { titles.join(", ") };
        let glyph = f.w.last_after_upload.and_then(|n| AFTER_UPLOAD_TASKS.iter().find(|(t, _)| *t == n)).map_or(icon::CloudUpload, |(_, i)| *i);
        if let Some(anchor) = f.menu_button(None, "after-upload", glyph, &text, 300.0, true, o) {
            let entries = AFTER_UPLOAD_TASKS
                .iter()
                .map(|(n, i)| {
                    let checked = AfterUploadTasks::parse(n).is_some_and(|fl| value.contains(fl));
                    MenuEntry::item(sharex_settings::localized_description("AfterUploadTasks", n).unwrap_or_else(|| n.to_string()), i, MenuCmd::AfterUpload(n)).toggle(Toggle::CheckBox, checked)
                })
                .collect();
            f.w.menu = Some(Flyout::menu(Id::new("after-upload-flyout"), entries, anchor, Placement::BottomEdgeAlignedLeft));
        }
    });
    f.card(4.0, &mut |f| {
        f.title(s::TaskSettingsWindow_Destinations());
        let mut o = !f.s.task.UseDefaultDestinations;
        if f.check(s::TaskSettingsWindow_OverrideDestinations(), &mut o, true) {
            f.s.task.UseDefaultDestinations = !o;
        }
        let text = format!("{}...", s::TaskSettingsWindow_Destinations());
        if let Some(anchor) = f.menu_button(None, "destinations", icon::Destinations, &text, 300.0, true, o) {
            let entries = crate::main_menu::destinations_menu(&crate::main_menu::MenuState::default());
            // `CreateDestinationMenuItem` takes the entry's `Header` alone:
            // no accent on the selected service's name, as the main menu has.
            let entries = destination_items(map_entries(entries, &|_| MenuCmd::Destination));
            f.w.menu = Some(Flyout::menu(Id::new("destinations-flyout"), entries, anchor, Placement::BottomEdgeAlignedLeft));
        }
    });
    // Uploader accounts: FTP accounts and custom uploaders aren't loaded yet
    // (`UploadersConfig`), so the card has only its title, as ShareX's
    // does without any.
    f.card(4.0, &mut |f| f.title(s::TaskSettingsWindow_UploaderAccounts()));
    f.card(4.0, &mut |f| {
        f.title(s::TaskSettingsWindow_ScreenshotsFolder());
        let mut o = f.s.task.OverrideScreenshotsFolder;
        if f.check(s::TaskSettingsWindow_OverrideScreenshotsFolder(), &mut o, true) {
            f.s.task.OverrideScreenshotsFolder = o;
        }
        let saved = f.enabled;
        f.enabled = saved && o;
        let mut v = f.s.task.ScreenshotsFolder.clone();
        let (changed, clicked) = f.text_with_buttons_spaced("screenshots-folder", &mut v, &[s::TaskSettingsWindow_BrowseWithEllipsis()], 6.0);
        if changed {
            f.s.task.ScreenshotsFolder = v;
        }
        if clicked.is_some()
            && let Some(dir) = crate::app_settings::pick_folder(s::TaskSettingsWindow_ChooseScreenshotsFolder())
        {
            f.s.task.ScreenshotsFolder = dir.display().to_string();
        }
        f.enabled = saved;
    });
}

/// Menu entries with their commands mapped.
fn destination_items<C: Clone>(entries: Vec<MenuEntry<C>>) -> Vec<MenuEntry<C>> {
    entries
        .into_iter()
        .map(|mut e| {
            e.accent = None;
            // `CreateAccentMenuIcon`: 16px in an 18-wide box, in Fluent's
            // 16x16 icon `Viewbox`.
            if let sharex_ui::menu::MenuIcon::Key(k) = e.icon {
                e.icon = sharex_ui::menu::MenuIcon::Accent(k, 16.0 * 16.0 / 18.0);
            }
            e.children = e.children.map(destination_items);
            e
        })
        .collect()
}

fn map_entries<A: Clone, B: Clone>(entries: Vec<MenuEntry<A>>, map: &dyn Fn(A) -> B) -> Vec<MenuEntry<B>> {
    entries
        .into_iter()
        .map(|e| MenuEntry {
            header: e.header,
            accent: e.accent,
            icon: e.icon,
            command: e.command.map(map),
            children: e.children.map(|c| map_entries(c, map)),
            categories: None,
            separator: e.separator,
            enabled: e.enabled,
            visible: e.visible,
            checked: e.checked,
            toggle: e.toggle,
            stays_open: e.stays_open,
            bold_when_checked: e.bold_when_checked,
            gesture: e.gesture,
            badge: e.badge,
            id: e.id,
        })
        .collect()
}

fn page_notifications(f: &mut Form) {
    f.enabled_card(Section::General, &mut |f| {
        f.title(s::TaskSettingsWindow_Sounds());
        chk!(f, s::TaskSettingsWindow_PlaySoundAfterCaptureIsMade(), GeneralSettings.PlaySoundAfterCapture);
        chk!(f, s::TaskSettingsWindow_PlaySoundAfterTaskIsCompleted(), GeneralSettings.PlaySoundAfterUpload);
        chk!(f, s::TaskSettingsWindow_PlaySoundAfterActionIsCompleted(), GeneralSettings.PlaySoundAfterAction);
    });
    f.enabled_card(Section::General, &mut |f| {
        f.title(s::TaskSettingsWindow_ToastNotification());
        let show = chk!(f, s::TaskSettingsWindow_ShowToastNotificationAfterTaskIsCompleted(), GeneralSettings.ShowToastNotificationAfterTaskCompleted);
        let saved = f.enabled;
        f.enabled = saved && show;
        f.stack(4.0, &mut |f| {
            let g = sec!(f, GeneralSettings);
            let mut v = g.ToastWindowDuration as f64;
            if f.number_f64(s::TaskSettingsWindow_DurationSeconds(), LABEL_W, 0.0, 60.0, 0.1, decimals(v), &mut v, true) {
                sec!(f, GeneralSettings).ToastWindowDuration = v as f32;
            }
            let mut v = sec!(f, GeneralSettings).ToastWindowFadeDuration as f64;
            if f.number_f64(s::TaskSettingsWindow_FadeDurationSeconds(), LABEL_W, 0.0, 10.0, 0.1, decimals(v), &mut v, true) {
                sec!(f, GeneralSettings).ToastWindowFadeDuration = v as f32;
            }
            enum_combo!(f, s::TaskSettingsWindow_Placement(), true, GeneralSettings.ToastWindowPlacement);
            toast_monitor_row(f);
            let mut w = sec!(f, GeneralSettings).ToastWindowSize.width as i64;
            if f.number(s::TaskSettingsWindow_Width(), LABEL_W, 100, 2000, &mut w, true, None) {
                sec!(f, GeneralSettings).ToastWindowSize.width = w as i32;
            }
            let mut h = sec!(f, GeneralSettings).ToastWindowSize.height as i64;
            if f.number(s::TaskSettingsWindow_Height(), LABEL_W, 50, 2000, &mut h, true, None) {
                sec!(f, GeneralSettings).ToastWindowSize.height = h as i32;
            }
            enum_combo!(f, s::TaskSettingsWindow_LeftClickAction(), true, GeneralSettings.ToastWindowLeftClickAction);
            enum_combo!(f, s::TaskSettingsWindow_RightClickAction(), true, GeneralSettings.ToastWindowRightClickAction);
            enum_combo!(f, s::TaskSettingsWindow_MiddleClickAction(), true, GeneralSettings.ToastWindowMiddleClickAction);
            num!(f, s::TaskSettingsWindow_NotificationButtonSize(), 16, 128, true, GeneralSettings.ToastWindowButtonSize);
            if label_button_row(f, s::TaskSettingsWindow_NotificationButtonsLabel(), s::TaskSettingsWindow_ConfigureWithEllipsis()) {
                let buttons = sec!(f, GeneralSettings).ToastWindowButtons.clone();
                f.w.notification_editor = Some(crate::task_editors::NotificationButtonsEditor::new(&buttons));
            }
            chk!(f, s::TaskSettingsWindow_AutomaticallyHideOnScreenCapture(), GeneralSettings.ToastWindowAutoHide);
            chk!(f, s::TaskSettingsWindow_DisableToastNotificationsOnFullscreen(), GeneralSettings.DisableNotificationsOnFullscreen);
        });
        f.enabled = saved;
    });
    f.enabled_card(Section::General, &mut |f| {
        f.title(s::TaskSettingsWindow_CustomSounds());
        sound_path(f, 0, s::TaskSettingsWindow_UseCustomCaptureSound());
        sound_path(f, 1, s::TaskSettingsWindow_UseCustomTaskCompletedSound());
        sound_path(f, 2, s::TaskSettingsWindow_UseCustomActionCompletedSound());
        sound_path(f, 3, s::TaskSettingsWindow_UseCustomErrorSound());
    });
}

/// The places .NET prints a `decimal` made from a `float` with: none for
/// whole numbers, else as many as it has (up to 3).
fn decimals(v: f64) -> usize {
    for d in 0..3 {
        let k = 10f64.powi(d);
        if ((v * k).round() - v * k).abs() < 1e-6 {
            return d as usize;
        }
    }
    3
}

/// A `Row` whose editor is a compact button.
fn label_button_row(f: &mut Form, label: &str, button: &str) -> bool {
    f.text(label);
    f.text(button);
    if f.collecting() {
        return false;
    }
    let b = Button::compact(f.d.settings_button_padding);
    let bh = f.d.settings_control_min_height;
    let m = f.style.button_margin;
    let r = f.slot(m + bh + m);
    let p = palette(f.ui.ctx());
    controls::label(f.ui, Rect::from_min_max(r.min, pos2(r.min.x + LABEL_W, r.max.y)), label, Form::label_font(), p.text_secondary);
    let w = b.width(f.ui, button);
    let br = Rect::from_min_size(pos2(r.min.x + LABEL_W + 8.0, r.min.y + m), vec2(w, bh));
    b.show(f.ui, br, f.id(button), button, f.enabled).clicked()
}

/// `ToastMonitorCombo`: active monitor, primary monitor, then the others.
fn toast_monitor_row(f: &mut Form) {
    use sharex_settings::ToastWindowMonitor as M;
    let g = sec!(f, GeneralSettings);
    let mut choices: Vec<(M, String, String)> = vec![
        (M::ActiveMonitor, String::new(), M::ActiveMonitor.localized_description()),
        (M::PrimaryMonitor, String::new(), M::PrimaryMonitor.localized_description()),
    ];
    let saved_name = g.ToastWindowMonitorDeviceName.clone();
    if g.ToastWindowMonitor == M::SpecificMonitor && !saved_name.is_empty() {
        let number: String = saved_name.chars().rev().take_while(char::is_ascii_digit).collect::<Vec<_>>().into_iter().rev().collect();
        let number = if number.is_empty() { saved_name.clone() } else { number };
        choices.push((M::SpecificMonitor, saved_name.clone(), s::TaskSettingsWindow_ToastMonitorNotConnected0().replace("{0}", &number)));
    }
    let current = choices
        .iter()
        .position(|(m, n, _)| *m == g.ToastWindowMonitor && (*m != M::SpecificMonitor || n.eq_ignore_ascii_case(&saved_name)))
        .unwrap_or(0);
    let items: Vec<ComboItem> = choices.iter().map(|(_, _, t)| ComboItem::text(t.clone())).collect();
    if let Some(i) = f.combo(s::TaskSettingsWindow_Monitor(), LABEL_W, &items, Some(current), ComboSize::default(), true) {
        let g = sec!(f, GeneralSettings);
        g.ToastWindowMonitor = choices[i].0.clone();
        g.ToastWindowMonitorDeviceName = choices[i].1.clone();
    }
}

/// `SoundPath`: the check box, then the path and "..." under it.
fn sound_path(f: &mut Form, k: usize, title: &str) {
    let g = sec!(f, GeneralSettings);
    let (mut on, mut path) = match k {
        0 => (g.UseCustomCaptureSound, g.CustomCaptureSoundPath.clone()),
        1 => (g.UseCustomTaskCompletedSound, g.CustomTaskCompletedSoundPath.clone()),
        2 => (g.UseCustomActionCompletedSound, g.CustomActionCompletedSoundPath.clone()),
        _ => (g.UseCustomErrorSound, g.CustomErrorSoundPath.clone()),
    };
    f.stack(3.0, &mut |f| {
        let changed_on = f.check(title, &mut on, true);
        let saved = f.enabled;
        f.enabled = saved && on;
        let (changed, clicked) = f.text_with_buttons_spaced(&format!("sound-{k}"), &mut path, &["..."], 6.0);
        f.enabled = saved;
        if clicked.is_some() {
            f.effect(Effect::NotPorted("sound file picker"));
        }
        if changed_on || changed {
            let g = sec!(f, GeneralSettings);
            match k {
                0 => (g.UseCustomCaptureSound, g.CustomCaptureSoundPath) = (on, path.clone()),
                1 => (g.UseCustomTaskCompletedSound, g.CustomTaskCompletedSoundPath) = (on, path.clone()),
                2 => (g.UseCustomActionCompletedSound, g.CustomActionCompletedSoundPath) = (on, path.clone()),
                _ => (g.UseCustomErrorSound, g.CustomErrorSoundPath) = (on, path.clone()),
            }
        }
    });
}

fn page_image(f: &mut Form) {
    use sharex_settings::EImageFormat as F;
    f.override_section(Section::Image, s::TaskSettingsWindow_OverrideImageSettings());
    let format = sec!(f, ImageSettings).ImageFormat.clone();
    f.enabled_card(Section::Image, &mut |f| {
        f.title(s::TaskSettingsWindow_ImageQuality());
        enum_combo!(f, s::TaskSettingsWindow_ImageFormat(), true, ImageSettings.ImageFormat);
        let format = sec!(f, ImageSettings).ImageFormat.clone();
        match format {
            F::PNG => {
                enum_combo!(f, s::TaskSettingsWindow_PNGBitDepth(), true, ImageSettings.ImagePNGBitDepth);
                let items: Vec<ComboItem> = (0..10).map(|i| ComboItem::text(i.to_string())).collect();
                let cur = sec!(f, ImageSettings).ImagePNGCompressionLevel.clamp(0, 9) as usize;
                if let Some(i) = f.combo(s::TaskSettingsWindow_PNGCompressionLevel(), LABEL_W, &items, Some(cur), ComboSize::default(), true) {
                    sec!(f, ImageSettings).ImagePNGCompressionLevel = i as i32;
                }
                let filters = [
                    ("AllFilters", s::TaskSettingsWindow_PNGFilterAutomatic()),
                    ("None", s::TaskSettingsWindow_PNGFilterNone()),
                    ("Sub", s::TaskSettingsWindow_PNGFilterSub()),
                    ("Up", s::TaskSettingsWindow_PNGFilterUp()),
                    ("Avg", s::TaskSettingsWindow_PNGFilterAverage()),
                    ("Paeth", s::TaskSettingsWindow_PNGFilterPaeth()),
                ];
                raw_combo(f, s::TaskSettingsWindow_PNGFilter(), &filters, true);
            }
            F::JPEG => {
                num!(f, s::TaskSettingsWindow_JPEGQuality(), 0, 100, true, ImageSettings.ImageJPEGQuality);
                let subs = [
                    ("Downsample420", s::TaskSettingsWindow_JPEGSubsampling420()),
                    ("Downsample422", s::TaskSettingsWindow_JPEGSubsampling422()),
                    ("Downsample444", s::TaskSettingsWindow_JPEGSubsampling444()),
                ];
                raw_combo(f, s::TaskSettingsWindow_JPEGSubsampling(), &subs, false);
            }
            F::GIF => enum_combo!(f, s::TaskSettingsWindow_GIFQuality(), true, ImageSettings.ImageGIFQuality),
            F::BMP => enum_combo!(f, s::TaskSettingsWindow_BMPBitDepth(), true, ImageSettings.ImageBMPBitDepth),
            F::WEBP => {
                f.stack(4.0, &mut |f| {
                    let lossless = chk!(f, s::TaskSettingsWindow_WebPLossless(), ImageSettings.ImageWebPLossless);
                    if lossless {
                        num!(f, s::TaskSettingsWindow_WebPCompressionEffort(), 0, 100, true, ImageSettings.ImageWebPCompressionEffort);
                    } else {
                        num!(f, s::TaskSettingsWindow_WebPQuality(), 0, 100, true, ImageSettings.ImageWebPQuality);
                    }
                });
            }
            F::Unknown(_) => {}
        }
        enum_combo!(f, s::TaskSettingsWindow_IfFileExists(), true, ImageSettings.FileExistAction);
    });
    // Automatic JPEG: not when the format is JPEG already.
    if format != F::JPEG {
        f.enabled_card(Section::Image, &mut |f| {
            f.title(s::TaskSettingsWindow_AutomaticJPEG());
            let on = chk!(f, s::TaskSettingsWindow_UseJPEGIfImageSizeIsBiggerThanTheSpecifiedSize(), ImageSettings.ImageAutoUseJPEG);
            num!(f, s::TaskSettingsWindow_SizeLimitKB(), 0, i32::MAX, on, ImageSettings.ImageAutoUseJPEGSize);
            chk!(f, s::TaskSettingsWindow_AdjustJPEGQualityAutomaticallyToKeepImageSizeNearTheSpecifiedSize(), on, ImageSettings.ImageAutoJPEGQuality);
        });
    }
}

/// A combo over SkiaSharp's encoder enums, kept by name (`RawEnum`).
fn raw_combo(f: &mut Form, label: &str, options: &[(&str, &str)], png: bool) {
    let img = sec!(f, ImageSettings);
    let raw = if png { &img.ImagePNGFilter } else { &img.ImageJPEGSubsampling };
    let current = raw.0.as_str().and_then(|n| options.iter().position(|(o, _)| o.eq_ignore_ascii_case(n))).unwrap_or(0);
    let items: Vec<ComboItem> = options.iter().map(|(_, t)| ComboItem::text(*t)).collect();
    if let Some(i) = f.combo(label, LABEL_W, &items, Some(current), ComboSize::default(), true) {
        let v = sharex_settings::RawEnum::name(options[i].0);
        let img = sec!(f, ImageSettings);
        if png {
            img.ImagePNGFilter = v;
        } else {
            img.ImageJPEGSubsampling = v;
        }
    }
}

fn page_effects(f: &mut Form) {
    f.enabled_card(Section::Image, &mut |f| {
        f.title(s::TaskSettingsWindow_ImageEffects());
        chk!(f, s::TaskSettingsWindow_ShowImageEffectsWindowAfterCapture(), ImageSettings.ShowImageEffectsWindowAfterCapture);
        chk!(f, s::TaskSettingsWindow_OnlyApplyEffectsToRegionCapture(), ImageSettings.ImageEffectOnlyRegionCapture);
        chk!(f, s::TaskSettingsWindow_UseRandomImageEffect(), ImageSettings.UseRandomImageEffect);
        if f.button(s::TaskSettingsWindow_ImageEffectsConfigurationWithEllipsis(), true) {
            f.effect(Effect::ImageEffects);
        }
        f.hint(s::TaskSettingsWindow_YouCanEnableOrDisableImageEffectsFromAfterCaptureTasksAddImageEffects());
    });
}

fn page_thumbnail(f: &mut Form) {
    f.enabled_card(Section::Image, &mut |f| {
        f.title(s::TaskSettingsWindow_Thumbnail());
        num!(f, s::TaskSettingsWindow_Width(), 0, 10000, true, ImageSettings.ThumbnailWidth);
        num!(f, s::TaskSettingsWindow_Height(), 0, 10000, true, ImageSettings.ThumbnailHeight);
        text!(f, s::TaskSettingsWindow_ThumbnailName(), true, ImageSettings.ThumbnailName);
        let preview = format!("{}{}.jpg", s::TaskSettingsWindow_ImageNameSample(), sec!(f, ImageSettings).ThumbnailName);
        f.hint_row(s::TaskSettingsWindow_Preview(), &preview);
        chk!(f, s::TaskSettingsWindow_CreateThumbnailOnlyIfImageIsBiggerThanThumbnailSize(), ImageSettings.ThumbnailCheckSize);
    });
}

fn page_capture(f: &mut Form) {
    f.override_section(Section::Capture, s::TaskSettingsWindow_OverrideCaptureSettings());
    f.enabled_card(Section::Capture, &mut |f| {
        f.title(s::TaskSettingsWindow_Screenshots());
        chk!(f, s::TaskSettingsWindow_ShowCursorInScreenshots(), CaptureSettings.ShowCursor);
        let mut v = sec!(f, CaptureSettings).ScreenshotDelay;
        // A `decimal` keeps its scale: ShareX writes the delay as "0.0".
        if f.number_f64(s::TaskSettingsWindow_ScreenshotDelaySeconds(), LABEL_W, 0.0, 60.0, 0.1, 1, &mut v, true) {
            sec!(f, CaptureSettings).ScreenshotDelay = v;
        }
        let transparent = chk!(f, s::TaskSettingsWindow_CaptureWindowWithTransparency(), CaptureSettings.CaptureTransparent);
        chk!(f, s::TaskSettingsWindow_CaptureWindowWithShadow(), transparent, CaptureSettings.CaptureShadow);
        num!(f, s::TaskSettingsWindow_ShadowOffset(), 0, 1000, transparent, CaptureSettings.CaptureShadowOffset);
        chk!(f, s::TaskSettingsWindow_CaptureClientAreaForWindowCaptures(), CaptureSettings.CaptureClientArea);
        chk!(f, s::TaskSettingsWindow_HideTaskbarWhenItIntersectsACapturedWindow(), CaptureSettings.CaptureAutoHideTaskbar);
        chk!(f, s::TaskSettingsWindow_AutomaticallyHideDesktopIcons(), CaptureSettings.CaptureAutoHideDesktopIcons);
        chk!(f, s::TaskSettingsWindow_HDRScreenshotColorCorrector(), CaptureSettings.HDRScreenshotColorCorrection);
    });
    f.enabled_card(Section::Capture, &mut |f| {
        f.title(s::TaskSettingsWindow_PreconfiguredRegion());
        region_grid(f);
        if f.button(s::TaskSettingsWindow_SelectRegionWithEllipsis(), true) {
            f.effect(Effect::SelectRegion);
        }
    });
    f.enabled_card(Section::Capture, &mut |f| {
        f.title(s::TaskSettingsWindow_PreconfiguredWindow());
        text!(f, s::TaskSettingsWindow_WindowTitle(), true, CaptureSettings.CaptureCustomWindow);
    });
}

/// The preconfigured region: `ColumnDefinitions="Auto,130,Auto,130"`,
/// two rows, `ColumnSpacing="8" RowSpacing="4"`.
fn region_grid(f: &mut Form) {
    let labels = [s::TaskSettingsWindow_X(), s::TaskSettingsWindow_Y(), s::TaskSettingsWindow_Width(), s::TaskSettingsWindow_Height()];
    for l in labels {
        f.text(l);
    }
    if f.collecting() {
        return;
    }
    let nh = f.d.settings_number_box_height;
    let r = f.slot(nh + 4.0 + nh);
    let p = palette(f.ui.ctx());
    let lf = Form::label_font();
    // The Auto columns are as wide as their widest label.
    let c0 = text_width(f.ui, labels[0], lf.clone()).max(text_width(f.ui, labels[2], lf.clone()));
    let c2 = text_width(f.ui, labels[1], lf.clone()).max(text_width(f.ui, labels[3], lf.clone()));
    let xs = [r.min.x, r.min.x + c0 + 8.0, r.min.x + c0 + 8.0 + 130.0 + 8.0, r.min.x + c0 + 8.0 + 130.0 + 8.0 + c2 + 8.0];
    let enabled = f.enabled;
    let rect = sec!(f, CaptureSettings).CaptureCustomRegion;
    let mut vals = [rect.x as i64, rect.y as i64, rect.width as i64, rect.height as i64];
    let mins = [-100000, -100000, 0, 0];
    let mut changed = false;
    for (k, label) in labels.iter().enumerate() {
        let row = k / 2;
        let col = k % 2;
        let y = r.min.y + row as f32 * (nh + 4.0);
        let cell = Rect::from_min_size(pos2(xs[col * 2], y), vec2(if col == 0 { c0 } else { c2 }, nh));
        controls::label(f.ui, cell, label, lf.clone(), p.text_secondary);
        let nr = Rect::from_min_size(pos2(xs[col * 2 + 1], y), vec2(130.0, nh));
        let id = f.id(&format!("region-{k}"));
        changed |= controls::NumericUpDown { minimum: mins[k], maximum: 100000, enabled }.show(f.ui, nr, id, &mut vals[k]);
    }
    if changed {
        sec!(f, CaptureSettings).CaptureCustomRegion = sharex_settings::types::Rectangle::new(vals[0] as i32, vals[1] as i32, vals[2] as i32, vals[3] as i32);
    }
}

fn page_region_capture(f: &mut Form) {
    use sharex_settings::region_capture::limits as l;
    f.enabled_card(Section::Capture, &mut |f| {
        f.title(s::TaskSettingsWindow_Selection());
        // Multi region mode keeps regions open for editing, so it always
        // overrides quick capture.
        let multi = sec!(f, CaptureSettings).RegionCaptureOptions.MultiRegionMode;
        chk!(f, s::TaskSettingsWindow_QuickCapture(), !multi, CaptureSettings.RegionCaptureOptions.QuickCapture);
        chk!(f, s::TaskSettingsWindow_MultiRegionMode(), CaptureSettings.RegionCaptureOptions.MultiRegionMode);
        let detect = chk!(f, s::TaskSettingsWindow_DetectWindowRegions(), CaptureSettings.RegionCaptureOptions.DetectWindows);
        chk!(f, s::TaskSettingsWindow_AlsoDetectControlsInsideWindows(), detect, CaptureSettings.RegionCaptureOptions.DetectControls);
        chk!(f, s::TaskSettingsWindow_RestrictCaptureAndCursorToTheActiveMonitor(), CaptureSettings.RegionCaptureOptions.ActiveMonitorMode);
        num!(f, s::TaskSettingsWindow_BackgroundDimStrengthPercent(), 0, 100, true, CaptureSettings.RegionCaptureOptions.BackgroundDimStrength);
        chk!(f, s::TaskSettingsWindow_DimHoveredWindowsAndControls(), CaptureSettings.RegionCaptureOptions.DimAllExceptSelection);
        chk!(f, s::TaskSettingsWindow_CompactRegionCaptureToolbar(), CaptureSettings.RegionCaptureOptions.CompactToolbar);
        let custom = chk!(f, s::TaskSettingsWindow_UseCustomToolbarButtonSize(), CaptureSettings.RegionCaptureOptions.UseCustomToolbarButtonSize);
        num!(f, s::TaskSettingsWindow_ToolbarButtonSizePixels(), l::TOOLBAR_BUTTON_SIZE_MINIMUM, l::TOOLBAR_BUTTON_SIZE_MAXIMUM, custom, CaptureSettings.RegionCaptureOptions.ToolbarButtonSize);
        if f.button(s::TaskSettingsWindow_CustomizeToolbarWithEllipsis(), true) {
            // `ShowRegionCaptureToolbarEditor`.
            let saved = sec!(f, CaptureSettings).RegionCaptureOptions.ToolbarItems.clone();
            f.w.toolbar_editor = Some(crate::toolbar_editor::ToolbarEditor::new(crate::toolbar_editor::Target::RegionCapture, &saved));
        }
    });
    f.enabled_card(Section::Capture, &mut |f| {
        f.title(s::TaskSettingsWindow_MouseActions());
        enum_combo!(f, s::TaskSettingsWindow_RightClick(), true, CaptureSettings.RegionCaptureOptions.RegionCaptureActionRightClick);
        enum_combo!(f, s::TaskSettingsWindow_MiddleClick(), true, CaptureSettings.RegionCaptureOptions.RegionCaptureActionMiddleClick);
        enum_combo!(f, s::TaskSettingsWindow_Mouse4Click(), true, CaptureSettings.RegionCaptureOptions.RegionCaptureActionX1Click);
        enum_combo!(f, s::TaskSettingsWindow_Mouse5Click(), true, CaptureSettings.RegionCaptureOptions.RegionCaptureActionX2Click);
        enum_combo!(f, s::TaskSettingsWindow_MouseWheel(), true, CaptureSettings.RegionCaptureOptions.MagnifierScrollAction);
    });
    f.enabled_card(Section::Capture, &mut |f| {
        f.title(s::TaskSettingsWindow_InformationAndMagnifier());
        chk!(f, s::TaskSettingsWindow_ShowPositionAndSizeInfo(), CaptureSettings.RegionCaptureOptions.ShowInfo);
        let custom = chk!(f, s::TaskSettingsWindow_UseCustomInfoText(), CaptureSettings.RegionCaptureOptions.UseCustomInfoText);
        let mut v = sec!(f, CaptureSettings).RegionCaptureOptions.CustomInfoText.clone();
        if f.text_row(None, &mut v, custom, false) {
            sec!(f, CaptureSettings).RegionCaptureOptions.CustomInfoText = v.replace("\r\n", "$n").replace('\n', "$n");
        }
        let mag = chk!(f, s::TaskSettingsWindow_ShowMagnifierNearCursor(), CaptureSettings.RegionCaptureOptions.ShowMagnifier);
        chk!(f, s::TaskSettingsWindow_UseSquareMagnifier(), mag, CaptureSettings.RegionCaptureOptions.UseSquareMagnifier);
        num!(f, s::TaskSettingsWindow_MagnifierSize(), l::MAGNIFIER_SIZE_MINIMUM, l::MAGNIFIER_SIZE_MAXIMUM, mag, CaptureSettings.RegionCaptureOptions.MagnifierSize);
        num!(f, s::TaskSettingsWindow_MagnifierPixelCount(), l::MAGNIFIER_PIXEL_COUNT_MINIMUM, l::MAGNIFIER_PIXEL_COUNT_MAXIMUM, mag, CaptureSettings.RegionCaptureOptions.MagnifierPixelCount);
        chk!(f, s::TaskSettingsWindow_ShowCenterCrosshair(), CaptureSettings.RegionCaptureOptions.ShowCenterCrosshair);
        chk!(f, s::TaskSettingsWindow_ShowScreenWideCrosshair(), CaptureSettings.RegionCaptureOptions.ShowScreenCrosshair);
    });
}

fn page_screen_recorder(f: &mut Form) {
    use sharex_settings::{CameraCaptureResolution as Res, CameraOverlayPosition as Pos, CameraOverlayShape as Shape};
    let native = sec!(f, CaptureSettings).ScreenRecordUseNative;
    f.enabled_card(Section::Capture, &mut |f| {
        f.title(s::TaskSettingsWindow_NativeRecorder());
        let native = chk!(f, s::TaskSettingsWindow_NativeRecorderEnabled(), CaptureSettings.ScreenRecordUseNative);
        let sys = chk!(f, s::TaskSettingsWindow_NativeRecorderSystemAudio(), native, CaptureSettings.ScreenRecordSystemAudio);
        device_row(f, s::TaskSettingsWindow_NativeRecorderSystemAudioDevice(), s::TaskSettingsWindow_NativeRecorderDefaultSystemAudioDevice(), s::TaskSettingsWindow_NativeRecorderUnavailableSystemAudioDevice(), native && sys, 0);
        let mic = chk!(f, s::TaskSettingsWindow_NativeRecorderMicrophone(), native, CaptureSettings.ScreenRecordMicrophone);
        device_row(f, s::TaskSettingsWindow_NativeRecorderMicrophoneDevice(), s::TaskSettingsWindow_NativeRecorderDefaultMicrophone(), s::TaskSettingsWindow_NativeRecorderUnavailableMicrophone(), native && mic, 1);
        chk!(f, s::TaskSettingsWindow_NativeRecorderHardware(), native, CaptureSettings.ScreenRecordRequireHardwareEncoder);
        let mut v = sec!(f, CaptureSettings).ScreenRecordVideoBitrate as f64;
        if f.number_f64_w(s::TaskSettingsWindow_NativeRecorderBitrate(), LABEL_W, 160.0, 100.0, 200000.0, 100.0, 0, &mut v, native) {
            sec!(f, CaptureSettings).ScreenRecordVideoBitrate = v as i32;
        }
        chk!(f, s::TaskSettingsWindow_NativeRecorderHDRToneMapping(), native, CaptureSettings.ScreenRecordHDRToneMapping);
    });
    f.enabled_card(Section::Capture, &mut |f| {
        f.title(s::TaskSettingsWindow_NativeRecorderCamera());
        let saved = f.enabled;
        f.enabled = saved && native;
        f.stack(4.0, &mut |f| {
            let cam = chk!(f, s::TaskSettingsWindow_NativeRecorderCamera_Enabled(), CaptureSettings.ScreenRecordCamera);
            device_row(f, s::TaskSettingsWindow_NativeRecorderCamera_Device(), s::TaskSettingsWindow_NativeRecorderCamera_NoDevices(), s::TaskSettingsWindow_NativeRecorderCamera_UnavailableDevice(), cam, 2);
            let res = [(Res::Size640x480, "640 × 480"), (Res::Size1280x720, "1280 × 720"), (Res::Size1920x1080, "1920 × 1080")];
            let cur = res.iter().position(|(r, _)| *r == sec!(f, CaptureSettings).ScreenRecordCameraResolution).unwrap_or(1);
            let items: Vec<ComboItem> = res.iter().map(|(_, t)| ComboItem::text(*t)).collect();
            if let Some(i) = f.combo(s::TaskSettingsWindow_NativeRecorderCamera_Resolution(), LABEL_W, &items, Some(cur), ComboSize::default(), cam) {
                sec!(f, CaptureSettings).ScreenRecordCameraResolution = res[i].0.clone();
            }
            num!(f, s::TaskSettingsWindow_NativeRecorderCamera_FPS(), 1, 60, cam, CaptureSettings.ScreenRecordCameraFPS);
            let shapes = [(Shape::Rectangle, s::TaskSettingsWindow_NativeRecorderCamera_Rectangle()), (Shape::Circle, s::TaskSettingsWindow_NativeRecorderCamera_Circle())];
            let cur = shapes.iter().position(|(v, _)| *v == sec!(f, CaptureSettings).ScreenRecordCameraShape).unwrap_or(0);
            let items: Vec<ComboItem> = shapes.iter().map(|(_, t)| ComboItem::text(*t)).collect();
            if let Some(i) = f.combo(s::TaskSettingsWindow_NativeRecorderCamera_Shape(), LABEL_W, &items, Some(cur), ComboSize::default(), cam) {
                sec!(f, CaptureSettings).ScreenRecordCameraShape = shapes[i].0.clone();
            }
            let pos = [
                (Pos::TopLeft, s::ApplicationSettingsWindow_TopLeft()),
                (Pos::TopRight, s::ApplicationSettingsWindow_TopRight()),
                (Pos::BottomLeft, s::ApplicationSettingsWindow_BottomLeft()),
                (Pos::BottomRight, s::ApplicationSettingsWindow_BottomRight()),
            ];
            let cur = pos.iter().position(|(v, _)| *v == sec!(f, CaptureSettings).ScreenRecordCameraPosition).unwrap_or(3);
            let items: Vec<ComboItem> = pos.iter().map(|(_, t)| ComboItem::text(*t)).collect();
            if let Some(i) = f.combo(s::TaskSettingsWindow_Placement(), LABEL_W, &items, Some(cur), ComboSize::default(), cam) {
                sec!(f, CaptureSettings).ScreenRecordCameraPosition = pos[i].0.clone();
            }
            num!(f, s::TaskSettingsWindow_NativeRecorderCamera_Width(), 5, 50, cam, CaptureSettings.ScreenRecordCameraWidthPercent);
            num!(f, s::TaskSettingsWindow_NativeRecorderCamera_Margin(), 0, 1000, cam, CaptureSettings.ScreenRecordCameraMargin);
        });
        f.enabled = saved;
    });
    f.enabled_card(Section::Capture, &mut |f| {
        f.title(s::TaskSettingsWindow_Recording());
        chk!(f, s::ScreenRecorderBar_ShowBeforeRecording(), CaptureSettings.ScreenRecordShowBar);
        num!(f, s::TaskSettingsWindow_ScreenRecordingFPS(), 1, 120, true, CaptureSettings.ScreenRecordFPS);
        num!(f, s::TaskSettingsWindow_GIFFPS(), 1, 30, true, CaptureSettings.GIFFPS);
        chk!(f, s::TaskSettingsWindow_ShowCursorInRecording(), CaptureSettings.ScreenRecordShowCursor);
        chk!(f, s::TaskSettingsWindow_HighlightMouseWhileRecording(), CaptureSettings.ScreenRecordMouseHighlighter);
        if f.button(s::TaskSettingsWindow_MouseHighlighterOptions(), true) {
            f.w.select_page(16);
        }
        chk!(f, s::TaskSettingsWindow_ShowRecordingTimer(), CaptureSettings.ScreenRecordShowTimer);
        chk!(f, s::TaskSettingsWindow_ShowRecordingButtonLabels(), CaptureSettings.ScreenRecordShowButtonLabels);
        let delay = chk!(f, s::TaskSettingsWindow_UseStartDelay(), CaptureSettings.ScreenRecordStartDelayEnabled);
        let mut v = sec!(f, CaptureSettings).ScreenRecordStartDelay as f64;
        if f.number_f64(s::TaskSettingsWindow_StartDelaySeconds(), LABEL_W, 0.0, 3600.0, 0.1, decimals(v), &mut v, delay) {
            sec!(f, CaptureSettings).ScreenRecordStartDelay = v as f32;
        }
        let fixed = chk!(f, s::TaskSettingsWindow_UseFixedDuration(), CaptureSettings.ScreenRecordFixedDuration);
        let mut v = sec!(f, CaptureSettings).ScreenRecordDuration as f64;
        if f.number_f64(s::TaskSettingsWindow_DurationSeconds(), LABEL_W, 0.0, 86400.0, 0.1, decimals(v), &mut v, fixed) {
            sec!(f, CaptureSettings).ScreenRecordDuration = v as f32;
        }
    });
    f.enabled_card(Section::Capture, &mut |f| {
        f.title(s::TaskSettingsWindow_EncodingAndCapture());
        chk!(f, s::TaskSettingsWindow_RecordLosslesslyFirstThenApplyEncodingOptions(), !native, CaptureSettings.ScreenRecordTwoPassEncoding);
        chk!(f, s::TaskSettingsWindow_AskForConfirmationWhenAborting(), CaptureSettings.ScreenRecordAskConfirmationOnAbort);
        if f.buttons(&[(s::TaskSettingsWindow_ScreenRecordingOptionsWithEllipsis(), !native)], 6.0).is_some() {
            f.effect(Effect::FFmpegOptions);
        }
    });
}

/// `CaptureDeviceCombo`: the default device, then the devices; one that's
/// saved but not connected stays selectable. Device listing isn't ported,
/// so only the default and a saved device show.
fn device_row(f: &mut Form, label: &str, default_label: &str, unavailable: &str, enabled: bool, k: usize) {
    let c = sec!(f, CaptureSettings);
    let saved = match k {
        0 => c.ScreenRecordSystemAudioDeviceId.clone(),
        1 => c.ScreenRecordMicrophoneDeviceId.clone(),
        _ => c.ScreenRecordCameraDeviceId.clone(),
    };
    let mut items = vec![ComboItem::text(default_label)];
    if !saved.is_empty() {
        items.push(ComboItem::text(unavailable));
    }
    let cur = if saved.is_empty() { 0 } else { 1 };
    if f.combo(label, LABEL_W, &items, Some(cur), ComboSize::default(), enabled) == Some(0) {
        let c = sec!(f, CaptureSettings);
        match k {
            0 => c.ScreenRecordSystemAudioDeviceId.clear(),
            1 => c.ScreenRecordMicrophoneDeviceId.clear(),
            _ => c.ScreenRecordCameraDeviceId.clear(),
        }
    }
}

fn page_ocr(f: &mut Form) {
    f.enabled_card(Section::Capture, &mut |f| {
        f.title(s::TaskSettingsWindow_OpticalCharacterRecognition());
        let mut o = sec!(f, CaptureSettings).OCROptions.clone();
        let silent = jcheck(f, s::TaskSettingsWindow_ProcessOCRSilently(), &mut o, "Silent", false, true);
        jcheck(f, s::TaskSettingsWindow_AutomaticallyCopyResultsToClipboard(), &mut o, "AutoCopy", false, !silent);
        jcheck(f, s::TaskSettingsWindow_CloseOCRWindowAfterOpeningServiceLink(), &mut o, "CloseWindowAfterOpeningServiceLink", false, true);
        sec!(f, CaptureSettings).OCROptions = o;
    });
}

fn page_file_naming(f: &mut Form) {
    let auto = *f.s.auto_increment;
    f.enabled_card(Section::Upload, &mut |f| {
        f.title(s::TaskSettingsWindow_NamePatterns());
        let adv = f.s.task.AdvancedSettings.clone().unwrap_or_default();
        let preview = |pattern: &str, window: bool| {
            use sharex_helpers::name_parser::{NameParser, NameParserType};
            let mut p = NameParser::new(NameParserType::FileName);
            p.auto_increment_number = auto as i64;
            p.image_width = 1920;
            p.image_height = 1080;
            p.max_name_length = adv.NamePatternMaxLength.max(0) as usize;
            p.max_title_length = adv.NamePatternMaxTitleLength.max(0) as usize;
            p.is_preview_mode = true;
            if window {
                p.window_text = Some(s::TaskSettingsWindow_ShareXTaskSettings().to_string());
                p.process_name = Some("ShareX".into());
            }
            p.parse(pattern)
        };
        text!(f, s::TaskSettingsWindow_CaptureOrClipboardUpload(), true, UploadSettings.NameFormatPattern);
        let pv = preview(&sec!(f, UploadSettings).NameFormatPattern.clone(), false);
        f.hint_row(s::TaskSettingsWindow_Preview(), &pv);
        text!(f, s::TaskSettingsWindow_WindowCapture(), true, UploadSettings.NameFormatPatternActiveWindow);
        let pv = preview(&sec!(f, UploadSettings).NameFormatPatternActiveWindow.clone(), true);
        f.hint_row(s::TaskSettingsWindow_Preview(), &pv);
        chk!(f, s::TaskSettingsWindow_UseNamePatternForFileUploads(), UploadSettings.FileUploadUseNamePattern);
        chk!(f, s::TaskSettingsWindow_ReplaceURLProblematicCharactersWithUnderscores(), UploadSettings.FileUploadReplaceProblematicCharacters);
        let mut v = *f.s.auto_increment as i64;
        if f.number(s::TaskSettingsWindow_AutoIncrementNumber(), LABEL_W, 0, i32::MAX as i64, &mut v, true, None) {
            *f.s.auto_increment = v as i32;
        }
    });
    f.enabled_card(Section::Upload, &mut |f| {
        f.title(s::TaskSettingsWindow_TimeZone());
        let custom = chk!(f, s::TaskSettingsWindow_UseCustomTimeZone(), UploadSettings.UseCustomTimeZone);
        // The system's time zones aren't listed yet: the saved one shows.
        let name = jstr(&sec!(f, UploadSettings).CustomTimeZone, "DisplayName");
        f.combo(s::TaskSettingsWindow_TimeZoneLabel(), LABEL_W, &[ComboItem::text(name)], Some(0), ComboSize::default(), custom);
    });
    f.enabled_card(Section::Upload, &mut |f| {
        f.title(s::TaskSettingsWindow_URLReplacement());
        let on = chk!(f, s::TaskSettingsWindow_ReplaceResultURLUsingARegularExpression(), UploadSettings.URLRegexReplace);
        text!(f, s::TaskSettingsWindow_Pattern(), on, UploadSettings.URLRegexReplacePattern);
        text!(f, s::TaskSettingsWindow_Replacement(), on, UploadSettings.URLRegexReplaceReplacement);
    });
}

fn page_clipboard_upload(f: &mut Form) {
    f.enabled_card(Section::Upload, &mut |f| {
        f.title(s::TaskSettingsWindow_ClipboardContent());
        chk!(f, s::TaskSettingsWindow_IfClipboardContainsAFileURLDownloadAndUploadIt(), UploadSettings.ClipboardUploadURLContents);
        chk!(f, s::TaskSettingsWindow_IfClipboardContainsAURLUseURLShortener(), UploadSettings.ClipboardUploadShortenURL);
        chk!(f, s::TaskSettingsWindow_IfClipboardContainsAURLShareItUsingURLSharingService(), UploadSettings.ClipboardUploadShareURL);
        chk!(f, s::TaskSettingsWindow_IfClipboardContainsAFolderPathIndexAndUploadIt(), UploadSettings.ClipboardUploadAutoIndexFolder);
    });
}

fn page_uploader_filters(f: &mut Form) {
    f.enabled_card(Section::Upload, &mut |f| {
        f.title(s::TaskSettingsWindow_Filters());
        // `FilterTitle`: "uploader — extensions".
        let rows: Vec<String> = sec!(f, UploadSettings)
            .UploaderFilters
            .iter()
            .map(|v| {
                let ext = v.get("Extensions").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", ")).unwrap_or_default();
                format!("{} — {}", jstr(v, "Uploader"), ext)
            })
            .collect();
        let selected = f.w.filter_selected;
        if let Some(i) = f.list_box("filters", &rows, selected, 180.0) {
            f.w.filter_selected = Some(i);
        }
        // `UploaderFactory.AllGenericUploaderServices` isn't ported: no uploaders to pick.
        f.combo(s::TaskSettingsWindow_Uploader(), LABEL_W, &[], None, ComboSize::default(), false);
        let mut ext = f.w.filter_extensions.clone();
        if f.text_row(Some((s::TaskSettingsWindow_Extensions(), LABEL_W)), &mut ext, true, false) {
            f.w.filter_extensions = ext;
        }
        f.hint(s::TaskSettingsWindow_SeparateExtensionsWithCommasForExamplePngJpgJpeg());
        let has = f.w.filter_selected.is_some_and(|i| i < rows.len());
        match f.buttons(&[(s::TaskSettingsWindow_Add(), true), (s::TaskSettingsWindow_Update(), true), (s::TaskSettingsWindow_Remove(), true)], 6.0) {
            Some(2) if has => {
                let i = f.w.filter_selected.unwrap_or(0);
                sec!(f, UploadSettings).UploaderFilters.remove(i);
                f.w.filter_selected = None;
            }
            Some(_) => f.effect(Effect::NotPorted("uploader filters")),
            None => {}
        }
    });
}

fn page_tools(f: &mut Form) {
    f.override_section(Section::Tools, s::TaskSettingsWindow_OverrideToolsSettings());
    f.enabled_card(Section::Tools, &mut |f| {
        f.title(s::TaskSettingsWindow_ScreenColorPicker());
        let mut o = sec!(f, ToolsSettings).ScreenColorPickerOptions.clone();
        for (label, key) in [
            (s::TaskSettingsWindow_Format(), "Format"),
            (s::TaskSettingsWindow_FormatCtrlPlusClick(), "FormatCtrl"),
            (s::TaskSettingsWindow_InfoText(), "InfoText"),
        ] {
            let mut v = jstr(&o, key);
            if f.text_row(Some((label, LABEL_W)), &mut v, true, false) {
                jset(&mut o, key, Value::String(v));
            }
        }
        jcheck(f, s::TaskSettingsWindow_ShowMagnifier(), &mut o, "ShowMagnifier", true, true);
        sec!(f, ToolsSettings).ScreenColorPickerOptions = o;
    });
}

fn page_image_editor(f: &mut Form) {
    f.enabled_card(Section::Tools, &mut |f| {
        f.title(s::TaskSettingsWindow_General());
        let mut o = sec!(f, ToolsSettings).ImageEditorOptions.clone();
        let checks = [
            (s::TaskSettingsWindow_RememberWindowState(), "RememberWindowState", true),
            (s::TaskSettingsWindow_AutoSizeWindow(), "AutoSizeWindow", false),
            (s::TaskSettingsWindow_ShowExitConfirmation(), "ShowExitConfirmation", true),
            (s::TaskSettingsWindow_ZoomToFitOnOpen(), "ZoomToFitOnOpen", false),
            (s::TaskSettingsWindow_QuickCrop(), "QuickCrop", true),
            (s::TaskSettingsWindow_RightClickDeletesShape(), "RightClickDeletesShape", false),
            (s::TaskSettingsWindow_AutoCloseEditorOnTask(), "AutoCloseEditorOnTask", false),
            (s::TaskSettingsWindow_AutoCopyImageToClipboard(), "AutoCopyImageToClipboard", false),
            (s::TaskSettingsWindow_ShowInsertImageDialog(), "ShowInsertImageDialog", true),
            (s::TaskSettingsWindow_AutoExpandCanvas(), "AutoExpandCanvas", true),
        ];
        for (text, key, default) in checks {
            jcheck(f, text, &mut o, key, default, true);
        }
        canvas_fill_row(f, &mut o);
        jcheck(f, s::TaskSettingsWindow_ShowNotifications(), &mut o, "ShowNotifications", true, true);
        sec!(f, ToolsSettings).ImageEditorOptions = o;
        if f.button(s::TaskSettingsWindow_CustomizeToolbarWithEllipsis(), true) {
            // `ShowImageEditorToolbarEditor`.
            let saved = sec!(f, ToolsSettings).ImageEditorOptions.get("ToolbarItems").and_then(|v| v.as_array()).cloned().unwrap_or_default();
            f.w.toolbar_editor = Some(crate::toolbar_editor::ToolbarEditor::new(crate::toolbar_editor::Target::ImageEditor, &saved));
        }
    });
}

/// "Canvas fill color:" and its `ColorPickerDropdown` (`MinWidth="130"`).
fn canvas_fill_row(f: &mut Form, o: &mut Value) {
    let label = s::TaskSettingsWindow_CanvasFillColor();
    f.text(label);
    if f.collecting() {
        return;
    }
    let lh = line_height(f.ui, &Form::label_font());
    let r = f.slot(40f32.max(lh));
    let p = palette(f.ui.ctx());
    controls::label(f.ui, Rect::from_min_max(r.min, pos2(r.min.x + LABEL_W, r.max.y)), label, Form::label_font(), p.text_secondary);
    let rect = Rect::from_min_size(pos2(r.min.x + LABEL_W + 8.0, center_y(r, 40.0)), vec2(130.0, 40.0));
    let hex = jstr(o, "CanvasFillColorHex");
    let color = parse_argb(&hex).unwrap_or(egui::Color32::TRANSPARENT);
    let enabled = f.enabled;
    let resp = ColorButton { enabled }.show(f.ui, rect, f.id("canvas-fill"), color);
    let just_opened = resp.clicked() && !f.w.color_open;
    if resp.clicked() {
        f.w.color_open = !f.w.color_open;
    }
    if f.w.color_open {
        let mut c = color;
        let (changed, keep) = controls::color_popup(f.ui, f.id("canvas-fill-popup"), rect, &mut c, just_opened);
        if changed {
            jset(o, "CanvasFillColorHex", Value::String(format!("#{:02X}{:02X}{:02X}{:02X}", c.a(), c.r(), c.g(), c.b())));
        }
        f.w.color_open = keep;
    }
}

/// "#AARRGGBB".
fn parse_argb(hex: &str) -> Option<egui::Color32> {
    let h = hex.trim_start_matches('#');
    let v = u32::from_str_radix(h, 16).ok()?;
    let (a, r, g, b) = if h.len() == 8 { ((v >> 24) as u8, (v >> 16) as u8, (v >> 8) as u8, v as u8) } else { (255, (v >> 16) as u8, (v >> 8) as u8, v as u8) };
    Some(egui::Color32::from_rgba_unmultiplied(r, g, b, a))
}

/// `MouseHighlighterSettingsControl` (ShareX.Tools): a `StackPanel
/// Spacing="12"` of rows `ColumnDefinitions="*,180" ColumnSpacing="12"`,
/// Fluent's default `CheckBox`es, `ComboBox` and `NumericUpDown`s.
fn page_mouse_highlighter(f: &mut Form) {
    use sharex_strings::tools as t;
    f.enabled_card(Section::Tools, &mut |f| {
        f.title(s::MouseHighlighter());
        let mut o = sec!(f, ToolsSettings).MouseHighlighterOptions.clone();
        let ripple = jstr(&o, "Mode") != "Circle";
        f.stack(12.0, &mut |f| {
            let items = [ComboItem::text(t::MouseHighlighter_Circle()), ComboItem::text(t::MouseHighlighter_Ripple())];
            if let Some(i) = mh_combo(f, t::MouseHighlighter_Mode(), &items, usize::from(ripple)) {
                jset(&mut o, "Mode", Value::String(if i == 0 { "Circle" } else { "Ripple" }.into()));
            }
            for (label, cross, on_key, color_key, cross_key) in [
                (t::MouseHighlighter_PrimaryColor(), t::MouseHighlighter_PrimaryReleaseCrosshairs(), "HighlightPrimaryClicks", "PrimaryColor", "ShowPrimaryReleaseCrosshairs"),
                (t::MouseHighlighter_SecondaryColor(), t::MouseHighlighter_ReleaseCrosshairs(), "HighlightSecondaryClicks", "SecondaryColor", "ShowSecondaryReleaseCrosshairs"),
                (t::MouseHighlighter_MiddleColor(), t::MouseHighlighter_MiddleReleaseCrosshairs(), "HighlightMiddleClicks", "MiddleColor", "ShowMiddleReleaseCrosshairs"),
            ] {
                let mut on = jbool(&o, on_key, true);
                if mh_color_row(f, label, Some(&mut on), &jstr(&o, color_key)) {
                    jset(&mut o, on_key, Value::Bool(on));
                }
                if ripple {
                    let mut c = jbool(&o, cross_key, false);
                    if mh_check(f, cross, &mut c, on) {
                        jset(&mut o, cross_key, Value::Bool(c));
                    }
                }
            }
            if ripple {
                f.stack(10.0, &mut |f| {
                    mh_number(f, t::MouseHighlighter_RippleSize(), &mut o, "RippleSize", 10.0, 300.0, 1.0, 0);
                    mh_number(f, t::MouseHighlighter_RippleIntensity(), &mut o, "RippleIntensity", 0.15, 1.35, 0.05, 2);
                    mh_number(f, t::MouseHighlighter_RippleDuration(), &mut o, "RippleDuration", 60.0, 2000.0, 10.0, 0);
                    let mut v = jbool(&o, "FollowCursorWhileHeld", true);
                    if mh_check(f, t::MouseHighlighter_FollowCursor(), &mut v, true) {
                        jset(&mut o, "FollowCursorWhileHeld", Value::Bool(v));
                    }
                });
            } else {
                f.stack(10.0, &mut |f| {
                    mh_color_row(f, t::MouseHighlighter_AlwaysColor(), None, &jstr(&o, "AlwaysColor"));
                    let help = t::MouseHighlighter_AlwaysColorHelp();
                    f.text(help);
                    if !f.collecting() {
                        let body = font(13.0, 400);
                        let h = controls::wrapped(f.ui, pos2(0.0, 0.0), f.width, help, body.clone(), egui::Color32::GRAY, false);
                        let r = f.slot(h);
                        controls::wrapped(f.ui, r.min, f.width, help, body, egui::Color32::GRAY, true);
                    }
                    mh_number(f, t::MouseHighlighter_Radius(), &mut o, "Radius", 5.0, 500.0, 1.0, 0);
                    mh_number(f, t::MouseHighlighter_FadeDelay(), &mut o, "FadeDelay", 0.0, 10000.0, 10.0, 0);
                    mh_number(f, t::MouseHighlighter_FadeDuration(), &mut o, "FadeDuration", 0.0, 10000.0, 10.0, 0);
                });
            }
            let mut v = jbool(&o, "AutoActivate", false);
            if mh_check(f, t::MouseHighlighter_AutoActivate(), &mut v, true) {
                jset(&mut o, "AutoActivate", Value::Bool(v));
            }
        });
        sec!(f, ToolsSettings).MouseHighlighterOptions = o;
    });
}

/// A Fluent `CheckBox` (`MinHeight="32"`) with a `TextBlock` (Normal).
fn mh_check(f: &mut Form, text: &str, value: &mut bool, enabled: bool) -> bool {
    f.text(text);
    if f.collecting() {
        return false;
    }
    let r = f.slot(32.0);
    let cb = controls::CheckBox { min_height: 32.0, enabled: enabled && f.enabled, text_font: font(13.0, 400), text_color: Some(palette(f.ui.ctx()).text), ..controls::CheckBox::new(text) };
    let w = cb.width(f.ui);
    let id = f.id(text);
    cb.show(f.ui, Rect::from_min_size(r.min, vec2(w, 32.0)), id, value).changed()
}

/// The control column of a row: 180 wide at the right, 12 after the label.
fn mh_cell(r: Rect, h: f32) -> Rect {
    Rect::from_min_size(pos2(r.max.x - 180.0, center_y(r, h)), vec2(180.0, h))
}

fn mh_combo(f: &mut Form, label: &str, items: &[ComboItem], selected: usize) -> Option<usize> {
    f.text(label);
    if f.collecting() {
        return None;
    }
    let h = 40.0;
    let r = f.slot(h);
    let p = palette(f.ui.ctx());
    controls::label(f.ui, Rect::from_min_max(r.min, pos2(r.max.x - 192.0, r.max.y)), label, font(13.0, 400), p.text_secondary);
    let cell = mh_cell(r, h);
    let id = f.id(label);
    let combo = controls::ComboBox { padding: sharex_ui::theme::Thickness::new(12.0, 7.0, 11.0, 7.0), enabled: f.enabled };
    let out = combo.show(f.ui, cell, id, items, Some(selected));
    if controls::ComboBox::is_open(f.ui.ctx(), id) {
        f.ui.ctx().data_mut(|d| d.insert_temp(Id::new("settings-any-combo-next"), true));
    }
    out
}

/// "a, r, g, b" (`System.Drawing.Color` as Newtonsoft writes it), or a name.
fn parse_drawing_color(v: &str) -> egui::Color32 {
    let parts: Vec<u8> = v.split(',').filter_map(|p| p.trim().parse().ok()).collect();
    match parts.as_slice() {
        [a, r, g, b] => egui::Color32::from_rgba_unmultiplied(*r, *g, *b, *a),
        [r, g, b] => egui::Color32::from_rgb(*r, *g, *b),
        _ => egui::Color32::TRANSPARENT,
    }
}

/// `EnabledColorRow` / `ColorRow`: a check box (or label) and a button
/// holding a 28x20 swatch. The colour flyout (`ColorView`) isn't ported.
fn mh_color_row(f: &mut Form, label: &str, on: Option<&mut bool>, color: &str) -> bool {
    f.text(label);
    if f.collecting() {
        return false;
    }
    let h = 40.0;
    let r = f.slot(h);
    let p = palette(f.ui.ctx());
    let mut changed = false;
    let enabled = match on {
        Some(on) => {
            let cb = controls::CheckBox { min_height: 32.0, enabled: f.enabled, text_font: font(13.0, 400), text_color: Some(palette(f.ui.ctx()).text), ..controls::CheckBox::new(label) };
            let w = cb.width(f.ui);
            let id = f.id(label);
            changed = cb.show(f.ui, Rect::from_min_size(pos2(r.min.x, center_y(r, 32.0)), vec2(w, 32.0)), id, on).changed();
            *on
        }
        None => {
            controls::label(f.ui, Rect::from_min_max(r.min, pos2(r.max.x - 192.0, r.max.y)), label, font(13.0, 400), p.text_secondary);
            true
        }
    };
    let cell = mh_cell(r, h);
    let b = Rect::from_min_size(cell.min, vec2(1.0 + 10.0 + 28.0 + 10.0 + 1.0, h));
    let enabled = enabled && f.enabled;
    if Button::default().show(f.ui, b, f.id(&format!("{label}-color")), "", enabled).clicked() {
        f.effect(Effect::NotPorted("colour picker flyout"));
    }
    let swatch = Rect::from_center_size(b.center(), vec2(28.0, 20.0));
    let opacity = if enabled { 1.0 } else { 0.35 };
    f.ui.painter().rect_filled(swatch, 0.0, parse_drawing_color(color).gamma_multiply(opacity));
    f.ui.painter().rect_stroke(swatch, 0.0, Stroke::new(1.0, egui::Color32::GRAY.gamma_multiply(opacity)), StrokeKind::Inside);
    changed
}

/// `NumberRow`: a default `NumericUpDown`, 180 wide, `FormatString` 0 or 0.00.
#[allow(clippy::too_many_arguments)]
fn mh_number(f: &mut Form, label: &str, o: &mut Value, key: &str, min: f64, max: f64, inc: f64, decimals: usize) {
    f.text(label);
    if f.collecting() {
        return;
    }
    let h = 40.0;
    let r = f.slot(h);
    let p = palette(f.ui.ctx());
    controls::label(f.ui, Rect::from_min_max(r.min, pos2(r.max.x - 192.0, r.max.y)), label, font(13.0, 400), p.text_secondary);
    let cell = mh_cell(r, h);
    let mut v = o.get(key).and_then(Value::as_f64).unwrap_or(min);
    let id = f.id(label);
    if (controls::DecimalUpDown { minimum: min, maximum: max, increment: inc, decimals, enabled: f.enabled }).show(f.ui, cell, id, &mut v) {
        let x = if decimals == 0 { Value::from(v.round() as i64) } else { Value::from(v) };
        jset(o, key, x);
    }
}

fn page_actions(f: &mut Form) {
    f.override_section(Section::Actions, s::TaskSettingsWindow_OverrideActions());
    f.enabled_card(Section::Actions, &mut |f| {
        f.title(s::TaskSettingsWindow_Actions());
        action_list(f);
        let has = f.w.action_selected.is_some_and(|i| i < f.s.task.ExternalPrograms.as_ref().map_or(0, Vec::len));
        match f.buttons(
            &[
                (s::TaskSettingsWindow_AddWithEllipsis(), true),
                (s::TaskSettingsWindow_EditWithEllipsis(), true),
                (s::TaskSettingsWindow_Duplicate(), true),
                (s::TaskSettingsWindow_Remove(), true),
            ],
            6.0,
        ) {
            Some(0) => f.w.action_editor = Some(crate::task_editors::ActionEditor::new(None, None)),
            Some(1) if has => {
                let i = f.w.action_selected.unwrap_or(0);
                let action = f.s.task.ExternalPrograms.get_or_insert_default()[i].clone();
                f.w.action_editor = Some(crate::task_editors::ActionEditor::new(Some(i), Some(&action)));
            }
            // `action.Copy()`, added at the end and selected.
            Some(2) if has => {
                let list = f.s.task.ExternalPrograms.get_or_insert_default();
                let copy = list[f.w.action_selected.unwrap_or(0)].clone();
                list.push(copy);
                f.w.action_selected = Some(list.len() - 1);
            }
            Some(3) if has => {
                f.s.task.ExternalPrograms.get_or_insert_default().remove(f.w.action_selected.unwrap_or(0));
                f.w.action_selected = None;
            }
            _ => {}
        }
        f.hint(s::TaskSettingsWindow_YouCanEnableOrDisableActionsFromAfterCaptureTasksPerformActions());
    });
}

/// `ListBox.settings-list.action-list`: per action a `Grid.action-list-item`
/// (`MinHeight="26"`, `Margin="6,1"`, `ColumnDefinitions="Auto,Auto,*"`,
/// `ColumnSpacing="6"`) holding the `IsActive` check box without content
/// (Fluent's 32px `MinHeight`), the name and "— path" (both `TextBlock`s:
/// SemiBold 13, the path trimmed with an ellipsis); items have
/// `Padding="2,0"`. Clicking a row or its check box selects it.
fn action_list(f: &mut Form) {
    let rows: Vec<(String, String, bool)> = f
        .s
        .task
        .ExternalPrograms
        .get_or_insert_default()
        .iter()
        .map(|a| (a.Name.clone().unwrap_or_default(), format!("— {}", a.Path.as_deref().unwrap_or_default()), a.IsActive))
        .collect();
    for (name, path, _) in &rows {
        f.text(name);
        f.text(path);
    }
    if f.collecting() {
        return;
    }
    let p = palette(f.ui.ctx());
    let fl = sharex_ui::theme::fluent(f.ui.ctx());
    let text_font = font(13.0, 600);
    let lh = line_height(f.ui, &text_font);
    let row_h = 32f32.max(lh) + 2.0;
    let r = f.slot((row_h * rows.len() as f32 + 2.0).max(230.0));
    f.ui.painter().rect_filled(r, CornerRadius::same(3), p.background_main);
    f.ui.painter().rect_stroke(r, CornerRadius::same(3), Stroke::new(1.0, p.border), StrokeKind::Inside);
    let enabled = f.enabled;
    for (i, (name, path, active)) in rows.iter().enumerate() {
        let row = Rect::from_min_size(pos2(r.min.x + 1.0, r.min.y + 1.0 + row_h * i as f32), vec2(r.width() - 2.0, row_h));
        let resp = f.ui.interact(row, f.id(&format!("action-row-{i}")), egui::Sense::click());
        let sel = f.w.action_selected == Some(i);
        if sel {
            sharex_ui::menu::paint_accent_gradient(f.ui, row, CornerRadius::same(3));
        } else if resp.hovered() && enabled {
            f.ui.painter().rect_filled(row, CornerRadius::same(3), fl.list_low);
        }
        let grid = Rect::from_min_max(pos2(row.min.x + 2.0 + 6.0, row.min.y + 1.0), pos2(row.max.x - 2.0 - 6.0, row.max.y - 1.0));
        let mut on = *active;
        let cb = controls::CheckBox { min_height: 32.0, enabled, ..controls::CheckBox::new("") };
        let cb_rect = Rect::from_min_size(pos2(grid.min.x, center_y(grid, 32.0)), vec2(20.0, 32.0));
        let id = f.id(&format!("action-active-{i}"));
        let toggled = cb.show(f.ui, cb_rect, id, &mut on).on_hover_text(s::TaskSettingsWindow_EnableAction()).clicked();
        let fg = if sel { p.accent_foreground } else { p.text_secondary };
        let ty = center_y(grid, lh);
        let x = cb_rect.max.x + 6.0;
        let ng = galley(f.ui, name, text_font.clone(), fg, f32::INFINITY);
        let name_w = ng.size().x.ceil();
        f.ui.painter().galley(pos2(x, ty), ng, fg);
        let px = x + name_w + 6.0;
        let pg = galley(f.ui, path, text_font.clone(), fg, (grid.max.x - px).max(0.0));
        f.ui.painter().galley(pos2(px, ty), pg, fg);
        if toggled {
            f.s.task.ExternalPrograms.get_or_insert_default()[i].IsActive = on;
        }
        if (resp.clicked() || toggled) && enabled {
            f.w.action_selected = Some(i);
        }
    }
}

fn page_watch_folders(f: &mut Form) {
    f.card(4.0, &mut |f| {
        f.title(s::TaskSettingsWindow_WatchFolders());
        let mut on = f.s.task.WatchFolderEnabled;
        if f.check(s::TaskSettingsWindow_WatchFoldersAndUploadNewlyCreatedFiles(), &mut on, true) {
            f.s.task.WatchFolderEnabled = on;
            f.effect(Effect::WatchFoldersChanged);
        }
        let rows: Vec<String> = f.s.task.WatchFolderList.iter().map(watch_folder_title).collect();
        let sel = f.w.watch_selected;
        if let Some(i) = f.list_box("watch", &rows, sel, 230.0) {
            f.w.watch_selected = Some(i);
        }
        let has = f.w.watch_selected.is_some_and(|i| i < rows.len());
        match f.buttons(&[(s::TaskSettingsWindow_AddWithEllipsis(), true), (s::TaskSettingsWindow_EditWithEllipsis(), true), (s::TaskSettingsWindow_Remove(), true)], 6.0) {
            Some(0) => f.w.watch_editor = Some(crate::task_editors::WatchFolderEditor::new(None, None)),
            Some(1) if has => {
                let i = f.w.watch_selected.unwrap_or(0);
                let folder = f.s.task.WatchFolderList[i].clone();
                f.w.watch_editor = Some(crate::task_editors::WatchFolderEditor::new(Some(i), Some(&folder)));
            }
            Some(2) if has => {
                f.s.task.WatchFolderList.remove(f.w.watch_selected.unwrap_or(0));
                f.w.watch_selected = None;
                f.effect(Effect::WatchFoldersChanged);
            }
            _ => {}
        }
    });
}

/// `WatchFolderTitle`: `TaskSettingsWindow_WatchFolderSummary` with the
/// folder, the filter and Yes/No for subfolders.
fn watch_folder_title(w: &sharex_settings::WatchFolderSettings) -> String {
    let sub = if w.IncludeSubdirectories { s::TaskSettingsWindow_Yes() } else { s::TaskSettingsWindow_No() };
    s::TaskSettingsWindow_WatchFolderSummary()
        .replace("{0}", w.FolderPath.as_deref().unwrap_or_default())
        .replace("{1}", w.Filter.as_deref().unwrap_or_default())
        .replace("{2}", sub)
}

fn page_advanced(f: &mut Form) {
    f.override_section(Section::Advanced, s::TaskSettingsWindow_OverrideAdvancedSettings());
    // `TaskSettingsAdvanced`'s properties by `[Category]`, in declaration order.
    f.enabled_card(Section::Advanced, &mut |f| {
        f.title(s::TaskSettingsWindow_General());
        chk!(f, s::TaskSettingsWindow_ProcessImagesDuringFileUpload(), AdvancedSettings.ProcessImagesDuringFileUpload);
        chk!(f, s::TaskSettingsWindow_ProcessImagesDuringClipboardUpload(), AdvancedSettings.ProcessImagesDuringClipboardUpload);
        chk!(f, s::TaskSettingsWindow_ProcessImagesDuringExtensionUpload(), AdvancedSettings.ProcessImagesDuringExtensionUpload);
        chk!(f, s::TaskSettingsWindow_UseAfterCaptureTasksDuringFileUpload(), AdvancedSettings.UseAfterCaptureTasksDuringFileUpload);
        chk!(f, s::TaskSettingsWindow_TextTaskSaveAsFile(), AdvancedSettings.TextTaskSaveAsFile);
        chk!(f, s::TaskSettingsWindow_AutoClearClipboard(), AdvancedSettings.AutoClearClipboard);
    });
    f.enabled_card(Section::Advanced, &mut |f| {
        f.title(s::TaskSettingsWindow_Capture());
        chk!(f, s::TaskSettingsWindow_RegionCaptureDisableAnnotation(), AdvancedSettings.RegionCaptureDisableAnnotation);
    });
    f.enabled_card(Section::Advanced, &mut |f| {
        f.title(s::TaskSettingsWindow_Upload());
        list_row(f, &format!("{}:", s::TaskSettingsWindow_ImageExtensions()), true);
        list_row(f, &format!("{}:", s::TaskSettingsWindow_TextExtensions()), false);
        chk!(f, s::TaskSettingsWindow_EarlyCopyURL(), AdvancedSettings.EarlyCopyURL);
    });
    f.enabled_card(Section::Advanced, &mut |f| {
        f.title(s::TaskSettingsWindow_UploadText());
        text!(f, &format!("{}:", s::TaskSettingsWindow_TextFileExtension()), true, AdvancedSettings.TextFileExtension);
        text!(f, &format!("{}:", s::TaskSettingsWindow_TextFormat()), true, AdvancedSettings.TextFormat);
        // `TextCustom`: `AcceptsReturn`, `TextWrapping.Wrap`, `MinHeight="80"`.
        let mut v = sec!(f, AdvancedSettings).TextCustom.clone();
        if f.text_area_row((&format!("{}:", s::TaskSettingsWindow_TextCustom()), LABEL_W), &mut v, true, 80.0) {
            sec!(f, AdvancedSettings).TextCustom = v;
        }
        chk!(f, s::TaskSettingsWindow_TextCustomEncodeInput(), AdvancedSettings.TextCustomEncodeInput);
    });
    f.enabled_card(Section::Advanced, &mut |f| {
        f.title(s::TaskSettingsWindow_AfterUpload());
        chk!(f, s::TaskSettingsWindow_ResultForceHTTPS(), AdvancedSettings.ResultForceHTTPS);
        text!(f, &format!("{}:", s::TaskSettingsWindow_ClipboardContentFormat()), true, AdvancedSettings.ClipboardContentFormat);
        text!(f, &format!("{}:", s::TaskSettingsWindow_BalloonTipContentFormat()), true, AdvancedSettings.BalloonTipContentFormat);
        text!(f, &format!("{}:", s::TaskSettingsWindow_OpenURLFormat()), true, AdvancedSettings.OpenURLFormat);
        num!(f, &format!("{}:", s::TaskSettingsWindow_AutoShortenURLLength()), i32::MIN, i32::MAX, true, AdvancedSettings.AutoShortenURLLength);
        chk!(f, s::TaskSettingsWindow_AutoCloseAfterUploadForm(), AdvancedSettings.AutoCloseAfterUploadForm);
    });
    f.enabled_card(Section::Advanced, &mut |f| {
        f.title(s::TaskSettingsWindow_NamePattern());
        num!(f, &format!("{}:", s::TaskSettingsWindow_NamePatternMaxLength()), i32::MIN, i32::MAX, true, AdvancedSettings.NamePatternMaxLength);
        num!(f, &format!("{}:", s::TaskSettingsWindow_NamePatternMaxTitleLength()), i32::MIN, i32::MAX, true, AdvancedSettings.NamePatternMaxTitleLength);
    });
}

/// A `List<string>` as "a, b, c".
fn list_row(f: &mut Form, label: &str, image: bool) {
    let adv = sec!(f, AdvancedSettings);
    let list = if image { &adv.ImageExtensions } else { &adv.TextExtensions };
    let mut v = list.join(", ");
    if f.text_row(Some((label, LABEL_W)), &mut v, true, false) {
        let items: Vec<String> = v.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect();
        let adv = sec!(f, AdvancedSettings);
        if image {
            adv.ImageExtensions = items;
        } else {
            adv.TextExtensions = items;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hotkey_window_takes_the_defaults_first() {
        let mut defaults = TaskSettings::default();
        defaults.CaptureSettings.get_or_insert_default().ShowCursor = false;
        let mut task = TaskSettings { CaptureSettings: None, ..Default::default() };
        let w = TaskSettingsWindow::new(&mut task, Some(&defaults));
        assert!(!w.is_default);
        assert_eq!(w.page, 0);
        assert!(!task.CaptureSettings.as_ref().unwrap().ShowCursor);
        let w = TaskSettingsWindow::new(&mut defaults, None);
        assert_eq!(w.page, 2, "Notifications");
    }

    #[test]
    fn default_window_skips_heading_pages() {
        let mut t = TaskSettings::default();
        let mut w = TaskSettingsWindow::new(&mut t, None);
        w.select_page(10);
        assert_eq!(w.page, 11);
        w.select_page(6);
        assert_eq!(w.page, 6);
    }

    #[test]
    fn decimal_places() {
        assert_eq!(decimals(3.0), 0);
        assert_eq!(decimals(0.5), 1);
        assert_eq!(decimals(0.25), 2);
    }

    #[test]
    fn titles() {
        let mut t = TaskSettings { Job: HotkeyType::RectangleRegion, ..Default::default() };
        let w = TaskSettingsWindow::new(&mut t.clone(), Some(&TaskSettings::default()));
        assert_eq!(w.title(&t), "ShareXrs - Task settings for Capture region");
        let w = TaskSettingsWindow::new(&mut t, None);
        assert_eq!(w.title(&TaskSettings::default()), "ShareXrs - Task settings");
    }
}
