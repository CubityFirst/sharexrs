//! `ShareX/Presentation/FileExist`: `FileExistWindow`, which
//! `TaskHelpers.HandleExistsFile` shows when a file a task saves already
//! exists and `FileExistAction` is Ask. 580 wide, as tall as its content,
//! topmost; the task's thread waits for the path it answers (empty:
//! cancelled).

use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;

use egui::{Id, Rect, Vec2, pos2, vec2};
use sharex_strings::sharex as s;
use sharex_ui::controls::{Button, TextBox, card};
use sharex_ui::keys::app_icons as icon;
use sharex_ui::theme::{Thickness, palette};
use sharex_ui::widgets::{center_y, font, galley, line_height, mono};
use sharex_ui::icons;

use super::layout;
use super::{Cx, Dialog, Effect, Spec};

const WIDTH: f32 = 580.0;
/// The body grid's `Margin="18,14,18,18"` and `RowSpacing="8"`.
const MARGIN: (f32, f32, f32) = (18.0, 14.0, 18.0);
const ROW_SPACING: f32 = 8.0;
/// `Button.action-button`: `Height="52"`, `Padding="11,6"`.
const ACTION_HEIGHT: f32 = 52.0;
const ACTION_PADDING: Thickness = Thickness { left: 11.0, top: 6.0, right: 11.0, bottom: 6.0 };
/// The new name `TextBox`: `MinHeight="32" Padding="8,5"`.
const TEXT_BOX_MIN_HEIGHT: f32 = 32.0;

pub struct FileExistWindow {
    /// `FilePath`: what the window answers.
    pub file_path: PathBuf,
    original_file_name: String,
    unique_file_path: PathBuf,
    pub new_name: String,
    reply: Option<Sender<Option<PathBuf>>>,
    opened: bool,
    /// `OnOpened`'s `SelectAll` (off for a `--render` of a typed name).
    pub select_on_open: bool,
}

impl FileExistWindow {
    pub fn new(file_path: &Path, reply: Option<Sender<Option<PathBuf>>>) -> Self {
        let original = file_path.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        Self {
            file_path: file_path.to_path_buf(),
            new_name: original.clone(),
            original_file_name: original,
            unique_file_path: sharex_helpers::file_helpers::unique_file_path(file_path),
            reply,
            opened: false,
            select_on_open: true,
        }
    }

    /// For `--render`: the unique name as if the file were there.
    pub fn assume_exists(&mut self) {
        if !self.file_path.exists() {
            let stem = self.file_path.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let ext = self.file_path.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
            self.unique_file_path = self.file_path.with_file_name(format!("{stem} (2){ext}"));
        }
    }

    /// `GetNewFileName`: the name and the file's extension, or nothing.
    pub fn new_file_name(&self) -> String {
        if self.new_name.is_empty() {
            return String::new();
        }
        match self.file_path.extension() {
            Some(e) => format!("{}.{}", self.new_name, e.to_string_lossy()),
            None => self.new_name.clone(),
        }
    }

    /// `NewNameButton.IsEnabled`: a name that isn't the file's own
    /// (`OrdinalIgnoreCase`).
    pub fn new_name_enabled(&self) -> bool {
        !self.new_name.is_empty() && !eq_ignore_case(&self.new_name, &self.original_file_name)
    }

    fn answer(&mut self, path: Option<PathBuf>) -> Vec<Effect> {
        if let Some(r) = self.reply.take() {
            let _ = r.send(path);
        }
        vec![Effect::Close]
    }

    /// `UseNewFileName`.
    fn use_new_name(&mut self) -> Vec<Effect> {
        let name = self.new_file_name();
        if name.is_empty() {
            return Vec::new();
        }
        let path = self.file_path.parent().map(|d| d.join(&name)).unwrap_or_else(|| PathBuf::from(&name));
        self.answer(Some(path))
    }

    fn label_font() -> egui::FontId {
        font(13.0, 600)
    }

    /// `TextBlock.action-file` sets no weight: `Button TextBlock`'s Normal.
    fn file_font() -> egui::FontId {
        font(12.0, 400)
    }

    fn path_font() -> egui::FontId {
        mono(13.0)
    }

    fn text_box_height(ui: &egui::Ui) -> f32 {
        (line_height(ui, &TextBox::font()) + 10.0 + 2.0).max(TEXT_BOX_MIN_HEIGHT)
    }

    fn path_card_height(ui: &egui::Ui) -> f32 {
        2.0 + 16.0 + line_height(ui, &Self::path_font()).max(16.0)
    }

