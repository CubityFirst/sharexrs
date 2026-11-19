//! `ShareX.UploadersLib/Presentation/OAuth/OAuthListenerWindow.axaml(.cs)`:
//! "Waiting for authorization in your browser to complete..." while a
//! Google sign-in (`signin::connect_loopback`) waits for the browser on a
//! worker thread. Cancel, or closing it, stops the listener; the window
//! closes itself once the sign-in is over.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use egui::{Rect, Ui, pos2, vec2};
use sharex_strings::uploaders_lib as s;
use sharex_ui::controls;
use sharex_ui::theme::palette;
use sharex_uploaders::http::CancellationToken;

use super::forms::{self, CARD_PADDING, COMPACT, MARGIN, ROW_SPACING};
use super::layout::{self, Compact};
use super::{Cx, Dialog, Effect, Spec};

/// `Width="500"`, `MinWidth="420"`.
pub const WIDTH: f32 = 500.0;
/// The card's `StackPanel Spacing="12"` and the bar's `Height="5"`.
const SPACING: f32 = 12.0;
const BAR_HEIGHT: f32 = 5.0;

pub struct OAuthListenerWindow {
    cancel: CancellationToken,
    /// Set by the sign-in's thread when it's over (`Close()` after
    /// `ConnectAsync`).
    done: Arc<AtomicBool>,
    shown: Instant,
}

impl OAuthListenerWindow {
    pub fn new(cancel: CancellationToken, done: Arc<AtomicBool>) -> Self {
        Self { cancel, done, shown: Instant::now() }
    }

    fn message_width() -> f32 {
        WIDTH - 2.0 * MARGIN - 2.0 * (1.0 + CARD_PADDING)
    }

    fn message_height(ui: &Ui) -> f32 {
        let p = palette(ui.ctx());
        let text = s::OAuthListenerWindow_Waiting_for_authorization_in_your_browser_to_complete();
        controls::wrapped(ui, pos2(0.0, 0.0), Self::message_width(), text, layout::text_font(), p.text, false)
    }

    fn card_height(ui: &Ui) -> f32 {
        2.0 * (1.0 + CARD_PADDING) + Self::message_height(ui) + SPACING + BAR_HEIGHT
    }
}

impl Dialog for OAuthListenerWindow {
    /// `SizeToContent="Height"`, `CanResize="False"`.
    fn spec(&self) -> Spec {
        Spec { maximizable: false, ..Spec::fit_height(crate::rs_title(s::OAuthListenerWindow_ShareX_Authorization()), WIDTH, 420.0) }
    }

    fn content_height(&mut self, ui: &mut Ui) -> f32 {
        MARGIN + Self::card_height(ui) + ROW_SPACING + Compact::HEIGHT + MARGIN
    }

    fn ui(&mut self, ui: &mut Ui, _cx: &mut Cx) -> Vec<Effect> {
        if self.done.load(Ordering::SeqCst) {
            return vec![Effect::Close];
        }
        let p = palette(ui.ctx());
        let full = layout::background(ui);
        let inner = full.shrink(MARGIN);
        let card = Rect::from_min_size(inner.min, vec2(inner.width(), Self::card_height(ui)));
        let content = forms::card(ui, card);
        let text = s::OAuthListenerWindow_Waiting_for_authorization_in_your_browser_to_complete();
        let text_h = controls::wrapped(ui, content.min, content.width(), text, layout::text_font(), p.text, true);
        let bar = Rect::from_min_size(pos2(content.min.x, content.min.y + text_h + SPACING), vec2(content.width(), BAR_HEIGHT));
        controls::indeterminate_progress(ui.painter(), bar, self.shown.elapsed().as_secs_f64());
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(16));
        let label = s::OAuthListenerWindow_Cancel();
        let rects = COMPACT.row(ui, inner.max.x, card.max.y + ROW_SPACING, 7.0, &[label]);
        let cancel = COMPACT.show(ui, rects[0], ui.id().with("cancel"), label, false, true).clicked();
        if cancel || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.cancel.cancel();
            return vec![Effect::Close];
        }
        Vec::new()
    }

    /// `OnClosed`: the listener stops.
    fn closed(&mut self, _cx: &mut Cx) -> Vec<Effect> {
        if !self.done.load(Ordering::SeqCst) {
            self.cancel.cancel();
        }
        Vec::new()
    }
}
