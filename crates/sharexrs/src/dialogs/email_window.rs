//! `ShareX.UploadersLib/Presentation/Email/EmailWindow.axaml(.cs)`
//! (`EmailWindowIntegration.Show`): to, subject and message for an email
//! upload or share when automatic sending is off; Send answers them,
//! Cancel (or closing) answers nothing.

use std::sync::mpsc;

use egui::{Rect, pos2};
use sharex_strings::uploaders_lib as s;
use sharex_uploaders::destinations::email::EmailFields;

use super::forms::{self, BOX_HEIGHT, COMPACT, MARGIN, ROW_SPACING};
use super::layout::{self, Compact};
use super::{Cx, Dialog, Effect, Spec};

pub struct EmailWindow {
    pub to: String,
    pub subject: String,
    pub body: String,
    opened: bool,
    reply: Option<mpsc::Sender<Option<EmailFields>>>,
}

impl EmailWindow {
    pub fn new(to: &str, subject: &str, body: &str, reply: Option<mpsc::Sender<Option<EmailFields>>>) -> Self {
        Self { to: to.to_owned(), subject: subject.to_owned(), body: body.to_owned(), opened: false, reply }
    }

    fn answer(&mut self, fields: Option<EmailFields>) {
        if let Some(r) = self.reply.take() {
            let _ = r.send(fields);
        }
    }
}

impl Dialog for EmailWindow {
    /// `Width="620" Height="480"`, `MinWidth="480" MinHeight="360"`.
    fn spec(&self) -> Spec {
        Spec::sized(crate::rs_title(s::EmailWindow_ShareX_Send_email()), 620.0, 480.0, 480.0, 360.0)
    }

    fn ui(&mut self, ui: &mut egui::Ui, _cx: &mut Cx) -> Vec<Effect> {
        let full = layout::background(ui);
        let inner = full.shrink(MARGIN);
        let buttons_top = inner.max.y - Compact::HEIGHT;
        let content = forms::card(ui, Rect::from_min_max(inner.min, pos2(inner.max.x, buttons_top - ROW_SPACING)));
        let (x0, x1) = (content.min.x, content.max.x);
        let mut y = forms::label(ui, content.min, s::EmailWindow_To_email(), content.width());
        forms::text_box(ui, x0, x1, y, ui.id().with("to"), &mut self.to, s::EmailWindow_name_example_com());
        y += BOX_HEIGHT + ROW_SPACING;
        y = forms::label(ui, pos2(x0, y), s::EmailWindow_Subject(), content.width());
        forms::text_box(ui, x0, x1, y, ui.id().with("subject"), &mut self.subject, s::EmailWindow_Email_subject());
        y += BOX_HEIGHT + ROW_SPACING;
        y = forms::label(ui, pos2(x0, y), s::EmailWindow_Message(), content.width());
        let id = ui.id().with("message");
        forms::message_box(ui, Rect::from_min_max(pos2(x0, y), content.max), id, &mut self.body);
        if !self.opened {
            // `OnOpened`: the message focused, the caret at its end.
            self.opened = true;
            ui.memory_mut(|m| m.request_focus(id));
            let mut state = egui::text_edit::TextEditState::load(ui.ctx(), id).unwrap_or_default();
            let end = egui::text::CCursor::new(self.body.chars().count());
            state.cursor.set_char_range(Some(egui::text::CCursorRange::one(end)));
            state.store(ui.ctx(), id);
        }
        let labels = [s::EmailWindow_Send(), s::EmailWindow_Cancel()];
        let rects = COMPACT.row(ui, inner.max.x, buttons_top, 7.0, &labels);
        let send = COMPACT.show(ui, rects[0], ui.id().with("send"), labels[0], true, true).clicked();
        let cancel = COMPACT.show(ui, rects[1], ui.id().with("cancel"), labels[1], false, true).clicked();
        // `IsDefault`: Enter sends, except in the message (`AcceptsReturn`).
        let enter = ui.input(|i| i.key_pressed(egui::Key::Enter)) && !ui.memory(|m| m.has_focus(id));
        if send || enter {
            let fields = (self.to.clone(), self.subject.clone(), self.body.clone());
            self.answer(Some(fields));
            return vec![Effect::Close];
        }
        // `IsCancel`: Escape closes.
        if cancel || sharex_ui::popup::escape_pressed(ui.ctx()) {
            return vec![Effect::Close];
        }
        Vec::new()
    }

    fn closed(&mut self, _cx: &mut Cx) -> Vec<Effect> {
        self.answer(None);
        Vec::new()
    }
}
