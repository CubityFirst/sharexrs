//! `ShareX.Tools/Tools/ClipboardViewer` (`ClipboardViewerWindow.axaml`,
//! `ClipboardViewerViewModel`): the clipboard's formats (read through a
//! [`ClipboardSource`]), the selected one's preview (text in a read-only
//! Consolas box, an image in the comparison slider) and details, Refresh
//! (keeping the selection when the format is still there) and Clear.

use egui::{FontId, Id, Rect, Ui, Vec2, pos2, vec2};
use sharex_strings::tools as s;
use sharex_tools::system::clipboard_viewer::{self as cv, ClipboardSnapshot, Preview};
use sharex_ui::controls::{self, TextBox};
use sharex_ui::keys::app_icons as icon;
use sharex_ui::theme::{Thickness, palette};
use sharex_ui::widgets::{center_y, galley, line_height, text_width};

use super::native::ClipboardSource;
use crate::dialogs::{Cx, Dialog, Effect, Spec};
use crate::image_tools::{self as tools, Picture};

pub const WIDTH: f32 = 900.0;
pub const HEIGHT: f32 = 580.0;
/// The formats column (`ColumnDefinitions="250,*"`).
const LIST_W: f32 = 250.0;

pub struct ClipboardViewerWindow {
    source: Box<dyn ClipboardSource>,
    snapshot: Option<ClipboardSnapshot>,
    pub formats: Vec<String>,
    pub selected: Option<String>,
    preview_text: String,
    preview_image: Option<Picture>,
    pub preview_details: String,
    pub empty_message: String,
    hovered: Option<usize>,
    list_scroll: f32,
}

impl ClipboardViewerWindow {
    /// `new ClipboardViewerViewModel()`: it reads the clipboard at once.
    pub fn new(source: Box<dyn ClipboardSource>) -> Self {
        let mut w = Self {
            source,
            snapshot: None,
            formats: Vec::new(),
            selected: None,
            preview_text: String::new(),
            preview_image: None,
            preview_details: s::ClipboardViewerViewModel_Select_format().to_owned(),
            empty_message: s::ClipboardViewerViewModel_Clipboard_empty().to_owned(),
            hovered: None,
            list_scroll: 0.0,
        };
        w.refresh();
        w
    }

    /// `ClearPreview`.
    fn clear_preview(&mut self) {
        self.preview_image = None;
        self.preview_text.clear();
        self.preview_details = s::ClipboardViewerViewModel_Select_format().to_owned();
    }

    /// `Refresh`.
    pub fn refresh(&mut self) {
        let previous = self.selected.take();
        self.clear_preview();
        match self.source.capture() {
            Ok(snapshot) => {
                self.formats = snapshot.formats().into_iter().map(str::to_owned).collect();
                self.snapshot = Some(snapshot);
                self.empty_message = s::ClipboardViewerViewModel_Clipboard_empty().to_owned();
                let names: Vec<&str> = self.formats.iter().map(String::as_str).collect();
                let pick = cv::reselect(&names, previous.as_deref()).map(str::to_owned);
                self.select(pick);
            }
            Err(e) => {
                eprintln!("ClipboardViewerViewModel: Failed to read clipboard contents. {e}");
                self.snapshot = None;
                self.formats.clear();
                self.empty_message = super::super::data_tools::format(s::ClipboardViewerViewModel_Unable_read_clipboard(), &[&e]);
            }
        }
    }

    /// `ClearClipboard`.
    pub fn clear_clipboard(&mut self) {
        self.source.clear();
        self.refresh();
    }

    /// `OnSelectedFormatChanged`.
    pub fn select(&mut self, format: Option<String>) {
        self.selected = format.clone();
        self.clear_preview();
        let (Some(f), Some(snapshot)) = (format, &self.snapshot) else { return };
        if f.trim().is_empty() {
            return;
        }
        self.empty_message = s::ClipboardViewerViewModel_No_previewable_content().to_owned();
        match snapshot.preview(&f) {
            Ok(p) => {
                self.preview_details = cv::preview_details(&p);
                match p {
                    Preview::Image(bmp) => {
                        let rgba = image::RgbaImage::from_raw(bmp.width(), bmp.height(), bmp.to_straight());
                        self.preview_image = rgba.map(Picture::new);
                    }
                    Preview::Text(t) => self.preview_text = t,
                }
            }
            Err(e) => {
                eprintln!("ClipboardViewerViewModel: Failed to preview clipboard format '{f}'. {e}");
                self.preview_text = cv::preview_failed_text(&e);
                self.preview_details = s::ClipboardViewerViewModel_Preview_unavailable().to_owned();
            }
        }
    }

