//! `AnimatedGifTrimmerTimeline` and `VideoTrimmerTimeline` (the same
//! control over two view models): the filmstrip of 12 thumbnails, the
//! shaded parts outside the selection, the selection frame with its two
//! handles, and the playhead. A press near a handle drags it, anywhere
//! else seeks; Left/Right, Home/End and I/O when it has focus.

use egui::{Color32, Id, Pos2, Rect, Sense, Ui, pos2, vec2};
use sharex_ui::border::{self, Brush};
use sharex_ui::theme::palette;

use crate::image_tools::Picture;

const INSET: f32 = 14.0;
const TRACK_TOP: f32 = 12.0;
const TRACK_HEIGHT: f32 = 48.0;
const TRACK_BOTTOM: f32 = TRACK_TOP + TRACK_HEIGHT;
/// The control's `Height="72"`.
pub const HEIGHT: f32 = 72.0;

/// A filmstrip cell: where its thumbnail starts on the timeline.
pub enum CellPlacement {
    /// The GIF trimmer's: cell `i` at `i` twelfths of the track.
    Index,
    /// The video trimmer's: at its thumbnail's position (`X(Position)`).
    Position,
}

/// What a press or drag on the timeline asked for, in seconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TimelineInput {
    Start(f64),
    End(f64),
    Seek(f64),
}

/// The keys the timeline handles while it has focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimelineKey {
    Left { shift: bool },
    Right { shift: bool },
    Home,
    End,
    I,
    O,
}

/// The selection the timeline shows.
pub struct TimelineState<'a> {
    pub duration: f64,
    pub start: f64,
    pub end: f64,
    pub position: f64,
    /// (position in seconds, image).
    pub thumbnails: &'a mut [(f64, Picture)],
    pub placement: CellPlacement,
    /// `HasGif` / `HasVideo`.
    pub has_media: bool,
    /// `IsEnabled` (`CanEdit`).
    pub enabled: bool,
}

/// The cropped and resized thumbnail as Avalonia draws it with
/// `DrawImage(bitmap, source, dest)`: the centre of the bitmap that fills the
/// cell's aspect, sampled bilinearly (no mipmaps) at the cell's pixel size.
fn cell_image(pic: &Picture, cell: (u32, u32), cell_w: f64) -> image::RgbaImage {
    let (bw, bh) = (pic.image.width() as f64, pic.image.height() as f64);
    let crop_w = bw.min(bh * cell_w / TRACK_HEIGHT as f64);
    let crop_h = bh.min(bw * TRACK_HEIGHT as f64 / cell_w);
    let (sx, sy) = ((bw - crop_w) / 2.0, (bh - crop_h) / 2.0);
    let (w, h) = (cell.0.max(1), cell.1.max(1));
    let src = &pic.image;
    image::RgbaImage::from_fn(w, h, |x, y| {
        let u = sx + (x as f64 + 0.5) * crop_w / w as f64 - 0.5;
        let v = sy + (y as f64 + 0.5) * crop_h / h as f64 - 0.5;
        bilinear(src, u, v)
    })
}

fn bilinear(img: &image::RgbaImage, u: f64, v: f64) -> image::Rgba<u8> {
    let (w, h) = (img.width() as i64, img.height() as i64);
    let (x0, y0) = (u.floor(), v.floor());
    let (fx, fy) = (u - x0, v - y0);
    let at = |x: i64, y: i64| img.get_pixel(x.clamp(0, w - 1) as u32, y.clamp(0, h - 1) as u32).0;
    let (x0, y0) = (x0 as i64, y0 as i64);
    let (a, b, c, d) = (at(x0, y0), at(x0 + 1, y0), at(x0, y0 + 1), at(x0 + 1, y0 + 1));
    image::Rgba(std::array::from_fn(|i| {
        let top = a[i] as f64 * (1.0 - fx) + b[i] as f64 * fx;
        let bottom = c[i] as f64 * (1.0 - fx) + d[i] as f64 * fx;
        (top * (1.0 - fy) + bottom * fy).round().clamp(0.0, 255.0) as u8
    }))
}

