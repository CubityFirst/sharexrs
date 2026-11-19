//! ShareX's shared controls, each ported from the style it's named after in
//! `ControlStyles.axaml` or the window that defines it.
//!
//! A note on text: ShareX's global `TextBlock` style is **SemiBold**
//! (`ControlStyles.axaml`), so plain text is semibold unless a control sets
//! `FontWeight="Normal"`. [`text`] takes the weight explicitly to keep that
//! visible at every call.

use egui::{
    Align2, Color32, CornerRadius, FontFamily, FontId, Rect, Response, Sense, Stroke, Ui, Vec2, pos2, vec2,
};

use crate::icons;
use crate::theme::{Density, density, palette};

/// Text in ShareX's font at `size` and XAML `weight` (400/600/700).
pub fn font(size: f32, weight: u16) -> FontId {
    FontId::new(size, crate::fonts::weight(weight))
}

pub fn mono(size: f32) -> FontId {
    FontId::new(size, FontFamily::Monospace)
}

/// The height Avalonia gives a line of text in `font`: the font's line
/// height (ascent + descent + line gap), rounded up to whole pixels by
/// layout rounding. Not a galley's height, which egui rounds *down*.
pub fn line_height(ui: &Ui, font: &FontId) -> f32 {
    ui.fonts_mut(|f| f.row_height(font)).ceil()
}

/// The width Avalonia measures `text` at: rounded up to whole pixels.
pub fn text_width(ui: &Ui, text: &str, font: FontId) -> f32 {
    galley(ui, text, font, Color32::WHITE, f32::INFINITY).size().x.ceil()
}

/// Lays out `text` on one line, trimmed with "…" to `max_width`
/// (`TextTrimming="CharacterEllipsis"`).
pub fn galley(ui: &Ui, text: &str, font: FontId, color: Color32, max_width: f32) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_owned(), font, color);
    job.wrap = egui::text::TextWrapping::truncate_at_width(max_width);
    ui.fonts_mut(|f| f.layout_job(job))
}

/// Avalonia's layout rounding (`LayoutHelper.RoundLayoutValue`): a size or
/// position to whole device pixels, `Math.Round(value * scale) / scale`
/// (ties to even). At 125% a 430-wide column is 538 pixels, 430.4 points.
pub fn layout_round(ctx: &egui::Context, value: f32) -> f32 {
    let ppp = ctx.pixels_per_point();
    (value * ppp).round_ties_even() / ppp
}

/// Where Avalonia puts the top of a line of text of `height` centred
/// (`VerticalAlignment="Center"`) in `rect`: the line box is rounded up to
/// whole pixels and its offset rounded half-to-even, as layout rounding does.
pub fn center_y(rect: Rect, height: f32) -> f32 {
    rect.min.y + ((rect.height() - height.ceil()) / 2.0).round_ties_even()
}

/// Paints `text` with its left edge at `x`, vertically centred in `rect`.
pub fn text_left(ui: &Ui, rect: Rect, x: f32, text: &str, font: FontId, color: Color32) {
    let y = center_y(rect, line_height(ui, &font));
    let g = galley(ui, text, font, color, (rect.max.x - x).max(0.0));
    ui.painter().galley(pos2(x, y), g, color);
}

/// A 1px `Separator` across `ui`'s width, inside `margin` (`h`, `v`).
pub fn separator(ui: &mut Ui, h: f32, v: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0 + 2.0 * v), Sense::hover());
    let y = rect.min.y + v;
    let line = Rect::from_min_max(pos2(rect.min.x + h, y), pos2(rect.max.x - h, y + 1.0));
    ui.painter().rect_filled(line, 0.0, palette(ui.ctx()).separator);
}

