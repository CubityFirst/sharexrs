//! `ShareX/Presentation/About`: `AboutWindow`. A 300px panel on the left
//! (logo, brand, product name, the update panel, the copyright) and the
//! team and link sections centred on the right. Opening it starts an
//! update check unless `DisableUpdateCheck` (`OnOpened`); a newer version
//! shows a link to the update message.
//!
//! ShareX's brand under its logo becomes ShareXrs's, as the logo does.

use std::sync::mpsc::Receiver;

use egui::{Id, Rect, Sense, Vec2, pos2, vec2};
use sharex_strings::sharex as s;
use sharex_ui::controls::{content_font, indeterminate_progress, wrap_lines};
use sharex_ui::theme::palette;
use sharex_ui::widgets::{center_y, font, galley, line_height, text_width};
use sharex_update::GitHubUpdateChecker;
use sharex_update::checker::UpdateStatus;
use sharex_update::message::AboutUpdate;

use super::update::{UpdateMessageWindow, UpdateOrigin};
use super::{Cx, Dialog, Effect, Spec};

/// `ColumnDefinitions="300,*"`; the panel's `Padding="24"`, rows
/// `RowSpacing="14"`.
const PANEL_WIDTH: f32 = 300.0;
const PANEL_PADDING: f32 = 24.0;
const ROW_SPACING: f32 = 14.0;
/// The logo grid: 240x330, the 160px logo 50 down, the brand 225 down.
const LOGO_BOX: (f32, f32) = (240.0, 330.0);
const LOGO: f32 = 160.0;
const LOGO_TOP: f32 = 50.0;
const BRAND_TOP: f32 = 225.0;
/// The sections' `ScrollViewer`: `Margin="22,14,14,14"`.
const SECTIONS_MARGIN: (f32, f32, f32, f32) = (22.0, 14.0, 14.0, 14.0);
/// A link row: `ColumnDefinitions="180,*"`, `Margin="0,2"`.
const LABEL_COLUMN: f32 = 180.0;

/// `Links`.
pub mod links {
    pub const WEBSITE: &str = "https://getsharex.com";
    pub const CHANGELOG: &str = "https://getsharex.com/changelog";
    pub const DONATE: &str = "https://getsharex.com/donate";
    pub const PRIVACY_POLICY: &str = "https://getsharex.com/privacy-policy";
    pub const GITHUB: &str = "https://github.com/ShareX/ShareX";
    pub const JAEX: &str = "https://github.com/Jaex";
    pub const MCORED: &str = "https://github.com/McoreD";
    pub const DISCORD: &str = "https://discord.gg/ShareX";
    pub const X: &str = "https://x.com/ShareX";
    pub const X_FOLLOW: &str = "https://x.com/intent/follow?screen_name=ShareX";
    pub const REDDIT: &str = "https://www.reddit.com/r/sharex";
    pub const STEAM: &str = "https://store.steampowered.com/app/400040/ShareX/";
    pub const MICROSOFT_STORE: &str = "https://apps.microsoft.com/detail/9nblggh4z1sp";
}

/// `AboutSection`: a title and its links (label, URL).
pub struct Section {
    pub title: &'static str,
    pub items: Vec<(&'static str, &'static str)>,
}

/// `CreateSections`.
pub fn sections() -> Vec<Section> {
    use links::*;
    vec![
        Section { title: s::AboutWindow_Team(), items: vec![("Jaex", JAEX), ("McoreD", MCORED)] },
        Section {
            title: s::AboutWindow_Links(),
            items: vec![
                (s::AboutWindow_Website(), WEBSITE),
                (s::AboutWindow_ProjectPage(), GITHUB),
                (s::AboutWindow_Changelog(), CHANGELOG),
                (s::AboutWindow_PrivacyPolicy(), PRIVACY_POLICY),
                (s::AboutWindow_Donate(), DONATE),
                ("X", X),
                ("Discord", DISCORD),
                ("Reddit", REDDIT),
                ("Steam", STEAM),
                ("Microsoft Store", MICROSOFT_STORE),
            ],
        },
    ]
}

pub struct AboutWindow {
    /// `ApplicationInfo.Title`.
    product: String,
    /// The update panel as it shows now.
    pub update: AboutUpdate,
    /// The running check (`CheckUpdateAsync`).
    check: Option<Receiver<GitHubUpdateChecker>>,
    checker: Option<GitHubUpdateChecker>,
}

impl AboutWindow {
    /// `disable_update_check`: `SystemOptions.DisableUpdateCheck`; `check`
    /// delivers the finished check.
    pub fn new(product: String, disable_update_check: bool, check: Option<Receiver<GitHubUpdateChecker>>) -> Self {
        Self { product, update: AboutUpdate::new(disable_update_check, None), check: if disable_update_check { None } else { check }, checker: None }
    }

