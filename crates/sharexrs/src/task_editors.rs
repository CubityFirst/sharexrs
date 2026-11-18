//! Task settings' editor overlays for actions and watch folders
//! (`TaskSettingsWindow.axaml`: `ActionEditorOverlay`,
//! `WatchFolderEditorOverlay`; `TaskSettingsWindow.axaml.cs`:
//! `ShowActionEditor`, `OnSaveActionEditorClick`, `AttachActionArgumentsMenu`,
//! `ShowWatchFolderEditor`, `OnSaveWatchFolderEditorClick`): a card over the
//! dimmed window, Save and Cancel; Escape or a click on the backdrop cancels.

use egui::{CornerRadius, Id, Rect, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};
use sharex_settings::{CodeMenuEntryActions, ExternalProgram, WatchFolderSettings};
use sharex_strings::sharex as s;
use sharex_ui::controls::{self, Button, CheckBox, TextBox};
use sharex_ui::icons;
use sharex_ui::keys::app_icons as icon;
use sharex_ui::menu::{Flyout, MenuEntry, MenuStyle, Placement};
use sharex_ui::theme::{Density, density, palette};
use sharex_ui::widgets::{center_y, font, galley, line_height};

/// What an editor did this frame.
pub enum Outcome<T> {
    Open,
    Saved(T),
    Cancelled,
}

/// `ActionEditorOverlay`'s fields.
pub struct ActionEditor {
    /// The action edited (`_editedAction`), `None` when adding.
    pub index: Option<usize>,
    pub name: String,
    pub path: String,
    pub args: String,
    pub output_extension: String,
    pub extensions: String,
    pub hidden_window: bool,
    pub delete_input_file: bool,
    /// `ActionEditorErrorText`.
    pub error: Option<&'static str>,
    /// The box to focus (and select) next frame.
    focus: Option<Field>,
    /// The arguments box's context menu (`AttachActionArgumentsMenu`).
    menu: Option<Flyout<CodeMenuEntryActions>>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Field {
    Name,
    Path,
}

impl ActionEditor {
    /// `ShowActionEditor(action)`: the action's values, or a new action's.
    pub fn new(index: Option<usize>, action: Option<&ExternalProgram>) -> Self {
        let default = ExternalProgram::default();
        let v = action.unwrap_or(&default);
        let text = |s: &Option<String>| s.clone().unwrap_or_default();
        Self {
            index,
            name: text(&v.Name),
            path: text(&v.Path),
            args: text(&v.Args),
            output_extension: text(&v.OutputExtension),
            extensions: text(&v.Extensions),
            hidden_window: v.HiddenWindow,
            delete_input_file: v.DeleteInputFile,
            error: None,
            focus: Some(Field::Name),
            menu: None,
        }
    }

    /// `OnSaveActionEditorClick`: the name and path can't be blank; the
    /// edited action takes the values (a new one starts active).
    pub fn save(&mut self, actions: &mut Vec<ExternalProgram>) -> Option<usize> {
        if self.name.trim().is_empty() {
            self.error = Some(s::ActionsForm_btnOK_Click_Name_can_t_be_empty_());
            self.focus = Some(Field::Name);
            return None;
        }
        if self.path.trim().is_empty() {
            self.error = Some(s::ActionsForm_btnOK_Click_File_path_can_t_be_empty_());
            self.focus = Some(Field::Path);
            return None;
        }
        let index = match self.index.filter(|i| *i < actions.len()) {
            Some(i) => i,
            None => {
                actions.push(ExternalProgram { IsActive: true, ..Default::default() });
                actions.len() - 1
            }
        };
        let a = &mut actions[index];
        a.Name = Some(self.name.clone());
        a.Path = Some(self.path.clone());
        a.Args = Some(self.args.clone());
        a.OutputExtension = Some(self.output_extension.clone());
        a.Extensions = Some(self.extensions.clone());
        a.HiddenWindow = self.hidden_window;
        // `IsChecked == true`: a disabled box keeps its check.
        a.DeleteInputFile = self.delete_input_file;
        Some(index)
    }

