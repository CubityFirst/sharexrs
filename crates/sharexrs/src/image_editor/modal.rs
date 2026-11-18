//! The editor's modal dialogs' common parts: `EditorView`'s `modal-scrim`
//! (`ShareX.Brush.Overlay.Modal` over the whole view, a press on it closes
//! the dialog) and the frame the dialog views share (a `Border` of
//! `ShareX.Brush.Background.Popup` with a 1px border and 4px corners and a
//! `BoxShadow`, the `Background.Panel` grid inside, a title row on the
//! toolbar background with a bottom border and `Padding="14,8"`, and for
//! some a button bar on the toolbar background with a top border).

use egui::{Color32, CornerRadius, Id, Rect, Response, Sense, Ui, pos2, vec2};
use sharex_ui::border::{self, Brush};
use sharex_ui::controls::Button;
use sharex_ui::theme::Thickness;
use sharex_ui::{Palette, icons, widgets};

/// A dialog's `BoxShadow`: offset y, blur, alpha.
#[derive(Clone, Copy)]
pub struct Shadow(pub i8, pub u8, pub u8);

/// `0 8 24 0 #50000000` (start screen, insert image, emoji picker).
pub const SHADOW_LARGE: Shadow = Shadow(8, 24, 0x50);
/// `0 4 10 0 #40000000` (new image, confirmation).
pub const SHADOW_SMALL: Shadow = Shadow(4, 10, 0x40);

/// The default `TextBlock` (ShareX's style: Segoe UI 13, SemiBold,
/// `Text.Secondary`), at `weight`.
pub fn text_font(size: f32, weight: u16) -> egui::FontId {
    widgets::font(size, weight)
}

/// The modal host centres the dialog in the view (`HorizontalAlignment`
/// and `VerticalAlignment` `Center`, rounded as Avalonia lays it out).
pub fn centered(view: Rect, size: egui::Vec2) -> Rect {
    let x = view.min.x + ((view.width() - size.x) / 2.0).round_ties_even();
    let y = view.min.y + ((view.height() - size.y) / 2.0).round_ties_even().max(0.0);
    Rect::from_min_size(pos2(x, y), size)
}

/// The scrim; returns whether it was pressed outside `dialog`.
pub fn scrim(ui: &Ui, p: &Palette, view: Rect, dialog: Rect) -> bool {
    ui.painter().rect_filled(view, 0.0, p.overlay_modal);
    ui.ctx().input(|i| i.pointer.any_pressed())
        && ui.ctx().input(|i| i.pointer.interact_pos()).is_some_and(|at| view.contains(at) && !dialog.contains(at))
}

/// The dialog's shadow, border and panel; returns the inner rectangle.
pub fn frame(ui: &Ui, p: &Palette, rect: Rect, shadow: Shadow) -> Rect {
    let painter = ui.painter();
    let s = egui::epaint::Shadow { offset: [0, shadow.0], blur: shadow.1, spread: 0, color: Color32::from_black_alpha(shadow.2) };
    painter.add(s.as_shape(rect, CornerRadius::same(4)));
    border::border(painter, rect, 4.0, 1.0, Some(Brush::Solid(p.background_popup)), Some(p.border));
    // The grid's `Background.Panel` fills the border's inside, square (the
    // border doesn't clip it).
    let inner = rect.shrink(1.0);
    painter.rect_filled(inner, 0.0, p.background_panel);
    inner
}

/// The title row's height: `Padding="14,8"` round the 15px title (or the
/// 32px close button), and its bottom border.
pub fn title_height(ui: &Ui, close: bool) -> f32 {
    let lh = widgets::line_height(ui, &text_font(15.0, 600));
    8.0 + if close { lh.max(32.0) } else { lh } + 8.0 + 1.0
}

/// The title row at the top of `inner`: the title (15px SemiBold in
/// `color`) and, with `close`, the 32x32 `toolbar-button` with the 16px
/// close icon. Returns the row and the close button's response.
pub fn title_bar(ui: &Ui, p: &Palette, inner: Rect, title: &str, color: Color32, close: Option<Id>) -> (Rect, Option<Response>) {
    let h = title_height(ui, close.is_some());
    let row = Rect::from_min_size(inner.min, vec2(inner.width(), h));
    let painter = ui.painter();
    painter.rect_filled(Rect::from_min_max(row.min, pos2(row.max.x, row.max.y - 1.0)), 0.0, p.background_toolbar);
    painter.rect_filled(Rect::from_min_max(pos2(row.min.x, row.max.y - 1.0), row.max), 0.0, p.border);
    let content = Rect::from_min_max(pos2(row.min.x + 14.0, row.min.y + 8.0), pos2(row.max.x - 14.0, row.max.y - 9.0));
    let font = text_font(15.0, 600);
    let g = widgets::galley(ui, title, font.clone(), color, f32::INFINITY);
    painter.galley(pos2(content.min.x, widgets::center_y(content, widgets::line_height(ui, &font))), g, color);
    let resp = close.map(|id| {
        let r = Rect::from_min_size(pos2(content.max.x - 32.0, content.center().y - 16.0), vec2(32.0, 32.0));
        let resp = ui.interact(r, id, Sense::click());
        toolbar_button(ui, p, r, &resp, sharex_ui::keys::app_icons::Close, 16.0);
        resp
    });
    (row, resp)
}

