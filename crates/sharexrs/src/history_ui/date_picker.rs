//! Fluent's `DatePicker` flyout (`DatePickerPresenter`), as the History
//! window's advanced filters open it. Values from Avalonia 12.1.6 with
//! ShareX's theme, laid out headlessly (`tools/editor-harness
//! --control-dump datepicker|datepicker-presenter`):
//!
//! - 296 x 398, `#2B2B2B`/`#F2F2F2`, a 1 px border of black at 0.36/0.14,
//!   corners 5 (the tooltip's overlay brushes);
//! - the picker area 355 tall: day (78), a 1 px spacer, month (137, items
//!   padded 9 from the left), a spacer, year (77); items 40 tall
//!   (`DatePickerFlyoutPresenterItemHeight`), padded 0,3,0,6, the selected
//!   one at y 159 under the highlight (`SystemAccentColor` at 0.6 dark, 0.4
//!   light), spacers `#33FFFFFF`/`#33000000`;
//! - a spacer line and the accept/dismiss buttons (147 x 41 each,
//!   corners 4, padding 10,6) with their 0.75 px round-capped glyphs
//!   (`M0.5,8.5 5,13.5 15.5,3`, `M2,2 14,14 M2,14 14,2`);
//! - opened `AnchorAndGravity` Bottom/Bottom 193.5 px up from the picker's
//!   bottom edge, so the selected row sits over the picker.
//!
//! Day and month loop (`ShouldLoop`); the wheel moves a column one item a
//! notch (`ScrollSize` = the item height), a click picks an item, Enter or
//! the accept button commits, Escape, the dismiss button or a press outside
//! closes it unchanged.
//!
//! The column under the pointer (`PART_*Host:pointerover`) shows its up and
//! down `RepeatButton`s (`--control-dump datepicker-presenter-hover`): 22
//! tall across the column at its top and bottom, the presenter's
//! background, `DateTimePickerFlyoutButtonBackgroundPointerOver`/`Pressed`
//! (`#19`/`#33` white or black) over it, a 10x5 `Viewbox` of an 18x9
//! chevron (`M 0,9 L 9,0 L 18,9`, `M 0,0 L 9,9 L 18,0`, 1 px, flat caps)
//! in the foreground; `Delay` 300 and `Interval` 100 ms: held, it scrolls
//! after the delay and every interval, and the release on it scrolls once
//! more (`ScrollUp`/`ScrollDown`: one item).

use chrono::{Datelike, NaiveDate};
use egui::{Color32, Id, Order, Pos2, Rect, Stroke, Ui, pos2, vec2};
use sharex_ui::border::{Brush, border};
use sharex_ui::theme::palette;
use sharex_ui::widgets::{center_y, font, galley, line_height, text_width};

pub const WIDTH: f32 = 296.0;
pub const HEIGHT: f32 = 398.0;
const PICKER_HEIGHT: f32 = 355.0;
const ITEM_HEIGHT: f32 = 40.0;
/// The selected row's top, from the presenter's top.
const HIGHLIGHT_TOP: f32 = 159.0;
const BUTTONS_HEIGHT: f32 = 41.0;
/// The popup's `VerticalOffset` from the picker's bottom edge.
const POPUP_OFFSET: f32 = -193.5;
/// The columns: x from the presenter's left, width.
const DAY: (f32, f32) = (1.0, 78.0);
const MONTH: (f32, f32) = (80.0, 137.0);
const YEAR: (f32, f32) = (218.0, 77.0);
/// The up/down `RepeatButton`s' height, and their `Delay`/`Interval`.
const SCROLL_BUTTON_HEIGHT: f32 = 22.0;
const REPEAT_DELAY: f64 = 0.3;
const REPEAT_INTERVAL: f64 = 0.1;

/// What the flyout did this frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Out {
    Open,
    Accept(NaiveDate),
    Dismiss,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Column {
    Day,
    Month,
    Year,
}

pub struct DateFlyout {
    pub id: Id,
    /// The picker the flyout belongs to.
    pub anchor: Rect,
    pub year: i32,
    pub month: u32,
    pub day: u32,
    /// The press that opened it: its release isn't a pick.
    opened_this_press: bool,
    /// An up (-1) or down (1) button held: its column and when it repeats next.
    repeat: Option<(Column, i32, f64)>,
}

