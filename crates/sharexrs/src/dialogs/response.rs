//! `ShareX.UploadersLib/Presentation/Response/ResponseWindow.axaml(.cs)`:
//! an upload's result, its response information and its response text,
//! one section at a time; and `ShareX.HelpersLib/Presentation/Error/
//! ErrorWindow` for a task's errors.

use egui::{Rect, Vec2, pos2, vec2};
use sharex_strings::{helpers_lib as hs, uploaders_lib as s};
use sharex_ui::border::{self, Brush};
use sharex_ui::controls::{Button, TextBox};
use sharex_ui::icons;
use sharex_ui::keys::app_icons as icon;
use sharex_ui::theme::{Thickness, fluent, palette};
use sharex_ui::widgets::{center_y, font, line_height};
use sharex_uploaders::http::ResponseInfo;

use super::layout::{self, Compact};
use super::{Cx, Dialog, Effect, Spec};

/// What the response window shows (`UploadResult`).
#[derive(Clone, Debug, Default)]
pub struct ResponseData {
    pub url: String,
    pub shortened_url: String,
    pub thumbnail_url: String,
    pub deletion_url: String,
    pub is_error: bool,
    /// `ErrorsToString()`.
    pub errors: String,
    pub response: String,
    pub response_info: Option<ResponseInfo>,
}

/// `AppendInfo`: "name:" and the value, blocks a blank line apart.
fn append_info(text: &mut String, name: &str, value: &str) {
    if value.is_empty() {
        return;
    }
    if !text.is_empty() {
        text.push_str("\r\n\r\n");
    }
    text.push_str(name);
    text.push_str(":\r\n");
    text.push_str(value);
}

/// `BuildResultText`.
pub fn result_text(r: &ResponseData) -> String {
    let mut text = String::new();
    append_info(&mut text, s::ResponseWindow_Shortened_URL(), &r.shortened_url);
    append_info(&mut text, s::ResponseWindow_URL(), &r.url);
    append_info(&mut text, s::ResponseWindow_Thumbnail_URL(), &r.thumbnail_url);
    append_info(&mut text, s::ResponseWindow_Deletion_URL(), &r.deletion_url);
    if r.is_error {
        append_info(&mut text, sharex_strings::uploaders_lib::Common_Error(), &r.errors);
    }
    if text.is_empty() { s::ResponseWindow_No_result_details_are_available().to_owned() } else { text }
}

/// `BuildResponseInfoText`.
pub fn response_info_text(info: &ResponseInfo) -> String {
    let mut text = String::new();
    append_info(&mut text, s::ResponseWindow_Status_code(), &format!("({}) {}", info.status_code, info.status_description));
    append_info(&mut text, s::ResponseWindow_Response_URL(), info.response_url.as_deref().unwrap_or(""));
    if !info.headers.is_empty() {
        append_info(&mut text, s::ResponseWindow_Headers(), info.headers_to_string().trim_end_matches(['\r', '\n']));
    }
    append_info(&mut text, s::ResponseWindow_Response_text(), info.response_text.as_deref().unwrap_or(""));
    text
}

/// `Helpers.JSONFormat(text, Formatting.Indented)`: Json.NET's two-space
/// indentation.
fn json_format(text: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    let s = serde_json::to_string_pretty(&value).ok()?;
    Some(s.replace('\n', "\r\n"))
}

/// `Helpers.XMLFormat`: the document indented by two spaces.
fn xml_format(text: &str) -> Option<String> {
    let mut out = String::new();
    let mut depth = 0usize;
    let mut rest = text.trim();
    if !rest.starts_with('<') {
        return None;
    }
    let mut opened = 0i32;
    while !rest.is_empty() {
        let start = rest.find('<')?;
        let between = rest[..start].trim();
        if !between.is_empty() {
            out.push_str(between);
        }
        let end = rest[start..].find('>')? + start;
        let tag = &rest[start..=end];
        let closing = tag.starts_with("</");
        let self_closing = tag.ends_with("/>") || tag.starts_with("<?") || tag.starts_with("<!");
        if closing {
            depth = depth.saturating_sub(1);
            opened -= 1;
        }
        // A text-only element stays on one line.
        if !closing || between.is_empty() {
            if !out.is_empty() {
                out.push_str("\r\n");
            }
            out.push_str(&"  ".repeat(depth));
        }
        out.push_str(tag);
        if !closing && !self_closing {
            depth += 1;
            opened += 1;
        }
        rest = &rest[end + 1..];
    }
    (opened == 0).then_some(out)
}