    fn body_height(ui: &egui::Ui) -> f32 {
        let name = line_height(ui, &Self::label_font()) + 4.0 + Self::text_box_height(ui);
        MARGIN.1 + Self::path_card_height(ui) + ROW_SPACING + name + (ROW_SPACING + ACTION_HEIGHT) * 4.0 + MARGIN.2
    }
}

fn eq_ignore_case(a: &str, b: &str) -> bool {
    a.chars().flat_map(char::to_uppercase).eq(b.chars().flat_map(char::to_uppercase))
}

/// A `Button.action-button`: an icon, then a title and (optionally) a file
/// name under it in a `StackPanel` with `Spacing="1"`.
#[allow(clippy::too_many_arguments)]
fn action_button(ui: &egui::Ui, rect: Rect, id: Id, key: &str, accent: bool, title: &str, file: Option<&str>, enabled: bool) -> egui::Response {
    let p = palette(ui.ctx());
    let b = Button { padding: ACTION_PADDING, ..Button::default() };
    let r = b.show(ui, rect, id, "", enabled);
    let behind = p.background_main;
    let inner = Rect::from_min_max(
        pos2(rect.min.x + 1.0 + ACTION_PADDING.left, rect.min.y + 1.0 + ACTION_PADDING.top),
        pos2(rect.max.x - 1.0 - ACTION_PADDING.right, rect.max.y - 1.0 - ACTION_PADDING.bottom),
    );
    let title_font = FileExistWindow::label_font();
    let file_font = FileExistWindow::file_font();
    let title_h = line_height(ui, &title_font);
    let file_h = line_height(ui, &file_font);
    let stack = match file {
        Some(_) => title_h + 1.0 + file_h,
        None => title_h,
    };
    // The grid is as tall as its tallest cell, centred in the presenter.
    let content = Rect::from_min_size(pos2(inner.min.x, center_y(inner, stack.max(17.0))), vec2(inner.width(), stack.max(17.0)));
    let icon_color = if accent { b.content(ui, enabled, p.accent_icon, behind) } else { b.foreground(ui, &r, enabled, behind) };
    let icon_rect = Rect::from_min_size(pos2(content.min.x, center_y(content, 17.0)), Vec2::splat(17.0));
    icons::paint(ui.painter(), icon_rect, key, icon_color);
    let x = content.min.x + 17.0 + 10.0;
    let w = (content.max.x - x).max(0.0);
    let text_color = b.content(ui, enabled, p.text_secondary, behind);
    let mut y = if file.is_some() { content.min.y } else { center_y(content, title_h) };
    ui.painter().galley(pos2(x, y), galley(ui, title, title_font, text_color, w), text_color);
    y += title_h + 1.0;
    if let Some(f) = file {
        ui.painter().galley(pos2(x, y), galley(ui, f, file_font, text_color, w), text_color);
    }
    r
}

impl Dialog for FileExistWindow {
    fn spec(&self) -> Spec {
        Spec { topmost: true, maximizable: false, ..Spec::fit_height(crate::rs_title(s::FileExistWindow_Title()), WIDTH, 0.0) }
    }

    fn content_height(&mut self, ui: &mut egui::Ui) -> f32 {
        layout::header_height(ui, 1) + Self::body_height(ui)
    }

