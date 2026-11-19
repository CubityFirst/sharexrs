//! `ShareX/Presentation/UploadConfirmation`: `MultiUploadConfirmationWindow`
//! (more than 10 files) and `LargeFileUploadWarningWindow` (a file over
//! `ShowLargeFileSizeWarning`), both 440px wide, as tall as their content,
//! topmost and out of the taskbar.

use std::path::PathBuf;
use std::sync::mpsc::Sender;

use egui::{Rect, Vec2, pos2, vec2};
use sharex_settings::TaskSettings;
use sharex_strings::sharex as s;
use sharex_ui::controls::{CheckBox, content_font, wrap_lines};
use sharex_ui::keys::app_icons as icon;
use sharex_ui::theme::palette;
use sharex_ui::widgets::{font, line_height};
use sharex_ui::icons;

use super::layout::{self, Compact};
use super::{Cx, Dialog, Effect, Spec, UploadRequest};

const WIDTH: f32 = 440.0;
/// The grid's `Margin="18"`, the card's `Padding="14"` (and 1px border).
const MARGIN: f32 = 18.0;
const PADDING: f32 = 14.0;
/// The 32px icon and `ColumnSpacing="16"`.
const ICON: f32 = 32.0;
const COLUMN_SPACING: f32 = 16.0;

/// What the window asks about.
pub enum Confirm {
    /// `MultiUploadConfirmationWindow(fileCount)`: the files wait for the answer.
    MultiUpload { files: Vec<PathBuf>, task: Box<TaskSettings> },
    /// `LargeFileUploadWarningWindow`: the task's thread waits for
    /// (continue, don't show again).
    LargeFile { reply: Option<Sender<(bool, bool)>> },
}

pub struct ConfirmWindow {
    pub kind: Confirm,
    pub dont_show_again: bool,
    answered: bool,
}

impl ConfirmWindow {
    pub fn new(kind: Confirm) -> Self {
        Self { kind, dont_show_again: false, answered: false }
    }

    fn message(&self) -> String {
        match &self.kind {
            Confirm::MultiUpload { files, .. } => s::MultiUploadConfirmationWindow_Message().replace("{0}", &files.len().to_string()),
            Confirm::LargeFile { .. } => s::LargeFileUploadWarningWindow_Message().to_owned(),
        }
    }

    fn texts(&self) -> (&'static str, &'static str, &'static str, &'static str) {
        match self.kind {
            Confirm::MultiUpload { .. } => (
                s::MultiUploadConfirmationWindow_Title(),
                s::MultiUploadConfirmationWindow_DontShowAgain(),
                s::MultiUploadConfirmationWindow_Upload(),
                s::MultiUploadConfirmationWindow_Cancel(),
            ),
            Confirm::LargeFile { .. } => (
                s::LargeFileUploadWarningWindow_Title(),
                s::LargeFileUploadWarningWindow_DontShowAgain(),
                s::LargeFileUploadWarningWindow_Continue(),
                s::LargeFileUploadWarningWindow_Cancel(),
            ),
        }
    }

    fn text_width() -> f32 {
        WIDTH - 2.0 * MARGIN - 2.0 * (PADDING + 1.0) - ICON - COLUMN_SPACING
    }

    /// The message: `FontSize="15" FontWeight="Normal"`, wrapped.
    fn message_font() -> egui::FontId {
        font(15.0, 400)
    }

