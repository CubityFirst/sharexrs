//! A tooltip in a popup window of its own, as Avalonia opens `ToolTip`s
//! (a `PopupRoot`), for the windows too small to hold theirs: the task
//! toast's buttons and the Actions toolbar (`PlacementMode.Top`,
//! `VerticalOffset="-4"`). Borderless, topmost, never activated, the
//! pointer passing through it; it fades in over `ToolTip`'s 0.15 s.
//!
//! The owner asks with [`super::Effect::Tooltip`] (the text and the element
//! in its own client area); the app places this window above the element,
//! slid onto the element's monitor, flipped below when there's no room.

use std::time::Instant;

use egui::{Rect, vec2};

use super::{Cx, Dialog, Effect, Spec};

pub struct TooltipWindow {
    pub text: String,
    /// The element, in desktop pixels, and the scale it's at.
    pub anchor: sharex_capture::Rect,
    pub scale: f32,
    pub dark: bool,
    shown: Option<Instant>,
}

impl TooltipWindow {
    pub fn new(text: String, anchor: sharex_capture::Rect, scale: f32, dark: bool) -> Self {
        Self { text, anchor, scale, dark, shown: None }
    }
}

/// `PlacementMode.Top` with `VerticalOffset` -4: centred above the element,
/// below it when the monitor has no room above, kept on the monitor.
pub fn place_top(anchor: sharex_capture::Rect, size: (u32, u32), offset: i32, monitor: sharex_capture::Rect) -> (i32, i32) {
    let (w, h) = (size.0 as i32, size.1 as i32);
    let mut x = anchor.x + (anchor.w - w) / 2;
    let mut y = anchor.y - offset - h;
    if y < monitor.y {
        y = anchor.y + anchor.h + offset;
    }
    x = x.min(monitor.x + monitor.w - w).max(monitor.x);
    y = y.min(monitor.y + monitor.h - h).max(monitor.y);
    (x, y)
}

impl Dialog for TooltipWindow {
    fn spec(&self) -> Spec {
        Spec { topmost: true, skip_taskbar: true, maximizable: false, activate: false, borderless: true, click_through: true, ..Spec::fit_height("", 1.0, 1.0) }
    }

    fn theme(&self) -> Option<egui::Theme> {
        Some(if self.dark { egui::Theme::Dark } else { egui::Theme::Light })
    }

    fn content_width(&mut self, ui: &mut egui::Ui) -> Option<f32> {
        Some(sharex_ui::tooltip::size(ui, &self.text).x)
    }

    fn content_height(&mut self, ui: &mut egui::Ui) -> f32 {
        sharex_ui::tooltip::size(ui, &self.text).y
    }

    fn opacity(&self) -> f32 {
        self.shown.map_or(0.0, |s| (s.elapsed().as_secs_f64() / sharex_ui::tooltip::FADE).min(1.0) as f32)
    }

    fn position(&self, _work_area: sharex_capture::Rect, size: (u32, u32)) -> Option<(i32, i32)> {
        let screens = sharex_capture::screenshot::screens();
        let (cx, cy) = (self.anchor.x + self.anchor.w / 2, self.anchor.y + self.anchor.h / 2);
        let monitor = screens.iter().find(|s| s.bounds.contains(cx, cy)).or(screens.first()).map_or(self.anchor, |s| s.bounds);
        Some(place_top(self.anchor, size, (4.0 * self.scale).round() as i32, monitor))
    }

    fn ui(&mut self, ui: &mut egui::Ui, _cx: &mut Cx) -> Vec<Effect> {
        let shown = *self.shown.get_or_insert_with(Instant::now);
        if shown.elapsed().as_secs_f64() < sharex_ui::tooltip::FADE {
            ui.ctx().request_repaint();
        }
        let full = ui.max_rect();
        let size = sharex_ui::tooltip::size(ui, &self.text);
        let rect = Rect::from_min_size(full.min, vec2(size.x.min(full.width()), size.y.min(full.height())));
        sharex_ui::tooltip::paint(ui.painter(), ui, rect, &self.text, 1.0);
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tooltip_goes_above_its_element_or_below_at_the_top() {
        let monitor = sharex_capture::Rect::new(0, 0, 1920, 1080);
        // A 28x28 button at (1700, 1000): a 100x32 tip centred 4 above it.
        assert_eq!(place_top(sharex_capture::Rect::new(1700, 1000, 28, 28), (100, 32), 4, monitor), (1664, 964));
        // At the top of the monitor: below it instead.
        assert_eq!(place_top(sharex_capture::Rect::new(10, 5, 28, 28), (100, 32), 4, monitor), (0, 37));
        // At the right edge: slid left.
        assert_eq!(place_top(sharex_capture::Rect::new(1900, 500, 20, 20), (100, 32), 4, monitor), (1820, 464));
    }
}
