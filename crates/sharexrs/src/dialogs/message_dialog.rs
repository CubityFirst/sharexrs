//! ShareX's `MessageBox.Show(owner, ...)` from a window: the message box
//! (`message_box::MessageBox`) in a window of its own, its answer sent
//! back to the window that asked (the remote storage browser's
//! confirmations and errors, `TaskHelpers.OpenRemoteStorageBrowser`'s
//! "not configured").

use std::sync::mpsc;

use crate::message_box::{self, DialogResult, MessageBox};

use super::{Cx, Dialog, Effect, Spec};

pub struct MessageDialog {
    pub message: MessageBox,
    reply: Option<mpsc::Sender<DialogResult>>,
}

impl MessageDialog {
    pub fn new(message: MessageBox, reply: Option<mpsc::Sender<DialogResult>>) -> Self {
        Self { message, reply }
    }

    fn answer(&mut self, result: DialogResult) {
        if let Some(r) = self.reply.take() {
            let _ = r.send(result);
        }
    }
}

impl Dialog for MessageDialog {
    /// `MessageBoxWindow`: 480 wide, `SizeToContent="Height"`, not
    /// resizable, `Topmost`.
    fn spec(&self) -> Spec {
        Spec { topmost: true, maximizable: false, ..Spec::fit_height(self.message.caption.clone(), message_box::WIDTH, message_box::WIDTH) }
    }

    fn content_height(&mut self, ui: &mut egui::Ui) -> f32 {
        self.message.height(ui)
    }

    fn ui(&mut self, ui: &mut egui::Ui, _cx: &mut Cx) -> Vec<Effect> {
        match self.message.ui(ui) {
            Some(result) => {
                self.answer(result);
                vec![Effect::Close]
            }
            None => Vec::new(),
        }
    }

    /// Closed by its title bar: `GetCancelResult`.
    fn closed(&mut self, _cx: &mut Cx) -> Vec<Effect> {
        let result = self.message.cancel_result();
        self.answer(result);
        Vec::new()
    }
}
