//! The update windows (`ShareX.HelpersLib/Presentation/UpdateChecker`):
//!
//! - `UpdateMessageWindow` (400 wide, as tall as its content, not
//!   resizable): the header with the download tile, the message card with
//!   "View changelog", Yes and No;
//! - `DownloaderWindow` (560x310, not resizable): the header, the card with
//!   the file name, status, progress text and bar, and the one action button.
//!
//! Their logic is `sharex_update::message` and
//! `sharex_update::downloader_window`; these draw them and turn clicks
//! into [`Effect`]s.

use std::sync::Arc;

use egui::{Rect, Sense, Vec2, pos2, vec2};
use sharex_strings::helpers_lib as hs;
use sharex_ui::controls::{Button, content_font, wrap_lines};
use sharex_ui::keys::app_icons as icon;
use sharex_ui::theme::{Thickness, palette};
use sharex_ui::widgets::{center_y, font, galley, line_height};
use sharex_ui::{border, icons};
use sharex_update::checker::GitHubUpdateChecker;
use sharex_update::downloader_window::{DownloaderWindow, DownloaderWindowStatus};
use sharex_update::install::UpdateHost;
use sharex_update::message::{DialogResult, UpdateMessage};

use super::layout::{self, Compact};
use super::{Cx, Dialog, Effect, Spec};

/// The grid's `Margin="18"`.
const MARGIN: f32 = 18.0;
/// The card's `Padding="14"` inside its 1px border.
const PADDING: f32 = 14.0;
/// The header tile: 42x42, `CornerRadius="6"`, the icon at `FontSize="21"`.
const TILE: f32 = 42.0;
const TILE_ICON: f32 = 21.0;
/// `ColumnSpacing="10"`.
const TILE_SPACING: f32 = 10.0;

/// The header text: `FontSize="20" FontWeight="SemiBold"`.
fn header_font() -> egui::FontId {
    font(20.0, 600)
}

/// `\r\n` (`AppendLine`, a verbatim string) as the line breaks the text layout knows.
fn lines_of(text: &str) -> String {
    text.replace("\r\n", "\n")
}

/// The 42px tile with the download icon at `pos`.
fn tile(ui: &egui::Ui, pos: egui::Pos2) {
    let p = palette(ui.ctx());
    let tile = Rect::from_min_size(pos, Vec2::splat(TILE));
    border::border(ui.painter(), tile, 6.0, 0.0, Some(border::Brush::Solid(p.background_panel)), None);
    icons::paint(ui.painter(), Rect::from_center_size(tile.center(), Vec2::splat(TILE_ICON)), icon::Download, p.accent_icon);
}

/// Where the update flow came from: ShareX's hourly check
/// (`GitHubUpdateManager.CheckUpdate`, which turns itself off after No) or
/// a check the user asked for (About).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateOrigin {
    Timer,
    User,
}

// ---------------------------------------------------------------------------
// UpdateMessageWindow.

pub struct UpdateMessageWindow {
    pub checker: Box<GitHubUpdateChecker>,
    pub message: UpdateMessage,
    pub origin: UpdateOrigin,
    result: DialogResult,
    started: bool,
}

impl UpdateMessageWindow {
    pub fn new(checker: Box<GitHubUpdateChecker>, activate: bool, origin: UpdateOrigin) -> Self {
        let message = UpdateMessage::new(&checker, activate);
        Self { checker, message, origin, result: DialogResult::None, started: false }
    }

    fn header_text(&self) -> String {
        // The update is ShareXrs's, so its name, as in window titles.
        crate::rs_title(&self.message.header_text)
    }

    fn text_width() -> f32 {
        400.0 - 2.0 * MARGIN - 2.0 * (PADDING + 1.0)
    }

    fn header_height(&self, ui: &egui::Ui) -> f32 {
        let w = 400.0 - 2.0 * MARGIN - TILE - TILE_SPACING;
        let lines = wrap_lines(ui, &self.header_text(), &header_font(), w).len().max(1);
        (line_height(ui, &header_font()) * lines as f32).max(TILE)
    }

    fn message_font() -> egui::FontId {
        font(15.0, 400)
    }

    /// The link's string content (Inter, as a `ContentPresenter` draws it).
    fn link_font() -> egui::FontId {
        content_font(15.0, 400)
    }