    /// The check finished.
    pub fn checked(&mut self, checker: GitHubUpdateChecker) {
        if self.update == AboutUpdate::Hidden {
            return;
        }
        let status = checker.status;
        self.update = AboutUpdate::new(false, Some(status));
        if status == UpdateStatus::UpdateAvailable {
            self.checker = Some(checker);
        }
    }

    fn heading_font() -> egui::FontId {
        font(17.0, 600)
    }

    fn label_font() -> egui::FontId {
        font(13.0, 400)
    }

    /// The `HyperlinkButton`'s string content: Inter at the control's 14px,
    /// `FontWeight="Normal"`.
    fn link_font() -> egui::FontId {
        content_font(14.0, 400)
    }

    fn row_height(ui: &egui::Ui) -> f32 {
        line_height(ui, &Self::label_font()).max(line_height(ui, &Self::link_font()))
    }

    /// The sections' size: each `StackPanel Margin="0,0,8,14"`, its heading
    /// `Margin="0,8,0,4"`, its rows `Margin="0,2"`.
    fn sections_size(ui: &egui::Ui, sections: &[Section]) -> Vec2 {
        let row = Self::row_height(ui);
        let heading = line_height(ui, &Self::heading_font());
        let mut size = Vec2::ZERO;
        for section in sections {
            let mut w = text_width(ui, section.title, Self::heading_font());
            for (_, url) in &section.items {
                w = w.max(LABEL_COLUMN + text_width(ui, url, Self::link_font()));
            }
            size.x = size.x.max(w + 8.0);
            size.y += 8.0 + heading + 4.0 + (row + 4.0) * section.items.len() as f32 + 14.0;
        }
        size
    }

    /// A link (`HyperlinkButton.about-link`): Fluent's accent foreground,
    /// `SystemAccentColorDark1` pressed, its text underlined.
    fn link(ui: &egui::Ui, pos: egui::Pos2, id: Id, text: &str) -> egui::Response {
        let p = palette(ui.ctx());
        let lf = Self::link_font();
        let rect = Rect::from_min_size(pos, vec2(text_width(ui, text, lf.clone()), line_height(ui, &lf)));
        let r = ui.interact(rect, id, Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
        let color = if r.is_pointer_button_down_on() { p.accent_fill_dark[0] } else { p.accent_start };
        let g = galley(ui, text, lf.clone(), color, f32::INFINITY);
        // The underline: a pixel under the baseline.
        let baseline = rect.min.y + g.rows.first().and_then(|r| r.glyphs.first().map(|gl| r.pos.y + gl.pos.y)).unwrap_or(rect.height()).round();
        ui.painter().galley(rect.min, g, color);
        ui.painter().rect_filled(Rect::from_min_size(pos2(rect.min.x, baseline + 1.0), vec2(rect.width(), 1.0)), 0.0, color);
        r
    }
}

impl Dialog for AboutWindow {
    fn spec(&self) -> Spec {
        Spec::sized(crate::rs_title(s::AboutWindow_Title()), 920.0, 540.0, 650.0, 500.0)
    }

    fn single(&self) -> Option<&'static str> {
        Some("about")
    }

