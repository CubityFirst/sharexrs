//! `ShareX/Presentation/URLUpload/URLUploadWindow.axaml(.cs)` and
//! `ShortenURL/ShortenURLWindow.axaml(.cs)`: the same window for a URL to
//! download and upload, or to shorten.

use egui::{Rect, pos2, vec2};
use sharex_settings::TaskSettings;
use sharex_strings::sharex as s;
use sharex_ui::controls::TextBox;
use sharex_ui::keys::app_icons as icon;
use sharex_ui::theme::{Thickness, palette};
use sharex_ui::widgets::{font, line_height};

use super::layout::{self, Compact};
use super::text_upload::focus_text_box;
use super::{Cx, Dialog, Effect, Spec, UploadRequest};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum UrlKind {
    /// `URLUploadWindow`.
    Upload,
    /// `ShortenURLWindow`.
    Shorten,
}

pub struct UrlWindow {
    pub kind: UrlKind,
    pub url: String,
    select_initial: bool,
    opened: bool,
    task: TaskSettings,
}

/// `URLInputValidation.IsSupported`: an absolute HTTP, HTTPS or FTP URI
/// (`Uri.TryCreate(url, UriKind.Absolute)`).
pub fn is_supported(url: &str) -> bool {
    let Some((scheme, rest)) = url.split_once("://") else { return false };
    let scheme = scheme.to_ascii_lowercase();
    if !matches!(scheme.as_str(), "http" | "https" | "ftp") {
        return false;
    }
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = authority.rsplit('@').next().unwrap_or("");
    let host = match host.rsplit_once(':') {
        Some((h, port)) if !h.ends_with(']') || host.starts_with('[') => {
            if !port.is_empty() && !port.chars().all(|c| c.is_ascii_digit()) {
                return false;
            }
            h
        }
        _ => host,
    };
    !host.is_empty() && !url.chars().any(char::is_whitespace)
}

impl UrlWindow {
    /// `new URLUploadWindow(initialURL)` / `new ShortenURLWindow(initialURL)`.
    pub fn new(kind: UrlKind, initial: Option<String>, task: TaskSettings) -> Self {
        let url = initial.unwrap_or_default();
        Self { kind, select_initial: !url.is_empty(), url, opened: false, task }
    }

    fn strings(&self) -> (&'static str, &'static str, &'static str, &'static str, &'static str, &'static str, &'static str) {
        match self.kind {
            UrlKind::Upload => (
                s::URLUploadWindow_Title(),
                s::URLUploadWindow_Heading(),
                s::URLUploadWindow_URL(),
                s::URLUploadWindow_Validation(),
                s::URLUploadWindow_Cancel(),
                s::URLUploadWindow_Upload(),
                "https://example.com/file",
            ),
            UrlKind::Shorten => (
                s::ShortenURLWindow_Title(),
                s::ShortenURLWindow_Heading(),
                s::ShortenURLWindow_URL(),
                s::ShortenURLWindow_Validation(),
                s::ShortenURLWindow_Cancel(),
                s::ShortenURLWindow_Shorten(),
                "https://example.com/long-url",
            ),
        }
    }

    fn valid(&self) -> bool {
        is_supported(self.url.trim())
    }

    /// `ValidationText.IsVisible`.
    fn shows_validation(&self) -> bool {
        let url = self.url.trim();
        !url.is_empty() && !is_supported(url)
    }

    fn header_subtitle(&self) -> Option<&'static str> {
        (self.kind == UrlKind::Upload).then(s::URLUploadWindow_Prompt)
    }

    /// The card: `Padding="12"`, the label, 6px, the 32px text box, and
    /// the validation line when it shows.
    fn card_height(&self, ui: &egui::Ui) -> f32 {
        let mut h = 2.0 + 24.0 + line_height(ui, &layout::text_font()) + 6.0 + 32.0;
        if self.shows_validation() {
            h += 6.0 + line_height(ui, &font(12.0, 600));
        }
        h
    }
}