    /// `InsertArgumentToken`: the token replaces the selection (or goes at
    /// the caret), and the caret follows it.
    fn insert_token(&mut self, ctx: &egui::Context, id: Id, token: &str) {
        let chars: Vec<char> = self.args.chars().collect();
        let mut state = egui::text_edit::TextEditState::load(ctx, id).unwrap_or_default();
        let (a, b) = state.cursor.char_range().map_or((chars.len(), chars.len()), |r| (r.primary.index.into(), r.secondary.index.into()));
        let start = a.min(b).min(chars.len());
        let end = a.max(b).clamp(start, chars.len());
        let mut out: String = chars[..start].iter().collect();
        out.push_str(token);
        out.extend(&chars[end..]);
        self.args = out;
        let caret = egui::text::CCursor::new(start + token.chars().count());
        state.cursor.set_char_range(Some(egui::text::CCursorRange::one(caret)));
        state.store(ctx, id);
        ctx.memory_mut(|m| m.request_focus(id));
    }

    /// Draws the editor over `full`.
    pub fn show(&mut self, ui: &mut Ui, full: Rect) -> Outcome<()> {
        let d = density(ui.ctx());
        let p = palette(ui.ctx());
        let (mut ui, backdrop) = overlay(ui, full, "action-editor");
        let m = Metrics::new(&ui, &d);
        let inner_w = 640.0 - 2.0 - 32.0;
        let body_w = inner_w - 4.0;
        let output_label = s::TaskSettingsWindow_OutputFileNameExtensionEmptyUsesTheSameFileName();
        let error_h = self.error.map_or(0.0, |e| 8.0 + controls::wrapped(&ui, pos2(0.0, 0.0), body_w, e, font(13.0, 400), p.cancel_border, false));
        let body_h = m.field() * 4.0 + m.path_field() + m.hint() + 8.0 * 5.0 + m.check_h + 2.0 + error_h;
        let card = card(&ui, full, 640.0, 590.0, m.header(), body_h, m.footer());
        let inner = card.shrink(17.0);
        let closed = header(&ui, inner, icon::Zap, if self.index.is_none() { s::TaskSettingsWindow_AddAction() } else { s::TaskSettingsWindow_EditAction() }, s::TaskSettingsWindow_RunAnExternalProgramAsPartOfACaptureWorkflow(), true, &m);
        let body = Rect::from_min_max(pos2(inner.min.x, inner.min.y + m.header() + 12.0), pos2(inner.min.x + body_w, inner.max.y - m.footer() - 12.0));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(body));
        child.set_clip_rect(body.intersect(ui.clip_rect()));
        let mut y = body.min.y;
        let focus = self.focus.take();
        y = text_field(&mut child, &m, body, y, s::TaskSettingsWindow_Name(), Id::new("action-name"), &mut self.name, focus == Some(Field::Name)) + 8.0;
        let (ny, browse) = path_field(&mut child, &m, body, y, s::TaskSettingsWindow_FilePath(), Id::new("action-path"), &mut self.path, focus == Some(Field::Path));
        y = ny + 8.0;
        if browse && let Some(file) = rfd::FileDialog::new().set_title(s::TaskSettingsWindow_ChooseProgram()).pick_file() {
            self.path = file.to_string_lossy().into_owned();
            self.error = None;
        }
        let args_id = Id::new("action-args");
        let args_top = y + m.label_h + 3.0;
        y = text_field(&mut child, &m, body, y, s::TaskSettingsWindow_Arguments(), args_id, &mut self.args, false);
        let args_rect = Rect::from_min_size(pos2(body.min.x, args_top), vec2(body_w, m.tb_h));
        y = hint(&child, body, y + 3.0, s::TaskSettingsWindow_RightClickToInsertAnInputOrOutputFileToken()) + 8.0;
        y = text_field(&mut child, &m, body, y, output_label, Id::new("action-output"), &mut self.output_extension, false) + 8.0;
        y = text_field(&mut child, &m, body, y, s::TaskSettingsWindow_ExtensionFilterExampleJpgPngMp4(), Id::new("action-extensions"), &mut self.extensions, false) + 8.0;
        // `StackPanel Orientation="Horizontal" Spacing="18"` of `CheckBox.setting`.
        let hidden_w = check(&child, &m, pos2(body.min.x, y + 1.0), s::TaskSettingsWindow_HiddenWindow(), &mut self.hidden_window, true);
        // `UpdateDeleteInputFileState`: only with an output extension.
        let can_delete = !self.output_extension.is_empty();
        check(&child, &m, pos2(body.min.x + hidden_w + 18.0, y + 1.0), s::TaskSettingsWindow_DeleteInputFile(), &mut self.delete_input_file, can_delete);
        y += m.check_h + 2.0;
        if let Some(e) = self.error {
            controls::wrapped(&child, pos2(body.min.x, y + 8.0), body_w, e, font(13.0, 400), p.cancel_border, true);
        }
        // The arguments box's context menu, at the pointer.
        let right_click = child.input(|i| i.pointer.secondary_clicked().then(|| i.pointer.interact_pos()).flatten());
        if self.menu.is_none()
            && let Some(pos) = right_click.filter(|pos| args_rect.contains(*pos))
        {
            let entries = CodeMenuEntryActions::ALL.iter().map(|e| MenuEntry::item(format!("{} — {}", e.to_prefix_string(), e.description()), "", *e)).collect();
            let mut fl = Flyout::menu(Id::new("action-args-menu"), entries, Rect::from_min_size(pos, Vec2::ZERO), Placement::Pointer(pos));
            fl.style = MenuStyle::Standard;
            self.menu = Some(fl);
            return Outcome::Open;
        }
        if let Some(menu) = self.menu.as_mut() {
            let out = menu.show(&ui, full);
            let outside = ui.input(|i| i.pointer.any_pressed()) && ui.input(|i| i.pointer.interact_pos()).is_some_and(|pos| !menu.contains(pos));
            if let Some(entry) = out.command {
                self.insert_token(ui.ctx(), args_id, &entry.to_prefix_string());
            }
            if out.closed || outside || out.command.is_some() {
                self.menu = None;
            }
            return Outcome::Open;
        }
        let (save, cancel) = footer(&ui, inner, &m, "action");
        if closed || cancel || backdrop.clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            return Outcome::Cancelled;
        }
        if save {
            return Outcome::Saved(());
        }
        Outcome::Open
    }
}

