//! Avalonia's `Border`, drawn the way Skia draws it, for every bordered
//! control.
//!
//! Avalonia (`BorderRenderHelper`, uniform thickness) doesn't fill a box and
//! then stroke inside it: it deflates the bounds by half the border
//! thickness (the "CenterBorder" geometry), keeps the nominal `CornerRadius`
//! on that path (Skia's `SkRRect` scales radii down only when they don't
//! fit), fills the path with the background, and strokes the same path
//! centred with the border pen. So the background reaches the middle of the
//! border, a 1px border on a whole-pixel box covers two half pixels at the
//! corners, and a nominal 4px corner shows about 4.5px of outer coverage.
//! egui's own fill and stroke tessellate separately, each with its own
//! feathering, which neither layers nor antialiases the same.
//!
//! [`paint`] rasterises a stack of [`Part`]s on the CPU with exact area
//! coverage (exact across, finely sampled down), composites them as Skia
//! does (each part source-over the ones before it, in sRGB values), and
//! draws the result as one premultiplied texture, optionally at a group
//! opacity (Avalonia's `Opacity` on a presenter applies to its whole
//! subtree at once). Textures are cached by their parts and dropped once
//! unused for a while.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{Arc, Mutex};

use egui::{Color32, ColorImage, Context, Id, Painter, Rect, TextureOptions, pos2, vec2};

use crate::tiled::TiledTexture;
use crate::theme::palette;

/// A background brush.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Brush {
    Solid(Color32),
    /// A `LinearGradientBrush` from 0%,0% to 100%,100% of the shape it
    /// fills (`ShareX.Brush.Accent`): Skia interpolates the sRGB values at
    /// each pixel's centre.
    Diagonal(Color32, Color32),
    /// `ShareX.Brush.Checkered.Small`: a tiled `DrawingBrush` of `cell`
    /// squares, `colors[1]` in the squares where (column + row) is even
    /// (the tile's top-left and bottom-right), from the shape's top-left.
    Checkered { colors: [Color32; 2], cell: f32 },
}

impl Brush {
    /// `ShareX.Brush.Accent`, the accent gradient.
    pub fn accent(ctx: &Context) -> Self {
        let p = palette(ctx);
        Brush::Diagonal(p.accent_start, p.accent_end)
    }
}

impl From<Color32> for Brush {
    fn from(c: Color32) -> Self {
        Brush::Solid(c)
    }
}

/// One layer of a stack drawn by [`paint`]. Rects are in points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Part {
    /// A `Border` with a uniform `thickness` and `radius`: `background` up
    /// to the middle of the border, then `border` stroked centred on it. A
    /// thickness with no border brush (a transparent `BorderBrush`) still
    /// deflates the background.
    Border { rect: Rect, radius: f32, thickness: f32, background: Option<Brush>, border: Option<Color32> },
    /// An inset `BoxShadow` with no blur or offset (Avalonia's Skia
    /// renderer): `color` between `rect` (with `radius`) and `rect` deflated
    /// by `spread` (its radius reduced as much), clipped to `rect`.
    InsetShadow { rect: Rect, radius: f32, spread: f32, color: Color32 },
    /// `DrawingContext.DrawRectangle(null, pen, rect, radius, radius)`: the
    /// rounded rectangle `rect` itself (not deflated) stroked centred with
    /// a `thickness` pen of `brush`, the brush relative to `rect`.
    Stroke { rect: Rect, radius: f32, thickness: f32, brush: Brush },
    /// `color` in `rect` outside the rounded rectangle `rect` with
    /// `radius`: what a `PushClip(RoundedRect)` leaves of the content
    /// beneath, drawn over content to clip its corners.
    Outside { rect: Rect, radius: f32, color: Color32 },
}

impl Part {
    fn rect(&self) -> Rect {
        match *self {
            Part::Border { rect, .. } | Part::InsetShadow { rect, .. } | Part::Outside { rect, .. } => rect,
            Part::Stroke { rect, thickness, .. } => rect.expand(thickness / 2.0),
        }
    }
}

