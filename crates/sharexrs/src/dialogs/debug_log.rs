//! `ShareX/Presentation/DebugLog`: `DebugLogWindow`, the logger's text as
//! it grows (`MessageAdded`), with Loaded assemblies, Copy all, Open log
//! file, Upload log (after its confirmation overlay) and the startup path.
//! 850x600, at least 600x420.

use egui::{Id, Rect, Sense, Vec2, pos2, vec2};
use sharex_strings::sharex as s;
use sharex_ui::controls::{Button, TextBox, content_font, wrap_lines};
use sharex_ui::keys::app_icons as icon;
use sharex_ui::theme::{Thickness, palette};
use sharex_ui::widgets::{center_y, font, galley, line_height, mono, text_width};
use sharex_ui::icons;

use super::{Cx, Dialog, Effect, Spec, UploadRequest};
use crate::history_ui::common::{docked_toolbar, tool_panel};

/// The grid's `Margin="12,0,12,12"`, `RowSpacing="8"`.
const MARGIN: f32 = 12.0;
const ROW_SPACING: f32 = 8.0;
/// `Button.compact`: `Height="34"`, `Padding="10,4"`; icon and text
/// `Spacing="7"`; the toolbar's `ColumnSpacing="6"` and `Padding="8"`.
const BUTTON_HEIGHT: f32 = 34.0;
const BUTTON_PADDING: f32 = 10.0;
const ICON_SPACING: f32 = 7.0;
const TOOLBAR_SPACING: f32 = 6.0;
const TOOLBAR_PADDING: f32 = 8.0;

pub struct DebugLogWindow {
    pub text: String,
    version: u64,
    /// `_logger.LogFilePath`.
    log_file: Option<std::path::PathBuf>,
    /// `_startupPath`.
    startup_path: String,
    /// `UploadConfirmationOverlay.IsVisible`.
    pub confirm_upload: bool,
    /// Follows the logger (off for `--render`).
    live: bool,
    /// How much of the logger's text (with its `\r\n`s) is in.
    logged: usize,
    /// Scrolled to the end once (`CaretIndex = Text.Length`).
    scrolled: bool,
}

impl DebugLogWindow {
    pub fn new() -> Self {
        let (text, version) = crate::logger::since(u64::MAX, 0);
        let logged = text.len();
        let text = text.replace("\r\n", "\n");
        Self { text, version, log_file: crate::logger::log_file(), startup_path: startup_path(), confirm_upload: false, live: true, logged, scrolled: false }
    }

    /// A window over `text` that doesn't follow the logger (`--render`).
    pub fn with_text(text: String, log_file: Option<std::path::PathBuf>, startup_path: String) -> Self {
        let text = text.replace("\r\n", "\n");
        Self { text, version: 0, log_file, startup_path, confirm_upload: false, live: false, logged: 0, scrolled: false }
    }

    fn button_font() -> egui::FontId {
        font(13.0, 400)
    }

    fn button_width(ui: &egui::Ui, text: &str) -> f32 {
        2.0 + 2.0 * BUTTON_PADDING + 13.0 + ICON_SPACING + text_width(ui, text, Self::button_font())
    }

    /// A toolbar button: an icon of the button's 13px and a `TextBlock`.
    fn button(ui: &egui::Ui, rect: Rect, id: Id, key: &str, text: &str, enabled: bool) -> egui::Response {
        let p = palette(ui.ctx());
        let b = Button { padding: Thickness::hv(BUTTON_PADDING, 4.0), ..Button::default() };
        let r = b.show(ui, rect, id, "", enabled);
        let inner = rect.shrink2(vec2(1.0 + BUTTON_PADDING, 5.0));
        let bf = Self::button_font();
        let content = 13.0 + ICON_SPACING + text_width(ui, text, bf.clone());
        let x = inner.min.x + ((inner.width() - content) / 2.0).round_ties_even().max(0.0);
        let lh = line_height(ui, &bf);
        let icon_color = b.foreground(ui, &r, enabled, p.background_panel);
        icons::paint(ui.painter(), Rect::from_min_size(pos2(x, center_y(inner, 13.0)), Vec2::splat(13.0)), key, icon_color);
        let tc = b.content(ui, enabled, p.text_secondary, p.background_panel);
        ui.painter().galley(pos2(x + 13.0 + ICON_SPACING, center_y(inner, lh)), galley(ui, text, bf, tc, f32::INFINITY), tc);
        r
    }
}

impl Default for DebugLogWindow {
    fn default() -> Self {
        Self::new()
    }
}

