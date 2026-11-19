//! `ShareX.HistoryLib/Views/HistoryWindow` (`.axaml` and code-behind): the
//! toolbar (search with process name suggestions, Advanced, Favorites,
//! Stats, Import, Settings), the list with `HistoryColumnLayout`'s columns
//! (resizable, shown and hidden from the header's menu), the image preview
//! beside it (a splitter between), the advanced filters panel, the context
//! menu and keys, and the overlay dialogs. Items are loaded newest first and
//! filtered 220 ms after the last change (`_filterTimer`).

use std::sync::mpsc;
use std::time::{Duration, Instant};

use egui::{CornerRadius, Id, Key, Modifiers, Rect, Ui, Vec2, pos2, vec2};
use sharex_history::actions::{CopyFormat, MenuEntry as Entry};
use sharex_history::query::{self, HistoryFilter};
use sharex_history::settings::{HistoryColumn, HistoryColumnLayout, HistorySettings};
use sharex_history::{DateTime, HistoryItem};
use sharex_settings::ApplicationConfig;
use sharex_strings::history_lib as s;
use sharex_ui::border::{self, Brush};
use sharex_ui::controls::{self, Button, CheckBox, ComboBox, ComboItem, TextBox};
use sharex_ui::icons;
use sharex_ui::keys::app_icons as icon;
use sharex_ui::menu::{Flyout, FlyoutOutput, MenuEntry, MenuStyle, Placement, Toggle};
use sharex_ui::theme::{Thickness, fluent, palette};
use sharex_ui::widgets::{center_y, font, galley, line_height, text_width};

use super::common::{self, Cmd, MenuSpec, Modal, ModalOut};
use crate::dialogs::render::Options;
use crate::dialogs::{Cx, Dialog, Effect, Spec};

/// `HistoryWindow.axaml`'s `Width`, `Height`, `MinWidth`, `MinHeight`.
pub const WIDTH: f32 = 1300.0;
pub const HEIGHT: f32 = 700.0;
pub const MIN_WIDTH: f32 = 850.0;
pub const MIN_HEIGHT: f32 = 560.0;
/// `ListBoxItem` `MinHeight="34"`, the row's `Height="34"`.
const ROW_HEIGHT: f32 = 34.0;
/// `HistoryColumnsPanel.Spacing`.
const COLUMN_SPACING: f32 = 8.0;
/// `_filterTimer.Interval`.
const FILTER_DELAY: Duration = Duration::from_millis(220);

/// The header's context menu commands.
#[derive(Clone, Copy, Debug, PartialEq)]
enum ColumnCmd {
    Toggle(HistoryColumn),
    Reset,
}

/// The advanced filters (`AdvancedPanel`).
struct Advanced {
    filename: String,
    url: String,
    filter_type: bool,
    type_index: Option<usize>,
    filter_host: bool,
    host: String,
    filter_date: bool,
    from: DateTime,
    to: DateTime,
}

impl Advanced {
    fn new() -> Self {
        let today = DateTime::now().date();
        Self { filename: String::new(), url: String::new(), filter_type: false, type_index: None, filter_host: false, host: String::new(), filter_date: false, from: today, to: today }
    }
}

/// The preview's picture: the item it's of, and the decoded image (or
/// `None` once it's known there's none).
struct Preview {
    id: i64,
    image: Option<image::RgbaImage>,
    /// The box the texture was drawn for, and the texture.
    texture: Option<((u32, u32), egui::TextureHandle)>,
    /// A URL-only image being downloaded (`PreviewHttpClient`), cancelled
    /// when another item is selected (`_previewCancellation`).
    download: Option<(mpsc::Receiver<Option<image::RgbaImage>>, sharex_uploaders::http::CancellationToken)>,
}

impl Drop for Preview {
    fn drop(&mut self) {
        if let Some((_, cancel)) = &self.download {
            cancel.cancel();
        }
    }
}

pub struct HistoryWindow {
    settings: HistorySettings,
    layout: HistoryColumnLayout,
    /// Every item, newest first (`_allHistoryItems`).
    items: Vec<HistoryItem>,
    loaded: bool,
    load_error: Option<String>,
    loader: Option<mpsc::Receiver<Result<Vec<HistoryItem>, String>>>,
    types: Vec<String>,
    hosts: Vec<String>,
    process_names: Vec<String>,
    /// Indexes into `items` that pass the filter (`_filteredHistoryItems`).
    filtered: Vec<usize>,
    search: String,
    advanced_open: bool,
    advanced: Advanced,
    filter_due: Option<Instant>,
    /// Selected items' ids, in selection order; the anchor for Shift.
    selected: Vec<i64>,
    anchor: Option<usize>,
    scroll: f32,
    preview: Option<Preview>,
    menu: Option<Flyout<Cmd>>,
    columns_menu: Option<Flyout<ColumnCmd>>,
    modal: Option<Modal>,
    /// The settings dialog's toggles while it's open.
    settings_dialog: (bool, bool),
    /// A column being resized: the column and the pointer's last x.
    resizing: Option<(HistoryColumn, f32)>,
    /// `MainGrid`'s first column (the list), resized by the splitter.
    list_width: f32,
    splitter_drag: Option<f32>,
    title: String,
    effects: Vec<Effect>,
    /// `--render`: load and decode on the spot.
    sync: bool,
    /// The settings changed (saved into `ApplicationConfig` this frame).
    settings_changed: bool,
    /// A row to scroll into view (`ScrollIntoView`).
    scroll_into_view: Option<usize>,
    /// `RememberWindowState`: the saved state and the window's bounds.
    placement: super::window_state::Tracker,
    /// Dragging the selected files out of the list.
    drag: common::ListDrag,
    /// The From (0) or To (1) date picker's open flyout.
    date_flyout: Option<(usize, super::date_picker::DateFlyout)>,
    /// `--render ... --state date-flyout`: open the From picker's flyout.
    render_date_flyout: bool,
}

impl HistoryWindow {
    /// `new HistoryWindow(historyManager, settings, services)`.
    pub fn new(config: &ApplicationConfig) -> Self {
        let settings: HistorySettings = serde_json::from_value(config.HistorySettings.clone()).unwrap_or_default();
        let layout = HistoryColumnLayout::load(settings.columns.as_deref());
        let search = if settings.remember_search_text { settings.search_text.clone() } else { String::new() };
        let list_width = if settings.remember_window_state { settings.history_list_width.max(500.0) as f32 } else { 720.0 };
        Self {
            layout,
            items: Vec::new(),
            loaded: false,
            load_error: None,
            loader: None,
            types: Vec::new(),
            hosts: Vec::new(),
            process_names: Vec::new(),
            filtered: Vec::new(),
            search,
            advanced_open: false,
            advanced: Advanced::new(),
            filter_due: None,
            selected: Vec::new(),
            anchor: None,
            scroll: 0.0,
            preview: None,
            menu: None,
            columns_menu: None,
            modal: None,
            settings_dialog: (settings.remember_search_text, settings.remember_window_state),
            resizing: None,
            list_width,
            splitter_drag: None,
            title: String::new(),
            effects: Vec::new(),
            sync: false,
            settings_changed: false,
            scroll_into_view: None,
            placement: super::window_state::Tracker::new(&settings.window_state),
            drag: common::ListDrag::default(),
            date_flyout: None,
            render_date_flyout: false,
            settings,
        }
    }

    fn title_text(&self) -> String {
        let base = crate::rs_title(s::HistoryWindows_ShareX_History());
        if self.loaded { format!("{base} ({})", common::format_count(s::HistoryWindow_ItemsFormat(), self.filtered.len())) } else { base }
    }

