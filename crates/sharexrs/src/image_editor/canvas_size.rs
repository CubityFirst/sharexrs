//! The Canvas size tool (`EditorView.CanvasResize`): a frame over the
//! image with eight handles, dragged past the image (room is added round
//! it in the scroll viewer) to grow the canvas or inside it to crop; the
//! area to add previewed in the fill colour (`CanvasFillColor`, the
//! checkerboard when transparent), the area to drop shaded, a panel under
//! the frame with the size, the fill colour, Apply and Cancel.

use egui::{Color32, CornerRadius, Id, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};
use sharex_capture::editor_popups::ColorDropdown;
use sharex_strings::image_editor as ie;
use sharex_ui::controls::Button;
use sharex_ui::theme::Thickness;
use sharex_ui::{Palette, border, widgets};

use super::EditorWindow;

const HANDLE: f32 = 12.0;
const HIT: f32 = 10.0;
const MIN_SIZE: f64 = 1.0;
const MAX_SIZE: f64 = 30000.0;
/// `CanvasResizeRoomFactor`.
const ROOM_FACTOR: f32 = 0.3;
/// `CanvasResizeFitFactor`.
const FIT_FACTOR: f64 = 0.7;

pub const LEFT: u8 = 1;
pub const TOP: u8 = 2;
pub const RIGHT: u8 = 4;
pub const BOTTOM: u8 = 8;

/// `CanvasResizeHandleKinds`.
const HANDLES: [u8; 8] = [LEFT | TOP, TOP, RIGHT | TOP, RIGHT, RIGHT | BOTTOM, BOTTOM, LEFT | BOTTOM, LEFT];

pub struct CanvasResize {
    /// The new canvas in image pixels: left, top, right, bottom.
    pub rect: [f64; 4],
    drag: Option<(u8, (f64, f64), [f64; 4])>,
    pub saved_zoom: Option<f64>,
    dropdown: ColorDropdown,
    button: Rect,
    /// The panel as last drawn (presses on it aren't the frame's).
    pub panel: Rect,
    /// The room is laid out after the zoom settles (the next frame).
    pub room_pending: bool,
}

impl CanvasResize {
    pub fn new(w: u32, h: u32) -> Self {
        Self {
            rect: [0.0, 0.0, w as f64, h as f64],
            drag: None,
            saved_zoom: None,
            dropdown: ColorDropdown::default(),
            button: Rect::NOTHING,
            panel: Rect::NOTHING,
            room_pending: true,
        }
    }

    pub fn is_dragging(&self) -> bool {
        self.drag.is_some()
    }

    /// Presses on the panel or its colour popup go to them.
    pub fn passes_through(&self, p: Pos2) -> bool {
        self.panel.contains(p) || self.dropdown.contains(p)
    }
}

/// `ResizeCanvasFrame`.
fn resize_frame(start: [f64; 4], edges: u8, dx: f64, dy: f64) -> [f64; 4] {
    let [mut l, mut t, mut r, mut b] = start;
    if edges & LEFT != 0 {
        l = (start[0] + dx).round_ties_even().clamp(r - MAX_SIZE, r - MIN_SIZE);
    } else if edges & RIGHT != 0 {
        r = (start[2] + dx).round_ties_even().clamp(l + MIN_SIZE, l + MAX_SIZE);
    }
    if edges & TOP != 0 {
        t = (start[1] + dy).round_ties_even().clamp(b - MAX_SIZE, b - MIN_SIZE);
    } else if edges & BOTTOM != 0 {
        b = (start[3] + dy).round_ties_even().clamp(t + MIN_SIZE, t + MAX_SIZE);
    }
    [l, t, r, b]
}

/// `SubtractRect`: the parts of `a` outside `b`.
fn subtract(a: Rect, b: Rect) -> Vec<Rect> {
    if a.width() <= 0.0 || a.height() <= 0.0 {
        return Vec::new();
    }
    let (l, t, r, bt) = (a.min.x.max(b.min.x), a.min.y.max(b.min.y), a.max.x.min(b.max.x), a.max.y.min(b.max.y));
    if r <= l || bt <= t {
        return vec![a];
    }
    let mut out = Vec::new();
    if t > a.min.y {
        out.push(Rect::from_min_max(a.min, pos2(a.max.x, t)));
    }
    if bt < a.max.y {
        out.push(Rect::from_min_max(pos2(a.min.x, bt), a.max));
    }
    if l > a.min.x {
        out.push(Rect::from_min_max(pos2(a.min.x, t), pos2(l, bt)));
    }
    if r < a.max.x {
        out.push(Rect::from_min_max(pos2(r, t), pos2(a.max.x, bt)));
    }
    out
}