/// `WatchFolderEditorOverlay`'s fields.
pub struct WatchFolderEditor {
    /// The folder edited (`_editedWatchFolder`), `None` when adding.
    pub index: Option<usize>,
    pub folder_path: String,
    pub filter: String,
    pub include_subdirectories: bool,
    pub move_files_to_screenshots_folder: bool,
    focus: bool,
}

impl WatchFolderEditor {
    /// `ShowWatchFolderEditor(folder)`.
    pub fn new(index: Option<usize>, folder: Option<&WatchFolderSettings>) -> Self {
        Self {
            index,
            folder_path: folder.and_then(|f| f.FolderPath.clone()).unwrap_or_default(),
            filter: folder.and_then(|f| f.Filter.clone()).unwrap_or_default(),
            include_subdirectories: folder.is_some_and(|f| f.IncludeSubdirectories),
            move_files_to_screenshots_folder: folder.is_some_and(|f| f.MoveFilesToScreenshotsFolder),
            focus: true,
        }
    }

    /// `OnSaveWatchFolderEditorClick`: the folder takes the values (no
    /// checks); returns its index.
    pub fn save(&self, list: &mut Vec<WatchFolderSettings>) -> usize {
        let index = match self.index.filter(|i| *i < list.len()) {
            Some(i) => i,
            None => {
                list.push(WatchFolderSettings::default());
                list.len() - 1
            }
        };
        let f = &mut list[index];
        f.FolderPath = Some(self.folder_path.clone());
        f.Filter = Some(self.filter.clone());
        f.IncludeSubdirectories = self.include_subdirectories;
        f.MoveFilesToScreenshotsFolder = self.move_files_to_screenshots_folder;
        index
    }