/// `Button.nav-button`: the main window's sidebar entries
/// (`MainWindow.CreateNavigationContent`): a 16px accent icon in a 20px
/// column, the label at Normal weight, and a 14px chevron at 70% opacity
/// for entries with a submenu.
pub fn nav_button(ui: &mut Ui, icon: &str, label: &str, chevron: bool) -> Response {
    let d: Density = density(ui.ctx());
    let p = palette(ui.ctx());
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), d.nav_button_height), Sense::click());
    if response.hovered() || response.is_pointer_button_down_on() {
        ui.painter().rect_filled(rect, CornerRadius::same(4), p.control_background_hover);
    }
    let pad = d.nav_button_padding;
    let inner = Rect::from_min_max(
        pos2(rect.min.x + pad.left, rect.min.y + pad.top),
        pos2(rect.max.x - pad.right, rect.max.y - pad.bottom),
    );
    // Column 0, 20 wide: the icon, centred.
    let icon_rect = Rect::from_center_size(pos2(inner.min.x + 10.0, inner.center().y), Vec2::splat(16.0));
    icons::paint(ui.painter(), icon_rect, icon, p.accent_icon);
    let mut right = inner.max.x;
    if chevron {
        let c = Rect::from_center_size(pos2(inner.max.x - 7.0, inner.center().y), Vec2::splat(14.0));
        icons::paint(ui.painter(), c, "app/chevron-right", p.accent_icon.gamma_multiply(0.7));
        right = c.min.x - 5.0;
    }
    let x = inner.min.x + 20.0 + d.nav_label_margin.left;
    let label_rect = Rect::from_min_max(pos2(x, inner.min.y), pos2(right, inner.max.y));
    text_left(ui, label_rect, x, label, font(crate::theme::FONT_SIZE, 400), p.text_secondary);
    response.on_hover_cursor(egui::CursorIcon::Default)
}

/// Which caption button, for its hover colours.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Caption {
    Normal,
    /// `Button.caption-close`: red on hover, with the window's top-right
    /// corner rounded unless maximized.
    Close { maximized: bool },
}

/// `Button.caption-button`: 46x38, a 16px icon (20px for the theme toggle).
pub fn caption_button(ui: &mut Ui, icon: &str, icon_size: f32, kind: Caption) -> Response {
    let p = palette(ui.ctx());
    let (rect, response) = ui.allocate_exact_size(vec2(46.0, 38.0), Sense::click());
    let pressed = response.is_pointer_button_down_on();
    let hovered = response.hovered();
    let (bg, fg) = match kind {
        Caption::Close { .. } if pressed => (Some(Color32::from_rgb(0xA5, 0x1D, 0x12)), Color32::WHITE),
        Caption::Close { .. } if hovered => (Some(Color32::from_rgb(0xC4, 0x2B, 0x1C)), Color32::WHITE),
        _ if pressed => (Some(p.control_background), p.text),
        _ if hovered => (Some(p.control_background_hover), p.text),
        _ => (None, p.text),
    };
    if let Some(bg) = bg {
        let radius = match kind {
            Caption::Close { maximized: false } => CornerRadius { ne: 7, ..Default::default() },
            _ => CornerRadius::ZERO,
        };
        ui.painter().rect_filled(rect, radius, bg);
    }
    icons::paint(ui.painter(), Rect::from_center_size(rect.center(), Vec2::splat(icon_size)), icon, fg);
    response
}

/// A `Border` with ShareX's panel look (`Border.hotkey-panel`): panel fill,
/// 1px border, rounded `radius`.
pub fn panel_frame(ui: &Ui, radius: u8, padding: f32) -> egui::Frame {
    let p = palette(ui.ctx());
    egui::Frame::new()
        .fill(p.background_panel)
        .stroke(Stroke::new(1.0, p.border))
        .corner_radius(CornerRadius::same(radius))
        .inner_margin(egui::Margin::same(padding as i8))
}

/// Paints `text` centred on `pos`.
pub fn text_centered(ui: &Ui, pos: egui::Pos2, text: &str, font: FontId, color: Color32) -> Rect {
    ui.painter().text(pos, Align2::CENTER_CENTER, text, font, color)
}

