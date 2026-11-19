//! Text as Avalonia lays it out and outlines it for ShareX's text
//! annotations (`FormattedText` + `BuildGeometry`): HarfBuzz advances
//! (font units times `size / unitsPerEm`, in doubles), lines as
//! `TextLineImpl` measures them, and each glyph's outline as Skia gets it
//! (`SKFont.GetGlyphPath` with no hinting: the TrueType outline scaled),
//! added at its pen position.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use sharex_skia::{FillType, Path};

/// A font file, as Avalonia's font manager finds it for a family, weight
/// and style.
pub struct FontFace {
    data: Arc<Vec<u8>>,
    /// The face's index in a collection (.ttc); 0 for a single font.
    index: u32,
}

impl FontFace {
    fn face(&self) -> Option<ttf_parser::Face<'_>> {
        ttf_parser::Face::parse(&self.data, self.index).ok()
    }
}

/// The Inter Avalonia bundles (`WithInterFont()`), the same files ShareXrs's
/// UI uses: Avalonia's `FontFamily.Default`, and what a family that isn't
/// installed falls back to.
const INTER_REGULAR: &[u8] = include_bytes!("../../../sharex-ui/assets/fonts/Inter-Regular.ttf");
const INTER_BOLD: &[u8] = include_bytes!("../../../sharex-ui/assets/fonts/Inter-Bold.ttf");

fn inter(bold: bool) -> FontFace {
    static FACES: OnceLock<[Arc<Vec<u8>>; 2]> = OnceLock::new();
    let [regular, bold_face] = FACES.get_or_init(|| [Arc::new(INTER_REGULAR.to_vec()), Arc::new(INTER_BOLD.to_vec())]);
    FontFace { data: if bold { bold_face.clone() } else { regular.clone() }, index: 0 }
}

/// Any installed family, as the system lists it (Windows' and the user's
/// font folders, macOS's and Linux's font directories).
fn system_face(family: &str, bold: bool, italic: bool) -> Option<FontFace> {
    static DB: OnceLock<fontdb::Database> = OnceLock::new();
    let db = DB.get_or_init(|| {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        db
    });
    let query = fontdb::Query {
        families: &[fontdb::Family::Name(family)],
        weight: if bold { fontdb::Weight::BOLD } else { fontdb::Weight::NORMAL },
        style: if italic { fontdb::Style::Italic } else { fontdb::Style::Normal },
        ..Default::default()
    };
    let id = db.query(&query)?;
    let (_, index) = db.face_source(id)?;
    let data = db.with_face_data(id, |bytes, _| bytes.to_vec())?;
    Some(FontFace { data: Arc::new(data), index })
}

fn fonts_dir() -> std::path::PathBuf {
    let windir = std::env::var_os("WINDIR").unwrap_or_else(|| "C:\\Windows".into());
    std::path::PathBuf::from(windir).join("Fonts")
}

/// The file of a system family's face (Windows' fonts folder).
fn font_file(family: &str, bold: bool, italic: bool) -> Option<std::path::PathBuf> {
    let base = match family.to_ascii_lowercase().as_str() {
        "segoe ui" => "segoeui",
        "arial" => "arial",
        "consolas" => "consola",
        "calibri" => "calibri",
        _ => return None,
    };
    let suffix = match (base, bold, italic) {
        ("segoeui", false, false) => "",
        ("segoeui", true, false) => "b",
        ("segoeui", false, true) => "i",
        ("segoeui", true, true) => "z",
        (_, false, false) => "",
        (_, true, false) => "bd",
        (_, false, true) => "i",
        (_, true, true) => "bi",
    };
    let p = fonts_dir().join(format!("{base}{suffix}.ttf"));
    p.exists().then_some(p)
}