    /// `RefreshHistoryAsync`: every item, newest first, off the UI thread.
    fn refresh(&mut self, ctx: &egui::Context) {
        if self.sync {
            self.loaded_items(crate::history::load());
            return;
        }
        let (tx, rx) = mpsc::channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(crate::history::load());
            ctx.request_repaint();
        });
        self.loader = Some(rx);
    }

    fn loaded_items(&mut self, result: Result<Vec<HistoryItem>, String>) {
        let mut items = match result {
            Ok(items) => {
                self.load_error = None;
                items
            }
            Err(e) => {
                eprintln!("history: Error occurred while reading history file: {e}");
                self.load_error = Some(e);
                Vec::new()
            }
        };
        items.reverse();
        self.types = query::distinct_types(&items);
        self.hosts = query::distinct_hosts(&items);
        self.process_names = query::process_names(&items);
        self.items = items;
        self.loaded = true;
        if self.advanced.type_index.is_some_and(|i| i >= self.types.len()) {
            self.advanced.type_index = None;
        }
        self.apply_filter();
    }

    /// `CreateCurrentFilter`.
    fn current_filter(&self) -> HistoryFilter {
        if !self.advanced_open {
            return HistoryFilter::search(&self.search, self.settings.favorites);
        }
        let a = &self.advanced;
        HistoryFilter {
            filename: a.filename.clone(),
            url: a.url.clone(),
            filter_type: a.filter_type,
            r#type: a.type_index.and_then(|i| self.types.get(i).cloned()),
            filter_host: a.filter_host,
            host: a.host.clone(),
            filter_date: a.filter_date,
            from_date: a.from,
            to_date: a.to,
            ..Default::default()
        }
    }

    /// `ApplyFilterAsync`: the matching items, the first selected.
    fn apply_filter(&mut self) {
        self.filter_due = None;
        if !self.loaded {
            return;
        }
        let filter = self.current_filter();
        let matched: std::collections::HashSet<*const HistoryItem> = filter.apply(&self.items).into_iter().map(|i| i as *const _).collect();
        self.filtered = self.items.iter().enumerate().filter(|(_, i)| matched.contains(&(*i as *const _))).map(|(k, _)| k).collect();
        self.scroll = 0.0;
        self.selected.clear();
        self.anchor = None;
        if let Some(&first) = self.filtered.first() {
            self.selected.push(self.items[first].id);
            self.anchor = Some(0);
        }
    }

    fn schedule_filter(&mut self) {
        if self.loaded {
            self.filter_due = Some(Instant::now() + FILTER_DELAY);
        }
    }

    /// Indexes into `items` of the selected items, in selection order.
    fn selected_indexes(&self) -> Vec<usize> {
        self.selected.iter().filter_map(|id| self.items.iter().position(|i| i.id == *id)).collect()
    }

    fn primary(&self) -> Option<&HistoryItem> {
        self.selected.first().and_then(|id| self.items.iter().find(|i| i.id == *id))
    }

    fn select_only(&mut self, row: usize) {
        if let Some(&i) = self.filtered.get(row) {
            self.selected = vec![self.items[i].id];
            self.anchor = Some(row);
        }
    }

    fn save_settings(&self, cx: &mut Cx) {
        let mut s = self.settings.clone();
        s.columns = Some(self.layout.to_settings());
        if let Ok(v) = serde_json::to_value(&s) {
            cx.settings.HistorySettings = v;
        }
    }

    /// `OnHistoryContextMenuOpening`'s menu for the selection.
    fn context_menu(&self) -> Vec<MenuEntry<Cmd>> {
        let idx = self.selected_indexes();
        let sel: Vec<&HistoryItem> = idx.iter().map(|&i| &self.items[i]).collect();
        let favorite = if sel.first().is_some_and(|i| i.favorite()) { s::HistoryWindow_Unfavorite() } else { s::HistoryWindows_Favorite() };
        let c = |rule, header: &str, gesture: &'static str, format| -> MenuSpec { Some((rule, header.into(), "", gesture, Cmd::Copy(format))) };
        let copy: Vec<MenuSpec> = vec![
            c(Entry::CopyUrl, s::HistoryWindows_URL(), "Ctrl+C", CopyFormat::Url),
            c(Entry::CopyShortUrl, s::HistoryWindows_Shortened_URL(), "", CopyFormat::ShortUrl),
            c(Entry::CopyThumbnailUrl, s::HistoryWindows_Thumbnail_URL(), "", CopyFormat::ThumbnailUrl),
            c(Entry::CopyDeletionUrl, s::HistoryWindows_Deletion_URL(), "", CopyFormat::DeletionUrl),
            None,
            Some((Entry::CopyFile, s::HistoryWindows_File().into(), "", "Shift+C", Cmd::CopyFile)),
            Some((Entry::CopyImage, s::HistoryWindows_Image().into(), "", "Alt+C", Cmd::CopyImage)),
            Some((Entry::CopyText, s::HistoryWindows_Text().into(), "", "", Cmd::CopyText)),
            None,
            c(Entry::CopyHtmlLink, s::HistoryWindows_HTML_link(), "", CopyFormat::HtmlLink),
            c(Entry::CopyHtmlImage, s::HistoryWindows_HTML_image(), "", CopyFormat::HtmlImage),
            c(Entry::CopyHtmlLinkedImage, s::HistoryWindows_HTML_linked_image(), "", CopyFormat::HtmlLinkedImage),
            None,
            c(Entry::CopyForumLink, s::HistoryWindows_Forum_BBCode_link(), "", CopyFormat::ForumLink),
            c(Entry::CopyForumImage, s::HistoryWindows_Forum_BBCode_image(), "", CopyFormat::ForumImage),
            c(Entry::CopyForumLinkedImage, s::HistoryWindows_Forum_BBCode_linked_image(), "", CopyFormat::ForumLinkedImage),
            None,
            c(Entry::CopyMarkdownLink, s::HistoryWindows_Markdown_link(), "", CopyFormat::MarkdownLink),
            c(Entry::CopyMarkdownImage, s::HistoryWindows_Markdown_image(), "", CopyFormat::MarkdownImage),
            c(Entry::CopyMarkdownLinkedImage, s::HistoryWindows_Markdown_linked_image(), "", CopyFormat::MarkdownLinkedImage),
            None,
            c(Entry::CopyFilePath, s::HistoryWindows_File_path(), "Ctrl+Shift+C", CopyFormat::FilePath),
            c(Entry::CopyFileName, s::HistoryWindows_File_name(), "", CopyFormat::FileName),
            c(Entry::CopyFileNameExtension, s::HistoryWindows_File_name_with_extension(), "", CopyFormat::FileNameWithExtension),
            c(Entry::CopyFolder, s::HistoryWindows_Folder(), "", CopyFormat::Folder),
        ];
        let mut tail = vec![Some((Entry::Preview, s::HistoryWindows_Show_image_preview().into(), icon::Eye, "", Cmd::Preview))];
        tail.extend(common::service_entries());
        let spec = vec![
            common::open_menu(),
            (Some((s::HistoryWindows_Copy().into(), icon::Copy)), copy),
            common::separator(),
            (None, common::item_entries(favorite.into())),
            common::separator(),
            (None, tail),
        ];
        common::build_menu(&spec, &sel)
    }

    /// `OnColumnsMenuOpening`: a check item a configurable column, then
    /// Reset columns.
    fn columns_menu(&self) -> Vec<MenuEntry<ColumnCmd>> {
        let mut out: Vec<MenuEntry<ColumnCmd>> = HistoryColumn::CONFIGURABLE
            .iter()
            .map(|&c| {
                let visible = self.layout.is_visible(c);
                MenuEntry::item(column_title(c), "", ColumnCmd::Toggle(c))
                    .toggle(Toggle::CheckBox, visible)
                    .enabled(!visible || self.layout.visible_count() > 1)
            })
            .collect();
        out.push(MenuEntry::separator());
        out.push(MenuEntry::item(s::HistoryWindow_ResetColumns(), "", ColumnCmd::Reset));
        out
    }

    fn run(&mut self, cmd: Cmd) {
        let idx = self.selected_indexes();
        let out = common::run_cmd(cmd, &mut self.items, &idx, &common::HISTORY_PROMPTS);
        self.effects.extend(out.effects);
        if let Some(m) = out.modal {
            self.modal = Some(m);
        }
        if out.edited {
            // `OnToggleFavoriteClick`: with Favorites on, the list is filtered again.
            if cmd == Cmd::Favorite && self.settings.favorites {
                self.apply_filter();
            }
            self.preview = None;
        }
    }

    /// `TryOpen`: the shortened URL, else the URL, else the file.
    fn try_open(&mut self) {
        let Some(item) = self.primary() else { return };
        match sharex_history::actions::try_open(item, common::file_exists) {
            Some(sharex_history::actions::OpenTarget::Url(u)) => self.effects.push(Effect::OpenUrl(u)),
            Some(sharex_history::actions::OpenTarget::File(f)) => self.effects.push(Effect::OpenFile(f.into())),
            None => {}
        }
    }

    /// `OnHistoryListKeyDown` and `OnWindowKeyDown`.
    fn keys(&mut self, ui: &Ui) {
        let pressed = |k: Key, m: Modifiers| ui.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Key { key, pressed: true, modifiers, .. } if *key == k && *modifiers == m)));
        let none = Modifiers::NONE;
        let ctrl = Modifiers::COMMAND;
        let shift = Modifiers::SHIFT;
        let alt = Modifiers::ALT;
        if pressed(Key::F5, none) {
            self.loaded = false;
            self.refresh(ui.ctx());
            return;
        }
        if pressed(Key::Escape, none) {
            if self.modal.is_some() {
                self.close_modal();
            } else if self.menu.is_none() && self.columns_menu.is_none() {
                self.effects.push(Effect::Close);
            }
            return;
        }
        // The list has the keyboard unless a text box does.
        if self.modal.is_some() || self.menu.is_some() || common::text_box_focused(ui.ctx()) {
            return;
        }
        // egui-winit turns Ctrl+C (with or without Shift) into `Event::Copy`.
        let copy = ui.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Copy)).then_some(i.modifiers.shift));
        if let Some(shift) = copy {
            self.run(Cmd::Copy(if shift { CopyFormat::FilePath } else { CopyFormat::Url }));
        } else if pressed(Key::Enter, ctrl) {
            self.run(Cmd::OpenFile);
        } else if pressed(Key::Enter, shift) {
            self.run(Cmd::OpenFolder);
        } else if pressed(Key::Enter, none) {
            self.try_open();
        } else if pressed(Key::C, ctrl) {
            self.run(Cmd::Copy(CopyFormat::Url));
        } else if pressed(Key::C, shift) {
            self.run(Cmd::CopyFile);
        } else if pressed(Key::C, alt) {
            self.run(Cmd::CopyImage);
        } else if pressed(Key::C, ctrl | shift) {
            self.run(Cmd::Copy(CopyFormat::FilePath));
        } else if pressed(Key::Delete, shift) || ui.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Cut)) && i.modifiers.shift) {
            self.run(Cmd::DeleteFile);
        } else if pressed(Key::Delete, none) {
            self.run(Cmd::Delete);
        } else if pressed(Key::U, ctrl) {
            self.run(Cmd::Service(crate::dialogs::FileService::UploadFile));
        } else if pressed(Key::E, ctrl) {
            self.run(Cmd::Service(crate::dialogs::FileService::EditImage));
        } else if pressed(Key::P, ctrl) {
            self.run(Cmd::Service(crate::dialogs::FileService::PinToScreen));
        } else if pressed(Key::ArrowDown, none) || pressed(Key::ArrowUp, none) {
            let row = self.anchor.unwrap_or(0) as i64 + if pressed(Key::ArrowDown, none) { 1 } else { -1 };
            if row >= 0 && (row as usize) < self.filtered.len() {
                self.select_only(row as usize);
                self.scroll_into_view = Some(row as usize);
            }
        }
    }

    /// `CloseModal`.
    fn close_modal(&mut self) {
        self.modal = None;
    }

    /// Sets the window up for a `--render` state.
    fn render_state(&mut self, ctx: &egui::Context, o: &Options) {
        self.sync = true;
        self.refresh(ctx);
        if !o.select.is_empty() {
            self.selected = o.select.iter().filter_map(|&r| self.filtered.get(r)).map(|&i| self.items[i].id).collect();
            self.anchor = o.select.first().copied();
        }
        match o.state.as_deref() {
            Some("advanced") => {
                self.advanced_open = true;
                self.apply_filter();
            }
            Some("date-flyout") => {
                self.advanced_open = true;
                self.render_date_flyout = true;
                self.apply_filter();
            }
            Some("favorites") => {
                self.settings.favorites = true;
                self.apply_filter();
            }
            Some("search") => {
                self.search = o.text.clone().unwrap_or_default();
                self.apply_filter();
            }
            Some("stats") => self.modal = Some(Modal::Stats(sharex_history::stats::output_stats(&self.items))),
            Some("import") => self.modal = Some(Modal::Import { folder: String::new(), images_only: false, skip_duplicates: true, status: String::new() }),
            Some("settings") => {
                self.settings_dialog = (self.settings.remember_search_text, self.settings.remember_window_state);
                self.modal = Some(Modal::Settings);
            }
            Some("edit") => {
                if let Some(item) = self.primary() {
                    self.modal = Some(Modal::edit(item));
                }
            }
            Some("delete") => self.run(Cmd::Delete),
            Some("menu") => {
                let pos = o.context.unwrap_or(egui::pos2(112.0, 120.0));
                let mut f = Flyout::menu(Id::new("history-menu"), self.context_menu(), Rect::from_min_size(pos, Vec2::ZERO), Placement::Pointer(pos));
                f.style = MenuStyle::Standard;
                f.open_path(&o.open);
                self.menu = Some(f);
            }
            Some("columns") => {
                let pos = o.context.unwrap_or(egui::pos2(112.0, 85.0));
                let mut f = Flyout::menu(Id::new("history-columns"), self.columns_menu(), Rect::from_min_size(pos, Vec2::ZERO), Placement::Pointer(pos));
                f.style = MenuStyle::Standard;
                self.columns_menu = Some(f);
            }
            _ => {}
        }
    }
}

