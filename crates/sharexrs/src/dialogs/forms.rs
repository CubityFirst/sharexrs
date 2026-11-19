//! What UploadersLib's small form windows share (`EmailWindow`,
//! `YouTubeVideoOptionsWindow`, `OAuthListenerWindow`): `Margin="16"`, a
//! `Border.section-card` (panel background, 1px border, `CornerRadius="8"`,
//! `Padding="16"`), `TextBlock.field-label`s (`Margin="0,0,0,5"`, secondary)
//! over text boxes (the global `TextBox` style: `Padding="8,6"`,
//! `MinHeight="40"`), and `Button.compact` rows (`MinWidth="96"`, 7 apart).

use egui::{Id, Rect, Ui, pos2, vec2};
use sharex_ui::controls::TextBox;
use sharex_ui::theme::{Thickness, palette};
use sharex_ui::widgets::line_height;

use super::layout::{self, Compact};

pub const MARGIN: f32 = 16.0;
pub const CARD_PADDING: f32 = 16.0;
pub const ROW_SPACING: f32 = 12.0;
pub const LABEL_GAP: f32 = 5.0;
pub const BOX_HEIGHT: f32 = 40.0;
pub const COMPACT: Compact = Compact { min_width: 96.0 };

/// The section card in `rect` (its border box); returns its content box.
pub fn card(ui: &Ui, rect: Rect) -> Rect {
    layout::panel(ui, rect, 8.0);
    rect.shrink(1.0 + CARD_PADDING)
}

/// A field label's height, with its bottom margin.
pub fn label_height(ui: &Ui) -> f32 {
    line_height(ui, &layout::text_font()) + LABEL_GAP
}

/// A field label at `pos`; returns where its field starts.
pub fn label(ui: &Ui, pos: egui::Pos2, text: &str, width: f32) -> f32 {
    layout::text(ui, pos, text, layout::text_font(), palette(ui.ctx()).text_secondary, width);
    pos.y + label_height(ui)
}

/// A one-line text box (`Padding="8,6"`, 40 high) at `top`, across `x0..x1`.
pub fn text_box(ui: &mut Ui, x0: f32, x1: f32, top: f32, id: Id, text: &mut String, placeholder: &str) -> Rect {
    let r = Rect::from_min_size(pos2(x0, top), vec2(x1 - x0, BOX_HEIGHT));
    let tb = TextBox { padding: Thickness::hv(8.0, 6.0), placeholder, ..TextBox::default() };
    tb.show(ui, r, id, text);
    r
}

/// A multi-line, wrapping text box (`AcceptsReturn`, `TextWrapping="Wrap"`,
/// `Padding="9"`, top-aligned) filling `rect`.
pub fn message_box(ui: &mut Ui, rect: Rect, id: Id, text: &mut String) {
    let tb = TextBox { padding: Thickness::all(9.0), multiline: true, wrap: true, ..TextBox::default() };
    tb.show(ui, rect, id, text);
}
