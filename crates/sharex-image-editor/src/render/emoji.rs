//! `WindowsEmojiBitmapRenderer`: the bitmap an emoji annotation shows.
//!
//! ShareX draws the emoji's text with Direct2D (`DrawTextLayout` with
//! `EnableColorFont`, grayscale anti-aliasing) in Segoe UI Emoji, whose
//! glyphs are COLRv1 paint graphs (outlines clipped and filled with solid
//! colours and linear and radial gradients), on a transparent canvas twice
//! the sticker's size at 1.32 times its size; trims the transparent edges;
//! and draws what's left, Catmull-Rom, centred in a `size` x `size` square
//! with 14 px of padding (`RenderStickerBitmap`).
//!
//! ShareXrs paints the same font's COLRv1 graphs itself (shaped with
//! HarfBuzz, laid out as DirectWrite centres the line), then trims and fits
//! as ShareX does. Direct2D's rasteriser isn't Skia's, so edges and
//! gradients differ slightly (a "pending" category, like DirectWrite's
//! glyph masks). Without Segoe UI Emoji (macOS, Linux) any installed
//! COLR emoji font is used; ShareX falls back to Skia's default typeface
//! there, which has no emoji.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use sharex_skia::raster::{Bitmap, Canvas, Paint, Rect as RRect, Sampling, Shader};
use sharex_skia::{Color, Path, Pixmap};
use ttf_parser::colr::{ClipBox, CompositeMode, GradientExtend, Painter};
use ttf_parser::{GlyphId, RgbaColor};

/// `EmojiFontFamily`, then the colour fonts other systems have.
const FAMILIES: [&str; 3] = ["Segoe UI Emoji", "Noto Color Emoji", "Twemoji Mozilla"];
const STICKER_PADDING: u32 = 14;
const RAW_CANVAS_SCALE: f32 = 2.0;
const RAW_FONT_SCALE: f32 = 1.32;

/// `EmojiCatalogService.ToGlyph`: hex code points separated by spaces or
/// dashes, as text (unparseable parts skipped).
pub fn to_glyph(sequence: &str) -> String {
    sequence
        .split([' ', '-'])
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .filter_map(|p| u32::from_str_radix(p, 16).ok())
        .filter_map(char::from_u32)
        .collect()
}

/// The emoji font's bytes and face index.
fn font() -> Option<&'static (Vec<u8>, u32)> {
    static FONT: OnceLock<Option<(Vec<u8>, u32)>> = OnceLock::new();
    FONT.get_or_init(|| {
        #[cfg(windows)]
        {
            let windir = std::env::var_os("WINDIR").unwrap_or_else(|| "C:\\Windows".into());
            let p = std::path::PathBuf::from(windir).join("Fonts").join("seguiemj.ttf");
            if let Ok(d) = std::fs::read(p) {
                return Some((d, 0));
            }
        }
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        for family in FAMILIES {
            let query = fontdb::Query {
                families: &[fontdb::Family::Name(family)],
                ..Default::default()
            };
            let Some(id) = db.query(&query) else { continue };
            let Some((_, index)) = db.face_source(id) else { continue };
            let Some(data) = db.with_face_data(id, |bytes, _| bytes.to_vec()) else { continue };
            let colr = ttf_parser::Face::parse(&data, index).is_ok_and(|f| f.tables().colr.is_some());
            if colr {
                return Some((data, index));
            }
        }
        None
    })
    .as_ref()
}

/// Whether an emoji font is installed (Segoe UI Emoji, or another COLR
/// font off Windows).
pub fn is_available() -> bool {
    font().is_some()
}

/// `RenderStickerBitmap(sequence, size)`: the emoji, trimmed and fitted in
/// a `size` x `size` square (cached, as ShareX caches its stickers).
pub fn sticker_bitmap(sequence: &str, size: u32) -> Option<Arc<Pixmap>> {
    if sequence.trim().is_empty() || size == 0 {
        return None;
    }
    static CACHE: OnceLock<Mutex<HashMap<(String, u32), Option<Arc<Pixmap>>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    let key = (sequence.to_string(), size);
    if let Some(v) = cache.lock().unwrap().get(&key) {
        return v.clone();
    }
    let v = render_square(sequence, size, STICKER_PADDING).map(Arc::new);
    let mut c = cache.lock().unwrap();
    if c.len() >= 256 {
        c.clear();
    }
    c.insert(key, v.clone());
    v
}