pub fn days_in_month(year: i32, month: u32) -> u32 {
    let (y, m) = if month == 12 { (year + 1, 1) } else { (year, month + 1) };
    NaiveDate::from_ymd_opt(y, m, 1).and_then(|d| d.pred_opt()).map_or(31, |d| d.day())
}

impl DateFlyout {
    pub fn new(id: Id, anchor: Rect, date: NaiveDate) -> Self {
        Self { id, anchor, year: date.year(), month: date.month(), day: date.day(), opened_this_press: true, repeat: None }
    }

    pub fn date(&self) -> NaiveDate {
        NaiveDate::from_ymd_opt(self.year, self.month, self.day.min(days_in_month(self.year, self.month))).unwrap_or_default()
    }

    /// Where the presenter goes, kept inside `bounds` (a popup of its own in
    /// ShareX; here it slides as the window's menus do).
    pub fn rect(&self, bounds: Rect) -> Rect {
        let y = (self.anchor.max.y + POPUP_OFFSET).round_ties_even();
        let x = self.anchor.min.x.min(bounds.max.x - WIDTH).max(bounds.min.x);
        let y = y.min(bounds.max.y - HEIGHT).max(bounds.min.y);
        Rect::from_min_size(pos2(x, y), vec2(WIDTH, HEIGHT))
    }

    /// Moves `column` by `steps` items: day and month loop, the year stops
    /// at its ends (1 and 9999); a day past the month's end is clamped.
    fn step(&mut self, column: Column, steps: i32) {
        match column {
            Column::Day => {
                let n = days_in_month(self.year, self.month) as i32;
                self.day = ((self.day as i32 - 1 + steps).rem_euclid(n) + 1) as u32;
            }
            Column::Month => {
                self.month = ((self.month as i32 - 1 + steps).rem_euclid(12) + 1) as u32;
            }
            Column::Year => self.year = (self.year + steps).clamp(1, 9999),
        }
        self.day = self.day.min(days_in_month(self.year, self.month));
    }

    fn column_at(r: Rect, pos: Pos2) -> Option<Column> {
        let local = pos - r.min;
        if local.y < 1.0 || local.y >= 1.0 + PICKER_HEIGHT {
            return None;
        }
        [(Column::Day, DAY), (Column::Month, MONTH), (Column::Year, YEAR)]
            .into_iter()
            .find(|(_, (x, w))| local.x >= *x && local.x < x + w)
            .map(|(c, _)| c)
    }

    /// `column`'s up (-1) and down (1) buttons, in a presenter at `r`.
    fn scroll_buttons(r: Rect, column: Column) -> [(i32, Rect); 2] {
        let (x, w) = match column {
            Column::Day => DAY,
            Column::Month => MONTH,
            Column::Year => YEAR,
        };
        let up = Rect::from_min_size(pos2(r.min.x + x, r.min.y + 1.0), vec2(w, SCROLL_BUTTON_HEIGHT));
        let down = Rect::from_min_size(pos2(r.min.x + x, r.min.y + 1.0 + PICKER_HEIGHT - SCROLL_BUTTON_HEIGHT), vec2(w, SCROLL_BUTTON_HEIGHT));
        [(-1, up), (1, down)]
    }

    /// The up/down button under `pos`, if its column is the hovered one.
    fn scroll_button_at(r: Rect, pos: Pos2) -> Option<(Column, i32)> {
        let column = Self::column_at(r, pos)?;
        Self::scroll_buttons(r, column).into_iter().find(|(_, b)| b.contains(pos)).map(|(dir, _)| (column, dir))
    }

    /// `RepeatButton`: a press arms it (`StartTimer`), each tick after the
    /// delay and every interval scrolls while it's still pressed, and the
    /// release over it (`Button.Click`) scrolls once more.
    fn repeat_tick(&mut self, now: f64, pointer: Option<Pos2>, r: Rect) -> Option<f64> {
        let (column, dir, next) = self.repeat?;
        let over = pointer.and_then(|p| Self::scroll_button_at(r, p)) == Some((column, dir));
        if now < next {
            return Some(next);
        }
        if over {
            self.step(column, dir);
        }
        let next = next + REPEAT_INTERVAL;
        self.repeat = Some((column, dir, next));
        Some(next)
    }

