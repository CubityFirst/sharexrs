//! The pieces ShareX's upload windows share: the header band (`Border`
//! on the toolbar background, `Padding="18,12"`, a 40px icon tile, the
//! title and a line under it), `Button.compact` rows, and the panel cards.

use egui::{Id, Rect, Response, Ui, Vec2, pos2, vec2};
use sharex_ui::controls::{Button, card};
use sharex_ui::theme::{Thickness, palette};
use sharex_ui::widgets::{center_y, font, galley, line_height, text_width};
use sharex_ui::{border, icons};

/// The header's `Padding="18,12"`.
pub const HEADER_PADDING: (f32, f32) = (18.0, 12.0);
/// The icon tile: 40x40, `CornerRadius="6"`, a 20px icon.
const TILE: f32 = 40.0;
const TILE_ICON: f32 = 20.0;
/// `ColumnSpacing="10"` between the tile and the text.
const TILE_SPACING: f32 = 10.0;

/// The title (`FontSize="19" FontWeight="SemiBold"`).
pub fn title_font() -> egui::FontId {
    font(19.0, 600)
}

/// A plain `TextBlock`: ShareX's global style (13px SemiBold Segoe UI).
pub fn text_font() -> egui::FontId {
    font(13.0, 600)
}

/// The header's height: the tile or the text stack (`Spacing="2"` between
/// title and subtitle), whichever is taller, in the padding.
pub fn header_height(ui: &Ui, subtitle_lines: usize) -> f32 {
    let mut text = line_height(ui, &title_font());
    if subtitle_lines > 0 {
        text += 2.0 + line_height(ui, &text_font()) * subtitle_lines as f32;
    }
    HEADER_PADDING.1 * 2.0 + text.max(TILE)
}

/// Draws the header across the top of `full`, with `subtitle` lines under
/// the title (trimmed with an ellipsis, or wrapped when `wrap`). Returns
/// its height.
pub fn header(ui: &Ui, full: Rect, icon: &str, title: &str, subtitle: Option<&str>, wrap: bool) -> f32 {
    let p = palette(ui.ctx());
    let text_x = full.min.x + HEADER_PADDING.0 + TILE + TILE_SPACING;
    let text_w = full.max.x - HEADER_PADDING.0 - text_x;
    let lines: Vec<String> = match subtitle {
        Some(s) if wrap => sharex_ui::controls::wrap_lines(ui, s, &text_font(), text_w),
        Some(s) => vec![s.to_owned()],
        None => Vec::new(),
    };
    let height = header_height(ui, lines.len());
    let band = Rect::from_min_size(full.min, vec2(full.width(), height));
    ui.painter().rect_filled(band, 0.0, p.background_toolbar);
    let row = Rect::from_min_max(pos2(band.min.x + HEADER_PADDING.0, band.min.y + HEADER_PADDING.1), pos2(band.max.x - HEADER_PADDING.0, band.max.y - HEADER_PADDING.1));
    let tile = Rect::from_min_size(pos2(row.min.x, center_y(row, TILE)), Vec2::splat(TILE));
    border::border(ui.painter(), tile, 6.0, 0.0, Some(border::Brush::Solid(p.background_panel)), None);
    let off = ((TILE - TILE_ICON) / 2.0).round_ties_even();
    icons::paint(ui.painter(), Rect::from_min_size(tile.min + vec2(off, off), Vec2::splat(TILE_ICON)), icon, p.accent_icon);
    let title_h = line_height(ui, &title_font());
    let sub_h = line_height(ui, &text_font());
    let stack = title_h + if lines.is_empty() { 0.0 } else { 2.0 + sub_h * lines.len() as f32 };
    let mut y = center_y(row, stack);
    let g = galley(ui, title, title_font(), p.accent_text, text_w);
    ui.painter().galley(pos2(text_x, y), g, p.accent_text);
    y += title_h + 2.0;
    for line in &lines {
        let g = galley(ui, line, text_font(), p.text_secondary, text_w);
        ui.painter().galley(pos2(text_x, y), g, p.text_secondary);
        y += sub_h;
    }
    height
}

/// `Button.compact` in these windows: `MinWidth`, `Padding="14,5"`, and
/// the global style's `Height="40"`.
pub struct Compact {
    pub min_width: f32,
}

impl Compact {
    pub const HEIGHT: f32 = 40.0;

    fn button(primary: bool) -> Button {
        Button { padding: Thickness::hv(14.0, 5.0), primary, ..Button::default() }
    }

    pub fn width(&self, ui: &Ui, text: &str) -> f32 {
        Self::button(false).width(ui, text).max(self.min_width)
    }

