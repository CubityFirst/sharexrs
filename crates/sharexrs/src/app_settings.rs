//! Application settings: `ShareX/Presentation/ApplicationSettings`
//! (`ApplicationSettingsWindow.axaml`, `ApplicationSettingsViewModel.cs`),
//! with its navigation and search (`ShareX.Avalonia/Controls/SettingsNavigation`,
//! `SettingsSearch`).
//!
//! Each page is written once, as calls on a [`Form`]: the same code lays
//! the page out and paints it, or only collects its text for the search
//! (`SettingsSearch.Apply`: a page matches its title and every card's text,
//! a card its own), so search and layout can't disagree.

use std::path::PathBuf;

use egui::{CornerRadius, Id, Rect, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};
use sharex_settings::{ApplicationConfig, HotkeyType, ImageContentAlignment, ProxyMethod, SupportedLanguage, ThumbnailTitleLocation};
use sharex_strings::sharex as s;
use sharex_ui::controls::{self, Button, ColorButton, ComboItem, NumericUpDown, TextBox};
use sharex_ui::keys::app_icons as icon;
use sharex_ui::theme::{density, fluent, palette};
use sharex_ui::widgets::{center_y, font, galley, line_height, text_width};
use sharex_helpers::code_menu;
use sharex_ui::menu::{Flyout, MenuEntry, MenuStyle, Placement};
use sharex_ui::{icons, menu::paint_accent_gradient};

use crate::appearance;
use crate::settings_form::{self, ComboSize, Mode, any_combo_open, matches, scroll_indicator};
use crate::startup;

/// A page of this window.
type Form<'u, 'a> = settings_form::Form<'u, 'a, ApplicationConfig, ApplicationSettings>;

/// `Height="720"`; the width is `ShareX.Density.Settings.WindowWidth`.
pub const HEIGHT: f32 = 720.0;
pub const MIN_HEIGHT: f32 = 600.0;
/// The navigation column (`ColumnDefinitions="220,*"`).
const NAV_WIDTH: f32 = 220.0;
/// Fluent's horizontal `ProgressBar` `MinWidth`.
const PROGRESS_MIN_WIDTH: f32 = 200.0;

/// What the window asks of the app.
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    /// A theme option changed (`ThemeManager` applies them as they change).
    Appearance,
    /// `MainWindowIntegration.SetTrayVisible`.
    ShowTray(bool),
    /// `ApplicationSettingsRuntime.UpdateTrayIcon`.
    TrayIcon,
    /// "Restart now".
    Restart,
    OpenFolder(PathBuf),
    OpenUrl(&'static str),
    /// A feature that isn't ported yet was asked for.
    NotPorted(&'static str),
    /// Image print settings (`ShowImagePrintSettings`).
    PrintSettings,
    /// `QuickTaskMenuEditorIntegration.Show`.
    QuickTaskMenuEditor,
    /// `OnExportClick`: the picker, then `ExportAsync` with the Settings and
    /// History check boxes.
    ExportSettings { settings: bool, history: bool },
    /// `OnImportClick`: the picker, then `ImportAsync`.
    ImportSettings,
    /// `ResetAsync`: the question, then the reset.
    ResetSettings,
    /// `Close()`: Escape (`OnWindowKeyDown`, ShareX #9304).
    Close,
}

/// The pages, in ShareX's order (`CreateNavigationItems`).
pub const PAGES: [&str; 12] =
    ["general", "theme", "integration", "paths", "settings", "main-window", "clipboard-formats", "upload", "history", "print", "proxy", "advanced"];

fn page_title(i: usize) -> (&'static str, &'static str) {
    match i {
        0 => (s::ApplicationSettingsWindow_General(), icon::Settings),
        1 => (s::ApplicationSettingsWindow_Theme(), icon::Palette),
        2 => (s::ApplicationSettingsWindow_Integration(), icon::Plug),
        3 => (s::ApplicationSettingsWindow_Paths(), icon::Folder),
        4 => (s::ApplicationSettingsWindow_Settings(), icon::DatabaseBackup),
        5 => (s::ApplicationSettingsWindow_MainWindow(), icon::Monitor),
        6 => (s::ApplicationSettingsWindow_ClipboardFormats(), icon::ClipboardList),
        7 => (s::ApplicationSettingsWindow_Upload(), icon::Upload),
        8 => (s::ApplicationSettingsWindow_History(), icon::History),
        9 => (s::ApplicationSettingsWindow_Print(), icon::Printer),
        10 => (s::ApplicationSettingsWindow_Proxy(), icon::Network),
        _ => (s::ApplicationSettingsWindow_Advanced(), icon::Sliders),
    }
}

/// The clipboard format editor overlay (`ClipboardFormatEditorOverlay`).
struct FormatEditor {
    /// The format being edited, `None` when adding.
    index: Option<usize>,
    description: String,
    format: String,
    focus: bool,
}

/// The text box a variable menu inserts into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeTarget {
    /// Paths › Sub folder pattern (`SaveImageSubFolderPatternTextBox`).
    SubFolder,
    /// Paths › Sub folder pattern for window captures.
    SubFolderWindow,
    /// The clipboard format editor's Format box (`ClipboardFormatTextBox`).
    ClipboardFormat,
}

/// An open variable menu (`AttachFilenamePatternMenu`, `AttachClipboardFormatMenu`).
pub struct CodeMenu {
    pub target: CodeTarget,
    pub flyout: Flyout<String>,
    /// The text box's id (its caret is where the variable goes).
    field: Id,
    /// Opened this frame: the press that opened it (a click whose press
    /// and release came in one frame) isn't a press outside it. Avalonia
    /// light-dismisses on the press and reopens on the release (SET-04a).
    just_opened: bool,
}

/// `CodeMenuEntryFilename`s each sub folder pattern box leaves out.
fn ignored_variables(target: CodeTarget) -> &'static [&'static str] {
    match target {
        CodeTarget::SubFolder => &["t", "pn", "i", "width", "height", "n"],
        CodeTarget::SubFolderWindow => &["i", "n"],
        CodeTarget::ClipboardFormat => &[],
    }
}

/// `ClipboardResultTokens`.
pub const CLIPBOARD_RESULT_TOKENS: [&str; 13] =
    ["$result", "$url", "$shorturl", "$thumbnailurl", "$deletionurl", "$filepath", "$filename", "$filenamenoext", "$thumbnailfilename", "$thumbnailfilenamenoext", "$folderpath", "$foldername", "$uploadtime"];

/// `AttachFilenamePatternMenu`'s items: "`%y` - Current year" by category
/// (a submenu each; `%n` has none), without the box's ignored entries.
pub fn filename_pattern_menu(target: CodeTarget) -> Vec<MenuEntry<String>> {
    let ignored = ignored_variables(target);
    let entries = code_menu::ALL.into_iter().filter(|e| !ignored.contains(&e.value));
    let item = |e: &code_menu::CodeMenuEntryFilename| MenuEntry::item(format!("{} - {}", e.to_prefix_string(), e.description()), "", e.to_prefix_string());
    let mut out = Vec::new();
    for (category, group) in code_menu::group_by_category(entries) {
        let items: Vec<_> = group.iter().map(item).collect();
        match category {
            Some(c) if !c.trim().is_empty() => out.push(MenuEntry::parent(c, "", items)),
            _ => out.extend(items),
        }
    }
    out
}

/// `AttachClipboardFormatMenu`'s items: Upload result's tokens, then every
/// name pattern variable by category, each item its token alone (the
/// description is its tooltip).
pub fn clipboard_format_menu() -> Vec<MenuEntry<String>> {
    let mut out = vec![MenuEntry::parent(s::ApplicationSettingsWindow_UploadResult(), "", CLIPBOARD_RESULT_TOKENS.iter().map(|t| MenuEntry::item(*t, "", t.to_string())).collect())];
    for (category, group) in code_menu::group_by_category(code_menu::ALL) {
        // `CreateClipboardTokenItem(token, description)`: the description is
        // the item's tooltip.
        let items: Vec<_> = group.iter().map(|e| MenuEntry::item(e.to_prefix_string(), "", e.to_prefix_string()).tooltip(e.description())).collect();
        match category {
            Some(c) if !c.trim().is_empty() => out.push(MenuEntry::parent(c, "", items)),
            _ => out.extend(items),
        }
    }
    out
}

/// Puts `token` into `text` at the caret of the text box `id`, the caret
/// after it, and focuses the box: `InsertText` (at the selection's start,
/// keeping it) or, with `replace_selection`, `InsertClipboardToken`.
pub fn insert_at_caret(ctx: &egui::Context, id: Id, text: &mut String, token: &str, replace_selection: bool) {
    let chars: Vec<char> = text.chars().collect();
    let mut state = egui::text_edit::TextEditState::load(ctx, id).unwrap_or_default();
    let (a, b) = state.cursor.char_range().map_or((chars.len(), chars.len()), |r| (r.primary.index.into(), r.secondary.index.into()));
    let start = a.min(b).min(chars.len());
    let end = if replace_selection { a.max(b).clamp(start, chars.len()) } else { start };
    let mut out: String = chars[..start].iter().collect();
    out.push_str(token);
    out.extend(&chars[end..]);
    *text = out;
    let caret = egui::text::CCursor::new(start + token.chars().count());
    state.cursor.set_char_range(Some(egui::text::CCursorRange::one(caret)));
    state.store(ctx, id);
    ctx.memory_mut(|m| m.request_focus(id));
}

/// The window's own state (the view model's fields that aren't settings).
pub struct ApplicationSettings {
    pub page: usize,
    pub search: String,
    scroll: [f32; 12],
    pub restart_required: bool,
    pub status: String,
    startup: Option<startup::StartupView>,
    personal_folder: String,
    personal_dirty: bool,
    /// The personal folder in use (`AppPaths.PersonalFolder`) and whether
    /// it's portable, for the previews.
    portable: bool,
    screenshots_parent: PathBuf,
    pub export_settings: bool,
    pub export_history: bool,
    /// `SelectedSpacingPreview`: 0 sidebar, 1 menus, 2 tool menus.
    spacing_preview: usize,
    clipboard_selected: Option<usize>,
    editor: Option<FormatEditor>,
    color_open: bool,
    /// The Explorer and browser integrations' check boxes
    /// (`RefreshIntegration`), in `integration::SWITCHES`' order.
    integrations: [bool; 6],
    /// Where they're read and changed (the system, or a test's).
    integration: Box<dyn crate::integration::IntegrationBackend>,
    /// `IsBusy`: an export, import or reset is running.
    pub busy: bool,
    /// When `busy` started (egui's clock), for the progress bar's animation;
    /// renders set the phase instead.
    busy_since: Option<f64>,
    pub busy_phase: Option<f64>,
    /// The open variable menu (`_activeCodeMenu`).
    pub code_menu: Option<CodeMenu>,
    /// A box focused again after an insertion: it doesn't open its menu
    /// (ShareX's menu never takes the focus, so `GotFocus` doesn't fire).
    refocused: Option<Id>,
    effects: Vec<Effect>,
}

impl ApplicationSettings {
    pub fn new(settings: &ApplicationConfig, portable: bool, screenshots_parent: PathBuf) -> Self {
        Self {
            page: 0,
            search: String::new(),
            scroll: [0.0; 12],
            restart_required: false,
            status: String::new(),
            startup: None,
            personal_folder: sharex_settings::paths::read_personal_path_config(),
            personal_dirty: false,
            portable,
            screenshots_parent,
            export_settings: true,
            export_history: true,
            spacing_preview: 0,
            clipboard_selected: (!settings.ClipboardContentFormats.is_empty()).then_some(0),
            editor: None,
            color_open: false,
            integrations: crate::integration::SWITCHES.map(crate::integration::check),
            integration: Box::new(crate::integration::SystemIntegration),
            busy: false,
            busy_since: None,
            busy_phase: None,
            code_menu: None,
            refocused: None,
            effects: Vec::new(),
        }
    }