/// The cells' textures, made again when the track's size changes.
#[derive(Default)]
pub struct CellCache {
    key: Option<(u64, u32, u32, usize)>,
    textures: Vec<egui::TextureHandle>,
}

impl CellCache {
    pub fn clear(&mut self) {
        self.key = None;
        self.textures.clear();
    }
}

/// Draws the timeline in `rect` (the control's bounds) and handles the
/// pointer. `generation` changes whenever the thumbnails do.
pub fn show(ui: &Ui, rect: Rect, id: Id, state: TimelineState, cache: &mut CellCache, generation: u64) -> Option<TimelineInput> {
    let p = palette(ui.ctx());
    let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    let track_w = (rect.width() - INSET * 2.0).max(1.0);
    let x_of = |v: f64| rect.min.x + INSET + (v / state.duration.max(0.001)) as f32 * track_w;
    let track = Rect::from_min_size(pos2(rect.min.x + INSET, rect.min.y + TRACK_TOP), vec2(track_w, TRACK_HEIGHT));
    border::border(&painter, track, 4.0, 0.0, Some(Brush::Solid(p.control_background)), None);

    // Input first, so a press shows where it went this frame.
    let sense = if state.enabled && state.has_media { Sense::click_and_drag() } else { Sense::hover() };
    let r = ui.interact(rect, id, sense);
    let drag_id = id.with("drag");
    let mut input = None;
    if state.enabled && state.has_media {
        let value_at = |x: f32| (((x - rect.min.x - INSET) / track_w).clamp(0.0, 1.0) as f64) * state.duration;
        if r.is_pointer_button_down_on() && ui.input(|i| i.pointer.primary_pressed()) {
            r.request_focus();
            if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                let local = pos - rect.min;
                let (ds, de) = ((pos.x - x_of(state.start)).abs(), (pos.x - x_of(state.end)).abs());
                let near = local.y >= TRACK_TOP - 3.0 && local.y <= TRACK_BOTTOM + 3.0 && ds.min(de) <= 12.0;
                let drag: u8 = if near { if ds < de { 1 } else { 2 } } else { 3 };
                ui.ctx().data_mut(|d| d.insert_temp(drag_id, drag));
            }
        }
        let drag: u8 = ui.ctx().data(|d| d.get_temp(drag_id)).unwrap_or(0);
        if drag != 0 {
            if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                let v = value_at(pos.x);
                input = Some(match drag {
                    1 => TimelineInput::Start(v),
                    2 => TimelineInput::End(v),
                    _ => TimelineInput::Seek(v),
                });
            }
            if !ui.input(|i| i.pointer.primary_down()) {
                ui.ctx().data_mut(|d| d.remove::<u8>(drag_id));
            }
        }
    }

    if !state.has_media {
        return input;
    }
    // The filmstrip, clipped to the track's rounded rectangle.
    let cell_w = track_w as f64 / 12.0;
    let px = ui.ctx().pixels_per_point();
    let cell_px = ((cell_w as f32 * px).round() as u32, (TRACK_HEIGHT * px).round() as u32);
    let key = (generation, cell_px.0, cell_px.1, state.thumbnails.len());
    if cache.key != Some(key) {
        cache.textures = state
            .thumbnails
            .iter()
            .enumerate()
            .map(|(i, (_, pic))| {
                let img = cell_image(pic, cell_px, cell_w);
                ui.ctx().load_texture(format!("timeline-cell-{generation}-{i}"), crate::thumbnail::to_color_image(&img), egui::TextureOptions::NEAREST)
            })
            .collect();
        cache.key = Some(key);
    }
    let strip = painter.with_clip_rect(track.intersect(painter.clip_rect()));
    for (i, ((pos, _), tex)) in state.thumbnails.iter().zip(&cache.textures).enumerate() {
        let x = match state.placement {
            CellPlacement::Index => track.min.x + i as f32 * cell_w as f32,
            CellPlacement::Position => x_of(*pos),
        };
        let dest = Rect::from_min_size(pos2(x, track.min.y), vec2(cell_w as f32, TRACK_HEIGHT));
        strip.image(tex.id(), dest, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    }
    let shade = Color32::from_rgba_unmultiplied(0, 0, 0, 180);
    let (left, right) = (x_of(state.start), x_of(state.end));
    strip.rect_filled(Rect::from_min_max(track.min, pos2(left, track.max.y)), 0.0, shade);
    strip.rect_filled(Rect::from_min_max(pos2(right, track.min.y), pos2(x_of(state.duration), track.max.y)), 0.0, shade);
    border::clip_corners(&painter, track, 4.0, p.background_panel);

    // The frame and handles in the accent, outlined with its halo colour.
    let accent = p.accent_start;
    let halo = halo_color(accent);
    let grip = sharex_ui::accent::text_color_on(accent);
    let frame = Rect::from_min_max(pos2(left, rect.min.y + TRACK_TOP - 1.0), pos2(right, rect.min.y + TRACK_BOTTOM + 1.0));
    border::stroke(&painter, frame, 4.0, 4.0, Brush::Solid(halo));
    border::stroke(&painter, frame, 4.0, 2.0, Brush::accent(ui.ctx()));
    for x in [left, right] {
        let handle = Rect::from_min_size(pos2(x - 5.0, rect.min.y + TRACK_TOP - 3.0), vec2(10.0, TRACK_HEIGHT + 6.0));
        border::paint(
            &painter,
            &[
                border::Part::Border { rect: handle, radius: 3.0, thickness: 0.0, background: Some(Brush::accent(ui.ctx())), border: None },
                border::Part::Stroke { rect: handle, radius: 3.0, thickness: 1.0, brush: Brush::Solid(halo) },
            ],
            1.0,
        );
        line(&painter, pos2(x, rect.min.y + TRACK_TOP + 16.0), rect.min.y + TRACK_BOTTOM - 16.0, 2.0, grip);
    }
    let playhead = x_of(state.position);
    line(&painter, pos2(playhead, rect.min.y + 4.0), rect.min.y + TRACK_BOTTOM + 8.0, 4.0, Color32::BLACK);
    line(&painter, pos2(playhead, rect.min.y + 4.0), rect.min.y + TRACK_BOTTOM + 8.0, 2.0, Color32::WHITE);
    border::ellipse(&painter, Rect::from_center_size(pos2(playhead, rect.min.y + 5.0), vec2(8.0, 8.0)), Color32::WHITE);
    input
}

