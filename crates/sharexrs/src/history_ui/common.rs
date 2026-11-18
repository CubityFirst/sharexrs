//! What the History and Image history windows share: the docked toolbar
//! (`Border.tool-panel.docked-top-toolbar`) with its search box and icon
//! buttons, the overlay dialogs (`ModalOverlay`: statistics, import, edit
//! item, the prompts), the context menu's commands and what they do
//! (`HistoryContextMenuHelpers`, the windows' click handlers).

use std::collections::HashSet;
use std::path::PathBuf;

use egui::{CornerRadius, FontId, Id, Rect, Response, Ui, Vec2, pos2, vec2};
use sharex_helpers::date_format::CultureFormat;
use sharex_history::actions::{self, CopyFormat, MenuEntry as Entry, Services};
use sharex_history::HistoryItem;
use sharex_strings::history_lib as s;
use sharex_ui::border::{self, Brush};
use sharex_ui::controls::{self, Button, CheckBox, TextBox, ToggleSwitch, content_font};
use sharex_ui::icons;
use sharex_ui::keys::app_icons as icon;
use sharex_ui::menu::{MenuEntry, MenuIcon};
use sharex_ui::theme::{Thickness, palette};
use sharex_ui::widgets::{center_y, font, galley, line_height, text_width};

use crate::dialogs::{Effect, FileService};

/// `Border.docked-top-toolbar`: `Padding="8"`, a 40px row, no top border.
pub const TOOLBAR_HEIGHT: f32 = 8.0 + 40.0 + 8.0 + 1.0;
/// `ColumnSpacing="6"` of the toolbar's grid.
pub const TOOLBAR_SPACING: f32 = 6.0;
/// `Button`'s height (ControlStyles.axaml).
pub const BUTTON_HEIGHT: f32 = 40.0;

/// Every file exists the way `File.Exists` sees it (a path, not a folder).
pub fn file_exists(path: &str) -> bool {
    !path.is_empty() && std::path::Path::new(path).is_file()
}

/// Which services the app gives the windows (`TaskHelpers.OpenHistory`
/// fills in all of them; the ones not ported yet log when used).
pub fn services() -> Services {
    Services {
        upload_file: true,
        edit_image: true,
        edit_image_in_photo_editor: true,
        trim_video: true,
        trim_animated_gif: true,
        pin_to_screen: true,
        analyze_image: true,
        show_image: true,
    }
}

/// `DateTime.ToString("g")` in the current culture.
pub fn format_g(item: &HistoryItem) -> String {
    CultureFormat::current().format_g(item.date_time.naive())
}

/// `Border.tool-panel.docked-top-toolbar` in `rect` (its border box): the
/// panel background, a 1px border left, right and below, the bottom corners
/// rounded 4.
pub fn docked_toolbar(ui: &Ui, rect: Rect) {
    let p = palette(ui.ctx());
    let radius = CornerRadius { sw: 4, se: 4, ..Default::default() };
    ui.painter().rect_filled(rect, radius, p.border);
    let inner = Rect::from_min_max(pos2(rect.min.x + 1.0, rect.min.y), pos2(rect.max.x - 1.0, rect.max.y - 1.0));
    ui.painter().rect_filled(inner, CornerRadius { sw: 3, se: 3, ..Default::default() }, p.background_panel);
}

/// `Border.tool-panel` (with its own padding set by the caller): the panel
/// background, a 1px border, 4px corners.
pub fn tool_panel(ui: &Ui, rect: Rect) {
    let p = palette(ui.ctx());
    border::border(ui.painter(), rect, 4.0, 1.0, Some(Brush::Solid(p.background_panel)), Some(p.border));
}

/// A `TextBlock` in a `Button` (ShareX's `Button TextBlock` style: Segoe UI
/// Normal).
fn button_text_font() -> FontId {
    font(13.0, 400)
}

/// A toolbar `Button` with an `IconPresenter` (the button's 13px font) and
/// a `TextBlock`, `StackPanel Spacing="8"`, `Padding="10,6"`.
pub fn icon_button_width(ui: &Ui, text: &str) -> f32 {
    1.0 + 10.0 + 13.0 + 8.0 + text_width(ui, text, button_text_font()) + 10.0 + 1.0
}

/// Draws that button; `active` is the `.active` class (the accent fill,
/// its foreground for the icon and text).
pub fn icon_button(ui: &Ui, rect: Rect, id: Id, key: &str, text: &str, active: bool, enabled: bool) -> Response {
    let p = palette(ui.ctx());
    let r = if active {
        let r = ui.interact(rect, id, if enabled { egui::Sense::click() } else { egui::Sense::hover() });
        border::border(ui.painter(), rect, 4.0, 1.0, Some(Brush::accent(ui.ctx())), Some(p.border));
        r
    } else {
        Button::compact(Thickness::hv(10.0, 6.0)).show(ui, rect, id, "", enabled)
    };
    let opacity = if enabled { 1.0 } else { 0.35 };
    let body = button_text_font();
    let tw = text_width(ui, text, body.clone());
    let content_w = 13.0 + 8.0 + tw;
    let inner = Rect::from_min_max(pos2(rect.min.x + 11.0, rect.min.y + 7.0), pos2(rect.max.x - 11.0, rect.max.y - 7.0));
    let x = inner.min.x + ((inner.width() - content_w) / 2.0).round_ties_even().max(0.0);
    let lh = line_height(ui, &body);
    let top = center_y(inner, lh);
    let (icon_color, text_color) = if active { (p.accent_foreground, p.accent_foreground) } else { (p.text, p.text_secondary) };
    icons::paint(ui.painter(), Rect::from_min_size(pos2(x, top + (lh - 13.0) / 2.0), Vec2::splat(13.0)), key, icon_color.gamma_multiply(opacity));
    let c = text_color.gamma_multiply(opacity);
    ui.painter().galley(pos2(x + 21.0, top), galley(ui, text, body, c, f32::INFINITY), c);
    r
}