    /// `Reload()` after an import or a reset: the personal folder read
    /// again, the integrations and startup state refreshed, the clipboard
    /// formats with the first selected, and the first page.
    pub fn reload(&mut self, settings: &ApplicationConfig) {
        self.personal_folder = sharex_settings::paths::read_personal_path_config();
        self.personal_dirty = false;
        self.refresh_integrations();
        self.startup = None;
        self.clipboard_selected = (!settings.ClipboardContentFormats.is_empty()).then_some(0);
        self.editor = None;
        self.page = 0;
    }

    /// Opens the clipboard format editor: Add, or Edit on format `index`.
    pub fn open_format_editor(&mut self, settings: &ApplicationConfig, index: Option<usize>) {
        let g = |i: usize, k: &str| settings.ClipboardContentFormats.get(i).and_then(|v| v.get(k)).and_then(|x| x.as_str()).unwrap_or_default().to_string();
        let (description, format) = index.map_or((String::new(), String::new()), |i| (g(i, "Description"), g(i, "Format")));
        self.editor = Some(FormatEditor { index, description, format, focus: true });
    }

    /// Scrolls page `page` to `y` (clamped when drawn); `f32::MAX` is the end.
    pub fn set_scroll(&mut self, page: usize, y: f32) {
        self.scroll[page] = y;
    }

    /// Uses `backend` for the integration check boxes, and reads them from it.
    pub fn with_integration(mut self, backend: Box<dyn crate::integration::IntegrationBackend>) -> Self {
        self.integration = backend;
        self.refresh_integrations();
        self
    }

    /// `RefreshIntegrations`.
    fn refresh_integrations(&mut self) {
        let backend = &self.integration;
        self.integrations = crate::integration::SWITCHES.map(|s| backend.check(s));
    }

    /// `IsBusy = true` with the status (`ExportingBackup`, `ImportingBackup`).
    pub fn start_busy(&mut self, status: &str) {
        self.busy = true;
        self.busy_since = None;
        self.status = status.to_owned();
    }

    /// `FlushPersonalPath` (on close): the personal folder is written to
    /// `PersonalPath.cfg` when it was edited.
    pub fn flush_personal_path(&mut self) {
        if !std::mem::take(&mut self.personal_dirty) {
            return;
        }
        match sharex_settings::paths::write_personal_path_config(&sharex_helpers::file_helpers::sanitize_path(&self.personal_folder)) {
            Ok(true) => self.restart_required = true,
            Ok(false) => {}
            Err(e) => self.status = e,
        }
    }

    /// The query's terms (`Split(' ', RemoveEmptyEntries | TrimEntries)`).
    fn terms(&self) -> Vec<String> {
        self.search.split(' ').map(|t| t.trim().to_lowercase()).filter(|t| !t.is_empty()).collect()
    }

