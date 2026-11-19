//! Drawing one monitor's part of the overlay, in ShareX's layers: the
//! frozen screen (`EditorWorkspace`), the region chrome
//! (`RegionSelectionOverlay.Render`, in screenshot pixels), the resize
//! nodes, the HUD (`MagnifierPanel`, `SelectionInfoPanel`, in DIPs) and the
//! toolbar.

use std::time::Instant;

use egui::{Color32, CornerRadius, Pos2, Rect, Stroke, StrokeKind, TextureHandle, TextureOptions, pos2, vec2};
use sharex_ui::{Palette, palette, widgets};

use super::{RegionCapture, ResizeNode, options_row, toolbar};
use crate::geometry::{PointF, Rect as RectI, RectF};
use sharex_image_editor::editor as ed;

/// What the magnifier picture shows: the pixel, size, count and scale.
type MagnifierKey = (i32, i32, i32, i32, u32);

/// One overlay window's view of the session: the monitor it covers and
/// what it uploaded.
pub struct OverlayView {
    /// The desktop pixels the window covers.
    pub monitor: RectI,
    frozen: Option<TextureHandle>,
    /// A frozen screen texture from an earlier capture, uploaded into
    /// rather than allocating another when it's the size needed.
    spare: Option<TextureHandle>,
    /// The editor's version the frozen texture shows (with annotations).
    frozen_version: Option<u64>,
    magnifier: Option<(TextureHandle, MagnifierKey)>,
    /// Where the toolbar was drawn, in points.
    toolbar: Option<toolbar::Layout>,
    /// Where the tool options row was drawn.
    options: Option<options_row::OptionsLayout>,
    /// Where the open option popup was drawn.
    popup: Option<Rect>,
    /// The picture of the ellipse and freehand chrome.
    shapes: Option<(TextureHandle, super::chrome::ShapeChrome)>,
}

impl OverlayView {
    pub fn new(monitor: RectI) -> Self {
        Self {
            monitor,
            frozen: None,
            spare: None,
            frozen_version: None,
            magnifier: None,
            toolbar: None,
            options: None,
            popup: None,
            shapes: None,
        }
    }

    /// Gives the view a frozen screen texture kept from an earlier capture
    /// (its window's), to upload into.
    pub fn keep_texture(&mut self, texture: Option<TextureHandle>) {
        self.spare = texture;
    }

    /// The frozen screen's texture, to keep for the next capture.
    pub fn take_frozen_texture(&mut self) -> Option<TextureHandle> {
        self.frozen.take().or_else(|| self.spare.take())
    }

    /// Shows `image` as the frozen screen: uploaded into the texture
    /// there is when it's the same size, else into a new one.
    fn set_frozen(&mut self, ctx: &egui::Context, image: egui::ColorImage) {
        let texture = self.frozen.take().or_else(|| self.spare.take());
        self.frozen = Some(match texture {
            Some(mut tex) if tex.size() == image.size => {
                tex.set_partial([0, 0], image, TextureOptions::NEAREST);
                tex
            }
            _ => ctx.load_texture("frozen screen", image, TextureOptions::NEAREST),
        });
    }

    /// The toolbar part under `p` (points in this window), as last drawn.
    pub fn toolbar_part_at(&self, p: Pos2) -> Option<toolbar::Part> {
        if self.popup.is_some_and(|r| r.contains(p)) {
            return Some(toolbar::Part::Popup);
        }
        if let Some(item) = self.options.as_ref().and_then(|l| l.hit(p)) {
            return Some(toolbar::Part::Option(item));
        }
        if self.options.as_ref().is_some_and(|l| l.rect.contains(p)) {
            return Some(toolbar::Part::Background);
        }
        self.toolbar.as_ref().and_then(|l| l.hit(p))
    }

    /// Where `p` (points) falls along an options row slider: 0 to 1.
    pub fn slider_fraction(&self, item: options_row::OptionItem, p: Pos2) -> Option<f32> {
        let r = self.options.as_ref()?.item_rect(item)?;
        let scale = r.height() / 40.0;
        // The 120px slider 29px in, its thumb's centre 10px from each end.
        let left = r.min.x + (29.0 + 10.0) * scale;
        let width = (120.0 - 20.0) * scale;
        Some(((p.x - left) / width).clamp(0.0, 1.0))
    }
}

/// The `w` x `h` part of `image` at `x`, `y` as egui's colours
/// (`ColorImage::from_rgba_unmultiplied` of the crop, without the crop).
fn frozen_image(image: &image::RgbaImage, x: i32, y: i32, w: i32, h: i32) -> egui::ColorImage {
    let stride = image.width() as usize * 4;
    let raw = image.as_raw();
    let mut pixels = Vec::with_capacity(w as usize * h as usize);
    for row in y as usize..(y + h) as usize {
        let start = row * stride + x as usize * 4;
        let line = &raw[start..start + w as usize * 4];
        pixels.extend(line.chunks_exact(4).map(|p| Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3])));
    }
    egui::ColorImage::new([w as usize, h as usize], pixels)
}