/// A square `Button.toolbar-button` (`Height="32" Width="32"`) with an
/// icon of the button's 18px font.
pub fn small_icon_button(ui: &Ui, rect: Rect, id: Id, key: &str) -> Response {
    let p = palette(ui.ctx());
    let r = Button::compact(Thickness::hv(0.0, 0.0)).show(ui, rect, id, "", true);
    let inner = rect.shrink(1.0);
    let size = 18.0;
    let b = Rect::from_min_size(pos2(inner.min.x + ((inner.width() - size) / 2.0).round_ties_even(), center_y(inner, size)), Vec2::splat(size));
    icons::paint(ui.painter(), b, key, p.text);
    r
}

/// The search `AutoCompleteBox` (`Padding="12,7,38,7"`) with the search icon
/// 12px from its right edge. Returns whether the text changed.
pub fn search_box(ui: &mut Ui, rect: Rect, id: Id, text: &mut String) -> bool {
    let p = palette(ui.ctx());
    let before = text.clone();
    let tb = TextBox { padding: Thickness::new(12.0, 7.0, 38.0, 7.0), placeholder: s::HistoryWindows_Search_file_names_and_tags(), ..TextBox::default() };
    tb.show(ui, rect, id, text);
    let icon_rect = Rect::from_min_size(pos2(rect.max.x - 12.0 - 13.0, center_y(rect, 13.0)), Vec2::splat(13.0));
    icons::paint(ui.painter(), icon_rect, icon::Search, p.text_secondary);
    *text != before
}

/// A dialog card over `ShareX.Brush.Overlay.Modal`, centred in `full`:
/// `width` wide (border box), `height` tall. Returns the content box
/// (inside the 1px border and `Padding="18"`), drawn on a foreground layer.
pub fn modal_card(ui: &Ui, full: Rect, width: f32, height: f32) -> Rect {
    let p = palette(ui.ctx());
    ui.painter().rect_filled(full, 0.0, p.overlay_modal);
    let x = full.min.x + ((full.width() - width) / 2.0).round_ties_even();
    let y = full.min.y + ((full.height() - height) / 2.0).round_ties_even();
    let card = Rect::from_min_size(pos2(x, y), vec2(width, height));
    tool_panel(ui, card);
    card.shrink(19.0)
}

/// `TextBlock.section-heading`: 15px SemiBold, the text colour.
pub fn heading_font() -> FontId {
    font(15.0, 600)
}

/// A plain `TextBlock` with `FontWeight="Normal"`.
pub fn normal_font() -> FontId {
    font(13.0, 400)
}

/// A `ToggleSwitch` with `Content`: the content (Inter SemiBold, the text
/// colour) as its header, the switch under it. Returns its height.
pub fn toggle_with_header(ui: &Ui, pos: egui::Pos2, id: Id, text: &str, on: &mut bool) -> f32 {
    let p = palette(ui.ctx());
    let f = content_font(13.0, 600);
    let lh = line_height(ui, &f);
    ui.painter().galley(pos, galley(ui, text, f, p.text, f32::INFINITY), p.text);
    // `ToggleSwitchTopHeaderMargin` under the header.
    let top = pos.y + lh + TOGGLE_HEADER_GAP;
    ToggleSwitch { enabled: true }.show(ui, pos2(pos.x, top), id, on);
    toggle_height(ui)
}

/// The space between a `ToggleSwitch`'s header and its switch.
const TOGGLE_HEADER_GAP: f32 = 6.0;

/// A `ToggleSwitch` with a header: the header line, the gap, the switch.
pub fn toggle_height(ui: &Ui) -> f32 {
    line_height(ui, &content_font(13.0, 600)) + TOGGLE_HEADER_GAP + ToggleSwitch::SIZE.y
}

/// The commands of the windows' context menus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cmd {
    OpenImage,
    OpenUrl,
    OpenShortUrl,
    OpenThumbnailUrl,
    OpenDeletionUrl,
    OpenFile,
    OpenFolder,
    Copy(CopyFormat),
    CopyFile,
    CopyImage,
    CopyText,
    Favorite,
    EditTag,
    EditItem,
    Rename,
    Delete,
    DeleteFile,
    Preview,
    Service(FileService),
}