pub struct ResponseWindow {
    data: ResponseData,
    pub section: usize,
    response_text: String,
    format_status: Option<String>,
}

impl ResponseWindow {
    pub fn new(data: ResponseData) -> Self {
        let mut w = Self { data: ResponseData::default(), section: 0, response_text: String::new(), format_status: None };
        w.update(data);
        w
    }

    /// `UpdateResult` (`ShowInstance` reuses the open window).
    pub fn update(&mut self, data: ResponseData) {
        self.response_text = data.response_info.as_ref().and_then(|i| i.response_text.clone()).unwrap_or_else(|| data.response.clone());
        self.data = data;
        self.format_status = None;
    }

    fn summary(&self) -> &'static str {
        if self.data.is_error {
            s::ResponseWindow_The_upload_completed_with_an_error()
        } else {
            s::ResponseWindow_Inspect_the_URLs_and_response_returned_by_the_destination()
        }
    }
}

/// A `Button.compact` with an icon (15px) and a `TextBlock`, 6px apart;
/// returns its rect and whether it was clicked.
pub fn icon_text_button(ui: &egui::Ui, pos: egui::Pos2, id: egui::Id, key: &str, text: &str, padding: Thickness) -> (Rect, bool) {
    let p = palette(ui.ctx());
    let f = font(13.0, 400);
    let w = 1.0 + padding.left + 15.0 + 6.0 + layout::width(ui, text, f.clone()) + padding.right + 1.0;
    let rect = Rect::from_min_size(pos, vec2(w, 40.0));
    let r = Button { padding, ..Button::default() }.show(ui, rect, id, "", true);
    let color = if r.hovered() { fluent(ui.ctx()).base_high } else { p.text };
    let inner = rect.shrink2(vec2(1.0 + padding.left, 1.0 + padding.top));
    icons::paint(ui.painter(), Rect::from_min_size(pos2(inner.min.x, center_y(rect, 15.0)), Vec2::splat(15.0)), key, color);
    layout::text(ui, pos2(inner.min.x + 21.0, center_y(rect, line_height(ui, &f))), text, f, color, f32::INFINITY);
    (rect, r.clicked())
}

impl Dialog for ResponseWindow {
    fn spec(&self) -> Spec {
        Spec::sized(crate::rs_title(s::ResponseWindow_ShareX_Response()), 820.0, 600.0, 620.0, 440.0)
    }