/// The `w` x `h` part of `pm` (premultiplied ARGB) at `x`, `y` as egui's
/// colours.
fn composite_image(pm: &sharex_skia::Pixmap, x: i32, y: i32, w: i32, h: i32) -> egui::ColorImage {
    let stride = pm.width() as usize;
    let data = pm.pixels();
    let mut pixels = Vec::with_capacity(w as usize * h as usize);
    for row in y as usize..(y + h) as usize {
        let start = row * stride + x as usize;
        let line = &data[start..start + w as usize];
        pixels.extend(line.iter().map(|&c| Color32::from_rgba_premultiplied((c >> 16) as u8, (c >> 8) as u8, c as u8, (c >> 24) as u8)));
    }
    egui::ColorImage::new([w as usize, h as usize], pixels)
}

/// How much of `[d0, d1]` a `[3, 3]` dash pattern (shifted by `offset`)
/// covers.
fn dash_cover(d0: f64, d1: f64, offset: f64) -> f64 {
    let mut cover = 0.0;
    let first = ((d0 + offset) / 6.0).floor() as i64 - 1;
    for k in first..=first + 2 {
        let (s, e) = (k as f64 * 6.0 - offset, k as f64 * 6.0 + 3.0 - offset);
        cover += (e.min(d1) - s.max(d0)).max(0.0);
    }
    cover.min(d1 - d0)
}

/// A 1px axis-aligned dashed line (`StrokeDashArray="3,3"`) from `a` to `b`
/// (image pixels, on the line's centre), black, then white dashes offset
/// by 3, each pixel as much as the line and its dash cover it. `start` is
/// the distance along the path at `a`.
pub(crate) fn dashed_axis_line(painter: &egui::Painter, m: &Mapper, a: (f64, f64), b: (f64, f64), start: f64) {
    let vertical = (a.0 - b.0).abs() < 1e-9;
    let len = if vertical { (b.1 - a.1).abs() } else { (b.0 - a.0).abs() };
    if len <= 0.0 {
        return;
    }
    let dir = if vertical { (b.1 - a.1).signum() } else { (b.0 - a.0).signum() };
    // Across the line: the pixels the 1px width covers.
    let across = if vertical { a.0 } else { a.1 };
    let (lo, hi) = (across - 0.5, across + 0.5);
    let along0 = if vertical { a.1 } else { a.0 };
    let (p0, p1) = (along0.min(along0 + dir * len), along0.max(along0 + dir * len));
    for cross in lo.floor() as i64..hi.ceil() as i64 {
        let wc = (hi.min(cross as f64 + 1.0) - lo.max(cross as f64)).max(0.0);
        if wc <= 0.0 {
            continue;
        }
        for px in p0.floor() as i64..p1.ceil() as i64 {
            let (s, e) = ((px as f64).max(p0), (px as f64 + 1.0).min(p1));
            if e <= s {
                continue;
            }
            // Distances along the path for this pixel's span.
            let (d0, d1) = if dir > 0.0 { (s - along0, e - along0) } else { (along0 - e, along0 - s) };
            let (d0, d1) = (start + d0, start + d1);
            let black = dash_cover(d0, d1, 0.0) * wc;
            let white = dash_cover(d0, d1, 3.0) * wc;
            let r = if vertical { m.rect(cross as f64, px as f64, 1.0, 1.0) } else { m.rect(px as f64, cross as f64, 1.0, 1.0) };
            if black > 0.0 {
                pixel_rect(painter, r, Color32::from_black_alpha((black * 255.0).round() as u8));
            }
            if white > 0.0 {
                pixel_rect(painter, r, Color32::from_white_alpha((white * 255.0).round() as u8));
            }
        }
    }
}

/// `SelectionResizeNode`: a 15px white circle with a soft shadow.
pub fn paint_node(painter: &egui::Painter, m: &Mapper, c: (f64, f64)) {
    let half = super::RESIZE_NODE_SIZE / 2.0;
    let r = m.rect(c.0 - half, c.1 - half, half * 2.0, half * 2.0);
    let radius = CornerRadius::same((r.width() / 2.0).round() as u8);
    let shadow = egui::epaint::Shadow { offset: [0, 0], blur: 8, spread: 0, color: Color32::from_black_alpha(100) };
    painter.add(shadow.as_shape(r, radius));
    painter.rect_filled(r, radius, Color32::WHITE);
}