/// `RenderSquareBitmap`.
fn render_square(sequence: &str, size: u32, padding: u32) -> Option<Pixmap> {
    let glyph = to_glyph(sequence);
    if glyph.is_empty() {
        return None;
    }
    let raw_canvas = ((size as f32 * RAW_CANVAS_SCALE).ceil() as u32).max(64);
    let raw_font = (size as f32 * RAW_FONT_SCALE).max(24.0);
    let raw = render_raw(&glyph, raw_canvas, raw_font)?;
    let trimmed = trim(&raw).unwrap_or(raw);
    Some(fit_into_square(&trimmed, size, padding))
}

/// `TrimTransparentBounds`: the pixels inside the bounds of everything not
/// fully transparent (a same-size draw: a copy).
fn trim(src: &Pixmap) -> Option<Pixmap> {
    let (w, h) = (src.width() as i32, src.height() as i32);
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (w, h, -1, -1);
    for y in 0..h {
        for x in 0..w {
            if src.pixel(x, y) >> 24 != 0 {
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
    }
    if max_x < min_x || max_y < min_y {
        return None;
    }
    crate::editor::image_ops::crop(src, min_x, min_y, max_x - min_x + 1, max_y - min_y + 1)
}

/// `FitIntoSquare`: scaled (Catmull-Rom, anti-aliased) to fit inside the
/// padding, centred.
fn fit_into_square(src: &Pixmap, size: u32, padding: u32) -> Pixmap {
    let available = (size as f32 - (padding * 2) as f32).max(1.0);
    let scale = (available / src.width() as f32).min(available / src.height() as f32);
    let draw_w = (src.width() as f32 * scale).max(1.0);
    let draw_h = (src.height() as f32 * scale).max(1.0);
    let left = (size as f32 - draw_w) / 2.0;
    let top = (size as f32 - draw_h) / 2.0;
    let mut out = Bitmap::new(size, size);
    {
        let mut canvas = Canvas::new(&mut out);
        let image = Bitmap::from_premultiplied(src.width(), src.height(), src.to_premul_rgba());
        let mut paint = Paint::shader(Shader::Color([0, 0, 0, 255]));
        paint.anti_alias = true;
        canvas.draw_image_rect(
            &image,
            None,
            RRect::xywh(left, top, draw_w, draw_h),
            Sampling::CatmullRom,
            Some(&paint),
        );
    }
    Pixmap::from_premul_rgba(size, size, out.data())
}

/// `RenderWithDirect2D`: the emoji text drawn as `DrawTextLayout` places
/// it in a `canvas` x `canvas` layout box. ShareX centres the text format
/// only after creating the layout, which keeps the format's defaults: the
/// line starts at the left and top (DirectWrite's line: the font's Windows
/// ascent), its baseline on a whole pixel.
pub fn render_raw(text: &str, canvas: u32, font_size: f32) -> Option<Pixmap> {
    let (data, index) = font()?;
    let face = ttf_parser::Face::parse(data, *index).ok()?;
    let fref = harfrust::FontRef::from_index(data, *index).ok()?;
    let shaper_data = harfrust::ShaperData::new(&fref);
    let shaper = shaper_data.shaper(&fref).build();
    let mut buffer = harfrust::UnicodeBuffer::new();
    buffer.push_str(text);
    buffer.guess_segment_properties();
    let out = shaper.shape(buffer, harfrust::ShapeOptions::new());
    let em = face.units_per_em() as f32;
    let scale = font_size / em;
    let ascent = match face.tables().os2 {
        Some(os2) => os2.windows_ascender() as f32,
        None => face.ascender() as f32,
    };
    let baseline = (ascent * scale).round();
    let mut pen_x = 0.0;

    let mut painter = ColrPainter::new(&face, canvas);
    for (info, pos) in out.glyph_infos().iter().zip(out.glyph_positions()) {
        let gid = GlyphId(info.glyph_id as u16);
        let x = pen_x + pos.x_offset as f32 * scale;
        let y = baseline - pos.y_offset as f32 * scale;
        // Font units (y up) to the canvas.
        painter.base = Affine([scale, 0.0, 0.0, -scale, x, y]);
        painter.stack = vec![painter.base];
        if face.is_color_glyph(gid) {
            // The palette's foreground is the brush: white.
            face.paint_color_glyph(gid, 0, RgbaColor::new(255, 255, 255, 255), &mut painter);
        } else {
            painter.outline_glyph(gid);
            painter.push_clip();
            painter.paint(ttf_parser::colr::Paint::Solid(RgbaColor::new(255, 255, 255, 255)));
            painter.pop_clip();
        }
        pen_x += pos.x_advance as f32 * scale;
    }
    Some(painter.finish())
}

/// A 2D affine transform `[a, b, c, d, e, f]`: `x' = a x + c y + e`,
/// `y' = b x + d y + f`.
#[derive(Clone, Copy, Debug)]
struct Affine([f32; 6]);

impl Affine {
    fn apply(&self, x: f32, y: f32) -> (f32, f32) {
        let m = &self.0;
        (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5])
    }

    /// `self` after `o` (`o` applies first).
    fn then(&self, o: &Affine) -> Affine {
        let (a, b) = (&self.0, &o.0);
        Affine([
            a[0] * b[0] + a[2] * b[1],
            a[1] * b[0] + a[3] * b[1],
            a[0] * b[2] + a[2] * b[3],
            a[1] * b[2] + a[3] * b[3],
            a[0] * b[4] + a[2] * b[5] + a[4],
            a[1] * b[4] + a[3] * b[5] + a[5],
        ])
    }

    fn invert(&self) -> Option<Affine> {
        let m = &self.0;
        let det = m[0] * m[3] - m[1] * m[2];
        if det.abs() < 1e-12 {
            return None;
        }
        let (a, b, c, d) = (m[3] / det, -m[1] / det, -m[2] / det, m[0] / det);
        Some(Affine([a, b, c, d, -(a * m[4] + c * m[5]), -(b * m[4] + d * m[5])]))
    }
}

/// Paints COLR glyphs into premultiplied float layers.
struct ColrPainter<'a> {
    face: &'a ttf_parser::Face<'a>,
    size: u32,
    base: Affine,
    stack: Vec<Affine>,
    outline: Option<Path>,
    clips: Vec<Vec<f32>>,
    layers: Vec<(Vec<[f32; 4]>, CompositeMode)>,
}