    fn single(&self) -> Option<&'static str> {
        Some("response")
    }

    fn show_response(&mut self, data: ResponseData) {
        self.update(data);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _cx: &mut Cx) -> Vec<Effect> {
        let p = palette(ui.ctx());
        let f = fluent(ui.ctx());
        let full = layout::background(ui);
        // The header: `Padding="20,18"`, a 1px bottom border, a 44px accent
        // tile (10px corners) with the server icon, title and summary.
        let title_font = font(24.0, 600);
        let desc_font = font(13.0, 600);
        let text_w = full.width() - 40.0 - 44.0 - 14.0;
        let desc_lines = sharex_ui::controls::wrap_lines(ui, self.summary(), &desc_font, text_w);
        let stack = line_height(ui, &title_font) + 3.0 + line_height(ui, &desc_font) * desc_lines.len() as f32;
        let header_h = 18.0 + stack.max(44.0) + 18.0 + 1.0;
        let band = Rect::from_min_size(full.min, vec2(full.width(), header_h));
        ui.painter().rect_filled(band, 0.0, p.background_toolbar);
        ui.painter().rect_filled(Rect::from_min_size(pos2(band.min.x, band.max.y - 1.0), vec2(band.width(), 1.0)), 0.0, p.border);
        let row = Rect::from_min_max(pos2(band.min.x + 20.0, band.min.y + 18.0), pos2(band.max.x - 20.0, band.max.y - 19.0));
        let tile = Rect::from_min_size(pos2(row.min.x, center_y(row, 44.0)), Vec2::splat(44.0));
        border::border(ui.painter(), tile, 10.0, 0.0, Some(Brush::accent(ui.ctx())), None);
        icons::paint(ui.painter(), Rect::from_center_size(tile.center(), Vec2::splat(22.0)), icon::Server, p.accent_foreground);
        let x = tile.max.x + 14.0;
        let mut y = center_y(row, stack);
        layout::text(ui, pos2(x, y), s::ResponseWindow_Upload_response(), title_font.clone(), p.text, text_w);
        y += line_height(ui, &title_font) + 3.0;
        for line in &desc_lines {
            layout::text(ui, pos2(x, y), line, desc_font.clone(), p.text_secondary, text_w);
            y += line_height(ui, &desc_font);
        }
        // The section list: a toolbar strip (`Padding="4"`, 6px corners) of
        // items (`Padding="11,6"`, `Margin="0,0,3,0"`), the selected one on
        // the accent.
        let body = Rect::from_min_max(pos2(full.min.x + 16.0, band.max.y + 16.0), pos2(full.max.x - 16.0, full.max.y - 16.0));
        let item_font = font(13.0, 600);
        let sections = [
            (icon::CircleCheck, s::ResponseWindow_Result()),
            (icon::Info, s::ResponseWindow_Response_info()),
            (icon::Code, s::ResponseWindow_Response_text()),
        ];
        let item_h = 6.0 + line_height(ui, &item_font).max(15.0) + 6.0;
        let widths: Vec<f32> = sections.iter().map(|(_, t)| 11.0 + 15.0 + 6.0 + layout::width(ui, t, item_font.clone()) + 11.0).collect();
        let strip_w = 4.0 + widths.iter().map(|w| w + 3.0).sum::<f32>() + 4.0;
        let strip = Rect::from_min_size(body.min, vec2(strip_w, 4.0 + item_h + 4.0));
        border::border(ui.painter(), strip, 6.0, 0.0, Some(Brush::Solid(p.background_toolbar)), None);
        let mut x = strip.min.x + 4.0;
        for (i, ((key, text), w)) in sections.iter().zip(&widths).enumerate() {
            let r = Rect::from_min_size(pos2(x, strip.min.y + 4.0), vec2(*w, item_h));
            let resp = ui.interact(r, ui.id().with(("section", i)), egui::Sense::click());
            let color = if self.section == i {
                border::border(ui.painter(), r, 4.0, 0.0, Some(Brush::accent(ui.ctx())), None);
                p.accent_foreground
            } else {
                if resp.hovered() {
                    border::border(ui.painter(), r, 4.0, 0.0, Some(Brush::Solid(p.control_background_hover)), None);
                }
                p.text_secondary
            };
            icons::paint(ui.painter(), Rect::from_min_size(pos2(r.min.x + 11.0, center_y(r, 15.0)), Vec2::splat(15.0)), key, color);
            layout::text(ui, pos2(r.min.x + 32.0, center_y(r, line_height(ui, &item_font))), text, item_font.clone(), color, f32::INFINITY);
            if resp.clicked() {
                self.section = i;
            }
            x += w + 3.0;
        }
        // The section: a card of buttons, 8px, then the text card.
        let panel = Rect::from_min_max(pos2(body.min.x, strip.max.y + 8.0), body.max);
        let pad = Thickness::hv(10.0, 4.0);
        let d = self.data.clone();
        let buttons: Vec<(&str, &str, Effect)> = match self.section {
            0 => {
                let mut b = Vec::new();
                if !d.shortened_url.is_empty() {
                    b.push((icon::Copy, s::ResponseWindow_Copy_shortened_URL(), Effect::CopyText(d.shortened_url.clone())));
                }
                if !d.url.is_empty() {
                    b.push((icon::Copy, s::ResponseWindow_Copy_URL(), Effect::CopyText(d.url.clone())));
                    b.push((icon::ExternalLink, s::ResponseWindow_Open_URL(), Effect::OpenUrl(d.url.clone())));
                }
                if !d.thumbnail_url.is_empty() {
                    b.push((icon::Copy, s::ResponseWindow_Copy_thumbnail_URL(), Effect::CopyText(d.thumbnail_url.clone())));
                }
                if !d.deletion_url.is_empty() {
                    b.push((icon::Copy, s::ResponseWindow_Copy_deletion_URL(), Effect::CopyText(d.deletion_url.clone())));
                }
                b
            }
            1 => {
                let info_text = d.response_info.as_ref().map(response_info_text).unwrap_or_else(|| s::ResponseWindow_No_response_information_is_available().to_owned());
                let mut b = vec![(icon::Copy, s::ResponseWindow_Copy_response_info(), Effect::CopyText(info_text))];
                if let Some(url) = d.response_info.as_ref().and_then(|i| i.response_url.clone()).filter(|u| !u.is_empty()) {
                    b.push((icon::ExternalLink, s::ResponseWindow_Open_response_URL(), Effect::OpenUrl(url)));
                }
                b
            }
            _ => vec![
                (icon::FileJson, s::ResponseWindow_JSON_format(), Effect::NotPorted("json")),
                (icon::Code, s::ResponseWindow_XML_format(), Effect::NotPorted("xml")),
                (icon::Copy, s::ResponseWindow_Copy_response(), Effect::CopyText(self.response_text.clone())),
            ],
        };
        let mut out = Vec::new();
        let card = Rect::from_min_size(panel.min, vec2(panel.width(), 8.0 + 40.0 + 8.0 + 2.0));
        layout::panel(ui, card, 6.0);
        let mut x = card.min.x + 9.0;
        for (i, (key, text, effect)) in buttons.into_iter().enumerate() {
            let (r, clicked) = icon_text_button(ui, pos2(x, card.min.y + 9.0), ui.id().with(("button", self.section, i)), key, text, pad);
            if clicked {
                match (self.section, i) {
                    (2, 0) => self.format(json_format, s::ResponseWindow_JSON_formatting_failed()),
                    (2, 1) => self.format(xml_format, s::ResponseWindow_XML_formatting_failed()),
                    _ => out.push(effect),
                }
            }
            x = r.max.x + 6.0;
        }
        let mut text_top = card.max.y + 8.0;
        if self.section == 2 {
            // `FormatStatusText`'s row: its `RowSpacing` stays when it's hidden.
            if let Some(status) = &self.format_status {
                let sf = layout::text_font();
                layout::text(ui, pos2(panel.min.x, text_top), status, sf.clone(), p.cancel_border, panel.width());
                text_top += line_height(ui, &sf);
            }
            text_top += 8.0;
        }
        let text_card = Rect::from_min_max(pos2(panel.min.x, text_top), panel.max);
        layout::panel(ui, text_card, 6.0);
        let mono = egui::FontId::new(12.0, egui::FontFamily::Monospace);
        let tb = TextBox { padding: Thickness::all(11.0), multiline: true, plain: true, font: Some(mono), ..TextBox::default() };
        let inner = text_card.shrink(1.0);
        match self.section {
            0 => {
                let mut t = result_text(&self.data);
                TextBox { read_only: true, ..tb }.show(ui, inner, ui.id().with("result"), &mut t);
            }
            1 => {
                let mut t = self.data.response_info.as_ref().map(response_info_text).unwrap_or_else(|| s::ResponseWindow_No_response_information_is_available().to_owned());
                TextBox { read_only: true, ..tb }.show(ui, inner, ui.id().with("info"), &mut t);
            }
            _ => {
                tb.show(ui, inner, ui.id().with("text"), &mut self.response_text);
            }
        }
        let _ = f;
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            out.push(Effect::Close);
        }
        out
    }
}