/// `AccentPalette.GetHaloColor`: black or white, whichever contrasts more
/// with the accent.
pub fn halo(accent: Color32) -> Color32 {
    let lum = |c: Color32| {
        let f = |v: u8| {
            let v = v as f64 / 255.0;
            if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * f(c.r()) + 0.7152 * f(c.g()) + 0.0722 * f(c.b())
    };
    let ratio = |a: f64, b: f64| (a.max(b) + 0.05) / (a.min(b) + 0.05);
    let l = lum(accent);
    if ratio(l, 0.0) >= ratio(l, 1.0) { Color32::BLACK } else { Color32::WHITE }
}

/// Converts screenshot (image) pixels to this window's points.
pub struct Mapper {
    /// The window's top-left corner in screenshot pixels.
    pub origin: (f64, f64),
    pub ppp: f32,
    /// Physical pixels per image pixel (the editor's zoom; 1 in region
    /// capture).
    pub scale: f64,
    /// Where `origin` is, in points.
    pub offset: Pos2,
}

impl Mapper {
    /// Image pixels 1:1 with physical pixels, `origin` at the window's
    /// top-left corner.
    pub fn new(origin: (f64, f64), ppp: f32) -> Self {
        Self { origin, ppp, scale: 1.0, offset: Pos2::ZERO }
    }

    pub fn pos(&self, x: f64, y: f64) -> Pos2 {
        pos2(
            self.offset.x + (((x - self.origin.0) * self.scale) as f32) / self.ppp,
            self.offset.y + (((y - self.origin.1) * self.scale) as f32) / self.ppp,
        )
    }

    /// The image point at `p` (points).
    pub fn image_pos(&self, p: Pos2) -> (f64, f64) {
        (
            self.origin.0 + ((p.x - self.offset.x) * self.ppp) as f64 / self.scale,
            self.origin.1 + ((p.y - self.offset.y) * self.ppp) as f64 / self.scale,
        )
    }

    pub fn rect(&self, x: f64, y: f64, w: f64, h: f64) -> Rect {
        Rect::from_min_max(self.pos(x, y), self.pos(x + w, y + h))
    }
}

/// A 1px line of the selection's border with dashes (`DrawAntLine`): the
/// solid accent line, then `[5, 5]` dashes in the halo colour at 230/255
/// from its start, each pixel as much as the dash covers it (Skia's
/// anti-aliased butt-capped dashes on a half-pixel line).
fn dashes(painter: &egui::Painter, m: &Mapper, (x0, y0): (f64, f64), len: f64, vertical: bool, halo: Color32) {
    if len < 0.0 {
        return;
    }
    let mut runs: Vec<(i64, i64, f64)> = Vec::new(); // (first pixel, last pixel, coverage)
    let pixels = len.ceil() as i64;
    let mut push = |px: i64, c: f64| {
        if c <= 0.0 {
            return;
        }
        match runs.last_mut() {
            Some((_, last, cov)) if *last + 1 == px && (*cov - c).abs() < 1e-9 => *last = px,
            _ => runs.push((px, px, c)),
        }
    };
    // Pixel `i` along the line covers t in [i - 0.5, i + 0.5].
    for i in 0..=pixels {
        let (a, b) = (i as f64 - 0.5, i as f64 + 0.5);
        let mut cover = 0.0;
        let first = ((a.max(0.0)) / 10.0).floor() as i64;
        for k in first..=first + 1 {
            let (s, e) = ((k * 10) as f64, (k * 10 + 5) as f64);
            let (s, e) = (s.max(0.0), e.min(len));
            cover += (e.min(b) - s.max(a)).max(0.0);
        }
        push(i, cover.min(1.0));
    }
    for (first, last, c) in runs {
        let alpha = (230.0 * c).round() as u8;
        let color = Color32::from_rgba_unmultiplied(halo.r(), halo.g(), halo.b(), alpha);
        let n = (last - first + 1) as f64;
        let r = if vertical {
            m.rect(x0, y0 + first as f64, 1.0, n)
        } else {
            m.rect(x0 + first as f64, y0, n, 1.0)
        };
        pixel_rect(painter, r, color);
    }
}

/// Fills `r` exactly, with no anti-aliasing feather: these rects are whole
/// pixels with their coverage already in the colour, and egui's feathering
/// would blur 1px rects (a half-covered dash end came out fully covered).
pub(crate) fn pixel_rect(painter: &egui::Painter, r: Rect, color: Color32) {
    let mut mesh = egui::Mesh::default();
    mesh.add_colored_rect(r, color);
    painter.add(mesh);
}

/// `DrawAntRectangle` of `GetAntRectangle(r)`: on the region's own edge
/// pixels.
pub fn ant_rectangle(painter: &egui::Painter, m: &Mapper, r: RectF, accent: Color32, halo: Color32) {
    let (x, y) = (r.x.floor(), r.y.floor());
    let (w, h) = ((r.w - 1.0).max(0.0), (r.h - 1.0).max(0.0));
    let (right, bottom) = (x + w, y + h);
    // The solid 1px outline.
    pixel_rect(painter, m.rect(x, y, w + 1.0, 1.0), accent);
    if h > 0.0 {
        pixel_rect(painter, m.rect(x, bottom, w + 1.0, 1.0), accent);
    }
    if h > 1.0 {
        pixel_rect(painter, m.rect(x, y + 1.0, 1.0, h - 1.0), accent);
        if w > 0.0 {
            pixel_rect(painter, m.rect(right, y + 1.0, 1.0, h - 1.0), accent);
        }
    }
    // Top, left, right and bottom dashed lines, each from its start.
    dashes(painter, m, (x, y), w, false, halo);
    dashes(painter, m, (x, y), h, true, halo);
    dashes(painter, m, (right, y), h, true, halo);
    dashes(painter, m, (x, bottom), w, false, halo);
}

/// `DrawPixelCross`: 21 pixels each way through (`cx`, `cy`).
pub fn pixel_cross(painter: &egui::Painter, m: &Mapper, color: Color32, cx: f64, cy: f64) {
    painter.rect_filled(m.rect(cx - 10.0, cy, 21.0, 1.0), 0.0, color);
    painter.rect_filled(m.rect(cx, cy - 10.0, 1.0, 21.0), 0.0, color);
}

/// A `Border` label: padding 7,4, 5px corners, main background, 1px
/// border, 12px SemiBold text. Returns its size for `text`.
pub(crate) fn label_size(ui: &egui::Ui, text: &str) -> egui::Vec2 {
    let font = widgets::font(12.0, 600);
    let lines: Vec<&str> = text.lines().collect();
    let w = lines.iter().map(|l| widgets::text_width(ui, l, font.clone())).fold(0.0, f32::max);
    let h = widgets::line_height(ui, &font) * lines.len().max(1) as f32;
    vec2(w + 2.0 * (7.0 + 1.0), h + 2.0 * (4.0 + 1.0))
}

pub(crate) fn paint_label(ui: &egui::Ui, p: &Palette, rect: Rect, text: &str, centred: bool) {
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(5), p.background_main);
    painter.rect_stroke(rect, CornerRadius::same(5), Stroke::new(1.0, p.border), StrokeKind::Inside);
    let font = widgets::font(12.0, 600);
    let lh = widgets::line_height(ui, &font);
    let inner = rect.shrink2(vec2(8.0, 5.0));
    for (i, line) in text.lines().enumerate() {
        let g = widgets::galley(ui, line, font.clone(), p.text_secondary, f32::INFINITY);
        let x = if centred { inner.min.x + ((inner.width() - g.size().x.ceil()) / 2.0).round_ties_even() } else { inner.min.x };
        painter.galley(pos2(x, inner.min.y + i as f32 * lh), g, p.text_secondary);
    }
}