/// Avalonia's `FontFamily.Default` with `WithInterFont()`: the Inter
/// Avalonia.Fonts.Inter bundles, looked up in `SHAREXRS_INTER_DIR`.
fn default_font_file(bold: bool) -> Option<std::path::PathBuf> {
    let dir = std::env::var_os("SHAREXRS_INTER_DIR")?;
    let p = std::path::PathBuf::from(dir).join(if bold {
        "Inter-Bold.ttf"
    } else {
        "Inter-Regular.ttf"
    });
    p.exists().then_some(p)
}

/// Loads (once) the face for `family` (`$Default` for Avalonia's default),
/// as Avalonia's font manager finds it: the system's family, else the
/// default (Inter), so text never comes out empty on a system without it.
/// Whether `family` is installed (so [`load_face`] won't fall back to Inter
/// for it): tests compared with ShareX's renders skip text in fonts this
/// system hasn't.
pub fn is_installed(family: &str, bold: bool, italic: bool) -> bool {
    family == "$Default" || font_file(family, bold, italic).is_some() || system_face(family, bold, italic).is_some()
}

/// Found faces by (family, bold, italic): the file's bytes and face index,
/// or `None` when the system hasn't the family.
type FaceCache = HashMap<(String, bool, bool), Option<(Arc<Vec<u8>>, u32)>>;

pub fn load_face(family: &str, bold: bool, italic: bool) -> Option<FontFace> {
    static CACHE: OnceLock<Mutex<FaceCache>> = OnceLock::new();
    if family == "$Default" {
        // `SHAREXRS_INTER_DIR` lets the harness point at Avalonia's own files.
        if let Some(path) = default_font_file(bold)
            && let Ok(d) = std::fs::read(path)
        {
            return Some(FontFace { data: Arc::new(d), index: 0 });
        }
        return Some(inter(bold));
    }
    let key = (family.to_ascii_lowercase(), bold, italic);
    let mut cache = CACHE.get_or_init(Default::default).lock().ok()?;
    let found = cache
        .entry(key)
        .or_insert_with(|| {
            font_file(family, bold, italic)
                .and_then(|p| std::fs::read(p).ok())
                .map(|d| (Arc::new(d), 0))
                .or_else(|| system_face(family, bold, italic).map(|f| (f.data, f.index)))
        })
        .clone();
    Some(match found {
        Some((data, index)) => FontFace { data, index },
        None => inter(bold),
    })
}

/// A shaped glyph.
#[derive(Clone, Copy, Debug)]
pub struct Glyph {
    pub id: u16,
    pub advance: f64,
    pub offset_x: f64,
    pub offset_y: f64,
}

/// One laid out line.
#[derive(Clone, Debug)]
pub struct Line {
    pub glyphs: Vec<Glyph>,
    /// Width without trailing whitespace.
    pub width: f64,
    pub width_including_trailing_whitespace: f64,
    pub height: f64,
    pub baseline: f64,
    /// `TextLineMetrics.Start`: the alignment offset.
    pub start: f64,
}

/// `FormattedText`, laid out.
#[derive(Clone, Debug)]
pub struct TextLayout {
    pub lines: Vec<Line>,
    pub size: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
}

impl TextLayout {
    /// `FormattedText.Width`: the widest line.
    pub fn width(&self) -> f64 {
        self.lines.iter().map(|l| l.width).fold(0.0, f64::max)
    }

    /// `FormattedText.Height`.
    pub fn height(&self) -> f64 {
        self.lines.iter().map(|l| l.height).sum()
    }
}

/// Font metrics as Avalonia's `GlyphTypeface.Metrics` reads them
/// (HarfBuzz's: OS/2 typo metrics when USE_TYPO_METRICS is set, else
/// hhea), in font units, ascent negative.
fn metrics(face: &ttf_parser::Face) -> (f64, f64, f64, f64) {
    let em = face.units_per_em() as f64;
    let use_typo = face
        .tables()
        .os2
        .map(|t| t.use_typographic_metrics())
        .unwrap_or(false);
    let (asc, desc, gap) = if use_typo {
        (
            face.typographic_ascender().unwrap_or(0),
            face.typographic_descender().unwrap_or(0),
            face.typographic_line_gap().unwrap_or(0),
        )
    } else {
        (face.ascender(), face.descender(), face.line_gap())
    };
    (em, -(asc as f64), -(desc as f64), gap as f64)
}