impl ResponseWindow {
    /// `FormatResponse`.
    fn format(&mut self, formatter: fn(&str) -> Option<String>, failure: &str) {
        if self.response_text.is_empty() {
            return;
        }
        match formatter(&self.response_text) {
            Some(t) => {
                self.response_text = t;
                self.format_status = None;
            }
            None => self.format_status = Some(failure.to_owned()),
        }
    }
}

/// `ErrorWindow(errorTitle, errorMessage, logFilePath, bugReportPath,
/// unhandledException: false)`: the title, the message read-only in
/// monospace, Open log file and OK.
pub struct ErrorWindow {
    title: String,
    message: String,
    log_file: Option<std::path::PathBuf>,
    opened: bool,
}

impl ErrorWindow {
    pub fn new(title: String, message: String, log_file: Option<std::path::PathBuf>) -> Self {
        Self { title, message, log_file: log_file.filter(|p| p.is_file()), opened: false }
    }
}

impl Dialog for ErrorWindow {
    fn spec(&self) -> Spec {
        Spec { topmost: true, ..Spec::sized(crate::rs_title(hs::ErrorWindow_Title()), 760.0, 560.0, 620.0, 440.0) }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _cx: &mut Cx) -> Vec<Effect> {
        let p = palette(ui.ctx());
        let full = layout::background(ui);
        let header = layout::header(ui, full, icon::Warning, hs::ErrorWindow_Heading(), Some(hs::ErrorWindow_Description()), false);
        let grid = Rect::from_min_max(pos2(full.min.x + 18.0, full.min.y + header + 14.0), pos2(full.max.x - 18.0, full.max.y - 18.0));
        // The title card: `Padding="11,9"`, 14px wrapped.
        let tf = font(14.0, 600);
        let lines = sharex_ui::controls::wrap_lines(ui, &self.title, &tf, grid.width() - 24.0);
        let lh = line_height(ui, &tf);
        let card = Rect::from_min_size(grid.min, vec2(grid.width(), 2.0 + 18.0 + lh * lines.len() as f32));
        layout::panel(ui, card, 6.0);
        for (i, line) in lines.iter().enumerate() {
            layout::text(ui, pos2(card.min.x + 12.0, card.min.y + 10.0 + lh * i as f32), line, tf.clone(), p.text_secondary, f32::INFINITY);
        }
        // `Button.compact`: 36px tall, `MinWidth="104"`, `Padding="12,5"`.
        let compact = Compact { min_width: 104.0 };
        let button = |ui: &egui::Ui, rect: Rect, id: &str, text: &str, primary: bool| {
            Button { padding: Thickness::hv(12.0, 5.0), primary, ..Button::default() }.show(ui, rect, ui.id().with(id), text, true).clicked()
        };
        let bottom = grid.max.y - 36.0;
        let text_rect = Rect::from_min_max(pos2(grid.min.x, card.max.y + 10.0), pos2(grid.max.x, bottom - 10.0));
        let mono = egui::FontId::new(12.0, egui::FontFamily::Monospace);
        let id = ui.id().with("exception");
        TextBox { padding: Thickness::hv(9.0, 7.0), multiline: true, read_only: true, font: Some(mono), ..TextBox::default() }.show(ui, text_rect, id, &mut self.message.clone());
        if !self.opened {
            // `OnOpened`: focus, the caret at the end.
            self.opened = true;
            super::text_upload::focus_text_box(ui, id, "", false);
        }
        let mut out = Vec::new();
        let ok_w = compact.width(ui, hs::ErrorWindow_OK()).max(104.0);
        let ok = Rect::from_min_size(pos2(grid.max.x - ok_w, bottom), vec2(ok_w, 36.0));
        if button(ui, ok, "ok", hs::ErrorWindow_OK(), true) || ui.input(|i| i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::Escape)) {
            out.push(Effect::Close);
        }
        if let Some(log) = &self.log_file {
            let text = hs::ErrorWindow_Open_log_file();
            let w = compact.width(ui, text).max(104.0);
            if button(ui, Rect::from_min_size(pos2(grid.min.x, bottom), vec2(w, 36.0)), "log", text, false) {
                out.push(Effect::OpenFile(log.clone()));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_text_lists_what_there_is() {
        let r = ResponseData { url: "https://a.example/x.png".into(), deletion_url: "https://a.example/d".into(), ..Default::default() };
        assert_eq!(result_text(&r), "URL:\r\nhttps://a.example/x.png\r\n\r\nDeletion URL:\r\nhttps://a.example/d");
        assert_eq!(result_text(&ResponseData::default()), s::ResponseWindow_No_result_details_are_available());
    }

    #[test]
    fn json_is_indented_by_two() {
        assert_eq!(json_format("{\"a\":[1,2]}").unwrap(), "{\r\n  \"a\": [\r\n    1,\r\n    2\r\n  ]\r\n}");
        assert!(json_format("not json").is_none());
    }

    #[test]
    fn xml_is_indented_by_two() {
        assert_eq!(xml_format("<a><b>x</b><c/></a>").unwrap(), "<a>\r\n  <b>x</b>\r\n  <c/>\r\n</a>");
    }
}