    pub fn show(&mut self, ui: &mut Ui, full: Rect) -> Outcome<()> {
        let d = density(ui.ctx());
        let (mut ui, backdrop) = overlay(ui, full, "watch-editor");
        let m = Metrics::new(&ui, &d);
        let inner_w = 600.0 - 2.0 - 32.0;
        let body_w = inner_w - 4.0;
        let body_h = m.path_field() + 8.0 + m.field() + m.hint() + 8.0 + (m.check_h + 2.0) * 2.0 + 3.0;
        let card = card(&ui, full, 600.0, 480.0, m.header(), body_h, m.footer());
        let inner = card.shrink(17.0);
        let title = if self.index.is_none() { s::TaskSettingsWindow_AddWatchFolder() } else { s::TaskSettingsWindow_EditWatchFolder() };
        header(&ui, inner, icon::FolderSearch, title, s::TaskSettingsWindow_UploadFilesWhenTheyAreCreatedInThisFolder(), false, &m);
        let body = Rect::from_min_max(pos2(inner.min.x, inner.min.y + m.header() + 12.0), pos2(inner.min.x + body_w, inner.max.y - m.footer() - 12.0));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(body));
        child.set_clip_rect(body.intersect(ui.clip_rect()));
        let focus = std::mem::take(&mut self.focus);
        let (y, browse) = path_field(&mut child, &m, body, body.min.y, s::TaskSettingsWindow_FolderPath(), Id::new("watch-path"), &mut self.folder_path, focus);
        if browse && let Some(folder) = rfd::FileDialog::new().set_title(s::TaskSettingsWindow_ChooseWatchFolder()).pick_folder() {
            self.folder_path = folder.to_string_lossy().into_owned();
        }
        let y = text_field(&mut child, &m, body, y + 8.0, s::TaskSettingsWindow_Filter(), Id::new("watch-filter"), &mut self.filter, false);
        let y = hint(&child, body, y + 3.0, s::TaskSettingsWindow_ExamplePng()) + 8.0;
        // `StackPanel Spacing="3"` of two `CheckBox.setting`.
        check(&child, &m, pos2(body.min.x, y + 1.0), s::TaskSettingsWindow_IncludeSubdirectories(), &mut self.include_subdirectories, true);
        let y = y + m.check_h + 2.0 + 3.0;
        check(&child, &m, pos2(body.min.x, y + 1.0), s::TaskSettingsWindow_MoveFileToScreenshotsFolder(), &mut self.move_files_to_screenshots_folder, true);
        let (save, cancel) = footer(&ui, inner, &m, "watch");
        if cancel || backdrop.clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            return Outcome::Cancelled;
        }
        if save {
            return Outcome::Saved(());
        }
        Outcome::Open
    }
}

/// The sizes the editors lay out with.
struct Metrics {
    /// `TextBlock.action-editor-label` (13, Normal).
    label_h: f32,
    /// `TextBox.form-control`.
    tb_h: f32,
    /// `TextBlock.hint` (12, `Margin="0,4"`).
    hint_h: f32,
    title_h: f32,
    check_h: f32,
    button_h: f32,
    button_padding: sharex_ui::theme::Thickness,
}

impl Metrics {
    fn new(ui: &Ui, d: &Density) -> Self {
        let pad = d.settings_control_padding;
        Self {
            label_h: line_height(ui, &font(13.0, 400)),
            tb_h: (1.0 + pad.top + line_height(ui, &TextBox::font()) + pad.bottom + 1.0).max(d.settings_control_min_height),
            hint_h: line_height(ui, &font(12.0, 400)),
            title_h: line_height(ui, &font(19.0, 600)),
            check_h: d.settings_check_box_min_height,
            button_h: d.settings_control_min_height,
            button_padding: d.settings_button_padding,
        }
    }