fn shape(font: &FontFace, text: &str, scale: f64) -> Vec<Glyph> {
    let Some(fref) = harfrust::FontRef::new(&font.data).ok() else {
        return Vec::new();
    };
    let data = harfrust::ShaperData::new(&fref);
    let shaper = data.shaper(&fref).build();
    let mut buffer = harfrust::UnicodeBuffer::new();
    buffer.push_str(text);
    buffer.guess_segment_properties();
    let out = shaper.shape(buffer, harfrust::ShapeOptions::new());
    out.glyph_infos()
        .iter()
        .zip(out.glyph_positions())
        .map(|(i, p)| Glyph {
            id: i.glyph_id as u16,
            advance: p.x_advance as f64 * scale,
            offset_x: p.x_offset as f64 * scale,
            offset_y: -(p.y_offset as f64) * scale,
        })
        .collect()
}

/// Lays out `text` (lines split at line breaks; no wrapping yet) at
/// `size`, aligned within `paragraph_width` (infinite: no alignment).
pub fn layout(
    font: &FontFace,
    text: &str,
    size: f64,
    align: Align,
    paragraph_width: f64,
) -> Option<TextLayout> {
    let face = font.face()?;
    let (em, ascent, descent, gap) = metrics(&face);
    let scale = size / em;
    let (asc, desc, gap) = (ascent * scale, descent * scale, gap * scale);
    let mut lines = Vec::new();
    for line_text in text.split('\n') {
        let line_text = line_text.trim_end_matches('\r');
        let glyphs = shape(font, line_text, scale);
        let total: f64 = glyphs.iter().map(|g| g.advance).sum();
        let trailing_ws: f64 = {
            let n = line_text
                .chars()
                .rev()
                .take_while(|c| c.is_whitespace())
                .count();
            glyphs.iter().rev().take(n).map(|g| g.advance).sum()
        };
        let width = total - trailing_ws;
        let half_gap = gap * 0.5;
        let height = desc - asc + gap;
        let baseline = -asc + half_gap;
        let start = if paragraph_width.is_infinite() {
            0.0
        } else {
            match align {
                Align::Center => ((paragraph_width - width) / 2.0).max(0.0),
                Align::Right => (paragraph_width - total).max(0.0),
                Align::Left => 0.0,
            }
        };
        lines.push(Line {
            glyphs,
            width,
            width_including_trailing_whitespace: total,
            height,
            baseline,
            start,
        });
    }
    Some(TextLayout { lines, size })
}

/// The outline of glyph `id` at `size`, as `SKFont.GetGlyphPath` returns
/// it (y down, origin on the baseline).
fn glyph_path(face: &ttf_parser::Face, id: u16, scale: f32) -> Path {
    // DirectWrite's outline (through Skia's geometry sink) has no closing
    // line back to a contour's start: Close implies it.
    struct B {
        p: Path,
        s: f32,
        start: (f32, f32),
        pending: Option<(f32, f32)>,
    }
    impl B {
        fn flush(&mut self) {
            if let Some((x, y)) = self.pending.take() {
                self.p.line_to(x, y);
            }
        }
    }
    impl ttf_parser::OutlineBuilder for B {
        fn move_to(&mut self, x: f32, y: f32) {
            self.flush();
            self.start = (x * self.s, -y * self.s);
            self.p.move_to(x * self.s, -y * self.s);
        }
        fn line_to(&mut self, x: f32, y: f32) {
            self.flush();
            self.pending = Some((x * self.s, -y * self.s));
        }
        fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
            self.flush();
            self.p
                .quad_to(x1 * self.s, -y1 * self.s, x * self.s, -y * self.s);
        }
        fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
            self.flush();
            self.p.cubic_to(
                x1 * self.s,
                -y1 * self.s,
                x2 * self.s,
                -y2 * self.s,
                x * self.s,
                -y * self.s,
            );
        }
        fn close(&mut self) {
            if self.pending == Some(self.start) {
                self.pending = None;
            }
            self.flush();
            self.p.close();
        }
    }
    let mut b = B {
        p: Path::new(),
        s: scale,
        start: (0.0, 0.0),
        pending: None,
    };
    face.outline_glyph(ttf_parser::GlyphId(id), &mut b);
    b.p
}

