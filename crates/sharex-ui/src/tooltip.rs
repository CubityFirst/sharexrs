//! `ToolTip.Tip` as ShareX shows it: Avalonia's `ToolTip` with FluentTheme's
//! control theme (ShareX doesn't restyle it). FluentTheme 12.1.6's values,
//! as `sharex_capture::region::tooltip` resolved them (`tools/editor-harness
//! --tooltip-theme`): `#FF2B2B2B`/`#FFF2F2F2` with a black border at 0.36/0.14,
//! padding 8,5,8,7, corners 5, `MaxWidth` 320, `ShowDelay` 400 ms,
//! `BetweenShowDelay` 100 ms, a 0.15 s fade in, opened at the pointer 20 px
//! down (`Placement="Pointer"`, `VerticalOffset`). Its text is ShareX's
//! `TextBlock` style (Segoe UI 13, `ShareX.Brush.Text.Secondary`) but
//! Regular, as in every popup, wrapped at the width it has.
//!
//! [`show`] keeps the hover state in egui's memory, so any widget can have
//! one: call it every frame with the element's rect.

use egui::{Color32, Id, Order, Pos2, Rect, Ui, Vec2, pos2, vec2};

use crate::border::{Brush, border};
use crate::theme::palette;
use crate::widgets;

/// `ToolTip.ShowDelay`, seconds.
pub const SHOW_DELAY: f64 = 0.4;
/// `ToolTip.BetweenShowDelay`, seconds.
pub const BETWEEN_SHOW_DELAY: f64 = 0.1;
/// The `:open` opacity transition, seconds.
pub const FADE: f64 = 0.15;
/// `ToolTip.VerticalOffset` with `Placement="Pointer"`.
pub const VERTICAL_OFFSET: f32 = 20.0;
/// `ToolTipBorderThemePadding`: 8,5,8,7.
pub const PADDING: (f32, f32, f32, f32) = (8.0, 5.0, 8.0, 7.0);
/// `OverlayCornerRadius`.
pub const RADIUS: f32 = 5.0;
/// `ToolTipContentMaxWidth`.
pub const MAX_WIDTH: f32 = 320.0;

/// `ToolTipBackground`.
pub fn background(dark: bool) -> Color32 {
    if dark { Color32::from_rgb(0x2B, 0x2B, 0x2B) } else { Color32::from_rgb(0xF2, 0xF2, 0xF2) }
}

/// `ToolTipBorderBrush`.
pub fn border_color(dark: bool) -> Color32 {
    Color32::from_black_alpha(((if dark { 0.36 } else { 0.14 }) * 255.0f32).round() as u8)
}

fn text_font() -> egui::FontId {
    widgets::font(13.0, 400)
}

/// The text laid out as the tooltip wraps it.
fn layout(ui: &Ui, text: &str, color: Color32) -> std::sync::Arc<egui::Galley> {
    let wrap = MAX_WIDTH - PADDING.0 - PADDING.2 - 2.0;
    let mut job = egui::text::LayoutJob::simple(text.to_owned(), text_font(), color, wrap);
    job.wrap.max_width = wrap;
    ui.fonts_mut(|f| f.layout_job(job))
}

/// The tooltip's size for `text`: border, padding and the text (its lines
/// at the font's exact line height, rounded up once).
pub fn size(ui: &Ui, text: &str) -> Vec2 {
    let g = layout(ui, text, Color32::WHITE);
    let row = ui.fonts_mut(|f| f.row_height(&text_font()));
    let lines = g.rows.len().max(1) as f32;
    let w = g.size().x.ceil();
    vec2(w + PADDING.0 + PADDING.2 + 2.0, (row * lines).ceil() + PADDING.1 + PADDING.3 + 2.0)
}

/// Where a tooltip of `size` opened at `pointer` goes: 20 px below the
/// pointer, slid to stay inside `area` (Avalonia's popup positioner).
pub fn place(pointer: Pos2, size: Vec2, area: Rect) -> Rect {
    let at = pos2(pointer.x, pointer.y + VERTICAL_OFFSET);
    let x = at.x.min(area.max.x - size.x).max(area.min.x);
    let y = if at.y + size.y > area.max.y { (pointer.y - size.y).max(area.min.y) } else { at.y };
    Rect::from_min_size(pos2(x, y), size)
}

/// Paints the tooltip in `rect` at `opacity`.
pub fn paint(painter: &egui::Painter, ui: &Ui, rect: Rect, text: &str, opacity: f32) {
    let dark = ui.ctx().theme() == egui::Theme::Dark;
    let p = palette(ui.ctx());
    let mut painter = painter.clone();
    painter.set_opacity(opacity);
    border(&painter, rect, RADIUS, 1.0, Some(Brush::Solid(background(dark))), Some(border_color(dark)));
    let g = layout(ui, text, p.text_secondary);
    painter.galley(pos2(rect.min.x + 1.0 + PADDING.0, rect.min.y + 1.0 + PADDING.1), g, p.text_secondary);
}

