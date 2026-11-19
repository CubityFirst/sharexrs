//! `ShareX.Tools/Tools/Metadata` (`MetadataWindow.axaml`, `MetadataViewModel`)
//! with `TaskHelpers.OpenMetadataWindow`'s sound: the file (from the
//! command line, the picker the window opens with, Open... or a drop),
//! its metadata read on a thread (`sharex_tools::metadata::read`) and
//! grouped, the search, Copy all, and Strip metadata... behind its
//! confirmation (`strip`, then read again). URLs in values are links.

use std::path::PathBuf;

use egui::{Color32, Id, Rect, Stroke, Ui, Vec2, pos2, vec2};
use sharex_strings::tools as s;
use sharex_tools::metadata::{self, MetadataValue};
use sharex_ui::controls::{self, TextBox};
use sharex_ui::keys::app_icons as icon;
use sharex_ui::theme::palette;
use sharex_ui::widgets::{center_y, galley, line_height, text_width};

use crate::dialogs::{Cx, Dialog, Effect, Spec};
use crate::image_tools::{self as tools, Job};

pub const WIDTH: f32 = 900.0;
pub const HEIGHT: f32 = 650.0;
/// An entry's tag column (`ColumnDefinitions="190,*" ColumnSpacing="12"`).
const TAG_W: f32 = 190.0;
/// The search box's column.
const SEARCH_W: f32 = 260.0;

/// `MetadataEntry`: a value with the first URL in it.
#[derive(Clone, Debug)]
pub struct Entry {
    pub value: MetadataValue,
    pub url: Option<String>,
}

/// `UrlRegex`: `https?://[^\s]+`, ignoring case.
pub fn find_url(text: &str) -> Option<String> {
    let lower = text.to_ascii_lowercase();
    let start = [lower.find("http://"), lower.find("https://")].into_iter().flatten().min()?;
    let rest = &text[start..];
    let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
    let scheme = if lower[start..].starts_with("https://") { 8 } else { 7 };
    (end > scheme).then(|| rest[..end].to_owned())
}

pub struct MetadataWindow {
    pub file_path: String,
    entries: Vec<Entry>,
    pub search_text: String,
    pub empty_message: String,
    pub confirming_strip: bool,
    job: Job<Result<Vec<MetadataValue>, String>>,
    /// The notification sound after a strip.
    play_sound: bool,
    started: bool,
    /// The title last set (`WindowTitle`).
    title: String,
    scroll: f32,
    /// A strip running on the thread (`IsBusy` while it does).
    strip: Option<std::sync::mpsc::Receiver<Result<(), String>>>,
    pub sync: bool,
}

impl MetadataWindow {
    /// `new MetadataWindow(filePath, playNotificationSound)`.
    pub fn new(file_path: Option<String>) -> Self {
        Self {
            file_path: file_path.unwrap_or_default(),
            entries: Vec::new(),
            search_text: String::new(),
            empty_message: s::MetadataViewModel_Open_or_drop().to_owned(),
            confirming_strip: false,
            job: Job::new(0),
            play_sound: true,
            started: false,
            title: String::new(),
            scroll: 0.0,
            strip: None,
            sync: false,
        }
    }

    /// Reads at once, on this thread, and doesn't open the picker (`--render`).
    pub fn set_sync(&mut self) {
        self.sync = true;
        self.job.sync = true;
        self.started = true;
        self.play_sound = false;
        if tools::file_exists(&self.file_path) {
            self.load();
        }
    }

    fn file_exists(&self) -> bool {
        tools::file_exists(&self.file_path)
    }

    pub fn is_busy(&self) -> bool {
        self.job.busy || self.strip.is_some()
    }

    /// `FileName`.
    pub fn file_name(&self) -> String {
        if self.file_exists() { sharex_tools::dotnet::file_name(&self.file_path).to_owned() } else { s::MetadataViewModel_No_file_selected().to_owned() }
    }

    /// `FileDescription`.
    pub fn file_description(&self) -> String {
        if self.file_exists() { self.file_path.clone() } else { s::MetadataViewModel_Open_or_drop_supported().to_owned() }
    }

    /// `WindowTitle`.
    pub fn window_title(&self) -> String {
        let t = if self.file_exists() { super::format(s::MetadataViewModel_Window_title_file(), &[&self.file_name()]) } else { s::MetadataViewModel_Window_title().to_owned() };
        crate::rs_title(&t)
    }

