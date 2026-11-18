//! The image editor's `OverlayCanvas`, drawn over the workspace by both of
//! its hosts (region capture and the editor window): the hovered shape's
//! dashed outline, the selected one's handles and rotation line
//! (`EditorSelectionController`), the text box being typed in
//! (`CreateCreationTextEditor`, `ShowTextEditor`,
//! `ShowSpeechBalloonTextEditor`), the crop overlay with its adorner and
//! handles (`CropOverlay`, `CropAdorner`, `CropAnnotation.CreateResizeHandle`,
//! `ShowCropConfirmButton`) and the cut out and clone regions.
//!
//! Everything is in image pixels through a [`Mapper`]: the overlay scales
//! with the editor's zoom (`UpdateOverlayCanvasLayout`'s `ScaleTransform`).

use egui::{Color32, CornerRadius, Pos2, Rect, Stroke, StrokeKind, pos2};
use sharex_image_editor::editor::{self as ed, CropHandle, Outline, RegionPreview, TextKind, Workspace};
use sharex_ui::{Palette, widgets};

pub use crate::region::paint::Mapper;
use crate::region::paint::{dashed_axis_line, dashed_poly, label_size, paint_label, paint_node};

/// Where the chrome put what can be clicked, in points.
#[derive(Clone, Debug, Default)]
pub struct ChromeLayout {
    /// The crop's Crop button.
    pub crop_button: Option<Rect>,
}

fn color(hex: &str) -> Color32 {
    let c = sharex_image_editor::render::parse_color(hex);
    Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a)
}

/// Draws the editor's overlay for `e`.
pub fn paint(ui: &egui::Ui, painter: &egui::Painter, p: &Palette, m: &Mapper, e: &Workspace, crop_button_hovered: bool) -> ChromeLayout {
    let mut layout = ChromeLayout::default();
    paint_region_preview(painter, m, e);
    paint_outline(painter, m, e);
    let (handles, line) = e.handles();
    if let Some((a, b)) = line {
        if (a.0 - b.0).abs() < 1e-9 || (a.1 - b.1).abs() < 1e-9 {
            dashed_axis_line(painter, m, a, b, 0.0);
        } else {
            dashed_poly(painter, &[m.pos(a.0, a.1), m.pos(b.0, b.1)]);
        }
    }
    for h in handles {
        paint_node(painter, m, h.at);
    }
    // The text box (ZIndex 9999) covers the handles.
    if let Some(t) = e.text_edit()
        && let Some(b) = ed::text_box(t)
    {
        paint_text_box(painter, p, m, &b, &t.text);
    }
    if let Some(c) = e.crop_chrome() {
        layout.crop_button = paint_crop(ui, painter, p, m, e, &c, crop_button_hovered);
    }
    layout
}

/// `UpdateHoverOutline`: black and white 1px dashes, 3 on 3.
fn paint_outline(painter: &egui::Painter, m: &Mapper, e: &Workspace) {
    match e.outline() {
        Some(Outline::Rectangle { rect: (l, t, w, h), angle: 0.0, .. }) => {
            // `Rectangle` with a 1px stroke: its path half a pixel in,
            // clockwise from the top-left corner.
            let (x0, y0, x1, y1) = (l + 0.5, t + 0.5, l + w - 0.5, t + h - 0.5);
            let mut d = 0.0;
            for (a, b) in [((x0, y0), (x1, y0)), ((x1, y0), (x1, y1)), ((x1, y1), (x0, y1)), ((x0, y1), (x0, y0))] {
                dashed_axis_line(painter, m, a, b, d);
                d += (b.0 - a.0).abs() + (b.1 - a.1).abs();
            }
        }
        Some(Outline::Rectangle { rect: (l, t, w, h), angle, center }) => {
            let corners = [(l, t), (l + w, t), (l + w, t + h), (l, t + h), (l, t)];
            let pts: Vec<Pos2> =
                corners.iter().map(|c| ed::geometry::rotate_point(*c, center, angle)).map(|c| m.pos(c.0, c.1)).collect();
            dashed_poly(painter, &pts);
        }
        Some(Outline::Ellipse { rect: (l, t, w, h) }) => {
            let (cx, cy, rx, ry) = (l + w / 2.0, t + h / 2.0, (w - 1.0) / 2.0, (h - 1.0) / 2.0);
            let pts: Vec<Pos2> = (0..=96)
                .map(|i| {
                    let a = std::f64::consts::TAU * i as f64 / 96.0 - std::f64::consts::FRAC_PI_2;
                    m.pos(cx + rx * a.cos(), cy + ry * a.sin())
                })
                .collect();
            dashed_poly(painter, &pts);
        }
        Some(Outline::Polyline(points)) => {
            let pts: Vec<Pos2> = points.iter().map(|p| m.pos(p.0, p.1)).collect();
            dashed_poly(painter, &pts);
        }
        None => {}
    }
}