/// A `Button.toolbar-button` with an icon of `size`.
pub fn toolbar_button(ui: &Ui, p: &Palette, r: Rect, resp: &Response, icon: &str, size: f32) {
    let hovered = resp.hovered() && !resp.is_pointer_button_down_on();
    let fill = if hovered { p.control_background_hover } else { p.control_background };
    border::border(ui.painter(), r, 4.0, 1.0, Some(Brush::Solid(fill)), Some(p.border));
    let fg = if resp.hovered() || resp.is_pointer_button_down_on() { sharex_ui::theme::fluent(ui.ctx()).base_high } else { p.text };
    icons::paint(ui.painter(), Rect::from_center_size(r.center(), vec2(size, size)), icon, fg);
}

/// The button bar's height (`Padding="10"`, 40px buttons, its top border).
pub const BUTTON_BAR: f32 = 1.0 + 10.0 + 40.0 + 10.0;

/// The bottom bar of `inner` with `labels` at its right (`Padding="20,0"`
/// buttons, 5 apart); returns the index of the one clicked.
pub fn button_bar(ui: &Ui, p: &Palette, inner: Rect, id: Id, labels: &[&str]) -> Option<usize> {
    let bar = Rect::from_min_max(pos2(inner.min.x, inner.max.y - BUTTON_BAR), inner.max);
    ui.painter().rect_filled(bar, 0.0, p.background_toolbar);
    ui.painter().rect_filled(Rect::from_min_size(bar.min, vec2(bar.width(), 1.0)), 0.0, p.border);
    let b = Button { padding: Thickness::new(20.0, 0.0, 20.0, 0.0), ..Default::default() };
    let widths: Vec<f32> = labels.iter().map(|l| b.width(ui, l).ceil()).collect();
    let total = widths.iter().sum::<f32>() + 5.0 * (labels.len() as f32 - 1.0);
    let mut x = bar.max.x - 10.0 - total;
    let mut clicked = None;
    for (i, label) in labels.iter().enumerate() {
        let r = Rect::from_min_size(pos2(x, bar.min.y + 11.0), vec2(widths[i], 40.0));
        if b.show(ui, r, id.with(i), label, true).clicked() {
            clicked = Some(i);
        }
        x += widths[i] + 5.0;
    }
    clicked
}

/// A `Button` (`HorizontalContentAlignment="Left"`) whose content is an
/// icon of `icon_size` and a `TextBlock` (`FontWeight="Normal"`) 12px
/// after it, centred vertically, `padding` in.
#[allow(clippy::too_many_arguments)]
pub fn icon_text_button(ui: &Ui, p: &Palette, rect: Rect, id: Id, icon: &str, icon_size: f32, text: &str, padding: Thickness) -> Response {
    let b = Button { padding, ..Default::default() };
    let resp = b.show(ui, rect, id, "", true);
    let fg = if resp.hovered() || resp.is_pointer_button_down_on() { sharex_ui::theme::fluent(ui.ctx()).base_high } else { p.text };
    let inner = Rect::from_min_max(pos2(rect.min.x + 1.0 + padding.left, rect.min.y + 1.0 + padding.top), pos2(rect.max.x - 1.0 - padding.right, rect.max.y - 1.0 - padding.bottom));
    let font = text_font(13.0, 400);
    let lh = widgets::line_height(ui, &font);
    let row_h = lh.max(icon_size);
    let top = widgets::center_y(inner, row_h);
    let ir = Rect::from_min_size(pos2(inner.min.x, top + ((row_h - icon_size) / 2.0).round_ties_even()), vec2(icon_size, icon_size));
    icons::paint(ui.painter(), ir, icon, fg);
    let g = widgets::galley(ui, text, font, p.text_secondary, f32::INFINITY);
    ui.painter().galley(pos2(ir.max.x + 12.0, top + ((row_h - lh) / 2.0).round_ties_even()), g, p.text_secondary);
    resp
}