/// The keys pressed while the timeline `id` has focus.
pub fn keys(ui: &Ui, id: Id) -> Vec<TimelineKey> {
    if !ui.ctx().memory(|m| m.has_focus(id)) {
        return Vec::new();
    }
    ui.input(|i| {
        let shift = i.modifiers.shift;
        i.events
            .iter()
            .filter_map(|e| match e {
                egui::Event::Key { key, pressed: true, .. } => Some(match key {
                    egui::Key::ArrowLeft => TimelineKey::Left { shift },
                    egui::Key::ArrowRight => TimelineKey::Right { shift },
                    egui::Key::Home => TimelineKey::Home,
                    egui::Key::End => TimelineKey::End,
                    egui::Key::I => TimelineKey::I,
                    egui::Key::O => TimelineKey::O,
                    _ => return None,
                }),
                _ => None,
            })
            .collect()
    })
}

/// A vertical `DrawLine` with a flat-capped pen of `width`.
fn line(painter: &egui::Painter, top: Pos2, bottom: f32, width: f32, color: Color32) {
    painter.rect_filled(Rect::from_min_max(pos2(top.x - width / 2.0, top.y), pos2(top.x + width / 2.0, bottom)), 0.0, color);
}

/// `AccentPalette.GetHaloColor`: black or white, whichever contrasts more.
pub fn halo_color(color: Color32) -> Color32 {
    use sharex_ui::accent::contrast_ratio;
    if contrast_ratio(color, Color32::BLACK) >= contrast_ratio(color, Color32::WHITE) { Color32::BLACK } else { Color32::WHITE }
}