impl<'a> ColrPainter<'a> {
    fn new(face: &'a ttf_parser::Face<'a>, size: u32) -> Self {
        let n = (size * size) as usize;
        Self {
            face,
            size,
            base: Affine([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]),
            stack: Vec::new(),
            outline: None,
            clips: Vec::new(),
            layers: vec![(vec![[0.0; 4]; n], CompositeMode::SourceOver)],
        }
    }

    fn current(&self) -> Affine {
        *self.stack.last().unwrap_or(&self.base)
    }

    /// The coverage of `path` (anti-aliased, non-zero), 0..1 per pixel.
    fn coverage(&self, path: &Path) -> Vec<f32> {
        let mut pm = Pixmap::new(self.size, self.size);
        sharex_skia::draw::fill_path(&mut pm, path, Color::argb(255, 255, 255, 255), None);
        pm.pixels().iter().map(|&c| (c >> 24) as f32 / 255.0).collect()
    }

    fn push_mask(&mut self, mask: Vec<f32>) {
        let mask = match self.clips.last() {
            Some(top) => top.iter().zip(&mask).map(|(a, b)| a * b).collect(),
            None => mask,
        };
        self.clips.push(mask);
    }

    fn finish(mut self) -> Pixmap {
        while self.layers.len() > 1 {
            self.pop_layer();
        }
        let (layer, _) = self.layers.pop().unwrap();
        let mut pm = Pixmap::new(self.size, self.size);
        for (px, c) in pm.pixels_mut().iter_mut().zip(layer) {
            let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u32;
            *px = sharex_skia::pixmap::pack(b(c[3]), b(c[0]), b(c[1]), b(c[2]));
        }
        pm
    }
}

/// A colour stop list, sorted, as straight RGBA floats.
fn stops(iter: impl Iterator<Item = ttf_parser::colr::ColorStop>) -> Vec<(f32, [f32; 4])> {
    let mut v: Vec<(f32, [f32; 4])> = iter
        .map(|s| {
            let c = s.color;
            (
                s.stop_offset,
                [c.red as f32 / 255.0, c.green as f32 / 255.0, c.blue as f32 / 255.0, c.alpha as f32 / 255.0],
            )
        })
        .collect();
    v.sort_by(|a, b| a.0.total_cmp(&b.0));
    v
}

/// The colour line at `t` (extended), premultiplied after interpolation.
fn color_at(stops: &[(f32, [f32; 4])], extend: GradientExtend, t: f32) -> [f32; 4] {
    if stops.is_empty() {
        return [0.0; 4];
    }
    let (first, last) = (stops[0].0, stops[stops.len() - 1].0);
    let span = last - first;
    let t = if span <= 0.0 {
        t
    } else {
        match extend {
            GradientExtend::Pad => t,
            GradientExtend::Repeat => first + (t - first).rem_euclid(span),
            GradientExtend::Reflect => {
                let u = (t - first).rem_euclid(2.0 * span);
                first + if u > span { 2.0 * span - u } else { u }
            }
        }
    };
    let c = if t <= first {
        stops[0].1
    } else if t >= last {
        stops[stops.len() - 1].1
    } else {
        let i = stops.iter().position(|s| s.0 >= t).unwrap_or(stops.len() - 1);
        let (t0, c0) = stops[i - 1];
        let (t1, c1) = stops[i];
        let f = if t1 > t0 { (t - t0) / (t1 - t0) } else { 0.0 };
        [0, 1, 2, 3].map(|k| c0[k] + (c1[k] - c0[k]) * f)
    };
    [c[0] * c[3], c[1] * c[3], c[2] * c[3], c[3]]
}