    fn card_height(&self, ui: &egui::Ui) -> f32 {
        let mf = Self::message_font();
        let lines = wrap_lines(ui, &lines_of(&self.message.message_text), &mf, Self::text_width()).len().max(1);
        let mut h = line_height(ui, &mf) * lines as f32;
        if self.message.show_changelog {
            h += 18.0 + line_height(ui, &Self::link_font());
        }
        2.0 + 2.0 * PADDING + h
    }

    fn finish(&mut self, result: DialogResult) -> Vec<Effect> {
        self.result = result;
        let mut out = vec![Effect::Close];
        if result == DialogResult::Yes {
            out.push(Effect::UpdateDownload(self.checker.clone(), self.origin));
        }
        out
    }
}

impl Dialog for UpdateMessageWindow {
    fn spec(&self) -> Spec {
        Spec { activate: self.message.activate_window, maximizable: false, ..Spec::fit_height(&self.message.title, 400.0, 300.0) }
    }

    fn content_height(&mut self, ui: &mut egui::Ui) -> f32 {
        MARGIN + self.header_height(ui) + 14.0 + self.card_height(ui) + 14.0 + Compact::HEIGHT + MARGIN
    }

    fn ui(&mut self, ui: &mut egui::Ui, _cx: &mut Cx) -> Vec<Effect> {
        let mut out = Vec::new();
        if !self.started {
            self.started = true;
            // `OnOpened`: a later automatic check opens minimised and flashes.
            if !self.message.activate_window {
                out.push(Effect::MinimizeAndFlash);
            }
        }
        let p = palette(ui.ctx());
        let full = layout::background(ui);
        let left = full.min.x + MARGIN;
        let right = full.max.x - MARGIN;
        // Header.
        let header_h = self.header_height(ui);
        let top = full.min.y + MARGIN;
        tile(ui, pos2(left, center_y(Rect::from_min_size(pos2(left, top), vec2(TILE, header_h)), TILE)));
        let hf = header_font();
        let text_x = left + TILE + TILE_SPACING;
        let lines = wrap_lines(ui, &self.header_text(), &hf, right - text_x);
        let lh = line_height(ui, &hf);
        let mut y = center_y(Rect::from_min_size(pos2(text_x, top), vec2(right - text_x, header_h)), lh * lines.len() as f32);
        for line in &lines {
            layout::text(ui, pos2(text_x, y), line, hf.clone(), p.accent_text, f32::INFINITY);
            y += lh;
        }
        // The message card.
        let card = Rect::from_min_size(pos2(left, top + header_h + 14.0), vec2(right - left, self.card_height(ui)));
        layout::panel(ui, card, 6.0);
        let inner = card.shrink(PADDING + 1.0);
        let mf = Self::message_font();
        let mlh = line_height(ui, &mf);
        let lines = wrap_lines(ui, &lines_of(&self.message.message_text), &mf, inner.width());
        for (i, line) in lines.iter().enumerate() {
            layout::text(ui, pos2(inner.min.x, inner.min.y + mlh * i as f32), line, mf.clone(), p.text, f32::INFINITY);
        }
        if self.message.show_changelog {
            // `HyperlinkButton.link`: no padding, the link colour, underlined
            // while the pointer is over it.
            let lf = Self::link_font();
            let text = hs::UpdateMessageWindow_View_changelog();
            let w = layout::width(ui, text, lf.clone());
            let link = Rect::from_min_size(pos2(inner.min.x, inner.min.y + mlh * lines.len() as f32 + 18.0), vec2(w, line_height(ui, &lf)));
            let response = ui.interact(link, ui.id().with("changelog"), Sense::click());
            let g = galley(ui, text, lf, p.link, f32::INFINITY);
            ui.painter().galley(link.min, g, p.link);
            if response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                let base = link.max.y - 2.0;
                ui.painter().line_segment([pos2(link.min.x, base), pos2(link.max.x, base)], egui::Stroke::new(1.0, p.link));
            }
            if response.clicked() {
                out.push(Effect::OpenUrl(self.message.changelog_url.clone()));
            }
        }
        // Yes (primary) and No.
        let compact = Compact { min_width: 96.0 };
        let (yes_text, no_text) = (self.message.yes_text.clone(), self.message.no_text.clone());
        let rects = compact.row(ui, right, card.max.y + 14.0, 7.0, &[&yes_text, &no_text]);
        if compact.show(ui, rects[0], ui.id().with("yes"), &yes_text, true, true).clicked() {
            out.extend(self.finish(DialogResult::Yes));
        } else if compact.show(ui, rects[1], ui.id().with("no"), &no_text, false, true).clicked() {
            out.extend(self.finish(DialogResult::No));
        }
        out
    }

    /// `Closing`: closed without an answer is No.
    fn closed(&mut self, _cx: &mut Cx) -> Vec<Effect> {
        match self.result {
            DialogResult::Yes => Vec::new(),
            _ => vec![Effect::UpdateFinished { result: DialogResult::No, origin: self.origin, shutdown: false }],
        }
    }

    fn single(&self) -> Option<&'static str> {
        Some("update-message")
    }
}