/// `FormattedText.BuildGeometry(origin)`: every glyph's outline at its
/// pen position, in one non-zero path.
pub fn build_geometry(font: &FontFace, layout: &TextLayout, origin: (f64, f64)) -> Option<Path> {
    let face = font.face()?;
    let scale = (layout.size / face.units_per_em() as f64) as f32;
    let mut out = Path::new();
    out.fill_type = FillType::Winding;
    let mut line_y = origin.1;
    for line in &layout.lines {
        let mut x = origin.0 + line.start;
        let y = line_y + line.baseline;
        for g in &line.glyphs {
            let p = glyph_path(&face, g.id, scale);
            if !p.is_empty() {
                out.add_path(&p.translated(x as f32, y as f32));
            }
            x += g.advance;
        }
        line_y += line.height;
    }
    Some(out)
}

/// `OutlinedTextControl.Render` for a `TextAnnotation`: the text (wrapped
/// to the annotation's width less its padding, aligned) as glyph
/// outlines, stroked (round joins) then filled, clipped to the control.
pub fn draw_text_annotation(
    pm: &mut sharex_skia::Pixmap,
    c: &crate::model::Common,
    p: &crate::model::TextProps,
) {
    use super::shapes::{Pen, draw_geometry_clipped};
    const TEXT_PADDING: f64 = 8.0;
    if p.text.is_empty() {
        return;
    }
    let Some(font) = load_face(&p.font_family, p.is_bold, p.is_italic) else {
        return;
    };
    // TextAnnotation.GetBounds: at least 10x10.
    let (mut l, mut t, mut r, mut b) = super::shapes::bounds(c);
    if r - l < 10.0 {
        r = l + 10.0;
    }
    if b - t < 10.0 {
        b = t + 10.0;
    }
    let _ = (&mut l, &mut t);
    let a = super::layout::arrange(
        l as f64,
        t as f64,
        ((r - l) as f64).max(1.0),
        ((b - t) as f64).max(1.0),
    );
    let ctm = super::layout::visual_transform(&a, c.rotation_angle);

    let stroke = c.stroke_width as f64;
    let hp = TEXT_PADDING + stroke;
    let vp = TEXT_PADDING + stroke;
    let max_text_width = (a.width - hp * 2.0).max(0.0);
    let align = match p.horizontal_alignment {
        crate::model::TextHorizontalAlignment::Left => Align::Left,
        crate::model::TextHorizontalAlignment::Right => Align::Right,
        _ => Align::Center,
    };
    let paragraph = if max_text_width > 0.0 {
        max_text_width
    } else {
        f64::INFINITY
    };
    let Some(lay) = layout(&font, &p.text, p.font_size as f64, align, paragraph) else {
        return;
    };
    let text_height = lay.height();
    let mut origin_y = vp.max((a.height - text_height) / 2.0);
    origin_y = origin_y.min(vp.max(a.height - text_height - vp));
    let Some(geometry) = build_geometry(&font, &lay, (hp, origin_y)) else {
        return;
    };

    // ClipToBounds: the control's rectangle (in device space when unrotated).
    let clip = (c.rotation_angle == 0.0).then(|| {
        let (ox, oy) = super::layout::device_offset();
        sharex_skia::IRect::from_xywh((a.x + ox) as i32, (a.y + oy) as i32, a.width as i32, a.height as i32)
    });
    let text_color = super::color::parse_color(&p.text_color);
    let stroke_color = super::color::parse_color(&c.stroke_color);
    if c.stroke_width > 0.0 && stroke_color.a > 0 {
        let pen = Pen {
            color: stroke_color,
            thickness: stroke,
            dashes: Vec::new(),
            cap: sharex_skia::Cap::Butt,
            join: sharex_skia::Join::Round,
        };
        draw_geometry_clipped(pm, &ctm, None, &geometry, Some(&pen), &geometry, clip);
    }
    if text_color.a > 0 {
        draw_geometry_clipped(pm, &ctm, Some(text_color), &geometry, None, &geometry, clip);
    }
}