    /// A label, 3, a text box.
    fn field(&self) -> f32 {
        self.label_h + 3.0 + self.tb_h
    }

    /// A label, 3, a text box with the 32x32 browse button.
    fn path_field(&self) -> f32 {
        self.label_h + 3.0 + self.tb_h.max(32.0)
    }

    /// 3, then a hint with its 4px margins.
    fn hint(&self) -> f32 {
        3.0 + 4.0 + self.hint_h + 4.0
    }

    /// The title (19, SemiBold) and the hint under it (`Spacing="1"`).
    fn header(&self) -> f32 {
        (self.title_h + 1.0 + 4.0 + self.hint_h + 4.0).max(21.0)
    }

    /// `Button.compact` with `Margin="0,4"`.
    fn footer(&self) -> f32 {
        self.button_h + 8.0
    }
}

/// The dimmed backdrop over the window (`ShareX.Brush.Overlay.Modal`), on
/// its own layer so nothing under it takes the pointer.
fn overlay(ui: &mut Ui, full: Rect, key: &str) -> (Ui, egui::Response) {
    let p = palette(ui.ctx());
    let layer = egui::LayerId::new(egui::Order::Foreground, Id::new(key));
    let child = ui.new_child(egui::UiBuilder::new().layer_id(layer).max_rect(full));
    child.painter().rect_filled(full, 0.0, p.overlay_modal);
    let backdrop = child.interact(full, Id::new((key, "backdrop")), egui::Sense::click());
    (child, backdrop)
}

/// The dialog `Border`: `width` wide, as tall as its rows (`MaxHeight`),
/// centred; padding 16, a 1px border, 8px corners.
fn card(ui: &Ui, full: Rect, width: f32, max_h: f32, header: f32, body: f32, footer: f32) -> Rect {
    let p = palette(ui.ctx());
    let h = (1.0 + 16.0 + header + 12.0 + body + 12.0 + footer + 16.0 + 1.0).min(max_h);
    let c = Rect::from_center_size(full.center(), vec2(width, h));
    let card = Rect::from_min_size(c.min.round(), c.size().round());
    // The card eats clicks, so only the backdrop itself cancels.
    let _ = ui.interact(card, Id::new(("editor-card", width as i32)), egui::Sense::click());
    ui.painter().rect_filled(card, CornerRadius::same(8), p.background_panel);
    ui.painter().rect_stroke(card, CornerRadius::same(8), Stroke::new(1.0, p.border), StrokeKind::Inside);
    card
}

/// The header row: a 21px accent icon, the title and its hint, and (the
/// action editor's) close button. Returns whether that was clicked.
fn header(ui: &Ui, inner: Rect, key: &str, title: &str, sub: &str, close: bool, m: &Metrics) -> bool {
    let p = palette(ui.ctx());
    let head = Rect::from_min_size(inner.min, vec2(inner.width(), m.header()));
    icons::paint(ui.painter(), Rect::from_min_size(pos2(head.min.x, center_y(head, 21.0)), Vec2::splat(21.0)), key, p.accent_icon);
    let x = head.min.x + 21.0 + 9.0;
    ui.painter().galley(pos2(x, head.min.y), galley(ui, title, font(19.0, 600), p.text_secondary, f32::INFINITY), p.text_secondary);
    let hy = head.min.y + m.title_h + 1.0 + 4.0;
    ui.painter().galley(pos2(x, hy), galley(ui, sub, font(12.0, 400), p.text_secondary, f32::INFINITY), p.text_secondary);
    if !close {
        return false;
    }
    // A plain `Button` (40 tall, `Padding="10,6"`) holding a 16px icon.
    let w = 1.0 + 10.0 + 16.0 + 10.0 + 1.0;
    let b = Rect::from_min_size(pos2(head.max.x - w, center_y(head, 40.0)), vec2(w, 40.0));
    let r = Button::default().show(ui, b, Id::new("editor-close"), "", true);
    let content = b.shrink(1.0);
    icons::paint(ui.painter(), Rect::from_min_size(pos2(content.min.x + 10.0, center_y(content, 16.0)), Vec2::splat(16.0)), icon::Close, p.text);
    r.on_hover_text(s::TaskSettingsWindow_Close()).clicked()
}