    /// Draws the window over all of `ui`, changing `settings`; returns what
    /// the app should do.
    pub fn ui(&mut self, ui: &mut Ui, settings: &mut ApplicationConfig) -> Vec<Effect> {
        // `OnWindowKeyDown` (ShareX #9304): Escape closes the window; with
        // the clipboard format editor open it closes that instead.
        if self.editor.is_none() && self.code_menu.is_none() && !self.color_open && sharex_ui::popup::escape_pressed(ui.ctx()) {
            self.effects.push(Effect::Close);
        }
        let p = palette(ui.ctx());
        let full = ui.max_rect();
        ui.painter().rect_filled(full, 0.0, p.background_main);
        // Whether a combo box was open last frame (its dropdown takes the wheel).
        let open = ui.ctx().data_mut(|d| d.remove_temp::<bool>(Id::new("settings-any-combo-next"))).unwrap_or(false);
        ui.ctx().data_mut(|d| d.insert_temp(Id::new("settings-any-combo"), open));
        let terms = self.terms();
        // Which pages the search leaves (`SettingsNavigationItem.ApplyFilter`).
        let visible: Vec<bool> = (0..PAGES.len())
            .map(|i| {
                if terms.is_empty() {
                    return true;
                }
                let mut texts = Vec::new();
                self.run_page(ui, settings, i, Mode::Collect(&mut texts), Rect::NOTHING, &terms);
                matches(&texts.join(" "), &terms)
            })
            .collect();
        if !visible[self.page] && let Some(first) = visible.iter().position(|v| *v) {
            self.page = first;
        }
        self.navigation(ui, full, &visible);

        // The restart and status banners, then the page.
        let column = Rect::from_min_max(pos2(full.min.x + NAV_WIDTH, full.min.y), full.max);
        let mut top = column.min.y;
        if self.restart_required {
            top = self.banner(ui, column, top + 14.0, true);
        }
        if !self.status.trim().is_empty() {
            top = self.banner(ui, column, top + 10.0, false);
        }
        let view = Rect::from_min_max(pos2(column.min.x, top), column.max);
        if visible.iter().any(|v| *v) {
            let page = self.page;
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(view));
            child.set_clip_rect(view.intersect(ui.clip_rect()));
            let height = self.run_page(&mut child, settings, page, Mode::Paint, view, &terms);
            let max = (height - view.height()).max(0.0);
            if ui.rect_contains_pointer(view) && !self.color_open && self.editor.is_none() && !any_combo_open(ui.ctx()) {
                self.scroll[page] -= ui.input(|i| i.smooth_scroll_delta.y);
            }
            self.scroll[page] = self.scroll[page].clamp(0.0, max);
            if max > 0.0 {
                scroll_indicator(&child, view, self.scroll[page], height);
            }
        }
        if self.editor.is_some() {
            self.format_editor(ui, full, settings);
        }
        self.show_code_menu(ui, full, settings);
        std::mem::take(&mut self.effects)
    }

    /// Opens `target`'s menu (closing another one), unless it's open.
    /// `anchor` is the text box (`RightEdgeAlignedTop`); the clipboard
    /// format's context menu opens at `pointer`.
    pub fn open_code_menu(&mut self, target: CodeTarget, field: Id, anchor: Rect, pointer: Option<egui::Pos2>) {
        if self.code_menu.as_ref().is_some_and(|m| m.target == target) {
            return;
        }
        let (entries, placement) = match target {
            CodeTarget::ClipboardFormat => (clipboard_format_menu(), Placement::Pointer(pointer.unwrap_or(anchor.min))),
            t => (filename_pattern_menu(t), Placement::RightEdgeAlignedTop),
        };
        let mut flyout = Flyout::menu(Id::new(("code-menu", target as u8)), entries, anchor, placement);
        flyout.style = MenuStyle::Standard;
        self.code_menu = Some(CodeMenu { target, flyout, field, just_opened: true });
    }

    /// The open variable menu over the window: an item inserts its variable
    /// into its box; a click elsewhere or Escape closes it.
    fn show_code_menu(&mut self, ui: &mut Ui, full: Rect, settings: &mut ApplicationConfig) {
        let Some(menu) = self.code_menu.as_mut() else { return };
        let layer = egui::LayerId::new(egui::Order::Foreground, Id::new("code-menu-layer"));
        let child = ui.new_child(egui::UiBuilder::new().layer_id(layer).max_rect(full));
        let out = menu.flyout.show(&child, full);
        let just_opened = std::mem::take(&mut menu.just_opened);
        let pressed_outside = !just_opened && ui.input(|i| i.pointer.any_pressed()) && ui.input(|i| i.pointer.interact_pos()).is_some_and(|pos| !menu.flyout.contains(pos));
        if let Some(token) = out.command {
            let (target, field) = (menu.target, menu.field);
            match target {
                CodeTarget::SubFolder | CodeTarget::SubFolderWindow => {
                    let value = if target == CodeTarget::SubFolder { &mut settings.SaveImageSubFolderPattern } else { &mut settings.SaveImageSubFolderPatternWindow };
                    insert_at_caret(ui.ctx(), field, value, &token, false);
                    *value = sharex_helpers::file_helpers::sanitize_path(value);
                }
                CodeTarget::ClipboardFormat => {
                    if let Some(ed) = self.editor.as_mut() {
                        insert_at_caret(ui.ctx(), field, &mut ed.format, &token, true);
                    }
                }
            }
            self.refocused = Some(field);
            self.code_menu = None;
        } else if out.closed || pressed_outside || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.code_menu = None;
        }
    }

    /// The restart (accent border) or status banner at `y`; returns its bottom.
    fn banner(&mut self, ui: &Ui, column: Rect, y: f32, restart: bool) -> f32 {
        let p = palette(ui.ctx());
        let d = density(ui.ctx());
        let x0 = column.min.x + 20.0;
        let w = column.width() - 40.0;
        let body = font(13.0, 400);
        let lh = line_height(ui, &body);
        if restart {
            let button = Button::compact(d.settings_button_padding);
            let bw = button.width(ui, s::ApplicationSettingsWindow_RestartNow());
            let h = 1.0 + 8.0 + d.settings_control_min_height.max(lh).max(17.0) + 8.0 + 1.0;
            let r = Rect::from_min_size(pos2(x0, y), vec2(w, h));
            ui.painter().rect_filled(r, CornerRadius::same(5), p.background_panel);
            ui.painter().rect_stroke(r, CornerRadius::same(5), Stroke::new(1.0, p.accent_border), StrokeKind::Inside);
            let inner = Rect::from_min_max(pos2(r.min.x + 13.0, r.min.y + 9.0), pos2(r.max.x - 13.0, r.max.y - 9.0));
            icons::paint(ui.painter(), Rect::from_min_size(pos2(inner.min.x, center_y(inner, 17.0)), Vec2::splat(17.0)), icon::Reset, p.text);
            let text_rect = Rect::from_min_max(pos2(inner.min.x + 25.0, inner.min.y), pos2(inner.max.x - bw - 8.0, inner.max.y));
            controls::label(ui, text_rect, s::ApplicationSettingsWindow_RestartShareXToApplyThisChange(), body, p.text_secondary);
            let br = Rect::from_min_size(pos2(inner.max.x - bw, center_y(inner, d.settings_control_min_height)), vec2(bw, d.settings_control_min_height));
            if button.show(ui, br, Id::new("settings-restart"), s::ApplicationSettingsWindow_RestartNow(), true).clicked() {
                self.effects.push(Effect::Restart);
            }
            r.max.y
        } else {
            // `ColumnDefinitions="Auto,*" ColumnSpacing="8"`: the busy
            // `ProgressBar` (`Width="18" Height="18"`, which Fluent's
            // horizontal `MinWidth="200"` widens) then the wrapped text.
            let bar_w = if self.busy { PROGRESS_MIN_WIDTH } else { 0.0 };
            let text_w = w - 26.0 - bar_w - 8.0;
            let th = controls::wrapped(ui, pos2(0.0, 0.0), text_w, &self.status, body.clone(), p.text_secondary, false);
            let row = if self.busy { th.max(18.0) } else { th };
            let h = 1.0 + 8.0 + row + 8.0 + 1.0;
            let r = Rect::from_min_size(pos2(x0, y), vec2(w, h));
            ui.painter().rect_filled(r, CornerRadius::same(5), p.control_background);
            ui.painter().rect_stroke(r, CornerRadius::same(5), Stroke::new(1.0, p.border), StrokeKind::Inside);
            if self.busy {
                let now = ui.input(|i| i.time);
                let t = self.busy_phase.unwrap_or_else(|| now - *self.busy_since.get_or_insert(now));
                let bar = Rect::from_min_size(pos2(r.min.x + 13.0, center_y(Rect::from_min_size(pos2(r.min.x, r.min.y + 9.0), vec2(w, row)), 18.0)), vec2(bar_w, 18.0));
                controls::indeterminate_progress(ui.painter(), bar, t);
                if self.busy_phase.is_none() {
                    ui.ctx().request_repaint();
                }
            }
            controls::wrapped(ui, pos2(r.min.x + 13.0 + bar_w + 8.0, r.min.y + 9.0), text_w, &self.status, body, p.text_secondary, true);
            r.max.y
        }
    }

    /// `SettingsNavigation`: the search box, then the pages as a tree.
    fn navigation(&mut self, ui: &mut Ui, full: Rect, visible: &[bool]) {
        let p = palette(ui.ctx());
        let d = density(ui.ctx());
        let nav = Rect::from_min_size(full.min, vec2(NAV_WIDTH, full.height()));
        ui.painter().rect_filled(nav, 0.0, p.background_toolbar);
        ui.painter().rect_filled(Rect::from_min_max(pos2(nav.max.x - 1.0, nav.min.y), nav.max), 0.0, p.border);
        let inner = Rect::from_min_max(pos2(nav.min.x + 8.0, nav.min.y + 8.0), pos2(nav.max.x - 1.0 - 8.0, nav.max.y - 8.0));
        // The search box: `SearchPadding`, the 16px search icon 8px in.
        let font_h = line_height(ui, &TextBox::font());
        let search_h = (1.0 + d.settings_search_padding.top + font_h + d.settings_search_padding.bottom + 1.0).max(40.0);
        let search = Rect::from_min_size(inner.min, vec2(inner.width(), search_h));
        let tb = TextBox { padding: d.settings_search_padding, placeholder: sharex_strings::avalonia::SettingsNavigation_Search_settings(), ..Default::default() };
        tb.show(ui, search, Id::new("settings-search"), &mut self.search);
        icons::paint(ui.painter(), Rect::from_min_size(pos2(search.min.x + 8.0, center_y(search, 16.0)), Vec2::splat(16.0)), icon::Search, p.text);
        // The tree: items `MinHeight` NavItemMinHeight, `Margin="0,1"`; the
        // header after Fluent's hidden 12px chevron and its 12,0,12,0 margin.
        let mut y = search.max.y + d.settings_nav_spacing;
        let item_h = d.settings_nav_item_min_height.max(line_height(ui, &font(13.0, 400)) + d.settings_nav_item_margin.top + d.settings_nav_item_margin.bottom);
        for (i, shown) in visible.iter().enumerate() {
            if !shown {
                continue;
            }
            let r = Rect::from_min_size(pos2(inner.min.x, y + 1.0), vec2(inner.width(), item_h));
            y += item_h + 2.0;
            let resp = ui.interact(r, Id::new(("settings-nav", i)), egui::Sense::click());
            let selected = i == self.page;
            settings_form::nav_item_background(ui.painter(), r, &resp, selected);
            let (title, key) = page_title(i);
            let content = Rect::from_min_max(pos2(r.min.x + 36.0 + d.settings_nav_item_margin.left, r.min.y), pos2(r.max.x - d.settings_nav_item_margin.right, r.max.y));
            let fg = if selected { p.accent_foreground } else { p.text };
            let icon_color = if selected { p.accent_foreground } else { p.accent_icon };
            icons::paint(ui.painter(), Rect::from_min_size(pos2(content.min.x + 2.0, center_y(r, 16.0)), Vec2::splat(16.0)), key, icon_color);
            controls::label(ui, Rect::from_min_max(pos2(content.min.x + 26.0, r.min.y), pos2(content.max.x, r.max.y)), title, font(13.0, 400), fg);
            settings_form::nav_item_focus(ui.painter(), r, &resp);
            if resp.clicked() && !selected {
                self.page = i;
                if i == 2 {
                    // Integration refreshes "Run ShareX on startup".
                    self.startup = None;
                }
            }
        }
    }

    /// Lays out page `i` (painting it in `view`, or only collecting its
    /// text) and returns its height.
    fn run_page(&mut self, ui: &mut Ui, settings: &mut ApplicationConfig, i: usize, mode: Mode, view: Rect, terms: &[String]) -> f32 {
        let d = density(ui.ctx());
        let m = d.settings_page_margin;
        let top = view.min.y - self.scroll[i];
        let x = view.min.x + m.left;
        let width = view.width() - m.left - m.right;
        let (title, key) = page_title(i);
        let mut f = Form {
            ui,
            s: settings,
            w: self,
            paint: matches!(mode, Mode::Paint),
            collected: match mode {
                Mode::Collect(_) => Some(Vec::new()),
                Mode::Paint => None,
            },
            card_texts: None,
            terms: terms.to_vec(),
            page_title: title.to_string(),
            x,
            y: top + m.top,
            width,
            first: true,
            spacing: 0.0,
            d: d.clone(),
            page: i,
            enabled: true,
            style: settings_form::FormStyle::APPLICATION,
        };
        f.page_title(key, title);
        match i {
            0 => page_general(&mut f),
            1 => page_theme(&mut f),
            2 => page_integration(&mut f),
            3 => page_paths(&mut f),
            4 => page_settings(&mut f),
            5 => page_main_window(&mut f),
            6 => page_clipboard_formats(&mut f),
            7 => page_upload(&mut f),
            8 => page_history(&mut f),
            9 => page_print(&mut f),
            10 => page_proxy(&mut f),
            _ => page_advanced(&mut f),
        }
        let height = f.y + m.bottom - top;
        if let (Mode::Collect(out), Some(texts)) = (mode, f.collected.take()) {
            out.push(title.to_string());
            out.extend(texts);
        }
        height
    }

    /// The clipboard format editor: a modal card over the window.
    fn format_editor(&mut self, ui: &mut Ui, full: Rect, settings: &mut ApplicationConfig) {
        let p = palette(ui.ctx());
        let d = density(ui.ctx());
        let Some(ed) = self.editor.as_mut() else { return };
        // `Middle`, so the Format box's menu (an `Area` on `Foreground`) is on top.
        let layer = egui::LayerId::new(egui::Order::Middle, Id::new("format-editor"));
        let mut ui = ui.new_child(egui::UiBuilder::new().layer_id(layer).max_rect(full));
        ui.painter().rect_filled(full, 0.0, p.overlay_modal);
        let _ = ui.interact(full, Id::new("format-editor-block"), egui::Sense::click());
        let w = 640.0;
        let body = font(13.0, 400);
        let lh = line_height(&ui, &body);
        let hint = font(12.0, 400);
        let tokens = "$result, $url, $shorturl, $thumbnailurl, $deletionurl, $filepath, $filename, $filenamenoext, $thumbnailfilename, $thumbnailfilenamenoext, $folderpath, $foldername, $uploadtime";
        let vars = s::ClipboardFormatForm_ClipboardFormatForm_Supported_variables___0__and_other_variables_such_as__1__etc_()
            .replace("{0}", tokens)
            .replace("{1}", "%y, %mo, %d");
        let inner_w = w - 2.0 - 32.0 - 4.0;
        let vars_h = controls::wrapped(&ui, pos2(0.0, 0.0), inner_w, &vars, hint.clone(), p.text_secondary, false);
        let hint_lh = line_height(&ui, &hint);
        let title_h = line_height(&ui, &font(19.0, 600)) + 1.0 + hint_lh;
        let tb_h = (1.0 + 4.0 + line_height(&ui, &TextBox::font()) + 4.0 + 1.0).max(d.settings_control_min_height);
        let body_h = lh + 3.0 + tb_h + 8.0 + lh + 3.0 + 150.0 + 3.0 + hint_lh + 8.0 + vars_h;
        let h = (1.0 + 16.0 + title_h.max(21.0) + 12.0 + body_h + 12.0 + d.settings_control_min_height + 16.0 + 1.0).min(560.0);
        let c = Rect::from_center_size(full.center(), vec2(w, h));
        let card = Rect::from_min_size(pos2(c.min.x.round_ties_even(), c.min.y.round_ties_even()), c.size().round());
        ui.painter().rect_filled(card, CornerRadius::same(8), p.background_panel);
        ui.painter().rect_stroke(card, CornerRadius::same(8), Stroke::new(1.0, p.border), StrokeKind::Inside);
        let inner = card.shrink(17.0);
        let title = if ed.index.is_some() { s::ApplicationSettingsWindow_EditClipboardFormat() } else { s::ApplicationSettingsWindow_AddClipboardFormat() };
        let head = Rect::from_min_size(inner.min, vec2(inner.width(), title_h.max(21.0)));
        icons::paint(ui.painter(), Rect::from_min_size(pos2(head.min.x, center_y(head, 21.0)), Vec2::splat(21.0)), icon::ClipboardList, p.accent_icon);
        let ty = center_y(head, title_h);
        ui.painter().galley(pos2(head.min.x + 30.0, ty), galley(&ui, title, font(19.0, 600), p.text_secondary, f32::INFINITY), p.text_secondary);
        ui.painter().galley(pos2(head.min.x + 30.0, ty + title_h - hint_lh), galley(&ui, s::ApplicationSettingsWindow_CustomizeTheTextCopiedFromCompletedUploads(), hint.clone(), p.text_secondary, f32::INFINITY), p.text_secondary);
        let mut y = head.max.y + 12.0;
        ui.painter().galley(pos2(inner.min.x, y), galley(&ui, s::ApplicationSettingsWindow_Description(), body.clone(), p.text_secondary, f32::INFINITY), p.text_secondary);
        y += lh + 3.0;
        let tb = TextBox { padding: d.settings_control_padding, ..Default::default() };
        let desc_id = Id::new("format-description");
        tb.show(&mut ui, Rect::from_min_size(pos2(inner.min.x, y), vec2(inner_w, tb_h)), desc_id, &mut ed.description);
        if std::mem::take(&mut ed.focus) {
            ui.ctx().memory_mut(|m| m.request_focus(desc_id));
        }
        y += tb_h + 8.0;
        ui.painter().galley(pos2(inner.min.x, y), galley(&ui, s::ApplicationSettingsWindow_Format(), body, p.text_secondary, f32::INFINITY), p.text_secondary);
        y += lh + 3.0;
        let multi = TextBox { padding: d.settings_control_padding, multiline: true, ..Default::default() };
        let format_rect = Rect::from_min_size(pos2(inner.min.x, y), vec2(inner_w, 150.0));
        let format_id = Id::new("format-text");
        let _ = multi.show(&mut ui, format_rect, format_id, &mut ed.format);
        // `ClipboardFormatTextBox.ContextMenu`: a right click opens it at the
        // pointer. Read from the input rather than the text edit's response:
        // over the editor's blocker, egui never reports the box's secondary
        // click (SET-05a), so a press and a release of the right button in
        // the box (in one frame or two) make the click.
        let press_id = format_id.with("right-press");
        let (pressed, released, pos) = ui.input(|i| {
            let mut pressed = None;
            let mut released = None;
            for e in &i.events {
                if let egui::Event::PointerButton { pos, button: egui::PointerButton::Secondary, pressed: down, .. } = e {
                    if *down {
                        pressed = Some(*pos);
                    } else {
                        released = Some(*pos);
                    }
                }
            }
            (pressed, released, i.pointer.interact_pos())
        });
        if let Some(at) = pressed {
            ui.ctx().data_mut(|d| d.insert_temp(press_id, format_rect.contains(at)));
        }
        let context = match released {
            Some(at) if format_rect.contains(at) && ui.ctx().data(|d| d.get_temp::<bool>(press_id)).unwrap_or(false) => Some(pos.unwrap_or(at)),
            _ => None,
        };
        if released.is_some() {
            ui.ctx().data_mut(|d| d.remove::<bool>(press_id));
        }
        y += 150.0 + 3.0;
        ui.painter().galley(pos2(inner.min.x, y), galley(&ui, s::ApplicationSettingsWindow_RightClickToInsertAVariable(), hint.clone(), p.text_secondary, f32::INFINITY), p.text_secondary);
        y += hint_lh + 8.0;
        controls::wrapped(&ui, pos2(inner.min.x, y), inner_w, &vars, hint, p.text_secondary, true);
        // Save and Cancel, right-aligned, `MinWidth="84"`.
        let button = Button::compact(d.settings_button_padding);
        let bh = d.settings_control_min_height;
        let cw = button.width(&ui, s::ApplicationSettingsWindow_Cancel()).max(84.0);
        let sw = button.width(&ui, s::ApplicationSettingsWindow_Save()).max(84.0);
        let by = inner.max.y - bh;
        let cancel = Rect::from_min_size(pos2(inner.max.x - cw, by), vec2(cw, bh));
        let save = Rect::from_min_size(pos2(cancel.min.x - 8.0 - sw, by), vec2(sw, bh));
        let saved = button.show(&ui, save, Id::new("format-save"), s::ApplicationSettingsWindow_Save(), true).clicked();
        let menu_open = self.code_menu.is_some();
        let cancelled = button.show(&ui, cancel, Id::new("format-cancel"), s::ApplicationSettingsWindow_Cancel(), true).clicked()
            || (!menu_open && sharex_ui::popup::escape_pressed(ui.ctx()));
        if let Some(pos) = context {
            self.open_code_menu(CodeTarget::ClipboardFormat, format_id, format_rect, Some(pos));
        }
        if saved {
            let ed = self.editor.take().unwrap();
            let value = serde_json::json!({ "Description": ed.description, "Format": ed.format });
            match ed.index {
                Some(i) if i < settings.ClipboardContentFormats.len() => {
                    let item = &mut settings.ClipboardContentFormats[i];
                    item["Description"] = value["Description"].clone();
                    item["Format"] = value["Format"].clone();
                }
                _ => {
                    settings.ClipboardContentFormats.push(value);
                    self.clipboard_selected = Some(settings.ClipboardContentFormats.len() - 1);
                }
            }
        } else if cancelled {
            self.editor = None;
        }
    }
}