/// `GetColumnTitle`.
fn column_title(c: HistoryColumn) -> &'static str {
    match c {
        HistoryColumn::Icon => "",
        HistoryColumn::Date => s::HistoryWindows_Date(),
        HistoryColumn::FileName => s::HistoryWindows_File_name(),
        HistoryColumn::FilePath => s::HistoryWindows_File_path(),
        HistoryColumn::Type => s::HistoryWindows_Type(),
        HistoryColumn::Host => s::HistoryWindows_Host(),
        HistoryColumn::URL => s::HistoryWindows_URL(),
        HistoryColumn::ShortenedURL => s::HistoryWindows_Shortened_URL(),
        HistoryColumn::ProcessName => s::HistoryWindow_ProcessName(),
        HistoryColumn::WindowTitle => s::HistoryWindow_WindowTitle(),
        HistoryColumn::Tag => s::HistoryWindow_Tag(),
    }
}

/// `HistoryItemIconConverter`'s icon as an `AppIcons` key.
fn list_icon(item: &HistoryItem) -> &'static str {
    match item.icon_key() {
        "Star" => icon::Star,
        "Image" => icon::Image,
        "FileText" => icon::FileText,
        "File" => icon::File,
        _ => icon::Link,
    }
}

/// A cell's text and whether it's `.secondary`.
fn cell_text(item: &HistoryItem, c: HistoryColumn) -> (String, bool) {
    let v = |o: &Option<String>| o.clone().unwrap_or_default();
    match c {
        HistoryColumn::Icon => (String::new(), false),
        HistoryColumn::Date => (common::format_g(item), false),
        HistoryColumn::FileName => (v(&item.file_name), false),
        HistoryColumn::FilePath => (v(&item.file_path), true),
        HistoryColumn::Type => (v(&item.r#type), false),
        HistoryColumn::Host => (v(&item.host), false),
        HistoryColumn::URL => (v(&item.url), true),
        HistoryColumn::ShortenedURL => (v(&item.shortened_url), true),
        HistoryColumn::ProcessName => (item.tags_process_name().unwrap_or("").to_string(), false),
        HistoryColumn::WindowTitle => (item.tags_window_title().unwrap_or("").to_string(), false),
        HistoryColumn::Tag => (item.tag_text().unwrap_or("").to_string(), false),
    }
}

/// `HistoryColumnsPanel`'s cells in `width`: each visible column's x offset
/// and width (the fill column takes what's left, 40 at least).
fn column_cells(layout: &HistoryColumnLayout, width: f32) -> Vec<(HistoryColumn, f32, f32)> {
    let visible: Vec<HistoryColumn> = [HistoryColumn::Icon].into_iter().chain(HistoryColumn::CONFIGURABLE).filter(|&c| c == HistoryColumn::Icon || layout.is_visible(c)).collect();
    let fill = layout.fill_column();
    let used: f32 = visible.iter().filter(|&&c| Some(c) != fill).map(|&c| layout.width(c) as f32).sum::<f32>() + COLUMN_SPACING * (visible.len().saturating_sub(1)) as f32;
    let fill_w = (width - used).max(HistoryColumnLayout::MIN_COLUMN_WIDTH as f32);
    let mut x = 0.0;
    visible
        .into_iter()
        .map(|c| {
            let w = if Some(c) == fill { fill_w } else { layout.width(c) as f32 };
            let cell = (c, x, w);
            x += w + COLUMN_SPACING;
            cell
        })
        .collect()
}

impl HistoryWindow {
    /// The toolbar: search, Advanced, Favorites, Stats, Import, Settings.
    fn toolbar(&mut self, ui: &mut Ui, rect: Rect, live: bool) {
        common::docked_toolbar(ui, rect);
        let y = rect.min.y + 8.0;
        let mut x = rect.min.x + 1.0 + 8.0;
        let search_rect = Rect::from_min_size(pos2(x, y), vec2(250.0, 40.0));
        if live && common::search_box(ui, search_rect, Id::new("history-search"), &mut self.search) {
            if self.settings.remember_search_text {
                self.settings.search_text = self.search.clone();
            }
            self.schedule_filter();
        } else if !live {
            let mut copy = self.search.clone();
            common::search_box(ui, search_rect, Id::new("history-search"), &mut copy);
        }
        x += 250.0 + common::TOOLBAR_SPACING;
        let buttons = [
            (icon::Filter, s::HistoryWindows_Advanced(), self.advanced_open),
            (icon::Star, s::HistoryWindows_Favorites(), self.settings.favorites),
            (icon::BarChart, s::HistoryWindows_Stats(), false),
            (icon::FolderSearch, s::HistoryWindows_Import(), false),
            (icon::Settings, s::HistoryWindows_Settings(), false),
        ];
        let mut clicked = None;
        for (i, (key, text, active)) in buttons.into_iter().enumerate() {
            let w = common::icon_button_width(ui, text);
            let r = Rect::from_min_size(pos2(x, y), vec2(w, 40.0));
            if common::icon_button(ui, r, Id::new(("history-tool", i)), key, text, active, true).clicked() && live {
                clicked = Some(i);
            }
            x += w + common::TOOLBAR_SPACING;
        }
        match clicked {
            Some(0) => {
                self.advanced_open = !self.advanced_open;
                self.apply_filter();
            }
            Some(1) => {
                self.settings.favorites = !self.settings.favorites;
                self.apply_filter();
            }
            Some(2) => self.modal = Some(Modal::Stats(sharex_history::stats::output_stats(&self.items))),
            Some(3) => self.modal = Some(Modal::Import { folder: String::new(), images_only: false, skip_duplicates: true, status: String::new() }),
            Some(4) => {
                self.settings_dialog = (self.settings.remember_search_text, self.settings.remember_window_state);
                self.modal = Some(Modal::Settings);
            }
            _ => {}
        }
    }

    /// The list panel: the column header, then the rows.
    fn list(&mut self, ui: &mut Ui, panel: Rect, live: bool) {
        let p = palette(ui.ctx());
        common::tool_panel(ui, panel);
        let inner = panel.shrink(1.0);
        let header_font = font(13.0, 600);
        let hl = line_height(ui, &header_font);
        let header = Rect::from_min_size(inner.min, vec2(inner.width(), 8.0 + hl + 8.0 + 1.0));
        // The header `Border`, clipped by the panel's rounded corners.
        let radius = CornerRadius { nw: 3, ne: 3, ..Default::default() };
        ui.painter().rect_filled(header, radius, p.control_background);
        ui.painter().rect_filled(Rect::from_min_max(pos2(header.min.x, header.max.y - 1.0), header.max), 0.0, p.border);
        let cells_w = header.width() - 20.0;
        let cells = column_cells(&self.layout, cells_w);
        let fill = self.layout.fill_column();
        let hx = header.min.x + 10.0;
        let hy = header.min.y + 8.0;
        let header_clip = Rect::from_min_max(pos2(hx, hy - 8.0), pos2(hx + cells_w, header.max.y));
        for &(c, x, w) in &cells {
            if c == HistoryColumn::Icon {
                continue;
            }
            let cell = Rect::from_min_size(pos2(hx + x, hy), vec2(w, hl));
            let g = galley(ui, column_title(c), header_font.clone(), p.text_secondary, f32::INFINITY);
            ui.painter().with_clip_rect(cell.intersect(header_clip)).galley(pos2(cell.min.x, center_y(cell, hl)), g, p.text_secondary);
            if Some(c) != fill {
                // `Thumb.column-resizer`: 8 wide past the cell's right edge,
                // a 1px separator in its middle, 5 in from its ends.
                let thumb = Rect::from_min_max(pos2(cell.max.x, cell.min.y - 8.0), pos2(cell.max.x + 8.0, cell.max.y + 8.0));
                let line = Rect::from_min_size(pos2(thumb.center().x - 0.5, thumb.min.y + 5.0), vec2(1.0, thumb.height() - 10.0));
                ui.painter().rect_filled(line, 0.0, p.separator);
                if live {
                    let r = ui.interact(thumb, Id::new(("history-resizer", c.name())), egui::Sense::drag());
                    if r.hovered() || r.dragged() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                    }
                    if r.drag_started() {
                        self.resizing = Some((c, r.interact_pointer_pos().map(|p| p.x).unwrap_or(0.0)));
                    }
                    if r.dragged()
                        && let (Some((col, last)), Some(pos)) = (self.resizing, r.interact_pointer_pos())
                    {
                        self.layout.resize(col, (pos.x - last) as f64);
                        self.resizing = Some((col, pos.x));
                    }
                    if r.drag_stopped() {
                        self.resizing = None;
                        self.settings_changed = true;
                    }
                }
            }
        }
        if live {
            let hr = ui.interact(header, Id::new("history-header"), egui::Sense::click());
            if hr.secondary_clicked() {
                let pos = hr.interact_pointer_pos().unwrap_or(header.min);
                let mut f = Flyout::menu(Id::new("history-columns"), self.columns_menu(), Rect::from_min_size(pos, Vec2::ZERO), Placement::Pointer(pos));
                f.style = MenuStyle::Standard;
                self.columns_menu = Some(f);
            }
        }

        let list = Rect::from_min_max(pos2(inner.min.x, header.max.y), inner.max);
        let content_h = ROW_HEIGHT * self.filtered.len() as f32;
        let max_scroll = (content_h - list.height()).max(0.0);
        if live && ui.rect_contains_pointer(list) {
            self.scroll -= ui.input(|i| i.smooth_scroll_delta.y);
        }
        if let Some(row) = self.scroll_into_view.take() {
            let top = row as f32 * ROW_HEIGHT;
            if top < self.scroll {
                self.scroll = top;
            } else if top + ROW_HEIGHT > self.scroll + list.height() {
                self.scroll = top + ROW_HEIGHT - list.height();
            }
        }
        self.scroll = self.scroll.clamp(0.0, max_scroll);
        let painter = ui.painter().with_clip_rect(list.intersect(ui.clip_rect()));
        let cell_font = common::normal_font();
        let cl = line_height(ui, &cell_font);
        let f = fluent(ui.ctx());
        let first = (self.scroll / ROW_HEIGHT).floor() as usize;
        let mut clicked: Option<(usize, Modifiers, bool)> = None;
        let mut double = false;
        let mut right: Option<(usize, egui::Pos2)> = None;
        let mut pressed: Option<(usize, egui::Pos2)> = None;
        for row in first..self.filtered.len() {
            let y = list.min.y + row as f32 * ROW_HEIGHT - self.scroll;
            if y > list.max.y {
                break;
            }
            let rr = Rect::from_min_size(pos2(list.min.x, y), vec2(list.width(), ROW_HEIGHT));
            let item = &self.items[self.filtered[row]];
            let selected = self.selected.contains(&item.id);
            let resp = if live { Some(ui.interact(rr.intersect(list), Id::new(("history-row", item.id)), egui::Sense::click())) } else { None };
            if selected {
                border::border(&painter, rr, 0.0, 0.0, Some(Brush::accent(ui.ctx())), None);
                // `ShareX.BoxShadow.Accent.Outline`: a 1px inset outline.
                painter.rect_stroke(rr, 0.0, egui::Stroke::new(1.0, p.accent_border), egui::StrokeKind::Inside);
            } else if resp.as_ref().is_some_and(|r| r.hovered()) && self.menu.is_none() {
                painter.rect_filled(rr, 0.0, f.list_low);
            }
            let fg = |secondary: bool| if selected { p.accent_foreground } else if secondary { p.text_secondary } else { p.text };
            let rx = rr.min.x + 10.0;
            for &(c, x, w) in &cells {
                let cell = Rect::from_min_size(pos2(rx + x, y), vec2(w, ROW_HEIGHT));
                if c == HistoryColumn::Icon {
                    let ir = Rect::from_min_size(pos2(cell.min.x + 1.0, center_y(cell, 16.0)), Vec2::splat(16.0));
                    icons::paint(&painter, ir, list_icon(item), fg(true));
                    continue;
                }
                let (text, secondary) = cell_text(item, c);
                if text.is_empty() {
                    continue;
                }
                let color = fg(secondary);
                let g = galley(ui, &text, cell_font.clone(), color, w);
                painter.with_clip_rect(cell.intersect(list)).galley(pos2(cell.min.x, center_y(cell, cl)), g, color);
            }
            if let Some(r) = resp {
                // `OnHistoryPointerPressed`: a left press on an item arms the drag.
                if r.is_pointer_button_down_on() && ui.input(|i| i.pointer.primary_pressed()) {
                    pressed = ui.input(|i| i.pointer.press_origin()).map(|p| (row, p));
                }
                if r.clicked() && !self.drag.started() {
                    clicked = Some((row, ui.input(|i| i.modifiers), true));
                }
                if r.double_clicked() {
                    double = true;
                }
                if r.secondary_clicked() {
                    right = Some((row, r.interact_pointer_pos().unwrap_or(rr.center())));
                }
            }
        }
        // Fluent's collapsed scroll bar, its track 16px short of each end.
        crate::dialogs::layout::scroll_bar(ui, list, content_h, self.scroll, 16.0);
        if let Some((row, pos)) = pressed {
            self.drag.press(pos);
            // The `ListBox` selects on the press.
            let id = self.items[self.filtered[row]].id;
            if !self.selected.contains(&id) && !ui.input(|i| i.modifiers.command || i.modifiers.shift) {
                self.select_only(row);
            }
        }
        if live {
            let (pos, down, released) = ui.input(|i| (i.pointer.latest_pos(), i.pointer.primary_down(), i.pointer.primary_released()));
            if self.drag.moved(pos, down) {
                // `OnHistoryPointerMoved`: the selected items' existing files.
                let files = common::drag_files(self.selected_indexes().into_iter().map(|i| &self.items[i]));
                self.drag.start(!files.is_empty());
                if !files.is_empty() {
                    self.effects.push(Effect::DragFiles(files));
                }
            }
            if released && self.drag.released() {
                clicked = None;
            }
        }
        if let Some((row, m, _)) = clicked {
            let id = self.items[self.filtered[row]].id;
            if m.command {
                if let Some(k) = self.selected.iter().position(|s| *s == id) {
                    self.selected.remove(k);
                } else {
                    self.selected.push(id);
                }
                self.anchor = Some(row);
            } else if m.shift && self.anchor.is_some() {
                let a = self.anchor.unwrap();
                let (lo, hi) = (a.min(row), a.max(row));
                self.selected = (lo..=hi).map(|r| self.items[self.filtered[r]].id).collect();
            } else {
                self.select_only(row);
            }
        }
        if double {
            self.try_open();
        }
        if let Some((row, pos)) = right {
            let id = self.items[self.filtered[row]].id;
            if !self.selected.contains(&id) {
                self.select_only(row);
            }
            let entries = self.context_menu();
            if !entries.is_empty() {
                let mut f = Flyout::menu(Id::new("history-menu"), entries, Rect::from_min_size(pos, Vec2::ZERO), Placement::Pointer(pos));
                f.style = MenuStyle::Standard;
                self.menu = Some(f);
            }
        }
        // The loading and empty states.
        if !self.loaded || self.filtered.is_empty() {
            let (key, text, color) = if !self.loaded {
                (icon::Loading, s::HistoryWindows_Loading(), p.accent_icon)
            } else if self.items.is_empty() {
                (icon::History, s::HistoryWindows_No_history_items(), p.text_secondary)
            } else {
                (icon::History, s::HistoryWindow_NoItemsMatchCurrentFilters(), p.text_secondary)
            };
            empty_state(ui, list, key, 44.0, text, color);
        }
    }

    /// The advanced filters panel (`AdvancedPanel`, `Padding="12"`, a line
    /// under it). Returns its height.
    fn advanced_panel(&mut self, ui: &mut Ui, area: Rect, live: bool) -> f32 {
        let p = palette(ui.ctx());
        let heading = common::heading_font();
        let inner_x = area.min.x + 12.0;
        let inner_r = area.max.x - 12.0;
        let w = (inner_r - inner_x).max(0.0);
        let mut y = area.min.y + 12.0;
        // Row 0: the heading and the 32x32 reset button.
        let row0 = Rect::from_min_size(pos2(inner_x, y), vec2(w, 32.0));
        let hl = line_height(ui, &heading);
        ui.painter().galley(pos2(inner_x, center_y(row0, hl)), galley(ui, s::HistoryWindows_Advanced_filters(), heading, p.text, f32::INFINITY), p.text);
        let reset = Rect::from_min_size(pos2(inner_r - 32.0, y), Vec2::splat(32.0));
        if common::small_icon_button(ui, reset, Id::new("history-adv-reset"), icon::Reset).clicked() && live {
            self.advanced = Advanced::new();
            self.apply_filter();
        }
        y += 32.0 + 8.0;
        let mut changed = false;
        for (k, (text, placeholder)) in [(&mut self.advanced.filename, s::HistoryWindows_File_name()), (&mut self.advanced.url, s::HistoryWindows_URL())].into_iter().enumerate() {
            let r = Rect::from_min_size(pos2(inner_x, y), vec2(w, 40.0));
            let before = text.clone();
            TextBox { placeholder, ..TextBox::default() }.show(ui, r, Id::new(("history-adv-text", k)), text);
            changed |= *text != before;
            y += 40.0 + 8.0;
        }
        // Type: the check box and the combo box of the types.
        let combo = ComboBox { padding: Thickness::hv(8.0, 6.0), enabled: true };
        let cb = CheckBox::new(s::HistoryWindows_Type());
        let cbw = cb.width(ui);
        let before = self.advanced.filter_type;
        cb.show(ui, Rect::from_min_size(pos2(inner_x, y + 4.0), vec2(cbw, 32.0)), Id::new("history-adv-type"), &mut self.advanced.filter_type);
        changed |= before != self.advanced.filter_type;
        let items: Vec<ComboItem> = self.types.iter().map(ComboItem::text).collect();
        // No selection: sized to the placeholder.
        let cw = combo_width(ui, &combo, &[], s::HistoryWindows_All_types());
        let cr = Rect::from_min_size(pos2(inner_x + cbw + 8.0, y), vec2(cw, 40.0));
        if let Some(i) = combo.show(ui, cr, Id::new("history-adv-type-combo"), &items, self.advanced.type_index) {
            self.advanced.type_index = Some(i);
            changed = true;
        }
        if self.advanced.type_index.is_none() {
            combo_placeholder(ui, cr, &combo, s::HistoryWindows_All_types());
        }
        y += 40.0 + 8.0;
        // Host: the check box and the editable combo box of the hosts.
        let cb = CheckBox::new(s::HistoryWindows_Host());
        let cbw = cb.width(ui);
        let before = self.advanced.filter_host;
        cb.show(ui, Rect::from_min_size(pos2(inner_x, y + 4.0), vec2(cbw, 32.0)), Id::new("history-adv-host"), &mut self.advanced.filter_host);
        changed |= before != self.advanced.filter_host;
        let items: Vec<ComboItem> = self.hosts.iter().map(ComboItem::text).collect();
        // `IsEditable`: sized to its text box (the placeholder), not the items.
        let cw = combo_width(ui, &combo, &[], s::HistoryWindows_All_hosts());
        let cr = Rect::from_min_size(pos2(inner_x + cbw + 8.0, y), vec2(cw, 40.0));
        let current = self.hosts.iter().position(|h| *h == self.advanced.host);
        if let Some(i) = combo.show(ui, cr, Id::new("history-adv-host-combo"), &items, current) {
            self.advanced.host = self.hosts[i].clone();
            changed = true;
        }
        if current.is_none() {
            combo_placeholder(ui, cr, &combo, s::HistoryWindows_All_hosts());
        }
        y += 40.0 + 8.0;
        let cb = CheckBox::new(s::HistoryWindows_Date_range());
        let before = self.advanced.filter_date;
        cb.show(ui, Rect::from_min_size(pos2(inner_x, y), vec2(cb.width(ui), 32.0)), Id::new("history-adv-date"), &mut self.advanced.filter_date);
        changed |= before != self.advanced.filter_date;
        y += 32.0 + 8.0;
        for (k, date) in [self.advanced.from, self.advanced.to].into_iter().enumerate() {
            let r = Rect::from_min_size(pos2(inner_x, y), vec2(296.0, 40.0));
            date_picker(ui, r, date);
            // `OnFlyoutButtonClicked`: the flyout on this date.
            let opened = live && ui.interact(r, Id::new(("history-adv-date-picker", k)), egui::Sense::click()).is_pointer_button_down_on() && ui.input(|i| i.pointer.primary_pressed());
            if opened || (k == 0 && std::mem::take(&mut self.render_date_flyout)) {
                let flyout = super::date_picker::DateFlyout::new(Id::new(("history-date-flyout", k)), r, date.naive().date());
                self.date_flyout = Some((k, flyout));
            }
            y += 40.0 + 8.0;
        }
        y -= 8.0;
        let h = y + 12.0 - area.min.y + 1.0;
        ui.painter().rect_filled(Rect::from_min_size(pos2(area.min.x, area.min.y + h - 1.0), vec2(area.width(), 1.0)), 0.0, p.border);
        if changed && live {
            self.schedule_filter();
        }
        h
    }

    /// The preview column: the selected item's image on the checkered
    /// border, "No image preview", or "Select an item to preview".
    fn preview_area(&mut self, ui: &mut Ui, area: Rect, live: bool) {
        let p = palette(ui.ctx());
        let Some(item) = self.primary().cloned() else {
            empty_state(ui, area, icon::PointerClick, 40.0, s::HistoryWindows_Select_an_item_to_preview(), p.text_secondary);
            return;
        };
        border::border(ui.painter(), area, 4.0, 1.0, Some(Brush::Checkered { colors: p.checkered, cell: 15.0 }), Some(p.border));
        if self.preview.as_ref().is_none_or(|pv| pv.id != item.id) {
            let image = load_preview(&item);
            // Else the URL, when it looks like an image (not in `--render`:
            // no network there).
            let download = (image.is_none() && !self.sync).then(|| preview_url(&item)).flatten().map(|url| {
                let (tx, rx) = mpsc::channel();
                let cancel = sharex_uploaders::http::CancellationToken::new();
                let token = cancel.clone();
                let ctx = ui.ctx().clone();
                std::thread::spawn(move || {
                    let _ = tx.send(download_preview(&url, token));
                    ctx.request_repaint();
                });
                (rx, cancel)
            });
            self.preview = Some(Preview { id: item.id, image, texture: None, download });
        }
        let inner = area.shrink(1.0);
        let pv = self.preview.as_mut().unwrap();
        if let Some((rx, _)) = &pv.download
            && let Ok(image) = rx.try_recv()
        {
            pv.image = image;
            pv.download = None;
        }
        match &pv.image {
            Some(img) => {
                let size = (inner.width().max(1.0) as u32, inner.height().max(1.0) as u32);
                if pv.texture.as_ref().is_none_or(|(s, _)| *s != size) {
                    let fitted = crate::thumbnail::fit(img, size.0, size.1, crate::thumbnail::Stretch::UniformDownOnly, 3.0);
                    let tex = ui.ctx().load_texture(format!("history-preview-{}", item.id), crate::thumbnail::to_color_image(&fitted), egui::TextureOptions::NEAREST);
                    pv.texture = Some((size, tex));
                }
                if let Some((_, tex)) = &pv.texture {
                    let r = Rect::from_min_size(inner.min, vec2(size.0 as f32, size.1 as f32));
                    ui.painter().image(tex.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), egui::Color32::WHITE);
                }
            }
            None => empty_state(ui, inner, icon::File, 38.0, s::HistoryWindows_No_image_preview(), p.text_secondary),
        }
        if live {
            let r = ui.interact(area, Id::new("history-preview"), egui::Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
            if r.clicked() {
                self.run(Cmd::Preview);
            }
        }
    }

    /// The Settings dialog (`SettingsDialog`, 440 wide).
    fn settings_dialog_ui(&mut self, ui: &mut Ui, full: Rect) -> bool {
        let layer = egui::LayerId::new(egui::Order::Foreground, Id::new("history-settings-modal"));
        let ui = ui.new_child(egui::UiBuilder::new().layer_id(layer).max_rect(full));
        let _ = ui.interact(full, Id::new("history-settings-block"), egui::Sense::click());
        let p = palette(ui.ctx());
        let heading = common::heading_font();
        let hl = line_height(&ui, &heading);
        let toggle_h = common::toggle_height(&ui);
        let h = 38.0 + hl + 14.0 + toggle_h + 14.0 + toggle_h + 14.0 + common::BUTTON_HEIGHT;
        let c = common::modal_card(&ui, full, 440.0, h);
        ui.painter().galley(c.min, galley(&ui, s::HistoryWindows_History_settings(), heading, p.text, f32::INFINITY), p.text);
        let mut y = c.min.y + hl + 14.0;
        common::toggle_with_header(&ui, pos2(c.min.x, y), Id::new("history-set-search"), s::HistoryWindows_Remember_search_text(), &mut self.settings_dialog.0);
        y += toggle_h + 14.0;
        common::toggle_with_header(&ui, pos2(c.min.x, y), Id::new("history-set-window"), s::HistoryWindows_Remember_window_size_and_position(), &mut self.settings_dialog.1);
        let b = Button::default();
        let done = s::HistoryWindows_Done();
        let bw = b.width(&ui, done);
        let br = Rect::from_min_size(pos2(c.max.x - bw, c.max.y - common::BUTTON_HEIGHT), vec2(bw, common::BUTTON_HEIGHT));
        b.show(&ui, br, Id::new("history-set-done"), done, true).clicked()
    }
}

/// A `StackPanel` centred in `area` (`Spacing="8"`): an icon of `size` and
/// a Normal `TextBlock`.
fn empty_state(ui: &Ui, area: Rect, key: &str, size: f32, text: &str, icon_color: egui::Color32) {
    let p = palette(ui.ctx());
    let f = common::normal_font();
    let lh = line_height(ui, &f);
    let tw = text_width(ui, text, f.clone());
    let w = tw.max(size);
    let h = size + 8.0 + lh;
    let left = area.min.x + ((area.width() - w) / 2.0).round_ties_even();
    let top = area.min.y + ((area.height() - h) / 2.0).round_ties_even();
    let ix = left + ((w - size) / 2.0).round_ties_even();
    icons::paint(ui.painter(), Rect::from_min_size(pos2(ix, top), Vec2::splat(size)), key, icon_color);
    let tx = left + ((w - tw) / 2.0).round_ties_even();
    ui.painter().galley(pos2(tx, top + size + 8.0), galley(ui, text, f, p.text_secondary, f32::INFINITY), p.text_secondary);
}

/// A `ComboBox` sized to its content: its widest item or placeholder.
fn combo_width(ui: &Ui, combo: &ComboBox, items: &[ComboItem], placeholder: &str) -> f32 {
    let placeholder_w = text_width(ui, placeholder, ComboBox::item_font()) + combo.padding.left + combo.padding.right + 32.0;
    combo.content_width(ui, items).max(placeholder_w).max(64.0)
}

/// `PlaceholderText` of an empty `ComboBox`.
fn combo_placeholder(ui: &Ui, rect: Rect, combo: &ComboBox, text: &str) {
    let f = fluent(ui.ctx());
    let pf = ComboBox::item_font();
    let inner = Rect::from_min_max(pos2(rect.min.x + combo.padding.left, rect.min.y + combo.padding.top), pos2(rect.max.x - 32.0, rect.max.y - combo.padding.bottom));
    let lh = line_height(ui, &pf);
    ui.painter().galley(pos2(inner.min.x, center_y(inner, lh)), galley(ui, text, pf, f.base_medium, inner.width()), f.base_medium);
}

/// Fluent's `DatePicker` button: day, month and year in three cells with
/// separators between, the selected date.
fn date_picker(ui: &Ui, rect: Rect, date: DateTime) {
    let p = palette(ui.ctx());
    // Fluent's `DatePickerButtonBackground`/`BorderBrush` and the spacers'
    // `DatePickerSpacerFill`, as ShareX shows them.
    let dark = ui.ctx().theme() == egui::Theme::Dark;
    let (fill, stroke, spacer) = if dark {
        (egui::Color32::from_gray(23), egui::Color32::from_gray(165), egui::Color32::from_gray(69))
    } else {
        (egui::Color32::from_rgb(249, 249, 250), egui::Color32::from_rgb(99, 99, 100), egui::Color32::from_rgb(199, 199, 200))
    };
    border::border(ui.painter(), rect, 4.0, 1.0, Some(Brush::Solid(fill)), Some(stroke));
    let culture = sharex_helpers::date_format::CultureFormat::current();
    let d = date.naive();
    use chrono::Datelike;
    let texts = [d.day().to_string(), culture.month_names[d.month() as usize - 1].clone(), d.year().to_string()];
    let widths = [78.0, 137.0, 78.0];
    let f = controls::content_font(13.0, 400);
    let lh = line_height(ui, &f);
    let mut x = rect.min.x + 1.0;
    for (k, (t, w)) in texts.iter().zip(widths).enumerate() {
        let cell = Rect::from_min_size(pos2(x, rect.min.y + 1.0), vec2(w, rect.height() - 2.0));
        let tx = if k == 1 { cell.min.x + 9.0 } else { cell.min.x + ((w - text_width(ui, t, f.clone())) / 2.0).round_ties_even() };
        ui.painter().galley(pos2(tx, center_y(cell, lh)), galley(ui, t, f.clone(), p.text, w), p.text);
        x += w;
        if k < 2 {
            ui.painter().rect_filled(Rect::from_min_size(pos2(x, rect.min.y + 1.0), vec2(1.0, rect.height() - 2.0)), 0.0, spacer);
            x += 1.0;
        }
    }
}

/// `UpdatePreviewAsync`'s source: the local file when it's an image, else
/// the URL when it looks like one ([`preview_url`]).
fn load_preview(item: &HistoryItem) -> Option<image::RgbaImage> {
    use sharex_helpers::file_helpers::is_image_file;
    let path = item.file_path.as_deref().unwrap_or("");
    if !path.trim().is_empty() && common::file_exists(path) && is_image_file(path) {
        return crate::thumbnail::load(std::path::Path::new(path));
    }
    None
}

/// The URL `UpdatePreviewAsync` downloads when there's no local image:
/// `FileHelpers.IsImageFile(item.URL)`.
fn preview_url(item: &HistoryItem) -> Option<String> {
    let path = item.file_path.as_deref().unwrap_or("");
    let local = !path.trim().is_empty() && common::file_exists(path) && sharex_helpers::file_helpers::is_image_file(path);
    let url = item.url.as_deref().unwrap_or("");
    (!local && !url.trim().is_empty() && sharex_helpers::file_helpers::is_image_file(url)).then(|| url.to_owned())
}

/// `PreviewHttpClient.GetByteArrayAsync(url, token)` (a plain `HttpClient`:
/// its 100 s timeout, a failed status throws) and `new Bitmap(stream)`.
/// Failures are logged (`DebugHelper.WriteException`).
fn download_preview(url: &str, cancel: sharex_uploaders::http::CancellationToken) -> Option<image::RgbaImage> {
    use sharex_uploaders::http;
    let request = http::HttpRequest { method: sharex_uploaders::HttpMethod::GET, url: url.to_owned(), headers: Vec::new(), content: None };
    let options = http::RequestOptions { timeout: Some(Duration::from_secs(100)), cancel, read_body: true, factory_client: false, ..Default::default() };
    match http::send(request, options) {
        Ok(r) if r.is_success() => match image::load_from_memory(r.response_data.as_deref().unwrap_or_default()) {
            Ok(img) => Some(img.to_rgba8()),
            Err(e) => {
                eprintln!("history preview {url}: {e}");
                None
            }
        },
        Ok(r) => {
            eprintln!("history preview {url}: Response status code does not indicate success: {} ({}).", r.status_code, r.status_description);
            None
        }
        Err(sharex_uploaders::http::HttpError::Cancelled) => None,
        Err(e) => {
            eprintln!("history preview {url}: {e}");
            None
        }
    }
}

impl Dialog for HistoryWindow {
    /// `ApplySavedWindowState`: the saved size and maximized state.
    fn spec(&self) -> Spec {
        let remember = self.settings.remember_window_state;
        let (w, h) = if remember { self.placement.saved.size_or(WIDTH, HEIGHT) } else { (WIDTH, HEIGHT) };
        Spec {
            maximized: remember && self.placement.saved.maximized,
            ..Spec::sized(crate::rs_title(s::HistoryWindows_ShareX_History()), w, h, MIN_WIDTH, MIN_HEIGHT)
        }
    }

    /// The saved position, when a screen contains it.
    fn position(&self, _work_area: sharex_capture::Rect, _size: (u32, u32)) -> Option<(i32, i32)> {
        if !self.settings.remember_window_state || self.sync {
            return None;
        }
        self.placement.saved.position(&super::window_state::screens())
    }

    fn moved(&mut self, x: i32, y: i32, _cx: &mut Cx) {
        if let Some(size) = self.placement.size {
            self.placement.normal_bounds(size, Some((x, y)));
        } else if !self.placement.maximized {
            self.placement.location = Some((x, y));
        }
    }

    fn resized(&mut self, maximized: bool, _cx: &mut Cx) {
        self.placement.maximized = maximized;
    }

    fn ui(&mut self, ui: &mut Ui, cx: &mut Cx) -> Vec<Effect> {
        if !self.loaded && self.loader.is_none() && !self.sync {
            self.refresh(ui.ctx());
        }
        if let Some(rx) = &self.loader
            && let Ok(result) = rx.try_recv()
        {
            self.loader = None;
            self.loaded_items(result);
        }
        if let Some(due) = self.filter_due {
            if Instant::now() >= due {
                self.apply_filter();
            } else {
                ui.ctx().request_repaint_after(due - Instant::now());
            }
        }
        self.keys(ui);
        let p = palette(ui.ctx());
        let full = ui.max_rect();
        // `SaveNormalWindowSize` on `Resized`.
        self.placement.normal_bounds((full.width(), full.height()), None);
        ui.painter().rect_filled(full, 0.0, p.background_main);
        // `Grid Margin="10,0,10,10" RowSpacing="10"`.
        let content = Rect::from_min_max(pos2(full.min.x + 10.0, full.min.y), pos2(full.max.x - 10.0, full.max.y - 10.0));
        let live = self.modal.is_none() && self.menu.is_none() && self.columns_menu.is_none() && self.date_flyout.is_none();
        let had_menu = self.menu.is_some() || self.columns_menu.is_some();
        self.toolbar(ui, Rect::from_min_size(content.min, vec2(content.width(), common::TOOLBAR_HEIGHT)), live);
        let top = content.min.y + common::TOOLBAR_HEIGHT + 10.0;
        let list_w = self.list_width.min(content.width() - 10.0).max(0.0);
        let panel = Rect::from_min_max(pos2(content.min.x, top), pos2(content.min.x + list_w, content.max.y));
        self.list(ui, panel, live);
        // The `GridSplitter` (10 wide).
        let splitter = Rect::from_min_max(pos2(panel.max.x, top), pos2(panel.max.x + 10.0, content.max.y));
        if live {
            let r = ui.interact(splitter, Id::new("history-splitter"), egui::Sense::drag());
            if r.hovered() || r.dragged() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
            }
            if r.drag_started() {
                self.splitter_drag = Some(self.list_width);
            }
            if r.dragged() {
                self.list_width = (self.list_width + r.drag_delta().x).clamp(0.0, content.width() - 10.0);
            }
            if r.drag_stopped() {
                self.splitter_drag = None;
                if self.settings.remember_window_state {
                    self.settings.history_list_width = self.list_width as f64;
                }
                self.settings_changed = true;
            }
        }
        let right = Rect::from_min_max(pos2(splitter.max.x, top), content.max);
        let mut preview_top = right.min.y;
        if self.advanced_open && right.width() > 0.0 {
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(right));
            child.set_clip_rect(right.intersect(ui.clip_rect()));
            preview_top += self.advanced_panel(&mut child, right, live);
        }
        if right.width() > 0.0 {
            let area = Rect::from_min_max(pos2(right.min.x, preview_top), right.max);
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(right));
            child.set_clip_rect(right.intersect(ui.clip_rect()));
            self.preview_area(&mut child, area, live);
        }

        // The context menus.
        if let Some(flyout) = self.menu.as_mut() {
            let FlyoutOutput { command, closed } = flyout.show(ui, full);
            let pressed_outside = had_menu && ui.input(|i| i.pointer.any_pressed()) && ui.input(|i| i.pointer.interact_pos()).is_some_and(|pos| !flyout.contains(pos));
            if let Some(cmd) = command {
                self.menu = None;
                self.run(cmd);
            } else if closed || pressed_outside {
                self.menu = None;
            }
        }
        if let Some(flyout) = self.columns_menu.as_mut() {
            let FlyoutOutput { command, closed } = flyout.show(ui, full);
            let pressed_outside = had_menu && ui.input(|i| i.pointer.any_pressed()) && ui.input(|i| i.pointer.interact_pos()).is_some_and(|pos| !flyout.contains(pos));
            if let Some(cmd) = command {
                self.columns_menu = None;
                match cmd {
                    ColumnCmd::Toggle(c) => {
                        let v = self.layout.is_visible(c);
                        self.layout.set_visible(c, !v);
                    }
                    ColumnCmd::Reset => self.layout.reset(),
                }
                self.settings_changed = true;
            } else if closed || pressed_outside {
                self.columns_menu = None;
            }
        }

        // A date picker's flyout: `SelectedDateChanged` → `OnDateFilterChanged`.
        if let Some((k, flyout)) = self.date_flyout.as_mut() {
            match flyout.show(ui, full) {
                super::date_picker::Out::Open => {}
                super::date_picker::Out::Dismiss => self.date_flyout = None,
                super::date_picker::Out::Accept(d) => {
                    let picked = DateTime::new(d.and_hms_opt(0, 0, 0).unwrap_or_default(), self.advanced.from.kind());
                    if *k == 0 {
                        self.advanced.from = picked;
                    } else {
                        self.advanced.to = picked;
                    }
                    self.date_flyout = None;
                    self.schedule_filter();
                }
            }
        }

        // The overlay dialogs.
        if matches!(self.modal, Some(Modal::Settings)) {
            if self.settings_dialog_ui(ui, full) {
                // `OnSaveSettingsClick`.
                self.settings.remember_search_text = self.settings_dialog.0;
                self.settings.remember_window_state = self.settings_dialog.1;
                self.settings.search_text = if self.settings.remember_search_text { self.search.clone() } else { String::new() };
                self.settings_changed = true;
                self.close_modal();
            }
        } else if let Some(modal) = self.modal.as_mut() {
            match common::modal_ui(ui, full, modal) {
                ModalOut::None => {}
                ModalOut::Close => self.close_modal(),
                ModalOut::Confirm(action, input) => {
                    let (applied, effects) = common::apply_prompt(action, &input, &mut self.items);
                    self.effects.extend(effects);
                    match applied {
                        common::Applied::Deleted(ids) => {
                            self.items.retain(|i| !ids.contains(&i.id));
                            self.process_names = query::process_names(&self.items);
                            self.apply_filter();
                        }
                        common::Applied::Edited => self.preview = None,
                        common::Applied::Nothing => {}
                    }
                    self.close_modal();
                }
                ModalOut::SaveEdit(id, fields, tags) => {
                    common::apply_edit(id, fields, &tags, &mut self.items);
                    self.process_names = query::process_names(&self.items);
                    self.preview = None;
                    self.close_modal();
                }
                ModalOut::Import(folder, images_only, skip) => {
                    let status = if !std::path::Path::new(&folder).is_dir() {
                        s::HistoryWindow_SelectedFolderDoesNotExist().to_string()
                    } else {
                        match common::import(&folder, images_only, skip, &self.items) {
                            Ok(n) => {
                                if n > 0 {
                                    self.loaded = false;
                                    self.refresh(ui.ctx());
                                }
                                common::format_count(s::HistoryWindow_SuccessfullyImportedFilesFormat(), n)
                            }
                            Err(e) => e,
                        }
                    };
                    if let Some(Modal::Import { status: st, .. }) = self.modal.as_mut() {
                        *st = status;
                    }
                }
                ModalOut::Browse => {
                    if let Some(folder) = rfd::FileDialog::new().set_title(s::HistoryWindow_SelectFolderToImport()).pick_folder()
                        && let Some(Modal::Import { folder: f, .. }) = self.modal.as_mut()
                    {
                        *f = folder.to_string_lossy().into_owned();
                    }
                }
            }
        }

        if self.settings_changed {
            self.settings_changed = false;
            self.save_settings(cx);
        }
        let title = self.title_text();
        if title != self.title {
            self.title = title.clone();
            self.effects.push(Effect::SetTitle(title));
        }
        std::mem::take(&mut self.effects)
    }

    /// `OnClosing` → `SaveWindowState`.
    fn closed(&mut self, cx: &mut Cx) -> Vec<Effect> {
        if self.settings.remember_window_state {
            self.settings.window_state = self.placement.save();
            if self.list_width > 0.0 {
                self.settings.history_list_width = self.list_width as f64;
            }
        }
        self.save_settings(cx);
        Vec::new()
    }
}

