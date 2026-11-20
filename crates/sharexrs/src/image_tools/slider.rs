//! `ShareX.Tools/Controls/ImageComparisonSlider.cs`: two images over a
//! checkerboard, the left one clipped at a draggable divider with its
//! handle; with one image (the resizer, converter and watermark previews)
//! just that image. The images are fitted into the control
//! (`GetImageBounds`, `StretchDirection`) with their edges rounded to whole
//! pixels.

use egui::{Color32, CornerRadius, Id, Pos2, Rect, Stroke, Ui, pos2, vec2};
use sharex_ui::border::{self, Brush};

use super::Picture;

/// `StretchDirection`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stretch {
    Both,
    DownOnly,
}

/// `CreateCheckerBrush`: 12px cells, `#EBEBEB` and `#CDCDCD`.
const CELL: f32 = 12.0;
const LIGHT: Color32 = Color32::from_rgb(0xEB, 0xEB, 0xEB);
const DARK: Color32 = Color32::from_rgb(0xCD, 0xCD, 0xCD);
/// `HandleBrush`: `Color.FromArgb(230, 35, 35, 35)`.
const HANDLE: Color32 = Color32::from_rgba_premultiplied(31, 31, 31, 230);

/// `GetImageBounds`: the images' common size scaled into `bounds` and
/// centred, each edge `Math.Round`ed.
pub fn image_bounds(bounds: Rect, size: (u32, u32), stretch: Stretch) -> Rect {
    let (iw, ih) = (size.0 as f64, size.1 as f64);
    let (bw, bh) = (bounds.width() as f64, bounds.height() as f64);
    if iw <= 0.0 || ih <= 0.0 || bw <= 0.0 || bh <= 0.0 {
        return bounds;
    }
    let mut scale = (bw / iw).min(bh / ih);
    if stretch == Stretch::DownOnly {
        scale = scale.min(1.0);
    }
    let (w, h) = (iw * scale, ih * scale);
    let (bl, bt) = (bounds.min.x as f64, bounds.min.y as f64);
    let left = (bl + (bw - w) / 2.0).round_ties_even();
    let top = (bt + (bh - h) / 2.0).round_ties_even();
    let right = (bl + (bw + w) / 2.0).round_ties_even();
    let bottom = (bt + (bh + h) / 2.0).round_ties_even();
    Rect::from_min_size(pos2(left as f32, top as f32), vec2((right - left).max(1.0) as f32, (bottom - top).max(1.0) as f32))
}

/// `DrawTransparencyBackground`: the tiled checkerboard (its tiles start at
/// the control's origin) filling `rect`.
fn checkerboard(painter: &egui::Painter, origin: Pos2, rect: Rect) {
    let clip = painter.with_clip_rect(rect.intersect(painter.clip_rect()));
    clip.rect_filled(rect, 0.0, LIGHT);
    let x0 = ((rect.min.x - origin.x) / CELL).floor() as i64;
    let y0 = ((rect.min.y - origin.y) / CELL).floor() as i64;
    let x1 = ((rect.max.x - origin.x) / CELL).ceil() as i64;
    let y1 = ((rect.max.y - origin.y) / CELL).ceil() as i64;
    for cy in y0..y1 {
        for cx in x0..x1 {
            if (cx + cy).rem_euclid(2) == 1 {
                let r = Rect::from_min_size(pos2(origin.x + cx as f32 * CELL, origin.y + cy as f32 * CELL), vec2(CELL, CELL));
                clip.rect_filled(r, 0.0, DARK);
            }
        }
    }
}

fn draw_picture(ui: &Ui, painter: &egui::Painter, pic: &mut Picture, dest: Rect) {
    let size = (dest.width().round().max(1.0) as u32, dest.height().round().max(1.0) as u32);
    pic.texture(ui.ctx(), size).paint(painter, dest, Color32::WHITE);
}

/// The control's divider and drag state.
pub struct ComparisonSlider {
    /// `SliderPosition`, 0 to 1.
    pub position: f64,
    dragging: bool,
}

impl Default for ComparisonSlider {
    fn default() -> Self {
        Self { position: 0.5, dragging: false }
    }
}

impl ComparisonSlider {
    /// One image (the other is null): the checkerboard and the image.
    pub fn draw_single(ui: &Ui, bounds: Rect, pic: &mut Picture, stretch: Stretch) {
        let painter = ui.painter().with_clip_rect(bounds.intersect(ui.clip_rect()));
        let ib = image_bounds(bounds, pic.size(), stretch);
        checkerboard(&painter, bounds.min, ib);
        draw_picture(ui, &painter, pic, ib);
    }

    /// Both images, the divider and its handle; dragging moves the divider
    /// (`live`).
    pub fn show(&mut self, ui: &Ui, bounds: Rect, id: Id, left: &mut Picture, right: &mut Picture, live: bool) {
        let (lw, lh) = left.size();
        let (rw, rh) = right.size();
        let size = (lw.max(rw), lh.max(rh));
        let ib = image_bounds(bounds, size, Stretch::Both);
        if live {
            let r = ui.interact(bounds, id, egui::Sense::drag());
            if r.hovered() || r.dragged() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
            }
            if r.drag_started() || r.is_pointer_button_down_on() {
                self.dragging = true;
            }
            if self.dragging
                && let Some(pos) = ui.input(|i| i.pointer.interact_pos())
            {
                // `UpdateSliderPosition`: only when the divider moves a pixel.
                let np = if ib.width() <= 0.0 { 0.5 } else { (((pos.x - ib.min.x) / ib.width()) as f64).clamp(0.0, 1.0) };
                let cur = (ib.min.x as f64 + ib.width() as f64 * self.position).round_ties_even();
                let new = (ib.min.x as f64 + ib.width() as f64 * np).round_ties_even();
                if cur != new {
                    self.position = np;
                }
            }
            if !ui.input(|i| i.pointer.primary_down()) {
                self.dragging = false;
            }
        }
        let painter = ui.painter().with_clip_rect(bounds.intersect(ui.clip_rect()));
        checkerboard(&painter, bounds.min, ib);
        draw_picture(ui, &painter, right, ib);
        let x = (ib.min.x as f64 + ib.width() as f64 * self.position).round_ties_even() as f32;
        let left_clip = Rect::from_min_max(ib.min, pos2(x.max(ib.min.x), ib.max.y));
        let lp = painter.with_clip_rect(left_clip.intersect(painter.clip_rect()));
        checkerboard(&lp, bounds.min, ib);
        draw_picture(ui, &lp, left, ib);
        // `DrawSlider`: the 2px white line, the 34x52 handle (16 radius,
        // a 2px white pen) and its two grip lines.
        let white = Stroke::new(2.0, Color32::WHITE);
        painter.line_segment([pos2(x, ib.min.y), pos2(x, ib.max.y)], white);
        let handle = Rect::from_min_size(pos2(x - 17.0, ib.min.y + ib.height() / 2.0 - 26.0), vec2(34.0, 52.0));
        border::border(&painter, handle.expand(1.0), 17.0, 2.0, Some(Brush::Solid(HANDLE)), Some(Color32::WHITE));
        let _ = CornerRadius::ZERO;
        for dx in [-5.0, 5.0] {
            painter.line_segment([pos2(x + dx, handle.min.y + 17.0), pos2(x + dx, handle.max.y - 17.0)], white);
        }
    }
}