/// A context menu entry: its rule (`HistoryContextMenuHelpers`), header,
/// icon, gesture and command; `None` for a separator.
pub type MenuSpec = Option<(Entry, String, &'static str, &'static str, Cmd)>;

/// A group of a context menu: a submenu (its header and icon) of entries,
/// top-level entries (`None`), or a separator (`None` and no entries).
pub type MenuGroup = (Option<(String, &'static str)>, Vec<MenuSpec>);

/// Builds a context menu from `spec` with `UpdateVisibility`'s rules: each
/// entry shows when it's available, a submenu when any of its entries
/// does, and separators only between shown entries.
pub fn build_menu(spec: &[MenuGroup], selected: &[&HistoryItem]) -> Vec<MenuEntry<Cmd>> {
    let services = services();
    let available = |e: Entry| actions::menu_entry_available(e, selected, &services, file_exists);
    let level = |items: &[MenuSpec]| -> Vec<MenuEntry<Cmd>> {
        let mut out: Vec<MenuEntry<Cmd>> = Vec::new();
        let mut pending_separator = false;
        for item in items {
            match item {
                None => pending_separator = !out.is_empty(),
                Some((rule, header, key, gesture, cmd)) => {
                    if !available(*rule) {
                        continue;
                    }
                    if pending_separator {
                        out.push(MenuEntry::separator());
                        pending_separator = false;
                    }
                    let mut e = plain_icon(MenuEntry::item(header.clone(), "", *cmd), key);
                    if !gesture.is_empty() {
                        e = e.gesture(*gesture);
                    }
                    out.push(e);
                }
            }
        }
        out
    };
    let mut out: Vec<MenuEntry<Cmd>> = Vec::new();
    let mut pending_separator = false;
    for (parent, items) in spec {
        // An empty group is a separator between the groups around it.
        if parent.is_none() && items.is_empty() {
            pending_separator = true;
            continue;
        }
        match parent {
            // A group of top-level entries.
            None => {
                let entries = level(items);
                if entries.is_empty() {
                    continue;
                }
                if pending_separator && !out.is_empty() {
                    out.push(MenuEntry::separator());
                }
                out.extend(entries);
                pending_separator = false;
            }
            Some((header, key)) => {
                let children = level(items);
                if children.is_empty() {
                    continue;
                }
                if pending_separator && !out.is_empty() {
                    out.push(MenuEntry::separator());
                }
                out.push(plain_icon(MenuEntry::parent(header.clone(), "", children), key));
                pending_separator = false;
            }
        }
    }
    out
}

/// A plain `MenuItem.Icon` (`IconPresenter`, the item's 14px font and
/// foreground).
fn plain_icon(e: MenuEntry<Cmd>, key: &'static str) -> MenuEntry<Cmd> {
    if key.is_empty() { e } else { e.icon(MenuIcon::Plain(key, 14.0)) }
}

/// An empty group: a separator between the groups around it.
pub fn separator() -> MenuGroup {
    (None, Vec::new())
}

/// The Open › submenu (both windows).
pub fn open_menu() -> MenuGroup {
    (
        Some((s::HistoryWindows_Open().into(), icon::ExternalLink)),
        vec![
            Some((Entry::OpenUrl, s::HistoryWindows_URL().into(), "", "", Cmd::OpenUrl)),
            Some((Entry::OpenShortUrl, s::HistoryWindows_Shortened_URL().into(), "", "", Cmd::OpenShortUrl)),
            Some((Entry::OpenThumbnailUrl, s::HistoryWindows_Thumbnail_URL().into(), "", "", Cmd::OpenThumbnailUrl)),
            Some((Entry::OpenDeletionUrl, s::HistoryWindows_Deletion_URL().into(), "", "", Cmd::OpenDeletionUrl)),
            None,
            Some((Entry::OpenFile, s::HistoryWindows_File().into(), "", "Ctrl+Enter", Cmd::OpenFile)),
            Some((Entry::OpenFolder, s::HistoryWindows_Folder().into(), "", "Shift+Enter", Cmd::OpenFolder)),
        ],
    )
}

/// The entries after Copy › (both windows, as each lists them).
pub fn item_entries(favorite_header: String) -> Vec<MenuSpec> {
    vec![
        Some((Entry::Other, favorite_header, icon::Star, "", Cmd::Favorite)),
        Some((Entry::Tag, s::HistoryWindows_Edit_tag().into(), icon::Tag, "", Cmd::EditTag)),
        Some((Entry::Edit, s::HistoryWindows_Edit_item().into(), icon::Edit, "", Cmd::EditItem)),
        Some((Entry::Rename, s::HistoryWindows_Rename_file().into(), icon::FileEdit, "", Cmd::Rename)),
        Some((Entry::Other, s::HistoryWindows_Delete_from_history().into(), icon::ClearList, "Delete", Cmd::Delete)),
        Some((Entry::DeleteFile, s::HistoryWindows_Delete_file_and_history_item().into(), icon::FileDelete, "Shift+Delete", Cmd::DeleteFile)),
    ]
}

/// The service entries at the end of both menus.
pub fn service_entries() -> Vec<MenuSpec> {
    use FileService as F;
    vec![
        Some((Entry::Upload, s::HistoryWindows_Upload_file().into(), icon::Upload, "Ctrl+U", Cmd::Service(F::UploadFile))),
        Some((Entry::EditImage, s::HistoryWindows_Edit_image().into(), icon::ImageEditor, "Ctrl+E", Cmd::Service(F::EditImage))),
        Some((Entry::EditImageInPhotoEditor, s::HistoryWindows_EditImageInPhotoEditor().into(), icon::Layers, "", Cmd::Service(F::EditImageInPhotoEditor))),
        Some((Entry::TrimVideo, s::HistoryWindows_TrimVideo().into(), icon::Scissors, "", Cmd::Service(F::TrimVideo))),
        Some((Entry::TrimAnimatedGif, s::HistoryWindows_TrimAnimatedGif().into(), icon::Scissors, "", Cmd::Service(F::TrimAnimatedGif))),
        Some((Entry::Pin, s::HistoryWindows_Pin_to_screen().into(), icon::Pin, "Ctrl+P", Cmd::Service(F::PinToScreen))),
        Some((Entry::Analyze, s::HistoryWindows_Analyze_image().into(), icon::InspectWindow, "", Cmd::Service(F::AnalyzeImage))),
    ]
}

/// What an overlay prompt does when confirmed (`_promptAction`).
#[derive(Clone, Debug)]
pub enum PromptAction {
    EditTag(i64),
    Rename(i64),
    Delete { ids: Vec<i64>, files: bool },
    OpenFile(String),
}

/// The overlay dialogs (`ShowModal`).
pub enum Modal {
    /// `StatsDialog`: `HistoryHelpers.OutputStats`.
    Stats(String),
    /// `ImportDialog`.
    Import { folder: String, images_only: bool, skip_duplicates: bool, status: String },
    /// `SettingsDialog` (the window draws its own).
    Settings,
    /// `EditDialog`: the item's id and its fields as typed.
    Edit { id: i64, fields: [String; 8], date: String, tags: String },
    /// `PromptDialog`.
    Prompt { title: String, message: String, confirm: String, input: Option<String>, action: PromptAction },
}

impl Modal {
    /// `OnEditItemClick`: the item's fields, its date as `ToString("G")`,
    /// its tags as `key=value` lines.
    pub fn edit(item: &HistoryItem) -> Self {
        let v = |o: &Option<String>| o.clone().unwrap_or_default();
        Modal::Edit {
            id: item.id,
            fields: [
                v(&item.file_name),
                v(&item.file_path),
                v(&item.r#type),
                v(&item.host),
                v(&item.url),
                v(&item.thumbnail_url),
                v(&item.deletion_url),
                v(&item.shortened_url),
            ],
            date: CultureFormat::current().format_long_g(item.date_time.naive()),
            tags: actions::tags_to_edit_text(item.tags.as_ref()),
        }
    }
}

/// What a modal's frame did.
pub enum ModalOut {
    None,
    Close,
    /// The prompt's confirm button (with the text box's text).
    Confirm(PromptAction, String),
    /// Edit item's Save (the item's new fields).
    SaveEdit(i64, [String; 8], String),
    /// Import's Import (folder, images only, skip duplicates).
    Import(String, bool, bool),
    /// Import's Browse...
    Browse,
}

/// Lays out and draws the overlay `modal` over `full` (the stats, import,
/// edit and prompt dialogs; the settings dialog is the window's own).
pub fn modal_ui(ui: &mut Ui, full: Rect, modal: &mut Modal) -> ModalOut {
    let layer = egui::LayerId::new(egui::Order::Foreground, Id::new("history-modal"));
    let mut ui = ui.new_child(egui::UiBuilder::new().layer_id(layer).max_rect(full));
    let _ = ui.interact(full, Id::new("history-modal-block"), egui::Sense::click());
    let p = palette(ui.ctx());
    let heading = heading_font();
    let heading_h = line_height(&ui, &heading);
    let b = Button::default();
    let mut out = ModalOut::None;
    match modal {
        Modal::Stats(text) => {
            let c = modal_card(&ui, full, 640.0, 540.0);
            ui.painter().galley(c.min, galley(&ui, s::HistoryWindows_History_statistics(), heading, p.text, f32::INFINITY), p.text);
            let close = s::HistoryWindows_Close();
            let bw = b.width(&ui, close);
            let br = Rect::from_min_size(pos2(c.max.x - bw, c.max.y - BUTTON_HEIGHT), vec2(bw, BUTTON_HEIGHT));
            let tr = Rect::from_min_max(pos2(c.min.x, c.min.y + heading_h + 12.0), pos2(c.max.x, br.min.y - 12.0));
            TextBox { multiline: true, read_only: true, ..TextBox::default() }.show(&mut ui, tr, Id::new("history-stats-text"), text);
            if b.show(&ui, br, Id::new("history-stats-close"), close, true).clicked() {
                out = ModalOut::Close;
            }
        }
        Modal::Import { folder, images_only, skip_duplicates, status } => {
            let normal = normal_font();
            let lh = line_height(&ui, &normal);
            let inner_w = 540.0 - 38.0;
            let status_lines = if status.is_empty() { 1 } else { controls::wrap_lines(&ui, status, &normal, inner_w).len().max(1) };
            let h = 38.0 + heading_h + 12.0 + 40.0 + 12.0 + 32.0 + 8.0 + 32.0 + 12.0 + lh * status_lines as f32 + 12.0 + BUTTON_HEIGHT;
            let c = modal_card(&ui, full, 540.0, h);
            ui.painter().galley(c.min, galley(&ui, s::HistoryWindows_Import_folder(), heading, p.text, f32::INFINITY), p.text);
            let mut y = c.min.y + heading_h + 12.0;
            let browse = s::HistoryWindows_Browse();
            let bw = b.width(&ui, browse);
            let br = Rect::from_min_size(pos2(c.max.x - bw, y), vec2(bw, 40.0));
            let tr = Rect::from_min_max(pos2(c.min.x, y), pos2(br.min.x - 8.0, y + 40.0));
            TextBox { placeholder: s::HistoryWindows_Folder_path(), ..TextBox::default() }.show(&mut ui, tr, Id::new("history-import-folder"), folder);
            if b.show(&ui, br, Id::new("history-import-browse"), browse, true).clicked() {
                out = ModalOut::Browse;
            }
            y += 40.0 + 12.0;
            let cb = CheckBox::new(s::HistoryWindows_Only_import_image_files());
            cb.show(&ui, Rect::from_min_size(pos2(c.min.x, y), vec2(cb.width(&ui), 32.0)), Id::new("history-import-images"), images_only);
            y += 32.0 + 8.0;
            let cb = CheckBox::new(s::HistoryWindows_Skip_duplicate_files());
            cb.show(&ui, Rect::from_min_size(pos2(c.min.x, y), vec2(cb.width(&ui), 32.0)), Id::new("history-import-skip"), skip_duplicates);
            y += 32.0 + 12.0;
            controls::wrapped(&ui, pos2(c.min.x, y), inner_w, status, normal, p.text_secondary, true);
            let import = s::HistoryWindows_Import();
            let cancel = s::HistoryWindows_Cancel();
            let (iw, cw) = (b.width(&ui, import), b.width(&ui, cancel));
            let cr = Rect::from_min_size(pos2(c.max.x - cw, c.max.y - BUTTON_HEIGHT), vec2(cw, BUTTON_HEIGHT));
            let ir = Rect::from_min_size(pos2(cr.min.x - 8.0 - iw, cr.min.y), vec2(iw, BUTTON_HEIGHT));
            let can_import = !folder.trim().is_empty();
            if b.show(&ui, ir, Id::new("history-import-ok"), import, can_import).clicked() {
                out = ModalOut::Import(folder.trim().to_string(), *images_only, *skip_duplicates);
            }
            if b.show(&ui, cr, Id::new("history-import-cancel"), cancel, true).clicked() {
                out = ModalOut::Close;
            }
        }
        Modal::Settings => {}
        Modal::Edit { id, fields, date, tags } => {
            let labels = [
                s::HistoryWindows_File_name(),
                s::HistoryWindows_File_path(),
                s::HistoryWindows_Date(),
                s::HistoryWindows_Type(),
                s::HistoryWindows_Host(),
                s::HistoryWindows_URL(),
                s::HistoryWindows_Thumbnail_URL(),
                s::HistoryWindows_Deletion_URL(),
                s::HistoryWindows_Shortened_URL(),
            ];
            let content_h = 9.0 * 40.0 + 110.0 + 9.0 * 8.0;
            let natural = 38.0 + heading_h + 12.0 + content_h + 12.0 + BUTTON_HEIGHT;
            let h = natural.min(650.0).min(full.height());
            let c = modal_card(&ui, full, 700.0, h);
            ui.painter().galley(c.min, galley(&ui, s::HistoryWindows_Edit_history_item(), heading, p.text, f32::INFINITY), p.text);
            let save = s::HistoryWindows_Save();
            let cancel = s::HistoryWindows_Cancel();
            let (sw, cw) = (b.width(&ui, save), b.width(&ui, cancel));
            let cr = Rect::from_min_size(pos2(c.max.x - cw, c.max.y - BUTTON_HEIGHT), vec2(cw, BUTTON_HEIGHT));
            let sr = Rect::from_min_size(pos2(cr.min.x - 8.0 - sw, cr.min.y), vec2(sw, BUTTON_HEIGHT));
            // The `ScrollViewer` between the heading and the buttons; its
            // grid keeps 8px for the scroll bar (`Margin="0,0,8,0"`).
            let view = Rect::from_min_max(pos2(c.min.x, c.min.y + heading_h + 12.0), pos2(c.max.x, cr.min.y - 12.0));
            let scroll_id = Id::new("history-edit-scroll");
            let offset = crate::dialogs::layout::scroll_offset(&ui, scroll_id, view, content_h);
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(view));
            child.set_clip_rect(view.intersect(ui.clip_rect()));
            let normal = normal_font();
            let label_x = view.min.x;
            let box_x = view.min.x + 120.0 + 10.0;
            let box_r = view.max.x - 8.0;
            let mut y = view.min.y - offset;
            for (row, label) in labels.iter().enumerate() {
                let r = Rect::from_min_size(pos2(box_x, y), vec2(box_r - box_x, 40.0));
                controls::label(&child, Rect::from_min_max(pos2(label_x, y), pos2(box_x - 10.0, y + 40.0)), label, normal.clone(), p.text_secondary);
                let id = Id::new(("history-edit-field", row));
                if row == 2 {
                    TextBox { read_only: true, ..TextBox::default() }.show(&mut child, r, id, date);
                } else {
                    let i = if row < 2 { row } else { row - 1 };
                    TextBox::default().show(&mut child, r, id, &mut fields[i]);
                }
                y += 48.0;
            }
            let lh = line_height(&child, &normal);
            controls::label(&child, Rect::from_min_size(pos2(label_x, y + 8.0), vec2(120.0, lh)), s::HistoryWindows_Tags(), normal, p.text_secondary);
            let tr = Rect::from_min_size(pos2(box_x, y), vec2(box_r - box_x, 110.0));
            TextBox { multiline: true, placeholder: s::HistoryWindows_One_key_value_pair_per_line(), ..TextBox::default() }.show(
                &mut child,
                tr,
                Id::new("history-edit-tags"),
                tags,
            );
            crate::dialogs::layout::scroll_bar(&ui, view, content_h, offset, 0.0);
            if b.show(&ui, sr, Id::new("history-edit-save"), save, true).clicked() {
                out = ModalOut::SaveEdit(*id, fields.clone(), tags.clone());
            }
            if b.show(&ui, cr, Id::new("history-edit-cancel"), cancel, true).clicked() {
                out = ModalOut::Close;
            }
        }
        Modal::Prompt { title, message, confirm, input, action } => {
            let normal = normal_font();
            let inner_w = 480.0 - 38.0;
            let msg_h = controls::wrapped(&ui, pos2(0.0, 0.0), inner_w, message, normal.clone(), p.text, false);
            // The hidden text box's row keeps its `RowSpacing`.
            let mut h = 38.0 + heading_h + 12.0 + msg_h + 12.0 + 12.0 + BUTTON_HEIGHT;
            if input.is_some() {
                h += 40.0;
            }
            let c = modal_card(&ui, full, 480.0, h);
            ui.painter().galley(c.min, galley(&ui, title, heading, p.text, f32::INFINITY), p.text);
            let mut y = c.min.y + heading_h + 12.0;
            controls::wrapped(&ui, pos2(c.min.x, y), inner_w, message, normal, p.text_secondary, true);
            y += msg_h + 12.0;
            if let Some(text) = input {
                let id = Id::new("history-prompt-input");
                TextBox::default().show(&mut ui, Rect::from_min_size(pos2(c.min.x, y), vec2(c.width(), 40.0)), id, text);
            }
            let cancel = s::HistoryWindows_Cancel();
            let (ow, cw) = (b.width(&ui, confirm), b.width(&ui, cancel));
            let cr = Rect::from_min_size(pos2(c.max.x - cw, c.max.y - BUTTON_HEIGHT), vec2(cw, BUTTON_HEIGHT));
            let or = Rect::from_min_size(pos2(cr.min.x - 8.0 - ow, cr.min.y), vec2(ow, BUTTON_HEIGHT));
            if b.show(&ui, or, Id::new("history-prompt-ok"), confirm, true).clicked() {
                out = ModalOut::Confirm(action.clone(), input.clone().unwrap_or_default());
            }
            if b.show(&ui, cr, Id::new("history-prompt-cancel"), cancel, true).clicked() {
                out = ModalOut::Close;
            }
        }
    }
    out
}

/// Which strings a window's prompts use (the Image history window has its
/// own copies of the History window's).
pub struct PromptStrings {
    pub edit_tag: fn() -> &'static str,
    pub enter_tag: fn() -> &'static str,
    pub rename_file: fn() -> &'static str,
    pub enter_new_file_name: fn() -> &'static str,
    pub rename: fn() -> &'static str,
    pub delete: fn() -> &'static str,
    pub delete_files: fn() -> &'static str,
    pub delete_items: fn() -> &'static str,
    pub this_file: fn() -> &'static str,
    pub this_item: fn() -> &'static str,
    pub these_files: fn() -> &'static str,
    pub these_items: fn() -> &'static str,
    pub confirm_delete: fn() -> &'static str,
}

pub const HISTORY_PROMPTS: PromptStrings = PromptStrings {
    edit_tag: s::HistoryWindow_EditTag,
    enter_tag: s::HistoryWindow_EnterTagForHistoryItem,
    rename_file: s::HistoryWindow_RenameFile,
    enter_new_file_name: s::HistoryWindow_EnterNewFileName,
    rename: s::HistoryWindow_Rename,
    delete: s::HistoryWindow_Delete,
    delete_files: s::HistoryWindow_DeleteFiles,
    delete_items: s::HistoryWindow_DeleteHistoryItems,
    this_file: s::HistoryWindow_ThisFile,
    this_item: s::HistoryWindow_ThisItem,
    these_files: s::HistoryWindow_TheseFilesFormat,
    these_items: s::HistoryWindow_TheseItemsFormat,
    confirm_delete: s::HistoryWindow_ConfirmDeleteFormat,
};

pub const IMAGE_HISTORY_PROMPTS: PromptStrings = PromptStrings {
    edit_tag: s::ImageHistoryWindow_EditTag,
    enter_tag: s::ImageHistoryWindow_EnterTagForHistoryItem,
    rename_file: s::ImageHistoryWindow_RenameFile,
    enter_new_file_name: s::ImageHistoryWindow_EnterNewFileName,
    rename: s::ImageHistoryWindow_Rename,
    delete: s::ImageHistoryWindow_Delete,
    delete_files: s::ImageHistoryWindow_DeleteFiles,
    delete_items: s::ImageHistoryWindow_DeleteHistoryItems,
    this_file: s::ImageHistoryWindow_ThisFile,
    this_item: s::ImageHistoryWindow_ThisItem,
    these_files: s::ImageHistoryWindow_TheseFilesFormat,
    these_items: s::ImageHistoryWindow_TheseItemsFormat,
    confirm_delete: s::ImageHistoryWindow_ConfirmDeleteFormat,
};

/// What a command did to the window.
#[derive(Default)]
pub struct CmdOut {
    pub effects: Vec<Effect>,
    pub modal: Option<Modal>,
    /// Items were edited in place (refresh the view).
    pub edited: bool,
}

/// Runs `cmd` on the selected items (`selected`, the first the primary
/// one), as the windows' click handlers do. `filtered_files` is the image
/// viewer's file window around the primary item.
pub fn run_cmd(cmd: Cmd, items: &mut [HistoryItem], selected: &[usize], strings: &PromptStrings) -> CmdOut {
    let mut out = CmdOut::default();
    let sel: Vec<&HistoryItem> = selected.iter().filter_map(|&i| items.get(i)).collect();
    let Some(&primary) = sel.first() else { return out };
    let path = primary.file_path.clone().unwrap_or_default();
    let exists = file_exists(&path);
    let copy = |format: CopyFormat| actions::copy_text(format, &sel).map(Effect::CopyText);
    let open_url = |url: &Option<String>| url.as_deref().filter(|u| !u.trim().is_empty()).map(|u| Effect::OpenUrl(u.to_string()));
    use sharex_helpers::file_helpers::{is_image_file, is_text_file, is_video_file};
    match cmd {
        Cmd::OpenUrl => out.effects.extend(open_url(&primary.url)),
        Cmd::OpenShortUrl => out.effects.extend(open_url(&primary.shortened_url)),
        Cmd::OpenThumbnailUrl => out.effects.extend(open_url(&primary.thumbnail_url)),
        Cmd::OpenDeletionUrl => out.effects.extend(open_url(&primary.deletion_url)),
        Cmd::OpenFile if exists => out.effects.push(Effect::OpenFile(path.into())),
        Cmd::OpenFolder if exists => out.effects.push(Effect::OpenFolderWithFile(path.into())),
        Cmd::Copy(format) => out.effects.extend(copy(format)),
        Cmd::CopyFile => {
            let files: Vec<PathBuf> = sel.iter().filter_map(|i| i.file_path.as_deref()).filter(|p| file_exists(p)).map(PathBuf::from).collect();
            if !files.is_empty() {
                out.effects.push(Effect::CopyFiles(files));
            }
        }
        Cmd::CopyImage if exists && is_image_file(&path) => out.effects.push(Effect::CopyImageFile(path.into())),
        Cmd::CopyText if exists && is_text_file(&path) => {
            if let Ok(text) = std::fs::read_to_string(&path) {
                out.effects.push(Effect::CopyText(text));
            }
        }
        Cmd::Favorite => {
            for &i in selected {
                let it = &mut items[i];
                let fav = it.favorite();
                it.set_favorite(!fav);
                crate::history::edit(it);
            }
            out.edited = true;
        }
        Cmd::EditTag => {
            out.modal = Some(Modal::Prompt {
                title: (strings.edit_tag)().into(),
                message: (strings.enter_tag)().into(),
                confirm: s::HistoryWindows_Save().into(),
                input: Some(primary.tag_text().unwrap_or("").to_string()),
                action: PromptAction::EditTag(primary.id),
            })
        }
        Cmd::EditItem => out.modal = Some(Modal::edit(primary)),
        Cmd::Rename => {
            if !path.trim().is_empty() {
                out.modal = Some(Modal::Prompt {
                    title: (strings.rename_file)().into(),
                    message: (strings.enter_new_file_name)().into(),
                    confirm: (strings.rename)().into(),
                    input: Some(sharex_history::paths::file_name_without_extension(&path).to_string()),
                    action: PromptAction::Rename(primary.id),
                });
            }
        }
        Cmd::Delete | Cmd::DeleteFile => {
            let files = cmd == Cmd::DeleteFile;
            let n = sel.len();
            let noun = match (n == 1, files) {
                (true, true) => (strings.this_file)().to_string(),
                (true, false) => (strings.this_item)().to_string(),
                (false, true) => (strings.these_files)().replace("{0}", &n.to_string()),
                (false, false) => (strings.these_items)().replace("{0}", &n.to_string()),
            };
            out.modal = Some(Modal::Prompt {
                title: if files { (strings.delete_files)() } else { (strings.delete_items)() }.into(),
                message: (strings.confirm_delete)().replace("{0}", &noun),
                confirm: (strings.delete)().into(),
                input: None,
                action: PromptAction::Delete { ids: sel.iter().map(|i| i.id).collect(), files },
            });
        }
        Cmd::Preview | Cmd::OpenImage if exists && is_image_file(&path) => out.effects.push(Effect::FileService(FileService::ShowImage, path.into())),
        Cmd::Service(service) => {
            let ok = match service {
                FileService::UploadFile => exists,
                FileService::TrimVideo => sel.len() == 1 && exists && is_video_file(&path),
                FileService::TrimAnimatedGif => sel.len() == 1 && exists && sharex_history::paths::extension(&path).eq_ignore_ascii_case(".gif"),
                _ => exists && is_image_file(&path),
            };
            if ok {
                out.effects.push(Effect::FileService(service, path.into()));
            }
        }
        _ => {}
    }
    out
}

/// What confirming a prompt or saving the Edit dialog changed.
pub enum Applied {
    /// Items edited in place.
    Edited,
    /// Items deleted (their ids).
    Deleted(HashSet<i64>),
    Nothing,
}

/// `OnPromptConfirmClick`'s actions on `items`.
pub fn apply_prompt(action: PromptAction, input: &str, items: &mut [HistoryItem]) -> (Applied, Vec<Effect>) {
    let find = |items: &mut [HistoryItem], id: i64| items.iter().position(|i| i.id == id);
    match action {
        PromptAction::EditTag(id) => {
            if let Some(i) = find(items, id) {
                items[i].set_tag(Some(input));
                crate::history::edit(&items[i]);
                return (Applied::Edited, Vec::new());
            }
        }
        PromptAction::Rename(id) => {
            if let Some(i) = find(items, id) {
                let it = &mut items[i];
                let path = it.file_path.clone().unwrap_or_default();
                if let Some(new_name) = actions::rename_target(&path, input) {
                    // `FileHelpers.RenameFile`: the new path, or the old one
                    // when the rename fails.
                    let dir = std::path::Path::new(&path).parent().map(|d| d.to_path_buf()).unwrap_or_default();
                    let new_path = dir.join(&new_name);
                    let renamed = std::fs::rename(&path, &new_path).is_ok();
                    it.file_name = Some(new_name);
                    if renamed {
                        it.file_path = Some(new_path.to_string_lossy().into_owned());
                    }
                    crate::history::edit(it);
                    return (Applied::Edited, Vec::new());
                }
            }
        }
        PromptAction::Delete { ids, files } => {
            let ids: HashSet<i64> = ids.into_iter().collect();
            let doomed: Vec<HistoryItem> = items.iter().filter(|i| ids.contains(&i.id)).cloned().collect();
            if files {
                for it in &doomed {
                    if let Some(p) = it.file_path.as_deref().filter(|p| file_exists(p)) {
                        let _ = std::fs::remove_file(p);
                    }
                }
            }
            crate::history::delete(&doomed);
            return (Applied::Deleted(ids), Vec::new());
        }
        PromptAction::OpenFile(path) => return (Applied::Nothing, vec![Effect::OpenFile(path.into())]),
    }
    (Applied::Nothing, Vec::new())
}

/// `OnSaveEditClick`: the fields into the item, the tags parsed.
pub fn apply_edit(id: i64, fields: [String; 8], tags: &str, items: &mut [HistoryItem]) -> bool {
    let Some(it) = items.iter_mut().find(|i| i.id == id) else { return false };
    let [file_name, file_path, ty, host, url, thumb, deletion, short] = fields;
    it.file_name = Some(file_name);
    it.file_path = Some(file_path);
    it.r#type = Some(ty);
    it.host = Some(host);
    it.url = Some(url);
    it.thumbnail_url = Some(thumb);
    it.deletion_url = Some(deletion);
    it.shortened_url = Some(short);
    it.tags = Some(actions::parse_tags(tags));
    crate::history::edit(it);
    true
}

/// `ImportFolder` on a thread's worth of work: the new items appended;
/// returns how many, or the error message.
pub fn import(folder: &str, images_only: bool, skip_duplicates: bool, items: &[HistoryItem]) -> Result<usize, String> {
    let existing: HashSet<String> = if skip_duplicates {
        items.iter().filter_map(|i| i.file_path.clone()).filter(|p| !p.trim().is_empty()).collect()
    } else {
        HashSet::new()
    };
    let mut new = actions::import_folder(std::path::Path::new(folder), images_only, &existing, &sharex_history::SystemZone).map_err(|e| e.to_string())?;
    if !new.is_empty() {
        crate::history::append(&mut new);
    }
    Ok(new.len())
}