    pub fn can_copy(&self) -> bool {
        !self.is_busy() && !self.entries.is_empty()
    }

    pub fn can_strip(&self) -> bool {
        !self.is_busy() && metadata::can_strip(&self.file_path)
    }

    /// `MetadataCountText`.
    pub fn count_text(&self) -> String {
        if self.entries.len() == 1 { s::MetadataViewModel_One_tag().to_owned() } else { super::format(s::MetadataViewModel_Tag_count(), &[&self.entries.len().to_string()]) }
    }

    /// `OpenFileAsync`.
    pub fn open_file(&mut self, path: &str) {
        if self.is_busy() || !tools::file_exists(path) {
            return;
        }
        self.file_path = path.to_owned();
        self.search_text.clear();
        self.load();
    }

    /// `LoadMetadataAsync` (its start; [`Self::poll`] takes the result).
    fn load(&mut self) {
        self.confirming_strip = false;
        self.entries.clear();
        self.scroll = 0.0;
        self.empty_message = s::MetadataViewModel_Reading_metadata().to_owned();
        self.job.queue();
        if self.sync {
            self.poll(&egui::Context::default());
        }
    }

    pub fn poll(&mut self, ctx: &egui::Context) {
        if let Some(rx) = &self.strip
            && let Ok(r) = rx.try_recv()
        {
            self.strip = None;
            match r {
                Ok(()) => {
                    if self.play_sound {
                        crate::sound::play_action_completed();
                    }
                    self.load();
                }
                Err(e) => {
                    eprintln!("MetadataViewModel: Failed to strip file metadata. {e}");
                    self.empty_message = super::format(s::MetadataViewModel_Unable_strip(), &[&e]);
                }
            }
        }
        let path = self.file_path.clone();
        if let Some(r) = self.job.poll(ctx, move || Box::new(move || metadata::read(&path))) {
            match r {
                Ok(values) => {
                    self.entries = values.into_iter().map(|v| Entry { url: find_url(&v.value), value: v }).collect();
                    self.empty_message = if self.entries.is_empty() { s::MetadataViewModel_No_metadata() } else { s::MetadataViewModel_No_search_matches() }.to_owned();
                }
                Err(e) => {
                    eprintln!("MetadataViewModel: Failed to read file metadata. {e}");
                    self.empty_message = super::format(s::MetadataViewModel_Unable_read(), &[&e]);
                }
            }
        }
        if self.strip.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(30));
        }
    }

    /// `ConfirmStripAsync`.
    pub fn confirm_strip(&mut self) {
        if !self.can_strip() {
            return;
        }
        self.confirming_strip = false;
        let path = self.file_path.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        let work = move || metadata::strip(&path).map_err(|e| e.to_string());
        if self.sync {
            let _ = tx.send(work());
        } else {
            std::thread::spawn(move || {
                let _ = tx.send(work());
            });
        }
        self.strip = Some(rx);
        if self.sync {
            self.poll(&egui::Context::default());
        }
    }

    /// `Groups` after `ApplyFilter`: the entries whose group, tag or value
    /// holds the search text (`metadata::filter`'s comparison), grouped
    /// as `metadata::group` does.
    fn groups(&self) -> Vec<(String, Vec<&Entry>)> {
        use sharex_helpers::culture::{invariant_ignore_case_contains, ordinal_upper};
        let f = self.search_text.trim();
        let ci = |s: &str| invariant_ignore_case_contains(s, f);
        let mut out: Vec<(String, Vec<&Entry>)> = Vec::new();
        for e in &self.entries {
            if !f.is_empty() && !(ci(&e.value.group) || ci(&e.value.tag) || ci(&e.value.value)) {
                continue;
            }
            let key = ordinal_upper(&e.value.group);
            match out.iter_mut().find(|(k, _)| ordinal_upper(k) == key) {
                Some(g) => g.1.push(e),
                None => out.push((e.value.group.clone(), vec![e])),
            }
        }
        out
    }

    /// An entry row's height: `MinHeight="32"` and `Margin="10,2"`, taller
    /// when the tag or value wraps.
    fn entry_height(ui: &Ui, e: &Entry, width: f32) -> f32 {
        let f = tools::normal_font();
        let value_w = (width - 20.0 - TAG_W - 12.0).max(1.0);
        let tag_h = crate::video_tools::wrapped_text(ui, pos2(0.0, 0.0), TAG_W, &e.value.tag, f.clone(), Color32::WHITE, false);
        let value_h = crate::video_tools::wrapped_text(ui, pos2(0.0, 0.0), value_w, &e.value.value, f, Color32::WHITE, false);
        tag_h.max(value_h).max(32.0) + 4.0
    }

    fn header_height(ui: &Ui) -> f32 {
        7.0 + tools::heading_height(ui) + 7.0
    }

    /// The top panel: the file, Open..., Copy all, Strip metadata....
    fn top_panel(&mut self, ui: &mut Ui, rect: Rect) -> Vec<Effect> {
        let p = palette(ui.ctx());
        let mut out = Vec::new();
        tools::panel(ui, rect);
        let inner = rect.shrink(13.0);
        let row = Rect::from_min_size(inner.min, vec2(inner.width(), tools::BUTTON_HEIGHT));
        let ir = Rect::from_min_size(pos2(row.min.x, center_y(row, 28.0)), Vec2::splat(28.0));
        sharex_ui::icons::paint(ui.painter(), ir, icon::FileSearch, p.text_secondary);
        let buttons = [(s::MetadataWindow_Strip_metadata(), icon::Eraser, self.can_strip()), (s::MetadataWindow_Copy_all(), icon::Copy, self.can_copy()), (s::MetadataWindow_Open(), icon::FolderOpen, !self.is_busy())];
        let mut x = row.max.x;
        let mut clicked = [false; 3];
        for (k, (text, key, enabled)) in buttons.into_iter().enumerate() {
            let w = tools::icon_text_button_width(ui, text);
            x -= w;
            clicked[k] = tools::icon_text_button(ui, Rect::from_min_size(pos2(x, row.min.y), vec2(w, row.height())), Id::new(("metadata-button", k)), key, text, enabled).clicked();
            x -= 10.0;
        }
        // The file name and path (`StackPanel Spacing="2"`, centred).
        let hf = tools::heading_font();
        let nf = tools::normal_font();
        let (h1, h2) = (line_height(ui, &hf), line_height(ui, &nf));
        let y = center_y(row, h1 + 2.0 + h2);
        let text_x = ir.max.x + 10.0;
        let tw = x - text_x;
        let name = tools::ellipsize(ui, &self.file_name(), &hf, tw);
        tools::text_at(ui, pos2(text_x, y), &name, hf, p.text);
        tools::path_text(ui, Rect::from_min_size(pos2(text_x, y + h1 + 2.0), vec2(tw, h2)), &self.file_description());
        if self.is_busy() {
            // `ProgressBar Height="5"` in the second row.
            let bar = Rect::from_min_size(pos2(inner.min.x, row.max.y + 8.0), vec2(inner.width(), 5.0));
            controls::indeterminate_progress(ui.painter(), bar, self.job.busy_since.elapsed().as_secs_f64());
            ui.ctx().request_repaint();
        }
        if clicked[2]
            && let Some(f) = Self::pick(&self.file_path)
        {
            self.open_file(&f);
        }
        if clicked[1] && self.can_copy() {
            let values: Vec<MetadataValue> = self.entries.iter().map(|e| e.value.clone()).collect();
            out.push(Effect::CopyText(metadata::build_text(&values)));
        }
        if clicked[0] && self.can_strip() {
            self.confirming_strip = true;
        }
        out
    }

    /// `SelectFileAsync`.
    fn pick(current: &str) -> Option<String> {
        let mut d = rfd::FileDialog::new().set_title(s::MetadataWindow_Open_file_dialog());
        let folder = tools::directory_name(current);
        if !folder.trim().is_empty() && std::path::Path::new(&folder).is_dir() {
            d = d.set_directory(folder);
        }
        d.pick_file().map(|p| p.to_string_lossy().into_owned())
    }

    /// The Metadata panel: the heading, the count, the search, the groups.
    fn list_panel(&mut self, ui: &mut Ui, rect: Rect) -> Vec<Effect> {
        let p = palette(ui.ctx());
        let mut out = Vec::new();
        tools::panel(ui, rect);
        let inner = rect.shrink(1.0);
        let head = Rect::from_min_size(pos2(inner.min.x + 12.0, inner.min.y + 12.0), vec2(inner.width() - 24.0, tools::CONTROL_HEIGHT));
        let hf = tools::heading_font();
        let title = s::MetadataWindow_Metadata();
        let hw = text_width(ui, title, hf.clone());
        tools::heading(ui, Rect::from_min_size(head.min, vec2(hw, head.height())), title);
        let nf = tools::normal_font();
        tools::text_at(ui, pos2(head.min.x + hw + 8.0, center_y(head, line_height(ui, &nf))), &self.count_text(), nf.clone(), p.text_secondary);
        // `ColumnDefinitions="*,260,Auto"`: the icon's column measures to
        // nothing (its margins are wider than it), the spacing stays.
        let search = Rect::from_min_size(pos2(head.max.x - 10.0 - SEARCH_W, head.min.y), vec2(SEARCH_W, head.height()));
        let before = self.search_text.clone();
        TextBox { placeholder: s::MetadataWindow_Search_metadata(), enabled: self.can_copy(), ..TextBox::default() }.show(ui, search, Id::new("metadata-search"), &mut self.search_text);
        if before != self.search_text {
            self.scroll = 0.0;
        }
        // `Margin="-38,0,12,0"` in the empty column: a 26px slot from 38px
        // left of the column, the 13px icon centred in it.
        let slot_x = head.max.x - 38.0;
        let ir = Rect::from_min_size(pos2(slot_x + ((26.0f32 - 13.0) / 2.0).round_ties_even(), center_y(head, 13.0)), Vec2::splat(13.0));
        sharex_ui::icons::paint(ui.painter(), ir, icon::Search, p.text_secondary);

        let body = Rect::from_min_max(pos2(inner.min.x + 12.0, head.max.y + 12.0), pos2(inner.max.x - 12.0, inner.max.y - 12.0));
        let groups: Vec<(String, Vec<Entry>)> = self.groups().into_iter().map(|(n, e)| (n, e.into_iter().cloned().collect())).collect();
        if groups.is_empty() {
            super::notice(ui, body, icon::Tag, 48.0, 420.0, &self.empty_message);
            return out;
        }
        // The `ScrollViewer` and its `ItemsControl` (`Margin="0,0,10,0"`).
        let width = body.width() - 10.0;
        let hh = Self::header_height(ui);
        let heights: Vec<Vec<f32>> = groups.iter().map(|(_, items)| items.iter().map(|e| Self::entry_height(ui, e, width - 2.0)).collect()).collect();
        let group_h: Vec<f32> = heights.iter().map(|h| 2.0 + hh + h.iter().sum::<f32>()).collect();
        let content = group_h.iter().map(|h| h + 7.0).sum::<f32>();
        let max_scroll = (content - body.height()).max(0.0);
        if ui.rect_contains_pointer(body) {
            self.scroll -= ui.input(|i| i.smooth_scroll_delta.y);
        }
        self.scroll = self.scroll.clamp(0.0, max_scroll);
        let clip = body.intersect(ui.clip_rect());
        let painter = ui.painter().with_clip_rect(clip);
        let mut y = body.min.y - self.scroll;
        for (g, (name, items)) in groups.iter().enumerate() {
            let gr = Rect::from_min_size(pos2(body.min.x, y), vec2(width, group_h[g]));
            y += group_h[g] + 7.0;
            if gr.max.y < body.min.y || gr.min.y > body.max.y {
                continue;
            }
            // The header (`ShareX.Brush.Control.Background`, clipped by the
            // border's corners), then the border on top.
            let header = Rect::from_min_size(pos2(gr.min.x + 1.0, gr.min.y + 1.0), vec2(gr.width() - 2.0, hh));
            sharex_ui::border::border(&painter, gr, 4.0, 0.0, None, None);
            let radius = egui::CornerRadius { nw: 3, ne: 3, ..Default::default() };
            painter.rect_filled(header, radius, p.control_background);
            sharex_ui::border::border(&painter, gr, 4.0, 1.0, None, Some(p.border));
            let hrow = header.shrink2(vec2(10.0, 7.0));
            let hf = tools::heading_font();
            painter.galley(pos2(hrow.min.x, center_y(hrow, line_height(ui, &hf))), galley(ui, name, hf, p.text, f32::INFINITY), p.text);
            let count = format!("{} tags", items.len());
            let cw = text_width(ui, &count, nf.clone());
            painter.galley(pos2(hrow.max.x - cw, center_y(hrow, line_height(ui, &nf))), galley(ui, &count, nf.clone(), p.text_secondary, f32::INFINITY), p.text_secondary);
            let mut ey = header.max.y;
            for (k, e) in items.iter().enumerate() {
                let eh = heights[g][k];
                let row = Rect::from_min_max(pos2(header.min.x + 10.0, ey + 2.0), pos2(header.max.x - 10.0, ey + eh - 2.0));
                ey += eh;
                if row.max.y < body.min.y || row.min.y > body.max.y {
                    continue;
                }
                let tag_h = crate::video_tools::wrapped_text(ui, pos2(0.0, 0.0), TAG_W, &e.value.tag, nf.clone(), p.text_secondary, false);
                let child = ui.new_child(egui::UiBuilder::new().max_rect(clip));
                let mut child = child;
                child.set_clip_rect(clip);
                crate::video_tools::wrapped_text(&child, pos2(row.min.x, center_y(row, tag_h)), TAG_W, &e.value.tag, nf.clone(), p.text_secondary, true);
                let vx = row.min.x + TAG_W + 12.0;
                let vw = (row.max.x - vx).max(1.0);
                let color = if e.url.is_some() { p.accent_text } else { p.text };
                let value_h = crate::video_tools::wrapped_text(ui, pos2(0.0, 0.0), vw, &e.value.value, nf.clone(), color, false);
                let vy = center_y(row, value_h);
                crate::video_tools::wrapped_text(&child, pos2(vx, vy), vw, &e.value.value, nf.clone(), color, true);
                if let Some(url) = &e.url {
                    // `TextDecorations="Underline"` under each line; a click opens the URL.
                    let lines = controls::wrap_lines(ui, &e.value.value, &nf, vw);
                    let exact = ui.fonts_mut(|f| f.row_height(&nf));
                    for (i, line) in lines.iter().enumerate() {
                        let lw = text_width(ui, line, nf.clone());
                        let ly = vy + (exact * i as f32).round() + exact.round() - 1.5;
                        painter.line_segment([pos2(vx, ly), pos2(vx + lw, ly)], Stroke::new(1.0, color));
                    }
                    let hit = Rect::from_min_size(pos2(vx, vy), vec2(vw, value_h)).intersect(clip);
                    let r = ui.interact(hit, Id::new(("metadata-url", g, k)), egui::Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
                    if r.clicked() {
                        out.push(Effect::OpenUrl(url.clone()));
                    }
                }
            }
        }
        if max_scroll > 0.0 {
            crate::dialogs::layout::scroll_bar(ui, body, content, self.scroll, 0.0);
        }
        out
    }

    /// The strip confirmation over the window (`IsConfirmingStrip`).
    fn confirmation(&mut self, ui: &mut Ui, full: Rect) {
        let p = palette(ui.ctx());
        // The overlay takes the pointer away from the window under it.
        let _ = ui.interact(full, Id::new("metadata-modal"), egui::Sense::click());
        ui.painter().rect_filled(full, 0.0, p.overlay_modal);
        let width = 470.0;
        let content_w = width - 38.0;
        let hf = tools::heading_font();
        let nf = tools::normal_font();
        let message = s::MetadataWindow_This_permanently_removes_all_non_essential_metadata_from_the_selected_file_This_action_cannot_be_undone();
        let row1 = line_height(ui, &hf).max(24.0);
        let msg_h = crate::video_tools::wrapped_text(ui, pos2(0.0, 0.0), content_w, message, nf.clone(), p.text, false);
        let height = 38.0 + row1 + 14.0 + msg_h + 14.0 + tools::BUTTON_HEIGHT;
        let panel = Rect::from_min_size(pos2(full.min.x + ((full.width() - width) / 2.0).round_ties_even(), full.min.y + ((full.height() - height) / 2.0).round_ties_even()), vec2(width, height));
        tools::panel(ui, panel);
        let inner = panel.shrink(19.0);
        let r1 = Rect::from_min_size(inner.min, vec2(inner.width(), row1));
        sharex_ui::icons::paint(ui.painter(), Rect::from_min_size(pos2(r1.min.x, center_y(r1, 24.0)), Vec2::splat(24.0)), icon::Warning, p.cancel_border);
        tools::text_at(ui, pos2(r1.min.x + 24.0 + 10.0, center_y(r1, line_height(ui, &hf))), s::MetadataWindow_Strip_all_metadata(), hf, p.text);
        let my = r1.max.y + 14.0;
        crate::video_tools::wrapped_text(ui, pos2(inner.min.x, my), content_w, message, nf, p.text, true);
        let by = my + msg_h + 14.0;
        let cancel = s::MetadataWindow_Cancel();
        let cw = tools::text_button_width(ui, cancel);
        let cr = Rect::from_min_size(pos2(inner.max.x - cw, by), vec2(cw, tools::BUTTON_HEIGHT));
        let strip = s::MetadataWindow_Strip_metadata_2();
        let sw = tools::icon_text_button_width(ui, strip);
        let sr = Rect::from_min_size(pos2(cr.min.x - 8.0 - sw, by), vec2(sw, tools::BUTTON_HEIGHT));
        if tools::icon_text_button(ui, sr, Id::new("metadata-strip-confirm"), icon::Eraser, strip, true).clicked() {
            self.confirm_strip();
        }
        if tools::text_button(ui, cr, Id::new("metadata-strip-cancel"), cancel, true).clicked() {
            self.confirming_strip = false;
        }
    }
}