impl Form<'_, '_> {
    fn effect(&mut self, e: Effect) {
        if !self.w.effects.contains(&e) {
            self.w.effects.push(e);
        }
    }
}

/// `SetSetting` for a check box bound straight to a field.
macro_rules! check {
    ($f:expr, $text:expr, $($field:tt)+) => {{
        let mut v = $f.s.$($field)+;
        if $f.check($text, &mut v, true) {
            $f.s.$($field)+ = v;
            true
        } else {
            false
        }
    }};
}

/// An `i32` setting in a `NumericUpDown`.
macro_rules! number {
    ($f:expr, $label:expr, $w:expr, $min:expr, $max:expr, $($field:tt)+) => {{
        let mut v = $f.s.$($field)+ as i64;
        if $f.number($label, $w, $min, $max, &mut v, true, None) {
            $f.s.$($field)+ = v as i32;
        }
    }};
}

fn page_general(f: &mut Form) {
    f.card(4.0, &mut |f| {
        f.title(s::ApplicationSettingsWindow_Application());
        let items = crate::start_screen::language_items(f.ui.ctx());
        let current = SupportedLanguage::ALL.iter().position(|l| *l == f.s.Language);
        if let Some(i) = f.combo(s::ApplicationSettingsWindow_LanguageLabel(), 190.0, &items, current, ComboSize::default(), true) {
            // `SelectedLanguage`: the language applies after a restart.
            f.s.Language = SupportedLanguage::ALL[i].clone();
            if appearance::language_differs(&f.s.Language) {
                f.w.restart_required = true;
            }
        }
        if check!(f, s::ApplicationSettingsWindow_ShowTrayIcon(), ShowTray) {
            let v = f.s.ShowTray;
            f.effect(Effect::ShowTray(v));
        }
        let mut v = f.s.SilentRun;
        let tray = f.s.ShowTray;
        if f.check(s::ApplicationSettingsWindow_MinimizeToTrayOnStart(), &mut v, tray) {
            f.s.SilentRun = v;
        }
        check!(f, s::ApplicationSettingsWindow_ShowProgressInTrayIcon(), TrayIconProgressEnabled);
        let mut v = f.s.TaskbarProgressEnabled;
        if f.check(s::ApplicationSettingsWindow_ShowProgressInTaskbarButton(), &mut v, cfg!(windows)) {
            f.s.TaskbarProgressEnabled = v;
        }
        if check!(f, s::ApplicationSettingsWindow_UseWhiteShareXIcon(), UseWhiteShareXIcon) {
            f.effect(Effect::TrayIcon);
        }
        check!(f, s::ApplicationSettingsWindow_RememberMainWindowPosition(), RememberMainFormPosition);
        check!(f, s::ApplicationSettingsWindow_RememberMainWindowSize(), RememberMainFormSize);
    });
    f.card(6.0, &mut |f| {
        f.title(s::ApplicationSettingsWindow_TrayActions());
        let items: Vec<ComboItem> = HotkeyType::ALL.iter().map(|h| ComboItem::text(h.localized_description())).collect();
        let rows = [
            (s::ApplicationSettingsWindow_OnTrayIconDoubleLeftClickLabel(), 0),
            (s::ApplicationSettingsWindow_OnTrayIconLeftClickLabel(), 1),
            (s::ApplicationSettingsWindow_OnTrayIconMiddleClickLabel(), 2),
        ];
        for (label, k) in rows {
            let field = match k {
                0 => &mut f.s.TrayLeftDoubleClickAction,
                1 => &mut f.s.TrayLeftClickAction,
                _ => &mut f.s.TrayMiddleClickAction,
            };
            let current = HotkeyType::ALL.iter().position(|h| h == field);
            if let Some(i) = f.combo(label, 220.0, &items, current, ComboSize::default(), true) {
                let field = match k {
                    0 => &mut f.s.TrayLeftDoubleClickAction,
                    1 => &mut f.s.TrayLeftClickAction,
                    _ => &mut f.s.TrayMiddleClickAction,
                };
                *field = HotkeyType::ALL[i].clone();
            }
        }
        if f.button(s::ApplicationSettingsWindow_EditQuickTaskMenu(), true) {
            f.effect(Effect::QuickTaskMenuEditor);
        }
    });
    // Updates (`UpdatesVisible`: not with `DisableUpdateCheck`).
    f.card(6.0, &mut |f| {
        f.title(s::ApplicationSettingsWindow_Updates());
        check!(f, s::ApplicationSettingsWindow_AutomaticallyCheckForUpdates(), AutoCheckUpdate);
        let enabled = f.s.AutoCheckUpdate;
        let mut v = f.s.UpdateChannel.clone();
        if f.enum_combo(s::ApplicationSettingsWindow_UpdateChannelLabel(), 190.0, &mut v, enabled) {
            f.s.UpdateChannel = v;
        }
        let idle = !f.w.busy;
        if f.button(s::ApplicationSettingsWindow_InstallDevBuild(), idle) {
            f.effect(Effect::NotPorted("dev builds"));
        }
    });
}

fn page_theme(f: &mut Form) {
    f.card(6.0, &mut |f| {
        f.title(s::ApplicationSettingsWindow_ApplicationTheme());
        let mut o = f.s.ThemeOptions.clone().unwrap_or_default();
        if f.check(s::ApplicationSettingsWindow_FollowSystemTheme(), &mut o.UseSystemTheme, true) {
            f.s.theme_options().UseSystemTheme = o.UseSystemTheme;
            f.effect(Effect::Appearance);
        }
        if !o.UseSystemTheme {
            let items = [ComboItem::text(s::ApplicationSettingsWindow_Dark()), ComboItem::text(s::ApplicationSettingsWindow_Light())];
            let current = Some(usize::from(appearance::is_light(&o.Theme)));
            let size = ComboSize { width: Some(130.0), min_width: Some(130.0) };
            if let Some(i) = f.combo(s::ApplicationSettingsWindow_ThemeLabel(), 190.0, &items, current, size, true) {
                f.s.theme_options().Theme = if i == 1 { "Light" } else { "Dark" }.into();
                f.effect(Effect::Appearance);
            }
        }
        if f.check(s::ApplicationSettingsWindow_AlwaysExpandScrollBars(), &mut o.AlwaysExpandScrollBars, true) {
            f.s.theme_options().AlwaysExpandScrollBars = o.AlwaysExpandScrollBars;
        }
    });
    f.card(6.0, &mut |f| {
        f.title(s::ApplicationSettingsWindow_Spacing());
        let o = f.s.ThemeOptions.clone().unwrap_or_default();
        let mut compact = o.CompactMode;
        if f.check(s::ApplicationSettingsWindow_CompactMode(), &mut compact, true) {
            // Compact mode is a preset for the sliders: it drops their values.
            let t = f.s.theme_options();
            appearance::reset_spacing(t);
            t.CompactMode = compact;
            f.effect(Effect::Appearance);
        }
        f.stack(2.0, &mut |f| {
            let sp = appearance::spacing(&f.s.ThemeOptions.clone().unwrap_or_default());
            let rows = [
                (s::ApplicationSettingsWindow_SpacingSidebarLabel(), sp.sidebar, 0),
                (s::ApplicationSettingsWindow_SpacingMenusLabel(), sp.menus, 1),
                (s::ApplicationSettingsWindow_SpacingToolMenusLabel(), sp.tool_menus, 2),
                (s::ApplicationSettingsWindow_SpacingSettingsWindowsLabel(), sp.settings, 3),
            ];
            for (label, value, area) in rows {
                let mut v = value as i32;
                let text = match v {
                    0 => s::ApplicationSettingsWindow_SpacingCompact().to_string(),
                    100 => s::ApplicationSettingsWindow_SpacingDefault().to_string(),
                    n => format!("{n}%"),
                };
                if f.slider(label, &mut v, &text) {
                    let t = f.s.theme_options();
                    match area {
                        0 => t.SidebarSpacing = Some(v),
                        1 => t.MenuSpacing = Some(v),
                        2 => t.ToolMenuSpacing = Some(v),
                        _ => t.SettingsSpacing = Some(v),
                    }
                    // Show the part being adjusted.
                    if area < 3 {
                        f.w.spacing_preview = area;
                    }
                    f.effect(Effect::Appearance);
                }
            }
        });
        if f.button(s::ApplicationSettingsWindow_Reset(), true) {
            appearance::reset_spacing(f.s.theme_options());
            f.effect(Effect::Appearance);
        }
        let items = [
            ComboItem::text(s::ApplicationSettingsWindow_SpacingSidebar()),
            ComboItem::text(s::ApplicationSettingsWindow_SpacingMenus()),
            ComboItem::text(s::ApplicationSettingsWindow_SpacingToolMenus()),
        ];
        let size = ComboSize { width: None, min_width: Some(130.0) };
        let current = f.w.spacing_preview;
        if let Some(i) = f.combo(s::ApplicationSettingsWindow_SpacingPreviewLabel(), 190.0, &items, Some(current), size, true) {
            f.w.spacing_preview = i;
        }
        spacing_preview(f);
    });
    f.card(6.0, &mut |f| {
        f.title(s::ApplicationSettingsWindow_Icons());
        let items = [ComboItem::text(s::ApplicationSettingsWindow_IconPackModern()), ComboItem::text(s::ApplicationSettingsWindow_IconPackLegacy())];
        let legacy = f.s.ThemeOptions.as_ref().is_some_and(|o| o.IconPack == sharex_settings::IconPack::Legacy);
        let size = ComboSize { width: None, min_width: Some(130.0) };
        if let Some(i) = f.combo(s::ApplicationSettingsWindow_IconPackLabel(), 190.0, &items, Some(usize::from(legacy)), size, true) {
            f.s.theme_options().IconPack = if i == 1 { sharex_settings::IconPack::Legacy } else { sharex_settings::IconPack::Modern };
            f.effect(Effect::Appearance);
        }
    });
    f.card(6.0, &mut |f| {
        f.title(s::ApplicationSettingsWindow_AccentColor());
        let o = f.s.ThemeOptions.clone().unwrap_or_default();
        let mut follow = o.UseSystemAccentColor;
        if f.check(s::ApplicationSettingsWindow_FollowSystemAccentColor(), &mut follow, true) {
            f.s.theme_options().UseSystemAccentColor = follow;
            f.effect(Effect::Appearance);
        }
        if !follow {
            accent_row(f, &o);
        }
    });
}

