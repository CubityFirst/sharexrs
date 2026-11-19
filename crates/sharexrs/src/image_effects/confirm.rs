//! `ShareX.ImageEffectsLib/Presentation/ImageEffectsConfirmationWindow`:
//! the message (16px, Normal, wrapped) over Yes and No, 460 wide, as tall as
//! its content; the answer goes back to the window that asked.

use std::sync::mpsc;

use egui::{Id, Rect, Ui, pos2, vec2};
use sharex_strings::image_effects_lib as s;
use sharex_ui::controls;
use sharex_ui::theme::palette;
use sharex_ui::widgets::font;

use crate::dialogs::{Cx, Dialog, Effect, Spec};
use crate::image_tools as tools;

pub const WIDTH: f32 = 460.0;

pub struct ConfirmWindow {
    message: String,
    reply: Option<mpsc::Sender<bool>>,
}

impl ConfirmWindow {
    pub fn new(message: &str, reply: mpsc::Sender<bool>) -> Self {
        Self { message: message.to_string(), reply: Some(reply) }
    }

    fn message_height(&self, ui: &Ui) -> f32 {
        controls::wrapped(ui, pos2(0.0, 0.0), WIDTH - 32.0, &self.message, font(16.0, 400), palette(ui.ctx()).text_secondary, false)
    }

    fn answer(&mut self, yes: bool) -> Vec<Effect> {
        if let Some(r) = self.reply.take() {
            let _ = r.send(yes);
        }
        vec![Effect::Close]
    }
}

impl Dialog for ConfirmWindow {
    fn spec(&self) -> Spec {
        Spec::fit_height(crate::rs_title(s::ImageEffectsConfirmationWindow_Title()), WIDTH, WIDTH)
    }

    fn content_height(&mut self, ui: &mut Ui) -> f32 {
        16.0 + self.message_height(ui) + 16.0 + tools::BUTTON_HEIGHT + 16.0
    }

    fn ui(&mut self, ui: &mut Ui, _cx: &mut Cx) -> Vec<Effect> {
        let c = tools::background(ui);
        controls::wrapped(ui, c.min, c.width(), &self.message, font(16.0, 400), palette(ui.ctx()).text_secondary, true);
        let y = c.min.y + self.message_height(ui) + 16.0;
        let (yes, no) = (s::ImageEffectsConfirmationWindow_Yes(), s::ImageEffectsConfirmationWindow_No());
        let (yw, nw) = (tools::text_button_width(ui, yes), tools::text_button_width(ui, no));
        let nr = Rect::from_min_size(pos2(c.max.x - nw, y), vec2(nw, tools::BUTTON_HEIGHT));
        let yr = Rect::from_min_size(pos2(nr.min.x - 8.0 - yw, y), vec2(yw, tools::BUTTON_HEIGHT));
        if tools::text_button(ui, yr, Id::new("effects-confirm-yes"), yes, true).clicked() {
            return self.answer(true);
        }
        if tools::text_button(ui, nr, Id::new("effects-confirm-no"), no, true).clicked() || sharex_ui::popup::escape_pressed(ui.ctx()) {
            return self.answer(false);
        }
        Vec::new()
    }

    /// Closed without an answer is No (`ShowDialog<bool>` gives false).
    fn closed(&mut self, _cx: &mut Cx) -> Vec<Effect> {
        if let Some(r) = self.reply.take() {
            let _ = r.send(false);
        }
        Vec::new()
    }
}