/// `GetCanvasResizeCursor`.
pub fn cursor(edges: u8) -> egui::CursorIcon {
    let h = edges & (LEFT | RIGHT) != 0;
    let v = edges & (TOP | BOTTOM) != 0;
    if h && v {
        if edges == LEFT | TOP || edges == RIGHT | BOTTOM { egui::CursorIcon::ResizeNwSe } else { egui::CursorIcon::ResizeNeSw }
    } else if h {
        egui::CursorIcon::ResizeHorizontal
    } else {
        egui::CursorIcon::ResizeVertical
    }
}

/// `AccentPalette.GetHaloColor`.
fn halo(c: Color32) -> Color32 {
    let (b, w) = (sharex_ui::accent::contrast_ratio(c, Color32::BLACK), sharex_ui::accent::contrast_ratio(c, Color32::WHITE));
    if b >= w { Color32::BLACK } else { Color32::WHITE }
}

impl EditorWindow {
    /// `CanvasSizeModeRequested`: starts the tool, or applies it when open.
    pub(super) fn toggle_canvas_resize(&mut self) {
        if self.canvas_resize.is_some() {
            self.confirm_canvas_resize();
        } else {
            self.begin_canvas_resize();
        }
    }

    /// `BeginCanvasResize` / `StartCanvasResize`: whatever is in progress
    /// ends; a large image is zoomed out to fit 70% of the view.
    fn begin_canvas_resize(&mut self) {
        if !self.has_image {
            return;
        }
        self.ws.commit_text();
        self.ws.cancel_clone();
        self.ws.cancel_crop();
        self.ws.clear_selection();
        let (w, h) = self.ws.size();
        let mut cr = CanvasResize::new(w, h);
        let vp = self.layout.canvas;
        if vp.width() > 0.0 && vp.height() > 0.0 {
            let fit = (vp.width() as f64 * FIT_FACTOR / w as f64).min(vp.height() as f64 * FIT_FACTOR / h as f64);
            if fit < self.zoom * 0.99 {
                let z = ((self.zoom * fit / self.zoom * 100.0).floor() / 100.0).max(0.25);
                if z < self.zoom {
                    cr.saved_zoom = Some(self.zoom);
                    self.zoom = z;
                }
            }
        }
        self.canvas_resize = Some(cr);
    }

    /// The image's top-left corner in the view for this `room` and scroll.
    pub(super) fn image_origin(&self, room: Vec2, scroll: Vec2) -> Pos2 {
        let canvas = self.layout.canvas;
        let (w, h) = self.ws.size();
        let size = vec2(w as f32, h as f32) * self.zoom as f32;
        let content = size + room * 2.0;
        pos2(
            if content.x <= canvas.width() { canvas.center().x - size.x / 2.0 } else { canvas.min.x - scroll.x + room.x },
            if content.y <= canvas.height() { canvas.center().y - size.y / 2.0 } else { canvas.min.y - scroll.y + room.y },
        )
    }

    /// `SetCanvasResizeRoom`: the room, the scroll moved so the image's
    /// corner stays at `keep` (view coordinates).
    pub(super) fn set_room(&mut self, room: Vec2, keep: Option<Pos2>) {
        self.room = room;
        if let Some(k) = keep {
            let now = self.image_origin(room, self.scroll);
            let d = now - k;
            self.scroll = vec2((self.scroll.x + d.x).max(0.0), (self.scroll.y + d.y).max(0.0));
        }
    }

    /// `EnsureCanvasResizeRoom`: the room only grows while the tool is open.
    fn ensure_room(&mut self) {
        let Some(cr) = &self.canvas_resize else { return };
        let (w, h) = self.ws.size();
        let z = self.zoom as f32;
        let vp = self.layout.canvas;
        let [l, t, r, b] = cr.rect;
        let rx = vp.width() * ROOM_FACTOR + (((-l).max(0.0)).max((r - w as f64).max(0.0)) as f32) * z;
        let ry = vp.height() * ROOM_FACTOR + (((-t).max(0.0)).max((b - h as f64).max(0.0)) as f32) * z;
        let grown = vec2(self.room.x.max(rx), self.room.y.max(ry));
        if grown != self.room {
            let origin = self.image_origin(self.room, self.scroll);
            self.set_room(grown, Some(origin));
        }
    }