/// "Accent color:" and the `ColorPickerDropdown` (`MinWidth="130"`).
fn accent_row(f: &mut Form, o: &sharex_settings::ApplicationThemeOptions) {
    let label = s::ApplicationSettingsWindow_AccentColorLabel();
    f.text(label);
    if f.collecting() {
        return;
    }
    let lh = line_height(f.ui, &Form::label_font());
    let r = f.slot(40f32.max(lh));
    let p = palette(f.ui.ctx());
    controls::label(f.ui, Rect::from_min_max(r.min, pos2(r.min.x + 190.0, r.max.y)), label, Form::label_font(), p.text_secondary);
    let rect = Rect::from_min_size(pos2(r.min.x + 198.0, center_y(r, 40.0)), vec2(130.0, 40.0));
    let accent = appearance::accent_color(o);
    let resp = ColorButton { enabled: true }.show(f.ui, rect, Id::new("settings-accent"), accent);
    let just_opened = resp.clicked() && !f.w.color_open;
    if resp.clicked() {
        f.w.color_open = !f.w.color_open;
    }
    if f.w.color_open {
        let mut c = appearance::accent_rgba(o);
        let (changed, keep) = controls::color_popup(f.ui, Id::new("settings-accent-popup"), rect, &mut c, just_opened);
        if changed {
            f.s.theme_options().AccentColorHex = sharex_ui::accent::to_hex(c);
            f.effect(Effect::Appearance);
        }
        f.w.color_open = keep;
    }
}

/// `SpacingPreview` in its `Border.spacing-preview`: part of the sidebar,
/// a menu or the tools menu, drawn as the real ones at the current spacing.
fn spacing_preview(f: &mut Form) {
    if f.collecting() {
        return;
    }
    let ctx = f.ui.ctx().clone();
    let d = density(&ctx);
    let p = palette(&ctx);
    let area = f.w.spacing_preview;
    let sections = crate::main_window::nav_sections(true);
    let visible: Vec<_> = sections.iter().filter(|s| s.visible).take(6).collect();
    // The sidebar part: 6 entries and the separator before the fifth.
    let content_h = match area {
        0 => d.nav_panel_margin.top + d.nav_panel_margin.bottom + 6.0 * d.nav_button_height + 5.0,
        1 => 7.0 * d.menu_item_height + d.menu_padding.top + d.menu_padding.bottom + 2.0 + 12.0,
        _ => 200.0,
    };
    let r = f.slot(1.0 + 12.0 + content_h + 12.0 + 1.0);
    f.ui.painter().rect_filled(r, CornerRadius::same(6), p.background_main);
    let inner = r.shrink(13.0);
    let painter = f.ui.painter().with_clip_rect(r.shrink(1.0));
    match area {
        0 => {
            let w = d.nav_min_width.max(160.0);
            let side = Rect::from_min_size(inner.min, vec2(w, content_h));
            painter.rect_filled(side, 0.0, p.background_toolbar);
            painter.rect_filled(Rect::from_min_max(pos2(side.max.x - 1.0, side.min.y), side.max), 0.0, p.border);
            let mut child = f.ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_max(
                pos2(side.min.x + d.nav_panel_margin.left, side.min.y + d.nav_panel_margin.top),
                pos2(side.max.x - 1.0 - d.nav_panel_margin.right, side.max.y),
            )));
            child.set_clip_rect(r.shrink(1.0));
            for (i, sec) in visible.iter().enumerate() {
                if i == 4 {
                    sharex_ui::widgets::separator(&mut child, 5.0, 2.0);
                }
                let _ = sharex_ui::widgets::nav_button(&mut child, sec.icon, sec.header, sec.has_children);
            }
        }
        _ => {
            // Menus and tool menus: the Capture menu's first entries in a
            // compact menu panel.
            let entries = crate::main_menu::capture_menu(&crate::main_menu::MenuState::default());
            let mut flyout = sharex_ui::menu::Flyout::menu(
                Id::new(("spacing-preview-menu", area)),
                entries.into_iter().take(7).collect(),
                Rect::from_min_size(inner.min, Vec2::ZERO),
                sharex_ui::menu::Placement::BottomEdgeAlignedLeft,
            );
            if area == 2 {
                let cats: Vec<_> = crate::main_menu::tool_categories().into_iter().take(2).collect();
                flyout = sharex_ui::menu::Flyout::grid(
                    Id::new("spacing-preview-tools"),
                    cats,
                    Rect::from_min_size(inner.min, Vec2::ZERO),
                    sharex_ui::menu::Placement::BottomEdgeAlignedLeft,
                );
            }
            let mut child = f.ui.new_child(egui::UiBuilder::new().max_rect(inner));
            child.set_clip_rect(r.shrink(1.0));
            let bounds = r.shrink(1.0);
            let _ = flyout.show(&child, bounds);
        }
    }
}

fn page_integration(f: &mut Form) {
    f.card(4.0, &mut |f| {
        f.title(s::ApplicationSettingsWindow_Windows());
        let view = f.w.startup.get_or_insert_with(startup::view);
        let (text, enabled, mut on) = (view.text, view.enabled, view.checked);
        if f.check(text, &mut on, enabled) {
            let state = if on { startup::StartupState::Enabled } else { startup::StartupState::Disabled };
            if let Err(e) = startup::set_state(state) {
                f.w.status = e;
            }
            f.w.startup = None;
        }
        let names = [
            s::ApplicationSettingsWindow_ShowUploadWithShareXButtonInWindowsExplorerContextMenu(),
            s::ApplicationSettingsWindow_ShowEditWithShareXButtonInWindowsExplorerContextMenu(),
            s::ApplicationSettingsWindow_ShowEditWithShareXPhotoEditorButtonInWindowsExplorerContextMenu(),
            s::ApplicationSettingsWindow_ShowShareXInSendToMenu(),
        ];
        for (i, name) in names.into_iter().enumerate() {
            let mut v = f.w.integrations[i];
            if f.check(name, &mut v, true) {
                f.w.integrations[i] = v;
                set_integration(f, i, v);
            }
        }
    });
    let browsers = [
        (s::ApplicationSettingsWindow_ChromeExtension(), s::ApplicationSettingsWindow_EnableChromeExtensionSupport(), s::ApplicationSettingsWindow_InstallShareXChromeExtension(), "https://chrome.google.com/webstore/detail/sharex/nlkoigbdolhchiicbonbihbphgamnaoc"),
        (s::ApplicationSettingsWindow_FirefoxAddon(), s::ApplicationSettingsWindow_EnableFirefoxAddonSupport(), s::ApplicationSettingsWindow_InstallShareXFirefoxAddon(), "https://addons.mozilla.org/en-US/firefox/addon/sharex/"),
    ];
    for (k, (title, check, button, url)) in browsers.into_iter().enumerate() {
        f.card(4.0, &mut |f| {
            f.title(title);
            let mut v = f.w.integrations[4 + k];
            if f.check(check, &mut v, true) {
                f.w.integrations[4 + k] = v;
                set_integration(f, 4 + k, v);
            }
            if f.button(button, f.w.integrations[4 + k]) {
                f.effect(Effect::OpenUrl(url));
            }
        });
    }
    // Steam (`SteamIntegrationVisible`): only in Steam builds.
}

/// `ShellContextMenu`, `EditWithShareX`, ... setters:
/// `IntegrationHelpers.Create*(value)`. Send To's error is shown.
fn set_integration(f: &mut Form, i: usize, on: bool) {
    if let Err(e) = f.w.integration.set(crate::integration::SWITCHES[i], on) {
        f.w.status = e;
    }
}

/// `PersonalFolderPreview`.
fn personal_folder_preview(w: &ApplicationSettings) -> String {
    let path = sharex_helpers::file_helpers::sanitize_path(&w.personal_folder);
    if path.is_empty() {
        if w.portable {
            sharex_settings::paths::absolute_path("ShareXrs").display().to_string()
        } else {
            sharex_settings::paths::default_personal_folder().display().to_string()
        }
    } else {
        sharex_settings::paths::absolute_path(&path).display().to_string()
    }
}

/// `ScreenshotsFolderPreview` (`TaskHelpers.GetScreenshotsFolder`).
fn screenshots_folder_preview(f: &Form) -> PathBuf {
    let parent = if f.s.UseCustomScreenshotsPath && !f.s.CustomScreenshotsPath.is_empty() {
        sharex_settings::paths::absolute_path(&f.s.CustomScreenshotsPath)
    } else {
        f.w.screenshots_parent.clone()
    };
    let mut parser = sharex_helpers::name_parser::NameParser::new(sharex_helpers::name_parser::NameParserType::FilePath);
    parent.join(parser.parse(&f.s.SaveImageSubFolderPattern))
}

fn page_paths(f: &mut Form) {
    f.card(6.0, &mut |f| {
        f.title(s::ApplicationSettingsWindow_ShareXPersonalFolder());
        let mut v = f.w.personal_folder.clone();
        let (changed, clicked) = f.text_with_buttons("personal", &mut v, &[s::ApplicationSettingsWindow_Browse(), s::ApplicationSettingsWindow_Open()]);
        if changed {
            f.w.personal_folder = v;
            f.w.personal_dirty = true;
        }
        match clicked {
            Some(0) => {
                if let Some(dir) = pick_folder(s::ApplicationSettingsWindow_ChooseShareXPersonalFolderPath()) {
                    f.w.personal_folder = dir.display().to_string();
                    f.w.personal_dirty = true;
                }
            }
            Some(_) => {
                let path = PathBuf::from(personal_folder_preview(f.w));
                f.effect(Effect::OpenFolder(path));
            }
            None => {}
        }
        let preview = personal_folder_preview(f.w);
        f.hint(&preview);
        f.hint(s::ApplicationSettingsWindow_ChangesToThePersonalFolderAreStoredImmediatelyAndTakeEffectAfterRestart());
    });
    f.card(6.0, &mut |f| {
        f.title(s::ApplicationSettingsWindow_ScreenshotsFolder());
        check!(f, s::ApplicationSettingsWindow_UseCustomScreenshotsFolder(), UseCustomScreenshotsPath);
        let mut v = f.s.CustomScreenshotsPath.clone();
        let (changed, clicked) = f.text_with_buttons("screenshots", &mut v, &[s::ApplicationSettingsWindow_Browse()]);
        if changed {
            f.s.CustomScreenshotsPath = sharex_helpers::file_helpers::sanitize_path(&v);
        }
        if clicked.is_some()
            && let Some(dir) = pick_folder(s::ApplicationSettingsWindow_ChooseScreenshotsFolderPath())
        {
            f.s.CustomScreenshotsPath = dir.display().to_string();
        }
        let mut v = f.s.SaveImageSubFolderPattern.clone();
        let (changed, tb) = f.text_row_with_box(Some((s::ApplicationSettingsWindow_SubFolderPatternLabel(), 210.0)), &mut v, true, false);
        if changed {
            f.s.SaveImageSubFolderPattern = sharex_helpers::file_helpers::sanitize_path(&v);
        }
        attach_pattern_menu(f, CodeTarget::SubFolder, tb);
        let mut v = f.s.SaveImageSubFolderPatternWindow.clone();
        let (changed, tb) = f.text_row_with_box(Some((s::ApplicationSettingsWindow_SubFolderPatternForWindowLabel(), 210.0)), &mut v, true, false);
        if changed {
            f.s.SaveImageSubFolderPatternWindow = sharex_helpers::file_helpers::sanitize_path(&v);
        }
        attach_pattern_menu(f, CodeTarget::SubFolderWindow, tb);
        // The preview and Open (`ColumnDefinitions="*,Auto"`).
        let preview = screenshots_folder_preview(f);
        let open = s::ApplicationSettingsWindow_Open();
        f.text(open);
        if !f.collecting() {
            let bh = f.d.settings_control_min_height;
            let button = Button::compact(f.d.settings_button_padding);
            let bw = button.width(f.ui, open);
            let hf = font(12.0, 400);
            let p = palette(f.ui.ctx());
            let text = preview.display().to_string();
            let tw = f.width - bw - 8.0;
            let th = controls::wrapped(f.ui, pos2(0.0, 0.0), tw, &text, hf.clone(), p.text_secondary, false);
            let r = f.slot(th.max(bh));
            controls::wrapped(f.ui, pos2(r.min.x, center_y(r, th)), tw, &text, hf, p.text_secondary, true);
            let br = Rect::from_min_size(pos2(r.max.x - bw, center_y(r, bh)), vec2(bw, bh));
            if button.show(f.ui, br, Id::new("settings-open-screenshots"), open, true).clicked() {
                f.effect(Effect::OpenFolder(preview));
            }
        }
    });
}