    fn row_height(ui: &Ui) -> f32 {
        // `ListBoxItem` `Padding="10,8"` around the template's line.
        8.0 + line_height(ui, &tools::normal_font()) + 8.0
    }

    /// The formats panel.
    fn formats_panel(&mut self, ui: &mut Ui, rect: Rect) {
        let p = palette(ui.ctx());
        tools::panel(ui, rect);
        let inner = rect.shrink(13.0);
        let hh = tools::heading_height(ui);
        let head = Rect::from_min_size(inner.min, vec2(inner.width(), hh));
        tools::heading(ui, head, s::ClipboardViewerWindow_Formats());
        let count = cv::format_count_text(self.formats.len());
        let nf = tools::normal_font();
        let cw = text_width(ui, &count, nf.clone());
        tools::text_at(ui, pos2(head.max.x - cw, center_y(head, line_height(ui, &nf))), &count, nf.clone(), p.text_secondary);
        let list = Rect::from_min_max(pos2(inner.min.x, head.max.y + 10.0), inner.max);
        // The `ListBox`: Fluent's chrome background.
        let f = sharex_ui::theme::fluent(ui.ctx());
        ui.painter().rect_filled(list, 0.0, f.chrome_medium_low);
        let row_h = Self::row_height(ui);
        let content = row_h * self.formats.len() as f32;
        let max_scroll = (content - list.height()).max(0.0);
        if ui.rect_contains_pointer(list) {
            self.list_scroll -= ui.input(|i| i.smooth_scroll_delta.y);
        }
        self.list_scroll = self.list_scroll.clamp(0.0, max_scroll);
        let painter = ui.painter().with_clip_rect(list.intersect(ui.clip_rect()));
        let mut clicked = None;
        self.hovered = None;
        for (i, name) in self.formats.iter().enumerate() {
            let row = Rect::from_min_size(pos2(list.min.x, list.min.y + row_h * i as f32 - self.list_scroll), vec2(list.width(), row_h));
            if !row.intersects(list) {
                continue;
            }
            let r = ui.interact(row.intersect(list), Id::new(("clipboard-format", i)), egui::Sense::click());
            if r.clicked() {
                clicked = Some(i);
            }
            if r.hovered() {
                self.hovered = Some(i);
            }
            let selected = self.selected.as_deref() == Some(name.as_str());
            controls::list_item_background(&painter, row, 0.0, r.hovered(), r.is_pointer_button_down_on(), selected);
            let content = Rect::from_min_max(pos2(row.min.x + 10.0, row.min.y + 8.0), pos2(row.max.x - 10.0, row.max.y - 8.0));
            let ic = Rect::from_min_size(pos2(content.min.x, center_y(content, 13.0)), Vec2::splat(13.0));
            sharex_ui::icons::paint(&painter, ic, icon::FileText, if selected { p.accent_foreground } else { p.text_secondary });
            let tx = ic.max.x + 10.0;
            let text = tools::ellipsize(ui, name, &nf, content.max.x - tx);
            let color = if selected { p.accent_foreground } else { p.text };
            painter.galley(pos2(tx, center_y(content, line_height(ui, &nf))), galley(ui, &text, nf.clone(), color, f32::INFINITY), color);
        }
        if max_scroll > 0.0 {
            crate::dialogs::layout::scroll_bar(ui, list, content, self.list_scroll, 0.0);
        }
        if self.formats.is_empty() {
            tools::empty_state(ui, list, icon::ClipboardClear, 40.0, 8.0, &[(s::ClipboardViewerWindow_No_formats(), nf, p.text_secondary)]);
        }
        if let Some(i) = clicked {
            let f = self.formats[i].clone();
            if self.selected.as_deref() != Some(f.as_str()) {
                self.select(Some(f));
            }
        }
    }