/// A label, then a stretching text box. Returns the y after it.
#[allow(clippy::too_many_arguments)]
fn text_field(ui: &mut Ui, m: &Metrics, body: Rect, y: f32, label: &str, id: Id, value: &mut String, focus: bool) -> f32 {
    let p = palette(ui.ctx());
    controls::label(ui, Rect::from_min_size(pos2(body.min.x, y), vec2(body.width(), m.label_h)), label, font(13.0, 400), p.text_secondary);
    let r = Rect::from_min_size(pos2(body.min.x, y + m.label_h + 3.0), vec2(body.width(), m.tb_h));
    text_box(ui, r, id, value, focus);
    r.max.y
}

/// A label, then a text box and the 32x32 browse button
/// (`ColumnDefinitions="*,32" ColumnSpacing="6"`). Returns the y after it
/// and whether the button was clicked.
#[allow(clippy::too_many_arguments)]
fn path_field(ui: &mut Ui, m: &Metrics, body: Rect, y: f32, label: &str, id: Id, value: &mut String, focus: bool) -> (f32, bool) {
    let p = palette(ui.ctx());
    controls::label(ui, Rect::from_min_size(pos2(body.min.x, y), vec2(body.width(), m.label_h)), label, font(13.0, 400), p.text_secondary);
    let row = Rect::from_min_size(pos2(body.min.x, y + m.label_h + 3.0), vec2(body.width(), m.tb_h.max(32.0)));
    let tb = Rect::from_min_size(pos2(row.min.x, center_y(row, m.tb_h)), vec2(row.width() - 6.0 - 32.0, m.tb_h));
    text_box(ui, tb, id, value, focus);
    let b = Rect::from_min_size(pos2(row.max.x - 32.0, center_y(row, 32.0)), Vec2::splat(32.0));
    let r = Button::compact(sharex_ui::theme::Thickness::hv(0.0, 0.0)).show(ui, b, id.with("browse"), "", true);
    let content = b.shrink(1.0);
    let ip = Rect::from_min_size(pos2(content.min.x + ((content.width() - 15.0) / 2.0).round_ties_even(), center_y(content, 15.0)), Vec2::splat(15.0));
    icons::paint(ui.painter(), ip, icon::Folder, p.text);
    (row.max.y, r.on_hover_text(s::TaskSettingsWindow_BrowseWithEllipsis()).clicked())
}

fn text_box(ui: &mut Ui, rect: Rect, id: Id, value: &mut String, focus: bool) {
    let d = density(ui.ctx());
    let tb = TextBox { padding: d.settings_control_padding, ..Default::default() };
    tb.show(ui, rect, id, value);
    if focus {
        // `Focus()` and `SelectAll()`.
        ui.ctx().memory_mut(|m| m.request_focus(id));
        let mut state = egui::text_edit::TextEditState::load(ui.ctx(), id).unwrap_or_default();
        let all = egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(value.chars().count()));
        state.cursor.set_char_range(Some(all));
        state.store(ui.ctx(), id);
    }
}

/// `TextBlock.hint` after `y` (its 4px top margin included). Returns the y
/// after its bottom margin.
fn hint(ui: &Ui, body: Rect, y: f32, text: &str) -> f32 {
    let p = palette(ui.ctx());
    let h = controls::wrapped(ui, pos2(body.min.x, y + 4.0), body.width(), text, font(12.0, 400), p.text_secondary, true);
    y + 4.0 + h + 4.0
}