/// `AttachFilenamePatternMenu`: the box's menu opens when it gets the
/// focus or a pointer is released over it.
fn attach_pattern_menu(f: &mut Form, target: CodeTarget, tb: Option<(egui::Response, Rect, Id)>) {
    let Some((resp, rect, id)) = tb else { return };
    if resp.gained_focus() && f.w.refocused == Some(id) {
        f.w.refocused = None;
        return;
    }
    if resp.gained_focus() || resp.clicked() || resp.drag_stopped() {
        f.w.open_code_menu(target, id, rect, None);
    }
}

fn page_settings(f: &mut Form) {
    f.card(6.0, &mut |f| {
        f.title(s::ApplicationSettingsWindow_ExportAndImport());
        f.hint(s::ApplicationSettingsWindow_DoNotShareAnExportedBackupItCanContainPrivateInformationSuchAsAccountDetailsAndUploadHistory());
        let (mut a, mut b) = (f.w.export_settings, f.w.export_history);
        f.checks_row(&mut [(s::ApplicationSettingsWindow_Settings(), &mut a), (s::ApplicationSettingsWindow_History(), &mut b)], 8.0);
        (f.w.export_settings, f.w.export_history) = (a, b);
        // `CanExport => !IsBusy && (ExportSettings || ExportHistory)`.
        let idle = !f.w.busy;
        let can_export = idle && (a || b);
        let clicked = f.buttons(
            &[
                (s::ApplicationSettingsWindow_Export(), can_export),
                (s::ApplicationSettingsWindow_Import(), idle),
                (s::ApplicationSettingsWindow_ResetSettings(), idle),
            ],
            5.0,
        );
        match clicked {
            Some(0) => f.effect(Effect::ExportSettings { settings: a, history: b }),
            Some(1) => f.effect(Effect::ImportSettings),
            Some(_) => f.effect(Effect::ResetSettings),
            None => {}
        }
    });
    f.card(4.0, &mut |f| {
        f.title(s::ApplicationSettingsWindow_AutomaticCleanup());
        check!(f, s::ApplicationSettingsWindow_AutomaticallyCleanupOldBackupFiles(), AutoCleanupBackupFiles);
        check!(f, s::ApplicationSettingsWindow_AutomaticallyCleanupOldLogFiles(), AutoCleanupLogFiles);
        number!(f, s::ApplicationSettingsWindow_NumberOfFilesToKeepLabel(), 190.0, 1, 10000, CleanupKeepFileCount);
    });
}

fn page_main_window(f: &mut Form) {
    f.card(0.0, &mut |f| {
        let mut v = f.s.TaskViewMode.clone();
        if f.enum_combo(s::ApplicationSettingsWindow_TaskViewModeLabel(), 180.0, &mut v, true) {
            f.s.TaskViewMode = v;
        }
    });
    f.card(6.0, &mut |f| {
        f.title(s::ApplicationSettingsWindow_ListView());
        check!(f, s::ApplicationSettingsWindow_ShowColumns(), ShowColumns);
        let mut v = f.s.ImagePreview.clone();
        if f.enum_combo(s::ApplicationSettingsWindow_ImagePreviewLabel(), 180.0, &mut v, true) {
            f.s.ImagePreview = v;
        }
        let mut v = f.s.ImagePreviewLocation.clone();
        if f.enum_combo(s::ApplicationSettingsWindow_ImagePreviewLocationLabel(), 180.0, &mut v, true) {
            f.s.ImagePreviewLocation = v;
        }
    });
    f.card(6.0, &mut |f| {
        f.title(s::ApplicationSettingsWindow_ThumbnailView());
        check!(f, s::ApplicationSettingsWindow_ShowTitle(), ShowThumbnailTitle);
        let mut v: ThumbnailTitleLocation = f.s.ThumbnailTitleLocation.clone();
        if f.enum_combo(s::ApplicationSettingsWindow_TitleLocationLabel(), 180.0, &mut v, true) {
            f.s.ThumbnailTitleLocation = v;
        }
        thumbnail_size_row(f);
        let mut v = f.s.ThumbnailClickAction.clone();
        if f.enum_combo(s::ApplicationSettingsWindow_ThumbnailClickActionLabel(), 180.0, &mut v, true) {
            f.s.ThumbnailClickAction = v;
        }
    });
}

/// "Thumbnail size:" `ColumnDefinitions="180,130,Auto,130,Auto"`.
fn thumbnail_size_row(f: &mut Form) {
    let label = s::ApplicationSettingsWindow_ThumbnailSizeLabel();
    let reset = s::ApplicationSettingsWindow_Reset();
    f.text(label);
    f.text(reset);
    if f.collecting() {
        return;
    }
    let nh = f.d.settings_number_box_height;
    let r = f.slot(nh);
    let p = palette(f.ui.ctx());
    controls::label(f.ui, Rect::from_min_max(r.min, pos2(r.min.x + 180.0, r.max.y)), label, Form::label_font(), p.text_secondary);
    // Measured on ShareX: its grid puts the first box at 189 and gives
    // the "×" column 12px.
    let mut x = r.min.x + 189.0;
    let mut w = f.s.ThumbnailSize.width as i64;
    if (NumericUpDown { minimum: 50, maximum: 500, enabled: true }).show(f.ui, Rect::from_min_size(pos2(x, r.min.y), vec2(130.0, nh)), Id::new("thumb-w"), &mut w) {
        f.s.ThumbnailSize.width = w as i32;
    }
    x += 138.0;
    // "×": the global SemiBold `TextBlock`.
    let times = font(13.0, 600);
    let tw = text_width(f.ui, "×", times.clone());
    controls::label(f.ui, Rect::from_min_max(pos2(x + ((12.0 - tw) / 2.0).round(), r.min.y), pos2(x + 12.0, r.max.y)), "×", times, p.text_secondary);
    x += 12.0 + 8.0;
    let mut h = f.s.ThumbnailSize.height as i64;
    if (NumericUpDown { minimum: 50, maximum: 500, enabled: true }).show(f.ui, Rect::from_min_size(pos2(x, r.min.y), vec2(130.0, nh)), Id::new("thumb-h"), &mut h) {
        f.s.ThumbnailSize.height = h as i32;
    }
    x += 138.0;
    let button = Button::compact(f.d.settings_button_padding);
    let bh = f.d.settings_control_min_height;
    let bw = button.width(f.ui, reset);
    if button.show(f.ui, Rect::from_min_size(pos2(x, center_y(r, bh)), vec2(bw, bh)), Id::new("thumb-reset"), reset, true).clicked() {
        f.s.ThumbnailSize = sharex_settings::types::Size::new(200, 150);
    }
}

fn page_clipboard_formats(f: &mut Form) {
    // The hint above the card (`Margin="0,0,0,8"`), not part of a card.
    let hint = s::ApplicationSettingsWindow_TheseFormatsAppearUnderTheCopySubmenuInTheMainWindowContextMenu();
    f.text(hint);
    if !f.collecting() {
        let p = palette(f.ui.ctx());
        let hf = font(12.0, 400);
        let h = controls::wrapped(f.ui, pos2(f.x, f.y), f.width, hint, hf, p.text_secondary, true);
        f.y += h + 8.0;
    }
    f.card(5.0, &mut |f| {
        let formats: Vec<(String, String)> = f
            .s
            .ClipboardContentFormats
            .iter()
            .map(|v| {
                let g = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or_default().to_string();
                (g("Description"), g("Format"))
            })
            .collect();
        for (d, fm) in &formats {
            f.text(d);
            f.text(fm);
        }
        if !f.collecting() {
            clipboard_list(f, &formats);
        }
        let has = f.w.clipboard_selected.is_some_and(|i| i < formats.len());
        match f.buttons(
            &[(s::ApplicationSettingsWindow_Add(), true), (s::ApplicationSettingsWindow_Edit(), has), (s::ApplicationSettingsWindow_Remove(), has)],
            5.0,
        ) {
            Some(0) => f.w.editor = Some(FormatEditor { index: None, description: String::new(), format: String::new(), focus: true }),
            Some(1) => {
                let i = f.w.clipboard_selected.unwrap_or(0);
                let (d, fm) = formats[i].clone();
                f.w.editor = Some(FormatEditor { index: Some(i), description: d, format: fm, focus: true });
            }
            Some(_) => {
                // `RemoveSelectedClipboardFormat`: the next one is selected.
                if let Some(i) = f.w.clipboard_selected.filter(|&i| i < formats.len()) {
                    f.s.ClipboardContentFormats.remove(i);
                    let n = f.s.ClipboardContentFormats.len();
                    f.w.clipboard_selected = (n > 0).then(|| i.min(n - 1));
                }
            }
            None => {}
        }
    });
}

/// `ListBox.settings-list` (`MinHeight="260"`): each format's description
/// and, as a hint, its format; the selected row on the accent.
fn clipboard_list(f: &mut Form, formats: &[(String, String)]) {
    let p = palette(f.ui.ctx());
    let body = font(13.0, 400);
    let hint = font(12.0, 400);
    let row_h = 2.0 + line_height(f.ui, &body) + line_height(f.ui, &hint) + 2.0 + 18.0;
    let content = row_h * formats.len() as f32 + 2.0;
    let r = f.slot(content.max(260.0));
    f.ui.painter().rect_filled(r, CornerRadius::ZERO, p.background_main);
    f.ui.painter().rect_stroke(r, CornerRadius::ZERO, Stroke::new(1.0, p.border), StrokeKind::Inside);
    let f2 = fluent(f.ui.ctx());
    for (i, (d, fm)) in formats.iter().enumerate() {
        let row = Rect::from_min_size(pos2(r.min.x + 1.0, r.min.y + 1.0 + row_h * i as f32), vec2(r.width() - 2.0, row_h));
        let resp = f.ui.interact(row, Id::new(("clipboard-format", i)), egui::Sense::click());
        let selected = f.w.clipboard_selected == Some(i);
        if selected {
            paint_accent_gradient(f.ui, row, CornerRadius::same(3));
            f.ui.painter().rect_stroke(row, CornerRadius::same(3), Stroke::new(1.0, p.accent_border), StrokeKind::Inside);
        } else if resp.hovered() {
            f.ui.painter().rect_filled(row, CornerRadius::same(3), f2.list_low);
        }
        let fg = if selected { p.accent_foreground } else { p.text_secondary };
        let x = row.min.x + 12.0 + 4.0;
        let y = row.min.y + 9.0 + 2.0;
        f.ui.painter().galley(pos2(x, y), galley(f.ui, d, body.clone(), fg, row.max.x - x - 12.0), fg);
        let y2 = y + line_height(f.ui, &body);
        f.ui.painter().galley(pos2(x, y2), galley(f.ui, fm, hint.clone(), fg, row.max.x - x - 16.0), fg);
        if resp.clicked() {
            f.w.clipboard_selected = Some(i);
        }
        if resp.double_clicked() {
            f.w.editor = Some(FormatEditor { index: Some(i), description: d.clone(), format: fm.clone(), focus: true });
        }
    }
}

/// `BufferSizeOptions`: 2^0..2^13 KiB as `ToSizeString(binary, 0)`.
fn buffer_size_items(binary: bool) -> Vec<ComboItem> {
    (0..14)
        .map(|power| {
            let size = (2f64.powi(power) * 1024.0) as i64;
            ComboItem::text(size_string(size, binary))
        })
        .collect()
}

/// `NumberExtensions.ToSizeString(size, binary, 0)`.
fn size_string(size: i64, binary: bool) -> String {
    let base = if binary { 1024.0 } else { 1000.0 };
    if (size as f64) < base {
        return format!("{} B", size.max(0));
    }
    let place = (size as f64).log(base).floor() as i32;
    let num = size as f64 / base.powi(place);
    let suffixes = if binary { ["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB"] } else { ["B", "KB", "MB", "GB", "TB", "PB", "EB"] };
    // .NET's "0" format rounds half away from zero.
    format!("{} {}", num.round(), suffixes[place as usize])
}

