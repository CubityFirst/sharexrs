//! `ShareX.Avalonia/Controls/MessageBox.axaml(.cs)`: ShareX's message box,
//! a 480px wide window (`SizeToContent="Height"`) with an icon, the text and
//! a footer of buttons. Only what ShareXrs shows yet: OK buttons.

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
    Ok,
}

pub struct MessageBox {
    pub caption: String,
    pub text: String,
    pub icon: MessageBoxIcon,
}

impl MessageBox {
    /// `MessageBox.Show(text, caption, MessageBoxButtons.OK, icon)`.
    pub fn ok(caption: impl Into<String>, text: impl Into<String>, icon: MessageBoxIcon) -> Self {
        Self { caption: caption.into(), text: text.into(), icon }
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
        let icon = if self.icon == MessageBoxIcon::None { 0.0 } else { ICON_BOX + COLUMN_SPACING };
        WIDTH - MARGIN.0 - MARGIN.2 - icon
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
    pub fn ui(&self, ui: &mut Ui) -> Option<DialogResult> {
        let p = palette(ui.ctx());
        let full = ui.max_rect();
        ui.painter().rect_filled(full, 0.0, p.background_main);

        let row_h = self.row_height(ui);
        let top = full.min.y + MARGIN.1;
        let mut x = full.min.x + MARGIN.0;
        if let Some((key, color)) = self.icon_key(p) {
            let bx = Rect::from_min_size(pos2(x, top), Vec2::splat(ICON_BOX));
            ui.painter().rect_filled(bx, CornerRadius::same(8), p.background_panel);
            let off = ((ICON_BOX - ICON) / 2.0).round_ties_even();
            icons::paint(ui.painter(), Rect::from_min_size(bx.min + vec2(off, off), Vec2::splat(ICON)), key, color);
            x += ICON_BOX + COLUMN_SPACING;
        }
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

        // The OK button: `Classes="active"` (the default button), right aligned.
        let label = av::MessageBox_OK();
        let label_font = egui::FontId::new(13.0, sharex_ui::fonts::inter(400));
        let width = BUTTON_MIN_WIDTH.max(widgets::text_width(ui, label, label_font.clone()) + 2.0 * (10.0 + 1.0));
        let button = Rect::from_min_size(
            pos2(footer.max.x - FOOTER_PADDING.0 - width, footer_top + 1.0 + FOOTER_PADDING.1),
            vec2(width, BUTTON_HEIGHT),
        );
        paint_accent_gradient(ui, button, CornerRadius::same(4));
        ui.painter().rect_stroke(button, CornerRadius::same(4), Stroke::new(1.0, p.border), egui::StrokeKind::Inside);
        let line = ui.fonts_mut(|f| f.row_height(&label_font));
        let g = widgets::galley(ui, label, label_font, p.accent_foreground, f32::INFINITY);
        let tx = button.min.x + ((button.width() - g.size().x) / 2.0).round_ties_even();
        ui.painter().galley(pos2(tx, widgets::center_y(button, line)), g, p.accent_foreground);

        let clicked = ui.interact(button, ui.id().with("message-box-ok"), Sense::click()).clicked();
        let key = ui.input(|i| {
            i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::Escape) || i.key_pressed(egui::Key::Space)
        });
        (clicked || key).then_some(DialogResult::Ok)
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
