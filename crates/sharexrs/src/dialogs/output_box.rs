//! `ShareX.HelpersLib/Presentation/OutputBox`: `OutputBoxWindow`, a
//! read-only monospace text (a failed FFmpeg run's output, a custom
//! uploader's `{outputbox}`) with Copy all and Close. 900x650, at least
//! 520x360.

use egui::{Rect, Sense, pos2, vec2};
use sharex_strings::helpers_lib as hs;
use sharex_ui::border::{self, Brush};
use sharex_ui::controls::{Button, TextBox};
use sharex_ui::theme::{Thickness, palette};
use sharex_ui::widgets::mono;

use super::{Cx, Dialog, Effect, Spec};

pub struct OutputBoxWindow {
    pub text: String,
    title: String,
    /// `scrollToEnd`: the caret at the end, else at the start.
    scroll_to_end: bool,
    scrolled: bool,
    /// Dropped when the window closes: `Show` returns to its caller then.
    closed: Option<std::sync::mpsc::Sender<()>>,
}

impl OutputBoxWindow {
    pub fn new(text: &str, title: &str, scroll_to_end: bool) -> Self {
        Self { text: text.replace("\r\n", "\n"), title: title.to_owned(), scroll_to_end, scrolled: false, closed: None }
    }

    /// A caller waiting for the window to close (`window.Closed += completed`).
    pub fn on_closed(mut self, closed: std::sync::mpsc::Sender<()>) -> Self {
        self.closed = Some(closed);
        self
    }
}

impl Dialog for OutputBoxWindow {
    fn spec(&self) -> Spec {
        // `string.Format(OutputBoxWindow_Title, title)`.
        let title = crate::rs_title(&hs::OutputBoxWindow_Title().replace("{0}", &self.title));
        Spec::sized(title, 900.0, 650.0, 520.0, 360.0)
    }

    fn ui(&mut self, ui: &mut egui::Ui, _cx: &mut Cx) -> Vec<Effect> {
        let p = palette(ui.ctx());
        let full = super::layout::background(ui).shrink(12.0);
        let mut out = Vec::new();
        // The buttons: `Button.compact` (`MinWidth="96"`, `Padding="14,5"`, 40 high).
        let compact = |primary: bool| Button { padding: Thickness::hv(14.0, 5.0), primary, ..Button::default() };
        let (copy, close) = (hs::OutputBoxWindow_Copy_all(), hs::OutputBoxWindow_Close());
        let top = full.max.y - 40.0;
        let cw = compact(false).width(ui, copy).max(96.0);
        let xw = compact(true).width(ui, close).max(96.0);
        if compact(false).show(ui, Rect::from_min_size(pos2(full.min.x, top), vec2(cw, 40.0)), ui.id().with("copy"), copy, true).clicked() && !self.text.is_empty() {
            out.push(Effect::CopyText(self.text.replace('\n', "\r\n")));
        }
        let closed = compact(true).show(ui, Rect::from_min_size(pos2(full.max.x - xw, top), vec2(xw, 40.0)), ui.id().with("close"), close, true).clicked();
        // `IsDefault` and `IsCancel`: Enter and Escape close.
        if closed || ui.input(|i| i.key_pressed(egui::Key::Escape) || i.key_pressed(egui::Key::Enter)) {
            out.push(Effect::Close);
        }
        // The text box: the control background, a 1px border, `Padding="10"`.
        let rect = Rect::from_min_max(full.min, pos2(full.max.x, top - 10.0));
        let fill = if ui.rect_contains_pointer(rect) { p.control_background_hover } else { p.control_background };
        border::border(ui.painter(), rect, 4.0, 1.0, Some(Brush::Solid(fill)), Some(p.border));
        let view = rect.shrink(11.0);
        let id = ui.id().with("output");
        let tb = TextBox { padding: Thickness::all(0.0), multiline: true, read_only: true, plain: true, font: Some(mono(12.0)), ..TextBox::default() };
        let content = ui.fonts_mut(|f| f.layout_no_wrap(self.text.clone(), mono(12.0), p.text)).size();
        let content = vec2(content.x.ceil(), content.y.ceil());
        let mut scroll_ui = ui.new_child(egui::UiBuilder::new().max_rect(view));
        scroll_ui.set_clip_rect(view.intersect(ui.clip_rect()));
        let mut area = egui::ScrollArea::both().id_salt(id.with("scroll")).auto_shrink([false, false]).scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden);
        if !self.scrolled {
            self.scrolled = true;
            if self.scroll_to_end {
                area = area.vertical_scroll_offset((content.y - view.height()).max(0.0));
            }
        }
        let shown = area.show(&mut scroll_ui, |ui| {
            let r = Rect::from_min_size(ui.min_rect().min, content.max(view.size()));
            ui.allocate_rect(r, Sense::hover());
            tb.show(ui, r, id, &mut self.text)
        });
        super::debug_log::overlay_scroll_bars(ui, view, content, shown.state.offset);
        out
    }

    fn closed(&mut self, _cx: &mut Cx) -> Vec<Effect> {
        if let Some(r) = self.closed.take() {
            let _ = r.send(());
        }
        Vec::new()
    }
}

#[cfg(test)]
mod closed_tests {
    #[test]
    fn closing_the_box_lets_its_caller_go_on() {
        // REC-02: the recording's worker waits on the FFmpeg error box.
        let (closed, wait) = std::sync::mpsc::channel();
        let window = super::OutputBoxWindow::new("error", "FFmpeg error", true).on_closed(closed);
        assert_eq!(wait.try_recv(), Err(std::sync::mpsc::TryRecvError::Empty));
        drop(window);
        assert_eq!(wait.recv(), Err(std::sync::mpsc::RecvError));
    }
}