    /// The item `offset` rows from the selected one in `column`, if any.
    fn item(&self, column: Column, offset: i32) -> Option<String> {
        match column {
            Column::Day => {
                let n = days_in_month(self.year, self.month) as i32;
                Some(((self.day as i32 - 1 + offset).rem_euclid(n) + 1).to_string())
            }
            Column::Month => {
                let names = &sharex_helpers::date_format::CultureFormat::current().month_names;
                let m = (self.month as i32 - 1 + offset).rem_euclid(12) as usize;
                names.get(m).cloned()
            }
            Column::Year => {
                let y = self.year + offset;
                (1..=9999).contains(&y).then(|| y.to_string())
            }
        }
    }

    pub fn show(&mut self, ui: &Ui, bounds: Rect) -> Out {
        let r = self.rect(bounds);
        let dark = ui.ctx().theme() == egui::Theme::Dark;
        let p = palette(ui.ctx());
        let layer = egui::LayerId::new(Order::Foreground, self.id);
        let painter = ui.ctx().layer_painter(layer);
        border(&painter, r, 5.0, 1.0, Some(Brush::Solid(sharex_ui::tooltip::background(dark))), Some(sharex_ui::tooltip::border_color(dark)));
        let spacer = if dark { Color32::from_rgba_unmultiplied(255, 255, 255, 0x33) } else { Color32::from_rgba_unmultiplied(0, 0, 0, 0x33) };
        let (pointer, pressed, released, wheel, keys) = ui.input(|i| {
            let wheel: f32 = i
                .events
                .iter()
                .filter_map(|e| match e {
                    egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta, .. } => Some(delta.y / 50.0),
                    egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Page, delta, .. } => Some(delta.y * 3.0),
                    egui::Event::MouseWheel { delta, .. } => Some(delta.y),
                    _ => None,
                })
                .sum();
            let keys: Vec<egui::Key> = i
                .events
                .iter()
                .filter_map(|e| match e {
                    egui::Event::Key { key, pressed: true, .. } => Some(*key),
                    _ => None,
                })
                .collect();
            (i.pointer.latest_pos(), i.pointer.primary_pressed(), i.pointer.primary_released(), wheel, keys)
        });
        let picker = Rect::from_min_size(pos2(r.min.x + 1.0, r.min.y + 1.0), vec2(WIDTH - 2.0, PICKER_HEIGHT));
        let clip = painter.with_clip_rect(picker);
        // The highlight under the selected row.
        let accent = p.accent_start;
        let highlight = Rect::from_min_size(pos2(picker.min.x, r.min.y + HIGHLIGHT_TOP), vec2(picker.width(), ITEM_HEIGHT));
        clip.rect_filled(highlight, 0.0, accent.gamma_multiply(if dark { 0.6 } else { 0.4 }));
        let hover_col = pointer.and_then(|pos| Self::column_at(r, pos));
        let f = font(13.0, 400);
        let lh = line_height(ui, &f);
        let fg = p.text_secondary;
        for (column, (x, w)) in [(Column::Day, DAY), (Column::Month, MONTH), (Column::Year, YEAR)] {
            for offset in -5..=5 {
                let top = r.min.y + HIGHLIGHT_TOP + offset as f32 * ITEM_HEIGHT;
                let item = Rect::from_min_size(pos2(r.min.x + x, top), vec2(w, ITEM_HEIGHT));
                if !item.intersects(picker) {
                    continue;
                }
                let Some(text) = self.item(column, offset) else { continue };
                if offset != 0 && hover_col == Some(column) && pointer.is_some_and(|pos| item.contains(pos)) {
                    // `ListBoxItem:pointerover`: `SystemControlHighlightListLowBrush`.
                    let low = if dark { Color32::from_rgba_unmultiplied(255, 255, 255, 0x19) } else { Color32::from_rgba_unmultiplied(0, 0, 0, 0x19) };
                    clip.rect_filled(item, 0.0, low);
                }
                // Padding 0,3,0,6 (month 9,3,0,6), the text centred in what's left.
                let content = Rect::from_min_max(pos2(item.min.x, item.min.y + 3.0), pos2(item.max.x, item.max.y - 6.0));
                let ty = center_y(content, lh);
                let tx = if column == Column::Month {
                    item.min.x + 9.0
                } else {
                    item.min.x + ((w - text_width(ui, &text, f.clone())) / 2.0).round_ties_even()
                };
                clip.galley(pos2(tx, ty), galley(ui, &text, f.clone(), fg, w), fg);
            }
        }
        for x in [DAY.0 + DAY.1, MONTH.0 + MONTH.1] {
            painter.rect_filled(Rect::from_min_size(pos2(r.min.x + x, picker.min.y), vec2(1.0, PICKER_HEIGHT)), 0.0, spacer);
        }
        // The hovered column's up/down buttons.
        let down_now = ui.input(|i| i.pointer.primary_down());
        if let Some(column) = hover_col {
            let bg = sharex_ui::tooltip::background(dark);
            let glyph = if dark { Color32::WHITE } else { Color32::BLACK };
            for (dir, b) in Self::scroll_buttons(r, column) {
                painter.rect_filled(b, 0.0, bg);
                if pointer.is_some_and(|pos| b.contains(pos)) {
                    let alpha = if down_now && self.repeat.is_some_and(|(c, d, _)| c == column && d == dir) { 0x33 } else { 0x19 };
                    let over = if dark { Color32::from_rgba_unmultiplied(255, 255, 255, alpha) } else { Color32::from_rgba_unmultiplied(0, 0, 0, alpha) };
                    painter.rect_filled(b, 0.0, over);
                }
                // The 18x9 chevron scaled into its 10x5 `Viewbox`, centred
                // (Avalonia's layout rounding).
                let s = 10.0 / 18.0;
                let o = pos2(b.min.x + ((b.width() - 10.0) / 2.0).round_ties_even(), b.min.y + ((b.height() - 5.0) / 2.0).round_ties_even());
                let pts = if dir < 0 { [(0.0, 9.0), (9.0, 0.0), (18.0, 9.0)] } else { [(0.0, 0.0), (9.0, 9.0), (18.0, 0.0)] };
                painter.add(egui::Shape::line(pts.iter().map(|(x, y)| o + vec2(x * s, y * s)).collect(), Stroke::new(s, glyph)));
            }
        }
        // The accept/dismiss row.
        let row_top = picker.max.y;
        painter.rect_filled(Rect::from_min_size(pos2(picker.min.x, row_top), vec2(picker.width(), 1.0)), 0.0, spacer);
        let accept = Rect::from_min_size(pos2(r.min.x + 1.0, row_top), vec2(147.0, BUTTONS_HEIGHT));
        let dismiss = Rect::from_min_size(pos2(r.min.x + 148.0, row_top), vec2(147.0, BUTTONS_HEIGHT));
        let down = ui.input(|i| i.pointer.primary_down());
        for b in [accept, dismiss] {
            if pointer.is_some_and(|pos| b.contains(pos)) {
                // `ButtonBackgroundPointerOver` / `Pressed`.
                let c = match (down, dark) {
                    (false, true) => Color32::from_rgba_unmultiplied(255, 255, 255, 26),
                    (false, false) => Color32::from_rgba_unmultiplied(0, 0, 0, 26),
                    (true, true) => Color32::from_rgba_unmultiplied(255, 255, 255, 0x66),
                    (true, false) => Color32::from_rgba_unmultiplied(0, 0, 0, 0x66),
                };
                border(&painter, b, 4.0, 0.0, Some(Brush::Solid(c)), None);
            }
        }
        let stroke = Stroke::new(0.75, p.text);
        let o = pos2(r.min.x + 67.0, r.min.y + 370.0);
        painter.add(egui::Shape::line(vec![o + vec2(0.5, 8.5), o + vec2(5.0, 13.5), o + vec2(15.5, 3.0)], stroke));
        let o = pos2(r.min.x + 214.0, r.min.y + 370.0);
        painter.line_segment([o + vec2(2.0, 2.0), o + vec2(14.0, 14.0)], stroke);
        painter.line_segment([o + vec2(2.0, 14.0), o + vec2(14.0, 2.0)], stroke);

        // Input.
        if wheel != 0.0
            && let Some(c) = hover_col
        {
            let steps = -(wheel.round() as i32);
            if steps != 0 {
                self.step(c, steps);
            }
        }
        for k in keys {
            match k {
                egui::Key::Escape => return Out::Dismiss,
                egui::Key::Enter => return Out::Accept(self.date()),
                _ => {}
            }
        }
        if pressed && pointer.is_some_and(|pos| !r.contains(pos)) {
            return Out::Dismiss;
        }
        let now = ui.input(|i| i.time);
        if pressed
            && !self.opened_this_press
            && let Some((column, dir)) = pointer.and_then(|pos| Self::scroll_button_at(r, pos))
        {
            self.repeat = Some((column, dir, now + REPEAT_DELAY));
        }
        if let Some(next) = self.repeat_tick(now, pointer, r) {
            ui.ctx().request_repaint_after(std::time::Duration::from_secs_f64((next - now).max(0.0)));
        }
        if released && let Some((column, dir, _)) = self.repeat.take() {
            // `Button.Click` on a release over it.
            if pointer.and_then(|pos| Self::scroll_button_at(r, pos)) == Some((column, dir)) {
                self.step(column, dir);
            }
            return Out::Open;
        }
        if released {
            if std::mem::take(&mut self.opened_this_press) {
                return Out::Open;
            }
            let Some(pos) = pointer else { return Out::Open };
            if accept.contains(pos) {
                return Out::Accept(self.date());
            }
            if dismiss.contains(pos) {
                return Out::Dismiss;
            }
            if Self::scroll_button_at(r, pos).is_some() {
                return Out::Open;
            }
            if let Some(c) = Self::column_at(r, pos) {
                let offset = ((pos.y - (r.min.y + HIGHLIGHT_TOP)) / ITEM_HEIGHT).floor() as i32;
                if offset != 0 && self.item(c, offset).is_some() {
                    self.step(c, offset);
                }
            }
        } else if !down {
            self.opened_this_press = false;
        }
        Out::Open
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn days_and_months_loop_and_the_day_follows_the_month() {
        let mut f = DateFlyout::new(Id::new("t"), Rect::from_min_size(pos2(0.0, 0.0), vec2(296.0, 40.0)), NaiveDate::from_ymd_opt(2024, 1, 31).unwrap());
        f.step(Column::Month, 1);
        // 31 January → February of a leap year: the 29th.
        assert_eq!(f.date(), NaiveDate::from_ymd_opt(2024, 2, 29).unwrap());
        f.step(Column::Year, 1);
        assert_eq!(f.date(), NaiveDate::from_ymd_opt(2025, 2, 28).unwrap());
        f.step(Column::Day, 1);
        assert_eq!(f.day, 1);
        f.step(Column::Month, -2);
        assert_eq!((f.month, f.day), (12, 1));
        f.step(Column::Year, -3000);
        assert_eq!(f.year, 1);
    }

    #[test]
    fn the_up_and_down_buttons_repeat_after_the_delay() {
        let mut f = DateFlyout::new(Id::new("t"), Rect::from_min_size(pos2(0.0, 0.0), vec2(296.0, 40.0)), NaiveDate::from_ymd_opt(2026, 11, 19).unwrap());
        let r = Rect::from_min_size(pos2(0.0, 0.0), vec2(WIDTH, HEIGHT));
        // The day column's up button: x 1..79, y 1..23; the month's down: x 80..217, y 334..356.
        assert_eq!(DateFlyout::scroll_button_at(r, pos2(40.0, 10.0)), Some((Column::Day, -1)));
        assert_eq!(DateFlyout::scroll_button_at(r, pos2(100.0, 340.0)), Some((Column::Month, 1)));
        assert_eq!(DateFlyout::scroll_button_at(r, pos2(100.0, 200.0)), None);
        let up = Some(pos2(40.0, 10.0));
        f.repeat = Some((Column::Day, -1, 10.0 + REPEAT_DELAY));
        // Nothing before the delay; then a step a tick, every 100 ms.
        assert_eq!(f.repeat_tick(10.1, up, r), Some(10.3));
        assert_eq!(f.day, 19);
        f.repeat_tick(10.3, up, r);
        assert_eq!(f.day, 18);
        f.repeat_tick(10.4, up, r);
        assert_eq!(f.day, 17);
        // Off the button: the timer runs but doesn't scroll.
        f.repeat_tick(10.5, Some(pos2(40.0, 200.0)), r);
        assert_eq!(f.day, 17);
    }

    #[test]
    fn the_selected_row_sits_over_the_picker() {
        let f = DateFlyout::new(Id::new("t"), Rect::from_min_size(pos2(10.0, 400.0), vec2(296.0, 40.0)), NaiveDate::from_ymd_opt(2026, 11, 19).unwrap());
        let r = f.rect(Rect::from_min_size(pos2(0.0, 0.0), vec2(1300.0, 700.0)));
        // 440 - 193.5 = 246.5, rounded to even: 246; the highlight 159 below.
        assert_eq!(r.min, pos2(10.0, 246.0));
        assert_eq!(r.min.y + HIGHLIGHT_TOP, 405.0);
        assert_eq!(days_in_month(2026, 12), 31);
        assert_eq!(days_in_month(2026, 4), 30);
    }
}