/// Rounds a point to whole physical pixels.
fn snap(v: f32, ppp: f32) -> f32 {
    (v * ppp).round() / ppp
}

impl RegionCapture {
    /// Uploads `view`'s part of the frozen screen when it isn't up to date:
    /// the screenshot at first, the editor's composite once it has
    /// annotations (or text being typed). [`Self::paint`] does it first;
    /// done before the first frame, that frame only draws.
    pub fn update_frozen(&mut self, ctx: &egui::Context, view: &mut OverlayView) {
        let sb = self.screen_bounds;
        let part = view.monitor.intersect(&sb);
        if !part.is_valid() {
            return;
        }
        if let Some(e) = self.editor.as_mut()
            && view.frozen_version != Some(e.version())
            && (!e.annotations().is_empty() || e.text_edit().is_some() || view.frozen_version.is_some())
        {
            let version = e.version();
            let pm = e.composite();
            crate::trace::mark("editor composite");
            let image = composite_image(pm, part.x - sb.x, part.y - sb.y, part.w, part.h);
            crate::trace::mark("frozen screen to ColorImage");
            view.set_frozen(ctx, image);
            view.frozen_version = Some(version);
        } else if view.frozen.is_none() {
            let image = frozen_image(&self.screenshot, part.x - sb.x, part.y - sb.y, part.w, part.h);
            crate::trace::mark("frozen screen to ColorImage");
            view.set_frozen(ctx, image);
        }
    }

