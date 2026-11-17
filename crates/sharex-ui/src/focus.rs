//! Keyboard focus as Avalonia shows it: `:focus-visible` and Fluent's
//! default focus adorner.
//!
//! egui moves focus with Tab / Shift+Tab and the arrow keys among widgets
//! whose `Sense` is focusable (every control here that is a tab stop in
//! ShareX), but doesn't say how a widget got focus. Avalonia shows the
//! adorner only after keyboard navigation (`NavigationMethod.Tab` /
//! `Directional`) and hides it once the pointer is pressed, so this tracks
//! the same: a navigation key makes focus visible, a pointer press hides
//! it.

use std::sync::Arc;

use egui::{Color32, Context, Event, Id, Key, Painter, Rect, pos2};

use crate::theme::fluent;

fn flag() -> Id {
    Id::new("sharex-focus-visible")
}

/// Tracks how focus moves, each frame (called by [`crate::install`]).
pub fn install(ctx: &Context) {
    ctx.on_begin_pass(
        "sharex-focus-visible",
        Arc::new(|ui: &mut egui::Ui| {
            let ctx = ui.ctx();
            let change = ctx.input(|i| {
                i.events.iter().fold(None, |state, e| match e {
                    Event::Key { key: Key::Tab | Key::ArrowUp | Key::ArrowDown | Key::ArrowLeft | Key::ArrowRight, pressed: true, .. } => Some(true),
                    Event::PointerButton { pressed: true, .. } => Some(false),
                    _ => state,
                })
            });
            if let Some(visible) = change {
                ctx.data_mut(|d| d.insert_temp(flag(), visible));
            }
        }),
    );
}

/// Whether `id` has keyboard focus that Avalonia would show
/// (`:focus-visible`).
pub fn visible(ctx: &Context, id: Id) -> bool {
    ctx.memory(|m| m.has_focus(id)) && ctx.data(|d| d.get_temp::<bool>(flag())).unwrap_or(false)
}

/// Shows focus as keyboard focus from now on, as Tab would (for rendering
/// a focused control offscreen).
pub fn set_visible(ctx: &Context, visible: bool) {
    ctx.data_mut(|d| d.insert_temp(flag(), visible));
}

/// Fluent's default `FocusAdorner` around `rect` (a control's bounds): a
/// square 2px `FocusStrokeColorOuter` border and, 2px in, a 1px
/// `FocusStrokeColorInner` one.
pub fn paint_adorner(painter: &Painter, rect: Rect) {
    let f = fluent(painter.ctx());
    ring(painter, rect, 2.0, f.focus_outer);
    ring(painter, rect.shrink(2.0), 1.0, f.focus_inner);
}

/// A square border `width` wide inside `rect`.
fn ring(painter: &Painter, rect: Rect, width: f32, color: Color32) {
    let (l, t, r, b) = (rect.min.x, rect.min.y, rect.max.x, rect.max.y);
    painter.rect_filled(Rect::from_min_max(pos2(l, t), pos2(r, t + width)), 0.0, color);
    painter.rect_filled(Rect::from_min_max(pos2(l, b - width), pos2(r, b)), 0.0, color);
    painter.rect_filled(Rect::from_min_max(pos2(l, t + width), pos2(l + width, b - width)), 0.0, color);
    painter.rect_filled(Rect::from_min_max(pos2(r - width, t + width), pos2(r, b - width)), 0.0, color);
}