/// Draws one `Border` in `rect`.
pub fn border(painter: &Painter, rect: Rect, radius: f32, thickness: f32, background: Option<Brush>, border: Option<Color32>) {
    paint(painter, &[Part::Border { rect, radius, thickness, background, border }], 1.0);
}

/// `DrawRectangle(null, new Pen(brush, thickness), rect, radius, radius)`.
pub fn stroke(painter: &Painter, rect: Rect, radius: f32, thickness: f32, brush: Brush) {
    paint(painter, &[Part::Stroke { rect, radius, thickness, brush }], 1.0);
}

/// `color` over the corners of `rect` that a rounded clip of `radius`
/// would hide (see [`Part::Outside`]).
pub fn clip_corners(painter: &Painter, rect: Rect, radius: f32, color: Color32) {
    let r = radius.ceil() + 1.0;
    for (x, y) in [(rect.min.x, rect.min.y), (rect.max.x - r, rect.min.y), (rect.min.x, rect.max.y - r), (rect.max.x - r, rect.max.y - r)] {
        let corner = Rect::from_min_size(pos2(x, y), vec2(r, r));
        // The shape is the whole rectangle; only this corner's pixels are drawn.
        let clip = painter.with_clip_rect(corner.intersect(painter.clip_rect()));
        paint(&clip, &[Part::Outside { rect, radius, color }], 1.0);
    }
}

/// An ellipse filled with `color` in `rect` (`Ellipse`, as Skia draws an
/// oval: the same coverage as a rounded rectangle with half-size radii).
pub fn ellipse(painter: &Painter, rect: Rect, color: Color32) {
    let radius = rect.width().min(rect.height()) / 2.0;
    border(painter, rect, radius, 0.0, Some(Brush::Solid(color)), None);
}

/// A filled polygon (a `Path` of straight segments, such as a `PathIcon`'s
/// chevron) at `points`, antialiased as Skia's GPU backend does: each
/// pixel covered by half a pixel minus the signed distance from its centre
/// to the outline (even-odd inside).
pub fn polygon(painter: &Painter, points: &[egui::Pos2], color: Color32) {
    if points.len() < 3 {
        return;
    }
    let ctx = painter.ctx();
    let ppp = ctx.pixels_per_point();
    let bounds = Rect::from_points(points);
    let min = pos2((bounds.min.x * ppp).floor() - 1.0, (bounds.min.y * ppp).floor() - 1.0);
    let max = pos2((bounds.max.x * ppp).ceil() + 1.0, (bounds.max.y * ppp).ceil() + 1.0);
    let (w, h) = ((max.x - min.x) as usize, (max.y - min.y) as usize);
    let local: Vec<egui::Pos2> = points.iter().map(|p| pos2(p.x * ppp - min.x, p.y * ppp - min.y)).collect();
    let mut hasher = DefaultHasher::new();
    ("polygon", w, h).hash(&mut hasher);
    for p in &local {
        (p.x.to_bits(), p.y.to_bits()).hash(&mut hasher);
    }
    let tex = cached(ctx, hasher.finish(), || {
        let pixels = (0..w * h)
            .map(|i| {
                let p = pos2((i % w) as f32 + 0.5, (i / w) as f32 + 0.5);
                let c = (0.5 - signed_distance(&local, p)).clamp(0.0, 1.0);
                let v = (c * 255.0).round() as u8;
                Color32::from_rgba_premultiplied(v, v, v, v)
            })
            .collect();
        ColorImage { size: [w, h], pixels, source_size: vec2(w as f32, h as f32) }
    });
    let rect = Rect::from_min_max(pos2(min.x / ppp, min.y / ppp), pos2(max.x / ppp, max.y / ppp));
    tex.paint(painter, rect, color);
}

/// The distance from `p` to the polygon's outline, negative inside.
fn signed_distance(poly: &[egui::Pos2], p: egui::Pos2) -> f32 {
    let mut d = f32::INFINITY;
    let mut inside = false;
    for (i, &a) in poly.iter().enumerate() {
        let b = poly[(i + 1) % poly.len()];
        let ab = b - a;
        let t = ((p - a).dot(ab) / ab.length_sq().max(f32::EPSILON)).clamp(0.0, 1.0);
        d = d.min((p - (a + ab * t)).length());
        if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) / (b.y - a.y) * ab.x {
            inside = !inside;
        }
    }
    if inside { -d } else { d }
}

