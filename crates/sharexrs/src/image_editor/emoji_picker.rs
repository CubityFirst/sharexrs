//! `EmojiPickerDialogView` with its `EmojiPickerDialogViewModel`: the
//! search box, the group combo box and the group's emoji (or the search's
//! matches) as 52px tiles drawn by `EmojiPreviewImage`
//! (`WindowsEmojiBitmapRenderer.RenderPreviewBitmap`, 48px), a click picks
//! one.

use std::collections::HashMap;

use egui::{Color32, CornerRadius, Id, Rect, Sense, TextureHandle, Ui, Vec2, pos2, vec2};
use sharex_image_editor::emoji_catalog::{self, EmojiEntry};
use sharex_image_editor::render::emoji;
use sharex_strings::image_editor as ie;
use sharex_ui::controls::{Button, ComboBox, ComboItem, TextBox};
use sharex_ui::theme::Thickness;
use sharex_ui::{Palette, border, widgets};

use super::modal::text_font;

const WIDTH: f32 = 760.0;
const HEIGHT: f32 = 640.0;
/// A tile (`Width="52"`, `Margin="2"`).
const TILE: f32 = 52.0;
const CELL: f32 = TILE + 4.0;
const PREVIEW: u32 = 48;
/// Previews rendered a frame (ShareX's `PreviewRenderThrottle` renders
/// them in the background, four at a time).
const PREVIEWS_PER_FRAME: usize = 6;

pub struct EmojiPicker {
    groups: Vec<String>,
    group: usize,
    search: String,
    scroll: f32,
    focus: bool,
    textures: HashMap<String, Option<TextureHandle>>,
    unthrottled: bool,
    /// The annotation to change instead of a new one
    /// (`ShowEmojiPickerForReplacement`).
    pub replace: Option<String>,
}

pub enum Outcome {
    Pick(&'static EmojiEntry),
    Cancel,
}

impl EmojiPicker {
    pub fn new(replace: Option<String>) -> Self {
        Self { groups: emoji_catalog::groups(), group: 0, search: String::new(), scroll: 0.0, focus: true, textures: HashMap::new(), unthrottled: false, replace }
    }

    /// For `--render`: a search typed.
    pub fn set_search(&mut self, q: &str) {
        self.search = q.to_string();
    }

    /// Previews are drawn as they're ready; `--render` draws them all.
    pub fn render_all_previews(&mut self) {
        self.unthrottled = true;
    }

    /// The dialog's size: 760x640 at most the area it's centred in.
    pub fn size(&self, area: Rect) -> Vec2 {
        vec2(WIDTH.min(area.width()), HEIGHT.min(area.height()))
    }

    fn texture(&mut self, ctx: &egui::Context, seq: &str, budget: &mut usize) -> Option<TextureHandle> {
        if let Some(t) = self.textures.get(seq) {
            return t.clone();
        }
        if *budget == 0 {
            return None;
        }
        *budget -= 1;
        let t = emoji::preview_bitmap(seq, PREVIEW).map(|pm| {
            let mut rgba = Vec::with_capacity(pm.pixels().len() * 4);
            for c in pm.pixels() {
                rgba.extend_from_slice(&[(c >> 16) as u8, (c >> 8) as u8, *c as u8, (c >> 24) as u8]);
            }
            let img = egui::ColorImage::from_rgba_premultiplied([pm.width() as usize, pm.height() as usize], &rgba);
            ctx.load_texture(format!("emoji-{seq}"), img, egui::TextureOptions::LINEAR)
        });
        self.textures.insert(seq.to_string(), t.clone());
        t
    }