// ---------------------------------------------------------------------------
// DownloaderWindow.

pub struct DownloaderDialog {
    pub window: DownloaderWindow,
    pub origin: UpdateOrigin,
    /// `FFmpegGitHubDownloader.DownloadFFmpeg`'s window (not an update).
    ffmpeg: bool,
    opened: bool,
    asked: bool,
}

impl DownloaderDialog {
    /// `DownloaderWindow.ShowAsync(updateChecker)`.
    pub fn new(checker: &GitHubUpdateChecker, host: Arc<dyn UpdateHost>, origin: UpdateOrigin) -> Self {
        Self { window: DownloaderWindow::for_update(checker, host), origin, ffmpeg: false, opened: false, asked: false }
    }

    /// `FFmpegGitHubDownloader.DownloadFFmpeg(async, installRequested)`:
    /// `DownloaderWindow.ShowAsync(url, "ffmpeg.zip")` with `InstallType.Event`.
    pub fn ffmpeg(window: DownloaderWindow) -> Self {
        Self { window, origin: UpdateOrigin::User, ffmpeg: true, opened: false, asked: false }
    }

    /// The description: a secondary `TextBlock` (the global 13px SemiBold).
    fn description_lines(&self, ui: &egui::Ui, width: f32) -> Vec<String> {
        wrap_lines(ui, &self.window.description, &layout::text_font(), width)
    }

    fn header_height(&self, ui: &egui::Ui, width: f32) -> f32 {
        let text = line_height(ui, &header_font()) + 2.0 + line_height(ui, &layout::text_font()) * self.description_lines(ui, width).len().max(1) as f32;
        text.max(TILE)
    }

    /// `Button.action`: `MinWidth="112"`, `Padding="16,5"`, the accent fill,
    /// and the global style's 40px height.
    fn action_button() -> Button {
        Button { padding: Thickness::hv(16.0, 5.0), primary: true, ..Button::default() }
    }
}