    /// `TryConfirmCanvasResize`: the canvas grown or cropped to the frame,
    /// its new corner kept where it is on screen.
    pub(super) fn confirm_canvas_resize(&mut self) -> bool {
        let Some(cr) = self.canvas_resize.take() else { return false };
        let (w, h) = self.ws.size();
        let [l, t, r, b] = cr.rect.map(|v| v.round_ties_even() as i32);
        let (add_l, add_t, add_r, add_b) = (-l, -t, r - w as i32, b - h as i32);
        if add_l == 0 && add_t == 0 && add_r == 0 && add_b == 0 {
            self.end_canvas_resize(cr.saved_zoom, true);
            return true;
        }
        let o = self.image_origin(self.room, self.scroll);
        let z = self.zoom as f32;
        let target = pos2(o.x + l as f32 * z, o.y + t as f32 * z);
        let fill = sharex_ui::accent::parse_hex(&self.ws.tools.options.CanvasFillColorHex).unwrap_or(Color32::TRANSPARENT);
        let resized = self.ws.resize_canvas(add_t, add_r, add_b, add_l, fill.to_srgba_unmultiplied());
        let keep = (resized && cr.saved_zoom.is_none()).then_some(target);
        self.set_room(Vec2::ZERO, keep);
        if let Some(z) = cr.saved_zoom {
            self.zoom = z;
        }
        self.update_title();
        true
    }

    /// `CancelCanvasResize`.
    pub(super) fn cancel_canvas_resize(&mut self) -> bool {
        let Some(cr) = self.canvas_resize.take() else { return false };
        self.end_canvas_resize(cr.saved_zoom, true);
        true
    }

    fn end_canvas_resize(&mut self, saved_zoom: Option<f64>, keep_image: bool) {
        let origin = (keep_image && saved_zoom.is_none()).then(|| self.image_origin(self.room, self.scroll));
        self.set_room(Vec2::ZERO, origin);
        if let Some(z) = saved_zoom {
            self.zoom = z;
        }
    }

    /// The frame and the image in the view.
    fn canvas_resize_rects(&self) -> Option<(Rect, Rect)> {
        let cr = self.canvas_resize.as_ref()?;
        let (w, h) = self.ws.size();
        let o = self.image_origin(self.room, self.scroll);
        let z = self.zoom as f32;
        let image = Rect::from_min_size(o, vec2(w as f32, h as f32) * z);
        let [l, t, r, b] = cr.rect.map(|v| v as f32);
        let frame = Rect::from_min_max(pos2(o.x + l * z, o.y + t * z), pos2(o.x + r * z, o.y + b * z));
        Some((frame, image))
    }

    /// `HitTestCanvasResizeFrame`.
    pub(super) fn canvas_resize_hit(&self, p: Pos2) -> u8 {
        let Some((f, _)) = self.canvas_resize_rects() else { return 0 };
        let within_x = p.x >= f.min.x - HIT && p.x <= f.max.x + HIT;
        let within_y = p.y >= f.min.y - HIT && p.y <= f.max.y + HIT;
        let mut edges = 0;
        if within_y {
            let (tl, tr) = ((p.x - f.min.x).abs(), (p.x - f.max.x).abs());
            if tl <= HIT || tr <= HIT {
                edges |= if tr <= tl { RIGHT } else { LEFT };
            }
        }
        if within_x {
            let (tt, tb) = ((p.y - f.min.y).abs(), (p.y - f.max.y).abs());
            if tt <= HIT || tb <= HIT {
                edges |= if tb <= tt { BOTTOM } else { TOP };
            }
        }
        edges
    }

    /// `OnCanvasResizePointerPressed` (left): a handle or edge starts a
    /// drag, a double click elsewhere applies.
    pub(super) fn canvas_resize_pressed(&mut self, pos: Pos2, double: bool) {
        let edges = self.canvas_resize_hit(pos);
        if edges == 0 {
            if double {
                self.confirm_canvas_resize();
            }
            return;
        }
        let at = self.mapper_ppp(1.0).image_pos(pos);
        if let Some(cr) = self.canvas_resize.as_mut() {
            cr.drag = Some((edges, at, cr.rect));
        }
    }