/// `SKTypeface.Default` (Segoe UI on Windows) measured as `SKFont` does:
/// the advance width of `text` and `descent - ascent`, at `size`.
pub fn default_typeface_measure(text: &str, size: f32) -> Option<(f32, f32)> {
    let font = load_face("Segoe UI", false, false)?;
    let face = font.face()?;
    let em = face.units_per_em() as f32;
    let scale = size / em;
    let mut width = 0.0f32;
    for ch in text.chars() {
        let g = face.glyph_index(ch)?;
        width += face.glyph_hor_advance(g).unwrap_or(0) as f32 * scale;
    }
    // DWRITE_FONT_METRICS: usWinAscent / usWinDescent.
    let os2 = face.tables().os2?;
    let ascent = -(os2.windows_ascender() as f32) * scale;
    let descent = (os2.windows_descender().unsigned_abs() as f32) * scale;
    Some((width, descent - ascent))
}

/// `StepTypeFormatter.Format`.
pub fn step_text(value: i32, step_type: &crate::model::StepType) -> String {
    use crate::model::StepType;
    if value <= 0 {
        return value.to_string();
    }
    let alpha = |upper: bool| {
        let mut s = Vec::new();
        let mut cur = value;
        while cur > 0 {
            cur -= 1;
            s.insert(0, (b'A' + (cur % 26) as u8) as char);
            cur /= 26;
        }
        let s: String = s.into_iter().collect();
        if upper { s } else { s.to_lowercase() }
    };
    let roman = |upper: bool| {
        const R: [(i32, &str); 13] = [
            (1000, "M"),
            (900, "CM"),
            (500, "D"),
            (400, "CD"),
            (100, "C"),
            (90, "XC"),
            (50, "L"),
            (40, "XL"),
            (10, "X"),
            (9, "IX"),
            (5, "V"),
            (4, "IV"),
            (1, "I"),
        ];
        let mut s = String::new();
        let mut rem = value;
        for (v, sym) in R {
            while rem >= v {
                s.push_str(sym);
                rem -= v;
            }
        }
        if upper { s } else { s.to_lowercase() }
    };
    match step_type {
        StepType::UppercaseLetter => alpha(true),
        StepType::LowercaseLetter => alpha(false),
        StepType::UppercaseRoman => roman(true),
        StepType::LowercaseRoman => roman(false),
        _ => value.to_string(),
    }
}

/// `NumberAnnotation.Radius` (`CalculateRadius`).
pub fn step_radius(p: &crate::model::NumberProps) -> f32 {
    let base = 12.0f32.max(p.font_size * 0.7);
    let text = step_text(p.number, &p.step_type);
    let (w, h) =
        default_typeface_measure(&text, (p.font_size * 0.6).max(1.0)).unwrap_or((0.0, 0.0));
    let padding = 6.0f32.max(p.font_size * 0.35);
    let measured = w.max(h) * 0.5 + padding;
    base.max(measured)
}

