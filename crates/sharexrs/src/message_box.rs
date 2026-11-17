//! `ShareX.Avalonia/Controls/MessageBox.axaml(.cs)`: ShareX's message box,
//! a 480px wide window (`SizeToContent="Height"`) with an icon, the text and
//! a footer of buttons (OK, or Yes and No) and an optional check box
//! (`OptionCheckBox`).

use egui::{Align, CornerRadius, Rect, Sense, Stroke, Ui, Vec2, pos2, vec2};
use sharex_strings::avalonia as av;
use sharex_ui::icons;
use sharex_ui::keys::app_icons as icon;
use sharex_ui::menu::paint_accent_gradient;
use sharex_ui::theme::palette;
use sharex_ui::widgets::{self, font};

/// `MessageBoxWindow`'s `Width`.
pub const WIDTH: f32 = 480.0;
/// The content grid's `Margin="22,22,22,20"`.
const MARGIN: (f32, f32, f32, f32) = (22.0, 22.0, 22.0, 20.0);
/// `IconContainer`: 48px, `CornerRadius="8"`, a 24px icon.
const ICON_BOX: f32 = 48.0;
const ICON: f32 = 24.0;
/// `ColumnSpacing="16"`.
const COLUMN_SPACING: f32 = 16.0;
/// `MessageText`: `FontSize="14"` in `ShareX.FontFamily.Default`.
const TEXT_SIZE: f32 = 14.0;
/// The footer `Border`: `Padding="14,12"`, a 1px top border.
const FOOTER_PADDING: (f32, f32) = (14.0, 12.0);
/// A footer button: `MinWidth="88"`, `Height="36"` (the `Button` style's
/// `Padding="10,6"`, `CornerRadius="4"`, `FontSize="13"`).
const BUTTON_MIN_WIDTH: f32 = 88.0;
const BUTTON_HEIGHT: f32 = 36.0;

/// `MessageBoxIcon` (all of them, for the message boxes still to come).
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageBoxIcon {
    None,
    Error,
    Question,
    Warning,
    Information,
}

/// `DialogResult` (the ones ShareXrs uses so far).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogResult {
    /// Closed without a button (`DialogResult.None`).
    None,
    Ok,
    Yes,
    No,
}

/// `MessageBoxButtons` (the ones ShareXrs uses so far).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageBoxButtons {
    Ok,
    YesNo,
}

pub struct MessageBox {
    pub caption: String,
    pub text: String,
    pub icon: MessageBoxIcon,
    pub buttons: MessageBoxButtons,
    /// `OptionCheckBox`'s text, if shown, and whether it's ticked.
    pub check_box: Option<String>,
    pub checked: bool,
}

impl MessageBox {
    /// `MessageBox.Show(text, caption, MessageBoxButtons.OK, icon)`.
    pub fn ok(caption: impl Into<String>, text: impl Into<String>, icon: MessageBoxIcon) -> Self {
        Self { caption: caption.into(), text: text.into(), icon, buttons: MessageBoxButtons::Ok, check_box: None, checked: false }
    }

    /// `MessageBox.Show(text, caption, MessageBoxButtons.YesNo, checkBoxText, out checked)`
    /// (no icon).
    pub fn yes_no_with_check_box(caption: impl Into<String>, text: impl Into<String>, check_box: impl Into<String>) -> Self {
        Self {
            caption: caption.into(),
            text: text.into(),
            icon: MessageBoxIcon::None,
            buttons: MessageBoxButtons::YesNo,
            check_box: Some(check_box.into()),
            checked: false,
        }
    }

    /// `GetButtonDefinitions`; the first is the default (`active`) one.
    fn button_definitions(&self) -> Vec<(DialogResult, &'static str)> {
        match self.buttons {
            MessageBoxButtons::Ok => vec![(DialogResult::Ok, av::MessageBox_OK())],
            MessageBoxButtons::YesNo => vec![(DialogResult::Yes, av::MessageBox_Yes()), (DialogResult::No, av::MessageBox_No())],
        }
    }

    /// `GetCancelResult`: what Escape (`IsCancel`) and closing the window answer.
    pub fn cancel_result(&self) -> DialogResult {
        match self.buttons {
            MessageBoxButtons::Ok => DialogResult::Ok,
            MessageBoxButtons::YesNo => DialogResult::None,
        }
    }

    fn text_job(&self, ui: &Ui) -> std::sync::Arc<egui::Galley> {
        let p = palette(ui.ctx());
        let mut job = egui::text::LayoutJob::single_section(
            self.text.clone(),
            egui::TextFormat::simple(font(TEXT_SIZE, 400), p.text),
        );
        job.wrap.max_width = self.text_width();
        job.halign = Align::LEFT;
        ui.fonts_mut(|f| f.layout_job(job))
    }

