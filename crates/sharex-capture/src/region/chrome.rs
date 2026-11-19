//! `RegionSelectionOverlay.Render` beyond a single rectangle: dimming
//! around several regions (`GetUncoveredRectangles`, or the screen minus
//! every region's geometry when one isn't a rectangle), the ellipse and
//! freehand outlines (`DrawAntRegion`: a 1px accent pen, then `[5, 5]`
//! dashes in the halo colour at 230/255), and the screen-wide crosshair
//! (`DrawCursorCrosshair`). Non-rectangular parts are rasterised with
//! Skia's CPU renderer ([`sharex_skia`]) into a picture over their bounds.

use egui::{Color32, TextureHandle, TextureOptions, pos2};
use sharex_skia::{Cap, Color, IRect, Join, Matrix, Path, Pixmap, Stroke};

use super::paint::{Mapper, pixel_rect};
use super::shape::{CaptureRegion, RegionShape};
use crate::geometry::RectF;

/// `GetUncoveredRectangles`: the part of `surface` none of `covered`
/// overlap, split into rectangles (column by column) so overlapping
/// regions are never dimmed twice.
pub fn uncovered_rectangles(surface: RectF, covered: &[RectF]) -> Vec<RectF> {
    let mut result = Vec::new();
    let mut regions = Vec::new();
    let mut columns = vec![surface.x, surface.right()];
    for r in covered {
        let c = r.intersect(&surface);
        if c.is_valid() {
            regions.push(c);
            columns.push(c.x);
            columns.push(c.right());
        }
    }
    if regions.is_empty() {
        result.push(surface);
        return result;
    }
    // `SortedSet<double>`.
    columns.sort_by(f64::total_cmp);
    columns.dedup();
    for w in columns.windows(2) {
        let (left, right) = (w[0], w[1]);
        if right <= left {
            continue;
        }
        let mut spans: Vec<(f64, f64)> =
            regions.iter().filter(|r| r.x <= left && r.right() >= right).map(|r| (r.y, r.bottom())).collect();
        // `List.Sort` by top (an unstable sort; equal tops don't matter).
        spans.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut y = surface.y;
        for (top, bottom) in spans {
            if top > y {
                result.push(RectF::new(left, y, right - left, top - y));
            }
            y = y.max(bottom);
        }
        if surface.bottom() > y {
            result.push(RectF::new(left, y, right - left, surface.bottom() - y));
        }
    }
    result
}

/// `GetAntRectangle`: half a pixel in, so 1px lines cover whole pixels.
fn ant_rectangle(r: RectF) -> RectF {
    RectF::new(r.x + 0.5, r.y + 0.5, (r.w - 1.0).max(0.0), (r.h - 1.0).max(0.0))
}

/// The outline `DrawAntRegion` strokes for an ellipse or freehand region.
fn ant_geometry(region: &CaptureRegion) -> Option<Path> {
    match region.shape {
        RegionShape::Ellipse => CaptureRegion::new(RegionShape::Ellipse, ant_rectangle(region.bounds)).geometry(),
        RegionShape::Freehand => region.offset(0.5, 0.5).geometry(),
        RegionShape::Rectangle => None,
    }
}

/// What the picture of the non-rectangular chrome shows.
#[derive(Clone, Debug, PartialEq)]
pub struct ShapeChrome {
    /// The picture's place in image pixels (whole pixels).
    pub area: crate::geometry::Rect,
    /// Dims the area outside these geometries (the `CombinedGeometry`
    /// exclusion), with this alpha; `None` when dimming is drawn as
    /// rectangles instead.
    pub dim: Option<(u8, Vec<CaptureRegion>)>,
    /// Outlined with `DrawAntRegion`, in order.
    pub outlines: Vec<CaptureRegion>,
    pub accent: Color32,
    pub halo: Color32,
}