/// The `CombinedGeometry(Union, body, tail)` ShareX draws when an annotation
/// has a tail: `SKPath.Op` of the body and the tail triangle (a
/// `StreamGeometry`: even-odd, closed). `None` when the op fails, which
/// Avalonia draws as nothing.
fn union_with_tail(
    body: &Path,
    tail: (crate::model::Point, crate::model::Point, crate::model::Point),
    origin: (f32, f32),
) -> Option<Path> {
    let mut tri = Path::new();
    tri.fill_type = sharex_skia::FillType::EvenOdd;
    // ToRenderPoint: the float difference, as a double, back to float
    let pt = |q: crate::model::Point| (q.x - origin.0, q.y - origin.1);
    let (s, t, e) = (pt(tail.0), pt(tail.1), pt(tail.2));
    tri.move_to(s.0, s.1);
    tri.line_to(t.0, t.1);
    tri.line_to(e.0, e.1);
    tri.close();
    sharex_skia::pathops::op(body, &tri, sharex_skia::pathops::PathOp::Union)
}

/// `StepControl.Render`: the circle (united with its tail) filled then
/// stroked, the number centred in it. The control spans the interaction
/// bounds (`GetInteractionBounds`: the circle and the tail). The number is
/// drawn from Inter's outlines; ShareX draws it as glyph masks rasterised
/// by DirectWrite, so its edges differ.
/// The `StepControl`'s arranged size and transform (its body and tail's
/// bounds), as a drop shadow needs them.
pub(super) fn step_frame(c: &crate::model::Common, p: &crate::model::NumberProps) -> ((f64, f64), super::layout::AMatrix) {
    use crate::editor::geometry::{Rect, step_tail_points, triangle_bounds};
    let r = step_radius(p);
    let (sx, sy) = (c.start_point.x, c.start_point.y);
    let body_bounds = Rect::new(sx - r, sy - r, sx + r, sy + r);
    let render_bounds = match step_tail_points(c.start_point, r, p) {
        Some((s, t, e)) => body_bounds.union(&triangle_bounds(s, t, e)),
        None => body_bounds,
    };
    let a = super::layout::arrange(
        render_bounds.l as f64,
        render_bounds.t as f64,
        render_bounds.width() as f64,
        render_bounds.height() as f64,
    );
    ((a.width, a.height), super::layout::visual_transform(&a, 0.0))
}

/// The `SpeechBalloonControl`'s arranged size and transform (rotated about
/// its body's centre when it has a tail), as a drop shadow needs them.
pub(super) fn balloon_frame(
    c: &crate::model::Common,
    p: &crate::model::SpeechBalloonProps,
) -> ((f64, f64), super::layout::AMatrix) {
    use crate::editor::geometry::{Rect, balloon_tail_points, triangle_bounds};
    let (l, t, r, b) = super::shapes::bounds(c);
    let body_bounds = Rect::new(l, t, r, b);
    let tail = balloon_tail_points(&body_bounds, p);
    let render_bounds = match tail {
        Some((s, tt, e)) => body_bounds.union(&triangle_bounds(s, tt, e)),
        None => body_bounds,
    };
    let a = super::layout::arrange(
        render_bounds.l as f64,
        render_bounds.t as f64,
        render_bounds.width() as f64,
        render_bounds.height() as f64,
    );
    let ctm = if tail.is_some() {
        let (rw, rh) = (render_bounds.width(), render_bounds.height());
        let origin = if rw <= 0.0 || rh <= 0.0 {
            (0.5, 0.5)
        } else {
            (
                ((body_bounds.mid_x() - render_bounds.l) / rw) as f64,
                ((body_bounds.mid_y() - render_bounds.t) / rh) as f64,
            )
        };
        super::layout::visual_transform_origin(&a, c.rotation_angle, origin)
    } else {
        super::layout::visual_transform(&a, c.rotation_angle)
    };
    ((a.width, a.height), ctm)
}

