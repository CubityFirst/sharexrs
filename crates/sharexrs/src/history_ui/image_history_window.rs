//! `ShareX.HistoryLib/Views/ImageHistoryWindow` (`.axaml` and code-behind):
//! the toolbar (search, Favorites, Stats, Import, Settings), the thumbnail
//! grid in rows of as many cards as fit, `MaxItemCount` at a time (the next
//! batch near the end), thumbnails from the shell or decoded to fit
//! (`ImageHistoryThumbnailLoader`, 2 to 4 at a time), selection and keys,
//! the context menu and the overlay dialogs (its settings among them).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use egui::{Id, Key, Modifiers, Rect, Ui, Vec2, pos2, vec2};
use sharex_history::actions::{CopyFormat, ImageOpenAction, MenuEntry as Entry};
use sharex_history::query::{self, ImageHistoryFilter};
use sharex_history::settings::{ImageHistorySettings, ImageHistoryThumbnailHeaderPosition as HeaderPos};
use sharex_history::HistoryItem;
use sharex_settings::ApplicationConfig;
use sharex_strings::history_lib as s;
use sharex_ui::border::{self, Brush};
use sharex_ui::controls::{Button, ComboBox, ComboItem, NumericUpDown};
use sharex_ui::icons;
use sharex_ui::keys::app_icons as icon;
use sharex_ui::menu::{Flyout, FlyoutOutput, MenuEntry, MenuStyle, Placement};
use sharex_ui::theme::{Thickness, palette};
use sharex_ui::widgets::{galley, line_height, text_width};

use super::common::{self, Cmd, MenuSpec, Modal, ModalOut};
use crate::dialogs::render::Options;
use crate::dialogs::{Cx, Dialog, Effect, FileService, Spec};

pub const WIDTH: f32 = 1100.0;
pub const HEIGHT: f32 = 720.0;
pub const MIN_WIDTH: f32 = 720.0;
pub const MIN_HEIGHT: f32 = 500.0;
const FILTER_DELAY: Duration = Duration::from_millis(220);
/// A card's chrome: `Margin="4"`, a 1px border, `Padding="4"`, both sides.
const CARD_CHROME: f32 = 18.0;

/// A thumbnail: loading, loaded, or none to show.
enum Thumb {
    Loading,
    Loaded(egui::TextureHandle, (u32, u32)),
    None,
}

/// The settings dialog's fields while it's open.
struct SettingsForm {
    width: i64,
    height: i64,
    header: usize,
    batch: i64,
    auto_load: bool,
    filter_missing: bool,
    image_only: bool,
    remember_search: bool,
    remember_window: bool,
}

pub struct ImageHistoryWindow {
    settings: ImageHistorySettings,
    items: Vec<HistoryItem>,
    loaded: bool,
    loader: Option<mpsc::Receiver<Result<Vec<HistoryItem>, String>>>,
    process_names: Vec<String>,
    /// Indexes into `items` that pass the filter.
    filtered: Vec<usize>,
    /// How many of `filtered` are loaded as cards (`_loadedEntries`).
    loaded_count: usize,
    search: String,
    filter_due: Option<Instant>,
    /// Selected cards (indexes into the loaded cards) and the anchor.
    selected: Vec<usize>,
    anchor: Option<usize>,
    scroll: f32,
    thumbs: HashMap<(i64, String), Thumb>,
    thumb_tx: Option<mpsc::Sender<(i64, String, u32, u32)>>,
    thumb_rx: Option<mpsc::Receiver<(i64, String, Option<image::RgbaImage>)>>,
    menu: Option<Flyout<Cmd>>,
    modal: Option<Modal>,
    form: Option<SettingsForm>,
    columns: usize,
    title: String,
    effects: Vec<Effect>,
    settings_changed: bool,
    scroll_to: Option<usize>,
    sync: bool,
    /// A press on a card that may be a click (not with Ctrl or Shift).
    click_armed: bool,
    /// `RememberWindowState`: the saved state and the window's bounds.
    placement: super::window_state::Tracker,
}

impl ImageHistoryWindow {
    pub fn new(config: &ApplicationConfig) -> Self {
        let settings: ImageHistorySettings = serde_json::from_value(config.ImageHistorySettings.clone()).unwrap_or_default();
        let search = if settings.remember_search_text { settings.search_text.clone() } else { String::new() };
        Self {
            items: Vec::new(),
            loaded: false,
            loader: None,
            process_names: Vec::new(),
            filtered: Vec::new(),
            loaded_count: 0,
            search,
            filter_due: None,
            selected: Vec::new(),
            anchor: None,
            scroll: 0.0,
            thumbs: HashMap::new(),
            thumb_tx: None,
            thumb_rx: None,
            menu: None,
            modal: None,
            form: None,
            columns: 1,
            title: String::new(),
            effects: Vec::new(),
            settings_changed: false,
            scroll_to: None,
            sync: false,
            click_armed: false,
            placement: super::window_state::Tracker::new(&settings.window_state),
            settings,
        }
    }