/// A `CheckBox.setting` at `pos`; returns its width.
fn check(ui: &Ui, m: &Metrics, pos: egui::Pos2, text: &str, value: &mut bool, enabled: bool) -> f32 {
    let cb = CheckBox { min_height: m.check_h, enabled, text_font: controls::content_font(13.0, 400), ..CheckBox::new(text) };
    let w = cb.width(ui);
    cb.show(ui, Rect::from_min_size(pos, vec2(w, m.check_h)), Id::new(("editor-check", text)), value);
    w
}

/// Save and Cancel, right-aligned (`MinWidth="84"`, `Spacing="8"`).
fn footer(ui: &Ui, inner: Rect, m: &Metrics, key: &str) -> (bool, bool) {
    let button = Button::compact(m.button_padding);
    let cw = button.width(ui, s::TaskSettingsWindow_Cancel()).max(84.0);
    let sw = button.width(ui, s::TaskSettingsWindow_Save()).max(84.0);
    let by = inner.max.y - 4.0 - m.button_h;
    let cancel = Rect::from_min_size(pos2(inner.max.x - cw, by), vec2(cw, m.button_h));
    let save = Rect::from_min_size(pos2(cancel.min.x - 8.0 - sw, by), vec2(sw, m.button_h));
    let saved = button.show(ui, save, Id::new((key, "save")), s::TaskSettingsWindow_Save(), true).clicked();
    let cancelled = button.show(ui, cancel, Id::new((key, "cancel")), s::TaskSettingsWindow_Cancel(), true).clicked();
    (saved, cancelled)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_save_checks_name_and_path() {
        let mut list = Vec::new();
        let mut ed = ActionEditor::new(None, None);
        assert_eq!(ed.args, "\"$input\"");
        assert_eq!(ed.save(&mut list), None);
        assert_eq!(ed.error, Some(s::ActionsForm_btnOK_Click_Name_can_t_be_empty_()));
        ed.name = "Paint".into();
        assert_eq!(ed.save(&mut list), None);
        assert_eq!(ed.error, Some(s::ActionsForm_btnOK_Click_File_path_can_t_be_empty_()));
        ed.path = "mspaint.exe".into();
        assert_eq!(ed.save(&mut list), Some(0));
        let a = &list[0];
        assert!(a.IsActive);
        assert_eq!((a.Name.as_deref(), a.OutputExtension.as_deref()), (Some("Paint"), Some("")));
        // Editing keeps the action's state.
        list[0].IsActive = false;
        let mut ed = ActionEditor::new(Some(0), Some(&list[0]));
        ed.extensions = "png".into();
        assert_eq!(ed.save(&mut list), Some(0));
        assert_eq!((list.len(), list[0].IsActive, list[0].Extensions.as_deref()), (1, false, Some("png")));
    }

    #[test]
    fn insert_token_replaces_the_selection() {
        let ctx = egui::Context::default();
        let id = Id::new("args");
        let mut ed = ActionEditor::new(None, None);
        ed.args = "a XX b".into();
        let mut state = egui::text_edit::TextEditState::default();
        state.cursor.set_char_range(Some(egui::text::CCursorRange::two(egui::text::CCursor::new(4), egui::text::CCursor::new(2))));
        state.store(&ctx, id);
        ed.insert_token(&ctx, id, "$output");
        assert_eq!(ed.args, "a $output b");
        let caret: usize = egui::text_edit::TextEditState::load(&ctx, id).unwrap().cursor.char_range().unwrap().primary.index.into();
        assert_eq!(caret, 9usize);
    }

    #[test]
    fn watch_folder_save() {
        let mut list = Vec::new();
        let mut ed = WatchFolderEditor::new(None, None);
        ed.folder_path = "C:\\in".into();
        ed.include_subdirectories = true;
        assert_eq!(ed.save(&mut list), 0);
        assert_eq!(list[0].Filter.as_deref(), Some(""));
        let mut ed = WatchFolderEditor::new(Some(0), Some(&list[0]));
        ed.filter = "*.png".into();
        assert_eq!(ed.save(&mut list), 0);
        assert_eq!((list.len(), list[0].Filter.as_deref(), list[0].IncludeSubdirectories), (1, Some("*.png"), true));
    }
}