/// `SystemControlErrorTextForegroundBrush` (Fluent's `SystemErrorTextColor`).
pub fn error_text(ctx: &egui::Context) -> egui::Color32 {
    match ctx.theme() {
        egui::Theme::Dark => egui::Color32::from_rgb(0xFF, 0xF0, 0x00),
        egui::Theme::Light => egui::Color32::from_rgb(0xC5, 0x05, 0x00),
    }
}

impl Dialog for UrlWindow {
    fn spec(&self) -> Spec {
        Spec::fit_height(crate::rs_title(self.strings().0), 620.0, 480.0)
    }

    fn content_height(&mut self, ui: &mut egui::Ui) -> f32 {
        let lines = usize::from(self.header_subtitle().is_some());
        layout::header_height(ui, lines) + 14.0 + self.card_height(ui) + 14.0 + Compact::HEIGHT + 18.0
    }

    fn ui(&mut self, ui: &mut egui::Ui, _cx: &mut Cx) -> Vec<Effect> {
        let p = palette(ui.ctx());
        let (_, heading, label, validation, cancel_text, ok_text, placeholder) = self.strings();
        let full = layout::background(ui);
        let icon = if self.kind == UrlKind::Upload { icon::Link } else { icon::ShortLink };
        let header = layout::header(ui, full, icon, heading, self.header_subtitle(), false);
        let card = Rect::from_min_size(pos2(full.min.x + 18.0, full.min.y + header + 14.0), vec2(full.width() - 36.0, self.card_height(ui)));
        layout::panel(ui, card, 6.0);
        let inner = card.shrink(13.0);
        let label_h = line_height(ui, &layout::text_font());
        layout::text(ui, inner.min, label, layout::text_font(), p.text_secondary, inner.width());
        let box_rect = Rect::from_min_size(pos2(inner.min.x, inner.min.y + label_h + 6.0), vec2(inner.width(), 32.0));
        let id = ui.id().with("url");
        let tb = TextBox { padding: Thickness::hv(8.0, 5.0), placeholder, ..TextBox::default() };
        let before = self.url.clone();
        tb.show(ui, box_rect, id, &mut self.url);
        if !self.opened {
            self.opened = true;
            focus_text_box(ui, id, &self.url, self.select_initial);
        }
        if self.shows_validation() {
            let f = font(12.0, 600);
            layout::text(ui, pos2(inner.min.x, box_rect.max.y + 6.0), validation, f, error_text(ui.ctx()), inner.width());
        }
        let mut out = Vec::new();
        if before != self.url {
            // The validation line comes and goes: `SizeToContent` follows.
            out.push(Effect::SetHeight(self.content_height(ui)));
        }
        let compact = Compact { min_width: 96.0 };
        let rects = compact.row(ui, full.max.x - 18.0, card.max.y + 14.0, 7.0, &[cancel_text, ok_text]);
        let valid = self.valid();
        let cancel = compact.show(ui, rects[0], ui.id().with("cancel"), cancel_text, false, true).clicked();
        let ok = compact.show(ui, rects[1], ui.id().with("ok"), ok_text, true, valid).clicked();
        let (enter, escape) = ui.input(|i| (i.key_pressed(egui::Key::Enter), i.key_pressed(egui::Key::Escape)));
        let escape = escape && !sharex_ui::popup::was_open(ui.ctx());
        if cancel || escape {
            return vec![Effect::Close];
        }
        // `IsDefault`: Enter presses the (enabled) primary button.
        if (ok || enter) && valid {
            let url = self.url.trim().to_owned();
            let task = self.task.clone();
            let request = match self.kind {
                UrlKind::Upload => UploadRequest::DownloadAndUpload { url, task },
                UrlKind::Shorten => UploadRequest::ShortenUrl { url, task },
            };
            return vec![Effect::Upload(Box::new(request)), Effect::Close];
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::is_supported;

    #[test]
    fn only_absolute_http_https_and_ftp_urls() {
        assert!(is_supported("https://example.com/a.png"));
        assert!(is_supported("HTTP://example.com"));
        assert!(is_supported("ftp://user@host:21/x"));
        assert!(!is_supported("not a url"));
        assert!(!is_supported("file:///C:/a.png"));
        assert!(!is_supported("https://"));
        assert!(!is_supported("example.com/a"));
    }
}