    /// `OnCanvasResizePointerMoved`, with `AutoScrollForCanvasResize`.
    pub(super) fn canvas_resize_moved(&mut self, pos: Pos2) {
        let Some((edges, start, rect)) = self.canvas_resize.as_ref().and_then(|c| c.drag) else { return };
        let vp = self.layout.canvas;
        let (mut dx, mut dy) = (0.0, 0.0);
        let (px, py) = (pos.x - vp.min.x, pos.y - vp.min.y);
        if px < 16.0 {
            dx = (px - 16.0) * 0.5;
        } else if px > vp.width() - 16.0 {
            dx = (px - (vp.width() - 16.0)) * 0.5;
        }
        if py < 16.0 {
            dy = (py - 16.0) * 0.5;
        } else if py > vp.height() - 16.0 {
            dy = (py - (vp.height() - 16.0)) * 0.5;
        }
        if dx != 0.0 || dy != 0.0 {
            self.scroll = vec2((self.scroll.x + dx).max(0.0), (self.scroll.y + dy).max(0.0));
        }
        let cur = self.mapper_ppp(1.0).image_pos(pos);
        if let Some(cr) = self.canvas_resize.as_mut() {
            cr.rect = resize_frame(rect, edges, cur.0 - start.0, cur.1 - start.1);
        }
        self.ensure_room();
    }

    pub(super) fn canvas_resize_released(&mut self) {
        if let Some(cr) = self.canvas_resize.as_mut() {
            cr.drag = None;
        }
    }