    /// The preview panel.
    fn preview_panel(&mut self, ui: &mut Ui, rect: Rect) {
        let p = palette(ui.ctx());
        tools::panel(ui, rect);
        let inner = rect.shrink(1.0);
        let head = Rect::from_min_size(pos2(inner.min.x + 12.0, inner.min.y + 12.0), vec2(inner.width() - 24.0, tools::BUTTON_HEIGHT));
        let clear = s::ClipboardViewerWindow_Clear();
        let cw = tools::icon_text_button_width(ui, clear);
        let cr = Rect::from_min_size(pos2(head.max.x - cw, head.min.y), vec2(cw, tools::BUTTON_HEIGHT));
        let refresh = s::ClipboardViewerWindow_Refresh();
        let rw = tools::icon_text_button_width(ui, refresh);
        let rr = Rect::from_min_size(pos2(cr.min.x - 8.0 - rw, head.min.y), vec2(rw, tools::BUTTON_HEIGHT));
        // `PreviewTitle` and `PreviewDetails` (`StackPanel Spacing="2"`).
        let hf = tools::heading_font();
        let nf = tools::normal_font();
        let (h1, h2) = (line_height(ui, &hf), line_height(ui, &nf));
        let y = center_y(head, h1 + 2.0 + h2);
        let title = self.selected.clone().unwrap_or_else(|| s::ClipboardViewerViewModel_Clipboard_content().to_owned());
        let tw = rr.min.x - 8.0 - head.min.x;
        tools::text_at(ui, pos2(head.min.x, y), &tools::ellipsize(ui, &title, &hf, tw), hf, p.text);
        tools::text_at(ui, pos2(head.min.x, y + h1 + 2.0), &self.preview_details, nf.clone(), p.text_secondary);
        if tools::icon_text_button(ui, rr, Id::new("clipboard-refresh"), icon::Refresh, refresh, true).clicked() {
            self.refresh();
        }
        if tools::icon_text_button(ui, cr, Id::new("clipboard-clear"), icon::Trash, clear, !self.formats.is_empty()).clicked() {
            self.clear_clipboard();
        }
        // The preview `Border` (1px, 4px corners, clipped).
        let frame = Rect::from_min_max(pos2(inner.min.x + 12.0, head.max.y + 12.0), pos2(inner.max.x - 12.0, inner.max.y - 12.0));
        let body = frame.shrink(1.0);
        if !self.preview_text.is_empty() {
            // `TextBox.preview-text`: Consolas, Normal, `Padding="12"`, top.
            let tb = TextBox {
                read_only: true,
                multiline: true,
                padding: Thickness::all(12.0),
                font: Some(FontId::new(14.0, sharex_ui::fonts::consolas())),
                ..TextBox::default()
            };
            tb.show(ui, body, Id::new("clipboard-preview-text"), &mut self.preview_text);
        } else if let Some(pic) = self.preview_image.as_mut() {
            crate::image_tools::slider::ComparisonSlider::draw_single(ui, body, pic, crate::image_tools::slider::Stretch::Both);
        } else {
            super::super::data_tools::notice(ui, body, icon::ClipboardList, 48.0, 360.0, &self.empty_message);
        }
        sharex_ui::border::border(ui.painter(), frame, 4.0, 1.0, None, Some(p.border));
    }
}

impl Dialog for ClipboardViewerWindow {
    fn spec(&self) -> Spec {
        Spec::sized(crate::rs_title(s::ClipboardViewerWindow_ShareX_Clipboard_Viewer()), WIDTH, HEIGHT, 720.0, 460.0)
    }

    fn ui(&mut self, ui: &mut Ui, _cx: &mut Cx) -> Vec<Effect> {
        let c = tools::background(ui);
        let left = Rect::from_min_size(c.min, vec2(LIST_W, c.height()));
        self.formats_panel(ui, left);
        let right = Rect::from_min_max(pos2(left.max.x + 12.0, c.min.y), c.max);
        self.preview_panel(ui, right);
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sharex_tools::system::clipboard_viewer::ClipboardValue;
    use std::sync::{Arc, Mutex};

    /// A clipboard in memory (the real one is never touched).
    #[derive(Clone)]
    struct Fake(Arc<Mutex<ClipboardSnapshot>>);

    impl ClipboardSource for Fake {
        fn capture(&self) -> Result<ClipboardSnapshot, String> {
            Ok(self.0.lock().unwrap().clone())
        }
        fn clear(&self) {
            *self.0.lock().unwrap() = ClipboardSnapshot::new();
        }
    }

    #[test]
    fn formats_preview_refresh_and_clear() {
        let mut snap = ClipboardSnapshot::new();
        snap.set("UnicodeText", ClipboardValue::Text("hello".into()));
        snap.set("ReferenceFixtureBytes", ClipboardValue::Bytes(vec![0xAB, 0x01]));
        let fake = Fake(Arc::new(Mutex::new(snap)));
        let mut w = ClipboardViewerWindow::new(Box::new(fake.clone()));
        assert_eq!(w.formats, ["UnicodeText", "ReferenceFixtureBytes"]);
        assert_eq!(w.selected.as_deref(), Some("UnicodeText"));
        assert_eq!(w.preview_text, "hello");
        w.select(Some("ReferenceFixtureBytes".into()));
        assert_eq!(w.preview_text, "AB01");
        // Refresh keeps the selection while the format is still there.
        w.refresh();
        assert_eq!(w.selected.as_deref(), Some("ReferenceFixtureBytes"));
        w.clear_clipboard();
        assert!(w.formats.is_empty());
        assert_eq!(w.selected, None);
        assert_eq!(w.empty_message, s::ClipboardViewerViewModel_Clipboard_empty());
    }
}