    fn title_text(&self) -> String {
        let base = crate::rs_title(s::HistoryWindows_ShareX_Image_History());
        if self.loaded { format!("{base} ({})", common::format_count(s::ImageHistoryWindow_ItemsFormat(), self.filtered.len())) } else { base }
    }

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
        let mut items = result.unwrap_or_else(|e| {
            eprintln!("history: Error occurred while reading history file: {e}");
            Vec::new()
        });
        items.reverse();
        self.process_names = query::process_names(&items);
        self.items = items;
        self.loaded = true;
        self.apply_filter();
    }

    /// `ApplyFilterAsync`, then `ResetLoadedEntries` and `LoadNextBatch`.
    fn apply_filter(&mut self) {
        self.filter_due = None;
        if !self.loaded {
            return;
        }
        let filter = ImageHistoryFilter {
            search: self.search.clone(),
            favorites: self.settings.favorites,
            media_only: self.settings.image_only,
            filter_missing: self.settings.filter_missing_files,
        };
        let matched: std::collections::HashSet<*const HistoryItem> = filter.apply(&self.items, common::file_exists).into_iter().map(|i| i as *const _).collect();
        self.filtered = self.items.iter().enumerate().filter(|(_, i)| matched.contains(&(*i as *const _))).map(|(k, _)| k).collect();
        self.loaded_count = 0;
        self.selected.clear();
        self.anchor = None;
        self.scroll = 0.0;
        self.load_next_batch();
    }

    /// `LoadNextBatch`.
    fn load_next_batch(&mut self) {
        let r = query::next_batch(self.loaded_count, self.filtered.len(), self.settings.max_item_count);
        self.loaded_count = r.end;
    }

    fn schedule_filter(&mut self) {
        if self.loaded {
            self.filter_due = Some(Instant::now() + FILTER_DELAY);
        }
    }

    fn item_at(&self, card: usize) -> Option<&HistoryItem> {
        self.filtered.get(card).map(|&i| &self.items[i])
    }

    fn selected_indexes(&self) -> Vec<usize> {
        let mut cards = self.selected.clone();
        cards.sort_unstable();
        cards.iter().filter_map(|&c| self.filtered.get(c).copied()).collect()
    }

    fn select_only(&mut self, card: usize) {
        self.selected = vec![card];
        self.anchor = Some(card);
    }

    fn select_range(&mut self, a: usize, b: usize) {
        let (lo, hi) = (a.min(b), a.max(b));
        self.selected = (lo..=hi).collect();
    }

    fn save_settings(&self, cx: &mut Cx) {
        if let Ok(v) = serde_json::to_value(&self.settings) {
            cx.settings.ImageHistorySettings = v;
        }
    }

    /// The thumbnail size (`ThumbnailSize`, 32 at least as the cards use it).
    fn thumb_size(&self) -> (u32, u32) {
        let (w, h) = self.settings.card_thumbnail_size();
        (w as u32, h as u32)
    }

    /// `OnThumbnailContextMenuOpening`'s menu.
    fn context_menu(&self) -> Vec<MenuEntry<Cmd>> {
        let idx = self.selected_indexes();
        let sel: Vec<&HistoryItem> = idx.iter().map(|&i| &self.items[i]).collect();
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
            c(Entry::CopyFilePath, s::HistoryWindows_File_path(), "", CopyFormat::FilePath),
            c(Entry::CopyFileName, s::HistoryWindows_File_name(), "", CopyFormat::FileName),
            c(Entry::CopyFileNameExtension, s::HistoryWindows_File_name_with_extension(), "", CopyFormat::FileNameWithExtension),
            c(Entry::CopyFolder, s::HistoryWindows_Folder(), "", CopyFormat::Folder),
            None,
            c(Entry::CopyHtmlImage, s::HistoryWindows_HTML_image(), "", CopyFormat::HtmlImage),
            c(Entry::CopyHtmlLinkedImage, s::HistoryWindows_HTML_linked_image(), "", CopyFormat::HtmlLinkedImage),
            c(Entry::CopyForumLink, s::HistoryWindows_Forum_link(), "", CopyFormat::ForumLink),
            c(Entry::CopyForumImage, s::HistoryWindows_Forum_image(), "", CopyFormat::ForumImage),
            c(Entry::CopyForumLinkedImage, s::HistoryWindows_Forum_linked_image(), "", CopyFormat::ForumLinkedImage),
            c(Entry::CopyMarkdownLink, s::HistoryWindows_Markdown_link(), "", CopyFormat::MarkdownLink),
            c(Entry::CopyMarkdownImage, s::HistoryWindows_Markdown_image(), "", CopyFormat::MarkdownImage),
            c(Entry::CopyMarkdownLinkedImage, s::HistoryWindows_Markdown_linked_image(), "", CopyFormat::MarkdownLinkedImage),
        ];
        let spec = vec![
            (None, vec![Some((Entry::Preview, s::HistoryWindows_Open_image().into(), icon::Image, "Enter", Cmd::OpenImage))]),
            common::open_menu(),
            common::separator(),
            (Some((s::HistoryWindows_Copy().into(), icon::Copy)), copy),
            common::separator(),
            (None, common::item_entries(s::HistoryWindows_Toggle_favorite().into())),
            common::separator(),
            (None, common::service_entries()),
        ];
        common::build_menu(&spec, &sel)
    }

    fn run(&mut self, cmd: Cmd) {
        let idx = self.selected_indexes();
        let out = common::run_cmd(cmd, &mut self.items, &idx, &common::IMAGE_HISTORY_PROMPTS);
        self.effects.extend(out.effects);
        if let Some(m) = out.modal {
            self.modal = Some(m);
        }
        if out.edited && cmd == Cmd::Favorite && self.settings.favorites {
            self.apply_filter();
        }
    }

    /// `OpenSelectedItem`.
    fn open_selected(&mut self) {
        let Some(&first) = self.selected_indexes().first() else { return };
        let item = &self.items[first];
        let path = item.file_path.clone().unwrap_or_default();
        match sharex_history::actions::image_history_open(item, common::file_exists) {
            Some(ImageOpenAction::ShowImage) => self.effects.push(Effect::FileService(FileService::ShowImage, PathBuf::from(path))),
            Some(ImageOpenAction::OpenFile) => self.effects.push(Effect::OpenFile(path.into())),
            Some(ImageOpenAction::ConfirmOpenFile) => {
                self.modal = Some(Modal::Prompt {
                    title: s::ImageHistoryWindow_OpenFile().into(),
                    message: s::ImageHistoryWindow_ConfirmOpenFileFormat().replace("{0}", &path),
                    confirm: s::HistoryWindows_Open().into(),
                    input: None,
                    action: common::PromptAction::OpenFile(path),
                    focused: false,
                })
            }
            None => {}
        }
    }

    /// `OnWindowKeyDown`.
    fn keys(&mut self, ui: &Ui) {
        let pressed = |k: Key, m: Modifiers| ui.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Key { key, pressed: true, modifiers, .. } if *key == k && *modifiers == m)));
        let any = |k: Key| ui.input(|i| i.events.iter().find_map(|e| match e { egui::Event::Key { key, pressed: true, modifiers, .. } if *key == k => Some(*modifiers), _ => None }));
        let none = Modifiers::NONE;
        let ctrl = Modifiers::COMMAND;
        let shift = Modifiers::SHIFT;
        if pressed(Key::Escape, none) {
            if self.modal.is_some() {
                self.modal = None;
                self.form = None;
            } else if self.menu.is_none() {
                self.effects.push(Effect::Close);
            }
            return;
        }
        if pressed(Key::F5, none) {
            self.loaded = false;
            self.refresh(ui.ctx());
            return;
        }
        if self.modal.is_some() || self.menu.is_some() || common::text_box_focused(ui.ctx()) {
            return;
        }
        let count = self.loaded_count;
        if pressed(Key::A, ctrl) {
            self.selected = (0..count).collect();
            return;
        }
        for k in [Key::ArrowLeft, Key::ArrowRight, Key::ArrowUp, Key::ArrowDown, Key::Home, Key::End] {
            if let Some(m) = any(k) {
                if count == 0 {
                    return;
                }
                let current = self.selected.iter().min().copied().unwrap_or(0) as i64;
                let cols = self.columns as i64;
                let next = match k {
                    Key::ArrowLeft => current - 1,
                    Key::ArrowRight => current + 1,
                    Key::ArrowUp => current - cols,
                    Key::ArrowDown => current + cols,
                    Key::Home => 0,
                    _ => count as i64 - 1,
                }
                .clamp(0, count as i64 - 1) as usize;
                if m.shift && self.anchor.is_some() {
                    self.select_range(self.anchor.unwrap(), next);
                } else {
                    self.select_only(next);
                }
                self.scroll_to = Some(next);
                return;
            }
        }
        // egui-winit turns Ctrl+C into `Event::Copy`.
        let copy = ui.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Copy)) && !i.modifiers.shift);
        if copy {
            self.run(Cmd::Copy(CopyFormat::Url));
        } else if pressed(Key::Enter, ctrl) {
            self.run(Cmd::OpenFile);
        } else if pressed(Key::Enter, shift) {
            self.run(Cmd::OpenFolder);
        } else if pressed(Key::Enter, none) {
            self.open_selected();
        } else if pressed(Key::C, ctrl) {
            self.run(Cmd::Copy(CopyFormat::Url));
        } else if pressed(Key::C, shift) {
            self.run(Cmd::CopyFile);
        } else if pressed(Key::C, Modifiers::ALT) {
            self.run(Cmd::CopyImage);
        } else if any(Key::Delete).is_some_and(|m| m.shift) || ui.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Cut)) && i.modifiers.shift) {
            self.run(Cmd::DeleteFile);
        } else if any(Key::Delete).is_some() {
            self.run(Cmd::Delete);
        } else if any(Key::U).is_some_and(|m| m.command) {
            self.run(Cmd::Service(FileService::UploadFile));
        } else if any(Key::E).is_some_and(|m| m.command) {
            self.run(Cmd::Service(FileService::EditImage));
        } else if any(Key::P).is_some_and(|m| m.command) {
            self.run(Cmd::Service(FileService::PinToScreen));
        }
    }

    fn open_settings(&mut self) {
        let st = &self.settings;
        let (w, h) = st.thumbnail_size();
        self.form = Some(SettingsForm {
            width: w as i64,
            height: h as i64,
            header: match st.thumbnail_header_position {
                HeaderPos::None => 0,
                HeaderPos::Top => 1,
                HeaderPos::Bottom => 2,
            },
            batch: st.max_item_count as i64,
            auto_load: st.auto_load_more_items,
            filter_missing: st.filter_missing_files,
            image_only: st.image_only,
            remember_search: st.remember_search_text,
            remember_window: st.remember_window_state,
        });
        self.modal = Some(Modal::Settings);
    }

    /// A card's thumbnail, asked for when first shown.
    fn thumb(&mut self, ctx: &egui::Context, item: &HistoryItem) -> Option<&Thumb> {
        let path = item.file_path.clone().unwrap_or_default();
        let key = (item.id, path.clone());
        if !self.thumbs.contains_key(&key) {
            let (w, h) = self.thumb_size();
            if self.sync {
                let t = match load_thumbnail(&path, w, h) {
                    Some(img) => {
                        let size = img.dimensions();
                        Thumb::Loaded(ctx.load_texture(format!("ih-{}", item.id), crate::thumbnail::to_color_image(&img), egui::TextureOptions::NEAREST), size)
                    }
                    None => Thumb::None,
                };
                self.thumbs.insert(key.clone(), t);
            } else {
                self.ensure_loader(ctx);
                if let Some(tx) = &self.thumb_tx {
                    let _ = tx.send((item.id, path.clone(), w, h));
                }
                self.thumbs.insert(key.clone(), Thumb::Loading);
            }
        }
        self.thumbs.get(&key)
    }

    /// `ImageHistoryThumbnailLoader`: workers (`ProcessorCount / 2`, 2 to 4)
    /// taking requests in order.
    fn ensure_loader(&mut self, ctx: &egui::Context) {
        if self.thumb_tx.is_some() {
            return;
        }
        let (req_tx, req_rx) = mpsc::channel::<(i64, String, u32, u32)>();
        let (res_tx, res_rx) = mpsc::channel();
        let req_rx = std::sync::Arc::new(std::sync::Mutex::new(req_rx));
        let workers = (std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4) / 2).clamp(2, 4);
        for _ in 0..workers {
            let rx = req_rx.clone();
            let tx = res_tx.clone();
            let ctx = ctx.clone();
            std::thread::spawn(move || {
                loop {
                    let job = rx.lock().unwrap().recv();
                    let Ok((id, path, w, h)) = job else { break };
                    let img = load_thumbnail(&path, w, h);
                    if tx.send((id, path, img)).is_err() {
                        break;
                    }
                    ctx.request_repaint();
                }
            });
        }
        self.thumb_tx = Some(req_tx);
        self.thumb_rx = Some(res_rx);
    }

    fn receive_thumbs(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.thumb_rx else { return };
        while let Ok((id, path, img)) = rx.try_recv() {
            let t = match img {
                Some(img) => {
                    let size = img.dimensions();
                    Thumb::Loaded(ctx.load_texture(format!("ih-{id}"), crate::thumbnail::to_color_image(&img), egui::TextureOptions::NEAREST), size)
                }
                None => Thumb::None,
            };
            self.thumbs.insert((id, path), t);
        }
    }

    fn render_state(&mut self, ctx: &egui::Context, o: &Options) {
        self.sync = true;
        self.refresh(ctx);
        if !o.select.is_empty() {
            self.selected = o.select.clone();
            self.anchor = o.select.first().copied();
        }
        match o.state.as_deref() {
            Some("settings") => self.open_settings(),
            Some("stats") => self.modal = Some(Modal::Stats(sharex_history::stats::output_stats(&self.items))),
            Some("search") => {
                self.search = o.text.clone().unwrap_or_default();
                self.apply_filter();
            }
            Some("menu") => {
                let pos = o.context.unwrap_or(egui::pos2(412.0, 160.0));
                let mut f = Flyout::menu(Id::new("ih-menu"), self.context_menu(), Rect::from_min_size(pos, Vec2::ZERO), Placement::Pointer(pos));
                f.style = MenuStyle::Standard;
                f.open_path(&o.open);
                self.menu = Some(f);
            }
            _ => {}
        }
    }
}