impl ShapeChrome {
    /// Rasterises the picture: premultiplied RGBA of `area`'s size.
    pub fn render(&self) -> Pixmap {
        let a = self.area;
        let mut pm = Pixmap::new(a.w.max(1) as u32, a.h.max(1) as u32);
        let to_local = Matrix { sx: 1.0, kx: 0.0, tx: -a.x as f32, ky: 0.0, sy: 1.0, ty: -a.y as f32 };
        if let Some((alpha, clear)) = &self.dim {
            // The holes' coverage, then the dim colour where they don't
            // cover (`DrawGeometry(dimBrush, CombinedGeometry(Exclude))`).
            let mut mask = Pixmap::new(pm.width(), pm.height());
            for region in clear {
                if region.bounds.is_valid()
                    && let Some(g) = region.geometry()
                {
                    sharex_skia::draw::fill_path(&mut mask, &g.transformed(&to_local), Color::argb(255, 255, 255, 255), None);
                }
            }
            for (d, m) in pm.pixels_mut().iter_mut().zip(mask.pixels()) {
                let cover = 255 - (m >> 24);
                let a = (*alpha as u32 * cover + 127) / 255;
                *d = a << 24;
            }
        }
        let accent = Color::argb(255, self.accent.r(), self.accent.g(), self.accent.b());
        let dash = Color::argb(230, self.halo.r(), self.halo.g(), self.halo.b());
        let solid = Stroke { width: 1.0, cap: Cap::Butt, join: Join::Miter, miter_limit: 10.0, dash: None };
        let dashed = Stroke { dash: Some((vec![5.0, 5.0], 0.0)), ..solid.clone() };
        let clip = Some(IRect::from_ltrb(0, 0, pm.width() as i32, pm.height() as i32));
        for region in &self.outlines {
            if let Some(g) = ant_geometry(region) {
                sharex_skia::draw::stroke_path(&mut pm, &g, &solid, &to_local, accent, clip);
                sharex_skia::draw::stroke_path(&mut pm, &g, &dashed, &to_local, dash, clip);
            }
        }
        pm
    }

    /// Draws the picture, uploading it when it changed (`cache`).
    pub fn paint(&self, painter: &egui::Painter, m: &Mapper, cache: &mut Option<(TextureHandle, ShapeChrome)>) {
        if !self.area.is_valid() {
            return;
        }
        if cache.as_ref().is_none_or(|(_, k)| k != self) {
            let pm = self.render();
            let image = egui::ColorImage::from_rgba_premultiplied([pm.width() as usize, pm.height() as usize], &pm.to_premul_rgba());
            match cache {
                Some((tex, key)) => {
                    tex.set(image, TextureOptions::NEAREST);
                    *key = self.clone();
                }
                None => *cache = Some((painter.ctx().load_texture("region shapes", image, TextureOptions::NEAREST), self.clone())),
            }
        }
        if let Some((tex, _)) = cache {
            let a = self.area;
            let r = m.rect(a.x as f64, a.y as f64, a.w as f64, a.h as f64);
            painter.image(tex.id(), r, egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        }
    }
}

/// `DrawAntLineIfVisible` for an axis-aligned line through pixel centres:
/// the 1px accent line, then `[5, 5]` halo dashes at 230/255 from `a`,
/// each pixel as much as the line and its dashes cover it.
pub fn ant_axis_line(painter: &egui::Painter, m: &Mapper, a: (f64, f64), b: (f64, f64), accent: Color32, halo: Color32) {
    if b.0 <= a.0 && b.1 <= a.1 {
        return;
    }
    let vertical = (a.0 - b.0).abs() < 1e-9;
    let (s, e) = if vertical { (a.1, b.1) } else { (a.0, b.0) };
    let across = if vertical { a.0 } else { a.1 }.floor();
    let rect = |p0: i64, n: i64| {
        if vertical { m.rect(across, p0 as f64, 1.0, n as f64) } else { m.rect(p0 as f64, across, n as f64, 1.0) }
    };
    // Runs of pixels with the same coverage.
    let runs = |cover: &dyn Fn(f64, f64) -> f64| -> Vec<(i64, i64, f64)> {
        let mut out: Vec<(i64, i64, f64)> = Vec::new();
        for px in s.floor() as i64..e.ceil() as i64 {
            let (p0, p1) = ((px as f64).max(s), (px as f64 + 1.0).min(e));
            if p1 <= p0 {
                continue;
            }
            let c = cover(p0 - s, p1 - s).clamp(0.0, 1.0);
            if c <= 0.0 {
                continue;
            }
            match out.last_mut() {
                Some((_, last, cov)) if *last + 1 == px && (*cov - c).abs() < 1e-9 => *last = px,
                _ => out.push((px, px, c)),
            }
        }
        out
    };
    for (p0, p1, c) in runs(&|d0, d1| d1 - d0) {
        let color = Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), (255.0 * c).round() as u8);
        pixel_rect(painter, rect(p0, p1 - p0 + 1), color);
    }
    let dash_cover = |d0: f64, d1: f64| {
        let mut cover = 0.0;
        let first = (d0 / 10.0).floor() as i64;
        for k in first..=first + 1 {
            let (ds, de) = ((k * 10) as f64, (k * 10 + 5) as f64);
            cover += (de.min(d1) - ds.max(d0)).max(0.0);
        }
        cover
    };
    for (p0, p1, c) in runs(&dash_cover) {
        let color = Color32::from_rgba_unmultiplied(halo.r(), halo.g(), halo.b(), (230.0 * c).round() as u8);
        pixel_rect(painter, rect(p0, p1 - p0 + 1), color);
    }
}