    fn ui(&mut self, ui: &mut egui::Ui, _cx: &mut Cx) -> Vec<Effect> {
        if let Some(rx) = &self.check
            && let Ok(checker) = rx.try_recv()
        {
            self.check = None;
            self.checked(checker);
        }
        if self.check.is_some() {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
        }
        let p = palette(ui.ctx());
        let full = super::layout::background(ui);
        let mut out = Vec::new();

        // The panel.
        let panel = Rect::from_min_size(full.min, vec2(PANEL_WIDTH, full.height()));
        ui.painter().rect_filled(panel, 0.0, p.background_panel);
        let inner = panel.shrink(PANEL_PADDING);
        let cx = inner.center().x;
        let logo_box = Rect::from_min_size(pos2(cx - LOGO_BOX.0 / 2.0, inner.min.y), vec2(LOGO_BOX.0, LOGO_BOX.1));
        crate::app_icon::paint(ui, Rect::from_min_size(pos2(cx - LOGO / 2.0, logo_box.min.y + LOGO_TOP), Vec2::splat(LOGO)));
        let bf = font(50.0, 700);
        let brand = "ShareXrs";
        let bw = text_width(ui, brand, bf.clone());
        ui.painter().galley(pos2((cx - bw / 2.0).round(), logo_box.min.y + BRAND_TOP), galley(ui, brand, bf, p.text, f32::INFINITY), p.text);
        let mut y = logo_box.max.y + ROW_SPACING;
        let centred = |ui: &egui::Ui, y: f32, text: &str, f: egui::FontId, color| {
            let lh = line_height(ui, &f);
            let lines = wrap_lines(ui, text, &f, inner.width());
            for (i, line) in lines.iter().enumerate() {
                let w = text_width(ui, line, f.clone());
                let x = inner.min.x + ((inner.width() - w) / 2.0).round_ties_even();
                ui.painter().galley(pos2(x, y + lh * i as f32), galley(ui, line, f.clone(), color, f32::INFINITY), color);
            }
            lh * lines.len().max(1) as f32
        };
        y += centred(ui, y, &self.product, font(21.0, 600), p.text_secondary) + ROW_SPACING;
        // The update panel (`StackPanel Spacing="6"`).
        match &self.update {
            AboutUpdate::Hidden => {}
            AboutUpdate::Text { text, checking } => {
                if *checking {
                    let bar = Rect::from_min_size(pos2(cx - 90.0, y), vec2(180.0, 3.0));
                    indeterminate_progress(ui.painter(), bar, ui.input(|i| i.time));
                    ui.ctx().request_repaint();
                    y += 3.0 + 6.0;
                }
                centred(ui, y, text, font(13.0, 400), p.text_secondary);
            }
            AboutUpdate::Button(text) => {
                let w = text_width(ui, text, Self::link_font());
                if Self::link(ui, pos2(cx - (w / 2.0).round(), y), ui.id().with("update"), text).clicked()
                    && let Some(checker) = self.checker.clone()
                {
                    // `UpdateMessageWindow.StartAsync(_updateChecker)`.
                    out.push(Effect::OpenDialog(Box::new(UpdateMessageWindow::new(Box::new(checker), true, UpdateOrigin::User))));
                }
            }
        }
        // The copyright, at the bottom.
        let cf = font(12.0, 400);
        let lines = wrap_lines(ui, s::AboutWindow_Copyright(), &cf, inner.width()).len().max(1);
        let ch = line_height(ui, &cf) * lines as f32;
        centred(ui, inner.max.y - ch, s::AboutWindow_Copyright(), cf, p.text_secondary);

        // The sections, centred in the scroll viewer.
        let view = Rect::from_min_max(
            pos2(panel.max.x + SECTIONS_MARGIN.0, full.min.y + SECTIONS_MARGIN.1),
            pos2(full.max.x - SECTIONS_MARGIN.2, full.max.y - SECTIONS_MARGIN.3),
        );
        let sections = sections();
        let size = Self::sections_size(ui, &sections);
        let scroll = super::layout::scroll_offset(ui, ui.id().with("sections"), view, size.y);
        let x = view.min.x + ((view.width() - size.x) / 2.0).max(0.0).round_ties_even();
        let mut y = view.min.y + ((view.height() - size.y) / 2.0).max(0.0).round_ties_even() - scroll;
        let painter_clip = ui.clip_rect();
        ui.set_clip_rect(view.intersect(painter_clip));
        let (hf, lf) = (Self::heading_font(), Self::label_font());
        let row = Self::row_height(ui);
        for (si, section) in sections.iter().enumerate() {
            y += 8.0;
            ui.painter().galley(pos2(x, y), galley(ui, section.title, hf.clone(), p.text_secondary, f32::INFINITY), p.text_secondary);
            y += line_height(ui, &hf) + 4.0;
            for (ii, (label, url)) in section.items.iter().enumerate() {
                y += 2.0;
                let r = Rect::from_min_size(pos2(x, y), vec2(size.x, row));
                let g = galley(ui, label, lf.clone(), p.text_secondary, LABEL_COLUMN);
                ui.painter().galley(pos2(x, center_y(r, line_height(ui, &lf))), g, p.text_secondary);
                let link_y = center_y(r, line_height(ui, &Self::link_font()));
                if Self::link(ui, pos2(x + LABEL_COLUMN, link_y), ui.id().with(("link", si, ii)), url).clicked() {
                    out.push(Effect::OpenUrl((*url).to_owned()));
                }
                y += row + 2.0;
            }
            y += 14.0;
        }
        ui.set_clip_rect(painter_clip);
        super::layout::scroll_bar(ui, view, size.y, scroll, 16.0);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_update_panel_follows_the_check() {
        let mut w = AboutWindow::new("ShareXrs 22.4".into(), false, None);
        assert_eq!(w.update, AboutUpdate::Text { text: s::AboutWindow_CheckingForUpdates(), checking: true });
        let mut c = super::super::render::update_checker("installed");
        c.status = UpdateStatus::UpToDate;
        w.checked(c);
        assert_eq!(w.update, AboutUpdate::Text { text: s::AboutWindow_UpToDate(), checking: false });
        let hidden = AboutWindow::new("ShareXrs 22.4".into(), true, None);
        assert_eq!(hidden.update, AboutUpdate::Hidden);
        let mut w = AboutWindow::new("ShareXrs 22.4".into(), false, None);
        w.checked(super::super::render::update_checker("installed"));
        assert!(matches!(w.update, AboutUpdate::Button(_)));
        assert!(w.checker.is_some());
    }

    #[test]
    fn sections_list_the_links() {
        let s = sections();
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].items.len(), 2);
        assert_eq!(s[1].items.len(), 10);
    }
}