impl Dialog for DownloaderDialog {
    fn spec(&self) -> Spec {
        Spec { resizable: false, maximizable: false, ..Spec::sized(crate::rs_title(&self.window.title), 560.0, 310.0, 480.0, 280.0) }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _cx: &mut Cx) -> Vec<Effect> {
        let mut out = Vec::new();
        if !self.opened {
            self.opened = true;
            self.window.opened();
        }
        self.window.poll();
        if matches!(self.window.status, DownloaderWindowStatus::DownloadStarted) {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
        }
        let p = palette(ui.ctx());
        let full = layout::background(ui);
        let (left, right) = (full.min.x + MARGIN, full.max.x - MARGIN);
        let top = full.min.y + MARGIN;
        // Header: the tile, the heading and the description.
        let text_x = left + TILE + TILE_SPACING;
        let header_h = self.header_height(ui, right - text_x);
        tile(ui, pos2(left, center_y(Rect::from_min_size(pos2(left, top), vec2(TILE, header_h)), TILE)));
        let (hf, tf) = (header_font(), layout::text_font());
        let desc = self.description_lines(ui, right - text_x);
        let stack = line_height(ui, &hf) + 2.0 + line_height(ui, &tf) * desc.len().max(1) as f32;
        let mut y = center_y(Rect::from_min_size(pos2(text_x, top), vec2(right - text_x, header_h)), stack);
        layout::text(ui, pos2(text_x, y), &self.window.heading, hf.clone(), p.accent_text, f32::INFINITY);
        y += line_height(ui, &hf) + 2.0;
        for line in &desc {
            layout::text(ui, pos2(text_x, y), line, tf.clone(), p.text_secondary, f32::INFINITY);
            y += line_height(ui, &tf);
        }
        // The action button, bottom right.
        let button = Self::action_button();
        let bw = button.width(ui, &self.window.action_text).max(112.0);
        let button_rect = Rect::from_min_size(pos2(right - bw, full.max.y - MARGIN - Compact::HEIGHT), vec2(bw, Compact::HEIGHT));
        // The card fills the row between them (`RowSpacing="12"`).
        let card = Rect::from_min_max(pos2(left, top + header_h + 12.0), pos2(right, button_rect.min.y - 12.0));
        layout::panel(ui, card, 6.0);
        let inner = card.shrink(PADDING + 1.0);
        let lh = line_height(ui, &tf);
        // File name (trimmed with an ellipsis), status, progress text, bar.
        let name = galley_trimmed(ui, &self.window.file_name_text, tf.clone(), p.text, inner.width());
        ui.painter().galley(inner.min, name, p.text);
        let status = wrap_lines(ui, &self.window.status_text, &tf, inner.width());
        let mut y = inner.min.y + lh + 8.0;
        for line in &status {
            layout::text(ui, pos2(inner.min.x, y), line, tf.clone(), p.text_secondary, f32::INFINITY);
            y += lh;
        }
        let bar = Rect::from_min_max(pos2(inner.min.x, inner.max.y - 18.0), inner.max);
        let row = Rect::from_min_max(pos2(inner.min.x, y + 8.0), pos2(inner.max.x, bar.min.y - 8.0));
        if !self.window.progress_text.is_empty() {
            let progress = wrap_lines(ui, &lines_of(&self.window.progress_text), &tf, inner.width());
            let mut y = center_y(row, lh * progress.len() as f32);
            for line in &progress {
                layout::text(ui, pos2(inner.min.x, y), line, tf.clone(), p.text_secondary, f32::INFINITY);
                y += lh;
            }
        }
        // `ProgressBar` (`Height="18"`): Fluent's base-low track and the
        // accent indicator, `ControlCornerRadius`.
        let f = sharex_ui::theme::fluent(ui.ctx());
        border::border(ui.painter(), bar, 3.0, 0.0, Some(border::Brush::Solid(f.base_low)), None);
        let done = (bar.width() * (self.window.progress_value as f32 / 100.0).clamp(0.0, 1.0)).round();
        if done > 0.0 {
            let fill = Rect::from_min_size(bar.min, vec2(done, bar.height()));
            border::border(ui.painter(), fill, 3.0, 0.0, Some(border::Brush::Solid(p.accent_start)), None);
        }
        if button.show(ui, button_rect, ui.id().with("action"), &self.window.action_text, self.window.action_enabled).clicked() {
            self.window.action_click();
        }
        if self.window.signature_prompt && !self.asked {
            self.asked = true;
            out.push(Effect::AskInvalidSignature);
        }
        if self.window.close_requested {
            out.push(Effect::Close);
        }
        out
    }

    /// `OnClosing` (a running download stops), then what
    /// `UpdateChecker.DownloadUpdateAsync` does with the result.
    fn closed(&mut self, _cx: &mut Cx) -> Vec<Effect> {
        self.window.closing();
        if self.ffmpeg {
            return Vec::new();
        }
        let installed = self.window.status == DownloaderWindowStatus::InstallStarted;
        vec![Effect::UpdateFinished { result: DialogResult::Yes, origin: self.origin, shutdown: installed && self.window.shutdown_after }]
    }

    /// The answer to the invalid signature question.
    fn answered(&mut self, yes: bool) {
        self.asked = false;
        self.window.confirm_signature(yes);
    }

    fn single(&self) -> Option<&'static str> {
        Some(if self.ffmpeg { "ffmpeg-downloader" } else { "update-downloader" })
    }
}

/// A one-line `TextBlock` with `TextTrimming="CharacterEllipsis"`.
fn galley_trimmed(ui: &egui::Ui, text: &str, f: egui::FontId, color: egui::Color32, width: f32) -> std::sync::Arc<egui::Galley> {
    if layout::width(ui, text, f.clone()) <= width {
        return galley(ui, text, f, color, f32::INFINITY);
    }
    let mut chars: Vec<char> = text.chars().collect();
    while !chars.is_empty() {
        chars.pop();
        let candidate: String = chars.iter().collect::<String>() + "…";
        if layout::width(ui, &candidate, f.clone()) <= width {
            return galley(ui, &candidate, f, color, f32::INFINITY);
        }
    }
    galley(ui, "…", f, color, f32::INFINITY)
}