/// `ImageHistoryThumbnailLoader.LoadCore`: the shell's thumbnail (shrunk to
/// fit if it's larger, `CreatePreview`), else the image decoded to the
/// width, or to the height when that's still too tall; nothing for videos
/// without a shell thumbnail.
fn load_thumbnail(path: &str, w: u32, h: u32) -> Option<image::RgbaImage> {
    if !common::file_exists(path) {
        return None;
    }
    let p = std::path::Path::new(path);
    if let Some(t) = crate::shell::file_thumbnail(p, w, h) {
        let scale = 1f64.min((w as f64 / t.width() as f64).min(h as f64 / t.height() as f64));
        let (tw, th) = (((t.width() as f64 * scale).round() as u32).max(1), ((t.height() as f64 * scale).round() as u32).max(1));
        return Some(if (tw, th) == t.dimensions() { t } else { crate::thumbnail::resize(&t, tw, th) });
    }
    if sharex_helpers::file_helpers::is_video_file(path) {
        return None;
    }
    let img = crate::thumbnail::load(p)?;
    Some(crate::thumbnail::decode_to_fit(&img, w, h))
}

impl ImageHistoryWindow {
    fn toolbar(&mut self, ui: &mut Ui, rect: Rect, live: bool) {
        common::docked_toolbar(ui, rect);
        let y = rect.min.y + 8.0;
        let mut x = rect.min.x + 1.0 + 8.0;
        let search_rect = Rect::from_min_size(pos2(x, y), vec2(280.0, 40.0));
        let mut text = self.search.clone();
        if common::search_box(ui, search_rect, Id::new("ih-search"), &mut text) && live {
            self.search = text;
            if self.settings.remember_search_text {
                self.settings.search_text = self.search.clone();
                self.settings_changed = true;
            }
            self.schedule_filter();
        }
        x += 280.0 + common::TOOLBAR_SPACING;
        let buttons = [
            (icon::Star, s::HistoryWindows_Favorites(), self.settings.favorites),
            (icon::BarChart, s::HistoryWindows_Stats(), false),
            (icon::FolderSearch, s::HistoryWindows_Import(), false),
            (icon::Settings, s::HistoryWindows_Settings(), false),
        ];
        let mut clicked = None;
        for (i, (key, text, active)) in buttons.into_iter().enumerate() {
            let w = common::icon_button_width(ui, text);
            let r = Rect::from_min_size(pos2(x, y), vec2(w, 40.0));
            if common::icon_button(ui, r, Id::new(("ih-tool", i)), key, text, active, true).clicked() && live {
                clicked = Some(i);
            }
            x += w + common::TOOLBAR_SPACING;
        }
        match clicked {
            Some(0) => {
                self.settings.favorites = !self.settings.favorites;
                self.settings_changed = true;
                self.apply_filter();
            }
            Some(1) => self.modal = Some(Modal::Stats(sharex_history::stats::output_stats(&self.items))),
            Some(2) => self.modal = Some(Modal::Import { folder: String::new(), images_only: true, skip_duplicates: true, status: String::new() }),
            Some(3) => self.open_settings(),
            _ => {}
        }
    }