    /// Lays the buttons out right-aligned, `spacing` apart, ending at
    /// `right`, `Self::HEIGHT` tall from `top`; returns each one's rect.
    pub fn row(&self, ui: &Ui, right: f32, top: f32, spacing: f32, labels: &[&str]) -> Vec<Rect> {
        let widths: Vec<f32> = labels.iter().map(|l| self.width(ui, l)).collect();
        let total = widths.iter().sum::<f32>() + spacing * (labels.len().saturating_sub(1)) as f32;
        let mut x = right - total;
        widths
            .iter()
            .map(|w| {
                let r = Rect::from_min_size(pos2(x, top), vec2(*w, Self::HEIGHT));
                x += w + spacing;
                r
            })
            .collect()
    }

    pub fn show(&self, ui: &Ui, rect: Rect, id: Id, text: &str, primary: bool, enabled: bool) -> Response {
        Self::button(primary).show(ui, rect, id, text, enabled)
    }
}

/// A panel card: panel background, 1px border, `radius` corners.
pub fn panel(ui: &Ui, rect: Rect, radius: f32) {
    card(ui, rect, radius);
}

/// The window's `Background` (`ShareX.Brush.Background.Main`).
pub fn background(ui: &Ui) -> Rect {
    let full = ui.max_rect();
    ui.painter().rect_filled(full, 0.0, palette(ui.ctx()).background_main);
    full
}

/// Text `TextBlock` of `font` at `pos` (its line box's top-left).
pub fn text(ui: &Ui, pos: egui::Pos2, s: &str, f: egui::FontId, color: egui::Color32, max_width: f32) {
    let g = galley(ui, s, f, color, max_width);
    ui.painter().galley(pos, g, color);
}

/// The width of `s` in `f`, as Avalonia measures it.
pub fn width(ui: &Ui, s: &str, f: egui::FontId) -> f32 {
    text_width(ui, s, f)
}

/// `ShareX.Brush.Checkered.Small` in `rect` with `radius` corners
/// (`Border.checkered-surface`).
pub fn checkered(ui: &Ui, rect: Rect, radius: f32) {
    let p = palette(ui.ctx());
    border::border(ui.painter(), rect, radius, 0.0, Some(border::Brush::Checkered { colors: p.checkered, cell: 4.0 }), None);
}

/// A `ScrollViewer` over `view` showing `content` pixels: the wheel scrolls
/// it while the pointer is over it. Returns the offset; draw the content,
/// then [`scroll_bar`] over it.
pub fn scroll_offset(ui: &Ui, id: Id, view: Rect, content: f32) -> f32 {
    let max = (content - view.height()).max(0.0);
    let mut offset: f32 = ui.ctx().data(|d| d.get_temp(id)).unwrap_or(0.0);
    if ui.rect_contains_pointer(view) {
        offset -= ui.input(|i| i.smooth_scroll_delta.y);
    }
    offset = offset.clamp(0.0, max);
    ui.ctx().data_mut(|d| d.insert_temp(id, offset));
    offset
}

/// Fluent's collapsed scroll bar: a 2px thumb 4px in from the right, in a
/// track `track_inset` short of each end, over the content.
pub fn scroll_bar(ui: &Ui, view: Rect, content: f32, offset: f32, track_inset: f32) {
    let max = (content - view.height()).max(0.0);
    if max <= 0.0 {
        return;
    }
    let f = sharex_ui::theme::fluent(ui.ctx());
    let track = Rect::from_min_max(pos2(view.max.x - 4.0, view.min.y + track_inset), pos2(view.max.x - 2.0, view.max.y - track_inset));
    let thumb_h = (track.height() * view.height() / content).max(20.0).round();
    let y = track.min.y + ((track.height() - thumb_h) * offset / max).round();
    let thumb = Rect::from_min_size(pos2(track.min.x, y), vec2(2.0, thumb_h));
    ui.painter().rect_filled(thumb, egui::CornerRadius::same(1), f.chrome_disabled_low);
}

/// An `Image` with `Stretch="Uniform"` in `rect`: the image scaled to fit,
/// centred (Avalonia's `Image` rounds its arranged size to pixels).
pub fn uniform(rect: Rect, size: (u32, u32)) -> Rect {
    let (w, h) = (size.0 as f32, size.1 as f32);
    if w <= 0.0 || h <= 0.0 {
        return Rect::from_min_size(rect.center(), Vec2::ZERO);
    }
    // Centred at its exact size, then both edges rounded to pixels.
    let scale = (rect.width() / w).min(rect.height() / h);
    let (sw, sh) = (w * scale, h * scale);
    let x = rect.min.x + (rect.width() - sw) / 2.0;
    let y = rect.min.y + (rect.height() - sh) / 2.0;
    Rect::from_min_max(pos2(x.round(), y.round()), pos2((x + sw).round(), (y + sh).round()))
}