fn page_upload(f: &mut Form) {
    f.card(6.0, &mut |f| {
        let mut v = f.s.UploadLimit as i64;
        if f.number(s::ApplicationSettingsWindow_SimultaneousUploadLimitLabel(), 240.0, 0, 25, &mut v, true, Some(s::ApplicationSettingsWindow_ZeroDisablesTheLimit())) {
            f.s.UploadLimit = v as i32;
        }
        let items = buffer_size_items(f.s.BinaryUnits);
        let current = (0..14).position(|p| p == f.s.BufferSizePower);
        if let Some(i) = f.combo(s::ApplicationSettingsWindow_BufferSizeLabel(), 240.0, &items, current, ComboSize::default(), true) {
            f.s.BufferSizePower = i as i32;
        }
        number!(f, s::ApplicationSettingsWindow_NumberOfTimesToRetryIfUploadFailsLabel(), 240.0, 0, 5, MaxUploadFailRetry);
    });
}

fn page_history(f: &mut Form) {
    f.card(4.0, &mut |f| {
        f.title(s::ApplicationSettingsWindow_History());
        check!(f, s::ApplicationSettingsWindow_SaveTasksToHistory(), HistorySaveTasks);
        check!(f, s::ApplicationSettingsWindow_OnlySaveIfUrlIsNotEmpty(), HistoryCheckURL);
        check!(f, s::ApplicationSettingsWindow_SaveOCRTextToHistory(), HistorySaveOCRText);
    });
    f.card(4.0, &mut |f| {
        f.title(s::ApplicationSettingsWindow_RecentTasks());
        check!(f, s::ApplicationSettingsWindow_SaveRecentTasks(), RecentTasksSave);
        number!(f, s::ApplicationSettingsWindow_MaximumNumberOfTasksToSaveLabel(), 230.0, 1, 10000, RecentTasksMaxCount);
        check!(f, s::ApplicationSettingsWindow_ShowRecentTasksInMainWindowOnStartup(), RecentTasksShowInMainWindow);
        check!(f, s::ApplicationSettingsWindow_ShowRecentTasksInTrayMenu(), RecentTasksShowInTrayMenu);
        check!(f, s::ApplicationSettingsWindow_InTrayMenuShowMostRecentTasksFirst(), RecentTasksTrayMenuMostRecentFirst);
    });
}

/// A string or bool in a `Value` object (`PrintSettings`, `ProxySettings`,
/// kept as JSON until their types are ported).
fn json_str(v: &serde_json::Value, key: &str) -> String {
    v.get(key).and_then(|x| x.as_str()).unwrap_or_default().to_string()
}

fn page_print(f: &mut Form) {
    f.card(6.0, &mut |f| {
        check!(f, s::ApplicationSettingsWindow_DontShowImagePrintSettingsDialog(), DontShowPrintSettingsDialog);
        if f.button(s::ApplicationSettingsWindow_ImagePrintSettings(), true) {
            f.effect(Effect::PrintSettings);
        }
        let show_dialog = f.s.PrintSettings.get("ShowPrintDialog").and_then(|x| x.as_bool()).unwrap_or(true);
        let mut dont = !show_dialog;
        if f.check(s::ApplicationSettingsWindow_DontShowWindowsPrintDialog(), &mut dont, true) {
            f.s.PrintSettings["ShowPrintDialog"] = serde_json::Value::Bool(!dont);
        }
        if dont {
            let mut v = json_str(&f.s.PrintSettings, "DefaultPrinterOverride");
            if f.text_row(Some((s::ApplicationSettingsWindow_DefaultPrinterOverrideLabel(), 190.0)), &mut v, true, false) {
                f.s.PrintSettings["DefaultPrinterOverride"] = serde_json::Value::String(v);
            }
        }
    });
}

fn page_proxy(f: &mut Form) {
    f.card(6.0, &mut |f| {
        let mut method = f
            .s
            .ProxySettings
            .get("ProxyMethod")
            .and_then(|x| serde_json::from_value::<ProxyMethod>(x.clone()).ok())
            .unwrap_or(ProxyMethod::Manual);
        if f.enum_combo(s::ApplicationSettingsWindow_ProxyConfigurationLabel(), 170.0, &mut method, true) {
            f.s.ProxySettings["ProxyMethod"] = serde_json::to_value(&method).unwrap_or_default();
        }
        let credentials = method != ProxyMethod::None;
        let manual = method == ProxyMethod::Manual;
        for (label, key, enabled, password) in [
            (s::ApplicationSettingsWindow_UsernameLabel(), "Username", credentials, false),
            (s::ApplicationSettingsWindow_PasswordLabel(), "Password", credentials, true),
            (s::ApplicationSettingsWindow_HostLabel(), "Host", manual, false),
        ] {
            let mut v = json_str(&f.s.ProxySettings, key);
            if f.text_row(Some((label, 170.0)), &mut v, enabled, password) {
                f.s.ProxySettings[key] = serde_json::Value::String(v);
            }
        }
        let mut port = f.s.ProxySettings.get("Port").and_then(|x| x.as_i64()).unwrap_or(0);
        if f.number(s::ApplicationSettingsWindow_PortLabel(), 170.0, 0, 65535, &mut port, manual, None) {
            f.s.ProxySettings["Port"] = serde_json::Value::from(port);
        }
    });
}

fn page_advanced(f: &mut Form) {
    f.card(6.0, &mut |f| {
        f.title(s::ApplicationSettingsWindow_AdvancedApplication());
        check!(f, s::ApplicationSettingsWindow_BinaryUnits(), BinaryUnits);
        check!(f, s::ApplicationSettingsWindow_ShowMostRecentTaskFirst(), ShowMostRecentTaskFirst);
        check!(f, s::ApplicationSettingsWindow_WorkflowsOnlyShowEdited(), WorkflowsOnlyShowEdited);
        check!(f, s::ApplicationSettingsWindow_TrayAutoExpandCaptureMenu(), TrayAutoExpandCaptureMenu);
        check!(f, s::ApplicationSettingsWindow_ShowFileDeleteConfirmation(), ShowFileDeleteConfirmation);
        let mut v = f.s.BrowserPath.clone();
        if f.text_row(Some((s::ApplicationSettingsWindow_BrowserPath(), 240.0)), &mut v, true, false) {
            f.s.BrowserPath = v;
        }
        check!(f, s::ApplicationSettingsWindow_SaveSettingsAfterTaskCompleted(), SaveSettingsAfterTaskCompleted);
        check!(f, s::ApplicationSettingsWindow_DevMode(), DevMode);
    });
    f.card(6.0, &mut |f| {
        f.title(s::ApplicationSettingsWindow_AdvancedHotkeys());
        check!(f, s::ApplicationSettingsWindow_DisableHotkeys(), DisableHotkeys);
        check!(f, s::ApplicationSettingsWindow_DisableHotkeysOnFullscreen(), DisableHotkeysOnFullscreen);
        number!(f, s::ApplicationSettingsWindow_HotkeyRepeatLimit(), 240.0, 200, i32::MAX as i64, HotkeyRepeatLimit);
    });
    f.card(6.0, &mut |f| {
        f.title(s::ApplicationSettingsWindow_AdvancedClipboard());
        check!(f, s::ApplicationSettingsWindow_ShowClipboardContentViewer(), ShowClipboardContentViewer);
        check!(f, s::ApplicationSettingsWindow_DefaultClipboardCopyImageFillBackground(), DefaultClipboardCopyImageFillBackground);
        check!(f, s::ApplicationSettingsWindow_UseAlternativeClipboardCopyImage(), UseAlternativeClipboardCopyImage);
        check!(f, s::ApplicationSettingsWindow_UseAlternativeClipboardGetImage(), UseAlternativeClipboardGetImage);
    });
    f.card(6.0, &mut |f| {
        f.title(s::ApplicationSettingsWindow_AdvancedImage());
        check!(f, s::ApplicationSettingsWindow_RotateImageByExifOrientationData(), RotateImageByExifOrientationData);
        check!(f, s::ApplicationSettingsWindow_PNGStripColorSpaceInformation(), PNGStripColorSpaceInformation);
    });
    f.card(6.0, &mut |f| {
        f.title(s::ApplicationSettingsWindow_AdvancedUpload());
        check!(f, s::ApplicationSettingsWindow_DisableUpload(), DisableUpload);
        check!(f, s::ApplicationSettingsWindow_URLEncodeIgnoreEmoji(), URLEncodeIgnoreEmoji);
        check!(f, s::ApplicationSettingsWindow_ShowMultiUploadWarning(), ShowMultiUploadWarning);
        number!(f, s::ApplicationSettingsWindow_ShowLargeFileSizeWarning(), 240.0, 0, i32::MAX as i64, ShowLargeFileSizeWarning);
    });
    f.card(6.0, &mut |f| {
        f.title(s::ApplicationSettingsWindow_AdvancedPaths());
        check!(f, s::ApplicationSettingsWindow_UseMachineSpecificUploadersConfig(), UseMachineSpecificUploadersConfig);
        for (label, k) in [
            (s::ApplicationSettingsWindow_CustomUploadersConfigPath(), 0),
            (s::ApplicationSettingsWindow_CustomHotkeysConfigPath(), 1),
            (s::ApplicationSettingsWindow_CustomScreenshotsPath2(), 2),
        ] {
            let field = match k {
                0 => &f.s.CustomUploadersConfigPath,
                1 => &f.s.CustomHotkeysConfigPath,
                _ => &f.s.CustomScreenshotsPath2,
            };
            let mut v = field.clone().unwrap_or_default();
            if f.text_row(Some((label, 240.0)), &mut v, true, false) {
                let field = match k {
                    0 => &mut f.s.CustomUploadersConfigPath,
                    1 => &mut f.s.CustomHotkeysConfigPath,
                    _ => &mut f.s.CustomScreenshotsPath2,
                };
                *field = Some(v);
            }
        }
    });
    f.card(6.0, &mut |f| {
        f.title(s::ApplicationSettingsWindow_AdvancedDragAndDropWindow());
        number!(f, s::ApplicationSettingsWindow_DropSize(), 240.0, 1, i32::MAX as i64, DropSize);
        number!(f, s::ApplicationSettingsWindow_DropOffset(), 240.0, i32::MIN as i64, i32::MAX as i64, DropOffset);
        let labels = [
            (ImageContentAlignment::TopLeft, s::ApplicationSettingsWindow_TopLeft()),
            (ImageContentAlignment::TopCenter, s::ApplicationSettingsWindow_TopCenter()),
            (ImageContentAlignment::TopRight, s::ApplicationSettingsWindow_TopRight()),
            (ImageContentAlignment::MiddleLeft, s::ApplicationSettingsWindow_MiddleLeft()),
            (ImageContentAlignment::MiddleCenter, s::ApplicationSettingsWindow_MiddleCenter()),
            (ImageContentAlignment::MiddleRight, s::ApplicationSettingsWindow_MiddleRight()),
            (ImageContentAlignment::BottomLeft, s::ApplicationSettingsWindow_BottomLeft()),
            (ImageContentAlignment::BottomCenter, s::ApplicationSettingsWindow_BottomCenter()),
            (ImageContentAlignment::BottomRight, s::ApplicationSettingsWindow_BottomRight()),
        ];
        let items: Vec<ComboItem> = labels.iter().map(|(_, t)| ComboItem::text(*t)).collect();
        let current = labels.iter().position(|(v, _)| *v == f.s.DropAlignment);
        if let Some(i) = f.combo(s::ApplicationSettingsWindow_DropAlignment(), 240.0, &items, current, ComboSize::default(), true) {
            f.s.DropAlignment = labels[i].0.clone();
        }
        number!(f, s::ApplicationSettingsWindow_DropOpacity(), 240.0, 0, 255, DropOpacity);
        number!(f, s::ApplicationSettingsWindow_DropHoverOpacity(), 240.0, 0, 255, DropHoverOpacity);
    });
}