/// `AppDomain.CurrentDomain.BaseDirectory`: the program's folder, with
/// its trailing separator.
fn startup_path() -> String {
    let dir = std::env::current_exe().ok().and_then(|e| e.parent().map(|p| p.to_string_lossy().into_owned())).unwrap_or_default();
    format!("{dir}{}", std::path::MAIN_SEPARATOR)
}

/// `GetUrlAtCaret`: the http(s) URL in the word at `caret` (characters).
pub fn url_at_caret(text: &str, caret: usize) -> Option<String> {
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return None;
    }
    let index = caret.min(chars.len() - 1);
    let (mut start, mut end) = (index, index);
    while start > 0 && !chars[start - 1].is_whitespace() {
        start -= 1;
    }
    while end < chars.len() && !chars[end].is_whitespace() {
        end += 1;
    }
    let word: String = chars[start..end].iter().collect();
    let candidate = word.trim_matches(['"', '\'', '(', ')', '[', ']', '{', '}', ',', ';']);
    let lower = candidate.to_ascii_lowercase();
    let scheme = lower.split_once("://").map(|(s, rest)| (s.to_owned(), rest.to_owned()));
    match scheme {
        Some((s, rest)) if (s == "http" || s == "https") && !rest.is_empty() && sharex_uploaders::url_helpers::is_valid_url(candidate) => Some(candidate.to_owned()),
        _ => None,
    }
}

impl Dialog for DebugLogWindow {
    fn spec(&self) -> Spec {
        Spec::sized(crate::rs_title(s::DebugLogWindow_Title()), 850.0, 600.0, 600.0, 420.0)
    }