    /// The check box's string content at `FontWeight="Normal"`.
    fn check_box(text: &str) -> CheckBox<'_> {
        CheckBox { text_font: content_font(13.0, 400), ..CheckBox::new(text) }
    }

    fn card_height(&self, ui: &egui::Ui) -> f32 {
        let lines = wrap_lines(ui, &self.message(), &Self::message_font(), Self::text_width()).len().max(1);
        let text = line_height(ui, &Self::message_font()) * lines as f32;
        let column = text + 14.0 + 32.0;
        2.0 + 2.0 * PADDING + column.max(ICON)
    }

    /// Answers once: the files go ahead or not, or the task's thread hears.
    fn answer(&mut self, ok: bool, cx: &mut Cx) -> Vec<Effect> {
        if self.answered {
            return Vec::new();
        }
        self.answered = true;
        match &mut self.kind {
            Confirm::MultiUpload { files, task } => {
                // `IsUploadConfirmed`: `ShowMultiUploadWarning = !DontShowAgain`.
                cx.settings.ShowMultiUploadWarning = !self.dont_show_again;
                if ok {
                    let files = std::mem::take(files);
                    return vec![Effect::Upload(Box::new(UploadRequest::Files { files, task: (**task).clone(), confirmed: true }))];
                }
                Vec::new()
            }
            Confirm::LargeFile { reply } => {
                if let Some(r) = reply.take() {
                    let _ = r.send((ok, self.dont_show_again));
                }
                Vec::new()
            }
        }
    }
}

impl Dialog for ConfirmWindow {
    fn spec(&self) -> Spec {
        Spec { topmost: true, skip_taskbar: true, maximizable: false, ..Spec::fit_height(crate::rs_title(self.texts().0), WIDTH, 360.0) }
    }

    fn content_height(&mut self, ui: &mut egui::Ui) -> f32 {
        MARGIN + self.card_height(ui) + 12.0 + Compact::HEIGHT + MARGIN
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Cx) -> Vec<Effect> {
        let p = palette(ui.ctx());
        let (_, check_text, ok_text, cancel_text) = self.texts();
        let full = layout::background(ui);
        let card = Rect::from_min_size(pos2(full.min.x + MARGIN, full.min.y + MARGIN), vec2(full.width() - 2.0 * MARGIN, self.card_height(ui)));
        sharex_ui::border::border(
            ui.painter(),
            card,
            8.0,
            1.0,
            Some(sharex_ui::border::Brush::Solid(p.background_panel)),
            Some(p.border),
        );
        let inner = card.shrink(PADDING + 1.0);
        // The icon: top-aligned in its two rows, centred in its column.
        let (key, color) = match self.kind {
            Confirm::MultiUpload { .. } => (icon::Files, p.accent_icon),
            Confirm::LargeFile { .. } => (icon::Warning, p.cancel_border),
        };
        icons::paint(ui.painter(), Rect::from_min_size(inner.min, Vec2::splat(ICON)), key, color);
        let x = inner.min.x + ICON + COLUMN_SPACING;
        let message = self.message();
        let mf = Self::message_font();
        let lh = line_height(ui, &mf);
        let lines = wrap_lines(ui, &message, &mf, Self::text_width());
        for (i, line) in lines.iter().enumerate() {
            layout::text(ui, pos2(x, inner.min.y + lh * i as f32), line, mf.clone(), p.text_secondary, f32::INFINITY);
        }
        let check_top = inner.min.y + lh * lines.len().max(1) as f32 + 14.0;
        let cb = Self::check_box(check_text);
        let check_rect = Rect::from_min_size(pos2(x, check_top), vec2(cb.width(ui), 32.0));
        cb.show(ui, check_rect, ui.id().with("dont-show-again"), &mut self.dont_show_again);
        let compact = Compact { min_width: 96.0 };
        let rects = compact.row(ui, full.max.x - MARGIN, card.max.y + 12.0, 7.0, &[ok_text, cancel_text]);
        let ok = compact.show(ui, rects[0], ui.id().with("ok"), ok_text, true, true).clicked();
        let cancel = compact.show(ui, rects[1], ui.id().with("cancel"), cancel_text, false, true).clicked();
        let (enter, escape) = ui.input(|i| (i.key_pressed(egui::Key::Enter), i.key_pressed(egui::Key::Escape)));
        let escape = escape && !sharex_ui::popup::was_open(ui.ctx());
        if ok || enter {
            let mut out = self.answer(true, cx);
            out.push(Effect::Close);
            return out;
        }
        if cancel || escape {
            let mut out = self.answer(false, cx);
            out.push(Effect::Close);
            return out;
        }
        Vec::new()
    }

    /// Closing the window is Cancel.
    fn closed(&mut self, cx: &mut Cx) -> Vec<Effect> {
        self.answer(false, cx)
    }
}