pub fn draw_step(
    pm: &mut sharex_skia::Pixmap,
    c: &crate::model::Common,
    p: &crate::model::NumberProps,
) {
    use super::shapes::{Pen, draw_geometry};
    use crate::editor::geometry::{Rect, step_tail_points, triangle_bounds};
    let r = step_radius(p);
    let (sx, sy) = (c.start_point.x, c.start_point.y);
    let body_bounds = Rect::new(sx - r, sy - r, sx + r, sy + r);
    let tail = step_tail_points(c.start_point, r, p);
    let render_bounds = match tail {
        Some((s, t, e)) => body_bounds.union(&triangle_bounds(s, t, e)),
        None => body_bounds,
    };
    let a = super::layout::arrange(
        render_bounds.l as f64,
        render_bounds.t as f64,
        render_bounds.width() as f64,
        render_bounds.height() as f64,
    );
    let ctm = super::layout::visual_transform(&a, 0.0);
    let off_x = (body_bounds.l - render_bounds.l) as f64;
    let off_y = (body_bounds.t - render_bounds.t) as f64;
    let width = (body_bounds.width() as f64).max(2.0);
    let height = (body_bounds.height() as f64).max(2.0);
    let mut body = Path::new();
    body.add_oval(
        sharex_skia::Rect::from_ltrb(off_x as f32, off_y as f32, (off_x + width) as f32, (off_y + height) as f32),
        sharex_skia::Direction::Cw,
    );
    let geometry = match tail {
        Some(t) => union_with_tail(&body, t, (render_bounds.l, render_bounds.t)),
        None => Some(body),
    };
    if let Some(geometry) = &geometry {
        let fill = super::color::parse_color(&c.fill_color);
        draw_geometry(pm, &ctm, Some(fill), geometry, None, geometry);
        let pen = Pen {
            color: super::color::parse_color(&c.stroke_color),
            thickness: c.stroke_width as f64,
            dashes: Vec::new(),
            cap: sharex_skia::Cap::Butt,
            join: sharex_skia::Join::Miter,
        };
        draw_geometry(pm, &ctm, None, geometry, Some(&pen), geometry);
    }

    let text = step_text(p.number, &p.step_type);
    let Some(font) = load_face("$Default", p.is_bold, false) else {
        return;
    };
    let size = p.font_size as f64 * 0.6;
    let Some(lay) = layout(&font, &text, size, Align::Left, f64::INFINITY) else {
        return;
    };
    let tx = off_x + (width - lay.width()) / 2.0;
    let ty = off_y + (height - lay.height()) / 2.0;
    if let Some(geometry) = build_geometry(&font, &lay, (tx, ty)) {
        let color = super::color::parse_color(&p.text_color);
        draw_geometry(pm, &ctm, Some(color), &geometry, None, &geometry);
    }
}