/// Hover state kept across frames.
#[derive(Clone, Copy, Debug, Default)]
struct State {
    /// The element under the pointer and since when (`ui.input().time`).
    hovered: Option<(Id, f64)>,
    /// The open tooltip: its element, where the pointer was, when it opened.
    open: Option<(Id, Pos2, f64)>,
    /// When the last one closed.
    closed: Option<f64>,
    /// The frame `hovered` was last confirmed in.
    seen: u64,
}

fn state_id() -> Id {
    Id::new("sharex-tooltip-state")
}

/// What to do this frame for an element hovered since `since`.
#[derive(Debug, PartialEq)]
pub enum Due {
    /// Open now.
    Open,
    /// Look again in this many seconds.
    In(f64),
}

/// `ToolTipService`'s timing: the delay, or none right after another
/// tooltip closed (`BetweenShowDelay`).
pub fn due(now: f64, since: f64, last_closed: Option<f64>) -> Due {
    let quick = last_closed.is_some_and(|c| since <= c + BETWEEN_SHOW_DELAY);
    let at = if quick { since } else { since + SHOW_DELAY };
    if now >= at { Due::Open } else { Due::In(at - now) }
}

/// The tooltip of the element in `rect` (`id`): shown once the pointer has
/// rested on it for the delay, until it leaves or a button is pressed. An
/// empty `text` shows nothing (`ToolTip.Tip` bound to null).
pub fn show(ui: &Ui, rect: Rect, id: Id, text: &str) {
    let ctx = ui.ctx();
    let frame = ctx.cumulative_frame_nr();
    let (now, pointer, pressed) = ui.input(|i| (i.time, i.pointer.latest_pos(), i.pointer.any_pressed()));
    let over = !text.is_empty()
        && pointer.is_some_and(|p| rect.contains(p) && ui.clip_rect().contains(p))
        && ctx.layer_id_at(pointer.unwrap_or_default()).is_none_or(|l| l == ui.layer_id());
    let mut s: State = ctx.data(|d| d.get_temp(state_id())).unwrap_or_default();
    // An element not drawn last frame isn't hovered any more.
    if s.hovered.is_some() && s.seen + 1 < frame {
        if s.open.take().is_some() {
            s.closed = Some(now);
        }
        s.hovered = None;
    }
    if over {
        if s.hovered.map(|(h, _)| h) != Some(id) {
            if s.open.take().is_some() {
                s.closed = Some(now);
            }
            s.hovered = Some((id, now));
        }
        s.seen = frame;
        if pressed && s.open.is_some_and(|(o, ..)| o == id) {
            s.open = None;
            s.closed = Some(now);
            // Closed by the press until the pointer leaves.
            s.hovered = Some((id, f64::INFINITY));
        }
    } else if s.hovered.is_some_and(|(h, _)| h == id) {
        s.hovered = None;
        if s.open.take().is_some() {
            s.closed = Some(now);
        }
    }
    if let (Some((h, since)), None) = (s.hovered, s.open)
        && h == id
        && since.is_finite()
    {
        match due(now, since, s.closed) {
            Due::Open => s.open = Some((id, pointer.unwrap_or(rect.center()), now)),
            Due::In(t) => ctx.request_repaint_after(std::time::Duration::from_secs_f64(t)),
        }
    }
    ctx.data_mut(|d| d.insert_temp(state_id(), s));
    let Some((o, at, opened)) = s.open else { return };
    if o != id {
        return;
    }
    let t = ((now - opened) / FADE).clamp(0.0, 1.0) as f32;
    if t < 1.0 {
        ctx.request_repaint();
    }
    let size = size(ui, text);
    let rect = place(at, size, ctx.content_rect());
    let painter = ctx.layer_painter(egui::LayerId::new(Order::Tooltip, id.with("tooltip")));
    paint(&painter, ui, rect, text, t);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tooltips_open_after_the_delay_and_at_once_after_another() {
        assert_eq!(due(0.0, 0.0, None), Due::In(SHOW_DELAY));
        assert_eq!(due(1.41, 1.0, None), Due::Open);
        // Another hovered within 100 ms of one closing: at once.
        assert_eq!(due(2.05, 2.05, Some(2.0)), Due::Open);
        assert!(matches!(due(2.2, 2.2, Some(2.0)), Due::In(_)));
    }

    #[test]
    fn a_tooltip_stays_in_its_area() {
        let area = Rect::from_min_size(Pos2::ZERO, vec2(400.0, 300.0));
        assert_eq!(place(pos2(10.0, 10.0), vec2(100.0, 30.0), area).min, pos2(10.0, 30.0));
        // Slid left at the right edge, flipped above at the bottom.
        let r = place(pos2(380.0, 290.0), vec2(100.0, 30.0), area);
        assert_eq!(r.min, pos2(300.0, 260.0));
    }
}