    /// The grid of cards in `area` (the `ListBox`, `Margin="6"`).
    fn grid(&mut self, ui: &mut Ui, area: Rect, live: bool) {
        let p = palette(ui.ctx());
        let (tw, th) = self.thumb_size();
        self.columns = query::image_history_columns(area.width() as f64, tw as i32);
        let normal = common::normal_font();
        let lh = line_height(ui, &normal);
        let header = self.settings.thumbnail_header_position;
        let header_h = if header == HeaderPos::None { 0.0 } else { lh + 3.0 };
        let card_w = tw as f32 + CARD_CHROME;
        let card_h = th as f32 + CARD_CHROME + header_h;
        let rows = self.loaded_count.div_ceil(self.columns.max(1));
        let content_h = rows as f32 * card_h;
        if live && ui.rect_contains_pointer(area) {
            self.scroll -= ui.input(|i| i.smooth_scroll_delta.y);
        }
        if let Some(card) = self.scroll_to.take() {
            let top = (card / self.columns) as f32 * card_h;
            if top < self.scroll {
                self.scroll = top;
            } else if top + card_h > self.scroll + area.height() {
                self.scroll = top + card_h - area.height();
            }
        }
        self.scroll = self.scroll.clamp(0.0, (content_h - area.height()).max(0.0));
        // `OnScrollChanged` / `CheckLoadMoreIfViewportNotFilled`.
        if self.settings.auto_load_more_items
            && self.loaded_count < self.filtered.len()
            && (query::should_load_more(content_h as f64, area.height() as f64, self.scroll as f64, th as i32) || content_h <= area.height() + 1.0)
        {
            self.load_next_batch();
            ui.ctx().request_repaint();
        }
        let painter = ui.painter().with_clip_rect(area.intersect(ui.clip_rect()));
        let first_row = (self.scroll / card_h).floor() as usize;
        let mut pressed: Option<(usize, Modifiers)> = None;
        let mut released = false;
        let mut double = false;
        let mut right: Option<(usize, egui::Pos2)> = None;
        let ctx = ui.ctx().clone();
        for row in first_row..rows {
            let y = area.min.y + row as f32 * card_h - self.scroll;
            if y > area.max.y {
                break;
            }
            for col in 0..self.columns {
                let card = row * self.columns + col;
                if card >= self.loaded_count {
                    break;
                }
                let item = self.item_at(card).cloned().unwrap();
                let outer = Rect::from_min_size(pos2(area.min.x + col as f32 * card_w, y), vec2(card_w, card_h));
                let border_rect = outer.shrink(4.0);
                let resp = live.then(|| ui.interact(border_rect.intersect(area), Id::new(("ih-card", card)), egui::Sense::click()));
                let hovered = resp.as_ref().is_some_and(|r| r.hovered()) && self.menu.is_none();
                let selected = self.selected.contains(&card);
                let (fill, stroke) = if hovered || selected { (p.control_background_hover, p.accent_border) } else { (p.control_background, p.border) };
                border::border(&painter, border_rect, 6.0, 1.0, Some(Brush::Solid(fill)), Some(stroke));
                let content = border_rect.shrink(5.0);
                let mut ty = content.min.y;
                let name = item.display_file_name();
                let draw_name = |ty: f32| {
                    let g = galley(ui, &name, normal.clone(), p.text, content.width());
                    let w = g.size().x.min(content.width());
                    let x = content.min.x + ((content.width() - w) / 2.0).round_ties_even().max(0.0);
                    painter.with_clip_rect(content.intersect(area)).galley(pos2(x, ty), g, p.text);
                };
                if header == HeaderPos::Top {
                    draw_name(ty);
                    ty += lh + 3.0;
                }
                let tb = Rect::from_min_size(pos2(content.min.x, ty), vec2(tw as f32, th as f32));
                border::border(&painter, tb, 4.0, 0.0, Some(Brush::Solid(p.background_panel)), None);
                let is_video = sharex_helpers::file_helpers::is_video_file(item.file_path.as_deref().unwrap_or(""));
                let thumb = self.thumb(&ctx, &item);
                let mut has_thumb = false;
                match thumb {
                    Some(Thumb::Loaded(tex, (iw, ih))) => {
                        has_thumb = true;
                        // `Stretch="Uniform" StretchDirection="DownOnly"`, centred.
                        let scale = 1f32.min((tw as f32 / *iw as f32).min(th as f32 / *ih as f32));
                        let (dw, dh) = (*iw as f32 * scale, *ih as f32 * scale);
                        let x = tb.min.x + ((tw as f32 - dw) / 2.0).round_ties_even();
                        let y = tb.min.y + ((th as f32 - dh) / 2.0).round_ties_even();
                        let r = Rect::from_min_size(pos2(x, y), vec2(dw, dh));
                        let clip = tb.intersect(area);
                        painter.with_clip_rect(clip).image(tex.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), egui::Color32::WHITE);
                    }
                    Some(Thumb::Loading) | Some(Thumb::None) | None => {
                        let loading = matches!(thumb, Some(Thumb::Loading));
                        let key = if is_video { icon::FileVideo } else { icon::Image };
                        let text_h = if loading { 6.0 + lh } else { 0.0 };
                        let total = 30.0 + text_h;
                        let top = tb.min.y + ((th as f32 - total) / 2.0).round_ties_even();
                        let tw2 = if loading { text_width(ui, s::HistoryWindows_Loading(), normal.clone()) } else { 0.0 };
                        let w = tw2.max(30.0);
                        let left = tb.min.x + ((tw as f32 - w) / 2.0).round_ties_even();
                        icons::paint(&painter, Rect::from_min_size(pos2(left + ((w - 30.0) / 2.0).round_ties_even(), top), Vec2::splat(30.0)), key, p.text_secondary);
                        if loading {
                            painter.galley(pos2(left + ((w - tw2) / 2.0).round_ties_even(), top + 36.0), galley(ui, s::HistoryWindows_Loading(), normal.clone(), p.text_secondary, f32::INFINITY), p.text_secondary);
                        }
                    }
                }
                if has_thumb && is_video {
                    // The play badge: 50px, `#B8181818`, an 18px `#A9A9A9` play icon.
                    let c = tb.center();
                    let badge = Rect::from_center_size(pos2(c.x.round(), c.y.round()), Vec2::splat(50.0));
                    border::ellipse(&painter, badge, egui::Color32::from_rgba_unmultiplied(0x18, 0x18, 0x18, 0xB8));
                    icons::paint(&painter, Rect::from_center_size(badge.center(), Vec2::splat(18.0)), icon::Play, egui::Color32::from_rgb(0xA9, 0xA9, 0xA9));
                }
                if item.favorite() {
                    let star = Rect::from_min_size(pos2(tb.max.x - 6.0 - 13.0, tb.min.y + 6.0), Vec2::splat(13.0));
                    icons::paint(&painter, star, icon::Star, p.accent_icon);
                }
                if header == HeaderPos::Bottom {
                    draw_name(tb.max.y + 3.0);
                }
                if let Some(r) = resp {
                    if r.is_pointer_button_down_on() && ui.input(|i| i.pointer.primary_pressed()) {
                        pressed = Some((card, ui.input(|i| i.modifiers)));
                    }
                    if r.clicked() {
                        released = true;
                    }
                    if r.double_clicked() {
                        double = true;
                    }
                    if r.secondary_clicked() {
                        right = Some((card, r.interact_pointer_pos().unwrap_or(border_rect.center())));
                    }
                }
            }
        }
        crate::dialogs::layout::scroll_bar(ui, area, content_h, self.scroll, 16.0);
        // `OnThumbnailPointerPressed` / `Released` / `DoubleTapped`.
        if let Some((card, m)) = pressed {
            self.click_armed = !(m.command || m.shift);
            if m.shift && self.anchor.is_some() {
                self.select_range(self.anchor.unwrap(), card);
            } else if m.command {
                if let Some(k) = self.selected.iter().position(|c| *c == card) {
                    self.selected.remove(k);
                } else {
                    self.selected.push(card);
                }
                self.anchor = Some(card);
            } else {
                self.select_only(card);
            }
        }
        if released && self.click_armed {
            self.click_armed = false;
            self.open_selected();
        } else if double {
            self.open_selected();
        }
        if let Some((card, pos)) = right {
            if !self.selected.contains(&card) {
                self.select_only(card);
            }
            let entries = self.context_menu();
            if !entries.is_empty() {
                let mut f = Flyout::menu(Id::new("ih-menu"), entries, Rect::from_min_size(pos, Vec2::ZERO), Placement::Pointer(pos));
                f.style = MenuStyle::Standard;
                self.menu = Some(f);
            }
        }
        if !self.loaded || self.filtered.is_empty() {
            let (key, text, color) = if !self.loaded {
                (icon::Loading, s::HistoryWindows_Loading(), p.accent_icon)
            } else if self.items.is_empty() {
                (icon::Images, s::HistoryWindows_No_image_history_items(), p.text_secondary)
            } else {
                (icon::Images, s::ImageHistoryWindow_NoItemsMatchCurrentFilter(), p.text_secondary)
            };
            let lh = line_height(ui, &normal);
            let tw2 = text_width(ui, text, normal.clone());
            let w = tw2.max(44.0);
            let h = 44.0 + 8.0 + lh;
            let left = area.min.x + ((area.width() - w) / 2.0).round_ties_even();
            let top = area.min.y + ((area.height() - h) / 2.0).round_ties_even();
            icons::paint(ui.painter(), Rect::from_min_size(pos2(left + ((w - 44.0) / 2.0).round_ties_even(), top), Vec2::splat(44.0)), key, color);
            ui.painter().galley(pos2(left + ((w - tw2) / 2.0).round_ties_even(), top + 52.0), galley(ui, text, normal, p.text_secondary, f32::INFINITY), p.text_secondary);
        }
    }

    /// The settings dialog (`SettingsDialog`, 480 wide, at most 650 tall).
    fn settings_ui(&mut self, ui: &mut Ui, full: Rect) -> bool {
        let layer = egui::LayerId::new(egui::Order::Foreground, Id::new("ih-settings-modal"));
        let mut ui = ui.new_child(egui::UiBuilder::new().layer_id(layer).max_rect(full));
        let _ = ui.interact(full, Id::new("ih-settings-block"), egui::Sense::click());
        let p = palette(ui.ctx());
        let heading = common::heading_font();
        let hl = line_height(&ui, &heading);
        let toggle_h = common::toggle_height(&ui);
        let content_h = 4.0 * 40.0 + 7.0 * 10.0 + 4.0 * toggle_h + 10.0 + toggle_h;
        let h = (38.0 + hl + 14.0 + content_h + 14.0 + common::BUTTON_HEIGHT).min(650.0).min(full.height());
        let c = common::modal_card(&ui, full, 480.0, h);
        ui.painter().galley(c.min, galley(&ui, s::HistoryWindows_Image_history_settings(), heading, p.text, f32::INFINITY), p.text);
        let b = Button::default();
        let done = s::HistoryWindows_Done();
        let bw = b.width(&ui, done);
        let br = Rect::from_min_size(pos2(c.max.x - bw, c.max.y - common::BUTTON_HEIGHT), vec2(bw, common::BUTTON_HEIGHT));
        let view = Rect::from_min_max(pos2(c.min.x, c.min.y + hl + 14.0), pos2(c.max.x, br.min.y - 14.0));
        let offset = crate::dialogs::layout::scroll_offset(&ui, Id::new("ih-settings-scroll"), view, content_h);
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(view));
        child.set_clip_rect(view.intersect(ui.clip_rect()));
        let form = self.form.as_mut().unwrap();
        let normal = common::normal_font();
        let label_w = 170.0;
        let field_x = view.min.x + label_w + 10.0;
        let field_r = view.max.x - 8.0;
        let mut y = view.min.y - offset;
        let label = |ui: &Ui, y: f32, text: &str| {
            sharex_ui::controls::label(ui, Rect::from_min_size(pos2(view.min.x, y), vec2(label_w, 40.0)), text, normal.clone(), p.text_secondary);
        };
        label(&child, y, s::HistoryWindows_Thumbnail_width());
        NumericUpDown { minimum: 32, maximum: 800, enabled: true }.show(&mut child, Rect::from_min_max(pos2(field_x, y), pos2(field_r, y + 40.0)), Id::new("ih-set-w"), &mut form.width);
        y += 50.0;
        label(&child, y, s::HistoryWindows_Thumbnail_height());
        NumericUpDown { minimum: 32, maximum: 800, enabled: true }.show(&mut child, Rect::from_min_max(pos2(field_x, y), pos2(field_r, y + 40.0)), Id::new("ih-set-h"), &mut form.height);
        y += 50.0;
        label(&child, y, s::HistoryWindows_Thumbnail_header());
        let items = [s::HistoryWindows_None(), s::HistoryWindows_Top(), s::HistoryWindows_Bottom()].map(ComboItem::text);
        let combo = ComboBox { padding: Thickness::hv(8.0, 6.0), enabled: true };
        // Sized to the selected item.
        let cw = combo.content_width(&child, &items[form.header..=form.header]);
        if let Some(i) = combo.show(&child, Rect::from_min_size(pos2(field_x, y), vec2(cw, 40.0)), Id::new("ih-set-header"), &items, Some(form.header)) {
            form.header = i;
        }
        y += 50.0;
        label(&child, y, s::HistoryWindows_Items_per_batch());
        NumericUpDown { minimum: 0, maximum: 100000, enabled: true }.show(&mut child, Rect::from_min_max(pos2(field_x, y), pos2(field_r, y + 40.0)), Id::new("ih-set-batch"), &mut form.batch);
        y += 50.0;
        let toggles: [(&str, &mut bool, f32); 5] = [
            (s::HistoryWindows_Load_the_next_batch_near_the_end(), &mut form.auto_load, 10.0),
            (s::HistoryWindows_Hide_missing_files(), &mut form.filter_missing, 10.0),
            (s::HistoryWindows_Show_image_files_only(), &mut form.image_only, 10.0),
            (s::HistoryWindows_Remember_search_text(), &mut form.remember_search, 10.0),
            (s::HistoryWindows_Remember_window_size_and_position(), &mut form.remember_window, 0.0),
        ];
        for (k, (text, value, gap)) in toggles.into_iter().enumerate() {
            common::toggle_with_header(&child, pos2(view.min.x, y), Id::new(("ih-set-toggle", k)), text, value);
            y += toggle_h + gap;
        }
        crate::dialogs::layout::scroll_bar(&ui, view, content_h, offset, 0.0);
        b.show(&ui, br, Id::new("ih-set-done"), done, true).clicked()
    }
}