impl Dialog for MetadataWindow {
    fn spec(&self) -> Spec {
        Spec::sized(self.window_title(), WIDTH, HEIGHT, 700.0, 500.0)
    }

    fn ui(&mut self, ui: &mut Ui, _cx: &mut Cx) -> Vec<Effect> {
        let ctx = ui.ctx().clone();
        if !self.started {
            // `OnOpened` → `StartAsync`: the file, else the picker.
            self.started = true;
            if self.file_exists() {
                self.load();
            } else if let Some(f) = Self::pick(&self.file_path) {
                self.open_file(&f);
            }
        }
        self.poll(&ctx);
        let mut out = Vec::new();
        let full = ui.max_rect();
        let c = tools::background(ui);
        let modal = self.confirming_strip;
        // Under the confirmation, the window's controls don't take input.
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(full));
        if modal {
            child.disable();
            child.set_opacity(1.0);
        }
        let ui2 = &mut child;
        let top_h = 26.0 + tools::BUTTON_HEIGHT + 8.0;
        let top = Rect::from_min_size(c.min, vec2(c.width(), top_h));
        out.extend(self.top_panel(ui2, top));
        let list = Rect::from_min_max(pos2(c.min.x, top.max.y + 12.0), c.max);
        out.extend(self.list_panel(ui2, list));
        if modal {
            self.confirmation(ui, full);
        }
        let title = self.window_title();
        if title != self.title {
            if !self.title.is_empty() {
                out.push(Effect::SetTitle(title.clone()));
            }
            self.title = title;
        }
        out
    }

    fn dropped(&mut self, files: Vec<PathBuf>, _cx: &mut Cx) -> Vec<Effect> {
        if let Some(f) = files.iter().find(|f| f.is_file()) {
            self.open_file(&f.to_string_lossy());
        }
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_as_the_regex_finds_them() {
        assert_eq!(find_url("see https://example.invalid/a?b=1 now").as_deref(), Some("https://example.invalid/a?b=1"));
        assert_eq!(find_url("HTTP://X.Y"), Some("HTTP://X.Y".into()));
        assert_eq!(find_url("http:// nothing"), None);
        assert_eq!(find_url("no link"), None);
    }

    #[test]
    fn reads_filters_and_strips_a_copy() {
        let dir = std::env::temp_dir().join(format!("sharexrs-metadata-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("a.png");
        image::RgbaImage::from_pixel(4, 4, image::Rgba([1, 2, 3, 255])).save(&path).unwrap();
        let mut w = MetadataWindow::new(Some(path.to_string_lossy().into_owned()));
        w.set_sync();
        assert!(w.can_copy());
        assert!(w.window_title().ends_with("a.png"));
        let all = w.groups().iter().map(|(_, e)| e.len()).sum::<usize>();
        assert_eq!(all, w.entries.len());
        w.search_text = "File Name".into();
        let groups = w.groups();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].1[0].value.value, "a.png");
        w.search_text = "zzzz-no-match".into();
        assert!(w.groups().is_empty());
        assert_eq!(w.empty_message, s::MetadataViewModel_No_search_matches());
        assert!(w.can_strip());
        w.confirming_strip = true;
        w.confirm_strip();
        assert!(!w.confirming_strip);
        assert!(!w.is_busy());
    }
}
