//! `ShareX.UploadersLib/Presentation/YouTube/YouTubeVideoOptionsWindow.axaml(.cs)`
//! (`YouTubeVideoOptionsWindowIntegration.Show`): a video's title,
//! description and visibility before a YouTube upload with "show dialog"
//! on; OK answers them, Cancel (or closing) answers nothing.

use std::sync::mpsc;

use egui::{Rect, pos2, vec2};
use sharex_strings::uploaders_lib as s;
use sharex_ui::controls::{ComboBox, ComboItem};
use sharex_ui::theme::Thickness;
use sharex_uploaders::destinations::oauth_services::YouTubeOptions;
use sharex_uploaders::enums::YouTubeVideoPrivacy;

use super::forms::{self, BOX_HEIGHT, COMPACT, MARGIN, ROW_SPACING};
use super::layout::{self, Compact};
use super::text_upload::focus_text_box;
use super::{Cx, Dialog, Effect, Spec};

/// `YouTubeVideoPrivacy` in order, with `GetLocalizedEnumDescriptions`'
/// texts.
fn privacy_items() -> [(YouTubeVideoPrivacy, &'static str); 3] {
    [
        (YouTubeVideoPrivacy::Public, s::YouTubeVideoPrivacy_Public()),
        (YouTubeVideoPrivacy::Unlisted, s::YouTubeVideoPrivacy_Unlisted()),
        (YouTubeVideoPrivacy::Private, s::YouTubeVideoPrivacy_Private()),
    ]
}

pub struct YouTubeOptionsWindow {
    pub title: String,
    pub description: String,
    pub visibility: usize,
    opened: bool,
    reply: Option<mpsc::Sender<Option<YouTubeOptions>>>,
}

impl YouTubeOptionsWindow {
    pub fn new(title: &str, description: &str, visibility: &YouTubeVideoPrivacy, reply: Option<mpsc::Sender<Option<YouTubeOptions>>>) -> Self {
        let visibility = privacy_items().iter().position(|(p, _)| p == visibility).unwrap_or(2);
        Self { title: title.to_owned(), description: description.to_owned(), visibility, opened: false, reply }
    }

    fn answer(&mut self, options: Option<YouTubeOptions>) {
        if let Some(r) = self.reply.take() {
            let _ = r.send(options);
        }
    }
}

impl Dialog for YouTubeOptionsWindow {
    /// `Width="600" Height="460"`, `MinWidth="500" MinHeight="380"`.
    fn spec(&self) -> Spec {
        Spec::sized(crate::rs_title(s::YouTubeVideoOptionsWindow_ShareX_YouTube_video_options()), 600.0, 460.0, 500.0, 380.0)
    }

    fn ui(&mut self, ui: &mut egui::Ui, _cx: &mut Cx) -> Vec<Effect> {
        let full = layout::background(ui);
        let inner = full.shrink(MARGIN);
        let buttons_top = inner.max.y - Compact::HEIGHT;
        let content = forms::card(ui, Rect::from_min_max(inner.min, pos2(inner.max.x, buttons_top - ROW_SPACING)));
        let (x0, x1) = (content.min.x, content.max.x);
        let mut y = forms::label(ui, content.min, s::YouTubeVideoOptionsWindow_Title(), content.width());
        let title_id = ui.id().with("title");
        forms::text_box(ui, x0, x1, y, title_id, &mut self.title, s::YouTubeVideoOptionsWindow_Video_title());
        y += BOX_HEIGHT + ROW_SPACING;
        // The visibility row at the bottom (`RowDefinitions="Auto,*,Auto"`).
        let visibility_top = content.max.y - BOX_HEIGHT - forms::label_height(ui);
        y = forms::label(ui, pos2(x0, y), s::YouTubeVideoOptionsWindow_Description(), content.width());
        let description = Rect::from_min_max(pos2(x0, y), pos2(x1, visibility_top - ROW_SPACING));
        forms::message_box(ui, description, ui.id().with("description"), &mut self.description);
        let combo_top = forms::label(ui, pos2(x0, visibility_top), s::YouTubeVideoOptionsWindow_Visibility(), content.width());
        let items: Vec<ComboItem> = privacy_items().iter().map(|(_, t)| ComboItem::text(*t)).collect();
        let combo = ComboBox { padding: Thickness::hv(8.0, 6.0), enabled: true };
        let combo_rect = Rect::from_min_size(pos2(x0, combo_top), vec2(x1 - x0, BOX_HEIGHT));
        if let Some(i) = combo.show(ui, combo_rect, ui.id().with("visibility"), &items, Some(self.visibility)) {
            self.visibility = i;
        }
        if !self.opened {
            // `OnOpened`: the title focused and selected.
            self.opened = true;
            focus_text_box(ui, title_id, &self.title, true);
        }
        let labels = [s::YouTubeVideoOptionsWindow_OK(), s::YouTubeVideoOptionsWindow_Cancel()];
        let rects = COMPACT.row(ui, inner.max.x, buttons_top, 7.0, &labels);
        let ok = COMPACT.show(ui, rects[0], ui.id().with("ok"), labels[0], true, true).clicked();
        let cancel = COMPACT.show(ui, rects[1], ui.id().with("cancel"), labels[1], false, true).clicked();
        let enter = ui.input(|i| i.key_pressed(egui::Key::Enter)) && ui.memory(|m| m.has_focus(title_id));
        if ok || enter {
            // `OnConfirmClick`: an undefined index is Private.
            let privacy = privacy_items().get(self.visibility).map_or(YouTubeVideoPrivacy::Private, |(p, _)| p.clone());
            let options = (self.title.clone(), self.description.clone(), privacy);
            self.answer(Some(options));
            return vec![Effect::Close];
        }
        if cancel || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            return vec![Effect::Close];
        }
        Vec::new()
    }

    fn closed(&mut self, _cx: &mut Cx) -> Vec<Effect> {
        self.answer(None);
        Vec::new()
    }
}