    fn single(&self) -> Option<&'static str> {
        Some("debug-log")
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Cx) -> Vec<Effect> {
        if self.live {
            let (added, version) = crate::logger::since(self.version, self.logged);
            if version != self.version {
                self.logged += added.len();
                self.text.push_str(&added.replace("\r\n", "\n"));
                self.version = version;
            }
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(250));
        }
        let p = palette(ui.ctx());
        let full = super::layout::background(ui);
        let mut out = Vec::new();
        let left = full.min.x + MARGIN;
        let right = full.max.x - MARGIN;
        let modal = self.confirm_upload;

        // The toolbar.
        let bar = Rect::from_min_max(pos2(left, full.min.y), pos2(right, full.min.y + TOOLBAR_PADDING * 2.0 + BUTTON_HEIGHT + 1.0));
        docked_toolbar(ui, bar);
        let row_top = bar.min.y + TOOLBAR_PADDING;
        let mut x = bar.min.x + 1.0 + TOOLBAR_PADDING;
        let buttons = [
            (icon::Blocks, s::DebugLogWindow_LoadedAssemblies(), true),
            (icon::Copy, s::DebugLogWindow_CopyAll(), true),
            (icon::FileText, s::DebugLogWindow_OpenLogFile(), self.log_file.is_some()),
        ];
        for (i, (key, text, enabled)) in buttons.into_iter().enumerate() {
            let w = Self::button_width(ui, text);
            let rect = Rect::from_min_size(pos2(x, row_top), vec2(w, BUTTON_HEIGHT));
            if Self::button(ui, rect, ui.id().with(("tool", i)), key, text, enabled).clicked() && !modal {
                match i {
                    0 => {
                        let names = crate::logger::loaded_modules().join("\r\n");
                        eprintln!("Loaded assemblies:\r\n{}", names.trim());
                    }
                    1 => {
                        let text = self.text.trim();
                        if !text.is_empty() {
                            out.push(Effect::CopyText(text.replace('\n', "\r\n")));
                        }
                    }
                    _ => {
                        if let Some(f) = &self.log_file {
                            out.push(Effect::OpenFile(f.clone()));
                        }
                    }
                }
            }
            x += w + TOOLBAR_SPACING;
        }
        // Upload log, at the right (`IsVisible` when an upload is possible).
        let upload = s::DebugLogWindow_UploadLog();
        let w = Self::button_width(ui, upload);
        let rect = Rect::from_min_size(pos2(bar.max.x - 1.0 - TOOLBAR_PADDING - w, row_top), vec2(w, BUTTON_HEIGHT));
        if Self::button(ui, rect, ui.id().with("upload"), icon::Upload, upload, true).clicked() && !modal && !self.text.is_empty() {
            self.confirm_upload = true;
        }

        // The startup path at the bottom.
        let label = s::DebugLogWindow_StartupPath();
        let lf = font(13.0, 400);
        let link_font = content_font(14.0, 400);
        let foot_h = line_height(ui, &lf).max(line_height(ui, &link_font));
        let foot = Rect::from_min_max(pos2(left, full.max.y - MARGIN - foot_h), pos2(right, full.max.y - MARGIN));
        ui.painter().galley(pos2(left, center_y(foot, line_height(ui, &lf))), galley(ui, label, lf.clone(), p.text_secondary, f32::INFINITY), p.text_secondary);
        let lx = left + text_width(ui, label, lf) + 7.0;
        let lw = text_width(ui, &self.startup_path, link_font.clone());
        let link = Rect::from_min_size(pos2(lx, center_y(foot, line_height(ui, &link_font))), vec2(lw, line_height(ui, &link_font)));
        let lr = ui.interact(link, ui.id().with("startup"), Sense::click());
        let color = if lr.is_pointer_button_down_on() { p.accent_fill_dark[0] } else { p.accent_start };
        let g = galley(ui, &self.startup_path, link_font, color, f32::INFINITY);
        let baseline = link.min.y + g.rows.first().and_then(|r| r.glyphs.first().map(|gl| r.pos.y + gl.pos.y)).unwrap_or(link.height()).round();
        ui.painter().galley(link.min, g, color);
        ui.painter().rect_filled(Rect::from_min_size(pos2(link.min.x, baseline + 1.0), vec2(link.width(), 1.0)), 0.0, color);
        if lr.clicked() && !modal {
            out.push(Effect::OpenFile(std::path::PathBuf::from(&self.startup_path)));
        }

        // The log (`TextBox` in a `tool-panel` with `Padding="1"`): its
        // `Padding="9"` around a scroll viewer whose collapsed bars overlay
        // the text.
        let panel = Rect::from_min_max(pos2(left, bar.max.y + ROW_SPACING), pos2(right, foot.min.y - ROW_SPACING));
        tool_panel(ui, panel);
        // `BorderThickness="0"`: the text box's own background, no border.
        let box_rect = panel.shrink(2.0);
        let fill = if ui.rect_contains_pointer(box_rect) { p.control_background_hover } else { p.control_background };
        sharex_ui::border::border(ui.painter(), box_rect, 4.0, 0.0, Some(sharex_ui::border::Brush::Solid(fill)), None);
        let view = box_rect.shrink(9.0);
        let id = ui.id().with("log");
        let tb = TextBox { padding: Thickness::all(0.0), multiline: true, read_only: true, plain: true, font: Some(mono(12.0)), ..TextBox::default() };
        let content = ui.fonts_mut(|f| f.layout_no_wrap(self.text.clone(), mono(12.0), p.text)).size();
        let content = vec2(content.x.ceil(), content.y.ceil());
        let mut scroll_ui = ui.new_child(egui::UiBuilder::new().max_rect(view));
        scroll_ui.set_clip_rect(view.intersect(ui.clip_rect()));
        let mut area = egui::ScrollArea::both()
            .id_salt(id.with("scroll"))
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden);
        if !self.scrolled {
            // `CaretIndex = Text.Length`: the end is in view.
            self.scrolled = true;
            area = area.vertical_scroll_offset((content.y - view.height()).max(0.0));
        }
        let out_area = area.show(&mut scroll_ui, |ui| {
            let rect = Rect::from_min_size(ui.min_rect().min, content.max(view.size()));
            ui.allocate_rect(rect, Sense::hover());
            tb.show(ui, rect, id, &mut self.text)
        });
        let offset = out_area.state.offset;
        let response = out_area.inner;
        overlay_scroll_bars(ui, view, content, offset);
        // `OnLogDoubleTapped`: a URL under the caret opens.
        if response.double_clicked()
            && let Some(range) = egui::text_edit::TextEditState::load(ui.ctx(), id).and_then(|s| s.cursor.char_range())
            && let Some(url) = url_at_caret(&self.text, range.primary.index.0)
        {
            out.push(Effect::OpenUrl(url));
        }

        if self.confirm_upload {
            // `UploadConfirmationOverlay`.
            ui.painter().rect_filled(full, 0.0, p.overlay_modal);
            let bf = font(13.0, 400);
            let hf = font(15.0, 600);
            let width = 440.0_f32.min(full.width());
            let text_w = width - 2.0 * 19.0;
            let warning = s::MainForm_UploadDebugLogWarning();
            let lines = wrap_lines(ui, warning, &bf, text_w);
            let head_h = line_height(ui, &hf).max(20.0);
            let body_h = line_height(ui, &bf) * lines.len().max(1) as f32;
            let height = 2.0 + 36.0 + head_h + 12.0 + body_h + 12.0 + 40.0;
            let card = Rect::from_min_size(pos2(full.min.x + ((full.width() - width) / 2.0).round_ties_even(), full.min.y + ((full.height() - height) / 2.0).round_ties_even()), vec2(width, height));
            tool_panel(ui, card);
            let inner = card.shrink(19.0);
            let head = Rect::from_min_size(inner.min, vec2(inner.width(), head_h));
            icons::paint(ui.painter(), Rect::from_min_size(pos2(head.min.x, center_y(head, 20.0)), Vec2::splat(20.0)), icon::Warning, p.cancel_border);
            let prompt = s::DebugLogWindow_UploadPrompt();
            ui.painter().galley(pos2(head.min.x + 30.0, center_y(head, line_height(ui, &hf))), galley(ui, prompt, hf, p.text, inner.width() - 30.0), p.text);
            let lh = line_height(ui, &bf);
            for (i, line) in lines.iter().enumerate() {
                ui.painter().galley(pos2(inner.min.x, head.max.y + 12.0 + lh * i as f32), galley(ui, line, bf.clone(), p.text_secondary, f32::INFINITY), p.text_secondary);
            }
            let (cancel, upload) = (s::DebugLogWindow_Cancel(), s::DebugLogWindow_Upload());
            let b = Button::default();
            let (cw, uw) = (b.width(ui, cancel), b.width(ui, upload));
            let top = inner.max.y - 40.0;
            let up_rect = Rect::from_min_size(pos2(inner.max.x - uw, top), vec2(uw, 40.0));
            let cancel_rect = Rect::from_min_size(pos2(up_rect.min.x - 8.0 - cw, top), vec2(cw, 40.0));
            if b.show(ui, cancel_rect, ui.id().with("cancel-upload"), cancel, true).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.confirm_upload = false;
            } else if b.show(ui, up_rect, ui.id().with("confirm-upload"), upload, true).clicked() {
                self.confirm_upload = false;
                if !self.text.is_empty() {
                    // `UploadManager.UploadText(text)`.
                    let task = sharex_settings::TaskSettings::default_task_settings(&cx.settings.DefaultTaskSettings);
                    out.push(Effect::Upload(Box::new(UploadRequest::Text { text: self.text.replace('\n', "\r\n"), task, allow_custom_text: false })));
                }
            }
        }
        out
    }
}

