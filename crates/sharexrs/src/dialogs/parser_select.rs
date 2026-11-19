//! `ShareX.UploadersLib/Presentation/ParserSelect/ParserSelectWindow.axaml(.cs)`
//! (`ParserSelectWindowIntegration.Show`), a custom uploader's
//! `{select:a|b|...}`: a button per value, ten to a column, all as wide as
//! the widest; the value clicked goes back to the task, and closing the
//! window gives the first (`SelectedText` starts as it).

use std::sync::mpsc;

use egui::{Rect, pos2, vec2};
use sharex_strings::uploaders_lib as u;
use sharex_ui::controls::Button;
use sharex_ui::theme::Thickness;
use sharex_ui::widgets;

use super::{Cx, Dialog, Effect, Spec};

/// `ItemsPerColumn`.
const ITEMS_PER_COLUMN: usize = 10;
/// The panel's `Margin="8"` and both `Spacing="6"`.
const MARGIN: f32 = 8.0;
const SPACING: f32 = 6.0;
/// `Button.parser-option`: `MinHeight="38"`, `Padding="12,7"`, `FontSize="16"`.
const MIN_HEIGHT: f32 = 38.0;

pub struct ParserSelectWindow {
    /// `Texts`: the values that aren't empty.
    pub texts: Vec<String>,
    /// `SelectedText`.
    pub selected: String,
    reply: Option<mpsc::Sender<Option<String>>>,
}

impl ParserSelectWindow {
    pub fn new(texts: &[String], reply: Option<mpsc::Sender<Option<String>>>) -> Self {
        let texts: Vec<String> = texts.iter().filter(|t| !t.is_empty()).cloned().collect();
        let selected = texts.first().cloned().unwrap_or_default();
        Self { texts, selected, reply }
    }

    fn button() -> Button {
        Button { padding: Thickness::hv(12.0, 7.0), font_size: 16.0, ..Button::default() }
    }

    /// The buttons' size: the widest text's (whole pixels), and the
    /// minimum height or the text's line.
    fn button_size(&self, ui: &egui::Ui) -> egui::Vec2 {
        let b = Self::button();
        let w = self.texts.iter().map(|t| b.width(ui, t).ceil()).fold(0.0, f32::max);
        let line = widgets::line_height(ui, &sharex_ui::controls::content_font(16.0, 400));
        vec2(w, MIN_HEIGHT.max(line + 14.0 + 2.0))
    }

    fn columns(&self) -> usize {
        self.texts.len().div_ceil(ITEMS_PER_COLUMN)
    }

    fn rows(&self) -> usize {
        self.texts.len().min(ITEMS_PER_COLUMN)
    }
}

impl Dialog for ParserSelectWindow {
    /// `MinWidth="200" MinHeight="50"`, `SizeToContent="WidthAndHeight"`,
    /// `CanResize="False"`, `Topmost`, centred on the screen.
    fn spec(&self) -> Spec {
        Spec {
            topmost: true,
            maximizable: false,
            resizable: false,
            size_to_content: true,
            ..Spec::sized(crate::rs_title(u::ParserSelectWindow_ShareX_Select()), 200.0, 50.0, 200.0, 50.0)
        }
    }

    fn content_width(&mut self, ui: &mut egui::Ui) -> Option<f32> {
        let size = self.button_size(ui);
        let cols = self.columns() as f32;
        Some((2.0 * MARGIN + cols * size.x + (cols - 1.0).max(0.0) * SPACING).max(200.0))
    }

    fn content_height(&mut self, ui: &mut egui::Ui) -> f32 {
        let size = self.button_size(ui);
        let rows = self.rows() as f32;
        (2.0 * MARGIN + rows * size.y + (rows - 1.0).max(0.0) * SPACING).max(50.0)
    }

    fn ui(&mut self, ui: &mut egui::Ui, _cx: &mut Cx) -> Vec<Effect> {
        let full = super::layout::background(ui);
        let size = self.button_size(ui);
        let b = Self::button();
        let mut clicked = None;
        for (i, text) in self.texts.iter().enumerate() {
            let (col, row) = (i / ITEMS_PER_COLUMN, i % ITEMS_PER_COLUMN);
            let at = pos2(full.min.x + MARGIN + col as f32 * (size.x + SPACING), full.min.y + MARGIN + row as f32 * (size.y + SPACING));
            if b.show(ui, Rect::from_min_size(at, size), ui.id().with(("option", i)), text, true).clicked() {
                clicked = Some(text.clone());
            }
        }
        match clicked {
            Some(text) => {
                // `Select(text)`: `SelectedText = text; Close()`.
                self.selected = text;
                vec![Effect::Close]
            }
            None => Vec::new(),
        }
    }

    fn closed(&mut self, _cx: &mut Cx) -> Vec<Effect> {
        if let Some(r) = self.reply.take() {
            let _ = r.send(Some(self.selected.clone()));
        }
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_and_layout() {
        let texts: Vec<String> = ["a.com", "", "b.com"].iter().map(|s| s.to_string()).collect();
        let w = ParserSelectWindow::new(&texts, None);
        // Empty values dropped; the first is selected until one is clicked.
        assert_eq!(w.texts, vec!["a.com", "b.com"]);
        assert_eq!(w.selected, "a.com");
        assert_eq!((w.columns(), w.rows()), (1, 2));
        let many: Vec<String> = (0..23).map(|i| format!("v{i}")).collect();
        let w = ParserSelectWindow::new(&many, None);
        assert_eq!((w.columns(), w.rows()), (3, 10));
    }
}