    fn text_width(&self) -> f32 {
        // A hidden icon leaves its column empty, but the column spacing stays.
        let icon = if self.icon == MessageBoxIcon::None { 0.0 } else { ICON_BOX };
        WIDTH - MARGIN.0 - MARGIN.2 - icon - COLUMN_SPACING
    }

    fn row_height(&self, ui: &Ui) -> f32 {
        let rows = self.text_job(ui).rows.len().max(1) as f32;
        let text_h = (rows * ui.fonts_mut(|f| f.row_height(&font(TEXT_SIZE, 400)))).ceil();
        let icon_h = if self.icon == MessageBoxIcon::None { 0.0 } else { ICON_BOX };
        // The `ScrollViewer`'s `MaxHeight="420"` (with the margins).
        text_h.max(icon_h).min(420.0 - MARGIN.1 - MARGIN.3)
    }

    /// The window's height for its content (`SizeToContent="Height"`).
    pub fn height(&self, ui: &Ui) -> f32 {
        MARGIN.1 + self.row_height(ui) + MARGIN.3 + 1.0 + FOOTER_PADDING.1 * 2.0 + BUTTON_HEIGHT
    }

    /// Draws the window's client area; returns the result once a button is
    /// pressed (Enter and Escape press OK: `IsDefault`, `IsCancel`).
    pub fn ui(&mut self, ui: &mut Ui) -> Option<DialogResult> {
        let p = palette(ui.ctx());
        let full = ui.max_rect();
        ui.painter().rect_filled(full, 0.0, p.background_main);

        let row_h = self.row_height(ui);
        let top = full.min.y + MARGIN.1;
        let mut x = full.min.x + MARGIN.0;
        if let Some((key, color)) = self.icon_key(&p) {
            let bx = Rect::from_min_size(pos2(x, top), Vec2::splat(ICON_BOX));
            ui.painter().rect_filled(bx, CornerRadius::same(8), p.background_panel);
            let off = ((ICON_BOX - ICON) / 2.0).round_ties_even();
            icons::paint(ui.painter(), Rect::from_min_size(bx.min + vec2(off, off), Vec2::splat(ICON)), key, color);
            x += ICON_BOX;
        }
        x += COLUMN_SPACING;
        // Avalonia steps wrapped lines by the font's exact line height and
        // puts each baseline at the nearest pixel; an egui galley steps
        // whole pixels and rounds its ascent up. So each line is laid out on
        // its own, with its baseline where Avalonia's is.
        let text = self.text_job(ui);
        let line_h = ui.fonts_mut(|f| f.row_height(&font(TEXT_SIZE, 400)));
        let text_h = text.rows.len().max(1) as f32 * line_h;
        let row = Rect::from_min_size(pos2(x, top), vec2(self.text_width(), row_h));
        let text_top = widgets::center_y(row, text_h);
        for (i, line) in text.rows.iter().enumerate() {
            let line: String = line.glyphs.iter().map(|g| g.chr).collect();
            let g = widgets::galley(ui, line.trim_end(), font(TEXT_SIZE, 400), p.text, f32::INFINITY);
            let Some(glyph) = g.rows.first().and_then(|r| r.glyphs.first()) else { continue };
            let baseline = (text_top + i as f32 * line_h + glyph.font_ascent).round();
            ui.painter().galley(pos2(x, baseline - glyph.pos.y), g, p.text);
        }

        // The footer: `Background.Toolbar` under a 1px `Border` line.
        let footer_top = top + row_h + MARGIN.3;
        let footer = Rect::from_min_max(pos2(full.min.x, footer_top), full.max);
        ui.painter().rect_filled(footer, 0.0, p.background_toolbar);
        ui.painter().rect_filled(Rect::from_min_size(footer.min, vec2(footer.width(), 1.0)), 0.0, p.border);

        // The buttons, right aligned 8px apart; the first is the default
        // (`Classes="active"`, Enter). Escape answers `GetCancelResult`.
        let label_font = egui::FontId::new(13.0, sharex_ui::fonts::inter(400));
        let line = ui.fonts_mut(|f| f.row_height(&label_font));
        let defs = self.button_definitions();
        let widths: Vec<f32> = defs
            .iter()
            .map(|(_, label)| BUTTON_MIN_WIDTH.max(widgets::text_width(ui, label, label_font.clone()) + 2.0 * (10.0 + 1.0)))
            .collect();
        let total = widths.iter().sum::<f32>() + 8.0 * (defs.len() as f32 - 1.0);
        let mut x = footer.max.x - FOOTER_PADDING.0 - total;
        let mut result = None;
        for (i, ((r, label), width)) in defs.iter().zip(&widths).enumerate() {
            let button = Rect::from_min_size(pos2(x, footer_top + 1.0 + FOOTER_PADDING.1), vec2(*width, BUTTON_HEIGHT));
            let response = ui.interact(button, ui.id().with(("message-box-button", i)), Sense::click());
            let text_color = if i == 0 {
                paint_accent_gradient(ui, button, CornerRadius::same(4));
                p.accent_foreground
            } else {
                let fill = if response.hovered() && !response.is_pointer_button_down_on() {
                    p.control_background_hover
                } else {
                    p.control_background
                };
                ui.painter().rect_filled(button, CornerRadius::same(4), fill);
                p.text
            };
            ui.painter().rect_stroke(button, CornerRadius::same(4), Stroke::new(1.0, p.border), egui::StrokeKind::Inside);
            let g = widgets::galley(ui, label, label_font.clone(), text_color, f32::INFINITY);
            let tx = button.min.x + ((button.width() - g.size().x) / 2.0).round_ties_even();
            ui.painter().galley(pos2(tx, widgets::center_y(button, line)), g, text_color);
            if response.clicked() {
                result = Some(*r);
            }
            x += width + 8.0;
        }

        // `OptionCheckBox`: Fluent's 20px box and its text, 8px in from the left.
        if let Some(text) = &self.check_box {
            let row = Rect::from_min_max(
                pos2(footer.min.x + FOOTER_PADDING.0 + 8.0, footer_top + 1.0 + FOOTER_PADDING.1),
                pos2(footer.max.x - FOOTER_PADDING.0 - total - 12.0, footer_top + 1.0 + FOOTER_PADDING.1 + BUTTON_HEIGHT),
            );
            // Plain string content: Avalonia's default family, Inter.
            let text_font = egui::FontId::new(13.0, sharex_ui::fonts::inter(400));
            let text_w = widgets::text_width(ui, text, text_font.clone());
            let hit = Rect::from_min_size(pos2(row.min.x, widgets::center_y(row, 32.0)), vec2(20.0 + 8.0 + text_w, 32.0));
            let response = ui.interact(hit, ui.id().with("message-box-check"), Sense::click());
            if response.clicked() {
                self.checked = !self.checked;
            }
            let check = Rect::from_min_size(pos2(row.min.x, widgets::center_y(row, 20.0)), Vec2::splat(20.0));
            if self.checked {
                paint_accent_gradient(ui, check, CornerRadius::same(4));
                ui.painter().rect_stroke(check, CornerRadius::same(4), Stroke::new(1.0, p.accent_border), egui::StrokeKind::Inside);
                icons::paint(ui.painter(), check.shrink(4.0), icon::Check, p.accent_foreground);
            } else {
                let fill = if response.hovered() { p.control_background_hover } else { p.control_background };
                ui.painter().rect_filled(check, CornerRadius::same(4), fill);
                ui.painter().rect_stroke(check, CornerRadius::same(4), Stroke::new(1.0, p.border), egui::StrokeKind::Inside);
            }
            let line_h = widgets::line_height(ui, &text_font);
            let g = widgets::galley(ui, text, text_font, p.text, row.width() - 28.0);
            ui.painter().galley(pos2(check.max.x + 8.0, widgets::center_y(row, line_h)), g, p.text);
        }

        let (enter, escape, space) =
            ui.input(|i| (i.key_pressed(egui::Key::Enter), i.key_pressed(egui::Key::Escape), i.key_pressed(egui::Key::Space)));
        if enter || (space && self.buttons == MessageBoxButtons::Ok) {
            result = result.or(Some(defs[0].0));
        }
        if escape && self.cancel_result() != DialogResult::None {
            result = result.or(Some(self.cancel_result()));
        }
        result
    }

    /// `ConfigureIcon`: the icon key and its colour.
    fn icon_key(&self, p: &sharex_ui::theme::Palette) -> Option<(&'static str, egui::Color32)> {
        const ERROR: egui::Color32 = egui::Color32::from_rgb(0xEF, 0x44, 0x44);
        const WARNING: egui::Color32 = egui::Color32::from_rgb(0xF5, 0x9E, 0x0B);
        match self.icon {
            MessageBoxIcon::None => None,
            MessageBoxIcon::Error => Some((icon::CircleX, ERROR)),
            MessageBoxIcon::Question => Some((icon::Help, p.accent_icon)),
            MessageBoxIcon::Warning => Some((icon::Warning, WARNING)),
            MessageBoxIcon::Information => Some((icon::Info, p.accent_icon)),
        }
    }
}