/// The `TextBox` (ShareX's style): the theme's control background; the
/// creation and edit boxes with a border and 4px corners, the balloon's
/// square and borderless; the text in its colour and the caret.
fn paint_text_box(painter: &egui::Painter, p: &Palette, m: &Mapper, b: &ed::TextBox, text: &str) {
    let (l, t, w, h) = b.rect;
    let rect = m.rect(l, t, w, h);
    if b.kind == TextKind::Balloon {
        painter.rect_filled(rect, 0.0, p.control_background);
    } else {
        let r = CornerRadius::same((4.0 * m.scale as f32).round() as u8);
        painter.rect_filled(rect, r, p.control_background);
        painter.rect_stroke(rect, r, Stroke::new(2.0 / m.ppp, p.border), StrokeKind::Inside);
    }
    let fg = color(&b.color);
    let weight = if b.bold { 700 } else { 400 };
    let size = (b.font_size * m.scale) as f32 / m.ppp;
    let font = egui::FontId::new(size.max(1.0), widgets::font(12.0, weight).family);
    let clip = painter.with_clip_rect(rect.intersect(painter.clip_rect()));
    for (line, x, y) in &b.lines {
        let g = painter.layout_no_wrap(line.clone(), font.clone(), fg);
        clip.galley(m.pos(*x, *y), g, fg);
    }
    let _ = text;
    clip.rect_filled(m.rect(b.caret.0, b.caret.1, 1.0 / m.scale.max(1e-6), b.line_height), 0.0, fg);
}

/// `CutOutOverlay` and `CloneOverlay`.
fn paint_region_preview(painter: &egui::Painter, m: &Mapper, e: &Workspace) {
    let (rect, fill, stroke) = match e.region_preview() {
        Some(RegionPreview::CutOut { rect }) => (rect, Color32::from_rgba_unmultiplied(0, 0, 0, 150), Color32::WHITE),
        Some(RegionPreview::Clone { rect }) => {
            (rect, Color32::from_rgba_unmultiplied(0, 120, 215, 40), Color32::from_rgb(0x1E, 0x90, 0xFF))
        }
        None => return,
    };
    let (l, t, w, h) = rect;
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    painter.rect_filled(m.rect(l, t, w, h), 0.0, fill);
    let pts = [(l + 0.5, t + 0.5), (l + w - 0.5, t + 0.5), (l + w - 0.5, t + h - 0.5), (l + 0.5, t + h - 0.5), (l + 0.5, t + 0.5)];
    let pts: Vec<Pos2> = pts.iter().map(|c| m.pos(c.0, c.1)).collect();
    let s = m.scale as f32;
    painter.extend(egui::Shape::dashed_line(&pts, Stroke::new(s / m.ppp, stroke), 5.0 * s / m.ppp, 3.0 * s / m.ppp));
}