/// Draws `parts`, composited in order, as one layer at `opacity`.
pub fn paint(painter: &Painter, parts: &[Part], opacity: f32) {
    if parts.is_empty() || opacity <= 0.0 {
        return;
    }
    let ctx = painter.ctx();
    let ppp = ctx.pixels_per_point();
    let bounds = parts.iter().fold(Rect::NOTHING, |b, p| b.union(p.rect()));
    // The texture covers every physical pixel the parts touch.
    let min = pos2((bounds.min.x * ppp).floor(), (bounds.min.y * ppp).floor());
    let max = pos2((bounds.max.x * ppp).ceil(), (bounds.max.y * ppp).ceil());
    let (w, h) = ((max.x - min.x) as usize, (max.y - min.y) as usize);
    if w == 0 || h == 0 {
        return;
    }
    // Parts in physical pixels from the texture's top-left.
    let local: Vec<Part> = parts.iter().map(|p| to_pixels(*p, ppp, min)).collect();
    let mut hasher = DefaultHasher::new();
    (w, h).hash(&mut hasher);
    for p in &local {
        hash_part(p, &mut hasher);
    }
    let key = hasher.finish();
    let tex = cached(ctx, key, || rasterize(&local, w, h));
    let rect = Rect::from_min_max(pos2(min.x / ppp, min.y / ppp), pos2(max.x / ppp, max.y / ppp));
    let tint = if opacity >= 1.0 { Color32::WHITE } else { Color32::WHITE.gamma_multiply(opacity) };
    tex.paint(painter, rect, tint);
}

/// The colour to draw content (text, a glyph: premultiplied `content`,
/// antialiased by coverage) with so that, over an opaque `under` already
/// drawn in a [`paint`] layer at `opacity`, it composites as if it were in
/// that same layer: Avalonia's group opacity on a presenter whose content
/// overlaps its own background. Exact where the content is lighter than
/// `under` in every channel (drawn additively); otherwise the darkening
/// assumes the layer sits on `behind`, so it's exact on that background.
pub fn group_content(content: Color32, under: Color32, opacity: f32, behind: Color32) -> Color32 {
    let c = content.to_array().map(|v| v as f32 / 255.0);
    let u = under.to_array().map(|v| v as f32 / 255.0);
    let b = behind.to_array().map(|v| v as f32 / 255.0);
    let a = c[3];
    // What the pixel needs to gain: opacity * (content over under - under).
    let delta: [f32; 3] = std::array::from_fn(|i| opacity * (c[i] - a * u[i]));
    // The pixel before the content: the layer over `behind`.
    let dst: [f32; 3] = std::array::from_fn(|i| b[i] * (1.0 - opacity) + u[i] * opacity);
    // out = dst * (1 - s) + rgb: the smallest `s` that keeps rgb >= 0.
    let s = (0..3).map(|i| if delta[i] < 0.0 && dst[i] > 0.0 { -delta[i] / dst[i] } else { 0.0 }).fold(0.0f32, f32::max).min(1.0);
    let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color32::from_rgba_premultiplied(q(delta[0] + s * dst[0]), q(delta[1] + s * dst[1]), q(delta[2] + s * dst[2]), q(s))
}

fn to_pixels(p: Part, ppp: f32, origin: egui::Pos2) -> Part {
    let r = |rect: Rect| Rect::from_min_max(pos2(rect.min.x * ppp - origin.x, rect.min.y * ppp - origin.y), pos2(rect.max.x * ppp - origin.x, rect.max.y * ppp - origin.y));
    match p {
        Part::Border { rect, radius, thickness, background, border } => {
            let background = background.map(|b| match b {
                Brush::Checkered { colors, cell } => Brush::Checkered { colors, cell: cell * ppp },
                other => other,
            });
            Part::Border { rect: r(rect), radius: radius * ppp, thickness: thickness * ppp, background, border }
        }
        Part::InsetShadow { rect, radius, spread, color } => Part::InsetShadow { rect: r(rect), radius: radius * ppp, spread: spread * ppp, color },
        Part::Stroke { rect, radius, thickness, brush } => Part::Stroke { rect: r(rect), radius: radius * ppp, thickness: thickness * ppp, brush },
        Part::Outside { rect, radius, color } => Part::Outside { rect: r(rect), radius: radius * ppp, color },
    }
}

