//! `ShareX.UploadersLib/Presentation/TextUpload/TextUploadWindow.axaml(.cs)`:
//! the text to upload, prefilled from the clipboard and selected.

use egui::{Id, Rect, pos2};
use sharex_settings::TaskSettings;
use sharex_strings::uploaders_lib as s;
use sharex_ui::controls::TextBox;
use sharex_ui::keys::app_icons as icon;
use sharex_ui::theme::{Thickness, palette};
use sharex_ui::widgets::{center_y, line_height};

use super::layout::{self, Compact};
use super::{Cx, Dialog, Effect, Spec, UploadRequest};

pub struct TextUploadWindow {
    pub text: String,
    /// `_selectInitialContent`: the prefilled text is selected on open.
    select_initial: bool,
    opened: bool,
    task: TaskSettings,
    /// Answers with the text instead of uploading it (`ShowAsync`'s result).
    pub reply: Option<std::sync::mpsc::Sender<Option<String>>>,
}

impl TextUploadWindow {
    /// `new TextUploadWindow(content)`: the clipboard's text when there's
    /// no content.
    pub fn new(content: Option<String>, task: TaskSettings) -> Self {
        let content = content.filter(|c| !c.is_empty()).or_else(|| {
            if sharex_capture::clipboard::contains_text() { sharex_capture::clipboard::get_text() } else { None }
        });
        let text = content.unwrap_or_default();
        Self { select_initial: !text.is_empty(), text, opened: false, task, reply: None }
    }

    /// `UpdateCharacterCount`: .NET's `Length`, UTF-16 units.
    fn character_count(&self) -> String {
        let n = self.text.encode_utf16().count();
        let format = if n == 1 { s::TextUploadWindow_Character_count_singular() } else { s::TextUploadWindow_Character_count_plural() };
        sharex_tools::text::format_dotnet(format, &[sharex_tools::text::Arg::I64(n as i64)])
    }
}

/// Focuses the text box `id` and selects all of `text` (`Focus()`,
/// `SelectAll()`), or puts the caret at the start.
pub fn focus_text_box(ui: &egui::Ui, id: Id, text: &str, select_all: bool) {
    use egui::text::{CCursor, CCursorRange};
    ui.memory_mut(|m| m.request_focus(id));
    let mut state = egui::text_edit::TextEditState::load(ui.ctx(), id).unwrap_or_default();
    let end = if select_all { text.chars().count() } else { 0 };
    state.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(0), CCursor::new(end))));
    state.store(ui.ctx(), id);
}

impl Dialog for TextUploadWindow {
    fn closed(&mut self, _cx: &mut Cx) -> Vec<Effect> {
        if let Some(r) = self.reply.take() {
            let _ = r.send(None);
        }
        Vec::new()
    }

    fn spec(&self) -> Spec {
        Spec::sized(crate::rs_title(s::TextUploadWindow_ShareX_Text_upload()), 680.0, 560.0, 480.0, 380.0)
    }

    fn ui(&mut self, ui: &mut egui::Ui, _cx: &mut Cx) -> Vec<Effect> {
        let p = palette(ui.ctx());
        let full = layout::background(ui);
        let header = layout::header(ui, full, icon::FileText, s::TextUploadWindow_Text_upload(), Some(s::TextUploadWindow_Enter_or_paste_the_text_you_want_to_upload()), false);
        // The bottom row: `Margin="18,0,18,18"`, the buttons' 40px.
        let bottom_top = full.max.y - 18.0 - Compact::HEIGHT;
        let card = Rect::from_min_max(pos2(full.min.x + 18.0, full.min.y + header + 14.0), pos2(full.max.x - 18.0, bottom_top - 14.0));
        layout::panel(ui, card, 6.0);
        // The `TextBox` fills the card (`Padding="12"`, top-aligned).
        let id = ui.id().with("content");
        let tb = TextBox { padding: Thickness::all(12.0), multiline: true, ..TextBox::default() };
        let inner = card.shrink(1.0);
        tb.show(ui, inner, id, &mut self.text);
        if !self.opened {
            // `OnOpened`: `Focus()`, and `SelectAll()` for prefilled text.
            self.opened = true;
            focus_text_box(ui, id, &self.text, self.select_initial);
        }
        let row = Rect::from_min_max(pos2(full.min.x + 18.0, bottom_top), pos2(full.max.x - 18.0, full.max.y - 18.0));
        let count_font = layout::text_font();
        let y = center_y(row, line_height(ui, &count_font));
        layout::text(ui, pos2(row.min.x, y), &self.character_count(), count_font, p.text_secondary, row.width());
        let compact = Compact { min_width: 96.0 };
        let labels = [s::TextUploadWindow_Cancel(), s::TextUploadWindow_Upload()];
        let rects = compact.row(ui, row.max.x, row.min.y, 7.0, &labels);
        let cancel = compact.show(ui, rects[0], ui.id().with("cancel"), labels[0], false, true).clicked();
        let upload = compact.show(ui, rects[1], ui.id().with("upload"), labels[1], true, true).clicked();
        // `IsCancel`: Escape closes.
        if cancel || sharex_ui::popup::escape_pressed(ui.ctx()) {
            return vec![Effect::Close];
        }
        if upload {
            if let Some(r) = self.reply.take() {
                let _ = r.send(Some(self.text.clone()));
                return vec![Effect::Close];
            }
            // `ShowTextUploadDialog`: an empty text uploads nothing.
            let mut out = Vec::new();
            if !self.text.is_empty() {
                out.push(Effect::Upload(Box::new(UploadRequest::Text { text: self.text.clone(), task: self.task.clone(), allow_custom_text: false })));
            }
            out.push(Effect::Close);
            return out;
        }
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn counts_are_grouped_by_thousands() {
        // `{0:N0}` in en-US's separators (the formatter takes the current culture's).
        assert_eq!(sharex_tools::text::format_number(16.0, "N0", true), "16");
        assert_eq!(sharex_tools::text::format_number(1234567.0, "N0", true), "1,234,567");
        assert_eq!(sharex_tools::text::with_separators("1,234,567.5", ".", ","), "1.234.567,5");
    }
}