impl<'a> Painter<'a> for ColrPainter<'a> {
    fn outline_glyph(&mut self, glyph_id: GlyphId) {
        struct Builder {
            path: Path,
            m: Affine,
        }
        impl ttf_parser::OutlineBuilder for Builder {
            fn move_to(&mut self, x: f32, y: f32) {
                let (x, y) = self.m.apply(x, y);
                self.path.move_to(x, y);
            }
            fn line_to(&mut self, x: f32, y: f32) {
                let (x, y) = self.m.apply(x, y);
                self.path.line_to(x, y);
            }
            fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
                let (x1, y1) = self.m.apply(x1, y1);
                let (x, y) = self.m.apply(x, y);
                self.path.quad_to(x1, y1, x, y);
            }
            fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
                let (x1, y1) = self.m.apply(x1, y1);
                let (x2, y2) = self.m.apply(x2, y2);
                let (x, y) = self.m.apply(x, y);
                self.path.cubic_to(x1, y1, x2, y2, x, y);
            }
            fn close(&mut self) {
                self.path.close();
            }
        }
        let mut b = Builder {
            path: Path::new(),
            m: self.current(),
        };
        self.outline = self.face.outline_glyph(glyph_id, &mut b).map(|_| b.path);
    }

    fn paint(&mut self, paint: ttf_parser::colr::Paint<'a>) {
        use ttf_parser::colr::Paint as P;
        let coords = self.face.variation_coordinates();
        let inv = self.current().invert();
        let n = self.size as usize;
        // Each pixel's colour (premultiplied) in the paint's space.
        let shade: Box<dyn Fn(f32, f32) -> [f32; 4]> = match paint {
            P::Solid(c) => {
                let a = c.alpha as f32 / 255.0;
                let px = [c.red as f32 / 255.0 * a, c.green as f32 / 255.0 * a, c.blue as f32 / 255.0 * a, a];
                Box::new(move |_, _| px)
            }
            P::LinearGradient(g) => {
                let s = stops(g.stops(0, coords));
                // The colour line runs p0 -> p3, p3 being p1 projected onto
                // the perpendicular to p0p2 through p0.
                let (p0, p1, p2) = ((g.x0, g.y0), (g.x1, g.y1), (g.x2, g.y2));
                let (nx, ny) = (p2.1 - p0.1, -(p2.0 - p0.0));
                let nn = nx * nx + ny * ny;
                let k = if nn > 0.0 { ((p1.0 - p0.0) * nx + (p1.1 - p0.1) * ny) / nn } else { 0.0 };
                let (dx, dy) = if nn > 0.0 { (nx * k, ny * k) } else { (p1.0 - p0.0, p1.1 - p0.1) };
                let dd = dx * dx + dy * dy;
                let extend = g.extend;
                Box::new(move |x, y| {
                    let t = if dd > 0.0 { ((x - p0.0) * dx + (y - p0.1) * dy) / dd } else { 0.0 };
                    color_at(&s, extend, t)
                })
            }
            P::RadialGradient(g) => {
                let s = stops(g.stops(0, coords));
                let (c0, r0, c1, r1) = ((g.x0, g.y0), g.r0, (g.x1, g.y1), g.r1);
                let extend = g.extend;
                Box::new(move |x, y| {
                    // The largest t with |p - c(t)| = r(t), r(t) >= 0.
                    let (cdx, cdy, dr) = (c1.0 - c0.0, c1.1 - c0.1, r1 - r0);
                    let (px, py) = (x - c0.0, y - c0.1);
                    let a = cdx * cdx + cdy * cdy - dr * dr;
                    let b = px * cdx + py * cdy + r0 * dr;
                    let c = px * px + py * py - r0 * r0;
                    let t = if a.abs() < 1e-6 {
                        if b.abs() < 1e-12 { None } else { Some(c / (2.0 * b)) }
                    } else {
                        let disc = b * b - a * c;
                        if disc < 0.0 {
                            None
                        } else {
                            let sq = disc.sqrt();
                            let (t1, t2) = ((b + sq) / a, (b - sq) / a);
                            let (hi, lo) = if t1 > t2 { (t1, t2) } else { (t2, t1) };
                            if r0 + hi * dr >= 0.0 {
                                Some(hi)
                            } else if r0 + lo * dr >= 0.0 {
                                Some(lo)
                            } else {
                                None
                            }
                        }
                    };
                    t.map_or([0.0; 4], |t| color_at(&s, extend, t))
                })
            }
            P::SweepGradient(g) => {
                let s = stops(g.stops(0, coords));
                let (cx, cy, start, end) = (g.center_x, g.center_y, g.start_angle, g.end_angle);
                let extend = g.extend;
                Box::new(move |x, y| {
                    // Angles counter-clockwise in font space, in half turns.
                    let mut angle = (y - cy).atan2(x - cx).to_degrees() / 180.0;
                    if angle < 0.0 {
                        angle += 2.0;
                    }
                    let t = if end != start { (angle - start) / (end - start) } else { 0.0 };
                    color_at(&s, extend, t)
                })
            }
        };
        let Some(inv) = inv else { return };
        let clip = self.clips.last();
        let layer = &mut self.layers.last_mut().unwrap().0;
        for y in 0..n {
            for x in 0..n {
                let i = y * n + x;
                let cov = clip.map_or(1.0, |c| c[i]);
                if cov <= 0.0 {
                    continue;
                }
                let (fx, fy) = inv.apply(x as f32 + 0.5, y as f32 + 0.5);
                let s = shade(fx, fy);
                let d = &mut layer[i];
                let sa = s[3] * cov;
                for k in 0..4 {
                    d[k] = s[k] * cov + d[k] * (1.0 - sa);
                }
            }
        }
    }

    fn push_clip(&mut self) {
        let mask = match &self.outline {
            Some(p) => self.coverage(p),
            None => vec![0.0; (self.size * self.size) as usize],
        };
        self.push_mask(mask);
    }

    fn push_clip_box(&mut self, clipbox: ClipBox) {
        let m = self.current();
        let mut path = Path::new();
        let corners = [
            (clipbox.x_min, clipbox.y_min),
            (clipbox.x_max, clipbox.y_min),
            (clipbox.x_max, clipbox.y_max),
            (clipbox.x_min, clipbox.y_max),
        ];
        for (i, (x, y)) in corners.into_iter().enumerate() {
            let (x, y) = m.apply(x, y);
            if i == 0 {
                path.move_to(x, y);
            } else {
                path.line_to(x, y);
            }
        }
        path.close();
        let mask = self.coverage(&path);
        self.push_mask(mask);
    }

    fn pop_clip(&mut self) {
        self.clips.pop();
    }

    fn push_layer(&mut self, mode: CompositeMode) {
        let n = (self.size * self.size) as usize;
        self.layers.push((vec![[0.0; 4]; n], mode));
    }

    fn pop_layer(&mut self) {
        if self.layers.len() < 2 {
            return;
        }
        let (src, mode) = self.layers.pop().unwrap();
        let dst = &mut self.layers.last_mut().unwrap().0;
        for (d, s) in dst.iter_mut().zip(src) {
            *d = match mode {
                CompositeMode::SourceIn => s.map(|v| v * d[3]),
                CompositeMode::DestinationIn => d.map(|v| v * s[3]),
                CompositeMode::SourceOut => s.map(|v| v * (1.0 - d[3])),
                CompositeMode::DestinationOut => d.map(|v| v * (1.0 - s[3])),
                CompositeMode::SourceAtop => [0, 1, 2, 3].map(|k| s[k] * d[3] + d[k] * (1.0 - s[3])),
                CompositeMode::DestinationOver => [0, 1, 2, 3].map(|k| d[k] + s[k] * (1.0 - d[3])),
                CompositeMode::Clear => [0.0; 4],
                CompositeMode::Source => s,
                CompositeMode::Destination => *d,
                CompositeMode::Plus => [0, 1, 2, 3].map(|k| (s[k] + d[k]).min(1.0)),
                // Source over, and the separable blends approximated by it.
                _ => [0, 1, 2, 3].map(|k| s[k] + d[k] * (1.0 - s[3])),
            };
        }
    }

    fn push_transform(&mut self, t: ttf_parser::Transform) {
        let m = Affine([t.a, t.b, t.c, t.d, t.e, t.f]);
        let top = self.current().then(&m);
        self.stack.push(top);
    }

    fn pop_transform(&mut self) {
        if self.stack.len() > 1 {
            self.stack.pop();
        }
    }
}
