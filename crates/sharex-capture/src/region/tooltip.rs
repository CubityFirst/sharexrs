//! The capture toolbar's tooltips (`ToolTip.Tip`) as ShareX shows them:
//! Avalonia's `ToolTip` with FluentTheme's control theme (ShareX doesn't
//! restyle it). Values from FluentTheme 12.1.6 (`tools/editor-harness
//! --tooltip-theme` resolves them and lays a `ToolTip` out). Its text is
//! Segoe UI 13 in `ShareX.Brush.Text.Secondary` (ShareX's `TextBlock`
//! style) but Regular: in its popup the style's SemiBold doesn't apply, as
//! in menus; measured on screen, "Rectangle (R, NumPad1)" is 157x32.

use std::time::{Duration, Instant};

use egui::{Color32, Rect, pos2, vec2};
use sharex_ui::border::{Brush, border};
use sharex_ui::{Palette, widgets};

use super::toolbar::Part;

/// `ToolTip.ShowDelay`.
pub const SHOW_DELAY: Duration = Duration::from_millis(400);
/// `ToolTip.BetweenShowDelay`: another tooltip within this of one
/// closing opens at once.
pub const BETWEEN_SHOW_DELAY: Duration = Duration::from_millis(100);
/// The `:open` opacity transition (`DoubleTransition`, 0.15 s).
pub const FADE: Duration = Duration::from_millis(150);
/// `ToolTip.VerticalOffset` with `Placement="Pointer"`.
pub const VERTICAL_OFFSET: f32 = 20.0;
/// `ToolTipBorderThemePadding`: 8,5,8,7.
const PADDING: (f32, f32, f32, f32) = (8.0, 5.0, 8.0, 7.0);
/// `OverlayCornerRadius`.
const RADIUS: f32 = 5.0;
/// `ToolTipContentMaxWidth`.
const MAX_WIDTH: f32 = 320.0;

/// `ToolTipBackground`: `#FF2B2B2B` dark, `#FFF2F2F2` light.
fn background(dark: bool) -> Color32 {
    if dark { Color32::from_rgb(0x2B, 0x2B, 0x2B) } else { Color32::from_rgb(0xF2, 0xF2, 0xF2) }
}

/// `ToolTipBorderBrush`: black at opacity 0.36 dark, 0.14 light.
fn border_color(dark: bool) -> Color32 {
    Color32::from_black_alpha(((if dark { 0.36 } else { 0.14 }) * 255.0f32).round() as u8)
}

/// When the tooltip of the toolbar part under the pointer opens
/// (`ToolTipService`).
#[derive(Clone, Debug, Default)]
pub struct TipState {
    /// The part under the pointer and since when.
    pub hovered: Option<(Part, Instant)>,
    /// The open tooltip's part, where the pointer was (desktop pixels),
    /// and when it opened.
    pub open: Option<(Part, (f64, f64), Instant)>,
    /// When the last tooltip closed.
    pub closed: Option<Instant>,
    /// A press closed it: none until the pointer leaves the part.
    pub suppressed: bool,
}

impl TipState {
    /// The part under the pointer changed.
    pub fn hover(&mut self, part: Option<Part>, now: Instant) {
        if self.hovered.map(|(p, _)| p) == part {
            return;
        }
        self.close(now);
        self.suppressed = false;
        self.hovered = part.filter(|p| has_tip(*p)).map(|p| (p, now));
    }

    pub fn close(&mut self, now: Instant) {
        if self.open.take().is_some() {
            self.closed = Some(now);
        }
    }

    /// A press on the part closes its tooltip until the pointer leaves.
    pub fn press(&mut self, now: Instant) {
        self.close(now);
        self.suppressed = true;
    }

    /// Opens the tooltip when it's due, with the pointer at `pointer`;
    /// returns when to look again.
    pub fn tick(&mut self, now: Instant, pointer: (f64, f64)) -> Option<Instant> {
        if let Some((_, _, at)) = self.open {
            // Frames until one is drawn past the fade (the host redraws
            // only while this asks to).
            return (now <= at + FADE + Duration::from_millis(32)).then(|| now + Duration::from_millis(16));
        }
        let (part, since) = self.hovered?;
        if self.suppressed {
            return None;
        }
        let quick = self.closed.is_some_and(|c| since <= c + BETWEEN_SHOW_DELAY);
        let due = if quick { since } else { since + SHOW_DELAY };
        if now >= due {
            self.open = Some((part, pointer, now));
            Some(now + Duration::from_millis(16))
        } else {
            Some(due)
        }
    }

    /// The fade-in: 0 to 1.
    pub fn opacity(&self, now: Instant) -> f32 {
        self.open.map(|(_, _, at)| (now.duration_since(at).as_secs_f32() / FADE.as_secs_f32()).min(1.0)).unwrap_or(0.0)
    }
}

/// Parts with a `ToolTip.Tip`.
fn has_tip(part: Part) -> bool {
    !matches!(part, Part::Background | Part::Popup)
}

/// The tooltip's size for `text` (`ToolTip` measured: the border, the
/// padding and the text, wrapped at `MaxWidth`).
pub fn size(ui: &egui::Ui, text: &str) -> egui::Vec2 {
    let font = widgets::font(13.0, 400);
    let w = widgets::text_width(ui, text, font.clone()).min(MAX_WIDTH - PADDING.0 - PADDING.2 - 2.0);
    let h = widgets::line_height(ui, &font);
    vec2(w + PADDING.0 + PADDING.2 + 2.0, h + PADDING.1 + PADDING.3 + 2.0)
}

/// Draws the tooltip with `text` at `at` (its top-left, points), kept
/// inside `area` as Avalonia's popup positioner slides it.
pub fn paint(ui: &egui::Ui, p: &Palette, at: egui::Pos2, area: Rect, text: &str, opacity: f32) {
    let dark = ui.ctx().theme() == egui::Theme::Dark;
    let s = size(ui, text);
    let x = at.x.min(area.max.x - s.x).max(area.min.x);
    let y = at.y.min(area.max.y - s.y).max(area.min.y);
    let rect = Rect::from_min_size(pos2(x, y), s);
    let mut painter = ui.painter().clone();
    painter.set_opacity(opacity);
    border(&painter, rect, RADIUS, 1.0, Some(Brush::Solid(background(dark))), Some(border_color(dark)));
    let font = widgets::font(13.0, 400);
    let g = widgets::galley(ui, text, font, p.text_secondary, f32::INFINITY);
    painter.galley(pos2(rect.min.x + 1.0 + PADDING.0, rect.min.y + 1.0 + PADDING.1), g, p.text_secondary);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tooltips_open_after_the_delay_and_at_once_after_another() {
        let t0 = Instant::now();
        let mut s = TipState::default();
        s.hover(Some(Part::RegionButton), t0);
        assert_eq!(s.tick(t0, (1.0, 2.0)), Some(t0 + SHOW_DELAY));
        assert!(s.open.is_none());
        s.tick(t0 + SHOW_DELAY, (1.0, 2.0));
        assert_eq!(s.open.map(|o| o.1), Some((1.0, 2.0)));
        // The next button within the between-show delay: at once.
        let t1 = t0 + Duration::from_secs(1);
        s.hover(Some(Part::Tool(0)), t1);
        s.tick(t1, (5.0, 2.0));
        assert!(s.open.is_some());
        // A press closes it until the pointer leaves.
        s.press(t1);
        s.tick(t1 + Duration::from_secs(1), (5.0, 2.0));
        assert!(s.open.is_none());
        s.hover(Some(Part::Background), t1);
        assert!(s.hovered.is_none());
    }
}