/// `DrawCursorCrosshair`: four ant lines from the screen's edges to 5px
/// short of the pointer's pixel.
pub fn cursor_crosshair(painter: &egui::Painter, m: &Mapper, surface: RectF, cursor: (f64, f64), accent: Color32, halo: Color32) {
    const GAP: f64 = 5.0;
    let x = cursor.0.floor().clamp(surface.x, surface.x.max(surface.right() - 1.0)) + 0.5;
    let y = cursor.1.floor().clamp(surface.y, surface.y.max(surface.bottom() - 1.0)) + 0.5;
    ant_axis_line(painter, m, (surface.x, y), (x - GAP, y), accent, halo);
    ant_axis_line(painter, m, (x + GAP, y), (surface.right(), y), accent, halo);
    ant_axis_line(painter, m, (x, surface.y), (x, y - GAP), accent, halo);
    ant_axis_line(painter, m, (x, y + GAP), (x, surface.bottom()), accent, halo);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uncovered_rectangles_leave_overlaps_once() {
        let surface = RectF::new(0.0, 0.0, 10.0, 10.0);
        let rects = uncovered_rectangles(surface, &[RectF::new(2.0, 2.0, 4.0, 4.0), RectF::new(4.0, 4.0, 4.0, 4.0)]);
        let area: f64 = rects.iter().map(|r| r.w * r.h).sum();
        // 100 - (16 + 16 - 4).
        assert_eq!(area, 72.0);
        assert_eq!(uncovered_rectangles(surface, &[]), vec![surface]);
    }

    #[test]
    fn the_ellipse_hole_is_undimmed_inside_and_dimmed_outside() {
        let e = CaptureRegion::new(RegionShape::Ellipse, RectF::new(10.0, 10.0, 40.0, 30.0));
        let chrome = ShapeChrome {
            area: crate::geometry::Rect::new(0, 0, 60, 50),
            dim: Some((51, vec![e.clone()])),
            outlines: vec![e],
            accent: Color32::from_rgb(9, 170, 255),
            halo: Color32::BLACK,
        };
        let pm = chrome.render();
        assert_eq!(pm.pixel(30, 25) >> 24, 0);
        assert_eq!(pm.pixel(2, 2) >> 24, 51);
        // The outline at the top of the ellipse.
        assert!(pm.pixel(30, 10) >> 24 > 51);
    }
}
