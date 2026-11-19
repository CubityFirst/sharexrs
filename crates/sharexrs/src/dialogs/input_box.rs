//! `ShareX.HelpersLib/Presentation/InputBox/InputBoxWindow.axaml(.cs)`
//! (`InputBoxWindowIntegration.Show`): a text box over OK and Cancel, the
//! question in the title ("ShareX - {0}"); the text, or `None` for
//! Cancel, goes back to the window that asked (MEGA's two-factor code,
//! the remote storage browser's new folder and rename).

use std::sync::mpsc;

use egui::{Rect, vec2};
use sharex_strings::helpers_lib as h;
use sharex_ui::controls::TextBox;
use sharex_ui::theme::Thickness;

use super::layout::{self, Compact};
use super::text_upload::focus_text_box;
use super::{Cx, Dialog, Effect, Spec};

/// `Margin="16"`, `RowSpacing="12"`, the text box's `MinHeight="36"`.
const MARGIN: f32 = 16.0;
const ROW_SPACING: f32 = 12.0;
const BOX_HEIGHT: f32 = 36.0;

pub struct InputBoxWindow {
    title: String,
    pub text: String,
    ok: String,
    cancel: String,
    opened: bool,
    reply: Option<mpsc::Sender<Option<String>>>,
}

impl InputBoxWindow {
    /// `new InputBoxWindow(title, inputText, okText, cancelText)`: empty
    /// button texts are "OK" and "Cancel".
    pub fn new(title: &str, input: Option<&str>, ok: Option<&str>, cancel: Option<&str>, reply: Option<mpsc::Sender<Option<String>>>) -> Self {
        let pick = |t: Option<&str>, default: &str| t.filter(|t| !t.is_empty()).unwrap_or(default).to_owned();
        Self {
            title: h::InputBoxWindow_Title().replace("{0}", title),
            text: input.unwrap_or_default().to_owned(),
            ok: pick(ok, h::MyMessageBox_MyMessageBox_OK()),
            cancel: pick(cancel, h::MyMessageBox_MyMessageBox_Cancel()),
            opened: false,
            reply,
        }
    }

    fn answer(&mut self, text: Option<String>) {
        if let Some(r) = self.reply.take() {
            let _ = r.send(text);
        }
    }
}

impl Dialog for InputBoxWindow {
    /// `Width="520" Height="120"`, `MinWidth="384" MaxWidth="1000"`, the
    /// height fixed at 120; `CanMaximize="False"`, `Topmost`,
    /// `ShowInTaskbar="False"`.
    fn spec(&self) -> Spec {
        Spec {
            maximizable: false,
            topmost: true,
            skip_taskbar: true,
            max_width: 1000.0,
            max_height: 120.0,
            ..Spec::sized(crate::rs_title(&self.title), 520.0, 120.0, 384.0, 120.0)
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _cx: &mut Cx) -> Vec<Effect> {
        let full = layout::background(ui);
        let inner = full.shrink(MARGIN);
        let id = ui.id().with("input");
        let tb = TextBox { padding: Thickness::hv(8.0, 6.0), ..TextBox::default() };
        tb.show(ui, Rect::from_min_size(inner.min, vec2(inner.width(), BOX_HEIGHT)), id, &mut self.text);
        if !self.opened {
            // `OnOpened`: `Focus()`, `SelectAll()`.
            self.opened = true;
            focus_text_box(ui, id, &self.text, true);
        }
        let compact = Compact { min_width: 96.0 };
        let labels = [self.ok.as_str(), self.cancel.as_str()];
        let rects = compact.row(ui, inner.max.x, inner.min.y + BOX_HEIGHT + ROW_SPACING, 7.0, &labels);
        let ok = compact.show(ui, rects[0], ui.id().with("ok"), labels[0], true, true).clicked();
        let cancel = compact.show(ui, rects[1], ui.id().with("cancel"), labels[1], false, true).clicked();
        let (enter, escape) = ui.input(|i| (i.key_pressed(egui::Key::Enter), i.key_pressed(egui::Key::Escape)));
        let escape = escape && !sharex_ui::popup::was_open(ui.ctx());
        if ok || enter {
            let text = self.text.clone();
            self.answer(Some(text));
            return vec![Effect::Close];
        }
        if cancel || escape {
            return vec![Effect::Close];
        }
        Vec::new()
    }

    /// `SubmittedText` stays null unless OK was pressed.
    fn closed(&mut self, _cx: &mut Cx) -> Vec<Effect> {
        self.answer(None);
        Vec::new()
    }
}