    pub fn show(&mut self, ui: &mut Ui, p: &Palette, rect: Rect) -> Option<Outcome> {
        let mut out = None;
        let painter = ui.painter().clone();
        let shadow = egui::epaint::Shadow { offset: [0, 6], blur: 18, spread: 0, color: Color32::from_black_alpha(0x40) };
        painter.add(shadow.as_shape(rect, CornerRadius::same(4)));
        border::border(&painter, rect, 4.0, 1.0, Some(border::Brush::Solid(p.background_panel)), Some(p.border));
        let inner = rect.shrink(1.0);
        let group = self.groups.get(self.group).cloned().unwrap_or_default();
        let (entries, count) = emoji_catalog::query(&group, &self.search);
        // The search box and the group combo box (`ColumnDefinitions="*,220"`).
        let row = Rect::from_min_max(pos2(inner.min.x + 12.0, inner.min.y + 12.0), pos2(inner.max.x - 12.0, inner.min.y + 12.0 + 40.0));
        let search_r = Rect::from_min_max(row.min, pos2(row.max.x - 220.0 - 12.0, row.max.y));
        let placeholder = ie::EmojiPickerDialogView_SearchEmojisCount().replace("{0}", &count.to_string());
        let tb = TextBox { placeholder: &placeholder, padding: Thickness::new(8.0, 0.0, 8.0, 0.0), ..Default::default() };
        let before = self.search.clone();
        let resp = tb.show(ui, search_r, Id::new("emoji-search"), &mut self.search);
        if std::mem::take(&mut self.focus) {
            resp.request_focus();
        }
        if self.search != before {
            self.scroll = 0.0;
        }
        let combo_r = Rect::from_min_max(pos2(row.max.x - 220.0, row.min.y), row.max);
        let items: Vec<ComboItem> = self.groups.iter().map(ComboItem::text).collect();
        if let Some(i) = (ComboBox { padding: Thickness::hv(8.0, 6.0), enabled: true }).show(ui, combo_r, Id::new("emoji-group"), &items, Some(self.group)) {
            self.group = i;
            self.scroll = 0.0;
        }
        // The tiles (`WrapPanel`), scrolled.
        let area = Rect::from_min_max(pos2(inner.min.x + 12.0, row.max.y + 8.0), pos2(inner.max.x - 12.0, inner.max.y - 12.0));
        if entries.is_empty() {
            // `!HasResults`: the message and Cancel, centred.
            let f13 = text_font(13.0, 600);
            let f11 = text_font(11.0, 600);
            let (lh13, lh11) = (widgets::line_height(ui, &f13), widgets::line_height(ui, &f11));
            let cancel = ie::EmojiPickerDialogView_Cancel();
            let b = Button { padding: Thickness::new(20.0, 0.0, 20.0, 0.0), ..Default::default() };
            let total = lh13 + 8.0 + lh11 + 8.0 + 40.0;
            let mut y = area.min.y + ((area.height() - total) / 2.0).round_ties_even();
            for (t, f, lh) in [(ie::EmojiPickerDialogView_NoMatches(), f13, lh13), (ie::EmojiPickerDialogView_TryDifferentKeyword(), f11, lh11)] {
                let w = widgets::text_width(ui, t, f.clone());
                let g = widgets::galley(ui, t, f, p.text_secondary, f32::INFINITY);
                painter.galley(pos2(area.center().x - (w / 2.0).round_ties_even(), y), g, p.text_secondary);
                y += lh + 8.0;
            }
            let bw = b.width(ui, cancel).ceil();
            if b.show(ui, Rect::from_min_size(pos2(area.center().x - (bw / 2.0).round_ties_even(), y), vec2(bw, 40.0)), Id::new("emoji-cancel"), cancel, true).clicked() {
                out = Some(Outcome::Cancel);
            }
            return out;
        }
        let per_row = ((area.width() / CELL).floor() as usize).max(1);
        let rows = entries.len().div_ceil(per_row);
        // `Margin="0,0,0,8"` under the tiles.
        let total = rows as f32 * CELL + 8.0;
        if ui.rect_contains_pointer(area) {
            let d = ui.ctx().input(|i| i.smooth_scroll_delta.y);
            self.scroll -= d;
        }
        self.scroll = self.scroll.clamp(0.0, (total - area.height()).max(0.0));
        let clip = painter.with_clip_rect(area.intersect(ui.clip_rect()));
        let first = (self.scroll / CELL).floor() as usize;
        let visible = (area.height() / CELL).ceil() as usize + 1;
        let mut budget = if self.unthrottled { usize::MAX } else { PREVIEWS_PER_FRAME };
        let mut pending = false;
        for r in first..(first + visible).min(rows) {
            for c in 0..per_row {
                let Some(e) = entries.get(r * per_row + c) else { break };
                let cell = Rect::from_min_size(pos2(area.min.x + c as f32 * CELL + 2.0, area.min.y + r as f32 * CELL + 2.0 - self.scroll), vec2(TILE, TILE));
                let resp = ui.interact(cell.intersect(area), Id::new(("emoji", &e.unicode)), Sense::click());
                // `Button.effect-item`: control background, no border, 6px
                // corners (`emoji-tile`), the hover background.
                let fill = if resp.hovered() || resp.is_pointer_button_down_on() { p.control_background_hover } else { p.control_background };
                clip.rect_filled(cell, CornerRadius::same(6), fill);
                match self.texture(ui.ctx(), &e.unicode, &mut budget) {
                    Some(t) => {
                        let img = Rect::from_center_size(cell.center(), vec2(PREVIEW as f32, PREVIEW as f32));
                        clip.image(t.id(), img, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                    }
                    None => pending |= !self.textures.contains_key(&e.unicode),
                }
                let resp = resp.on_hover_text(e.display_name());
                if resp.clicked() {
                    out = Some(Outcome::Pick(e));
                }
            }
        }
        if pending {
            ui.ctx().request_repaint();
        }
        // Fluent's collapsed vertical scroll bar: a 2px thumb 2px in from
        // the right, between the 16px line buttons.
        if total > area.height() {
            let track = Rect::from_min_max(pos2(area.max.x - 4.0, area.min.y + 16.0), pos2(area.max.x - 2.0, area.max.y - 16.0));
            let th = (track.height() * area.height() / total).max(16.0);
            let ty = track.min.y + (track.height() - th) * self.scroll / (total - area.height());
            painter.rect_filled(Rect::from_min_size(pos2(track.min.x, ty), vec2(2.0, th)), CornerRadius::same(1), sharex_ui::theme::fluent(ui.ctx()).base_medium);
        }
        out
    }
}
