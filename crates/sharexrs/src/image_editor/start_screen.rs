//! `StartScreenDialogView` with its `StartScreenDialogViewModel`: what the
//! image editor shows when it opens without an image (not in task mode):
//! new image, open, the clipboard, a URL, a screenshot, exit, and the
//! recent files.

use egui::{Id, Rect, Sense, Ui, Vec2, pos2, vec2};
use sharex_strings::image_editor as ie;
use sharex_ui::controls::{Button, TextBox};
use sharex_ui::keys::app_icons;
use sharex_ui::theme::Thickness;
use sharex_ui::{Palette, border, icons, widgets};

use super::modal::{self, text_font};

/// The dialog's `Width`.
const WIDTH: f32 = 750.0;
/// The left column (`ColumnDefinitions="350,*"`).
const LEFT: f32 = 350.0;
const BUTTON_H: f32 = 42.0;
const RECENT_H: f32 = 36.0;

/// What the start screen asks the editor for (its view model's callbacks).
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    NewImage,
    OpenFile,
    LoadFromClipboard,
    /// `onShowUrlInput`: the URL box (filled from the clipboard's URL).
    ShowUrlInput,
    SubmitUrl(String),
    Close,
    Exit,
    OpenRecent(String),
    CaptureRegion,
    CaptureFullscreen,
}

#[derive(Default)]
pub struct StartScreen {
    pub recent: Vec<String>,
    pub url: String,
    pub url_visible: bool,
    pub url_loading: bool,
    pub status: Option<String>,
    /// The recent files' `ScrollViewer` offset.
    scroll: f32,
    /// `ShowUrlInput` this frame: the box takes the focus.
    focus_url: bool,
}

impl StartScreen {
    pub fn new(recent: Vec<String>) -> Self {
        Self { recent, ..Default::default() }
    }

    /// `ShowUrlInput(initialUrl)`.
    pub fn show_url_input(&mut self, initial: Option<String>) {
        self.status = None;
        self.url_visible = true;
        self.url_loading = false;
        if let Some(u) = initial.filter(|u| !u.trim().is_empty()) {
            self.url = u;
        }
        self.focus_url = true;
    }

    /// `HideUrlInput` (Cancel).
    pub fn hide_url_input(&mut self) {
        self.url_loading = false;
        self.url_visible = false;
        self.status = None;
    }

    /// `SubmitUrl`: an absolute http(s) URL, or the status says it isn't.
    fn submit(&mut self) -> Option<Action> {
        self.status = None;
        let url = self.url.trim().to_string();
        let ok = is_http_url(&url);
        if !ok {
            self.status = Some(ie::StartScreenDialogView_InvalidURL().to_string());
            return None;
        }
        Some(Action::SubmitUrl(url))
    }

    fn can_submit(&self) -> bool {
        self.url_visible && !self.url_loading && !self.url.trim().is_empty()
    }

    fn status_height(&self, ui: &Ui) -> f32 {
        let Some(s) = &self.status else { return 0.0 };
        let font = text_font(13.0, 400);
        let w = LEFT - 28.0 - 2.0 - 24.0 - 18.0 - 12.0;
        let lines = sharex_ui::controls::wrap_lines(ui, s, &font, w).len().max(1);
        let text_h = lines as f32 * widgets::line_height(ui, &font);
        2.0 + 24.0 + text_h.max(18.0) + 4.0 + 6.0
    }

    fn url_height(&self) -> f32 {
        if !self.url_visible {
            return 0.0;
        }
        // `Margin="0,8"`, the box, the progress bar, the buttons.
        8.0 + 40.0 + 8.0 + if self.url_loading { 4.0 + 8.0 } else { 0.0 } + 40.0 + 8.0 + 6.0
    }

    fn left_height(&self, ui: &Ui) -> f32 {
        14.0 + self.status_height(ui) + 7.0 * BUTTON_H + 6.0 * 6.0 + self.url_height() + 14.0
    }

    fn header_height(ui: &Ui) -> f32 {
        widgets::line_height(ui, &text_font(12.0, 400)).max(14.0)
    }

    fn list_height(&self, ui: &Ui) -> f32 {
        if self.recent.is_empty() {
            let font = text_font(13.0, 400);
            2.0 + 24.0 + widgets::line_height(ui, &font)
        } else {
            self.recent.len() as f32 * (RECENT_H + 4.0) - 4.0
        }
    }

    /// The dialog's size within a `view` of that height.
    pub fn size(&self, ui: &Ui, view_h: f32) -> Vec2 {
        let right = 14.0 + Self::header_height(ui) + 10.0 + self.list_height(ui) + 14.0;
        let h = 2.0 + modal::title_height(ui, true) + self.left_height(ui).max(right);
        vec2(WIDTH, h.min(view_h))
    }