/// The crop: shades, the dashed rectangle, the thirds, the size label, the
/// handles and the Crop button. Returns the button's rectangle.
fn paint_crop(
    ui: &egui::Ui,
    painter: &egui::Painter,
    p: &Palette,
    m: &Mapper,
    e: &Workspace,
    c: &ed::CropChrome,
    button_hovered: bool,
) -> Option<Rect> {
    let s = m.scale as f32;
    for (l, t, w, h) in &c.shades {
        painter.rect_filled(m.rect(*l, *t, *w, *h), 0.0, Color32::from_rgba_unmultiplied(0, 0, 0, 140));
    }
    let (l, t, w, h) = c.rect;
    if w > 0.0 && h > 0.0 {
        // A 2px white stroke centred on the rectangle's edge (`4,2` dashes).
        let pts = [(l, t), (l + w, t), (l + w, t + h), (l, t + h), (l, t)];
        let pts: Vec<Pos2> = pts.iter().map(|q| m.pos(q.0, q.1)).collect();
        painter.extend(egui::Shape::dashed_line(&pts, Stroke::new(2.0 * s / m.ppp, Color32::WHITE), 8.0 * s / m.ppp, 4.0 * s / m.ppp));
    }
    let guide = Color32::from_rgba_unmultiplied(255, 255, 255, 210);
    for (a, b) in &c.guides {
        painter.extend(egui::Shape::dashed_line(
            &[m.pos(a.0, a.1), m.pos(b.0, b.1)],
            Stroke::new(s / m.ppp, guide),
            3.0 * s / m.ppp,
            3.0 * s / m.ppp,
        ));
    }
    // The label: above the top-left corner, or inside it near the top
    // edge; it doesn't scale with the zoom.
    let mut info_above = 0.0;
    if let Some(info) = &c.info {
        let size = label_size(ui, info);
        let margin = 6.0;
        let top_px = m.pos(l, t);
        let above = (t * m.scale) as f32 / m.ppp >= size.y + margin;
        let at = if above { pos2(top_px.x, top_px.y - size.y - margin) } else { pos2(top_px.x + margin, top_px.y + margin) };
        if above {
            info_above = size.y + margin;
        }
        paint_label(ui, p, Rect::from_min_size(at, size), info, false);
    }
    if !c.pending {
        return None;
    }
    for (kind, at) in &c.handles {
        paint_crop_handle(painter, m, *kind, *at);
    }
    // The Crop button: 80x40, centred below the crop, or above it (and the
    // label) when it would leave the image.
    let (bw, bh) = (80.0 * s / m.ppp, 40.0 * s / m.ppp);
    let margin = 10.0 * s / m.ppp;
    let (_, ih) = e.size();
    let below = m.pos(l, t + h).y + margin;
    let image_bottom = m.pos(0.0, ih as f64).y;
    let y = if below + bh <= image_bottom { below } else { m.pos(l, t).y - margin - bh - info_above };
    let x = m.pos(l + w / 2.0, t).x - bw / 2.0;
    let rect = Rect::from_min_size(pos2(x, y), egui::vec2(bw, bh));
    let r = CornerRadius::same(4);
    painter.rect_filled(rect, r, if button_hovered { p.control_background_hover } else { p.background_popup });
    painter.rect_stroke(rect, r, Stroke::new(1.0, p.border), StrokeKind::Inside);
    let font = egui::FontId::new(14.0, sharex_ui::fonts::inter(400));
    let text = sharex_strings::image_editor::EditorView_Crop();
    let tw = widgets::text_width(ui, text, font.clone());
    let g = widgets::galley(ui, text, font.clone(), p.text, f32::INFINITY);
    let ty = widgets::center_y(rect, widgets::line_height(ui, &font));
    painter.galley(pos2(rect.center().x - tw / 2.0, ty), g, p.text);
    Some(rect)
}

/// `CreateResizeHandle`'s visuals: a white 5px L with round caps at the
/// corners, a 30x5 rounded bar on the edges.
fn paint_crop_handle(painter: &egui::Painter, m: &Mapper, kind: CropHandle, (x, y): (f64, f64)) {
    let white = Color32::WHITE;
    let s = m.scale as f32;
    let thick = (ed::CROP_HANDLE_THICKNESS as f32) * s / m.ppp;
    if kind.is_corner() {
        let arm = ed::CROP_CORNER_ARM;
        let hx = if matches!(kind, CropHandle::TopRight | CropHandle::BottomRight) { -arm } else { arm };
        let vy = if matches!(kind, CropHandle::BottomLeft | CropHandle::BottomRight) { -arm } else { arm };
        let stroke = Stroke::new(thick, white);
        let pts = vec![m.pos(x + hx, y), m.pos(x, y), m.pos(x, y + vy)];
        painter.add(egui::Shape::line(pts.clone(), stroke));
        for q in [pts[0], pts[1], pts[2]] {
            painter.circle_filled(q, thick / 2.0, white);
        }
        return;
    }
    let (bw, bh) = if kind.is_horizontal_edge() {
        (ed::CROP_CENTER_BAR, ed::CROP_HANDLE_THICKNESS)
    } else {
        (ed::CROP_HANDLE_THICKNESS, ed::CROP_CENTER_BAR)
    };
    let r = m.rect(x - bw / 2.0, y - bh / 2.0, bw, bh);
    painter.rect_filled(r, CornerRadius::same((thick / 2.0).round() as u8), white);
}

/// The crop handle or Crop button under `pos` (points), for the cursor.
pub fn crop_handle_at(m: &Mapper, e: &Workspace, pos: Pos2) -> Option<CropHandle> {
    let c = e.crop_chrome()?;
    c.handles.iter().rev().find_map(|(k, at)| {
        let (w, h) = k.hit_size();
        let r = m.rect(at.0 - w / 2.0, at.1 - h / 2.0, w, h);
        r.contains(pos).then_some(*k)
    })
}