/// `SpeechBalloonControl.Render`: the rounded body (united with its tail)
/// filled then stroked, and the text centred in the body. The control spans
/// the interaction bounds and rotates about the body's centre. Text is drawn
/// from outlines (ShareX draws glyph masks).
pub fn draw_balloon(
    pm: &mut sharex_skia::Pixmap,
    c: &crate::model::Common,
    p: &crate::model::SpeechBalloonProps,
) {
    use super::shapes::{Pen, StreamGeometry, draw_geometry};
    use crate::editor::geometry::{Rect, balloon_tail_points, triangle_bounds};
    const TEXT_PADDING: f64 = 12.0;
    let (l, t, r, b) = super::shapes::bounds(c);
    let body_bounds = Rect::new(l, t, r, b);
    let tail = balloon_tail_points(&body_bounds, p);
    let render_bounds = match tail {
        Some((s, tt, e)) => body_bounds.union(&triangle_bounds(s, tt, e)),
        None => body_bounds,
    };
    let a = super::layout::arrange(
        render_bounds.l as f64,
        render_bounds.t as f64,
        render_bounds.width() as f64,
        render_bounds.height() as f64,
    );
    let ctm = if tail.is_some() {
        // GetRelativeCenterOrigin(GetBounds(), interaction bounds), in floats
        let (rw, rh) = (render_bounds.width(), render_bounds.height());
        let origin = if rw <= 0.0 || rh <= 0.0 {
            (0.5, 0.5)
        } else {
            (
                ((body_bounds.mid_x() - render_bounds.l) / rw) as f64,
                ((body_bounds.mid_y() - render_bounds.t) / rh) as f64,
            )
        };
        super::layout::visual_transform_origin(&a, c.rotation_angle, origin)
    } else {
        super::layout::visual_transform(&a, c.rotation_angle)
    };
    let off_x = (body_bounds.l - render_bounds.l) as f64;
    let off_y = (body_bounds.t - render_bounds.t) as f64;
    let width = (body_bounds.width() as f64).max(20.0);
    let height = (body_bounds.height() as f64).max(20.0);
    let radius = (p.corner_radius as f64).clamp(0.0, ((width.min(height)) / 2.0).trunc());
    let mut g = StreamGeometry::new();
    let (left, top, right, bottom) = (off_x, off_y, off_x + width, off_y + height);
    if radius <= 0.0 {
        g.begin_figure(left, top, true);
        g.line_to(right, top);
        g.line_to(right, bottom);
        g.line_to(left, bottom);
        g.end_figure(true);
    } else {
        g.begin_figure(left + radius, top, true);
        g.line_to(right - radius, top);
        g.arc_to(right, top + radius, radius, radius, false, true);
        g.line_to(right, bottom - radius);
        g.arc_to(right - radius, bottom, radius, radius, false, true);
        g.line_to(left + radius, bottom);
        g.arc_to(left, bottom - radius, radius, radius, false, true);
        g.line_to(left, top + radius);
        g.arc_to(left + radius, top, radius, radius, false, true);
        g.end_figure(true);
    }
    let (stroke_path, fill_path) = g.finish();
    let pen = Pen {
        color: super::color::parse_color(&c.stroke_color),
        thickness: c.stroke_width as f64,
        dashes: Vec::new(),
        cap: sharex_skia::Cap::Butt,
        join: sharex_skia::Join::Miter,
    };
    let fill = super::color::parse_color(&c.fill_color);
    match tail {
        Some(tt) => {
            // A closed, filled figure: Avalonia's fill path is the stroke
            // path, so one op serves both.
            if let Some(geometry) = union_with_tail(&stroke_path, tt, (render_bounds.l, render_bounds.t)) {
                draw_geometry(pm, &ctm, Some(fill), &geometry, None, &geometry);
                draw_geometry(pm, &ctm, None, &geometry, Some(&pen), &geometry);
            }
        }
        None => {
            draw_geometry(pm, &ctm, Some(fill), &fill_path, None, &stroke_path);
            draw_geometry(pm, &ctm, None, &fill_path, Some(&pen), &stroke_path);
        }
    }

    if p.text.is_empty() {
        return;
    }
    let family = if p.font_family.trim().is_empty() {
        "Segoe UI"
    } else {
        p.font_family.as_str()
    };
    let Some(font) = load_face(family, p.is_bold, p.is_italic) else {
        return;
    };
    let align = match p.horizontal_alignment {
        crate::model::TextHorizontalAlignment::Left => Align::Left,
        crate::model::TextHorizontalAlignment::Right => Align::Right,
        _ => Align::Center,
    };
    let max_text_width = (width - TEXT_PADDING * 2.0).max(0.0);
    let Some(lay) = layout(&font, &p.text, p.font_size as f64, align, max_text_width) else {
        return;
    };
    let tx = off_x + TEXT_PADDING;
    let mut ty = off_y + TEXT_PADDING.max((height - lay.height()) / 2.0);
    ty = ty.min(off_y + height - lay.height() - TEXT_PADDING);
    if let Some(geometry) = build_geometry(&font, &lay, (tx, ty)) {
        draw_geometry(
            pm,
            &ctm,
            Some(super::color::parse_color(&p.text_color)),
            &geometry,
            None,
            &geometry,
        );
    }
}