fn hash_part(p: &Part, h: &mut DefaultHasher) {
    let f = |v: f32, h: &mut DefaultHasher| v.to_bits().hash(h);
    let rect = |r: Rect, h: &mut DefaultHasher| [r.min.x, r.min.y, r.max.x, r.max.y].iter().for_each(|v| f(*v, h));
    match *p {
        Part::Border { rect: r, radius, thickness, background, border } => {
            0u8.hash(h);
            rect(r, h);
            f(radius, h);
            f(thickness, h);
            match background {
                None => 0u8.hash(h),
                Some(Brush::Solid(c)) => (1u8, c).hash(h),
                Some(Brush::Diagonal(a, b)) => (2u8, a, b).hash(h),
                Some(Brush::Checkered { colors, cell }) => {
                    (3u8, colors).hash(h);
                    f(cell, h);
                }
            }
            border.hash(h);
        }
        Part::InsetShadow { rect: r, radius, spread, color } => {
            1u8.hash(h);
            rect(r, h);
            f(radius, h);
            f(spread, h);
            color.hash(h);
        }
        Part::Stroke { rect: r, radius, thickness, brush } => {
            2u8.hash(h);
            rect(r, h);
            f(radius, h);
            f(thickness, h);
            match brush {
                Brush::Solid(c) => (1u8, c).hash(h),
                Brush::Diagonal(a, b) => (2u8, a, b).hash(h),
                Brush::Checkered { colors, cell } => {
                    (3u8, colors).hash(h);
                    f(cell, h);
                }
            }
        }
        Part::Outside { rect: r, radius, color } => {
            3u8.hash(h);
            rect(r, h);
            f(radius, h);
            color.hash(h);
        }
    }
}

/// A rounded rectangle in pixels, its radius already fitted as `SkRRect`
/// fits it (no larger than half the shorter side).
#[derive(Clone, Copy, Debug)]
struct RRect {
    l: f32,
    t: f32,
    r: f32,
    b: f32,
    rad: f32,
}

impl RRect {
    fn new(rect: Rect, radius: f32) -> Self {
        let rad = radius.max(0.0).min(rect.width() / 2.0).min(rect.height() / 2.0).max(0.0);
        Self { l: rect.min.x, t: rect.min.y, r: rect.max.x, b: rect.max.y, rad }
    }

    /// `SkRRect::outset` / `inset`: the rectangle grown by `d` (shrunk if
    /// negative), each radius with it, none below 0.
    fn outset(&self, d: f32, radius: f32) -> Self {
        let rect = Rect::from_min_max(pos2(self.l - d, self.t - d), pos2(self.r + d, self.b + d));
        if rect.width() <= 0.0 || rect.height() <= 0.0 {
            return Self { l: 0.0, t: 0.0, r: 0.0, b: 0.0, rad: 0.0 };
        }
        let rad = if radius > 0.0 { (radius + d).max(0.0) } else { 0.0 };
        Self::new(rect, rad)
    }

    /// The part of row `y` (a horizontal line) inside the shape.
    fn span(&self, y: f32) -> Option<(f32, f32)> {
        if y < self.t || y > self.b || self.r <= self.l {
            return None;
        }
        let dy = if y < self.t + self.rad {
            self.t + self.rad - y
        } else if y > self.b - self.rad {
            y - (self.b - self.rad)
        } else {
            0.0
        };
        let dx = if dy > 0.0 { self.rad - (self.rad * self.rad - dy * dy).max(0.0).sqrt() } else { 0.0 };
        Some((self.l + dx, self.r - dx))
    }