    /// Draws the overlay for the window `view` belongs to (its whole area).
    pub fn paint(&mut self, ui: &mut egui::Ui, view: &mut OverlayView, now: Instant) {
        let ctx = ui.ctx().clone();
        let p = palette(&ctx);
        let ppp = ctx.pixels_per_point();
        let painter = ui.painter().clone();
        let sb = self.screen_bounds;
        let m = Mapper::new(((view.monitor.x - sb.x) as f64, (view.monitor.y - sb.y) as f64), ppp);
        let accent = p.accent_start;
        let halo = halo(accent);

        // The frozen screen: this monitor's part, pixel for pixel, with the
        // annotations (the editor workspace under the region overlay).
        let part = view.monitor.intersect(&sb);
        self.update_frozen(&ctx, view);
        if part.is_valid()
            && let Some(tex) = &view.frozen
        {
            let r = m.rect((part.x - sb.x) as f64, (part.y - sb.y) as f64, part.w as f64, part.h as f64);
            painter.image(tex.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        }

        // `RegionSelectionOverlay.Render`; with an annotation tool, only a
        // selection kept outside quick capture.
        let show_region = self.region_tool_active || (!self.quick_capture() && self.has_any_region());
        if show_region {
            let part_image = RectF::new((part.x - sb.x) as f64, (part.y - sb.y) as f64, part.w as f64, part.h as f64);
            self.paint_regions(&painter, &m, view, part_image, accent, halo);
        }

        // The resize nodes: 15 screenshot pixels, white, soft shadow.
        if self.resize_nodes_visible() {
            for node in ResizeNode::ALL {
                let c = node.position(self.selection.bounds);
                let half = super::RESIZE_NODE_SIZE / 2.0;
                let r = m.rect(c.x - half, c.y - half, half * 2.0, half * 2.0);
                let radius = CornerRadius::same((r.width() / 2.0).round() as u8);
                let shadow = egui::epaint::Shadow { offset: [0, 0], blur: 8, spread: 0, color: Color32::from_black_alpha(100) };
                painter.add(shadow.as_shape(r, radius));
                painter.rect_filled(r, radius, Color32::WHITE);
            }
        }

        self.paint_editor_chrome(ui, &painter, &m);
        if self.region_tool_active {
            self.paint_hud(ui, view, &m, &p, ppp);
        }
        self.paint_toolbar(ui, view, &p, ppp, now);
        // The open option popup, under its button.
        view.popup = None;
        if let (Some(item), Some(row), Some(e)) = (self.popups.open, view.options.as_ref(), self.editor.as_mut())
            && let Some(anchor) = row.item_rect(item)
        {
            // The monitor's work area, in this window's points.
            let mon = view.monitor;
            let wa = self.screens.iter().find(|s| s.bounds == mon).map(|s| s.work_area).unwrap_or(mon);
            let screen = Rect::from_min_size(
                pos2((wa.x - mon.x) as f32 / ppp, (wa.y - mon.y) as f32 / ppp),
                vec2(wa.w as f32, wa.h as f32) / ppp,
            );
            let r = self.popups.show(ui, &p, anchor, screen, e);
            if r.pick_screen_color {
                // `ScreenColorPickerWindow.PickAsync`: the host opens it.
                self.pick_request = Some(item);
            }
            view.popup = self.popups.rect;
        }
        // The open tooltip, at the pointer where it opened, on its monitor.
        if let Some((part, (x, y), _)) = self.tip.open
            && view.toolbar.is_some()
            && view.monitor.contains(x.floor() as i32, y.floor() as i32)
            && let Some(text) = self.tooltip_text(part)
        {
            let at = pos2((x - view.monitor.x as f64) as f32 / ppp, (y - view.monitor.y as f64) as f32 / ppp + super::tooltip::VERTICAL_OFFSET);
            let area = Rect::from_min_size(Pos2::ZERO, vec2(view.monitor.w as f32, view.monitor.h as f32) / ppp);
            super::tooltip::paint(ui, &p, at, area, &text, self.tip.opacity(now));
        }
    }

    /// `RegionSelectionOverlay.Render` for this window's part of the
    /// screenshot (`part`, image pixels): the dimming, the added regions,
    /// a window hovered beside them, the selection (or hovered window), the
    /// centre crosshair and the screen crosshair.
    fn paint_regions(&self, painter: &egui::Painter, m: &Mapper, view: &mut OverlayView, part: RectF, accent: Color32, halo: Color32) {
        use super::chrome::{ShapeChrome, cursor_crosshair, uncovered_rectangles};
        use super::shape::{CaptureRegion, RegionShape};
        let surface = RectF::new(0.0, 0.0, self.width, self.height);
        let active = self.active_rectangle().intersect(&surface);
        let selection_valid = self.selection.bounds.is_valid();
        let active_region =
            if selection_valid { self.selection.with_bounds(active) } else { CaptureRegion::new(self.hover_shape(), active) };
        let extra_hover = if self.show_hover_with_selection() && selection_valid && self.hover != self.selection.bounds {
            self.hover.intersect(&surface)
        } else {
            RectF::default()
        };
        let hover_shape = self.hover_shape();
        let dim_all_except = self.options.DimAllExceptSelection;
        let alpha = dim_alpha(self.options.BackgroundDimStrength);
        let dim = Color32::from_rgba_unmultiplied(0, 0, 0, alpha);
        let fill = |r: RectF| {
            let r = r.intersect(&part);
            if r.is_valid() {
                painter.rect_filled(m.rect(r.x, r.y, r.w, r.h), 0.0, dim);
            }
        };
        // The non-rectangular chrome's picture: what it dims around and
        // outlines.
        let mut dim_shapes: Option<Vec<CaptureRegion>> = None;
        if alpha > 0 {
            let undimmed = if dim_all_except && (self.selection_pending || !selection_valid) { RectF::default() } else { active };
            let geometry = !active_region.is_rectangle()
                || (extra_hover.is_valid() && hover_shape != RegionShape::Rectangle)
                || self.regions.iter().any(|r| !r.is_rectangle());
            if geometry {
                let mut clear = self.regions.clone();
                if undimmed.is_valid() {
                    clear.push(active_region.clone());
                }
                if !dim_all_except && extra_hover.is_valid() {
                    clear.push(CaptureRegion::new(hover_shape, extra_hover));
                }
                dim_shapes = Some(clear);
            } else if !self.regions.is_empty() || extra_hover.is_valid() {
                let mut clear: Vec<RectF> = self.regions.iter().map(|r| r.bounds).collect();
                clear.push(undimmed);
                if !dim_all_except {
                    clear.push(extra_hover);
                }
                for r in uncovered_rectangles(surface, &clear) {
                    fill(r);
                }
            } else if undimmed.is_valid() {
                let c = undimmed;
                for r in [
                    RectF::new(surface.x, surface.y, surface.w, c.y - surface.y),
                    RectF::new(surface.x, c.y, c.x - surface.x, c.h),
                    RectF::new(c.right(), c.y, surface.right() - c.right(), c.h),
                    RectF::new(surface.x, c.bottom(), surface.w, surface.bottom() - c.bottom()),
                ] {
                    if r.is_valid() {
                        fill(r);
                    }
                }
            } else {
                fill(surface);
            }
        }
        // The outlines, in `Render`'s order.
        let mut outlines: Vec<CaptureRegion> = Vec::new();
        for region in &self.regions {
            let visible = region.bounds.intersect(&surface);
            if visible.is_valid() {
                outlines.push(region.with_bounds(visible));
            }
        }
        if extra_hover.is_valid() {
            outlines.push(CaptureRegion::new(hover_shape, extra_hover));
        }
        if active.is_valid() {
            outlines.push(active_region.clone());
        }
        // Ellipses and freehand regions (and the dimming around them) as a
        // picture over their bounds, the rest of the dimming as rectangles.
        let shaped: Vec<CaptureRegion> = outlines.iter().filter(|r| !r.is_rectangle()).cloned().collect();
        let mut area: Option<RectF> = None;
        let mut grow = |r: RectF| {
            if r.is_valid() {
                let r = RectF::new(r.x.floor() - 1.0, r.y.floor() - 1.0, r.w.ceil() + 3.0, r.h.ceil() + 3.0);
                area = Some(match area {
                    Some(a) => {
                        let (l, t) = (a.x.min(r.x), a.y.min(r.y));
                        RectF::new(l, t, a.right().max(r.right()) - l, a.bottom().max(r.bottom()) - t)
                    }
                    None => r,
                });
            }
        };
        shaped.iter().for_each(|r| grow(r.bounds));
        if let Some(clear) = &dim_shapes {
            clear.iter().for_each(|r| grow(r.bounds));
        }
        let area = area.map(|a| a.intersect(&part)).filter(RectF::is_valid);
        // Outside the picture the dimming is whole rectangles.
        if dim_shapes.is_some() {
            match area {
                Some(a) => uncovered_rectangles(part, &[a]).into_iter().for_each(&fill),
                None => fill(part),
            }
        }
        match area {
            Some(a) => {
                let chrome = ShapeChrome {
                    area: crate::geometry::Rect::new(a.x as i32, a.y as i32, a.w as i32, a.h as i32),
                    dim: dim_shapes.map(|c| (alpha, c)),
                    outlines: shaped,
                    accent,
                    halo,
                };
                chrome.paint(painter, m, &mut view.shapes);
            }
            None => view.shapes = None,
        }
        for region in outlines.iter().filter(|r| r.is_rectangle()) {
            ant_rectangle(painter, m, region.bounds, accent, halo);
        }
        if active.is_valid() && self.options.ShowCenterCrosshair && selection_valid {
            let c = self.selection.bounds.center();
            let (cx, cy) = (c.x.floor(), c.y.floor());
            pixel_cross(painter, m, halo, cx - 1.0, cy - 1.0);
            pixel_cross(painter, m, accent, cx, cy);
        }
        // `ShowCursorCrosshair`: with the region tool.
        if self.options.ShowScreenCrosshair && self.region_tool_active {
            cursor_crosshair(painter, m, surface, (self.last_pointer.x, self.last_pointer.y), accent, halo);
        }
    }

    /// The editor's selection chrome (`OverlayCanvas`).
    fn paint_editor_chrome(&self, ui: &egui::Ui, painter: &egui::Painter, m: &Mapper) {
        let Some(e) = &self.editor else { return };
        if self.region_tool_active {
            return;
        }
        let p = palette(painter.ctx());
        crate::editor_chrome::paint(ui, painter, &p, m, e, false);
    }

    /// The magnifier with the pointer's position under it, and the
    /// selection's (or hovered window's) position and size.
    fn paint_hud(&self, ui: &egui::Ui, view: &mut OverlayView, m: &Mapper, p: &Palette, ppp: f32) {
        let sb = self.screen_bounds;
        let on_this_monitor = |pt: PointF| {
            let d = (sb.x + pt.x.floor() as i32, sb.y + pt.y.floor() as i32);
            crate::screenshot::screen_from_point(&self.screens, d).map(|s| s.bounds) == Some(view.monitor)
                || (self.screens.is_empty() && view.monitor.contains(d.0, d.1))
        };
        let area = Rect::from_min_size(Pos2::ZERO, vec2(view.monitor.w as f32, view.monitor.h as f32) / ppp);
        let show_magnifier = self.magnifier_shown();
        let show_info = self.options.ShowInfo;
        if self.pointer_known && (show_magnifier || show_info) && on_this_monitor(self.last_pointer) {
            let pointer = m.pos(self.last_pointer.x, self.last_pointer.y);
            let ix = (self.last_pointer.x.round() as i32).clamp(0, self.width as i32 - 1);
            let iy = (self.last_pointer.y.round() as i32).clamp(0, self.height as i32 - 1);
            let text = self.pointer_info_text(ix, iy);
            let info = label_size(ui, &text);
            let mag_size = (self.options.MagnifierSize.clamp(50, 500) + 2) as f32;
            let width = if show_magnifier { mag_size.max(if show_info { info.x } else { 0.0 }) } else { info.x };
            let height = match (show_magnifier, show_info) {
                (true, true) => mag_size + 10.0 + info.y,
                (true, false) => mag_size,
                _ => info.y,
            };
            // `PositionPanelNearPointer`, 18 DIPs off the pointer.
            let mut x = pointer.x + 18.0;
            let mut y = pointer.y + 18.0;
            if x + width > area.max.x {
                x = pointer.x - width - 18.0;
            }
            if y + height > area.max.y {
                y = pointer.y - height - 18.0;
            }
            let x = snap(x.clamp(area.min.x, (area.max.x - width).max(area.min.x)), ppp);
            let y = snap(y.clamp(area.min.y, (area.max.y - height).max(area.min.y)), ppp);
            let mut top = y;
            if show_magnifier {
                let mx = x + ((width - mag_size) / 2.0).round_ties_even();
                let count = super::magnifier::normalize_pixel_count(
                    self.options.MagnifierPixelCount,
                    self.options.MagnifierSize,
                    ppp as f64,
                );
                let key = (ix, iy, self.options.MagnifierSize, count, (ppp * 100.0) as u32);
                let stale = view.magnifier.as_ref().is_none_or(|(_, k)| *k != key);
                if stale {
                    let accent = p.accent_start;
                    let img = super::magnifier::render(
                        &self.screenshot,
                        (ix, iy),
                        self.options.MagnifierSize.clamp(50, 500),
                        count,
                        self.options.UseSquareMagnifier,
                        ppp as f64,
                        [accent.r(), accent.g(), accent.b()],
                    );
                    let ci = egui::ColorImage::from_rgba_unmultiplied(
                        [img.width() as usize, img.height() as usize],
                        img.as_raw(),
                    );
                    match &mut view.magnifier {
                        Some((tex, k)) => {
                            tex.set(ci, TextureOptions::NEAREST);
                            *k = key;
                        }
                        None => view.magnifier = Some((ui.ctx().load_texture("magnifier", ci, TextureOptions::NEAREST), key)),
                    }
                }
                if let Some((tex, _)) = &view.magnifier {
                    let size = tex.size_vec2() / ppp;
                    ui.painter().image(
                        tex.id(),
                        Rect::from_min_size(pos2(mx, top), size),
                        Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                        Color32::WHITE,
                    );
                }
                top += mag_size + 10.0;
            }
            if show_info {
                let ix = x + ((width - info.x) / 2.0).round_ties_even();
                paint_label(ui, p, Rect::from_min_size(pos2(ix, top), info), &text, true);
            }
        }

        // `UpdateSelectionInfo`: below the selection, else above it.
        let selection = self.active_rectangle();
        if show_info && selection.is_valid() {
            let w = (selection.w.round() as i64).max(1);
            let h = (selection.h.round() as i64).max(1);
            let text = format!("X: {} Y: {} W: {w} H: {h}", sb.x + selection.x.round() as i32, sb.y + selection.y.round() as i32);
            let size = label_size(ui, &text);
            let below = PointF::new(selection.x, selection.y.max(selection.bottom() - 1.0));
            let mut anchor = below;
            let mut y = m.pos(0.0, selection.bottom()).y + 8.0;
            if !on_this_monitor(below) || y + size.y > area.max.y {
                anchor = PointF::new(selection.x, selection.y);
                y = m.pos(0.0, selection.y).y - size.y;
            }
            if on_this_monitor(anchor) {
                let left = m.pos(selection.x, 0.0).x;
                let x = left.clamp(area.min.x, (area.max.x - size.x).max(area.min.x));
                let y = y.clamp(area.min.y, (area.max.y - size.y).max(area.min.y));
                paint_label(ui, p, Rect::from_min_size(pos2(x.round(), y.round()), size), &text, false);
            }
        }
    }

    fn paint_toolbar(&self, ui: &egui::Ui, view: &mut OverlayView, p: &Palette, ppp: f32, now: Instant) {
        view.toolbar = None;
        if !self.enable_annotations {
            return;
        }
        let Some((ax, ay)) = self.toolbar_anchor() else { return };
        let hidden = self.toolbar_hidden_amount(now);
        let scale = self.toolbar_scale();
        let size = toolbar::size(scale, &self.bar);
        // The toolbar's monitor's scaling, in desktop pixels; kept on the
        // whole overlay (`PositionCaptureToolbar`).
        let sb = self.screen_bounds;
        let ts = crate::screenshot::screen_from_point(&self.screens, (ax as i32, ay as i32)).map(|s| s.scale).unwrap_or(ppp as f64);
        let (w, h) = (size.x as f64 * ts, size.y as f64 * ts);
        let left = (ax - w / 2.0).clamp(sb.x as f64, (sb.right() as f64 - w).max(sb.x as f64));
        let top = ay.clamp(sb.y as f64, (sb.bottom() as f64 - h).max(sb.y as f64));
        let min = pos2(
            (((left - view.monitor.x as f64) / ts) as f32 * ppp).round_ties_even() / ppp,
            (((top - view.monitor.y as f64) / ts) as f32 * ppp).round_ties_even() / ppp - 28.0 * hidden,
        );
        let layout = toolbar::layout(min, scale, &self.bar);
        let window = Rect::from_min_size(Pos2::ZERO, vec2(view.monitor.w as f32, view.monitor.h as f32) / ppp);
        view.options = None;
        if !layout.rect.intersects(window) || hidden >= 1.0 {
            return;
        }
        let active_tool = self.active_annotation_tool();
        let active_region = self.region_tool_active.then_some(self.region_shape);
        toolbar::paint(ui.painter(), p, &layout, self.toolbar_hover, active_region, active_tool.as_ref(), scale, 1.0 - hidden, &self.bar);
        // `ShowToolOptions` while an annotation tool is active.
        if let (Some(tool), Some(e)) = (&active_tool, &self.editor) {
            let visible = ed::Visible::of(tool.clone(), e.selected());
            if let Some(row) = options_row::layout(ui, layout.rect, scale, &visible, &e.tools) {
                let mut painter = ui.painter().clone();
                painter.set_opacity(1.0 - hidden);
                let hovered = match self.toolbar_hover {
                    Some(toolbar::Part::Option(item)) => Some(item),
                    _ => None,
                };
                options_row::paint(ui, &painter, p, &row, tool, &e.tools, hovered, self.popups.open, scale);
                if hidden <= 0.0 {
                    view.options = Some(row);
                }
            }
        }
        if hidden <= 0.0 {
            view.toolbar = Some(layout);
        }
    }
}

/// Black then white 1px dashes, 3 on 3, along `pts` (egui's dashes: for
/// the outlines Skia would draw anti-aliased off the pixel grid).
pub(crate) fn dashed_poly(painter: &egui::Painter, pts: &[Pos2]) {
    if pts.len() < 2 {
        return;
    }
    painter.extend(egui::Shape::dashed_line(pts, Stroke::new(1.0, Color32::BLACK), 3.0, 3.0));
    painter.extend(egui::Shape::dashed_line_with_offset(pts, Stroke::new(1.0, Color32::WHITE), &[3.0], &[3.0], 3.0));
}

/// `GetDimAlpha`: `BackgroundDimStrength` percent of 255.
fn dim_alpha(strength: i32) -> u8 {
    if strength <= 0 { 0 } else { ((strength as f64 / 100.0 * 255.0).round() as i32).clamp(0, 255) as u8 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn halo_of_the_sharex_accent_is_black() {
        assert_eq!(halo(Color32::from_rgb(0x09, 0xAA, 0xFF)), Color32::BLACK);
        assert_eq!(halo(Color32::from_rgb(0x20, 0x20, 0x60)), Color32::WHITE);
        assert_eq!(dim_alpha(20), 51);
    }

    /// A 7x5 image with opaque, translucent and transparent pixels.
    fn sample() -> image::RgbaImage {
        image::RgbaImage::from_fn(7, 5, |x, y| {
            let a = [255, 128, 0, 1, 254][(x as usize + y as usize) % 5];
            image::Rgba([(x * 37) as u8, (y * 51) as u8, (x * y * 13) as u8, a])
        })
    }

    #[test]
    fn the_frozen_image_is_the_crop_as_egui_converted_it() {
        let shot = sample();
        let (x, y, w, h) = (2, 1, 4, 3);
        let crop = image::imageops::crop_imm(&shot, x, y, w, h).to_image();
        let expected = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], crop.as_raw());
        let got = frozen_image(&shot, x as i32, y as i32, w as i32, h as i32);
        assert_eq!(got.size, expected.size);
        assert_eq!(got.pixels, expected.pixels);
    }

    #[test]
    fn the_composite_image_is_the_premultiplied_crop() {
        let shot = sample();
        let pm = sharex_skia::Pixmap::from_rgba(shot.width(), shot.height(), shot.as_raw());
        let (x, y, w, h) = (1, 2, 5, 3);
        let mut rgba = Vec::new();
        for row in y..y + h {
            for col in x..x + w {
                let c = pm.pixel(col, row);
                rgba.extend_from_slice(&[(c >> 16) as u8, (c >> 8) as u8, c as u8, (c >> 24) as u8]);
            }
        }
        let expected = egui::ColorImage::from_rgba_premultiplied([w as usize, h as usize], &rgba);
        let got = composite_image(&pm, x, y, w, h);
        assert_eq!(got.pixels, expected.pixels);
    }
}