    pub fn show(&mut self, ui: &mut Ui, p: &Palette, rect: Rect) -> Option<Action> {
        let mut action = None;
        let inner = modal::frame(ui, p, rect, modal::SHADOW_LARGE);
        let (title, close) = modal::title_bar(ui, p, inner, ie::StartScreenDialogView_Title(), p.text, Some(Id::new("start-close")));
        if close.is_some_and(|r| r.clicked()) {
            action = Some(Action::Close);
        }
        let body = Rect::from_min_max(pos2(inner.min.x, title.max.y), inner.max);
        // The left column.
        let x = body.min.x + 14.0;
        let w = LEFT - 28.0;
        let mut y = body.min.y + 14.0;
        if let Some(status) = self.status.clone() {
            let h = self.status_height(ui) - 4.0 - 6.0;
            let r = Rect::from_min_size(pos2(x, y), vec2(w, h));
            border::border(ui.painter(), r, 4.0, 1.0, Some(border::Brush::Solid(p.cancel_background)), Some(p.cancel_border));
            let content = r.shrink(13.0);
            icons::paint(ui.painter(), Rect::from_min_size(pos2(content.min.x, content.center().y - 9.0), vec2(18.0, 18.0)), app_icons::CircleX, p.text);
            let font = text_font(13.0, 400);
            let lines = sharex_ui::controls::wrap_lines(ui, &status, &font, content.width() - 30.0);
            let lh = widgets::line_height(ui, &font);
            let mut ty = widgets::center_y(content, lh * lines.len() as f32);
            for line in &lines {
                let g = widgets::galley(ui, line, font.clone(), p.text, f32::INFINITY);
                ui.painter().galley(pos2(content.min.x + 30.0, ty), g, p.text);
                ty += lh;
            }
            y += h + 4.0 + 6.0;
        }
        let pad = Thickness::hv(12.0, 10.0);
        let button = |ui: &Ui, y: &mut f32, key: &str, icon: &str, text: &str| -> bool {
            let r = Rect::from_min_size(pos2(x, *y), vec2(w, BUTTON_H));
            *y += BUTTON_H + 6.0;
            modal::icon_text_button(ui, p, r, Id::new(("start", key)), icon, 18.0, text, pad).clicked()
        };
        if button(ui, &mut y, "new", app_icons::ImageAdd, ie::StartScreenDialogView_CreateNewImage()) {
            action = Some(Action::NewImage);
        }
        if button(ui, &mut y, "open", app_icons::FolderOpen, ie::StartScreenDialogView_OpenImageFile()) {
            action = Some(Action::OpenFile);
        }
        if button(ui, &mut y, "clipboard", app_icons::ClipboardPaste, ie::StartScreenDialogView_LoadImageFromClipboard()) {
            self.status = None;
            action = Some(Action::LoadFromClipboard);
        }
        if button(ui, &mut y, "url", app_icons::Link, ie::StartScreenDialogView_LoadImageFromURL()) {
            action = Some(Action::ShowUrlInput);
        }
        if self.url_visible {
            y += 8.0;
            let br = Rect::from_min_size(pos2(x, y), vec2(w, 40.0));
            let tb = TextBox { placeholder: "https://example.com/image.png", enabled: !self.url_loading, ..Default::default() };
            let before = self.url.clone();
            let resp = tb.show(ui, br, Id::new("start-url"), &mut self.url);
            if std::mem::take(&mut self.focus_url) {
                resp.request_focus();
            }
            if self.url != before {
                self.status = None;
            }
            if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && self.can_submit() {
                action = self.submit();
            }
            y += 40.0 + 8.0;
            if self.url_loading {
                let pr = Rect::from_min_size(pos2(x, y), vec2(w, 4.0));
                sharex_ui::controls::indeterminate_progress(ui.painter(), pr, ui.input(|i| i.time));
                ui.ctx().request_repaint();
                y += 4.0 + 8.0;
            }
            let b = Button { padding: Thickness::new(18.0, 0.0, 18.0, 0.0), ..Default::default() };
            let (load, cancel) = (ie::StartScreenDialogView_Load(), ie::StartScreenDialogView_Cancel());
            let (lw, cw) = (b.width(ui, load).ceil(), b.width(ui, cancel).ceil());
            let cr = Rect::from_min_size(pos2(x + w - cw, y), vec2(cw, 40.0));
            let lr = Rect::from_min_size(pos2(cr.min.x - 6.0 - lw, y), vec2(lw, 40.0));
            if b.show(ui, lr, Id::new("start-url-load"), load, self.can_submit()).clicked() {
                action = self.submit();
            }
            if b.show(ui, cr, Id::new("start-url-cancel"), cancel, true).clicked() {
                self.hide_url_input();
            }
            y += 40.0 + 8.0 + 6.0;
        }
        if button(ui, &mut y, "region", app_icons::Region, ie::EditorView_NewFromScreenshotRegionEllipsis()) {
            action = Some(Action::CaptureRegion);
        }
        if button(ui, &mut y, "fullscreen", app_icons::Fullscreen, ie::EditorView_NewFromScreenshotFullScreen()) {
            action = Some(Action::CaptureFullscreen);
        }
        if button(ui, &mut y, "exit", app_icons::Exit, ie::StartScreenDialogView_Exit()) {
            action = Some(Action::Exit);
        }
        // The right column: its 1px left border, the header, the list.
        let right = Rect::from_min_max(pos2(body.min.x + LEFT, body.min.y), body.max);
        ui.painter().rect_filled(Rect::from_min_size(right.min, vec2(1.0, right.height())), 0.0, p.border);
        let cx = right.min.x + 1.0 + 14.0;
        let cw = right.max.x - 14.0 - cx;
        let hy = right.min.y + 14.0;
        let hh = Self::header_height(ui);
        let font12 = text_font(12.0, 400);
        icons::paint(ui.painter(), Rect::from_min_size(pos2(cx, hy + ((hh - 14.0) / 2.0).round_ties_even()), vec2(14.0, 14.0)), app_icons::History, p.text_secondary);
        let g = widgets::galley(ui, ie::StartScreenDialogView_RecentFiles(), font12.clone(), p.text_secondary, f32::INFINITY);
        let lh12 = widgets::line_height(ui, &font12);
        ui.painter().galley(pos2(cx + 14.0 + 8.0, hy + ((hh - lh12) / 2.0).round_ties_even()), g, p.text_secondary);
        let list = Rect::from_min_max(pos2(cx, hy + hh + 10.0), pos2(cx + cw, right.max.y - 14.0));
        if self.recent.is_empty() {
            let font = text_font(13.0, 400);
            let r = Rect::from_min_size(list.min, vec2(cw, self.list_height(ui)));
            border::border(ui.painter(), r, 4.0, 1.0, Some(border::Brush::Solid(p.background_popup)), Some(p.border));
            let g = widgets::galley(ui, ie::StartScreenDialogView_NoRecentImageFiles(), font, p.text_secondary, cw - 26.0);
            ui.painter().galley(pos2(r.min.x + 13.0, r.min.y + 13.0), g, p.text_secondary);
        } else {
            let total = self.list_height(ui);
            if ui.rect_contains_pointer(list) {
                let d = ui.ctx().input(|i| i.smooth_scroll_delta.y);
                self.scroll -= d;
            }
            self.scroll = self.scroll.clamp(0.0, (total - list.height()).max(0.0));
            let clip = ui.clip_rect();
            ui.set_clip_rect(list.intersect(clip));
            let b = Button { padding: Thickness::hv(12.0, 6.0), ..Default::default() };
            for (i, path) in self.recent.clone().iter().enumerate() {
                let r = Rect::from_min_size(pos2(list.min.x, list.min.y + i as f32 * (RECENT_H + 4.0) - self.scroll), vec2(cw, RECENT_H));
                if r.max.y < list.min.y || r.min.y > list.max.y {
                    continue;
                }
                let resp = b.show(ui, r, Id::new(("start-recent", i)), "", true).on_hover_text(path.as_str());
                let inner = Rect::from_min_max(pos2(r.min.x + 13.0, r.min.y + 7.0), pos2(r.max.x - 13.0, r.max.y - 7.0));
                let text = leading_ellipsis(ui, path, &font12, inner.width());
                let g = widgets::galley(ui, &text, font12.clone(), p.text_secondary, f32::INFINITY);
                ui.painter().galley(pos2(inner.min.x, widgets::center_y(inner, lh12)), g, p.text_secondary);
                if resp.clicked() {
                    action = Some(Action::OpenRecent(path.clone()));
                }
            }
            ui.set_clip_rect(clip);
        }
        let _ = ui.interact(rect, Id::new("start-screen-body"), Sense::hover());
        action
    }
}

/// `Uri.TryCreate(text, UriKind.Absolute)` with an http or https scheme.
pub fn is_http_url(text: &str) -> bool {
    sharex_uploaders::http::dotnet_absolute_uri(text).is_ok_and(|u| {
        let l = u.to_ascii_lowercase();
        l.starts_with("http://") || l.starts_with("https://")
    })
}

/// `TextTrimming="LeadingCharacterEllipsis"`: the start cut off with an
/// ellipsis until the rest fits.
pub fn leading_ellipsis(ui: &Ui, text: &str, font: &egui::FontId, width: f32) -> String {
    if widgets::text_width(ui, text, font.clone()) <= width {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    for start in 1..chars.len() {
        let s: String = std::iter::once('\u{2026}').chain(chars[start..].iter().copied()).collect();
        if widgets::text_width(ui, &s, font.clone()) <= width {
            return s;
        }
    }
    "\u{2026}".into()
}