    /// How much of the pixel at (`x`, `y`) the shape covers, as Skia's GPU
    /// backend antialiases a rounded rectangle (what ShareX shows on
    /// Windows): half a pixel minus the signed distance from the pixel's
    /// centre to the edge.
    fn coverage(&self, x: f32, y: f32) -> f32 {
        if self.r <= self.l || self.b <= self.t {
            return 0.0;
        }
        let (px, py) = (x + 0.5, y + 0.5);
        let (cx, cy) = ((self.l + self.r) / 2.0, (self.t + self.b) / 2.0);
        let (hw, hh) = ((self.r - self.l) / 2.0 - self.rad, (self.b - self.t) / 2.0 - self.rad);
        let (qx, qy) = ((px - cx).abs() - hw, (py - cy).abs() - hh);
        let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
        let sd = outside + qx.max(qy).min(0.0) - self.rad;
        (0.5 - sd).clamp(0.0, 1.0)
    }

    /// The exact area of the pixel at (`x`, `y`) inside the shape, as
    /// Skia's CPU rasteriser approximates it.
    #[allow(dead_code)]
    fn area_coverage(&self, x: f32, y: f32) -> f32 {
        let ox = (x + 1.0).min(self.r) - x.max(self.l);
        let oy = (y + 1.0).min(self.b) - y.max(self.t);
        if ox <= 0.0 || oy <= 0.0 {
            return 0.0;
        }
        if y >= self.t + self.rad && y + 1.0 <= self.b - self.rad {
            return ox.min(1.0) * oy.min(1.0);
        }
        // Rows meeting a corner: exact across each of N lines.
        const N: usize = 32;
        let mut sum = 0.0;
        for k in 0..N {
            if let Some((a, b)) = self.span(y + (k as f32 + 0.5) / N as f32) {
                sum += ((x + 1.0).min(b) - x.max(a)).max(0.0);
            }
        }
        (sum / N as f32).min(1.0)
    }
}

fn unpremul(c: Color32) -> [f32; 4] {
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    [r as f32, g as f32, b as f32, a as f32 / 255.0]
}

/// The brush's colour (unpremultiplied sRGB 0..255, alpha 0..1) at the
/// centre of pixel (`x`, `y`), for a shape whose brush rect is `area`.
fn brush_at(brush: &Brush, area: Rect, x: f32, y: f32) -> [f32; 4] {
    match *brush {
        Brush::Solid(c) => unpremul(c),
        Brush::Diagonal(a, b) => {
            let (w, h) = (area.width(), area.height());
            let (px, py) = (x + 0.5 - area.min.x, y + 0.5 - area.min.y);
            let t = ((px * w + py * h) / (w * w + h * h).max(f32::EPSILON)).clamp(0.0, 1.0);
            let (a, b) = (unpremul(a), unpremul(b));
            std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t)
        }
        Brush::Checkered { colors, cell } => {
            let cx = ((x + 0.5 - area.min.x) / cell).floor() as i64;
            let cy = ((y + 0.5 - area.min.y) / cell).floor() as i64;
            unpremul(if (cx + cy).rem_euclid(2) == 0 { colors[1] } else { colors[0] })
        }
    }
}

/// Source-over of `color` (unpremultiplied) at `coverage` onto premultiplied
/// `dst` (0..255, alpha 0..1).
fn over(dst: &mut [f32; 4], color: [f32; 4], coverage: f32) {
    let a = color[3] * coverage;
    if a <= 0.0 {
        return;
    }
    for i in 0..3 {
        dst[i] = color[i] * a + dst[i] * (1.0 - a);
    }
    dst[3] = a + dst[3] * (1.0 - a);
}

