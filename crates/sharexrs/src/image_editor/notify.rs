//! The editor's own notifications (`MainViewModel.Notifications` and
//! `EditorView`'s `EditorNotificationHost`): a `Border.popup-surface.
//! zoom-upward.app-toast` centred at the bottom of the view, the icon (18px,
//! `Accent.Icon`) and the message (wrapped, `Text`), shown for 2.4 s (longer
//! while the pointer is on it), faded in and out over 0.18 s
//! (`popup-surface`'s transitions: opacity and `translateY(8px)
//! scale(0.98)` from the bottom), gone 0.22 s after it closes; a click
//! dismisses it. They're the editor's, not the app's toasts.

use egui::{Color32, CornerRadius, Id, Rect, Sense, Ui, pos2, vec2};
use sharex_ui::{Palette, border, icons, widgets};

/// `DefaultNotificationDuration`.
const DURATION: f64 = 2.4;
/// `NotificationHideDuration`.
const HIDE: f64 = 0.22;
/// The `popup-surface` transitions.
const FADE: f64 = 0.18;

struct Toast {
    message: String,
    icon: Option<&'static str>,
    shown: f64,
    /// When it closed (`IsNotificationOpen = false`).
    closed: Option<f64>,
    /// The time spent under the pointer so far (`WaitForNotificationHoverExitAsync`).
    hovered_until: f64,
}

#[derive(Default)]
pub struct Notifications {
    toast: Option<Toast>,
    /// Shown now: the start time of the next toast is taken when it's
    /// first drawn (the frame's time).
    pending: Option<(String, Option<&'static str>)>,
    pub rect: Option<Rect>,
}

impl Notifications {
    /// `ShowNotification(message, icon)`, when `enabled`
    /// (`Options.ShowNotifications`).
    pub fn show(&mut self, enabled: bool, message: impl Into<String>, icon: Option<&'static str>) {
        let message = message.into();
        if !enabled || message.trim().is_empty() {
            return;
        }
        self.pending = Some((message, icon));
    }

    /// For `--render`: the pending notification shown fully open, and kept.
    pub fn settle(&mut self) {
        if let Some((message, icon)) = self.pending.take() {
            self.toast = Some(Toast { message, icon, shown: f64::NEG_INFINITY, closed: None, hovered_until: f64::INFINITY });
        }
    }

    /// `DismissNotification`.
    pub fn dismiss(&mut self) {
        self.toast = None;
        self.pending = None;
        self.rect = None;
    }

    pub fn contains(&self, p: egui::Pos2) -> bool {
        self.rect.is_some_and(|r| r.contains(p))
    }

    /// Draws the notification over `view`.
    pub fn paint(&mut self, ui: &Ui, p: &Palette, view: Rect) {
        let now = ui.input(|i| i.time);
        if let Some((message, icon)) = self.pending.take() {
            self.toast = Some(Toast { message, icon, shown: now, closed: None, hovered_until: 0.0 });
        }
        let Some(t) = self.toast.as_mut() else {
            self.rect = None;
            return;
        };
        let font = widgets::font(13.0, 600);
        let lh = ui.fonts_mut(|f| f.row_height(&font));
        // `MaxWidth` the view's; the grid's margins 16 either side.
        let max_text = (view.width() - 32.0 - 2.0 - 28.0 - if t.icon.is_some() { 28.0 } else { 0.0 }).max(10.0);
        let lines: Vec<String> = t.message.split('\n').flat_map(|l| sharex_ui::controls::wrap_lines(ui, l, &font, max_text)).collect();
        let text_w = lines.iter().map(|l| widgets::text_width(ui, l, font.clone())).fold(0.0, f32::max);
        let text_h = (lh * lines.len() as f32).ceil();
        let icon_w = if t.icon.is_some() { 18.0 + 10.0 } else { 0.0 };
        let size = vec2(2.0 + 14.0 + icon_w + text_w + 14.0, 2.0 + 10.0 + text_h.max(if t.icon.is_some() { 18.0 } else { 0.0 }) + 10.0);
        // Centred; its bottom 60px above the view's (measured against
        // ShareX: `Margin="16,0,16,68"` lands 8px lower in the window).
        let base = Rect::from_min_size(pos2(view.min.x + ((view.width() - size.x) / 2.0).round_ties_even(), view.max.y - 60.0 - size.y), size);
        // The fade: in over 0.18 s from `translateY(8px) scale(0.98)`, out
        // the same way once closed.
        let hovered = ui.rect_contains_pointer(base);
        if hovered && t.closed.is_none() {
            t.hovered_until = now;
        }
        if t.closed.is_none() && now - t.shown >= DURATION && now > t.hovered_until {
            t.closed = Some(now);
        }
        let k = match t.closed {
            Some(c) => {
                if now - c >= HIDE {
                    self.toast = None;
                    self.rect = None;
                    return;
                }
                1.0 - ((now - c) / FADE).min(1.0)
            }
            None => ((now - t.shown) / FADE).clamp(0.0, 1.0),
        } as f32;
        let scale = 0.98 + 0.02 * k;
        let dy = 8.0 * (1.0 - k);
        let center_x = base.center().x;
        let bottom = base.max.y + dy;
        let rect = Rect::from_min_max(pos2(center_x - size.x * scale / 2.0, bottom - size.y * scale), pos2(center_x + size.x * scale / 2.0, bottom));
        self.rect = Some(base);
        let mut painter = ui.painter().clone();
        painter.set_opacity(k);
        // `BoxShadow="0 14 28 -16 #70000000"` (the spread as a smaller box).
        let shadow = egui::epaint::Shadow { offset: [0, 14], blur: 28, spread: 0, color: Color32::from_black_alpha(0x70) };
        painter.add(shadow.as_shape(rect.shrink(16.0), CornerRadius::same(8)));
        border::border(&painter, rect, 8.0, 1.0, Some(border::Brush::Solid(p.background_popup)), Some(p.border));
        let content = Rect::from_min_max(pos2(rect.min.x + 15.0, rect.min.y + 11.0), pos2(rect.max.x - 15.0, rect.max.y - 11.0));
        let mut x = content.min.x;
        if let Some(icon) = t.icon {
            icons::paint(&painter, Rect::from_min_size(pos2(x, widgets::center_y(content, 18.0)), vec2(18.0, 18.0)), icon, p.accent_icon);
            x += 28.0;
        }
        let mut y = content.min.y + ((content.height() - text_h) / 2.0).round_ties_even();
        for line in &lines {
            let g = widgets::galley(ui, line, font.clone(), p.text, f32::INFINITY);
            painter.galley(pos2(x, y.round()), g, p.text);
            y += lh;
        }
        // `OnNotificationPointerPressed`: dismissed.
        if ui.interact(base, Id::new("editor-notification"), Sense::click()).clicked() {
            self.dismiss();
            return;
        }
        if hovered {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(16));
    }
}