/// Fluent's collapsed scroll bars over `view` (16px bars, a 2px thumb 4px
/// in from the outer edge, the line buttons' 16px at each end of the
/// track), where the content overflows.
fn overlay_scroll_bars(ui: &egui::Ui, view: Rect, content: Vec2, offset: Vec2) {
    let color = sharex_ui::theme::fluent(ui.ctx()).chrome_disabled_low;
    let (both_x, both_y) = (content.x > view.width(), content.y > view.height());
    if both_y {
        let bottom = if both_x { view.max.y - 16.0 } else { view.max.y };
        let track = (bottom - 16.0) - (view.min.y + 16.0);
        let len = (track * view.height() / content.y).max(20.0).min(track);
        let max = content.y - view.height();
        let y = view.min.y + 16.0 + (track - len) * (offset.y / max).clamp(0.0, 1.0);
        ui.painter().rect_filled(Rect::from_min_size(pos2(view.max.x - 4.0, y.round()), vec2(2.0, len.round())), 1.0, color);
    }
    if both_x {
        let right = if both_y { view.max.x - 16.0 } else { view.max.x };
        let track = (right - 16.0) - (view.min.x + 16.0);
        let len = (track * view.width() / content.x).max(20.0).min(track);
        let max = content.x - view.width();
        let x = view.min.x + 16.0 + (track - len) * (offset.x / max).clamp(0.0, 1.0);
        ui.painter().rect_filled(Rect::from_min_size(pos2(x.round(), view.max.y - 4.0), vec2(len.round(), 2.0)), 1.0, color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_at_the_caret() {
        let t = "2026-11-19 - URL opened: https://example.invalid/a?b=1, done";
        let at = t.find("example").unwrap();
        assert_eq!(url_at_caret(t, at).as_deref(), Some("https://example.invalid/a?b=1"));
        assert_eq!(url_at_caret(t, 2), None);
        assert_eq!(url_at_caret("(http://x.invalid)", 3).as_deref(), Some("http://x.invalid"));
        assert_eq!(url_at_caret("ftp://x.invalid", 3), None);
        assert_eq!(url_at_caret("", 0), None);
    }
}