fn rasterize(parts: &[Part], w: usize, h: usize) -> ColorImage {
    let mut px = vec![[0.0f32; 4]; w * h];
    for part in parts {
        match *part {
            Part::Border { rect, radius, thickness, background, border } => {
                // The nominal radius on the deflated path, fitted to it.
                let path_rect = rect.shrink(thickness / 2.0);
                if path_rect.width() <= 0.0 || path_rect.height() <= 0.0 {
                    continue;
                }
                let path = RRect::new(path_rect, radius);
                let stroke = (thickness > 0.0).then_some(()).and(border).map(|c| {
                    let outer = path.outset(thickness / 2.0, path.rad);
                    let inner = path.outset(-thickness / 2.0, path.rad);
                    (outer, inner, unpremul(c))
                });
                for y in 0..h {
                    for x in 0..w {
                        let (fx, fy) = (x as f32, y as f32);
                        let d = &mut px[y * w + x];
                        if let Some(bg) = &background {
                            let c = path.coverage(fx, fy);
                            if c > 0.0 {
                                over(d, brush_at(bg, path_rect, fx, fy), c);
                            }
                        }
                        if let Some((outer, inner, color)) = &stroke {
                            let c = outer.coverage(fx, fy) - inner.coverage(fx, fy);
                            if c > 0.0 {
                                over(d, *color, c);
                            }
                        }
                    }
                }
            }
            Part::Stroke { rect, radius, thickness, brush } => {
                let path = RRect::new(rect, radius);
                let outer = path.outset(thickness / 2.0, path.rad);
                let inner = path.outset(-thickness / 2.0, path.rad);
                for y in 0..h {
                    for x in 0..w {
                        let (fx, fy) = (x as f32, y as f32);
                        let c = outer.coverage(fx, fy) - inner.coverage(fx, fy);
                        if c > 0.0 {
                            over(&mut px[y * w + x], brush_at(&brush, rect, fx, fy), c);
                        }
                    }
                }
            }
            Part::Outside { rect, radius, color } => {
                let shape = RRect::new(rect, radius);
                let color = unpremul(color);
                let (x0, y0) = (rect.min.x.floor().max(0.0) as usize, rect.min.y.floor().max(0.0) as usize);
                let (x1, y1) = ((rect.max.x.ceil() as usize).min(w), (rect.max.y.ceil() as usize).min(h));
                for y in y0..y1 {
                    for x in x0..x1 {
                        let (fx, fy) = (x as f32, y as f32);
                        // Only the pixels the rectangle covers, less the shape.
                        let inside_rect = (((fx + 1.0).min(rect.max.x) - fx.max(rect.min.x)).max(0.0) * ((fy + 1.0).min(rect.max.y) - fy.max(rect.min.y)).max(0.0)).min(1.0);
                        let c = inside_rect - shape.coverage(fx, fy);
                        if c > 0.0 {
                            over(&mut px[y * w + x], color, c);
                        }
                    }
                }
            }
            Part::InsetShadow { rect, radius, spread, color } => {
                let clip = RRect::new(rect, radius);
                let hole = clip.outset(-spread, clip.rad);
                let color = unpremul(color);
                for y in 0..h {
                    for x in 0..w {
                        let (fx, fy) = (x as f32, y as f32);
                        let c = clip.coverage(fx, fy) * (1.0 - hole.coverage(fx, fy));
                        if c > 0.0 {
                            over(&mut px[y * w + x], color, c);
                        }
                    }
                }
            }
        }
    }
    let q = |v: f32| v.round().clamp(0.0, 255.0) as u8;
    let pixels = px.iter().map(|p| Color32::from_rgba_premultiplied(q(p[0]), q(p[1]), q(p[2]), q(p[3] * 255.0))).collect();
    ColorImage { size: [w, h], pixels, source_size: vec2(w as f32, h as f32) }
}

/// Unused textures live this many frames.
const KEEP_FRAMES: u64 = 600;

#[derive(Default)]
struct Cache {
    /// Tiled when a shape is larger than the GPU's largest texture.
    textures: HashMap<u64, (Arc<TiledTexture>, u64)>,
}

type SharedCache = Arc<Mutex<Cache>>;

fn cached(ctx: &Context, key: u64, make: impl FnOnce() -> ColorImage) -> Arc<TiledTexture> {
    let cache: SharedCache = ctx.data_mut(|d| d.get_temp_mut_or_default::<SharedCache>(Id::new("sharex-borders")).clone());
    let frame = ctx.cumulative_pass_nr();
    let mut cache = cache.lock().unwrap();
    if let Some((tex, used)) = cache.textures.get_mut(&key) {
        *used = frame;
        return tex.clone();
    }
    cache.textures.retain(|_, (_, used)| frame.saturating_sub(*used) < KEEP_FRAMES);
    let tex = Arc::new(TiledTexture::new(ctx, &format!("border:{key:016x}"), make(), TextureOptions::NEAREST));
    cache.textures.insert(key, (tex.clone(), frame));
    tex
}