    /// `UpdateCanvasResizeVisuals` and the panel.
    pub(super) fn paint_canvas_resize(&mut self, ui: &Ui, p: &Palette) {
        if self.canvas_resize.as_ref().is_some_and(|c| c.room_pending) {
            if let Some(c) = self.canvas_resize.as_mut() {
                c.room_pending = false;
            }
            self.ensure_room();
        }
        let Some((frame, image)) = self.canvas_resize_rects() else { return };
        let painter = ui.painter().clone();
        let fill = sharex_ui::accent::parse_hex(&self.ws.tools.options.CanvasFillColorHex).unwrap_or(Color32::TRANSPARENT);
        for r in subtract(frame, image) {
            if fill.a() == 0 {
                super::paint_checkered(&painter, r, r.min, 1.0, p);
            } else {
                painter.rect_filled(r, 0.0, fill);
            }
        }
        for r in subtract(image, frame) {
            painter.rect_filled(r, 0.0, Color32::from_black_alpha(140));
        }
        let accent = p.accent_start;
        let halo = halo(accent);
        // The strokes inside their rectangles: the halo 4px round the
        // frame grown by 1, the frame 2px.
        painter.rect_stroke(frame.expand(1.0), 0.0, Stroke::new(4.0, halo), StrokeKind::Inside);
        painter.rect_stroke(frame, 0.0, Stroke::new(2.0, accent), StrokeKind::Inside);
        let handle_fill = if sharex_ui::accent::contrast_ratio(accent, Color32::WHITE) >= 3.0 { Color32::WHITE } else { halo };
        for kind in HANDLES {
            let x = if kind & LEFT != 0 { frame.min.x } else if kind & RIGHT != 0 { frame.max.x } else { frame.center().x };
            let y = if kind & TOP != 0 { frame.min.y } else if kind & BOTTOM != 0 { frame.max.y } else { frame.center().y };
            let r = Rect::from_min_size(pos2(x - HANDLE / 2.0, y - HANDLE / 2.0), vec2(HANDLE, HANDLE));
            border::border(&painter, r, 2.0, 2.0, Some(border::Brush::Solid(handle_fill)), Some(accent));
        }
        // The panel: the size, the fill colour, Apply, Cancel (8 apart) in
        // a toolbar-coloured border, `Padding="10,6"`, 14px under the frame.
        let cr = self.canvas_resize.as_ref().unwrap();
        let text = format!("{} \u{00D7} {}", (cr.rect[2] - cr.rect[0]).round_ties_even() as i64, (cr.rect[3] - cr.rect[1]).round_ties_even() as i64);
        let font = widgets::font(13.0, 600);
        let tw = widgets::text_width(ui, &text, font.clone()).max(96.0);
        let b = Button { padding: Thickness::hv(14.0, 5.0), ..Default::default() };
        let (apply, cancel) = (ie::EditorView_CanvasSizeApply(), ie::EditorView_CanvasSizeCancel());
        let (aw, cw) = (b.width(ui, apply).ceil(), b.width(ui, cancel).ceil());
        let size = vec2(2.0 + 20.0 + tw + 8.0 + 60.0 + 8.0 + aw + 8.0 + cw, 2.0 + 12.0 + 40.0);
        let host = self.layout.canvas;
        let mut x = frame.center().x - size.x / 2.0 - host.min.x;
        let mut y = frame.max.y + 14.0 - host.min.y;
        if y + size.y > host.height() - 8.0 {
            y = frame.min.y - host.min.y - 14.0 - size.y;
        }
        if y < 8.0 {
            y = host.height() - size.y - 8.0;
        }
        x = x.clamp(8.0, (host.width() - size.x - 8.0).max(8.0));
        y = y.clamp(8.0, (host.height() - size.y - 8.0).max(8.0));
        let panel = Rect::from_min_size(pos2(host.min.x + x, host.min.y + y), size);
        border::border(&painter, panel, 6.0, 1.0, Some(border::Brush::Solid(p.background_toolbar)), Some(p.border));
        let row = Rect::from_min_max(pos2(panel.min.x + 11.0, panel.min.y + 7.0), pos2(panel.max.x - 11.0, panel.max.y - 7.0));
        let g = widgets::galley(ui, &text, font.clone(), p.text_secondary, f32::INFINITY);
        painter.galley(pos2(row.min.x, widgets::center_y(row, widgets::line_height(ui, &font))), g, p.text_secondary);
        let mut bx = row.min.x + tw + 8.0;
        let button = Rect::from_min_size(pos2(bx, row.min.y), vec2(60.0, 40.0));
        let resp = ui.interact(button, Id::new("canvas-size-fill"), Sense::click());
        let open = self.canvas_resize.as_ref().is_some_and(|c| c.dropdown.open);
        sharex_capture::region::options_row::color_dropdown_button(ui, &painter, p, button, fill, resp.hovered(), open);
        bx += 60.0 + 8.0;
        let ar = Rect::from_min_size(pos2(bx, row.min.y), vec2(aw, 40.0));
        // `crop-confirm-button`: the popup background, the hover one.
        let a_resp = ui.interact(ar, Id::new("canvas-size-apply"), Sense::click());
        let a_fill = if a_resp.hovered() && !a_resp.is_pointer_button_down_on() { p.control_background_hover } else { p.background_popup };
        border::border(&painter, ar, 4.0, 1.0, Some(border::Brush::Solid(a_fill)), Some(p.border));
        let bf = sharex_ui::controls::content_font(13.0, 400);
        let atw = widgets::text_width(ui, apply, bf.clone());
        let fg = if a_resp.hovered() { sharex_ui::theme::fluent(ui.ctx()).base_high } else { p.text };
        let g = widgets::galley(ui, apply, bf.clone(), fg, f32::INFINITY);
        painter.galley(pos2(ar.min.x + 15.0 + ((ar.width() - 30.0 - atw) / 2.0).round_ties_even(), widgets::center_y(ar, widgets::line_height(ui, &bf))), g, fg);
        bx += aw + 8.0;
        let c_resp = b.show(ui, Rect::from_min_size(pos2(bx, row.min.y), vec2(cw, 40.0)), Id::new("canvas-size-cancel"), cancel, true);
        let mut c = fill;
        let mut picked = None;
        if let Some(cr) = self.canvas_resize.as_mut() {
            cr.panel = panel;
            cr.button = button;
            if resp.clicked() {
                cr.dropdown.toggle(fill);
            }
            let bounds = self.layout.view;
            let r = cr.dropdown.show(ui, p, button, bounds, "canvas-size-fill", &mut c);
            if r.changed {
                picked = Some(c);
            }
        }
        if let Some(c) = picked {
            self.ws.tools.options.CanvasFillColorHex = sharex_ui::accent::to_hex(c);
        }
        if a_resp.clicked() {
            self.confirm_canvas_resize();
        } else if c_resp.clicked() {
            self.cancel_canvas_resize();
        }
        // The resize cursor over the frame's edges.
        if let Some(pos) = ui.ctx().pointer_hover_pos()
            && !panel.contains(pos)
        {
            let edges = self.canvas_resize.as_ref().and_then(|c| c.drag.map(|d| d.0)).unwrap_or_else(|| self.canvas_resize_hit(pos));
            if edges != 0 {
                ui.ctx().set_cursor_icon(cursor(edges));
            }
        }
        let _ = CornerRadius::ZERO;
    }
}