    fn ui(&mut self, ui: &mut egui::Ui, _cx: &mut Cx) -> Vec<Effect> {
        let p = palette(ui.ctx());
        let full = layout::background(ui);
        let header = layout::header(ui, full, icon::Warning, s::FileExistWindow_FileAlreadyExists(), Some(s::FileExistWindow_Instruction()), false);
        let left = full.min.x + MARGIN.0;
        let right = full.max.x - MARGIN.0;
        let mut y = full.min.y + header + MARGIN.1;

        // The existing path.
        let card_rect = Rect::from_min_max(pos2(left, y), pos2(right, y + Self::path_card_height(ui)));
        card(ui, card_rect, 6.0);
        let inner = Rect::from_min_max(pos2(card_rect.min.x + 11.0, card_rect.min.y + 9.0), pos2(card_rect.max.x - 11.0, card_rect.max.y - 9.0));
        icons::paint(ui.painter(), Rect::from_min_size(pos2(inner.min.x, center_y(inner, 16.0)), Vec2::splat(16.0)), icon::File, p.accent_icon);
        let path = self.file_path.to_string_lossy().into_owned();
        let pf = Self::path_font();
        let tx = inner.min.x + 16.0 + 8.0;
        let g = galley(ui, &path, pf.clone(), p.text_secondary, (inner.max.x - tx).max(0.0));
        ui.painter().galley(pos2(tx, center_y(inner, line_height(ui, &pf))), g, p.text_secondary);
        y = card_rect.max.y + ROW_SPACING;

        // The new name.
        let lf = Self::label_font();
        layout::text(ui, pos2(left, y), s::FileExistWindow_NewFileName(), lf.clone(), p.text_secondary, right - left);
        y += line_height(ui, &lf) + 4.0;
        let box_rect = Rect::from_min_max(pos2(left, y), pos2(right, y + Self::text_box_height(ui)));
        let box_id = ui.id().with("new-name");
        if !self.opened {
            // `OnOpened`: focused, all selected.
            self.opened = true;
            super::text_upload::focus_text_box(ui, box_id, &self.new_name, self.select_on_open);
        }
        TextBox { padding: Thickness::hv(8.0, 5.0), ..TextBox::default() }.show(ui, box_rect, box_id, &mut self.new_name);
        y = box_rect.max.y + ROW_SPACING;

        let row = |i: usize| Rect::from_min_size(pos2(left, y + (ACTION_HEIGHT + ROW_SPACING) * i as f32), vec2(right - left, ACTION_HEIGHT));
        let new_name = self.new_file_name();
        let overwrite = self.file_path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let unique = self.unique_file_path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let enabled = self.new_name_enabled();
        let use_new = action_button(ui, row(0), ui.id().with("use-new"), icon::Edit, true, s::FileExistWindow_UseNewName(), Some(&new_name), enabled).clicked();
        let over = action_button(ui, row(1), ui.id().with("overwrite"), icon::File, true, s::FileExistWindow_Overwrite(), Some(&overwrite), true).clicked();
        let uniq = action_button(ui, row(2), ui.id().with("unique"), icon::Duplicate, true, s::FileExistWindow_UseUniqueName(), Some(&unique), true).clicked();
        let cancel = action_button(ui, row(3), ui.id().with("cancel"), icon::Close, false, s::FileExistWindow_Cancel(), None, true).clicked();

        let (enter, escape) = ui.input(|i| (i.key_pressed(egui::Key::Enter), i.key_pressed(egui::Key::Escape)));
        if use_new && enabled {
            return self.use_new_name();
        }
        if over {
            let path = self.file_path.clone();
            return self.answer(Some(path));
        }
        if uniq {
            let path = self.unique_file_path.clone();
            return self.answer(Some(path));
        }
        if cancel || escape {
            return self.answer(None);
        }
        // `OnWindowKeyDown`: Enter keeps the file's own name (overwrite) or
        // uses the new one.
        if enter && !self.new_name.is_empty() {
            if eq_ignore_case(&self.new_name, &self.original_file_name) {
                let path = self.file_path.clone();
                return self.answer(Some(path));
            }
            return self.use_new_name();
        }
        Vec::new()
    }

    /// Closed without an answer: `FilePath` as it was, the existing file
    /// (overwritten).
    fn closed(&mut self, _cx: &mut Cx) -> Vec<Effect> {
        if let Some(r) = self.reply.take() {
            let _ = r.send(Some(self.file_path.clone()));
        }
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sharexrs-file-exist-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn new_name_keeps_the_extension_and_ignores_case() {
        let dir = scratch("names");
        let file = dir.join("Shot.png");
        std::fs::write(&file, b"x").unwrap();
        let mut w = FileExistWindow::new(&file, None);
        assert_eq!(w.new_name, "Shot");
        assert!(!w.new_name_enabled());
        w.new_name = "SHOT".into();
        assert!(!w.new_name_enabled());
        w.new_name = "other".into();
        assert!(w.new_name_enabled());
        assert_eq!(w.new_file_name(), "other.png");
        assert_eq!(w.unique_file_path, dir.join("Shot (2).png"));
        w.new_name.clear();
        assert_eq!(w.new_file_name(), "");
    }

    #[test]
    fn answers_through_the_channel() {
        let dir = scratch("answer");
        let file = dir.join("a.png");
        let (tx, rx) = std::sync::mpsc::channel();
        let mut w = FileExistWindow::new(&file, Some(tx));
        w.new_name = "b".into();
        let _ = w.use_new_name();
        assert_eq!(rx.recv().unwrap(), Some(dir.join("b.png")));
    }
}