/// Rasterises `parts` (in pixels) over an opaque `background`, for tests.
#[cfg(test)]
pub(crate) fn render_over(parts: &[Part], w: usize, h: usize, background: Color32) -> Vec<[u8; 3]> {
    let img = rasterize(parts, w, h);
    let bg = background.to_array();
    img.pixels
        .iter()
        .map(|p| {
            let [r, g, b, a] = p.to_array();
            let k = 1.0 - a as f32 / 255.0;
            let mix = |c: u8, under: u8| (c as f32 + under as f32 * k).round().min(255.0) as u8;
            [mix(r, bg[0]), mix(g, bg[1]), mix(b, bg[2])]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::Rect;

    fn hex(v: u32) -> Color32 {
        Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
    }

    #[test]
    fn interior_and_straight_edges_are_exact() {
        // A 1px border on whole pixels: the border row is exactly the border
        // colour, the next exactly the fill, away from the corners.
        let parts = [Part::Border {
            rect: Rect::from_min_size(pos2(0.0, 0.0), vec2(40.0, 20.0)),
            radius: 4.0,
            thickness: 1.0,
            background: Some(Brush::Solid(hex(0x2E2E2E))),
            border: Some(hex(0x1F1F1F)),
        }];
        let px = render_over(&parts, 40, 20, hex(0x242424));
        assert_eq!(px[20], [0x1F; 3]);
        assert_eq!(px[40 + 20], [0x2E; 3]);
        assert_eq!(px[10 * 40], [0x1F; 3]);
        assert_eq!(px[10 * 40 + 1], [0x2E; 3]);
    }

    #[test]
    fn radius_fits_the_path() {
        // A 40x20 switch track with r10 and a 1px border: the path is 39x19,
        // so its radius is 9.5 and the outer edge a full half-circle of 10.
        let r = RRect::new(Rect::from_min_size(pos2(0.5, 0.5), vec2(39.0, 19.0)), 10.0);
        assert_eq!(r.rad, 9.5);
    }

    #[test]
    fn selected_nav_corner_matches_sharex() {
        // Application settings' selected page (8,93,203,32) on #222222:
        // ShareX shows #195371 one pixel in on the top row (Dark, accent
        // #09AAFF): the r4 fill and the inset outline each cover it ~0.2.
        let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(203.0, 32.0));
        let parts = [
            Part::Border { rect, radius: 4.0, thickness: 0.0, background: Some(Brush::Solid(hex(0x09AAFF))), border: None },
            Part::InsetShadow { rect, radius: 4.0, spread: 1.0, color: hex(0x09AAFF) },
        ];
        let px = render_over(&parts, 203, 32, hex(0x222222));
        let near = |a: [u8; 3], b: u32| a.iter().zip([(b >> 16) as u8, (b >> 8) as u8, b as u8]).all(|(x, y)| x.abs_diff(y) <= 2);
        assert!(near(px[1], 0x195371), "{:02X?}", px[1]);
        assert!(near(px[2], 0x0B9EEA), "{:02X?}", px[2]);
        assert_eq!(px[0], [0x22; 3]);
    }

    #[test]
    fn checkerboard_alternate_cells_are_even() {
        // `ShareX.Brush.Checkered.Small`: the second colour in the tile's
        // top-left and bottom-right 4px squares.
        let board = Brush::Checkered { colors: [hex(0x272727), hex(0x2E2E2E)], cell: 4.0 };
        let parts = [Part::Border { rect: Rect::from_min_size(pos2(0.0, 0.0), vec2(24.0, 24.0)), radius: 4.0, thickness: 0.0, background: Some(board), border: None }];
        let px = render_over(&parts, 24, 24, hex(0x000000));
        assert_eq!(px[2 * 24 + 2], [0x2E; 3]);
        assert_eq!(px[2 * 24 + 6], [0x27; 3]);
        assert_eq!(px[6 * 24 + 6], [0x2E; 3]);
    }

    #[test]
    fn group_content_is_exact_when_lighter() {
        // White text at 40% over #2E2E2E in a 35% layer, on #242424.
        let c = group_content(Color32::from_white_alpha(102), hex(0x2E2E2E), 0.35, hex(0x242424));
        assert_eq!(c.a(), 0);
    }
}