/// `StorageProvider.OpenFolderPickerAsync`.
#[cfg(windows)]
pub(crate) fn pick_folder(title: &str) -> Option<PathBuf> {
    use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree};
    use windows::Win32::UI::Shell::{FOS_PICKFOLDERS, FileOpenDialog, IFileOpenDialog, SIGDN_FILESYSPATH};
    // SAFETY: COM calls on interfaces created here; the path string is
    // freed with CoTaskMemFree after copying.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let dialog: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER).ok()?;
        let options = dialog.GetOptions().ok()?;
        dialog.SetOptions(options | FOS_PICKFOLDERS).ok()?;
        dialog.SetTitle(&windows::core::HSTRING::from(title)).ok()?;
        dialog.Show(None).ok()?;
        let item = dialog.GetResult().ok()?;
        let path = item.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
        let s = path.to_string().ok();
        CoTaskMemFree(Some(path.0 as *const _));
        s.map(PathBuf::from)
    }
}

/// Avalonia's folder picker outside Windows: the system's (NSOpenPanel on
/// macOS, the XDG desktop portal on Linux), as `rfd` shows it.
#[cfg(not(windows))]
pub(crate) fn pick_folder(title: &str) -> Option<PathBuf> {
    rfd::FileDialog::new().set_title(title).pick_folder()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_as_sharex_writes_them() {
        assert_eq!(size_string(1024, true), "1 KiB");
        assert_eq!(size_string(32 * 1024, false), "33 KB");
        assert_eq!(size_string(8192 * 1024, true), "8 MiB");
    }

    /// The integration check boxes against a mock system.
    #[derive(Default)]
    struct Mock {
        on: std::rc::Rc<std::cell::RefCell<std::collections::BTreeMap<String, bool>>>,
        send_to_error: Option<String>,
    }

    impl crate::integration::IntegrationBackend for Mock {
        fn check(&self, switch: crate::integration::Switch) -> bool {
            self.on.borrow().get(&format!("{switch:?}")).copied().unwrap_or(false)
        }
        fn set(&mut self, switch: crate::integration::Switch, on: bool) -> Result<(), String> {
            if switch == crate::integration::Switch::SendToMenu
                && let Some(e) = &self.send_to_error
            {
                return Err(e.clone());
            }
            self.on.borrow_mut().insert(format!("{switch:?}"), on);
            Ok(())
        }
    }

    #[test]
    fn integration_boxes_read_and_change_the_backend() {
        use crate::integration::Switch;
        let state = std::rc::Rc::new(std::cell::RefCell::new(std::collections::BTreeMap::new()));
        state.borrow_mut().insert("FirefoxAddonSupport".to_string(), true);
        let mock = Mock { on: state.clone(), send_to_error: Some("Access denied".into()) };
        let mut settings = ApplicationConfig::default();
        let mut w = ApplicationSettings::new(&settings, true, PathBuf::from("shots")).with_integration(Box::new(mock));
        // `RefreshIntegrations` read the backend.
        assert_eq!(w.integrations, [false, false, false, false, false, true]);
        w.page = 2;
        let ctx = egui::Context::default();
        sharex_ui::install(&ctx);
        let mut boxes = Vec::new();
        // Lay the page out, then click each check box by its text.
        let names = [
            s::ApplicationSettingsWindow_ShowUploadWithShareXButtonInWindowsExplorerContextMenu(),
            s::ApplicationSettingsWindow_ShowShareXInSendToMenu(),
        ];
        let frame = |events: Vec<egui::Event>, w: &mut ApplicationSettings, settings: &mut ApplicationConfig| {
            let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(950.0, 720.0))), events, ..Default::default() };
            let mut out = ctx.run_ui(input, |ui| {
                let _ = w.ui(ui, settings);
            });
            out.textures_delta.clear();
            out.shapes
        };
        let shapes = frame(Vec::new(), &mut w, &mut settings);
        for name in names {
            let pos = shapes
                .iter()
                .find_map(|c| match &c.shape {
                    egui::Shape::Text(t) if t.galley.text() == name => Some(t.pos + vec2(-10.0, 8.0)),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("{name} not drawn"));
            boxes.push(pos);
        }
        for pos in boxes {
            let press = |pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
            frame(vec![egui::Event::PointerMoved(pos)], &mut w, &mut settings);
            frame(vec![press(true)], &mut w, &mut settings);
            frame(vec![press(false)], &mut w, &mut settings);
        }
        // The context menu box went to the backend; Send To's error is shown.
        assert_eq!(state.borrow().get(&format!("{:?}", Switch::ShellContextMenu)), Some(&true));
        assert_eq!(w.status, "Access denied");
    }

    #[test]
    fn variable_menus_as_sharex_builds_them() {
        let headers = |m: Vec<MenuEntry<String>>| m.iter().map(|e| e.header.clone()).collect::<Vec<_>>();
        assert_eq!(headers(filename_pattern_menu(CodeTarget::SubFolder)), ["Date and time", "Incremental", "Random", "Computer"]);
        assert_eq!(headers(filename_pattern_menu(CodeTarget::SubFolderWindow)), ["Window", "Date and time", "Incremental", "Random", "Image", "Computer"]);
        let clip = clipboard_format_menu();
        assert_eq!(headers(clip.clone()).first().map(String::as_str), Some("Upload result"));
        assert_eq!(clip.last().unwrap().header, "%n");
        let date = &filename_pattern_menu(CodeTarget::SubFolder)[0];
        assert_eq!(date.children.as_ref().unwrap()[0].header, "%y - Year");
        // `Incremental` without `%i` in the sub folder pattern's menu.
        let inc = &filename_pattern_menu(CodeTarget::SubFolder)[1];
        assert_eq!(inc.children.as_ref().unwrap()[0].command.as_deref(), Some("%ia"));
    }

    #[test]
    fn variables_go_in_at_the_caret() {
        let ctx = egui::Context::default();
        let id = Id::new("t");
        let mut state = egui::text_edit::TextEditState::default();
        state.cursor.set_char_range(Some(egui::text::CCursorRange::two(egui::text::CCursor::new(1), egui::text::CCursor::new(3))));
        state.clone().store(&ctx, id);
        // `InsertText` goes in at the selection's start and keeps it.
        let mut text = "abcd".to_string();
        insert_at_caret(&ctx, id, &mut text, "%y", false);
        assert_eq!(text, "a%ybcd");
        // `InsertClipboardToken` replaces the selection.
        state.store(&ctx, id);
        let mut text = "abcd".to_string();
        insert_at_caret(&ctx, id, &mut text, "$url", true);
        assert_eq!(text, "a$urld");
        let caret: usize = egui::text_edit::TextEditState::load(&ctx, id).unwrap().cursor.char_range().unwrap().primary.index.into();
        assert_eq!(caret, 5);
    }

    #[test]
    fn search_needs_every_term() {
        let terms = vec!["tray".to_string(), "icon".to_string()];
        assert!(matches("Show tray icon", &terms));
        assert!(!matches("Show tray", &terms));
    }
}

#[cfg(test)]
mod variable_menu_click_tests {
    use super::*;

    /// Application settings on `page` at `ppp` (with the clipboard format
    /// editor open for `clipboard-formats`), after these frames of input.
    fn after(page: &str, seq: &[Vec<egui::Event>], ppp: f32) -> ApplicationSettings {
        run(page, seq, ppp).0
    }

    /// [`after`], with how many times the window asked to close.
    fn run(page: &str, seq: &[Vec<egui::Event>], ppp: f32) -> (ApplicationSettings, usize) {
        let mut closes = 0;
        let ctx = egui::Context::default();
        sharex_ui::install(&ctx);
        let mut settings = ApplicationConfig::default();
        let mut state = ApplicationSettings::new(&settings, true, std::env::temp_dir()).with_integration(Box::new(crate::integration::NoIntegration));
        state.page = PAGES.iter().position(|p| *p == page).unwrap();
        if page == "clipboard-formats" {
            state.open_format_editor(&settings, None);
        }
        let mut t = 0.0;
        let mut frame = |events: Vec<egui::Event>| {
            t += 1.0 / 60.0;
            let mut input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(950.4, 720.0))), time: Some(t), events, ..Default::default() };
            input.viewports.entry(egui::ViewportId::ROOT).or_default().native_pixels_per_point = Some(ppp);
            let mut out = ctx.run_ui(input, |ui| {
                closes += state.ui(ui, &mut settings).iter().filter(|e| matches!(e, Effect::Close)).count();
            });
            out.textures_delta.clear();
        };
        for ev in std::iter::repeat_n(vec![], 3).chain(seq.iter().cloned()).chain(std::iter::repeat_n(vec![], 3)) {
            frame(ev);
        }
        (state, closes)
    }

    /// Whether the Sub folder pattern menu is open after these frames.
    fn menu_open_after(seq: &[Vec<egui::Event>], ppp: f32) -> bool {
        after("paths", seq, ppp).code_menu.as_ref().is_some_and(|m| m.target == CodeTarget::SubFolder)
    }

    #[test]
    fn escape_closes_the_variable_menu_before_the_format_editor() {
        // SET-05a: Clipboard formats › Add, a right-click in Format (in one
        // frame, as a synthetic click comes), then Escape: the menu goes,
        // the editor stays; a second Escape closes the editor.
        let p = egui::pos2(400.0, 360.0);
        let right = |pressed| egui::Event::PointerButton { pos: p, button: egui::PointerButton::Secondary, pressed, modifiers: Default::default() };
        let escape = egui::Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() };
        let click = vec![egui::Event::PointerMoved(p), right(true), right(false)];
        let separate = after("clipboard-formats", &[vec![egui::Event::PointerMoved(p)], vec![right(true)], vec![right(false)]], 1.25);
        assert!(separate.code_menu.as_ref().is_some_and(|m| m.target == CodeTarget::ClipboardFormat));
        let opened = after("clipboard-formats", std::slice::from_ref(&click), 1.25);
        assert!(opened.code_menu.as_ref().is_some_and(|m| m.target == CodeTarget::ClipboardFormat));
        let once = after("clipboard-formats", &[click.clone(), vec![escape.clone()]], 1.25);
        assert!(once.code_menu.is_none());
        assert!(once.editor.is_some());
        let (twice, closes) = run("clipboard-formats", &[click, vec![escape.clone()], vec![escape]], 1.25);
        assert!(twice.editor.is_none());
        // Neither Escape closed the window.
        assert_eq!(closes, 0);
    }

    #[test]
    fn escape_closes_the_window_once_nothing_else_takes_it() {
        // ShareX #9304: `OnWindowKeyDown` closes the window on Escape, after
        // the clipboard format editor.
        let escape = egui::Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() };
        assert_eq!(run("general", &[vec![escape.clone()]], 1.0).1, 1);
        let (editor_closed, closes) = run("clipboard-formats", &[vec![escape.clone()]], 1.0);
        assert!(editor_closed.editor.is_none());
        assert_eq!(closes, 0);
        assert_eq!(run("clipboard-formats", &[vec![escape.clone()], vec![escape]], 1.0).1, 1);
    }

    #[test]
    fn a_click_in_the_sub_folder_box_opens_its_menu() {
        // SET-04a: a quick click (press and release in one frame, as a
        // synthetic click often comes) opened the menu and closed it again
        // as a press outside it, at 100% and 125% alike.
        let p = egui::pos2(700.0, 352.0);
        let button = |pressed| egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        let moved = egui::Event::PointerMoved(p);
        for ppp in [1.0, 1.25] {
            assert!(menu_open_after(&[vec![moved.clone()], vec![button(true)], vec![button(false)]], ppp), "separate frames at {ppp}");
            assert!(menu_open_after(&[vec![moved.clone(), button(true), button(false)]], ppp), "one frame at {ppp}");
            assert!(menu_open_after(&[vec![moved.clone(), button(true)], vec![button(false)]], ppp), "move and press, then release at {ppp}");
        }
        // A press elsewhere still closes it.
        let away = egui::pos2(100.0, 650.0);
        let away_button = |pressed| egui::Event::PointerButton { pos: away, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        assert!(!menu_open_after(&[vec![moved.clone(), button(true), button(false)], vec![], vec![egui::Event::PointerMoved(away), away_button(true)], vec![away_button(false)]], 1.25));
    }
}