impl Dialog for ImageHistoryWindow {
    /// `ApplySavedWindowState`: the saved size and maximized state.
    fn spec(&self) -> Spec {
        let remember = self.settings.remember_window_state;
        let (w, h) = if remember { self.placement.saved.size_or(WIDTH, HEIGHT) } else { (WIDTH, HEIGHT) };
        Spec {
            maximized: remember && self.placement.saved.maximized,
            ..Spec::sized(crate::rs_title(s::HistoryWindows_ShareX_Image_History()), w, h, MIN_WIDTH, MIN_HEIGHT)
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
        self.receive_thumbs(ui.ctx());
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
        let live = self.modal.is_none() && self.menu.is_none();
        let had_menu = self.menu.is_some();
        let tb = Rect::from_min_size(pos2(full.min.x + 10.0, full.min.y), vec2(full.width() - 20.0, common::TOOLBAR_HEIGHT));
        self.toolbar(ui, tb, live);
        let area = Rect::from_min_max(pos2(full.min.x + 6.0, tb.max.y + 6.0), pos2(full.max.x - 6.0, full.max.y - 6.0));
        self.grid(ui, area, live);

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

        if matches!(self.modal, Some(Modal::Settings)) && self.form.is_some() {
            if self.settings_ui(ui, full) {
                // `OnSaveSettingsClick`.
                let f = self.form.take().unwrap();
                let st = &mut self.settings;
                st.set_thumbnail_size(f.width as i32, f.height as i32);
                st.thumbnail_header_position = match f.header {
                    0 => HeaderPos::None,
                    1 => HeaderPos::Top,
                    _ => HeaderPos::Bottom,
                };
                st.max_item_count = f.batch as i32;
                st.auto_load_more_items = f.auto_load;
                st.filter_missing_files = f.filter_missing;
                st.image_only = f.image_only;
                st.remember_search_text = f.remember_search;
                st.remember_window_state = f.remember_window;
                st.search_text = if f.remember_search { self.search.clone() } else { String::new() };
                self.modal = None;
                self.thumbs.clear();
                self.settings_changed = true;
                self.apply_filter();
            }
        } else if let Some(modal) = self.modal.as_mut() {
            match common::modal_ui(ui, full, modal) {
                ModalOut::None => {}
                ModalOut::Close => self.modal = None,
                ModalOut::Confirm(action, input) => {
                    let (applied, effects) = common::apply_prompt(action, &input, &mut self.items);
                    self.effects.extend(effects);
                    match applied {
                        common::Applied::Deleted(ids) => {
                            self.items.retain(|i| !ids.contains(&i.id));
                            self.process_names = query::process_names(&self.items);
                            self.apply_filter();
                        }
                        common::Applied::Edited => self.thumbs.clear(),
                        common::Applied::Nothing => {}
                    }
                    self.modal = None;
                }
                ModalOut::SaveEdit(id, fields, tags) => {
                    common::apply_edit(id, fields, &tags, &mut self.items);
                    self.process_names = query::process_names(&self.items);
                    self.thumbs.clear();
                    self.modal = None;
                }
                ModalOut::Import(folder, images_only, skip) => {
                    let status = if !std::path::Path::new(&folder).is_dir() {
                        s::ImageHistoryWindow_SelectedFolderDoesNotExist().to_string()
                    } else {
                        match common::import(&folder, images_only, skip, &self.items) {
                            Ok(n) => {
                                if n > 0 {
                                    self.loaded = false;
                                    self.refresh(ui.ctx());
                                }
                                common::format_count(s::ImageHistoryWindow_SuccessfullyImportedFilesFormat(), n)
                            }
                            Err(e) => e,
                        }
                    };
                    if let Some(Modal::Import { status: st, .. }) = self.modal.as_mut() {
                        *st = status;
                    }
                }
                ModalOut::Browse => {
                    if let Some(folder) = rfd::FileDialog::new().set_title(s::ImageHistoryWindow_SelectFolderToImport()).pick_folder()
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
        if self.thumbs.values().any(|t| matches!(t, Thumb::Loading)) {
            ui.ctx().request_repaint_after(Duration::from_millis(50));
        }
        std::mem::take(&mut self.effects)
    }

    /// `OnClosing` → `SaveWindowState`.
    fn closed(&mut self, cx: &mut Cx) -> Vec<Effect> {
        if self.settings.remember_window_state {
            self.settings.window_state = self.placement.save();
        }
        self.save_settings(cx);
        Vec::new()
    }
}

/// `--render image-history`: the window over `--db`, in `--state`
/// (`settings`, `stats`, `search` with `--text`, `menu` with `--context`),
/// cards `--select`ed.
pub fn render_dialog(_name: &str, o: &Options) -> Result<Box<dyn Dialog>, String> {
    let mut w = ImageHistoryWindow::new(&ApplicationConfig::default());
    w.render_state(&egui::Context::default(), o);
    Ok(Box::new(w))
}