/// `--render history[-state]`: the window over `--db`, in `--state`
/// (`advanced`, `favorites`, `search` with `--text`, `stats`, `import`,
/// `settings`, `edit`, `delete`, `menu` with `--context`/`--open`, `columns`),
/// rows `--select`ed.
pub fn render_dialog(_name: &str, o: &Options) -> Result<Box<dyn Dialog>, String> {
    let mut w = HistoryWindow::new(&ApplicationConfig::default());
    w.render_state(&egui::Context::default(), o);
    Ok(Box::new(w))
}

#[cfg(test)]
mod preview_tests {
    use super::*;

    /// A 127.0.0.1 server answering one request with `status` and `body`.
    fn serve_once(status: &str, body: Vec<u8>) -> String {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let status = status.to_owned();
        std::thread::spawn(move || {
            if let Ok((mut s, _)) = listener.accept() {
                let mut buf = [0u8; 4096];
                let _ = s.read(&mut buf);
                let head = format!("HTTP/1.1 {status}\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                let _ = s.write_all(head.as_bytes());
                let _ = s.write_all(&body);
            }
        });
        format!("http://127.0.0.1:{port}/shot.png")
    }

    fn png() -> Vec<u8> {
        let mut out = std::io::Cursor::new(Vec::new());
        image::RgbaImage::from_pixel(3, 2, image::Rgba([9, 8, 7, 255])).write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    #[test]
    fn a_url_only_image_is_downloaded_for_the_preview() {
        let url = serve_once("200 OK", png());
        let item = HistoryItem { url: Some(url.clone()), file_path: Some(String::new()), ..Default::default() };
        assert_eq!(preview_url(&item).as_deref(), Some(url.as_str()));
        let img = download_preview(&url, sharex_uploaders::http::CancellationToken::new()).unwrap();
        assert_eq!(img.dimensions(), (3, 2));
        assert_eq!(img.get_pixel(1, 1).0, [9, 8, 7, 255]);
    }

    #[test]
    fn a_failed_download_shows_no_preview() {
        let url = serve_once("404 Not Found", Vec::new());
        assert!(download_preview(&url, sharex_uploaders::http::CancellationToken::new()).is_none());
        // Not an image URL: nothing is downloaded.
        let item = HistoryItem { url: Some("http://127.0.0.1:1/page.html".into()), ..Default::default() };
        assert_eq!(preview_url(&item), None);
    }
}
